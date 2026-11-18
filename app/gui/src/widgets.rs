//! Small controls used across the tabs.

use eframe::egui::{
    self, epaint, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use f16_core::builtin::qmk_hsv;

/// Framework orange.
pub const ACCENT: Color32 = Color32::from_rgb(0xF7, 0x58, 0x21);
/// Text on the accent color.
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);

/// An on/off switch with a label. Returns true when clicked.
pub fn switch(ui: &mut egui::Ui, on: &mut bool, label: &str) -> bool {
    let resp = ui
        .horizontal(|ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());
            let track = if *on {
                ACCENT
            } else {
                ui.visuals().widgets.inactive.bg_fill
            };
            ui.painter()
                .rect_filled(rect, CornerRadius::same(10), track);
            let x = if *on {
                rect.right() - 10.0
            } else {
                rect.left() + 10.0
            };
            ui.painter()
                .circle_filled(Pos2::new(x, rect.center().y), 8.0, Color32::WHITE);
            let l =
                ui.add(egui::Label::new(egui::RichText::new(label).strong()).sense(Sense::click()));
            resp.union(l)
        })
        .inner;
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        false
    }
}

/// Segmented buttons. Returns true when the value changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (v, label) in options {
            if ui.selectable_label(*value == *v, *label).clicked() && *value != *v {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

/// A slider with a painted track (rainbow for hue, white-to-color for saturation).
fn painted_slider(ui: &mut egui::Ui, value: &mut u8, stops: &[Color32], width: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width() - 16.0, 12.0));
    let mut mesh = epaint::Mesh::default();
    let n = stops.len() - 1;
    for (i, c) in stops.iter().enumerate() {
        let x = track.left() + track.width() * i as f32 / n as f32;
        mesh.colored_vertex(Pos2::new(x, track.top()), *c);
        mesh.colored_vertex(Pos2::new(x, track.bottom()), *c);
    }
    for i in 0..n as u32 {
        let b = i * 2;
        mesh.add_triangle(b, b + 1, b + 2);
        mesh.add_triangle(b + 1, b + 2, b + 3);
    }
    ui.painter().add(mesh);
    ui.painter().rect_stroke(
        track,
        CornerRadius::same(2),
        Stroke::new(1.0, Color32::from_black_alpha(60)),
        StrokeKind::Outside,
    );
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos() {
        let v = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
        let nv = (v * 255.0).round() as u8;
        if nv != *value {
            *value = nv;
            changed = true;
        }
    }
    let x = track.left() + track.width() * *value as f32 / 255.0;
    ui.painter().circle(
        Pos2::new(x, track.center().y),
        9.0,
        Color32::WHITE,
        Stroke::new(2.0, Color32::from_rgb(0x14, 0x14, 0x14)),
    );
    changed
}

fn c32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Swatches shown under every color picker: (name, hue, sat).
pub const SWATCHES: [(&str, u8, u8); 9] = [
    ("Framework orange", 11, 220),
    ("Red", 0, 255),
    ("Yellow", 43, 255),
    ("Green", 85, 255),
    ("Cyan", 128, 255),
    ("Blue", 170, 255),
    ("Purple", 191, 255),
    ("Pink", 234, 200),
    ("White", 0, 0),
];

/// Hue + saturation sliders, swatches and the picked color with its hex code.
/// Covers every color the LEDs can make (QMK's 0..=255 hue and saturation).
pub fn color_picker(ui: &mut egui::Ui, hue: &mut u8, sat: &mut u8) -> bool {
    let mut changed = false;
    let w = (ui.available_width() - 140.0).clamp(160.0, 520.0);
    egui::Grid::new(ui.next_auto_id())
        .num_columns(3)
        .spacing([10.0, 8.0])
        .show(ui, |ui| {
            ui.label("Hue");
            let rainbow: Vec<Color32> = (0..=12)
                .map(|i| c32(qmk_hsv(i * 256 / 12, 255, 255)))
                .collect();
            changed |= painted_slider(ui, hue, &rainbow, w);
            ui.monospace(hue.to_string());
            ui.end_row();

            ui.label("Saturation");
            changed |= painted_slider(
                ui,
                sat,
                &[Color32::WHITE, c32(qmk_hsv(*hue as i32, 255, 255))],
                w,
            );
            ui.monospace(sat.to_string());
            ui.end_row();

            ui.label("Color");
            ui.horizontal_wrapped(|ui| {
                for (name, h, s) in SWATCHES {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
                    let selected = *hue == h && *sat == s;
                    ui.painter().circle_filled(
                        rect.center(),
                        12.0,
                        c32(qmk_hsv(h as i32, s as i32, 255)),
                    );
                    ui.painter().circle_stroke(
                        rect.center(),
                        12.5,
                        Stroke::new(
                            if selected { 3.0 } else { 1.0 },
                            if selected {
                                ui.visuals().text_color()
                            } else {
                                ui.visuals().widgets.noninteractive.bg_stroke.color
                            },
                        ),
                    );
                    if resp.on_hover_text(name).clicked() {
                        *hue = h;
                        *sat = s;
                        changed = true;
                    }
                }
            });
            ui.label("");
            ui.end_row();

            ui.label("");
            ui.horizontal(|ui| {
                let c = qmk_hsv(*hue as i32, *sat as i32, 255);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(6), c32(c));
                ui.monospace(format!(
                    "#{:02X}{:02X}{:02X}  ·  hue {}, sat {}",
                    c[0], c[1], c[2], hue, sat
                ));
            });
            ui.label("");
            ui.end_row();
        });
    changed
}

/// A 0..=255 slider row.
pub fn slider_row(ui: &mut egui::Ui, label: &str, value: &mut u8) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized([82.0, 20.0], egui::Label::new(label));
        ui.spacing_mut().slider_width = (ui.available_width() - 70.0).clamp(120.0, 520.0);
        ui.add(egui::Slider::new(value, 0..=255)).changed()
    })
    .inner
}

/// A card frame.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::group(ui.style())
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A big primary (orange) button.
pub fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).strong().color(ON_ACCENT))
            .fill(ACCENT)
            .min_size(Vec2::new(0.0, 36.0)),
    )
}

pub fn button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(text).strong()).min_size(Vec2::new(0.0, 36.0)))
}
