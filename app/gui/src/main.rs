//! Framework 16 Keyboard Control: controls the f16custom firmware on the Framework Laptop 16
//! keyboard and RGB macropad. The window is built with Slint in the style of the White Rabbit app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window

slint::include_modules!();

mod deck;
mod device;
mod engine;
mod render;
mod settings;
mod sim;
mod theme;

use deck::{Choice, Dev, KbType, Look, PlacedKey, Preview, Scene};
use engine::{Command, EngineHandle, Slot, Status};
use f16_core::builtin::qmk_hsv;
use f16_core::effects::LinkedEffect;
use f16_core::keys::{self, SideModule, KEYBOARD_SIZE, MODULE_GRID};
use f16_core::protocol::{DeviceKind, LightState, Paint, EFFECT_NAMES};
use settings::Saved;
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
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

struct State {
    engine: EngineHandle,
    s: Saved,
    last_saved: Saved,
    preview: Preview,
    keys: Vec<PlacedKey>,
    deck_width_units: f32,
    local_layer: u8,
    pad_key: usize,
    pc_live: bool,
    notice: String,
    detected_module: bool,
    adopted: Vec<DeviceKind>,
    /// Send to devices acts on: [keyboard, macropad]
    send_to: [bool; 2],
    windows: theme::Windows,
    last_theme_check: Instant,
    status: Status,
    legends: Rc<VecModel<Legend>>,
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

fn effect_index(l: &Look) -> i32 {
    match l.choice {
        Choice::Builtin(id) => id as i32,
        _ => 0,
    }
}

fn choice_of_index(i: i32) -> Choice {
    if (1..=44).contains(&i) {
        Choice::Builtin(i as u8)
    } else {
        Choice::PaintOnly
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

    fn targets(&self) -> Vec<(DeviceKind, Dev, bool, usize)> {
        vec![
            (
                DeviceKind::Keyboard,
                Dev::Keyboard,
                self.s.setup.kb.rgb(),
                0,
            ),
            (
                DeviceKind::Macropad,
                Dev::Module,
                self.s.setup.side == SideModule::Macropad,
                1,
            ),
        ]
    }

    /// Sends a look to each checked device (and the joined surface and painted keys).
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
            for (kind, look, n) in [
                (DeviceKind::Keyboard, kb, 0),
                (DeviceKind::Macropad, module, 1),
            ] {
                if let (Some(d), true) = (ready(&status, kind), self.send_to[n]) {
                    self.engine.send(Command::SetState {
                        kind,
                        state: Self::state_for(&look, d),
                        save,
                    });
                }
            }
            let both = ready(&status, DeviceKind::Keyboard).is_some()
                && ready(&status, DeviceKind::Macropad).is_some()
                && self.send_to == [true, true];
            let (ks, ms) = deck::span_slices(&self.s.setup);
            let on = join && both && self.s.setup.side == SideModule::Macropad;
            self.engine.send(Command::SetSpan {
                keyboard: on.then_some(ks),
                macropad: on.then_some(ms),
            });
        }
        for (kind, dev, rgb, n) in self.targets() {
            if let (Some(d), true, true) = (ready(&status, kind), rgb, self.send_to[n]) {
                let leds = self.paint_for(dev, d);
                self.engine.send(Command::SetPaint {
                    kind,
                    leds,
                    follow: self.s.follow,
                    save,
                });
            }
        }
        let any = self
            .targets()
            .iter()
            .any(|(kind, _, _, n)| ready(&status, *kind).is_some() && self.send_to[*n]);
        self.notice = if !any {
            "No device to send to: none is connected, or none is ticked under Send to.".into()
        } else if save {
            "Sent and saved on the devices.".into()
        } else {
            "Sent to the devices.".into()
        };
    }

    fn send_effects(&mut self, save: bool) {
        let (kb, m) = (
            self.s.kb,
            if self.s.join {
                self.s.kb
            } else {
                self.s.module
            },
        );
        let join = self.s.join;
        self.send_looks(kb, m, join, save);
    }

    fn scene(&self, page: &str) -> Scene<'_> {
        let show = page == "show";
        let joined = show || (self.s.join && self.s.setup.side != SideModule::None);
        let (kb, module) = if show {
            (self.s.show, self.s.show)
        } else if joined {
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
            follow: self.s.follow,
            macropad_layer: self.layer(),
            show_paint_marks: show,
            selected_module_key: (page == "macropad").then_some(self.pad_key),
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
    g.set_show_effect(
        settings::choice_str(s.show.choice)
            .replace("-", "_")
            .replace("_", "-")
            .into(),
    );
    g.set_show_hue(s.show.hue as i32);
    g.set_show_sat(s.show.sat as i32);
    g.set_show_val(s.show.val as i32);
    g.set_show_speed(s.show.speed as i32);
    g.set_follow(s.follow);
    g.set_brush_hue(s.brush.0 as i32);
    g.set_brush_sat(s.brush.1 as i32);
    g.set_join(s.join);
    g.set_kb_effect(effect_index(&s.kb));
    g.set_kb_hue(s.kb.hue as i32);
    g.set_kb_sat(s.kb.sat as i32);
    g.set_kb_val(s.kb.val as i32);
    g.set_kb_speed(s.kb.speed as i32);
    g.set_mod_effect(effect_index(&s.module));
    g.set_mod_hue(s.module.hue as i32);
    g.set_mod_sat(s.module.sat as i32);
    g.set_mod_val(s.module.val as i32);
    g.set_mod_speed(s.module.speed as i32);
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
    let (f, p, x) = picker(s.show.hue, s.show.sat);
    g.set_show_full(f);
    g.set_show_picked(p);
    g.set_show_hex(x);
    let (f, p, x) = picker(s.brush.0, s.brush.1);
    g.set_brush_full(f);
    g.set_brush_picked(p);
    g.set_brush_hex(x);
    let (f, p, x) = picker(s.kb.hue, s.kb.sat);
    g.set_kb_full(f);
    g.set_kb_picked(p);
    g.set_kb_hex(x);
    let (f, p, x) = picker(s.module.hue, s.module.sat);
    g.set_mod_full(f);
    g.set_mod_picked(p);
    g.set_mod_hex(x);
    g.set_show_effect_name(s.show.choice.name().into());

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
    if let Some(c) = settings::parse_choice(&g.get_show_effect()) {
        s.show.choice = c;
    }
    let b = |v: i32| v.clamp(0, 255) as u8;
    s.show.hue = b(g.get_show_hue());
    s.show.sat = b(g.get_show_sat());
    s.show.val = b(g.get_show_val());
    s.show.speed = b(g.get_show_speed());
    s.follow = g.get_follow();
    s.brush = (b(g.get_brush_hue()), b(g.get_brush_sat()));
    s.kb = Look {
        choice: choice_of_index(g.get_kb_effect()),
        hue: b(g.get_kb_hue()),
        sat: b(g.get_kb_sat()),
        val: b(g.get_kb_val()),
        speed: b(g.get_kb_speed()),
    };
    s.module = Look {
        choice: choice_of_index(g.get_mod_effect()),
        hue: b(g.get_mod_hue()),
        sat: b(g.get_mod_sat()),
        val: b(g.get_mod_val()),
        speed: b(g.get_mod_speed()),
    };
    if s.join {
        s.module = s.kb;
    }
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
    st.status = st.engine.status.lock().unwrap().clone();
    let status = st.status.clone();

    // Start the Effects page from what each device is running.
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
            adopted_any = true;
        }
    }
    // The first time a macropad shows up, put it in the setup.
    if !st.detected_module && ready(&status, DeviceKind::Macropad).is_some() {
        st.detected_module = true;
        if st.s.setup.side == SideModule::None {
            st.s.setup.side = SideModule::Macropad;
            adopted_any = true;
        }
    }
    if adopted_any {
        push_all(app, st);
    }

    // Devices at the bottom of the menu
    let rows: Vec<DeviceRow> = [DeviceKind::Keyboard, DeviceKind::Macropad]
        .iter()
        .enumerate()
        .map(|(i, &kind)| {
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
                checked: st.send_to[i],
            }
        })
        .collect();
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
        "Preview only: nothing changes on the devices until you press Send.".into()
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
    if matches!(page.as_str(), "show" | "effects" | "macropad" | "setup") {
        let scale = app.window().scale_factor();
        let width = ((g.get_deck_width() * scale).round() as u32).clamp(300, 1400);
        let mut preview = std::mem::take(&mut st.preview);
        let scene = st.scene(&page);
        let placed = deck::place_keys(&scene.setup, scene.macropad_layer);
        let shell = deck::shell(&scene.setup);
        let colors = preview.colors(&scene, &placed);
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
        caption.push(if joined {
            "joined: one surface".into()
        } else {
            "separate devices".into()
        });
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
    let st = Rc::new(RefCell::new(State {
        engine,
        last_saved: s.clone(),
        s,
        preview: Preview::default(),
        keys: vec![],
        deck_width_units: 1.0,
        local_layer: 0,
        pad_key: 0,
        pc_live: false,
        notice: String::new(),
        detected_module: false,
        adopted: vec![],
        send_to: [true, true],
        windows: theme::read_windows(),
        last_theme_check: Instant::now(),
        status: Status::default(),
        legends: Rc::new(VecModel::default()),
    }));
    effect_models(&app);
    {
        let st = st.borrow();
        apply_theme(&app, &st);
        push_all(&app, &st);
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
        push_derived(&app, st);
    });
    on!(on_show_changed, |app, st| {
        pull_all(&app, st);
        push_derived(&app, st);
    });
    on!(on_effects_changed, |app, st| {
        pull_all(&app, st);
        push_all(&app, st);
    });
    on!(on_join_toggled, |app, st, on: bool| {
        pull_all(&app, st);
        st.s.join = on;
        if on {
            st.s.module = st.s.kb;
        }
        push_all(&app, st);
        st.send_effects(false);
    });
    on!(on_copy_settings, |app, st, which: i32| {
        pull_all(&app, st);
        if which == 0 {
            st.s.module = st.s.kb;
        } else {
            st.s.kb = st.s.module;
        }
        push_all(&app, st);
    });
    on!(on_send_show, |app, st, save: bool| {
        pull_all(&app, st);
        let look = st.s.show;
        st.send_looks(look, look, true, save);
    });
    on!(on_send_effects, |app, st, save: bool| {
        pull_all(&app, st);
        st.send_effects(save);
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
    });
    on!(on_paint_clear, |app, st| {
        st.s.paint.clear();
        push_derived(&app, st);
    });
    on!(on_deck_pointer, |app, st, xf: f32, yf: f32, kind: i32| {
        let g = app.global::<App>();
        let (x, y) = (xf * st.deck_width_units, yf * st.deck_width_units);
        let Some(k) = deck::key_at(&st.keys, x, y).cloned() else {
            return;
        };
        match g.get_page().as_str() {
            "show" if k.rgb => {
                pull_all(&app, st);
                if g.get_paint_tool() == 1 {
                    st.s.paint.remove(&(k.dev, k.index));
                } else {
                    st.s.paint.insert((k.dev, k.index), st.s.brush);
                }
                push_derived(&app, st);
            }
            "macropad" if k.dev == Dev::Module && kind == 0 => {
                st.pad_key = k.index;
                push_derived(&app, st);
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
    on!(on_device_checked, |_app, st, i: i32, on: bool| {
        if let Some(v) = st.send_to.get_mut(i as usize) {
            *v = on;
        }
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
    app.run()?;
    // Hand the lights back to the devices' saved effects (EngineHandle's Drop does the rest).
    drop(timer);
    Ok(())
}
