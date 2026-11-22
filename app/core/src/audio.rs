//! Audio reactive light shows: turn the loudness of each frequency band (bass to treble) into
//! colors for every LED across the keyboard and side module, drawn as one surface.
//!
//! The sound itself is captured and measured by White Rabbit's `wr-music` code in the app; this
//! module only gets band levels (0..1) and is the same on every platform.

use crate::effects::{hsv_to_rgb, Rgb};
use crate::layout::Point;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioLook {
    #[default]
    Spectrum,
    CenterOut,
    Meter,
    BeatPulse,
    BassRipples,
    ColorWash,
    Strobe,
}

impl AudioLook {
    pub const ALL: [AudioLook; 7] = [
        AudioLook::Spectrum,
        AudioLook::CenterOut,
        AudioLook::Meter,
        AudioLook::BeatPulse,
        AudioLook::BassRipples,
        AudioLook::ColorWash,
        AudioLook::Strobe,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AudioLook::Spectrum => "Spectrum bars",
            AudioLook::CenterOut => "Centre out",
            AudioLook::Meter => "Level meter",
            AudioLook::BeatPulse => "Beat pulse",
            AudioLook::BassRipples => "Bass ripples",
            AudioLook::ColorWash => "Color wash",
            AudioLook::Strobe => "Strobe",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            AudioLook::Spectrum => "Bars rise from the bottom row: bass on the left, treble on the right of the macropad.",
            AudioLook::CenterOut => "Bass in the middle of the keyboard, treble at both ends.",
            AudioLook::Meter => "The whole surface fills from the bottom row up with the loudness, with a peak mark that falls back slowly.",
            AudioLook::BeatPulse => "Everything flashes on each beat, and the color moves on with every beat.",
            AudioLook::BassRipples => "Each bass hit sends a ring of color out across the keys.",
            AudioLook::ColorWash => "Slow flowing colors that brighten with the music and shift with the treble.",
            AudioLook::Strobe => "White flashes on the strongest beats.",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            AudioLook::Spectrum => "spectrum",
            AudioLook::CenterOut => "center",
            AudioLook::Meter => "meter",
            AudioLook::BeatPulse => "pulse",
            AudioLook::BassRipples => "ripples",
            AudioLook::ColorWash => "wash",
            AudioLook::Strobe => "strobe",
        }
    }

    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|l| l.key() == k)
    }
}

/// The colors a show is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AudioColors {
    /// What suits the show: green, yellow and red for the bars and the meter, rainbow otherwise.
    #[default]
    Auto,
    Rainbow,
    /// Green, then yellow, then red at the top, like a sound level meter.
    Meter,
    /// Red, orange and yellow.
    Fire,
    /// Deep blue to cyan.
    Ocean,
    /// One color: hue and saturation, 0..1.
    One(f32, f32),
}

impl AudioColors {
    pub fn key(self) -> String {
        match self {
            AudioColors::Auto => "auto".into(),
            AudioColors::Rainbow => "rainbow".into(),
            AudioColors::Meter => "meter".into(),
            AudioColors::Fire => "fire".into(),
            AudioColors::Ocean => "ocean".into(),
            AudioColors::One(h, s) => {
                format!("one {} {}", (h * 255.0).round(), (s * 255.0).round())
            }
        }
    }

    pub fn from_key(k: &str) -> Option<Self> {
        let p: Vec<&str> = k.split_whitespace().collect();
        Some(match p.first().copied()? {
            "auto" => AudioColors::Auto,
            "rainbow" => AudioColors::Rainbow,
            "meter" => AudioColors::Meter,
            "fire" => AudioColors::Fire,
            "ocean" => AudioColors::Ocean,
            "one" => {
                let h: f32 = p.get(1)?.parse().ok()?;
                let s: f32 = p.get(2)?.parse().ok()?;
                AudioColors::One(h.clamp(0.0, 255.0) / 255.0, s.clamp(0.0, 255.0) / 255.0)
            }
            _ => return None,
        })
    }

    /// The colors `look` really uses.
    fn resolve(self, look: AudioLook) -> Self {
        match (self, look) {
            (AudioColors::Auto, AudioLook::Spectrum | AudioLook::CenterOut | AudioLook::Meter) => {
                AudioColors::Meter
            }
            (AudioColors::Auto, _) => AudioColors::Rainbow,
            (c, _) => c,
        }
    }

    /// Whether these colors go up the keyboard (by height) rather than across it.
    fn by_height(self) -> bool {
        matches!(self, AudioColors::Meter | AudioColors::Fire)
    }

    /// Hue and saturation at `t` (0..1) along the palette.
    fn at(self, t: f32) -> (f32, f32) {
        let t = t.clamp(0.0, 1.0);
        match self {
            AudioColors::Auto | AudioColors::Rainbow => (t * 0.8, 1.0),
            AudioColors::Meter => (
                if t < 0.55 {
                    0.33
                } else if t < 0.8 {
                    0.14
                } else {
                    0.0
                },
                1.0,
            ),
            AudioColors::Fire => (t * 0.14, 1.0),
            AudioColors::Ocean => (0.66 - t * 0.17, 1.0),
            AudioColors::One(h, s) => (h, s),
        }
    }

    fn rgb(self, t: f32, v: f32) -> Rgb {
        let (h, s) = self.at(t);
        hsv_to_rgb(h, s, v)
    }
}

/// Bands the shows use (bass first).
pub const BANDS: usize = 16;

struct Ring {
    born: f32,
    hue: f32,
}

/// The show's state between frames: smoothed levels, beats, rings.
pub struct AudioShow {
    pub look: AudioLook,
    pub colors: AudioColors,
    levels: [f32; BANDS],
    loudness: f32,
    peak: f32,
    bass_avg: f32,
    last_beat: f32,
    flash: f32,
    hue: f32,
    rings: Vec<Ring>,
    t: f32,
}

impl AudioShow {
    pub fn new(look: AudioLook) -> Self {
        Self {
            look,
            colors: AudioColors::Auto,
            levels: [0.0; BANDS],
            loudness: 0.0,
            peak: 0.0,
            bass_avg: 0.0,
            last_beat: -10.0,
            flash: 0.0,
            hue: 0.0,
            rings: Vec::new(),
            t: 0.0,
        }
    }

    /// Feeds the latest band levels (0..1, any number of bands, bass first). Returns true on a beat.
    pub fn update(&mut self, bands: &[f32], dt: f32) -> bool {
        self.t += dt;
        let n = bands.len().max(1);
        for (i, l) in self.levels.iter_mut().enumerate() {
            let src = bands
                .get(i * n / BANDS)
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            // fast up, slower down, so bars don't flicker
            let k = if src > *l { 0.6 } else { (dt * 6.0).min(1.0) };
            *l += (src - *l) * k;
        }
        let loud = bands.iter().sum::<f32>() / n as f32;
        self.loudness += (loud - self.loudness) * (dt * 10.0).min(1.0);
        self.peak = (self.peak - dt * 0.4).max(self.loudness);

        let bass = bands.iter().take((n / 6).max(1)).sum::<f32>() / (n / 6).max(1) as f32;
        let beat =
            bass > self.bass_avg * 1.35 + 0.05 && bass > 0.25 && self.t - self.last_beat > 0.18;
        self.bass_avg += (bass - self.bass_avg) * (dt * 2.5).min(1.0);
        if beat {
            self.last_beat = self.t;
            self.flash = 1.0;
            self.hue = (self.hue + 0.13).fract();
            self.rings.push(Ring {
                born: self.t,
                hue: self.hue,
            });
        }
        self.flash *= (-dt * 6.0).exp();
        let t = self.t;
        self.rings.retain(|r| t - r.born < 1.6);
        beat
    }

    /// Colors for LEDs at `points` on a surface `width` x `height` (key units, y down).
    pub fn render(&self, points: &[Point], width: f32, height: f32) -> Vec<Rgb> {
        points
            .iter()
            .map(|p| self.color(*p, width, height))
            .collect()
    }

    fn band_at(&self, f: f32) -> f32 {
        let x = f.clamp(0.0, 0.999) * (BANDS - 1) as f32;
        let i = x as usize;
        let frac = x - i as f32;
        self.levels[i] * (1.0 - frac) + self.levels[(i + 1).min(BANDS - 1)] * frac
    }

    fn color(&self, p: Point, width: f32, height: f32) -> Rgb {
        let xn = (p.x / width.max(0.01)).clamp(0.0, 1.0);
        let yn = (p.y / height.max(0.01)).clamp(0.0, 1.0);
        let up = 1.0 - yn; // 0 bottom row, 1 top row
        let bar = |level: f32| ((level - up) * 6.0 + 0.3).clamp(0.0, 1.0);
        let pal = self.colors.resolve(self.look);
        match self.look {
            AudioLook::Spectrum => {
                let l = self.band_at(xn);
                let t = if pal.by_height() { up } else { xn };
                pal.rgb(t, 0.04 + 0.96 * bar(l))
            }
            AudioLook::CenterOut => {
                let d = ((xn - 0.5).abs() * 2.0).min(1.0);
                let l = self.band_at(d);
                let t = if pal.by_height() { up } else { d };
                pal.rgb(t, 0.04 + 0.96 * bar(l))
            }
            AudioLook::Meter => {
                // Fills from the bottom row up, with a peak mark that falls back slowly
                let level = (self.loudness * 1.4).min(1.0);
                let lit = ((level - up) * 6.0 + 0.3).clamp(0.0, 1.0);
                let peak = (self.peak * 1.4).min(1.0);
                let peak_mark = (1.0 - (up - peak).abs() * 6.0).max(0.0);
                pal.rgb(up, (0.03 + 0.97 * lit).max(peak_mark))
            }
            AudioLook::BeatPulse => {
                let v = 0.12 + 0.88 * self.flash.max(self.loudness * 0.4);
                pal.rgb((self.hue + xn * 0.15).fract(), v)
            }
            AudioLook::BassRipples => {
                let (cx, cy) = (0.5 * width, 0.55 * height);
                let d = ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt();
                let mut c = [0.0f32; 3];
                for r in &self.rings {
                    let age = self.t - r.born;
                    let radius = age * 14.0;
                    let w = (1.0 - (d - radius).abs() / 1.6).max(0.0) * (1.0 - age / 1.6).max(0.0);
                    let rgb = pal.rgb(r.hue, 1.0);
                    for k in 0..3 {
                        c[k] = (c[k] + rgb[k] as f32 * w).min(255.0);
                    }
                }
                let base = pal.rgb(self.hue, 0.04 + 0.1 * self.loudness);
                [
                    (c[0] as u8).max(base[0]),
                    (c[1] as u8).max(base[1]),
                    (c[2] as u8).max(base[2]),
                ]
            }
            AudioLook::ColorWash => {
                let treble = self.levels[BANDS - 4..].iter().sum::<f32>() / 4.0;
                let t = (self.t * 0.04 + xn * 0.35 + yn * 0.1 + treble * 0.3).fract();
                // Palettes with an end (not the rainbow) go there and back instead of jumping
                let t = if matches!(pal, AudioColors::Rainbow) {
                    t
                } else {
                    1.0 - (t * 2.0 - 1.0).abs()
                };
                pal.rgb(t, 0.15 + 0.85 * (self.loudness * 1.3).min(1.0))
            }
            AudioLook::Strobe => {
                let since = self.t - self.last_beat;
                if since < 0.06 {
                    match pal {
                        AudioColors::One(..) => pal.rgb(0.0, 1.0),
                        _ => [255, 255, 255],
                    }
                } else {
                    pal.rgb(self.hue, 0.03 + 0.12 * self.loudness)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts() -> Vec<Point> {
        let mut v = vec![];
        for y in 0..6 {
            for x in 0..20 {
                v.push(Point {
                    x: x as f32 + 0.5,
                    y: y as f32 + 0.5,
                });
            }
        }
        v
    }

    #[test]
    fn silence_is_dark_and_loud_bass_beats() {
        let mut show = AudioShow::new(AudioLook::Spectrum);
        for _ in 0..30 {
            show.update(&[0.0; 9], 1.0 / 30.0);
        }
        let quiet = show.render(&pts(), 20.0, 6.0);
        assert!(quiet.iter().all(|c| c.iter().all(|&v| v < 20)));
        // a sudden bass hit is a beat, and lights the bass bars on the left
        let mut bands = [0.1f32; 9];
        bands[0] = 1.0;
        bands[1] = 1.0;
        assert!(show.update(&bands, 1.0 / 30.0));
        let loud = show.render(&pts(), 20.0, 6.0);
        let left_bottom = loud[5 * 20];
        let right_bottom = loud[5 * 20 + 19];
        let lum = |c: Rgb| c.iter().map(|&v| v as u32).sum::<u32>();
        assert!(lum(left_bottom) > lum(right_bottom) * 2);
    }

    #[test]
    fn spectrum_is_green_yellow_red_by_default_and_rainbow_on_request() {
        let mut show = AudioShow::new(AudioLook::Spectrum);
        for _ in 0..10 {
            show.update(&[1.0; 16], 0.03);
        }
        let c = show.render(&pts(), 20.0, 6.0);
        let bottom = c[5 * 20 + 10];
        let top = c[10];
        assert!(
            bottom[1] > 200 && bottom[0] < 60,
            "bottom row is green: {bottom:?}"
        );
        assert!(top[0] > 200 && top[1] < 60, "top row is red: {top:?}");
        show.colors = AudioColors::Rainbow;
        let c = show.render(&pts(), 20.0, 6.0);
        assert_ne!(c[5 * 20], c[5 * 20 + 19], "rainbow changes across");
        for k in ["auto", "rainbow", "meter", "fire", "ocean", "one 170 255"] {
            assert_eq!(AudioColors::from_key(k).unwrap().key(), k);
        }
    }

    #[test]
    fn meter_fills_from_the_bottom() {
        let mut show = AudioShow::new(AudioLook::Meter);
        for _ in 0..20 {
            show.update(&[0.25; 16], 0.03);
        }
        let c = show.render(&pts(), 20.0, 6.0);
        let lum = |c: Rgb| c.iter().map(|&v| v as u32).sum::<u32>();
        assert!(lum(c[5 * 20 + 3]) > lum(c[3]) * 3, "bottom lit, top dark");
        assert_eq!(c[5 * 20 + 3], c[5 * 20 + 15], "same across a row");
    }

    #[test]
    fn every_look_draws_every_led() {
        for look in AudioLook::ALL {
            let mut show = AudioShow::new(look);
            show.update(&[0.5; 16], 0.03);
            assert_eq!(show.render(&pts(), 20.0, 6.0).len(), 120);
            assert_eq!(AudioLook::from_key(look.key()), Some(look));
        }
    }
}
