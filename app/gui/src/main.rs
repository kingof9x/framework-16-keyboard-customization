//! Framework 16 Lights: controls the f16custom firmware on the Framework
//! Laptop 16 keyboard and RGB macropad.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window

mod deck;
mod device;
mod engine;
mod settings;
mod sim;
mod widgets;

use deck::{Choice, Dev, KbType, Look, Preview, Scene};
use eframe::egui::{self, RichText, Vec2};
use engine::{Command, EngineHandle, Slot, Status};
use f16_core::effects::LinkedEffect;
use f16_core::keys::{self, SideModule, KEYBOARD_SIZE, MODULE_GRID};
use f16_core::protocol::{DeviceKind, LightState, Paint, EFFECT_NAMES};
use settings::Saved;
use widgets::{button, card, primary, segmented, switch, ACCENT, ON_ACCENT};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Framework 16 Lights")
            .with_inner_size([1280.0, 960.0])
            .with_min_inner_size([760.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Framework 16 Lights",
        options,
        Box::new(|cc| Ok(Box::new(LightsApp::new(cc)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Show,
    Effects,
    Macropad,
    Firmware,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Paint,
    Erase,
}

/// Built-in effects grouped for the Show tab.
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

struct LightsApp {
    engine: EngineHandle,
    tab: Tab,
    s: Saved,
    last_saved: Saved,
    tool: Tool,
    preview: Preview,
    /// Layer shown when no macropad is connected.
    local_layer: u8,
    pad_key: usize,
    pc_live: bool,
    notice: Option<String>,
    confirm_bootloader: Option<DeviceKind>,
    detected_module: bool,
    /// Devices whose current effect has been copied into the Effects tab.
    adopted: Vec<DeviceKind>,
}

impl LightsApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        // Follow the Windows light/dark setting, Framework orange as accent.
        ctx.set_theme(egui::ThemePreference::System);
        ctx.all_styles_mut(|style| {
            style.visuals.selection.bg_fill = ACCENT;
            style.visuals.selection.stroke = egui::Stroke::new(1.0, ON_ACCENT);
            style.visuals.hyperlink_color = ACCENT;
            style.spacing.button_padding = Vec2::new(10.0, 5.0);
            style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        });
        // `f16-lights.exe --demo` shows pretend devices (no hardware needed)
        let demo = std::env::args().any(|a| a == "--demo");
        let repaint = cc.egui_ctx.clone();
        let engine = engine::start(move || repaint.request_repaint(), demo);
        let s = settings::load();
        Self {
            engine,
            tab: Tab::Show,
            last_saved: s.clone(),
            s,
            tool: Tool::Paint,
            preview: Preview::default(),
            local_layer: 0,
            pad_key: 0,
            pc_live: false,
            notice: None,
            confirm_bootloader: None,
            detected_module: false,
            adopted: Vec::new(),
        }
    }

    fn status(&self) -> Status {
        self.engine.status.lock().unwrap().clone()
    }

    fn ready(status: &Status, kind: DeviceKind) -> Option<&engine::DeviceStatus> {
        match status.slot(kind) {
            Slot::Ready(d) => Some(d),
            _ => None,
        }
    }

    fn layer(&self, status: &Status) -> u8 {
        Self::ready(status, DeviceKind::Macropad)
            .and_then(|d| d.info.layer)
            .unwrap_or(self.local_layer)
    }

    fn joined(&self) -> bool {
        self.tab == Tab::Show || (self.s.join && self.s.setup.side != SideModule::None)
    }

    // -----------------------------------------------------------------------
    // Sending to the devices
    // -----------------------------------------------------------------------

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
    fn paint_for(&self, dev: Dev, d: &engine::DeviceStatus, layer: u8) -> Vec<Paint> {
        let (defs, size) = match dev {
            Dev::Keyboard => (keys::keyboard_keys(), KEYBOARD_SIZE),
            Dev::Module => (keys::macropad_keys(layer), MODULE_GRID),
        };
        keys::map_leds_to_keys(&d.leds, &defs, size)
            .into_iter()
            .map(|k| self.s.paint.get(&(dev, k)).copied())
            .collect()
    }

    fn send_paint(&self, status: &Status, save: bool) {
        let layer = self.layer(status);
        for (kind, dev, rgb) in [
            (DeviceKind::Keyboard, Dev::Keyboard, self.s.setup.kb.rgb()),
            (
                DeviceKind::Macropad,
                Dev::Module,
                self.s.setup.side == SideModule::Macropad,
            ),
        ] {
            if let (Some(d), true) = (Self::ready(status, kind), rgb) {
                let leds = self.paint_for(dev, d, layer);
                self.engine.send(Command::SetPaint {
                    kind,
                    leds,
                    follow: self.s.follow,
                    save,
                });
            }
        }
    }

    /// Sends one look to each device (`module` = None: the keyboard's look for both).
    fn send_looks(&mut self, status: &Status, kb: Look, module: Look, join: bool, save: bool) {
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
                if let Some(d) = Self::ready(status, kind) {
                    self.engine.send(Command::SetState {
                        kind,
                        state: Self::state_for(&look, d),
                        save,
                    });
                }
            }
            let both = Self::ready(status, DeviceKind::Keyboard).is_some()
                && Self::ready(status, DeviceKind::Macropad).is_some();
            let (ks, ms) = deck::span_slices(&self.s.setup);
            let on = join && both && self.s.setup.side == SideModule::Macropad;
            self.engine.send(Command::SetSpan {
                keyboard: on.then_some(ks),
                macropad: on.then_some(ms),
            });
        }
        self.send_paint(status, save);
        self.notice = Some(if save {
            "Sent and saved on the devices.".into()
        } else {
            "Sent to the devices.".into()
        });
    }

    // -----------------------------------------------------------------------
    // Top of the window
    // -----------------------------------------------------------------------

    fn header(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Framework 16 Lights").size(18.0).strong());
            ui.add_space(8.0);
            for kind in [DeviceKind::Keyboard, DeviceKind::Macropad] {
                let (dot, text) = match status.slot(kind) {
                    Slot::Missing => (egui::Color32::GRAY, format!("{}: not found", kind.name())),
                    Slot::Stock => (
                        egui::Color32::from_rgb(230, 160, 40),
                        format!("{}: needs the f16custom firmware", kind.name()),
                    ),
                    Slot::Ready(d) => (
                        egui::Color32::from_rgb(63, 185, 80),
                        if d.streaming {
                            format!("{}: PC is driving the lights", kind.name())
                        } else {
                            format!("{}: connected", kind.name())
                        },
                    ),
                };
                if kind == DeviceKind::Macropad && self.s.setup.side == SideModule::Numpad {
                    ui.label(
                        RichText::new("● Numpad (white backlight)").color(egui::Color32::GRAY),
                    );
                    continue;
                }
                ui.label(RichText::new("●").color(dot));
                ui.label(text);
            }
            if ui.small_button("Search again").clicked() {
                self.engine.send(Command::Rescan);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let locked = [DeviceKind::Keyboard, DeviceKind::Macropad]
                    .iter()
                    .any(|k| Self::ready(status, *k).is_some_and(|d| d.info.hotkey_lock));
                let mut lock = locked;
                if switch(ui, &mut lock, "Hotkey lock") {
                    self.engine.send(Command::SetHotkeyLock(lock));
                }
            });
        });
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (t, name) in [
                (Tab::Show, "Show"),
                (Tab::Effects, "Effects"),
                (Tab::Macropad, "Macropad"),
                (Tab::Firmware, "Firmware"),
            ] {
                let selected = self.tab == t;
                let text = RichText::new(name).size(15.0);
                let resp = ui.add(
                    egui::Button::new(if selected { text.strong() } else { text })
                        .frame(false)
                        .min_size(Vec2::new(90.0, 34.0)),
                );
                if selected {
                    let r = resp.rect;
                    ui.painter().line_segment(
                        [r.left_bottom(), r.right_bottom()],
                        egui::Stroke::new(3.0, ACCENT),
                    );
                }
                if resp.clicked() {
                    self.tab = t;
                }
            }
        });
    }

    fn setup_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Your setup").strong());
            ui.add_space(6.0);
            ui.label("Keyboard");
            segmented(
                ui,
                &mut self.s.setup.kb,
                &[
                    (KbType::ClearRgb, "Clear RGB"),
                    (KbType::BlackRgb, "Black RGB"),
                    (KbType::White, "Black, white backlight"),
                ],
            );
            ui.add_space(6.0);
            ui.label("Side module");
            segmented(
                ui,
                &mut self.s.setup.side,
                &[
                    (SideModule::None, "None"),
                    (SideModule::Macropad, "RGB macropad"),
                    (SideModule::Numpad, "Numpad"),
                ],
            );
            if self.s.setup.side != SideModule::None {
                ui.add_space(6.0);
                ui.label("Placed on the");
                segmented(
                    ui,
                    &mut self.s.setup.module_left,
                    &[(true, "Left"), (false, "Right")],
                );
            }
        });
        let mut notes = vec![];
        if self.s.setup.kb == KbType::White {
            notes.push("This keyboard has one white light for all keys: it can only change brightness, so it shows plain white and sits out of effects.");
        }
        if self.s.setup.side == SideModule::Numpad {
            notes.push("The numpad has one white light for all keys: it can only change brightness, so it shows plain white and sits out of effects.");
        }
        for n in notes {
            ui.label(RichText::new(n).weak());
        }
    }

    // -----------------------------------------------------------------------
    // Tabs
    // -----------------------------------------------------------------------

    fn show_tab(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.horizontal_wrapped(|ui| {
            ui.label("One effect across the keyboard and side module as one surface, plus your own key colors on top.");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, "Save as default").on_hover_text("Send and keep it after a restart").clicked() {
                    let look = self.s.show;
                    self.send_looks(status, look, look, true, true);
                }
                if self.pc_live && ui.add(egui::Button::new("Stop")).clicked() {
                    self.engine.send(Command::SetLinked(None));
                    self.pc_live = false;
                }
                if primary(ui, "Send to devices").clicked() {
                    let look = self.s.show;
                    self.send_looks(status, look, look, true, false);
                }
            });
        });

        ui.columns(2, |cols| {
            card(&mut cols[0], |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Effect").size(16.0).strong());
                    ui.label(RichText::new(self.s.show.choice.name()).weak());
                });
                let group = |ui: &mut egui::Ui, name: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                    ui.add_space(2.0);
                    ui.label(RichText::new(name).weak());
                    ui.horizontal_wrapped(|ui| add(ui));
                };
                let choice = &mut self.s.show.choice;
                group(ui, "Off", &mut |ui| {
                    if ui
                        .selectable_label(*choice == Choice::PaintOnly, "Off (painted keys only)")
                        .clicked()
                    {
                        *choice = Choice::PaintOnly;
                    }
                });
                for (name, ids) in GROUPS {
                    group(ui, name, &mut |ui| {
                        for &id in *ids {
                            let label = format!("{id} {}", EFFECT_NAMES[id as usize]);
                            if ui
                                .selectable_label(*choice == Choice::Builtin(id), label)
                                .clicked()
                            {
                                *choice = Choice::Builtin(id);
                            }
                        }
                    });
                }
                group(ui, "Drawn by the app", &mut |ui| {
                    for e in LinkedEffect::ALL {
                        if ui
                            .selectable_label(*choice == Choice::Pc(e), e.name())
                            .clicked()
                        {
                            *choice = Choice::Pc(e);
                        }
                    }
                });
            });
            card(&mut cols[1], |ui| {
                ui.label(
                    RichText::new("Effect color, speed and brightness")
                        .size(16.0)
                        .strong(),
                );
                let l = &mut self.s.show;
                widgets::color_picker(ui, &mut l.hue, &mut l.sat);
                widgets::slider_row(ui, "Speed", &mut l.speed);
                widgets::slider_row(ui, "Brightness", &mut l.val);
            });
        });

        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Key colors").size(16.0).strong());
                let n = self.s.paint.len();
                ui.label(
                    RichText::new(format!(
                        "{n} {}",
                        if n == 1 {
                            "key painted"
                        } else {
                            "keys painted"
                        }
                    ))
                    .weak(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if button(ui, "Clear painted keys").clicked() {
                        self.s.paint.clear();
                    }
                    if button(ui, "Example: WASD and arrows").clicked() {
                        for (i, k) in keys::keyboard_keys().iter().enumerate() {
                            if [
                                "W", "A", "S", "D", "\u{2190}", "\u{2191}", "\u{2192}", "\u{2193}",
                            ]
                            .contains(&k.main.as_str())
                            {
                                self.s.paint.insert((Dev::Keyboard, i), self.s.brush);
                            }
                        }
                    }
                    segmented(
                        ui,
                        &mut self.tool,
                        &[(Tool::Erase, "Erase"), (Tool::Paint, "Paint")],
                    );
                });
            });
            ui.label(RichText::new("Pick a color, then click or drag across keys in the picture. Painted keys keep their color while the effect keeps running around them.").weak());
            ui.horizontal(|ui| {
                switch(
                    ui,
                    &mut self.s.follow,
                    "Painted keys still react to the effect",
                );
                ui.label(
                    RichText::new(if self.s.follow {
                        "They brighten and dim with the effect."
                    } else {
                        "They stay at a steady color."
                    })
                    .weak(),
                );
            });
            let (mut h, mut s) = self.s.brush;
            widgets::color_picker(ui, &mut h, &mut s);
            self.s.brush = (h, s);
        });
    }

    fn effects_tab(&mut self, ui: &mut egui::Ui, status: &Status) {
        let has_module = self.s.setup.side != SideModule::None;
        ui.horizontal_wrapped(|ui| {
            ui.label(
                "Built-in effects run on the devices themselves, even when the app is closed.",
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary(ui, "Save as default").clicked() {
                    let (kb, m) = (
                        self.s.kb,
                        if self.s.join {
                            self.s.kb
                        } else {
                            self.s.module
                        },
                    );
                    let join = self.s.join;
                    self.send_looks(status, kb, m, join, true);
                }
                if button(ui, "Send to devices").clicked() {
                    let (kb, m) = (
                        self.s.kb,
                        if self.s.join {
                            self.s.kb
                        } else {
                            self.s.module
                        },
                    );
                    let join = self.s.join;
                    self.send_looks(status, kb, m, join, false);
                }
                if has_module {
                    switch(ui, &mut self.s.join, "Join into one device");
                }
            });
        });
        let module_name = if self.s.setup.side == SideModule::Numpad {
            "Numpad"
        } else {
            "Macropad"
        };
        let kb_rgb = self.s.setup.kb.rgb();
        let mod_rgb = self.s.setup.side == SideModule::Macropad;
        if self.s.join && has_module {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Keyboard + {module_name}, joined"))
                            .size(16.0)
                            .strong(),
                    );
                    ui.label(RichText::new("one effect running across both as one surface").weak());
                });
                device_controls(ui, &mut self.s.kb, kb_rgb || mod_rgb, "joined");
            });
            self.s.module = self.s.kb;
            return;
        }
        let mut copy_to_module = false;
        let mut copy_to_kb = false;
        ui.columns(if has_module { 2 } else { 1 }, |cols| {
            card(&mut cols[0], |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Keyboard").size(16.0).strong());
                    let sub = match self.s.setup.kb {
                        KbType::ClearRgb => "clear RGB, 97 LEDs",
                        KbType::BlackRgb => "black RGB, 97 LEDs",
                        KbType::White => "black, white backlight",
                    };
                    ui.label(RichText::new(sub).weak());
                    if has_module && kb_rgb && mod_rgb {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            copy_to_module = ui.button(format!("Copy to {module_name}")).clicked();
                        });
                    }
                });
                device_controls(ui, &mut self.s.kb, kb_rgb, "kb");
            });
            if has_module {
                card(&mut cols[1], |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(module_name).size(16.0).strong());
                        ui.label(
                            RichText::new(if mod_rgb {
                                "clear RGB, 24 LEDs"
                            } else {
                                "white backlight"
                            })
                            .weak(),
                        );
                        if kb_rgb && mod_rgb {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    copy_to_kb = ui.button("Copy to Keyboard").clicked();
                                },
                            );
                        }
                    });
                    device_controls(ui, &mut self.s.module, mod_rgb, "module");
                });
            }
        });
        if copy_to_module {
            self.s.module = self.s.kb;
        }
        if copy_to_kb {
            self.s.kb = self.s.module;
        }
    }

    fn macropad_tab(&mut self, ui: &mut egui::Ui, status: &Status) {
        let mut layer = self.layer(status);
        ui.horizontal(|ui| {
            ui.label("Active layer");
            if segmented(ui, &mut layer, &[(0, "A: MIDI notes"), (1, "B: F13–F24 keys")]) {
                self.local_layer = layer;
                self.engine.send(Command::SetLayer(layer));
            }
            ui.label(RichText::new("Click a macropad key in the picture to see what it sends. Or press the two top corner keys together.").weak());
        });
        if self.s.setup.side != SideModule::Macropad {
            ui.label(
                "No macropad in your setup. Pick RGB macropad under Your setup to see its keys.",
            );
            return;
        }
        let (a, b) = keys::macropad_sends(self.pad_key);
        let note = match self.pad_key {
            0 | 3 => "Top corner: press both top corners together to switch layers. Pressed alone it still works, about 40 ms later.",
            20 | 23 => "Bottom corner: press both bottom corners together for the lighting controls.",
            _ => "Map it in Resolume under Shortcuts: MIDI for layer A, Keyboard for layer B.",
        };
        ui.columns(4, |cols| {
            for (ui, (k, v)) in cols.iter_mut().zip([
                (
                    "Selected key",
                    format!(
                        "Row {}, column {}",
                        self.pad_key / 4 + 1,
                        self.pad_key % 4 + 1
                    ),
                ),
                ("Layer A sends", a),
                ("Layer B sends", b),
                ("Note", note.to_string()),
            ]) {
                card(ui, |ui| {
                    ui.label(RichText::new(k).weak());
                    ui.label(RichText::new(v).strong());
                });
            }
        });
    }

    fn firmware_tab(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.label("Restarts a device into flashing mode. A drive called RPI-RP2 appears; drag the matching .uf2 file onto it. While a device is in flashing mode it does not type, so use the touchpad or an external keyboard for the drag and drop.");
        for kind in [DeviceKind::Keyboard, DeviceKind::Macropad] {
            if matches!(status.slot(kind), Slot::Missing) {
                continue;
            }
            ui.horizontal(|ui| {
                if self.confirm_bootloader == Some(kind) {
                    ui.label(format!("Really restart the {}?", kind.name()));
                    if ui.button("Yes, restart it").clicked() {
                        self.engine.send(Command::Bootloader(kind));
                        self.confirm_bootloader = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_bootloader = None;
                    }
                } else if button(
                    ui,
                    &format!("Restart {} into flashing mode", kind.name().to_lowercase()),
                )
                .clicked()
                {
                    self.confirm_bootloader = Some(kind);
                }
            });
        }
        if matches!(status.keyboard, Slot::Missing) && matches!(status.macropad, Slot::Missing) {
            ui.label(RichText::new("No keyboard or macropad found.").weak());
        }
    }
}

/// Effect, color, speed and brightness for one device (or both, joined).
fn device_controls(ui: &mut egui::Ui, look: &mut Look, rgb: bool, id: &str) {
    if rgb {
        let current = match look.choice {
            Choice::Builtin(id) => id,
            _ => 0,
        };
        let mut sel = current;
        ui.horizontal(|ui| {
            ui.add_sized([82.0, 20.0], egui::Label::new("Effect"));
            egui::ComboBox::from_id_salt(("effect", id))
                .width(ui.available_width().min(360.0))
                .selected_text(if sel == 0 {
                    "Off (painted keys only)".to_string()
                } else {
                    format!("{sel}. {}", EFFECT_NAMES[sel as usize])
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut sel, 0, "Off (painted keys only)");
                    for i in 1..=44u8 {
                        ui.selectable_value(
                            &mut sel,
                            i,
                            format!("{i}. {}", EFFECT_NAMES[i as usize]),
                        );
                    }
                });
        });
        if sel != current || matches!(look.choice, Choice::Pc(_)) {
            look.choice = if sel == 0 {
                Choice::PaintOnly
            } else {
                Choice::Builtin(sel)
            };
        }
        widgets::color_picker(ui, &mut look.hue, &mut look.sat);
        widgets::slider_row(ui, "Speed", &mut look.speed);
    } else {
        ui.label(
            RichText::new(
                "White backlight: one light for all keys, so only brightness can be changed.",
            )
            .weak(),
        );
    }
    widgets::slider_row(ui, "Brightness", &mut look.val);
}

impl eframe::App for LightsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let status = self.status();
        // Start the Effects tab from what each device is running.
        for (kind, look) in [
            (DeviceKind::Keyboard, &mut self.s.kb),
            (DeviceKind::Macropad, &mut self.s.module),
        ] {
            if self.adopted.contains(&kind) {
                continue;
            }
            if let Some(d) = Self::ready(&status, kind) {
                self.adopted.push(kind);
                let st = d.state;
                if (1..=44).contains(&st.mode) {
                    *look = Look {
                        choice: Choice::Builtin(st.mode),
                        hue: st.hue,
                        sat: st.sat,
                        val: st.val,
                        speed: st.speed,
                    };
                } else if st.mode == d.info.paint_only_effect {
                    look.choice = Choice::PaintOnly;
                }
            }
        }
        // First time a macropad shows up, put it in the setup.
        if !self.detected_module && Self::ready(&status, DeviceKind::Macropad).is_some() {
            self.detected_module = true;
            if self.s.setup.side == SideModule::None {
                self.s.setup.side = SideModule::Macropad;
            }
        }

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.header(ui, &status);
                ui.separator();
                self.tabs(ui);
                ui.separator();
                self.setup_bar(ui);
                ui.separator();

                ui.horizontal(|ui| {
                    if self.pc_live || status.linked {
                        ui.label(
                            RichText::new("● Live: the PC is drawing on your devices")
                                .color(egui::Color32::from_rgb(63, 185, 80)),
                        );
                    } else {
                        ui.label(
                            RichText::new(
                                "Preview: nothing changes on the devices until you press Send",
                            )
                            .color(ACCENT),
                        );
                    }
                    if let Some(n) = &self.notice {
                        ui.label(RichText::new(n).weak());
                    }
                    let scene_typing = self.s.show.choice.reacts_to_typing();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let joined = self.joined() && self.s.setup.side != SideModule::None;
                        ui.label(
                            RichText::new(format!(
                                "{}{}",
                                if scene_typing && self.tab == Tab::Show {
                                    "pretend typing on · "
                                } else {
                                    ""
                                },
                                if joined {
                                    "joined: one surface"
                                } else {
                                    "separate devices"
                                }
                            ))
                            .weak()
                            .monospace(),
                        );
                    });
                });

                let layer = self.layer(&status);
                let joined = self.joined();
                let (kb_look, mod_look) = match self.tab {
                    Tab::Show => (self.s.show, self.s.show),
                    _ if joined => (self.s.kb, self.s.kb),
                    _ => (self.s.kb, self.s.module),
                };
                let scene = Scene {
                    setup: self.s.setup,
                    joined,
                    kb: kb_look,
                    module: mod_look,
                    paint: &self.s.paint,
                    follow: self.s.follow,
                    macropad_layer: layer,
                    show_paint_marks: self.tab == Tab::Show,
                    selected_module_key: (self.tab == Tab::Macropad).then_some(self.pad_key),
                };
                let (resp, placed) = ui
                    .vertical_centered(|ui| deck::draw(ui, &scene, &mut self.preview, 420.0))
                    .inner;
                // Painting (Show tab) and picking macropad keys (Macropad tab)
                if let Some(pos) = resp.interact_pointer_pos() {
                    if let Some(k) = deck::key_at(&placed, pos) {
                        match self.tab {
                            Tab::Show
                                if k.rgb
                                    && (resp.dragged()
                                        || resp.clicked()
                                        || resp.is_pointer_button_down_on()) =>
                            {
                                match self.tool {
                                    Tool::Paint => {
                                        self.s.paint.insert((k.dev, k.index), self.s.brush);
                                    }
                                    Tool::Erase => {
                                        self.s.paint.remove(&(k.dev, k.index));
                                    }
                                }
                            }
                            Tab::Macropad if k.dev == Dev::Module && resp.clicked() => {
                                self.pad_key = k.index
                            }
                            _ => {}
                        }
                    }
                }
                ui.add_space(8.0);

                match self.tab {
                    Tab::Show => self.show_tab(ui, &status),
                    Tab::Effects => self.effects_tab(ui, &status),
                    Tab::Macropad => self.macropad_tab(ui, &status),
                    Tab::Firmware => self.firmware_tab(ui, &status),
                }
            });
        });

        if self.s != self.last_saved {
            settings::save(&self.s);
            self.last_saved = self.s.clone();
        }
        // the preview is animated
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }
}
