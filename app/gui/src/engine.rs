//! Background thread that owns the USB devices: applies settings, streams
//! Linked frames and keeps a status snapshot for the UI.

use crate::device::{self, Device, Found};
use crate::sim::SimPort;
use f16_core::effects::{EffectParams, Renderer, Rgb};
use f16_core::layout::{CombinedLayout, MacropadSide, Point};
use f16_core::protocol::{self as proto, DeviceInfo, DeviceKind, LedPoint, LightState};
use hidapi::HidApi;
use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const FRAME_INTERVAL: Duration = Duration::from_millis(33); // ~30 fps
const IDLE_INTERVAL: Duration = Duration::from_millis(50);
const REFRESH_INTERVAL: Duration = Duration::from_millis(1000);
const SCAN_INTERVAL: Duration = Duration::from_millis(2500);
/// How often joined devices get their effect clocks lined up again.
const CLOCK_SYNC_INTERVAL: Duration = Duration::from_secs(10);

pub enum Command {
    /// Built-in effect settings for one device. `save` = write to EEPROM.
    SetState {
        kind: DeviceKind,
        state: LightState,
        save: bool,
    },
    /// Hotkey lock on every connected device.
    SetHotkeyLock(bool),
    /// Macropad layer: 0 = MIDI, 1 = F-keys.
    SetLayer(u8),
    /// Some = PC renders this effect on both devices, None = back to built-in effects.
    SetLinked(Option<EffectParams>),
    SetSide(MacropadSide),
    /// Restart a device into flashing mode (works on stock firmware too).
    Bootloader(DeviceKind),
    /// Joined surface: Some((x_start, x_end)) per device, None = separate.
    /// While joined, the effect clocks are kept in step.
    SetSpan {
        keyboard: Option<(u8, u8)>,
        macropad: Option<(u8, u8)>,
    },
    /// Painted keys for one device (one entry per LED). `save` = keep after unplugging.
    SetPaint {
        kind: DeviceKind,
        leds: Vec<proto::Paint>,
        follow: bool,
        save: bool,
    },
    Rescan,
    Shutdown,
}

#[derive(Clone)]
pub struct DeviceStatus {
    pub info: DeviceInfo,
    pub state: LightState,
    pub streaming: bool,
    /// LED positions, used to match keys to LEDs.
    pub leds: Vec<LedPoint>,
}

#[derive(Clone, Default)]
pub enum Slot {
    #[default]
    Missing,
    /// Found, but it is not running the f16custom firmware.
    Stock,
    Ready(DeviceStatus),
}

#[derive(Clone, Default)]
pub struct Status {
    pub keyboard: Slot,
    pub macropad: Slot,
    pub linked: bool,
    /// Linked mode preview: LED position + color.
    pub preview: Vec<(Point, Rgb)>,
    pub layout_size: (f32, f32),
    pub fps: f32,
    pub message: Option<String>,
}

impl Status {
    pub fn slot(&self, kind: DeviceKind) -> &Slot {
        match kind {
            DeviceKind::Keyboard => &self.keyboard,
            DeviceKind::Macropad => &self.macropad,
        }
    }
    pub fn slot_mut(&mut self, kind: DeviceKind) -> &mut Slot {
        match kind {
            DeviceKind::Keyboard => &mut self.keyboard,
            DeviceKind::Macropad => &mut self.macropad,
        }
    }
}

struct Open {
    dev: Device,
    info: DeviceInfo,
    leds: Vec<LedPoint>,
}

pub struct EngineHandle {
    pub tx: Sender<Command>,
    pub status: Arc<Mutex<Status>>,
    thread: Option<JoinHandle<()>>,
}

impl EngineHandle {
    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }

    /// Restores the built-in effects and stops the thread.
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for EngineHandle {
    // Runs when the window closes: hands the lights back to the devices' saved effects.
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// `demo = true` uses the built-in simulator instead of USB devices.
pub fn start(repaint: impl Fn() + Send + 'static, demo: bool) -> EngineHandle {
    let (tx, rx) = mpsc::channel();
    let status = Arc::new(Mutex::new(Status::default()));
    let st = status.clone();
    let thread = thread::Builder::new()
        .name("hid-engine".into())
        .spawn(move || Engine::new(rx, st, Box::new(repaint), demo).run())
        .expect("spawn engine thread");
    EngineHandle {
        tx,
        status,
        thread: Some(thread),
    }
}

/// Pretend devices for `--demo`.
fn demo_devices(skip: &[DeviceKind]) -> Vec<Found> {
    [
        (DeviceKind::Keyboard, SimPort::keyboard()),
        (DeviceKind::Macropad, SimPort::macropad()),
    ]
    .into_iter()
    .filter(|(kind, _)| !skip.contains(kind))
    .map(|(kind, port)| device::identify(Device::new(kind, Box::new(port))))
    .collect()
}

struct Engine {
    rx: Receiver<Command>,
    status: Arc<Mutex<Status>>,
    repaint: Box<dyn Fn() + Send>,
    api: Option<HidApi>,
    demo: bool,
    open: BTreeMap<u8, Open>, // key: 0 keyboard, 1 macropad
    stock: Vec<DeviceKind>,
    joined: bool,
    last_clock_sync: Instant,
    t0_clock: Instant,
    linked: Option<EffectParams>,
    renderer: Renderer,
    side: MacropadSide,
    layout: CombinedLayout,
    t0: Instant,
    last_scan: Option<Instant>,
    last_refresh: Instant,
    frames: u32,
    fps_timer: Instant,
}

fn key(kind: DeviceKind) -> u8 {
    match kind {
        DeviceKind::Keyboard => 0,
        DeviceKind::Macropad => 1,
    }
}

impl Engine {
    fn new(
        rx: Receiver<Command>,
        status: Arc<Mutex<Status>>,
        repaint: Box<dyn Fn() + Send>,
        demo: bool,
    ) -> Self {
        let api = match if demo { None } else { Some(HidApi::new()) } {
            None => None,
            Some(Ok(a)) => Some(a),
            Some(Err(e)) => {
                status.lock().unwrap().message =
                    Some(format!("Could not start USB HID access: {e}"));
                None
            }
        };
        Self {
            rx,
            status,
            repaint,
            api,
            demo,
            open: BTreeMap::new(),
            stock: Vec::new(),
            joined: false,
            last_clock_sync: Instant::now(),
            t0_clock: Instant::now(),
            linked: None,
            renderer: Renderer::default(),
            side: MacropadSide::default(),
            layout: CombinedLayout::default(),
            t0: Instant::now(),
            last_scan: None,
            last_refresh: Instant::now(),
            frames: 0,
            fps_timer: Instant::now(),
        }
    }

    fn set_message(&self, m: impl Into<String>) {
        self.status.lock().unwrap().message = Some(m.into());
        (self.repaint)();
    }

    fn run(mut self) {
        loop {
            let tick = Instant::now();
            loop {
                match self.rx.try_recv() {
                    Ok(Command::Shutdown) | Err(TryRecvError::Disconnected) => {
                        self.stop_linked();
                        return;
                    }
                    Ok(c) => self.handle(c),
                    Err(TryRecvError::Empty) => break,
                }
            }

            let all_ready = self.open.len() == 2;
            if !all_ready && self.last_scan.is_none_or(|t| t.elapsed() >= SCAN_INTERVAL) {
                self.scan();
            }
            if self.linked.is_some() {
                self.render_frame();
            }
            if self.joined && self.last_clock_sync.elapsed() >= CLOCK_SYNC_INTERVAL {
                self.sync_clocks();
            }
            if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
                self.refresh();
            }

            let interval = if self.linked.is_some() {
                FRAME_INTERVAL
            } else {
                IDLE_INTERVAL
            };
            if let Some(rest) = interval.checked_sub(tick.elapsed()) {
                thread::sleep(rest);
            }
        }
    }

    fn handle(&mut self, c: Command) {
        match c {
            Command::SetState { kind, state, save } => {
                if let Some(o) = self.open.get(&key(kind)) {
                    let res = proto::set_state(&state, save, o.info.effect_count)
                        .map_err(device::DeviceError::from)
                        .and_then(|r| o.dev.transact(&r));
                    match res {
                        Ok(_) => {
                            if save {
                                self.set_message(format!(
                                    "{} settings saved to the device.",
                                    kind.name()
                                ));
                            }
                        }
                        Err(e) => self.fail(kind, e),
                    }
                }
            }
            Command::SetHotkeyLock(on) => {
                let kinds: Vec<DeviceKind> = self.open.values().map(|o| o.dev.kind).collect();
                for kind in kinds {
                    let r = self.open[&key(kind)]
                        .dev
                        .transact(&proto::set_hotkey_lock(on));
                    if let Err(e) = r {
                        self.fail(kind, e);
                    }
                }
                self.refresh();
            }
            Command::SetLayer(layer) => {
                if let Some(o) = self.open.get(&key(DeviceKind::Macropad)) {
                    if let Err(e) = o.dev.transact(&proto::set_layer(layer)) {
                        self.fail(DeviceKind::Macropad, e);
                    }
                    self.refresh();
                }
            }
            Command::SetLinked(Some(params)) => {
                self.renderer.params = params;
                if self.linked.is_none() {
                    self.linked = Some(params);
                    self.start_linked();
                } else {
                    self.linked = Some(params);
                }
            }
            Command::SetLinked(None) => self.stop_linked(),
            Command::SetSide(side) => {
                self.side = side;
                self.rebuild_layout();
            }
            Command::SetSpan { keyboard, macropad } => {
                self.joined = keyboard.is_some() && macropad.is_some();
                for (kind, span) in [
                    (DeviceKind::Keyboard, keyboard),
                    (DeviceKind::Macropad, macropad),
                ] {
                    if let Some(o) = self.open.get(&key(kind)) {
                        let (x0, x1) = span.unwrap_or((0, 224));
                        if let Err(e) = o.dev.transact(&proto::set_span(span.is_some(), x0, x1)) {
                            self.fail(kind, e);
                        }
                    }
                }
                self.sync_clocks();
            }
            Command::SetPaint {
                kind,
                leds,
                follow,
                save,
            } => {
                if let Some(o) = self.open.get(&key(kind)) {
                    let mut msgs = proto::encode_paint(&leds);
                    msgs.push(proto::paint_ctrl(if follow {
                        proto::PaintOp::FollowOn
                    } else {
                        proto::PaintOp::FollowOff
                    }));
                    if save {
                        msgs.push(proto::paint_ctrl(proto::PaintOp::Save));
                    }
                    if let Err(e) = o.dev.transact_many(&msgs) {
                        self.fail(kind, e);
                    }
                }
            }
            Command::Rescan => {
                self.last_scan = None;
            }
            Command::Bootloader(kind) => {
                let sent = match self.open.remove(&key(kind)) {
                    Some(o) => o.dev.send(&proto::bootloader_request()).is_ok(),
                    None => self
                        .api
                        .as_mut()
                        .and_then(|api| device::open_raw(api, kind))
                        .is_some_and(|d| d.send(&proto::bootloader_request()).is_ok()),
                };
                self.rebuild_layout();
                self.set_message(if sent {
                    format!(
                        "{} restarted into flashing mode: drag the .uf2 file onto the new RPI-RP2 drive.",
                        kind.name()
                    )
                } else {
                    format!("Could not reach the {}.", kind.name())
                });
                self.stock.retain(|k| *k != kind);
                self.refresh();
            }
            Command::Shutdown => unreachable!(),
        }
    }

    /// Sends the same clock to both devices back to back so joined effects line up.
    fn sync_clocks(&mut self) {
        self.last_clock_sync = Instant::now();
        let ms = self.t0_clock.elapsed().as_millis() as u32;
        let msg = proto::set_clock(ms);
        for o in self.open.values() {
            let _ = o.dev.send(&msg);
        }
        for o in self.open.values() {
            let _ = o.dev.transact(&proto::state_request());
        }
    }

    fn scan(&mut self) {
        self.last_scan = Some(Instant::now());
        let skip: Vec<DeviceKind> = self.open.values().map(|o| o.dev.kind).collect();
        let found = if self.demo {
            demo_devices(&skip)
        } else if let Some(api) = self.api.as_mut() {
            device::scan(api, &skip)
        } else {
            return;
        };
        let mut stock = Vec::new();
        let mut added = false;
        for f in found {
            match f {
                Found::Ready(dev, info, leds) => {
                    let kind = dev.kind;
                    if self.linked.is_some() {
                        let _ = dev.transact(&proto::set_host_mode(true));
                    }
                    self.open.insert(key(kind), Open { dev, info, leds });
                    added = true;
                }
                Found::Stock(kind) => stock.push(kind),
            }
        }
        self.stock = stock;
        if added {
            self.rebuild_layout();
        }
        self.refresh();
    }

    fn rebuild_layout(&mut self) {
        let kb = self.open.get(&0).map(|o| o.leds.as_slice()).unwrap_or(&[]);
        let pad = self.open.get(&1).map(|o| o.leds.as_slice()).unwrap_or(&[]);
        self.layout = CombinedLayout::new(kb, pad, self.side);
        self.status.lock().unwrap().layout_size = (self.layout.width, self.layout.height);
    }

    /// Something went wrong talking to a device: assume it was unplugged.
    fn fail(&mut self, kind: DeviceKind, e: device::DeviceError) {
        self.open.remove(&key(kind));
        self.rebuild_layout();
        self.status.lock().unwrap().message = Some(format!("{} disconnected: {e}", kind.name()));
        self.refresh();
    }

    fn refresh(&mut self) {
        self.last_refresh = Instant::now();
        let kinds: Vec<DeviceKind> = self.open.values().map(|o| o.dev.kind).collect();
        let mut updates = Vec::new();
        for kind in kinds {
            let o = &self.open[&key(kind)];
            let res = o.dev.info().and_then(|info| {
                let p = o.dev.transact(&proto::state_request())?;
                Ok((info, proto::parse_state(&p)))
            });
            match res {
                Ok((info, (state, streaming))) => {
                    let o = self.open.get_mut(&key(kind)).unwrap();
                    o.info = info.clone();
                    let leds = o.leds.clone();
                    updates.push((
                        kind,
                        Slot::Ready(DeviceStatus {
                            info,
                            state,
                            streaming,
                            leds,
                        }),
                    ));
                }
                Err(e) => {
                    self.fail(kind, e);
                    return; // fail() refreshes again
                }
            }
        }
        {
            let mut st = self.status.lock().unwrap();
            for kind in [DeviceKind::Keyboard, DeviceKind::Macropad] {
                *st.slot_mut(kind) = if self.stock.contains(&kind) {
                    Slot::Stock
                } else {
                    Slot::Missing
                };
            }
            for (kind, slot) in updates {
                *st.slot_mut(kind) = slot;
            }
            st.linked = self.linked.is_some();
        }
        (self.repaint)();
    }

    fn start_linked(&mut self) {
        self.t0 = Instant::now();
        let kinds: Vec<DeviceKind> = self.open.values().map(|o| o.dev.kind).collect();
        for kind in kinds {
            let o = &self.open[&key(kind)];
            // throw away key presses from before Linked mode started
            let _ = o.dev.transact(&proto::key_events_request());
            if let Err(e) = o.dev.transact(&proto::set_host_mode(true)) {
                self.fail(kind, e);
            }
        }
        self.status.lock().unwrap().linked = true;
    }

    fn stop_linked(&mut self) {
        if self.linked.take().is_none() {
            return;
        }
        for o in self.open.values() {
            let _ = o.dev.transact(&proto::set_host_mode(false));
        }
        let mut st = self.status.lock().unwrap();
        st.linked = false;
        st.preview.clear();
        st.fps = 0.0;
        drop(st);
        (self.repaint)();
    }

    fn render_frame(&mut self) {
        let t = self.t0.elapsed().as_secs_f32();
        let kinds: Vec<DeviceKind> = self.open.values().map(|o| o.dev.kind).collect();

        // Key presses -> ripples
        for &kind in &kinds {
            let o = &self.open[&key(kind)];
            match o.dev.transact(&proto::key_events_request()) {
                Ok(p) => {
                    for led in proto::parse_key_events(&p) {
                        if let Some(pt) = self.layout.led(kind, led) {
                            self.renderer.add_ripple(pt, t);
                        }
                    }
                }
                Err(e) => {
                    self.fail(kind, e);
                    return;
                }
            }
        }

        self.renderer.prune(t);
        let mut preview = Vec::new();
        for &kind in &kinds {
            let points = self.layout.points(kind).to_vec();
            let colors = self.renderer.render(&points, t, self.layout.width);
            preview.extend(points.iter().copied().zip(colors.iter().copied()));
            let msgs = proto::encode_frame(&colors);
            if let Err(e) = self.open[&key(kind)].dev.transact_many(&msgs) {
                self.fail(kind, e);
                return;
            }
        }

        self.frames += 1;
        let mut st = self.status.lock().unwrap();
        st.preview = preview;
        if self.fps_timer.elapsed() >= Duration::from_secs(1) {
            st.fps = self.frames as f32 / self.fps_timer.elapsed().as_secs_f32();
            self.frames = 0;
            self.fps_timer = Instant::now();
        }
        drop(st);
        (self.repaint)();
    }
}
