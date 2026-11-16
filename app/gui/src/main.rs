//! Framework 16 Lights: controls the f16custom firmware on the Framework
//! Laptop 16 RGB keyboard and RGB macropad.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window

mod device;
mod engine;

use eframe::egui;
use engine::{Command, EngineHandle, Slot, Status};
use f16_core::effects::{EffectParams, LinkedEffect};
use f16_core::layout::MacropadSide;
use f16_core::protocol::{effect_name, DeviceKind, LightState};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Framework 16 Lights")
            .with_inner_size([760.0, 720.0])
            .with_min_inner_size([520.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Framework 16 Lights",
        options,
        Box::new(|cc| Ok(Box::new(LightsApp::new(cc)))),
    )
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Separate,
    Linked,
}

struct LightsApp {
    engine: EngineHandle,
    mode: Mode,
    linked: EffectParams,
    side: MacropadSide,
    confirm_bootloader: Option<DeviceKind>,
}

impl LightsApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        let engine = engine::start(move || ctx.request_repaint());
        Self {
            engine,
            mode: Mode::Separate,
            linked: EffectParams::default(),
            side: MacropadSide::Right,
            confirm_bootloader: None,
        }
    }

    fn status(&self) -> Status {
        self.engine.status.lock().unwrap().clone()
    }

    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        match mode {
            Mode::Linked => self.engine.send(Command::SetLinked(Some(self.linked))),
            Mode::Separate => self.engine.send(Command::SetLinked(None)),
        }
    }

    // -----------------------------------------------------------------------

    fn device_header(&mut self, ui: &mut egui::Ui, kind: DeviceKind, slot: &Slot) {
        ui.horizontal(|ui| {
            ui.strong(kind.name());
            match slot {
                Slot::Missing => {
                    ui.colored_label(egui::Color32::GRAY, "not found");
                }
                Slot::Stock => {
                    ui.colored_label(
                        egui::Color32::from_rgb(230, 160, 40),
                        "found, but not running the f16custom firmware (see README: Flashing)",
                    );
                }
                Slot::Ready(d) => {
                    ui.colored_label(
                        egui::Color32::from_rgb(80, 200, 120),
                        "connected (f16custom firmware)",
                    );
                    ui.weak(format!("{} LEDs", d.info.led_count));
                    if d.info.hotkey_lock {
                        ui.weak("· hotkeys locked");
                    }
                    if d.streaming {
                        ui.weak("· PC is driving the lights");
                    }
                }
            }
        });
    }

    /// "Restart into flashing mode" buttons, with a confirmation step.
    fn flashing_section(&mut self, ui: &mut egui::Ui, status: &Status) {
        egui::CollapsingHeader::new("Firmware update").show(ui, |ui| {
            ui.label(
                "Restarts a device into flashing mode. A USB drive called RPI-RP2 appears; \
                 drag the matching .uf2 file onto it. The device stops typing until a file is \
                 copied (or the laptop is restarted), so use an external keyboard/mouse or \
                 the touchpad for the drag and drop.",
            );
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
                    } else if ui
                        .button(format!("Restart {} into flashing mode", kind.name()))
                        .clicked()
                    {
                        self.confirm_bootloader = Some(kind);
                    }
                });
            }
        });
    }

    /// Controls for one device's built-in effect.
    fn separate_controls(&self, ui: &mut egui::Ui, kind: DeviceKind, slot: &Slot) {
        let Slot::Ready(d) = slot else { return };
        let mut s = d.state;
        let before = s;
        let host = d.info.host_effect;

        ui.horizontal(|ui| {
            ui.checkbox(&mut s.enabled, "Lights on");
            let mut selected = s.mode;
            egui::ComboBox::from_id_salt(("effect", kind.name()))
                .width(240.0)
                .selected_text(effect_name(selected, host))
                .show_ui(ui, |ui| {
                    for m in 1..d.info.effect_count {
                        if m == host {
                            continue; // only used by Linked mode
                        }
                        ui.selectable_value(&mut selected, m, effect_name(m, host));
                    }
                });
            if selected != s.mode {
                s.mode = selected;
            }
        });
        ui.add(egui::Slider::new(&mut s.val, 0..=d.info.max_brightness).text("Brightness"));
        ui.add(egui::Slider::new(&mut s.speed, 0..=255).text("Speed"));
        ui.horizontal(|ui| {
            ui.label("Colour");
            let mut hsva =
                egui::ecolor::Hsva::new(s.hue as f32 / 255.0, s.sat as f32 / 255.0, 1.0, 1.0);
            if egui::color_picker::color_edit_button_hsva(
                ui,
                &mut hsva,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                s.hue = (hsva.h.rem_euclid(1.0) * 255.0).round() as u8;
                s.sat = (hsva.s.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            ui.add(egui::Slider::new(&mut s.hue, 0..=255).text("Hue"));
            ui.add(egui::Slider::new(&mut s.sat, 0..=255).text("Saturation"));
        });

        if s.mode == host {
            ui.weak("This device is showing the PC-controlled effect. Pick an effect above to change it.");
        }

        if s != before && s.mode != host {
            self.apply_state(kind, s, false);
        }
        ui.horizontal(|ui| {
            if ui
                .button("Save as default")
                .on_hover_text(
                    "Write these settings to the device so they stay after a restart/unplug",
                )
                .clicked()
                && s.mode != host
            {
                self.apply_state(kind, s, true);
            }
            ui.weak("Changes apply live; \"Save as default\" keeps them after unplugging.");
        });
    }

    fn apply_state(&self, kind: DeviceKind, state: LightState, save: bool) {
        // Show the change immediately; the engine re-reads the device every second.
        if let Slot::Ready(d) = self.engine.status.lock().unwrap().slot_mut(kind) {
            d.state = state;
        }
        self.engine.send(Command::SetState { kind, state, save });
    }

    fn linked_controls(&mut self, ui: &mut egui::Ui, status: &Status) {
        let before = self.linked;
        let p = &mut self.linked;
        ui.horizontal(|ui| {
            ui.label("Effect");
            for e in LinkedEffect::ALL {
                ui.selectable_value(&mut p.effect, e, e.name());
            }
        });
        ui.add(egui::Slider::new(&mut p.speed, 0.05..=2.0).text("Speed"));
        ui.add(egui::Slider::new(&mut p.brightness, 0.0..=1.0).text("Brightness"));
        if p.effect != LinkedEffect::RainbowSweep {
            ui.horizontal(|ui| {
                ui.label("Colour");
                let mut hsva = egui::ecolor::Hsva::new(p.hue, p.saturation, 1.0, 1.0);
                if egui::color_picker::color_edit_button_hsva(
                    ui,
                    &mut hsva,
                    egui::color_picker::Alpha::Opaque,
                )
                .changed()
                {
                    p.hue = hsva.h;
                    p.saturation = hsva.s;
                }
            });
        }
        if p.effect != LinkedEffect::Ripple {
            ui.checkbox(&mut p.reverse, "Right to left");
        } else {
            ui.weak("Press keys on the keyboard or macropad to send ripples across both.");
        }
        if *p != before {
            self.engine.send(Command::SetLinked(Some(self.linked)));
        }

        ui.horizontal(|ui| {
            ui.label("Macropad is on the");
            let before = self.side;
            ui.radio_value(&mut self.side, MacropadSide::Left, "left");
            ui.radio_value(&mut self.side, MacropadSide::Right, "right");
            ui.label("of the keyboard");
            if self.side != before {
                self.engine.send(Command::SetSide(self.side));
            }
        });

        ui.add_space(6.0);
        Self::preview(ui, status);
        ui.weak(format!(
            "{:.0} frames/s. The device brightness (Fn keys / Separate tab) still scales these colours.",
            status.fps
        ));
    }

    /// Draws the combined layout with the colours currently being sent.
    fn preview(ui: &mut egui::Ui, status: &Status) {
        let (w, h) = status.layout_size;
        let avail = ui.available_width();
        let height = if w > 0.0 { avail * h / w } else { 60.0 };
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(avail, height.max(40.0)), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, egui::Color32::from_gray(20));
        if w <= 0.0 || status.preview.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "no preview",
                egui::FontId::proportional(14.0),
                egui::Color32::GRAY,
            );
            return;
        }
        let pad = 10.0;
        let sx = (rect.width() - 2.0 * pad) / w;
        let sy = (rect.height() - 2.0 * pad) / h;
        let r = (sx.min(sy) * 0.35).max(2.0);
        for (pt, c) in &status.preview {
            let pos = egui::pos2(rect.left() + pad + pt.x * sx, rect.top() + pad + pt.y * sy);
            painter.circle_filled(pos, r, egui::Color32::from_rgb(c[0], c[1], c[2]));
        }
    }
}

impl eframe::App for LightsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let status = self.status();
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Framework 16 Lights");
                self.device_header(ui, DeviceKind::Keyboard, &status.keyboard);
                self.device_header(ui, DeviceKind::Macropad, &status.macropad);
                self.flashing_section(ui, &status);
                ui.horizontal(|ui| {
                    if ui.button("Search again").clicked() {
                        self.engine.send(Command::Rescan);
                    }
                    if let Some(m) = &status.message {
                        ui.weak(m);
                    }
                });
                ui.separator();

                // Hotkey lock
                let locked = [&status.keyboard, &status.macropad]
                    .iter()
                    .any(|s| matches!(s, Slot::Ready(d) if d.info.hotkey_lock));
                let mut lock = locked;
                ui.checkbox(&mut lock, "Hotkey lock: Fn lighting keys do nothing")
                    .on_hover_text("Stops Fn+lighting keys (and the macropad lighting layer) from changing the lights by accident. Saved on the devices.");
                if lock != locked {
                    self.engine.send(Command::SetHotkeyLock(lock));
                }

                // Macropad layer
                if let Slot::Ready(d) = &status.macropad {
                    if let Some(layer) = d.info.layer {
                        ui.horizontal(|ui| {
                            ui.label("Macropad layer:");
                            let mut l = layer;
                            ui.radio_value(&mut l, 0, "A: MIDI notes");
                            ui.radio_value(&mut l, 1, "B: F13–F24 keys");
                            if l != layer {
                                self.engine.send(Command::SetLayer(l));
                            }
                        });
                        ui.weak("Tip: press the two top corner keys of the macropad together to switch.");
                    }
                }
                ui.separator();

                ui.horizontal(|ui| {
                    let mut mode = self.mode;
                    ui.selectable_value(&mut mode, Mode::Separate, "Separate effects");
                    ui.selectable_value(&mut mode, Mode::Linked, "Linked (PC drives both)");
                    if mode != self.mode {
                        self.set_mode(mode);
                    }
                });
                ui.add_space(6.0);

                match self.mode {
                    Mode::Separate => {
                        for kind in [DeviceKind::Keyboard, DeviceKind::Macropad] {
                            let slot = status.slot(kind);
                            if matches!(slot, Slot::Ready(_)) {
                                egui::CollapsingHeader::new(kind.name()).default_open(true).show(ui, |ui| {
                                    self.separate_controls(ui, kind, slot);
                                });
                            }
                        }
                        if !matches!(status.keyboard, Slot::Ready(_)) && !matches!(status.macropad, Slot::Ready(_)) {
                            ui.label("No device with the f16custom firmware found yet.");
                        }
                    }
                    Mode::Linked => self.linked_controls(ui, &status),
                }
            });
        });
    }
}
