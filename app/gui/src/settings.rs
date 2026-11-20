//! Remembers the user's setup, painted keys and effect choices between runs
//! (a small text file in %APPDATA%\framework-16-keyboard-control on Windows).

use crate::deck::{Choice, Dev, KbType, Look, Setup};
use f16_core::audio::AudioLook;
use f16_core::effects::LinkedEffect;
use f16_core::keycodes::APP_SLOTS;
use f16_core::keys::SideModule;
use f16_core::protocol::PaintMode;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Saved {
    pub setup: Setup,
    /// How painted keys look while an effect runs.
    pub paint_mode: PaintMode,
    /// Lighting page: one effect across both devices (joined), or each its own.
    pub join: bool,
    /// Lighting page, when not joined: edit the keyboard (false) or the side module (true).
    pub edit_module: bool,
    pub kb: Look,
    pub module: Look,
    pub brush: (u8, u8),
    pub paint: HashMap<(Dev, usize), (u8, u8)>,
    /// The window's look: "system", "light", "dark" or "classic".
    pub theme: String,
    /// Program each "Open an app" key starts (empty = not set).
    pub apps: Vec<String>,
    /// Closing the window keeps the app running in the notification area.
    pub tray: bool,
    /// Start with Windows. None = not chosen yet: on once an "Open an app" key is set up.
    pub autostart: Option<bool>,
    /// Keyboard type remembered per keyboard (USB serial number).
    pub kb_types: Vec<(String, KbType)>,
    /// Audio mode: which show, how sensitive (1..10), and the Ctrl+Alt+Shift+M shortcut.
    pub audio_look: AudioLook,
    pub audio_sensitivity: u8,
    pub audio_hotkey: bool,
    /// Look for a newer version when the app starts.
    pub auto_update: bool,
}

impl Saved {
    pub fn autostart_on(&self) -> bool {
        self.autostart
            .unwrap_or_else(|| self.apps.iter().any(|a| !a.is_empty()))
    }
}

impl Default for Saved {
    fn default() -> Self {
        let look = |choice, hue, sat, val, speed| Look {
            choice,
            hue,
            sat,
            val,
            speed,
        };
        Self {
            setup: Setup::default(),
            paint_mode: PaintMode::Follow,
            join: true,
            edit_module: false,
            kb: look(Choice::Builtin(13), 11, 220, 255, 128),
            module: look(Choice::Builtin(13), 128, 255, 230, 100),
            brush: (128, 255),
            paint: HashMap::new(),
            theme: "system".into(),
            apps: vec![String::new(); APP_SLOTS],
            tray: true,
            autostart: None,
            kb_types: vec![],
            audio_look: AudioLook::Spectrum,
            audio_sensitivity: 5,
            audio_hotkey: true,
            auto_update: true,
        }
    }
}

fn config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

fn path() -> Option<PathBuf> {
    Some(
        config_dir()?
            .join("framework-16-keyboard-control")
            .join("settings.txt"),
    )
}

/// Where the app kept its settings when it was called "Framework 16 Lights".
fn old_path() -> Option<PathBuf> {
    Some(config_dir()?.join("f16-lights").join("settings.txt"))
}

pub fn choice_str(c: Choice) -> String {
    match c {
        Choice::PaintOnly => "paint".into(),
        Choice::Builtin(id) => format!("b{id}"),
        Choice::Pc(LinkedEffect::Wave) => "pc-wave".into(),
        Choice::Pc(LinkedEffect::RainbowSweep) => "pc-rainbow".into(),
        Choice::Pc(LinkedEffect::Ripple) => "pc-ripple".into(),
    }
}

pub fn parse_choice(s: &str) -> Option<Choice> {
    Some(match s {
        "paint" => Choice::PaintOnly,
        "pc-wave" => Choice::Pc(LinkedEffect::Wave),
        "pc-rainbow" => Choice::Pc(LinkedEffect::RainbowSweep),
        "pc-ripple" => Choice::Pc(LinkedEffect::Ripple),
        _ => Choice::Builtin(
            s.strip_prefix('b')?
                .parse()
                .ok()
                .filter(|id| (1..=44).contains(id))?,
        ),
    })
}

fn look_str(l: &Look) -> String {
    format!(
        "{} {} {} {} {}",
        choice_str(l.choice),
        l.hue,
        l.sat,
        l.val,
        l.speed
    )
}

fn parse_look(s: &str) -> Option<Look> {
    let p: Vec<&str> = s.split_whitespace().collect();
    if p.len() != 5 {
        return None;
    }
    Some(Look {
        choice: parse_choice(p[0])?,
        hue: p[1].parse().ok()?,
        sat: p[2].parse().ok()?,
        val: p[3].parse().ok()?,
        speed: p[4].parse().ok()?,
    })
}

fn kb_str(t: KbType) -> &'static str {
    match t {
        KbType::ClearRgb => "clear",
        KbType::BlackRgb => "black-rgb",
        KbType::White => "white",
    }
}

fn parse_kb(v: &str) -> KbType {
    match v {
        "black-rgb" => KbType::BlackRgb,
        "white" => KbType::White,
        _ => KbType::ClearRgb,
    }
}

pub fn to_text(s: &Saved) -> String {
    let mut out = String::new();
    let kb = kb_str(s.setup.kb);
    let side = match s.setup.side {
        SideModule::None => "none",
        SideModule::Macropad => "macropad",
        SideModule::Numpad => "numpad",
    };
    let mode = match s.paint_mode {
        PaintMode::Steady => "steady",
        PaintMode::Follow => "follow",
        PaintMode::Tint => "tint",
    };
    out += &format!(
        "keyboard={kb}\nside={side}\nleft={}\npaint-mode={mode}\njoin={}\nedit-module={}\n",
        s.setup.module_left, s.join, s.edit_module
    );
    for (serial, t) in &s.kb_types {
        out += &format!("keyboard-of={} {serial}\n", kb_str(*t));
    }
    out += &format!(
        "kb={}\nmodule={}\nbrush={} {}\n",
        look_str(&s.kb),
        look_str(&s.module),
        s.brush.0,
        s.brush.1
    );
    out += &format!(
        "theme={}\ntray={}\nauto-update={}\naudio={} {} {}\n",
        s.theme,
        s.tray,
        s.auto_update,
        s.audio_look.key(),
        s.audio_sensitivity,
        s.audio_hotkey
    );
    if let Some(a) = s.autostart {
        out += &format!("autostart={a}\n");
    }
    for (i, a) in s.apps.iter().enumerate() {
        if !a.is_empty() {
            out += &format!("app={} {a}\n", i + 1);
        }
    }
    let mut keys: Vec<_> = s.paint.iter().collect();
    keys.sort();
    for ((dev, i), (h, sat)) in keys {
        out += &format!(
            "paint={} {i} {h} {sat}\n",
            if *dev == Dev::Keyboard {
                "kb"
            } else {
                "module"
            }
        );
    }
    out
}

pub fn from_text(text: &str) -> Saved {
    let mut s = Saved::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k {
            "keyboard" => {
                s.setup.kb = match v {
                    "black-rgb" => KbType::BlackRgb,
                    "white" => KbType::White,
                    _ => KbType::ClearRgb,
                }
            }
            "side" => {
                s.setup.side = match v {
                    "none" => SideModule::None,
                    "numpad" => SideModule::Numpad,
                    _ => SideModule::Macropad,
                }
            }
            "left" => s.setup.module_left = v == "true",
            // older versions: an on/off switch
            "follow" => {
                s.paint_mode = if v == "false" {
                    PaintMode::Steady
                } else {
                    PaintMode::Follow
                }
            }
            "paint-mode" => {
                s.paint_mode = match v {
                    "steady" => PaintMode::Steady,
                    "tint" => PaintMode::Tint,
                    _ => PaintMode::Follow,
                }
            }
            "join" => s.join = v == "true",
            "edit-module" => s.edit_module = v == "true",
            "keyboard-of" => {
                if let Some((t, serial)) = v.split_once(' ') {
                    s.kb_types.push((serial.to_string(), parse_kb(t)));
                }
            }
            "theme" if crate::theme::CHOICES.contains(&v) => s.theme = v.to_string(),
            "tray" => s.tray = v != "false",
            "auto-update" => s.auto_update = v != "false",
            "audio" => {
                let p: Vec<&str> = v.split_whitespace().collect();
                if let [look, sens, hotkey] = p[..] {
                    s.audio_look = AudioLook::from_key(look).unwrap_or_default();
                    s.audio_sensitivity = sens.parse::<u8>().unwrap_or(5).clamp(1, 10);
                    s.audio_hotkey = hotkey != "false";
                }
            }
            "autostart" => s.autostart = Some(v == "true"),
            "app" => {
                if let Some((n, path)) = v.split_once(' ') {
                    if let Some(slot) = n.parse::<usize>().ok().and_then(|n| n.checked_sub(1)) {
                        if slot < APP_SLOTS {
                            s.apps[slot] = path.trim().to_string();
                        }
                    }
                }
            }
            "kb" => s.kb = parse_look(v).unwrap_or(s.kb),
            "module" => s.module = parse_look(v).unwrap_or(s.module),
            "brush" => {
                let p: Vec<u8> = v
                    .split_whitespace()
                    .filter_map(|x| x.parse().ok())
                    .collect();
                if p.len() == 2 {
                    s.brush = (p[0], p[1]);
                }
            }
            "paint" => {
                let p: Vec<&str> = v.split_whitespace().collect();
                if p.len() == 4 {
                    let dev = if p[0] == "kb" {
                        Dev::Keyboard
                    } else {
                        Dev::Module
                    };
                    if let (Ok(i), Ok(h), Ok(sat)) = (p[1].parse(), p[2].parse(), p[3].parse()) {
                        s.paint.insert((dev, i), (h, sat));
                    }
                }
            }
            _ => {}
        }
    }
    s
}

pub fn load() -> Saved {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .or_else(|| old_path().and_then(|p| std::fs::read_to_string(p).ok()))
        .map(|t| from_text(&t))
        .unwrap_or_default()
}

pub fn save(s: &Saved) {
    if let Some(p) = path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, to_text(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let mut s = Saved {
            setup: Setup {
                kb: KbType::BlackRgb,
                side: SideModule::Numpad,
                module_left: true,
            },
            ..Default::default()
        };
        s.join = false;
        s.edit_module = true;
        s.paint_mode = PaintMode::Tint;
        s.kb.choice = Choice::Pc(LinkedEffect::Ripple);
        s.module.choice = Choice::PaintOnly;
        s.kb_types.push(("FRAKDEBZ0812".into(), KbType::BlackRgb));
        s.audio_look = AudioLook::BassRipples;
        s.audio_sensitivity = 8;
        s.audio_hotkey = false;
        s.paint.insert((Dev::Keyboard, 30), (128, 255));
        s.paint.insert((Dev::Module, 3), (0, 0));
        s.apps[2] = r"C:\Program Files\Resolume Arena\Arena.exe".into();
        s.tray = false;
        s.autostart = Some(false);
        assert_eq!(from_text(&to_text(&s)), s);
    }

    #[test]
    fn junk_falls_back_to_defaults() {
        let s = from_text("kb=b99 1 2 3 4\nside=?\nnonsense\nfollow=false\n");
        assert_eq!(s.kb, Saved::default().kb);
        assert_eq!(s.paint_mode, PaintMode::Steady);
        assert_eq!(s.setup.side, SideModule::Macropad);
    }
}
