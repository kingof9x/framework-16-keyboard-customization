//! The live keyboard + side module picture: layout, preview colors and drawing.

use eframe::egui::{
    self, epaint, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind,
    Vec2,
};
use f16_core::builtin::{self, BuiltinSim, EffectCfg, SimLed};
use f16_core::effects::{EffectParams, LinkedEffect, Renderer, Rgb};
use f16_core::keys::{self, KeyDef, SideModule, KEYBOARD_SIZE, MODULE_GRID};
use f16_core::layout::Point;
use std::collections::HashMap;

/// What kind of keyboard the user has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KbType {
    /// Clear RGB keycaps: the whole cap glows.
    #[default]
    ClearRgb,
    /// Black RGB keycaps: light comes through the legends and around the keys.
    BlackRgb,
    /// Black keyboard with a single white backlight (no per-key color).
    White,
}

impl KbType {
    pub fn rgb(self) -> bool {
        self != KbType::White
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setup {
    pub kb: KbType,
    pub side: SideModule,
    pub module_left: bool,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            kb: KbType::ClearRgb,
            side: SideModule::Macropad,
            module_left: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dev {
    Keyboard,
    Module,
}

/// What is shown on the keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Everything off except painted keys.
    PaintOnly,
    /// Built-in effect 1..=44.
    Builtin(u8),
    /// Drawn by the app across both devices.
    Pc(LinkedEffect),
}

impl Choice {
    pub fn name(self) -> String {
        match self {
            Choice::PaintOnly => "Off (painted keys only)".into(),
            Choice::Builtin(id) => {
                format!("{id}. {}", f16_core::protocol::EFFECT_NAMES[id as usize])
            }
            Choice::Pc(e) => e.name().into(),
        }
    }
    pub fn reacts_to_typing(self) -> bool {
        match self {
            Choice::Builtin(id) => builtin::reacts_to_typing(id),
            Choice::Pc(e) => e == LinkedEffect::Ripple,
            Choice::PaintOnly => false,
        }
    }
}

/// One device's settings for the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    pub choice: Choice,
    pub hue: u8,
    pub sat: u8,
    pub val: u8,
    pub speed: u8,
}

impl Look {
    pub fn cfg(&self) -> EffectCfg {
        EffectCfg {
            effect: if let Choice::Builtin(id) = self.choice {
                id
            } else {
                0
            },
            hue: self.hue,
            sat: self.sat,
            val: self.val,
            speed: self.speed,
        }
    }
    pub fn pc_params(&self, effect: LinkedEffect) -> EffectParams {
        EffectParams {
            effect,
            speed: (0.05 + self.speed as f32 / 255.0 * 1.95).clamp(0.05, 2.0),
            hue: self.hue as f32 / 256.0,
            saturation: self.sat as f32 / 255.0,
            brightness: self.val as f32 / 255.0,
            reverse: false,
        }
    }
}

/// What the preview should show this frame.
pub struct Scene<'a> {
    pub setup: Setup,
    /// One effect across both devices (else `kb` / `module` separately).
    pub joined: bool,
    pub kb: Look,
    pub module: Look,
    pub paint: &'a HashMap<(Dev, usize), (u8, u8)>,
    pub follow: bool,
    pub macropad_layer: u8,
    pub show_paint_marks: bool,
    pub selected_module_key: Option<usize>,
}

/// A key placed on the deck.
#[derive(Debug, Clone)]
pub struct PlacedKey {
    pub dev: Dev,
    pub index: usize,
    pub def: KeyDef,
    pub rect: Rect,
    /// Centre in deck key units (for the app-drawn effects).
    pub deck: Point,
    /// Position on its own device in QMK's 0..224 x 0..64 space.
    pub qmk: (u8, u8),
    pub rgb: bool,
}

/// Deck geometry in key units (independent of screen size).
struct Geo {
    total_w: f32,
    total_h: f32,
    kb_x: f32,
    mod_x: f32,
    pad: f32,
    module_w: f32,
    spacer_w: f32,
}

const PAD: f32 = 0.45;
const SEAM: f32 = 0.05;

fn module_scale() -> f32 {
    KEYBOARD_SIZE.1 / MODULE_GRID.1
}

fn geo(setup: &Setup) -> Geo {
    let module_w = MODULE_GRID.0 * module_scale();
    let spacer_w = module_w / 2.0;
    let has = setup.side != SideModule::None;
    let inner = KEYBOARD_SIZE.0
        + if has {
            module_w + SEAM
        } else {
            2.0 * (spacer_w + SEAM)
        };
    let (kb_x, mod_x) = if !has {
        (PAD + spacer_w + SEAM, 0.0)
    } else if setup.module_left {
        (PAD + module_w + SEAM, PAD)
    } else {
        (PAD, PAD + KEYBOARD_SIZE.0 + SEAM)
    };
    Geo {
        total_w: inner + 2.0 * PAD,
        total_h: KEYBOARD_SIZE.1 + 2.0 * PAD,
        kb_x,
        mod_x,
        pad: PAD,
        module_w,
        spacer_w,
    }
}

/// Joined-surface slices for the firmware: (keyboard, side module).
pub fn span_slices(setup: &Setup) -> ((u8, u8), (u8, u8)) {
    let module_w = MODULE_GRID.0 * module_scale();
    builtin::span_slices(KEYBOARD_SIZE.0, module_w, SEAM, setup.module_left)
}

pub fn place_keys(setup: &Setup, layer: u8, origin: Pos2, unit: f32) -> Vec<PlacedKey> {
    let g = geo(setup);
    let mut out = Vec::new();
    let px = |x: f32, y: f32| origin + Vec2::new(x * unit, y * unit);
    for (i, def) in keys::keyboard_keys().into_iter().enumerate() {
        let r = Rect::from_min_size(
            px(g.kb_x + def.x, g.pad + def.y),
            Vec2::new(def.w * unit, def.h * unit),
        )
        .shrink(2.0);
        let (cx, cy) = def.center();
        out.push(PlacedKey {
            dev: Dev::Keyboard,
            index: i,
            qmk: keys::qmk_point(&def, KEYBOARD_SIZE),
            deck: Point {
                x: g.kb_x + cx,
                y: g.pad + cy,
            },
            rgb: setup.kb.rgb(),
            rect: r,
            def,
        });
    }
    let s = module_scale();
    for (i, def) in keys::module_keys(setup.side, layer).into_iter().enumerate() {
        let r = Rect::from_min_size(
            px(g.mod_x + def.x * s, g.pad + def.y * s),
            Vec2::new(def.w * s * unit, def.h * s * unit),
        )
        .shrink(2.0);
        let (cx, cy) = def.center();
        out.push(PlacedKey {
            dev: Dev::Module,
            index: i,
            qmk: keys::qmk_point(&def, MODULE_GRID),
            deck: Point {
                x: g.mod_x + cx * s,
                y: g.pad + cy * s,
            },
            rgb: setup.side == SideModule::Macropad,
            rect: r,
            def,
        });
    }
    out
}

/// Runs the effects for the preview, including pretend typing.
pub struct Preview {
    t0: std::time::Instant,
    joined_sim: BuiltinSim,
    kb_sim: BuiltinSim,
    mod_sim: BuiltinSim,
    renderer: Renderer,
    next_press: f64,
    rng: u64,
    last_key: Option<(Dev, usize, Choice)>,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            t0: std::time::Instant::now(),
            joined_sim: BuiltinSim::default(),
            kb_sim: BuiltinSim::default(),
            mod_sim: BuiltinSim::default(),
            renderer: Renderer::default(),
            next_press: 0.0,
            rng: 0x2545_F491_4F6C_DD1D,
            last_key: None,
        }
    }
}

fn white(val: u8) -> Rgb {
    let q = (235.0 * val as f32 / 255.0) as u8;
    [q, q, (q as f32 * 0.96) as u8]
}

impl Preview {
    pub fn time(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    fn rand(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 1_000_000) as f64 / 1_000_000.0
    }

    /// True when pretend typing is running (an effect that reacts to keys).
    pub fn typing(&self, scene: &Scene) -> bool {
        scene.kb.choice.reacts_to_typing()
            || (!scene.joined && scene.module.choice.reacts_to_typing())
    }

    /// Colors for every placed key.
    pub fn colors(&mut self, scene: &Scene, keys: &[PlacedKey]) -> Vec<Rgb> {
        let t = self.time();
        let kb_slice_mod = span_slices(&scene.setup);
        let squeeze = |k: &PlacedKey| -> SimLed {
            let slice = if k.dev == Dev::Keyboard {
                kb_slice_mod.0
            } else {
                kb_slice_mod.1
            };
            SimLed {
                x: builtin::squeeze_x(k.qmk.0, slice),
                y: k.qmk.1,
                modifier: k.def.is_modifier(),
            }
        };
        let rgb_keys: Vec<usize> = (0..keys.len()).filter(|&i| keys[i].rgb).collect();
        let look_of = |dev: Dev| {
            if scene.joined || dev == Dev::Keyboard {
                scene.kb
            } else {
                scene.module
            }
        };

        // forget effect memory when the effect changes
        let sig = (Dev::Keyboard, 0usize, scene.kb.choice);
        if self.last_key.map(|l| l.2) != Some(sig.2) {
            self.joined_sim.clear();
            self.kb_sim.clear();
            self.mod_sim.clear();
            self.last_key = Some(sig);
        }

        // pretend typing
        if self.typing(scene) && t >= self.next_press && !rgb_keys.is_empty() {
            let pick = rgb_keys[(self.rand() * rgb_keys.len() as f64) as usize % rgb_keys.len()];
            let k = &keys[pick];
            let jl = squeeze(k);
            let joined_index = rgb_keys.iter().position(|&i| i == pick).unwrap_or(0);
            self.joined_sim.press(joined_index, jl.x, jl.y, t);
            match k.dev {
                Dev::Keyboard => self.kb_sim.press(k.index, k.qmk.0, k.qmk.1, t),
                Dev::Module => self.mod_sim.press(k.index, k.qmk.0, k.qmk.1, t),
            }
            self.renderer.add_ripple(k.deck, t as f32);
            self.next_press = t + 0.25 + self.rand() * 0.6;
        }

        let g = geo(&scene.setup);
        let mut out = vec![[0u8; 3]; keys.len()];
        if scene.joined {
            let look = scene.kb;
            let cols: Vec<Rgb> = match look.choice {
                Choice::PaintOnly => vec![[0, 0, 0]; rgb_keys.len()],
                Choice::Pc(e) => {
                    self.renderer.params = look.pc_params(e);
                    self.renderer.prune(t as f32);
                    rgb_keys
                        .iter()
                        .map(|&i| self.renderer.color_at(keys[i].deck, t as f32, g.total_w))
                        .collect()
                }
                Choice::Builtin(_) => {
                    let leds: Vec<SimLed> = rgb_keys.iter().map(|&i| squeeze(&keys[i])).collect();
                    self.joined_sim.render(&look.cfg(), &leds, t)
                }
            };
            for (j, &i) in rgb_keys.iter().enumerate() {
                out[i] = cols[j];
            }
        } else {
            for dev in [Dev::Keyboard, Dev::Module] {
                let idx: Vec<usize> = rgb_keys
                    .iter()
                    .copied()
                    .filter(|&i| keys[i].dev == dev)
                    .collect();
                if idx.is_empty() {
                    continue;
                }
                let look = look_of(dev);
                let leds: Vec<SimLed> = idx
                    .iter()
                    .map(|&i| SimLed {
                        x: keys[i].qmk.0,
                        y: keys[i].qmk.1,
                        modifier: keys[i].def.is_modifier(),
                    })
                    .collect();
                let sim = if dev == Dev::Keyboard {
                    &mut self.kb_sim
                } else {
                    &mut self.mod_sim
                };
                let cols = match look.choice {
                    Choice::Builtin(_) => sim.render(&look.cfg(), &leds, t),
                    _ => vec![[0, 0, 0]; idx.len()],
                };
                for (j, &i) in idx.iter().enumerate() {
                    out[i] = cols[j];
                }
            }
        }

        // white-backlight devices and painted keys
        for (i, k) in keys.iter().enumerate() {
            let look = look_of(k.dev);
            if !k.rgb {
                out[i] = white(look.val);
                continue;
            }
            if let Some(&(h, s)) = scene.paint.get(&(k.dev, k.index)) {
                let lvl = out[i].iter().copied().max().unwrap_or(0) as f32 / 255.0;
                let follow = scene.follow && look.choice != Choice::PaintOnly;
                let v = if follow {
                    lvl.max(0.3 * look.val as f32 / 255.0) * 255.0
                } else {
                    look.val as f32
                };
                out[i] = builtin::qmk_hsv(h as i32, s as i32, v as i32);
            }
        }
        out
    }
}

fn rgb32(c: Rgb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn glow(painter: &egui::Painter, rect: Rect, radius: u8, c: Rgb, alpha: f32, blur: f32) {
    if alpha <= 0.01 {
        return;
    }
    let shadow = epaint::Shadow {
        offset: [0, 0],
        blur: blur.clamp(0.0, 255.0) as u8,
        spread: 1,
        color: Color32::from_rgba_unmultiplied(c[0], c[1], c[2], (alpha * 255.0) as u8),
    };
    painter.add(shadow.as_shape(rect, radius));
}

/// Draws the deck and returns the placed keys and the response (for painting / picking).
pub fn draw(
    ui: &mut egui::Ui,
    scene: &Scene,
    preview: &mut Preview,
    height_cap: f32,
) -> (egui::Response, Vec<PlacedKey>) {
    let g = geo(&scene.setup);
    let avail = ui.available_width().min(1180.0);
    let unit = (avail / g.total_w).min(height_cap / g.total_h);
    let size = Vec2::new(g.total_w * unit, g.total_h * unit);
    let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let origin = resp.rect.min;
    let keys = place_keys(&scene.setup, scene.macropad_layer, origin, unit);
    let colors = preview.colors(scene, &keys);

    // laptop deck and module plates
    painter.rect_filled(
        resp.rect,
        CornerRadius::same(16),
        Color32::from_rgb(0x64, 0x66, 0x6c),
    );
    let plate = |x: f32, w: f32, r: CornerRadius| {
        let rect = Rect::from_min_size(
            origin + Vec2::new((x - 0.18) * unit, (g.pad - 0.18) * unit),
            Vec2::new((w + 0.36) * unit, (KEYBOARD_SIZE.1 + 0.36) * unit),
        );
        painter.rect_filled(rect, r, Color32::from_rgb(0x0c, 0x0c, 0x0e));
        rect
    };
    let has = scene.setup.side != SideModule::None;
    let (rl, rr) = (
        CornerRadius {
            nw: 9,
            sw: 9,
            ne: 0,
            se: 0,
        },
        CornerRadius {
            nw: 0,
            sw: 0,
            ne: 9,
            se: 9,
        },
    );
    let kb_plate = plate(
        g.kb_x,
        KEYBOARD_SIZE.0,
        if !has {
            CornerRadius::same(9)
        } else if scene.setup.module_left {
            rr
        } else {
            rl
        },
    );
    if has {
        let mp = plate(
            g.mod_x,
            g.module_w,
            if scene.setup.module_left { rl } else { rr },
        );
        let seam_x = if scene.setup.module_left {
            kb_plate.left()
        } else {
            mp.left()
        };
        painter.line_segment(
            [
                Pos2::new(seam_x, kb_plate.top()),
                Pos2::new(seam_x, kb_plate.bottom()),
            ],
            Stroke::new(1.5, Color32::BLACK),
        );
    } else {
        for x in [g.pad, g.kb_x + KEYBOARD_SIZE.0 + SEAM] {
            let r = Rect::from_min_size(
                origin + Vec2::new((x - 0.18) * unit, (g.pad - 0.18) * unit),
                Vec2::new(g.spacer_w * unit, (KEYBOARD_SIZE.1 + 0.36) * unit),
            );
            painter.rect_filled(
                r,
                CornerRadius::same(9),
                Color32::from_rgb(0x72, 0x74, 0x7a),
            );
        }
    }

    let fs = (unit * 0.3).max(8.0);
    let accent = ui.visuals().selection.bg_fill;
    for (k, &c) in keys.iter().zip(&colors) {
        let material = match k.dev {
            Dev::Keyboard => scene.setup.kb,
            Dev::Module => {
                if scene.setup.side == SideModule::Macropad {
                    KbType::ClearRgb
                } else {
                    KbType::White
                }
            }
        };
        draw_key(&painter, k, c, material, fs);
        if scene.show_paint_marks && scene.paint.contains_key(&(k.dev, k.index)) {
            painter.circle_filled(
                Pos2::new(k.rect.right() - 6.0, k.rect.top() + 6.0),
                2.6,
                Color32::from_white_alpha(230),
            );
        }
        if k.dev == Dev::Module && scene.selected_module_key == Some(k.index) {
            painter.rect_stroke(
                k.rect,
                CornerRadius::same(7),
                Stroke::new(2.5, accent),
                StrokeKind::Inside,
            );
        }
    }
    (resp, keys)
}

fn draw_key(painter: &egui::Painter, k: &PlacedKey, c: Rgb, material: KbType, fs: f32) {
    let r = k.rect;
    let lvl = c.iter().copied().max().unwrap_or(0) as f32 / 255.0;
    let face = Rect::from_min_max(r.min + Vec2::new(4.0, 3.0), r.max - Vec2::new(4.0, 7.0));
    let legend_color = match material {
        KbType::ClearRgb => {
            glow(painter, r, 7, c, 0.25 + 0.6 * lvl, 3.0 + 16.0 * lvl);
            // frosted white when dark, LED color when lit
            let frost = 150.0 * (1.0 - lvl) + 40.0;
            let f = |q: u8| (frost + q as f32 * 0.85).min(255.0) as u8;
            let fc = [f(c[0]), f(c[1]), f(c[2])];
            painter.rect_filled(
                r,
                CornerRadius::same(7),
                Color32::from_rgb(
                    (fc[0] as f32 * 0.62) as u8,
                    (fc[1] as f32 * 0.62) as u8,
                    (fc[2] as f32 * 0.62) as u8,
                ),
            );
            painter.rect_filled(face, CornerRadius::same(5), rgb32(fc));
            painter.line_segment(
                [
                    face.left_top() + Vec2::new(4.0, 0.5),
                    face.right_top() + Vec2::new(-4.0, 0.5),
                ],
                Stroke::new(1.0, Color32::from_white_alpha(90)),
            );
            let lum = 0.299 * fc[0] as f32 + 0.587 * fc[1] as f32 + 0.114 * fc[2] as f32;
            if lum > 125.0 {
                Color32::from_rgba_unmultiplied(20, 20, 24, 215)
            } else {
                Color32::from_white_alpha(235)
            }
        }
        KbType::BlackRgb => {
            if lvl > 0.05 {
                glow(
                    painter,
                    r.expand(1.0),
                    8,
                    c,
                    0.35 + 0.55 * lvl,
                    4.0 + 14.0 * lvl,
                );
            }
            painter.rect_filled(
                r,
                CornerRadius::same(7),
                Color32::from_rgb(0x14, 0x14, 0x16),
            );
            painter.rect_filled(
                face,
                CornerRadius::same(5),
                Color32::from_rgb(0x29, 0x29, 0x2e),
            );
            if lvl < 0.06 {
                Color32::from_rgb(0x55, 0x55, 0x5d)
            } else {
                rgb32(c)
            }
        }
        KbType::White => {
            painter.rect_filled(
                r,
                CornerRadius::same(7),
                Color32::from_rgb(0x14, 0x14, 0x16),
            );
            painter.rect_filled(
                face,
                CornerRadius::same(5),
                Color32::from_rgb(0x29, 0x29, 0x2e),
            );
            if lvl < 0.06 {
                Color32::from_rgb(0x55, 0x55, 0x5d)
            } else {
                rgb32(c)
            }
        }
    };
    // legends
    let main = &k.def.main;
    let long = main.chars().count() > 2 && r.width() > fs * 5.0;
    let (anchor, x) = if long {
        (Align2::LEFT_CENTER, face.left() + 6.0)
    } else {
        (Align2::CENTER_CENTER, face.center().x)
    };
    let cy = face.center().y;
    if !k.def.top.is_empty() && r.height() >= fs * 2.6 {
        painter.text(
            Pos2::new(x, cy - fs * 0.6),
            anchor,
            &k.def.top,
            FontId::proportional(fs * 0.78),
            legend_color.gamma_multiply(0.8),
        );
        painter.text(
            Pos2::new(x, cy + fs * 0.55),
            anchor,
            main,
            FontId::proportional(fs),
            legend_color,
        );
    } else {
        let size = if r.height() < fs * 2.0 {
            fs * 0.72
        } else if main.chars().count() <= 2 {
            fs * 1.12
        } else {
            fs * 0.82
        };
        painter.text(
            Pos2::new(x, cy),
            anchor,
            main,
            FontId::proportional(size),
            legend_color,
        );
    }
}

/// Key under a screen position.
pub fn key_at(keys: &[PlacedKey], pos: Pos2) -> Option<&PlacedKey> {
    keys.iter().find(|k| k.rect.contains(pos))
}
