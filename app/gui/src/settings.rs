//! Remembers the user's setup, painted keys and effect choices between runs
//! (a small text file in %APPDATA%\f16-lights on Windows).

use crate::deck::{Choice, Dev, KbType, Look, Setup};
use f16_core::effects::LinkedEffect;
use f16_core::keys::SideModule;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Saved {
    pub setup: Setup,
    pub follow: bool,
    pub join: bool,
    pub show: Look,
    pub kb: Look,
    pub module: Look,
    pub brush: (u8, u8),
    pub paint: HashMap<(Dev, usize), (u8, u8)>,
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
            follow: true,
            join: false,
            show: look(Choice::Builtin(13), 11, 220, 255, 128),
            kb: look(Choice::Builtin(27), 11, 220, 255, 128),
            module: look(Choice::Builtin(13), 128, 255, 230, 100),
            brush: (128, 255),
            paint: HashMap::new(),
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("f16-lights").join("settings.txt"))
}

fn choice_str(c: Choice) -> String {
    match c {
        Choice::PaintOnly => "paint".into(),
        Choice::Builtin(id) => format!("b{id}"),
        Choice::Pc(LinkedEffect::Wave) => "pc-wave".into(),
        Choice::Pc(LinkedEffect::RainbowSweep) => "pc-rainbow".into(),
        Choice::Pc(LinkedEffect::Ripple) => "pc-ripple".into(),
    }
}

fn parse_choice(s: &str) -> Option<Choice> {
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

pub fn to_text(s: &Saved) -> String {
    let mut out = String::new();
    let kb = match s.setup.kb {
        KbType::ClearRgb => "clear",
        KbType::BlackRgb => "black-rgb",
        KbType::White => "white",
    };
    let side = match s.setup.side {
        SideModule::None => "none",
        SideModule::Macropad => "macropad",
        SideModule::Numpad => "numpad",
    };
    out += &format!(
        "keyboard={kb}\nside={side}\nleft={}\nfollow={}\njoin={}\n",
        s.setup.module_left, s.follow, s.join
    );
    out += &format!(
        "show={}\nkb={}\nmodule={}\nbrush={} {}\n",
        look_str(&s.show),
        look_str(&s.kb),
        look_str(&s.module),
        s.brush.0,
        s.brush.1
    );
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
            "follow" => s.follow = v != "false",
            "join" => s.join = v == "true",
            "show" => s.show = parse_look(v).unwrap_or(s.show),
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
        s.join = true;
        s.follow = false;
        s.show.choice = Choice::Pc(LinkedEffect::Ripple);
        s.kb.choice = Choice::PaintOnly;
        s.paint.insert((Dev::Keyboard, 30), (128, 255));
        s.paint.insert((Dev::Module, 3), (0, 0));
        assert_eq!(from_text(&to_text(&s)), s);
    }

    #[test]
    fn junk_falls_back_to_defaults() {
        let s = from_text("show=b99 1 2 3 4\nside=?\nnonsense\n");
        assert_eq!(s.show, Saved::default().show);
        assert_eq!(s.setup.side, SideModule::Macropad);
    }
}
