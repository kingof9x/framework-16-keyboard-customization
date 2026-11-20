//! Framework 16 Keyboard Control: controls the f16custom firmware on the Framework Laptop 16
//! keyboard and RGB macropad. The window is built with Slint in the style of the White Rabbit app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window

slint::include_modules!();

mod deck;
mod device;
mod engine;
mod keyedit;
mod render;
mod settings;
mod sim;
mod theme;
mod update;
mod winsys;

/// The notification-area icon (and so "keep running when closed") exists on Windows only.
pub const HAS_TRAY: bool = cfg!(windows);

use deck::{Choice, Dev, KbType, Look, PlacedKey, Preview, Scene};
use engine::{Command, EngineHandle, Slot, Status};
use f16_core::builtin::qmk_hsv;
use f16_core::effects::LinkedEffect;
use f16_core::keys::{self, SideModule, KEYBOARD_SIZE, MODULE_GRID};
use f16_core::protocol::{DeviceKind, LightState, Paint, PaintMode, EFFECT_NAMES};
use settings::Saved;
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Built-in effects grouped for the Show page.
const GROUPS: &[(&str, &[u8])] = &[
    ("Solid", &[1, 2, 5]),
    ("Gradients and bands", &[3, 4, 6, 7, 8, 9, 10, 11]),
    ("Rainbow cycles", &[12, 13, 14, 15, 16, 17, 18, 19]),
    ("Beacons and hues", &[20, 21, 22, 25, 26, 27]),
    ("Random", &[23, 24, 28, 29, 30, 32]),
    (
        "React to typing",
        &[31, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44],
    ),
];

pub struct State {
    engine: EngineHandle,
    bg: winsys::Background,
    ke: keyedit::KeyEdit,
    /// A newer release found on GitHub.
    update: Option<update::Release>,
    /// Answers from the update thread, picked up by `tick`.
    update_inbox: Arc<Mutex<Option<UpdateMsg>>>,
    s: Saved,
    last_saved: Saved,
    preview: Preview,
    keys: Vec<PlacedKey>,
    deck_width_units: f32,
    local_layer: u8,
    pad_key: usize,
    pc_live: bool,
    notice: String,
    adopted: Vec<DeviceKind>,
    /// A lighting change waiting to go to the devices (sent live, a few times a second).
    dirty: bool,
    last_live: Instant,
    /// LED calibration waiting to be saved on a device: (keyboard, macropad) last change time.
    cal_dirty: [Option<Instant>; 2],
    /// Plain white on both devices while matching their brightness.
    white_test: bool,
    /// What the app found plugged in last time (to follow module swaps).
    last_detected: Option<SideModule>,
    windows: theme::Windows,
    last_theme_check: Instant,
    status: Status,
    legends: Rc<VecModel<Legend>>,
}

enum UpdateMsg {
    Checked(Result<Option<update::Release>, String>),
    Downloaded(Result<std::path::PathBuf, String>),
}

impl State {
    fn check_for_update(&mut self, app: &AppWindow) {
        app.global::<App>().set_update_busy(true);
        let inbox = self.update_inbox.clone();
        std::thread::spawn(move || {
            *inbox.lock().unwrap() = Some(UpdateMsg::Checked(update::check()));
        });
    }

    fn install_update(&mut self, app: &AppWindow) {
        let Some(r) = self.update.clone() else { return };
        let g = app.global::<App>();
        match r.installer {
            Some(url) => {
                g.set_update_busy(true);
                g.set_update_status(format!("Downloading version {}…", r.version).into());
                let inbox = self.update_inbox.clone();
                std::thread::spawn(move || {
                    *inbox.lock().unwrap() = Some(UpdateMsg::Downloaded(update::download(&url)));
                });
            }
            None => {
                if !winsys::open(&r.page) {
                    g.set_update_status(format!("Download it from {}", r.page).into());
                }
            }
        }
    }

    /// Handles what the update thread found.
    fn update_answers(&mut self, app: &AppWindow) {
        let Some(msg) = self.update_inbox.lock().unwrap().take() else {
            return;
        };
        let g = app.global::<App>();
        g.set_update_busy(false);
        match msg {
            UpdateMsg::Checked(Ok(Some(r))) => {
                g.set_update_status(
                    format!(
                        "Version {} is available.",
                        r.version.trim_start_matches('v')
                    )
                    .into(),
                );
                g.set_update_available(true);
                self.update = Some(r);
            }
            UpdateMsg::Checked(Ok(None)) => {
                g.set_update_status("You have the latest version.".into());
                g.set_update_available(false);
            }
            UpdateMsg::Checked(Err(e)) | UpdateMsg::Downloaded(Err(e)) => {
                g.set_update_status(e.into());
            }
            UpdateMsg::Downloaded(Ok(path)) => {
                if winsys::open(&path.to_string_lossy()) {
                    // The installer replaces the app, so get out of its way.
                    let _ = slint::quit_event_loop();
                } else {
                    g.set_update_status(format!("Installer saved to {}", path.display()).into());
                }
            }
        }
    }
}

/// "#RRGGBB" (or "RRGGBB") -> hue and saturation on the firmware's 0..255 scale.
fn parse_hex(t: &str) -> Option<(u8, u8)> {
    let t = t.trim().trim_start_matches('#');
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(t, 16).ok()?;
    let rgb = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
    let (h, s, _) = deck::rgb_to_qmk_hsv(rgb);
    Some((h, s))
}

fn color(c: [u8; 3]) -> Color {
    Color::from_rgb_u8(c[0], c[1], c[2])
}

fn hex(c: [u8; 3]) -> SharedString {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]).into()
}

fn ready(status: &Status, kind: DeviceKind) -> Option<&engine::DeviceStatus> {
    match status.slot(kind) {
        Slot::Ready(d) => Some(d),
        _ => None,
    }
}

impl State {
    fn layer(&self) -> u8 {
        ready(&self.status, DeviceKind::Macropad)
            .and_then(|d| d.info.layer)
            .unwrap_or(self.local_layer)
    }

    fn module_name(&self) -> &'static str {
        if self.s.setup.side == SideModule::Numpad {
            "Numpad"
        } else {
            "Macropad"
        }
    }

    // ---------------------------------------------------------------------
    // Sending to the devices
    // ---------------------------------------------------------------------

    fn state_for(look: &Look, d: &engine::DeviceStatus) -> LightState {
        let mode = match look.choice {
            Choice::Builtin(id) => id,
            _ => d.info.paint_only_effect,
        };
        LightState {
            enabled: true,
            mode,
            hue: look.hue,
            sat: look.sat,
            val: look.val,
            speed: look.speed,
        }
    }

    /// Painted keys for one device, one entry per LED.
    fn paint_for(&self, dev: Dev, d: &engine::DeviceStatus) -> Vec<Paint> {
        let (defs, size) = match dev {
            Dev::Keyboard => (keys::keyboard_keys(), KEYBOARD_SIZE),
            Dev::Module => (keys::macropad_keys(self.layer()), MODULE_GRID),
        };
        keys::map_leds_to_keys(&d.leds, &defs, size)
            .into_iter()
            .map(|k| self.s.paint.get(&(dev, k)).copied())
            .collect()
    }

    fn targets(&self) -> Vec<(DeviceKind, Dev, bool)> {
        vec![
            (DeviceKind::Keyboard, Dev::Keyboard, self.s.setup.kb.rgb()),
            (
                DeviceKind::Macropad,
                Dev::Module,
                self.s.setup.side == SideModule::Macropad,
            ),
        ]
    }

    /// Joined: one effect across both devices.
    fn joined(&self) -> bool {
        self.s.join && self.s.setup.side != SideModule::None
    }

    /// The look the Lighting page edits.
    fn edit_look(&self) -> Look {
        if self.joined() || !self.s.edit_module {
            self.s.kb
        } else {
            self.s.module
        }
    }

    fn set_edit_look(&mut self, l: Look) {
        if self.joined() {
            self.s.kb = l;
            self.s.module = l;
        } else if self.s.edit_module {
            self.s.module = l;
        } else {
            self.s.kb = l;
        }
    }

    /// Sends the looks, the joined surface and painted keys to the devices.
    fn send_looks(&mut self, kb: Look, module: Look, join: bool, save: bool) {
        let status = self.status.clone();
        if let Choice::Pc(e) = kb.choice {
            let side = if self.s.setup.module_left {
                f16_core::layout::MacropadSide::Left
            } else {
                f16_core::layout::MacropadSide::Right
            };
            self.engine.send(Command::SetSide(side));
            self.engine.send(Command::SetLinked(Some(kb.pc_params(e))));
            self.pc_live = true;
        } else {
            if self.pc_live {
                self.engine.send(Command::SetLinked(None));
                self.pc_live = false;
            }
            for (kind, look) in [(DeviceKind::Keyboard, kb), (DeviceKind::Macropad, module)] {
                if let Some(d) = ready(&status, kind) {
                    self.engine.send(Command::SetState {
                        kind,
                        state: Self::state_for(&look, d),
                        save,
                    });
                }
            }
            let both = ready(&status, DeviceKind::Keyboard).is_some()
                && ready(&status, DeviceKind::Macropad).is_some();
            let (ks, ms) = deck::span_slices(&self.s.setup);
            let on = join && both && self.s.setup.side == SideModule::Macropad;
            self.engine.send(Command::SetSpan {
                keyboard: on.then_some(ks),
                macropad: on.then_some(ms),
            });
        }
        for (kind, dev, rgb) in self.targets() {
            if let (Some(d), true) = (ready(&status, kind), rgb) {
                let leds = self.paint_for(dev, d);
                self.engine.send(Command::SetPaint {
                    kind,
                    leds,
                    mode: self.s.paint_mode,
                    save,
                });
            }
        }
        let any = self
            .targets()
            .iter()
            .any(|(kind, _, _)| ready(&status, *kind).is_some());
        self.notice = if !any {
            "No device connected.".into()
        } else if save {
            "Saved on the devices: they start this way after a restart.".into()
        } else {
            String::new()
        };
    }

    /// Sends what the Lighting page shows (live, or saved with `save`).
    fn send_current(&mut self, save: bool) {
        if self.white_test {
            return;
        }
        let join = self.joined();
        let (kb, m) = (self.s.kb, if join { self.s.kb } else { self.s.module });
        self.send_looks(kb, m, join, save);
    }

    /// Plain white on both devices, for matching their brightness by eye.
    fn send_white(&mut self) {
        let status = self.status.clone();
        if self.pc_live {
            self.engine.send(Command::SetLinked(None));
            self.pc_live = false;
        }
        let val = self.s.kb.val.max(128);
        for kind in [DeviceKind::Keyboard, DeviceKind::Macropad] {
            if ready(&status, kind).is_some() {
                self.engine.send(Command::SetState {
                    kind,
                    state: LightState {
                        enabled: true,
                        mode: 1, // solid color
                        hue: 0,
                        sat: 0,
                        val,
                        speed: 128,
                    },
                    save: false,
                });
                self.engine.send(Command::SetPaint {
                    kind,
                    leds: vec![],
                    mode: self.s.paint_mode,
                    save: false,
                });
            }
        }
    }

    fn scene(&self, page: &str) -> Scene<'_> {
        let joined = self.joined();
        let (kb, module) = if joined {
            (self.s.kb, self.s.kb)
        } else {
            (self.s.kb, self.s.module)
        };
        Scene {
            setup: self.s.setup,
            joined,
            kb,
            module,
            paint: &self.s.paint,
            paint_mode: self.s.paint_mode,
            macropad_layer: self.layer(),
            show_paint_marks: page == "lighting",
            selected: match page {
                "macropad" => Some((Dev::Module, self.pad_key)),
                "keys" => self.ke.sel,
                _ => None,
            },
        }
    }
}

// -------------------------------------------------------------------------
// Window <-> state
// -------------------------------------------------------------------------

/// Puts the saved choices into the window.
fn push_all(app: &AppWindow, st: &State) {
    let g = app.global::<App>();
    let s = &st.s;
    g.set_kb_type(match s.setup.kb {
        KbType::ClearRgb => 0,
        KbType::BlackRgb => 1,
        KbType::White => 2,
    });
    g.set_side(match s.setup.side {
        SideModule::None => 0,
        SideModule::Macropad => 1,
        SideModule::Numpad => 2,
    });
    g.set_module_left(s.setup.module_left);
    let look = st.edit_look();
    g.set_show_effect(settings::choice_str(look.choice).into());
    g.set_show_hue(look.hue as i32);
    g.set_show_sat(look.sat as i32);
    g.set_show_val(look.val as i32);
    g.set_show_speed(look.speed as i32);
    g.set_paint_mode(match s.paint_mode {
        PaintMode::Steady => 0,
        PaintMode::Follow => 1,
        PaintMode::Tint => 2,
    });
    g.set_brush_hue(s.brush.0 as i32);
    g.set_brush_sat(s.brush.1 as i32);
    g.set_join(s.join);
    g.set_target(if st.joined() {
        0
    } else if s.edit_module && s.setup.side != SideModule::None {
        2
    } else {
        1
    });
    g.set_theme_choice(
        theme::CHOICES
            .iter()
            .position(|c| *c == s.theme)
            .unwrap_or(0) as i32,
    );
    push_derived(app, st);
}

/// Values worked out from the choices: colors, names, notes.
fn push_derived(app: &AppWindow, st: &State) {
    let g = app.global::<App>();
    let s = &st.s;
    let picker = |h: u8, sat: u8| {
        (
            color(qmk_hsv(h as i32, 255, 255)),
            color(qmk_hsv(h as i32, sat as i32, 255)),
            hex(qmk_hsv(h as i32, sat as i32, 255)),
        )
    };
    let look = st.edit_look();
    let (f, p, x) = picker(look.hue, look.sat);
    g.set_show_full(f);
    g.set_show_picked(p);
    g.set_show_hex(x);
    let (f, p, x) = picker(s.brush.0, s.brush.1);
    g.set_brush_full(f);
    g.set_brush_picked(p);
    g.set_brush_hex(x);
    g.set_show_effect_name(look.choice.name().into());

    let has = s.setup.side != SideModule::None;
    g.set_has_module(has);
    g.set_module_name(st.module_name().into());
    g.set_kb_rgb(s.setup.kb.rgb());
    g.set_mod_rgb(s.setup.side == SideModule::Macropad);
    g.set_kb_about(
        match s.setup.kb {
            KbType::ClearRgb => "Clear RGB keys, 97 LEDs.",
            KbType::BlackRgb => "Black RGB keys, 97 LEDs.",
            KbType::White => "Black keys, white backlight.",
        }
        .into(),
    );
    g.set_mod_about(
        if s.setup.side == SideModule::Macropad {
            "Clear RGB keys, 24 LEDs."
        } else {
            "White backlight."
        }
        .into(),
    );
    let summary = |l: &Look| l.choice.name();
    g.set_effects_summary(
        if s.join && has {
            format!("Joined: {}", summary(&s.kb))
        } else if has {
            format!(
                "Keyboard: {}  ·  {}: {}",
                summary(&s.kb),
                st.module_name(),
                summary(&s.module)
            )
        } else {
            format!("Keyboard: {}", summary(&s.kb))
        }
        .into(),
    );
    let kb = match s.setup.kb {
        KbType::ClearRgb => "Clear RGB keyboard",
        KbType::BlackRgb => "Black RGB keyboard",
        KbType::White => "Black keyboard with white backlight",
    };
    let side = match s.setup.side {
        SideModule::None => "no side module".to_string(),
        SideModule::Macropad => format!(
            "RGB macropad on the {}",
            if s.setup.module_left { "left" } else { "right" }
        ),
        SideModule::Numpad => format!(
            "numpad on the {}",
            if s.setup.module_left { "left" } else { "right" }
        ),
    };
    g.set_setup_summary(format!("{kb}, {side}.").into());
    let mut notes = vec![];
    if s.setup.kb == KbType::White {
        notes.push("This keyboard has one white light for all keys: it can only change brightness, so it shows plain white and sits out of effects.");
    }
    if s.setup.side == SideModule::Numpad {
        notes.push("The numpad has one white light for all keys: it can only change brightness, so it shows plain white and sits out of effects.");
    }
    g.set_setup_note(notes.join(" ").into());
    let n = s.paint.len();
    g.set_paint_count(format!("{n} {} painted", if n == 1 { "key" } else { "keys" }).into());
    // macropad key info
    let (a, b) = keys::macropad_sends(st.pad_key);
    g.set_pad_key(format!("Row {}, column {}", st.pad_key / 4 + 1, st.pad_key % 4 + 1).into());
    g.set_pad_a(a.into());
    g.set_pad_b(b.into());
    g.set_pad_note(
        match st.pad_key {
            0 | 3 => "Top corner: press both top corners together to switch layers. Pressed alone it still works, about 40 ms later.",
            20 | 23 => "Bottom corner: press both bottom corners together for the lighting controls.",
            _ => "An ordinary key.",
        }
        .into(),
    );
}

/// Reads the window's choices back into the state.
fn pull_all(app: &AppWindow, st: &mut State) {
    let g = app.global::<App>();
    let b = |v: i32| v.clamp(0, 255) as u8;
    {
        let s = &mut st.s;
        s.setup.kb = match g.get_kb_type() {
            1 => KbType::BlackRgb,
            2 => KbType::White,
            _ => KbType::ClearRgb,
        };
        s.setup.side = match g.get_side() {
            0 => SideModule::None,
            2 => SideModule::Numpad,
            _ => SideModule::Macropad,
        };
        s.setup.module_left = g.get_module_left();
        s.brush = (b(g.get_brush_hue()), b(g.get_brush_sat()));
    }
    let mut look = st.edit_look();
    if let Some(c) = settings::parse_choice(&g.get_show_effect()) {
        look.choice = c;
    }
    look.hue = b(g.get_show_hue());
    look.sat = b(g.get_show_sat());
    look.val = b(g.get_show_val());
    look.speed = b(g.get_show_speed());
    st.set_edit_look(look);
}

fn effect_models(app: &AppWindow) {
    let g = app.global::<App>();
    let item = |key: String, number: String, label: &str| EffectItem {
        key: key.into(),
        number: number.into(),
        label: label.into(),
    };
    let mut groups = vec![EffectGroup {
        name: "Off".into(),
        items: ModelRc::new(VecModel::from(vec![item(
            "paint".into(),
            String::new(),
            "Off (painted keys only)",
        )])),
    }];
    for (name, ids) in GROUPS {
        let items: Vec<EffectItem> = ids
            .iter()
            .map(|&id| item(format!("b{id}"), id.to_string(), EFFECT_NAMES[id as usize]))
            .collect();
        groups.push(EffectGroup {
            name: (*name).into(),
            items: ModelRc::new(VecModel::from(items)),
        });
    }
    let pc: Vec<EffectItem> = LinkedEffect::ALL
        .iter()
        .map(|e| {
            item(
                settings::choice_str(Choice::Pc(*e)),
                String::new(),
                e.name(),
            )
        })
        .collect();
    groups.push(EffectGroup {
        name: "Drawn by the app (needs the app running)".into(),
        items: ModelRc::new(VecModel::from(pc)),
    });
    g.set_effect_groups(ModelRc::new(VecModel::from(groups)));
    let mut names: Vec<SharedString> = vec!["Off (painted keys only)".into()];
    names.extend((1..=44).map(|i| SharedString::from(format!("{i}. {}", EFFECT_NAMES[i]))));
    g.set_effect_names(ModelRc::new(VecModel::from(names)));
}

fn apply_theme(app: &AppWindow, st: &State) {
    theme::apply(app, &st.s.theme, &st.windows);
}

// -------------------------------------------------------------------------
// Every frame
// -------------------------------------------------------------------------

fn tick(app: &AppWindow, st: &mut State) {
    let g = app.global::<App>();
    st.update_answers(app);
    st.status = st.engine.status.lock().unwrap().clone();
    let status = st.status.clone();

    // Start the Lighting page from what each device is running.
    let mut adopted_any = false;
    for kind in [DeviceKind::Keyboard, DeviceKind::Macropad] {
        if st.adopted.contains(&kind) {
            continue;
        }
        if let Some(d) = ready(&status, kind) {
            st.adopted.push(kind);
            let look = if kind == DeviceKind::Keyboard {
                &mut st.s.kb
            } else {
                &mut st.s.module
            };
            let ds = d.state;
            if (1..=44).contains(&ds.mode) {
                *look = Look {
                    choice: Choice::Builtin(ds.mode),
                    hue: ds.hue,
                    sat: ds.sat,
                    val: ds.val,
                    speed: ds.speed,
                };
            } else if ds.mode == d.info.paint_only_effect {
                look.choice = Choice::PaintOnly;
            }
            if st.s.join && kind == DeviceKind::Keyboard {
                st.s.module = st.s.kb;
            }
            push_cal(app, &status, kind);
            adopted_any = true;
        }
    }
    // Follow what is plugged in: macropad (our firmware or not) or Framework's numpad.
    let detected = if !matches!(status.macropad, Slot::Missing) {
        Some(SideModule::Macropad)
    } else if status.numpad {
        Some(SideModule::Numpad)
    } else {
        None
    };
    if detected != st.last_detected {
        st.last_detected = detected;
        if let Some(side) = detected {
            if st.s.setup.side != side {
                st.s.setup.side = side;
                adopted_any = true;
            }
        }
    }
    // Remember clear / black / white per keyboard (by its serial number).
    if let Some(serial) = ready(&status, DeviceKind::Keyboard).and_then(|d| d.serial.clone()) {
        match st.s.kb_types.iter().find(|(s, _)| *s == serial) {
            Some((_, t)) if *t != st.s.setup.kb => {
                st.s.setup.kb = *t;
                adopted_any = true;
            }
            Some(_) => {}
            None => st.s.kb_types.push((serial, st.s.setup.kb)),
        }
    }
    if adopted_any {
        push_all(app, st);
    }

    // Live: send lighting changes a few times a second while they happen
    if st.dirty && st.last_live.elapsed() >= Duration::from_millis(80) {
        st.dirty = false;
        st.last_live = Instant::now();
        st.send_current(false);
    }
    // Save LED calibration on the device once the sliders have been still for a moment
    for (i, kind) in [DeviceKind::Keyboard, DeviceKind::Macropad]
        .into_iter()
        .enumerate()
    {
        if st.cal_dirty[i].is_some_and(|t| t.elapsed() >= Duration::from_millis(800)) {
            st.cal_dirty[i] = None;
            if let Some(cal) = cal_from_window(app, i) {
                st.engine.send(Command::SetLedCal {
                    kind,
                    cal,
                    save: true,
                });
            }
        }
    }

    // Devices at the bottom of the menu
    let mut rows: Vec<DeviceRow> = [DeviceKind::Keyboard, DeviceKind::Macropad]
        .iter()
        .filter(|&&kind| {
            kind == DeviceKind::Keyboard
                || !status.numpad
                || !matches!(status.macropad, Slot::Missing)
        })
        .map(|&kind| {
            let (text, ok) = match status.slot(kind) {
                Slot::Missing => ("Not found".to_string(), false),
                Slot::Stock => ("Needs the f16custom firmware".to_string(), false),
                Slot::Ready(d) => (
                    if d.streaming {
                        "Connected · the PC is drawing".into()
                    } else {
                        "Connected".into()
                    },
                    true,
                ),
            };
            DeviceRow {
                name: kind.name().into(),
                status: text.into(),
                ok,
                checked: false,
            }
        })
        .collect();
    if status.numpad {
        rows.push(DeviceRow {
            name: "Numpad".into(),
            status: "Connected · backlight only".into(),
            ok: true,
            checked: false,
        });
    }
    let same = g.get_devices().iter().eq(rows.iter().cloned());
    if !same {
        g.set_devices(ModelRc::new(VecModel::from(rows)));
    }
    let live = st.pc_live || status.linked;
    g.set_live(live);
    g.set_pc_live(st.pc_live);
    g.set_bar_status(if live {
        "Live: the PC is drawing on your devices.".into()
    } else if !st.notice.is_empty() {
        st.notice.clone().into()
    } else {
        "Live: changes show on the devices as you make them. Save as default keeps them after a restart.".into()
    });
    g.set_status(status.message.clone().unwrap_or_default().into());
    g.set_hotkey_lock(
        [DeviceKind::Keyboard, DeviceKind::Macropad]
            .iter()
            .any(|k| ready(&status, *k).is_some_and(|d| d.info.hotkey_lock)),
    );
    g.set_layer(st.layer() as i32);
    g.set_kb_present(!matches!(status.keyboard, Slot::Missing));
    g.set_mod_present(!matches!(status.macropad, Slot::Missing));

    // Follow Windows' look
    if st.last_theme_check.elapsed() >= Duration::from_secs(2) {
        st.last_theme_check = Instant::now();
        let w = theme::read_windows();
        if w != st.windows {
            st.windows = w;
            apply_theme(app, st);
        }
    }

    // The keyboard picture, on the pages that show it
    let page = g.get_page().to_string();
    if matches!(page.as_str(), "keys" | "home" | "macropad") {
        keyedit::push(app, st);
    }
    let visible = app.window().is_visible();
    if visible && matches!(page.as_str(), "lighting" | "macropad" | "setup" | "keys") {
        let scale = app.window().scale_factor();
        let width = ((g.get_deck_width() * scale).round() as u32).clamp(300, 1400);
        let mut preview = std::mem::take(&mut st.preview);
        let scene = st.scene(&page);
        let mut placed = deck::place_keys(&scene.setup, scene.macropad_layer);
        let shell = deck::shell(&scene.setup);
        let colors = if page == "keys" {
            keyedit::decorate(st, &mut placed)
        } else {
            preview.colors(&scene, &placed)
        };
        let accent = g_accent(app);
        let pic = render::draw(&scene, &placed, &colors, &shell, width, accent);
        let typing = preview.typing(&scene);
        let joined = scene.joined && scene.setup.side != SideModule::None;
        st.preview = preview;
        st.deck_width_units = shell.size.0;
        st.keys = placed;
        let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            &pic.pixels,
            pic.width,
            pic.height,
        );
        g.set_deck_image(slint::Image::from_rgba8_premultiplied(buffer));
        g.set_deck_aspect(pic.height as f32 / pic.width as f32);
        let legends: Vec<Legend> = pic
            .legends
            .into_iter()
            .map(|l| Legend {
                x: l.x,
                y: l.y,
                w: l.w,
                h: l.h,
                text: l.text.into(),
                color: Color::from_argb_u8(l.color[3], l.color[0], l.color[1], l.color[2]),
                fs: l.fs,
                left: l.left,
            })
            .collect();
        // Keep the same model and only touch rows that changed, so the window doesn't rebuild
        // every legend 30 times a second.
        let same_layout = st.legends.row_count() == legends.len()
            && st
                .legends
                .iter()
                .zip(&legends)
                .all(|(a, b)| a.text == b.text && a.x == b.x && a.y == b.y && a.w == b.w);
        if same_layout {
            for (i, l) in legends.into_iter().enumerate() {
                if st.legends.row_data(i).as_ref() != Some(&l) {
                    st.legends.set_row_data(i, l);
                }
            }
        } else {
            st.legends.set_vec(legends);
        }
        let mut caption = vec!["Preview".to_string()];
        if page == "keys" {
            caption = vec!["Orange keys are changed from the Framework default".to_string()];
        }
        if page != "keys" {
            caption.push(if joined {
                "joined: one surface".into()
            } else {
                "separate devices".into()
            });
        }
        if typing {
            caption.push("pretend typing on".into());
        }
        g.set_deck_caption(caption.join("  ·  ").into());
    }

    if st.s != st.last_saved {
        settings::save(&st.s);
        st.last_saved = st.s.clone();
    }
}

/// Puts one device's LED calibration into the window's sliders.
fn push_cal(app: &AppWindow, status: &Status, kind: DeviceKind) {
    let g = app.global::<App>();
    let cal = ready(status, kind).and_then(|d| d.led_cal);
    let c = cal.unwrap_or(f16_core::protocol::LedCal {
        current: 104,
        gains: [255; 3],
        default_current: 104,
    });
    if kind == DeviceKind::Keyboard {
        g.set_kb_cal_known(cal.is_some());
        g.set_kb_cal_current(c.current as i32);
        g.set_kb_cal_r(c.gains[0] as i32);
        g.set_kb_cal_g(c.gains[1] as i32);
        g.set_kb_cal_b(c.gains[2] as i32);
    } else {
        g.set_mod_cal_known(cal.is_some());
        g.set_mod_cal_current(c.current as i32);
        g.set_mod_cal_r(c.gains[0] as i32);
        g.set_mod_cal_g(c.gains[1] as i32);
        g.set_mod_cal_b(c.gains[2] as i32);
    }
}

/// One device's LED calibration as the window's sliders show it (0 keyboard, 1 macropad).
fn cal_from_window(app: &AppWindow, which: usize) -> Option<f16_core::protocol::LedCal> {
    let g = app.global::<App>();
    let b = |v: i32| v.clamp(1, 255) as u8;
    let (known, cur, r, gr, bl) = if which == 0 {
        (
            g.get_kb_cal_known(),
            g.get_kb_cal_current(),
            g.get_kb_cal_r(),
            g.get_kb_cal_g(),
            g.get_kb_cal_b(),
        )
    } else {
        (
            g.get_mod_cal_known(),
            g.get_mod_cal_current(),
            g.get_mod_cal_r(),
            g.get_mod_cal_g(),
            g.get_mod_cal_b(),
        )
    };
    known.then_some(f16_core::protocol::LedCal {
        current: b(cur),
        gains: [b(r), b(gr), b(bl)],
        default_current: 0,
    })
}

fn g_accent(app: &AppWindow) -> [u8; 3] {
    let c = app.global::<Theme>().get_accent();
    [c.red(), c.green(), c.blue()]
}

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    // `framework-16-keyboard-control.exe --demo` shows pretend devices (no hardware needed)
    let demo = std::env::args().any(|a| a == "--demo");
    let weak = app.as_weak();
    let engine = engine::start(
        move || {
            let _ = weak.upgrade_in_event_loop(|_| {});
        },
        demo,
    );
    let s = settings::load();
    // The hotkey and tray thread talks to the window through the event loop.
    let bg = {
        let weak = app.as_weak();
        winsys::Background::start(move |e| {
            let _ = weak.upgrade_in_event_loop(move |app| match e {
                winsys::TrayEvent::Open => {
                    let _ = app.show();
                    app.window().set_minimized(false);
                }
                winsys::TrayEvent::Quit => {
                    let _ = slint::quit_event_loop();
                }
            });
        })
    };
    let st = Rc::new(RefCell::new(State {
        engine,
        bg,
        ke: keyedit::KeyEdit::default(),
        update: None,
        update_inbox: Arc::new(Mutex::new(None)),
        last_saved: s.clone(),
        s,
        preview: Preview::default(),
        keys: vec![],
        deck_width_units: 1.0,
        local_layer: 0,
        pad_key: 0,
        pc_live: false,
        notice: String::new(),
        adopted: vec![],
        dirty: false,
        last_live: Instant::now(),
        cal_dirty: [None, None],
        white_test: false,
        last_detected: None,
        windows: theme::read_windows(),
        last_theme_check: Instant::now(),
        status: Status::default(),
        legends: Rc::new(VecModel::default()),
    }));
    effect_models(&app);
    {
        let mut st = st.borrow_mut();
        apply_theme(&app, &st);
        push_all(&app, &st);
        st.sync_background();
        keyedit::push(&app, &mut st);
        let g = app.global::<App>();
        g.set_version(update::CURRENT.into());
        g.set_auto_update(st.s.auto_update);
        g.set_update_status("".into());
        if st.s.auto_update {
            st.check_for_update(&app);
        }
    }
    let g = app.global::<App>();
    g.set_legends(ModelRc::from(st.borrow().legends.clone()));

    // A callback that reads the window, changes the state and refreshes what depends on it.
    macro_rules! on {
        ($setter:ident, |$app:ident, $st:ident $(, $arg:ident : $ty:ty)*| $body:block) => {{
            let weak = app.as_weak();
            let st = st.clone();
            g.$setter(move |$($arg: $ty),*| {
                let Some($app) = weak.upgrade() else { return };
                let mut guard = st.borrow_mut();
                let $st: &mut State = &mut guard;
                $body
            });
        }};
    }

    on!(on_setup_changed, |app, st| {
        pull_all(&app, st);
        // Remember the keyboard type for the keyboard that is plugged in
        if let Some(serial) = ready(&st.status, DeviceKind::Keyboard).and_then(|d| d.serial.clone())
        {
            let t = st.s.setup.kb;
            st.s.kb_types.retain(|(s, _)| *s != serial);
            st.s.kb_types.push((serial, t));
        }
        push_derived(&app, st);
        st.dirty = true;
    });
    on!(on_show_changed, |app, st| {
        pull_all(&app, st);
        push_derived(&app, st);
        st.dirty = true;
    });
    on!(on_join_toggled, |app, st, on: bool| {
        pull_all(&app, st);
        st.s.join = on;
        if on {
            st.s.module = st.s.kb;
        }
        push_all(&app, st);
        st.dirty = true;
    });
    on!(on_target_chosen, |app, st, target: i32| {
        pull_all(&app, st);
        st.s.join = target == 0;
        st.s.edit_module = target == 2;
        if st.s.join {
            st.s.module = st.s.kb;
        }
        push_all(&app, st);
        st.dirty = true;
    });
    on!(on_paint_mode_chosen, |app, st, mode: i32| {
        st.s.paint_mode = match mode {
            0 => PaintMode::Steady,
            2 => PaintMode::Tint,
            _ => PaintMode::Follow,
        };
        push_all(&app, st);
        st.dirty = true;
    });
    on!(on_send_show, |app, st, save: bool| {
        pull_all(&app, st);
        st.send_current(save);
    });
    on!(on_stop_pc, |_app, st| {
        st.engine.send(Command::SetLinked(None));
        st.pc_live = false;
    });
    on!(on_paint_example, |app, st| {
        pull_all(&app, st);
        for (i, k) in keys::keyboard_keys().iter().enumerate() {
            if [
                "W", "A", "S", "D", "\u{2190}", "\u{2191}", "\u{2192}", "\u{2193}",
            ]
            .contains(&k.main.as_str())
            {
                st.s.paint.insert((Dev::Keyboard, i), st.s.brush);
            }
        }
        push_derived(&app, st);
        st.dirty = true;
    });
    on!(on_paint_clear, |app, st| {
        st.s.paint.clear();
        push_derived(&app, st);
        st.dirty = true;
    });
    on!(on_deck_pointer, |app, st, xf: f32, yf: f32, kind: i32| {
        let g = app.global::<App>();
        let (x, y) = (xf * st.deck_width_units, yf * st.deck_width_units);
        let Some(k) = deck::key_at(&st.keys, x, y).cloned() else {
            return;
        };
        match g.get_page().as_str() {
            "lighting" if k.rgb => {
                pull_all(&app, st);
                if g.get_paint_tool() == 1 {
                    st.s.paint.remove(&(k.dev, k.index));
                } else {
                    // a painted key takes the new color too
                    st.s.paint.insert((k.dev, k.index), st.s.brush);
                }
                push_derived(&app, st);
                st.dirty = true;
            }
            "macropad" if k.dev == Dev::Module && kind == 0 => {
                st.pad_key = k.index;
                push_derived(&app, st);
            }
            "keys" if kind == 0 => {
                keyedit::select(st, k.dev, k.index);
                keyedit::push(&app, st);
            }
            _ => {}
        }
    });
    on!(on_layer_chosen, |_app, st, layer: i32| {
        st.local_layer = layer as u8;
        st.engine.send(Command::SetLayer(layer as u8));
    });
    on!(on_lock_toggled, |_app, st, on: bool| {
        st.engine.send(Command::SetHotkeyLock(on));
    });
    on!(on_bootloader, |_app, st, which: i32| {
        st.engine.send(Command::Bootloader(if which == 0 {
            DeviceKind::Keyboard
        } else {
            DeviceKind::Macropad
        }));
    });
    on!(on_rescan, |_app, st| {
        st.engine.send(Command::Rescan);
    });
    // Matching the two devices' brightness
    on!(on_cal_changed, |app, st, which: i32| {
        let i = which.clamp(0, 1) as usize;
        let kind = if i == 0 {
            DeviceKind::Keyboard
        } else {
            DeviceKind::Macropad
        };
        if let Some(cal) = cal_from_window(&app, i) {
            st.engine.send(Command::SetLedCal {
                kind,
                cal,
                save: false,
            });
            st.cal_dirty[i] = Some(Instant::now());
        }
    });
    on!(on_cal_reset, |app, st, which: i32| {
        let i = which.clamp(0, 1) as usize;
        let kind = if i == 0 {
            DeviceKind::Keyboard
        } else {
            DeviceKind::Macropad
        };
        if let Some(d) = ready(&st.status, kind) {
            if let Some(mut cal) = d.led_cal {
                cal.current = cal.default_current;
                cal.gains = [255; 3];
                st.engine.send(Command::SetLedCal {
                    kind,
                    cal,
                    save: true,
                });
                if let Slot::Ready(d) = st.status.slot_mut(kind) {
                    d.led_cal = Some(cal);
                }
                push_cal(&app, &st.status, kind);
            }
        }
    });
    on!(on_white_test_toggled, |app, st, on: bool| {
        st.white_test = on;
        app.global::<App>().set_white_test(on);
        if on {
            st.send_white();
        } else {
            st.send_current(false);
        }
    });
    // Color codes typed or pasted into a color picker
    {
        let g = app.global::<App>();
        g.on_hex_hue(|t| parse_hex(&t).map_or(-1, |(h, _)| h as i32));
        g.on_hex_sat(|t| parse_hex(&t).map_or(-1, |(_, s)| s as i32));
    }
    // Keys page
    on!(on_keys_view_chosen, |app, st, which: i32, view: i32| {
        keyedit::view_chosen(st, which, view);
        keyedit::push(&app, st);
    });
    on!(on_key_kind_chosen, |app, st, kind: i32| {
        keyedit::kind_chosen(st, kind);
        keyedit::push(&app, st);
    });
    on!(on_key_choice_chosen, |app, st, i: i32| {
        keyedit::choice_chosen(st, i);
        keyedit::push(&app, st);
    });
    on!(on_key_mod_toggled, |app, st, which: i32, on: bool| {
        keyedit::mod_toggled(st, which, on);
        keyedit::push(&app, st);
    });
    on!(on_key_app_edited, |_app, st, path: SharedString| {
        keyedit::app_edited(st, &path);
    });
    on!(on_key_app_browse, |app, st| {
        keyedit::app_browse(st);
        keyedit::push(&app, st);
    });
    on!(on_keys_reset, |app, st| {
        keyedit::reset_all(st);
        keyedit::push(&app, st);
    });
    on!(on_combo_toggled, |app, st, which: i32, on: bool| {
        keyedit::combo_toggled(st, which, on);
        keyedit::push(&app, st);
    });
    on!(on_combo_choice_chosen, |app, st, which: i32, i: i32| {
        keyedit::combo_chosen(st, which, i);
        keyedit::push(&app, st);
    });
    on!(
        on_combo_app_edited,
        |_app, st, which: i32, path: SharedString| {
            keyedit::combo_app_edited(st, which, &path);
        }
    );
    on!(on_combo_app_browse, |app, st, which: i32| {
        keyedit::combo_app_browse(st, which);
        keyedit::push(&app, st);
    });
    on!(on_tray_toggled, |app, st, on: bool| {
        st.s.tray = on;
        st.sync_background();
        keyedit::push(&app, st);
    });
    on!(on_autostart_toggled, |app, st, on: bool| {
        st.s.autostart = Some(on);
        st.sync_background();
        keyedit::push(&app, st);
    });

    on!(on_update_check, |app, st| {
        st.check_for_update(&app);
    });
    on!(on_update_install, |app, st| {
        st.install_update(&app);
    });
    on!(on_update_auto_toggled, |app, st, on: bool| {
        st.s.auto_update = on;
        app.global::<App>().set_auto_update(on);
    });
    on!(on_corner_lights_toggled, |app, st, on: bool| {
        keyedit::corner_lights_toggled(st, on);
        keyedit::push(&app, st);
    });
    on!(on_theme_chosen, |app, st, i: i32| {
        st.s.theme = theme::CHOICES
            .get(i as usize)
            .unwrap_or(&"system")
            .to_string();
        app.global::<App>().set_theme_choice(i);
        apply_theme(&app, st);
    });

    // ~30 frames a second for the live picture
    let timer = slint::Timer::default();
    {
        let weak = app.as_weak();
        let st = st.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                if let Some(app) = weak.upgrade() {
                    if let Ok(mut st) = st.try_borrow_mut() {
                        tick(&app, &mut st);
                    }
                }
            },
        );
    }
    // Closing the window: stay in the notification area, or quit.
    {
        let st = st.clone();
        app.window().on_close_requested(move || {
            let tray = st.try_borrow().map(|s| s.s.tray).unwrap_or(false);
            if !(tray && HAS_TRAY) {
                let _ = slint::quit_event_loop();
            }
            slint::CloseRequestResponse::HideWindow
        });
    }
    // Started by Windows at sign-in: begin in the notification area.
    let hidden = HAS_TRAY && st.borrow().s.tray && std::env::args().any(|a| a == "--tray");
    if !hidden {
        app.show()?;
    }
    slint::run_event_loop_until_quit()?;
    app.hide()?;
    st.borrow().bg.close();
    // Hand the lights back to the devices' saved effects (EngineHandle's Drop does the rest).
    drop(timer);
    Ok(())
}
