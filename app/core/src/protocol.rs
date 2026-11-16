//! Raw HID message encoding for the `f16custom` firmware.
//!
//! Must stay in sync with `firmware/users/f16custom/hostfx_protocol.h`.
//! Every message is a 32 byte report on the VIA raw HID interface
//! (usage page 0xFF60, usage 0x61):
//!
//! ```text
//! byte 0   0x07 = set, 0x08 = get   (VIA id_custom_set_value / get_value)
//! byte 1   0x00                     (VIA id_custom_channel)
//! byte 2   value id                 (ValueId)
//! byte 3.. payload
//! ```
//!
//! The device echoes bytes 0..=2 in its answer, or sets byte 0 to 0xFF when it
//! does not understand the command (for example on stock firmware).

use std::fmt;

/// Framework Computer USB vendor id.
pub const VID: u16 = 0x32AC;
/// Framework Laptop 16 ANSI keyboard (the RGB and white versions share this id).
pub const PID_KEYBOARD: u16 = 0x0012;
/// Framework Laptop 16 RGB macropad.
pub const PID_MACROPAD: u16 = 0x0013;
/// VIA raw HID interface.
pub const RAW_USAGE_PAGE: u16 = 0xFF60;
pub const RAW_USAGE: u16 = 0x61;

pub const REPORT_LEN: usize = 32;
pub type Report = [u8; REPORT_LEN];

pub const CMD_SET: u8 = 0x07;
pub const CMD_GET: u8 = 0x08;
pub const CHANNEL: u8 = 0x00;
pub const UNHANDLED: u8 = 0xFF;

pub const MAGIC: [u8; 2] = *b"FX";
pub const PROTOCOL_VERSION: u8 = 1;

/// Max LEDs per FRAME message. Host->device messages stay within 31 bytes
/// (3 + 2 + 8 * 3 = 29) because the RP2040 sometimes only receives 31 of 32.
pub const FRAME_LEDS_PER_MSG: usize = 8;
/// Max LEDs per LED_INFO answer (device->host, 3 + 2 + 9 * 3 = 32).
pub const INFO_LEDS_PER_MSG: usize = 9;
/// Max key presses returned per KEY_EVENTS poll.
pub const KEYS_PER_MSG: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueId {
    Info = 0x01,
    State = 0x02,
    HotkeyLock = 0x03,
    LedInfo = 0x04,
    Frame = 0x05,
    HostMode = 0x06,
    KeyEvents = 0x07,
    Layer = 0x08,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    Keyboard,
    Macropad,
}

impl DeviceKind {
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            PID_KEYBOARD => Some(Self::Keyboard),
            PID_MACROPAD => Some(Self::Macropad),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Keyboard => "Keyboard",
            Self::Macropad => "Macropad",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// Device answered 0xFF: the command is unknown (most likely stock firmware).
    Unsupported,
    /// Answer does not belong to the request we sent.
    Mismatch { expected: [u8; 3], got: [u8; 3] },
    /// INFO answer has no "FX" magic: not our firmware.
    NotCustomFirmware,
    /// Firmware speaks a newer/older protocol.
    VersionMismatch(u8),
    /// Bad argument (e.g. invalid effect id).
    Invalid(&'static str),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => write!(
                f,
                "command not supported (is the f16custom firmware flashed?)"
            ),
            Self::Mismatch { expected, got } => {
                write!(f, "unexpected answer {got:02X?}, expected {expected:02X?}")
            }
            Self::NotCustomFirmware => write!(f, "device is not running the f16custom firmware"),
            Self::VersionMismatch(v) => {
                write!(f, "firmware protocol v{v}, app expects v{PROTOCOL_VERSION}")
            }
            Self::Invalid(why) => write!(f, "invalid value: {why}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

fn request(cmd: u8, id: ValueId, payload: &[u8]) -> Report {
    assert!(payload.len() <= REPORT_LEN - 3, "payload too long");
    let mut r = [0u8; REPORT_LEN];
    r[0] = cmd;
    r[1] = CHANNEL;
    r[2] = id as u8;
    r[3..3 + payload.len()].copy_from_slice(payload);
    r
}

pub fn get(id: ValueId, payload: &[u8]) -> Report {
    request(CMD_GET, id, payload)
}

pub fn set(id: ValueId, payload: &[u8]) -> Report {
    request(CMD_SET, id, payload)
}

/// Checks that `resp` answers `req` and returns its 29 byte payload.
pub fn check_response<'a>(req: &Report, resp: &'a Report) -> Result<&'a [u8], ProtocolError> {
    if resp[0] == UNHANDLED {
        return Err(ProtocolError::Unsupported);
    }
    if resp[..3] != req[..3] {
        return Err(ProtocolError::Mismatch {
            expected: [req[0], req[1], req[2]],
            got: [resp[0], resp[1], resp[2]],
        });
    }
    Ok(&resp[3..])
}

// ---------------------------------------------------------------------------
// INFO
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub protocol_version: u8,
    pub led_count: u8,
    /// RGB_MATRIX_EFFECT_MAX: valid effect ids are 1..effect_count
    pub effect_count: u8,
    /// Effect id of the host-driven ("PC controlled") effect.
    pub host_effect: u8,
    pub kind: DeviceKind,
    pub hotkey_lock: bool,
    /// Macropad layer (0 = MIDI, 1 = F-keys); None on the keyboard.
    pub layer: Option<u8>,
    pub max_brightness: u8,
}

pub fn info_request() -> Report {
    get(ValueId::Info, &[])
}

pub fn parse_info(payload: &[u8]) -> Result<DeviceInfo, ProtocolError> {
    if payload[0..2] != MAGIC {
        return Err(ProtocolError::NotCustomFirmware);
    }
    if payload[2] != PROTOCOL_VERSION {
        return Err(ProtocolError::VersionMismatch(payload[2]));
    }
    let kind = match payload[6] {
        0 => DeviceKind::Keyboard,
        1 => DeviceKind::Macropad,
        _ => return Err(ProtocolError::Invalid("device kind")),
    };
    Ok(DeviceInfo {
        protocol_version: payload[2],
        led_count: payload[3],
        effect_count: payload[4],
        host_effect: payload[5],
        kind,
        hotkey_lock: payload[7] != 0,
        layer: if payload[8] == 0xFF {
            None
        } else {
            Some(payload[8])
        },
        max_brightness: payload[9],
    })
}

// ---------------------------------------------------------------------------
// STATE
// ---------------------------------------------------------------------------

/// Built-in effect state. Hue/sat/val/speed use QMK's 0..=255 scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightState {
    pub enabled: bool,
    pub mode: u8,
    pub hue: u8,
    pub sat: u8,
    pub val: u8,
    pub speed: u8,
}

impl Default for LightState {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: 1,
            hue: 0,
            sat: 0,
            val: 255,
            speed: 127,
        }
    }
}

pub fn state_request() -> Report {
    get(ValueId::State, &[])
}

/// Returns the state plus whether the PC is currently streaming frames.
pub fn parse_state(payload: &[u8]) -> (LightState, bool) {
    (
        LightState {
            enabled: payload[0] != 0,
            mode: payload[1],
            hue: payload[2],
            sat: payload[3],
            val: payload[4],
            speed: payload[5],
        },
        payload[6] != 0,
    )
}

/// `save = true` also writes it to the device's EEPROM (survives unplugging).
pub fn set_state(
    state: &LightState,
    save: bool,
    effect_count: u8,
) -> Result<Report, ProtocolError> {
    if state.mode == 0 || state.mode >= effect_count {
        return Err(ProtocolError::Invalid("effect id"));
    }
    Ok(set(
        ValueId::State,
        &[
            state.enabled as u8,
            state.mode,
            state.hue,
            state.sat,
            state.val,
            state.speed,
            save as u8,
        ],
    ))
}

// ---------------------------------------------------------------------------
// Hotkey lock / host mode / layer
// ---------------------------------------------------------------------------

pub fn set_hotkey_lock(on: bool) -> Report {
    set(ValueId::HotkeyLock, &[on as u8])
}

pub fn hotkey_lock_request() -> Report {
    get(ValueId::HotkeyLock, &[])
}

pub fn set_host_mode(on: bool) -> Report {
    set(ValueId::HostMode, &[on as u8])
}

pub fn set_layer(layer: u8) -> Report {
    set(ValueId::Layer, &[layer])
}

pub fn layer_request() -> Report {
    get(ValueId::Layer, &[])
}

/// Restarts the device into its RP2040 bootloader ("RPI-RP2" USB drive) so a
/// new UF2 can be dropped on it. This is Framework's own factory command
/// (VIA command 0x0B, sub command 0xFE), so it also works on stock firmware.
/// The device resets immediately and sends no answer.
pub fn bootloader_request() -> Report {
    let mut r = [0u8; REPORT_LEN];
    r[0] = 0x0B;
    r[1] = 0xFE;
    r
}

// ---------------------------------------------------------------------------
// LED positions
// ---------------------------------------------------------------------------

/// LED position in QMK's coordinate space (x 0..=224, y 0..=64) + LED flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedPoint {
    pub x: u8,
    pub y: u8,
    pub flags: u8,
}

pub fn led_info_request(start: u8) -> Report {
    get(ValueId::LedInfo, &[start])
}

/// All LED_INFO requests needed to read `led_count` LEDs.
pub fn led_info_requests(led_count: u8) -> Vec<Report> {
    (0..led_count as usize)
        .step_by(INFO_LEDS_PER_MSG)
        .map(|start| led_info_request(start as u8))
        .collect()
}

pub fn parse_led_info(payload: &[u8]) -> Vec<LedPoint> {
    let count = (payload[1] as usize).min(INFO_LEDS_PER_MSG);
    (0..count)
        .map(|i| {
            let b = &payload[2 + i * 3..2 + i * 3 + 3];
            LedPoint {
                x: b[0],
                y: b[1],
                flags: b[2],
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Frames and key events
// ---------------------------------------------------------------------------

/// Splits one frame (one colour per LED, in LED index order) into FRAME messages.
pub fn encode_frame(colors: &[[u8; 3]]) -> Vec<Report> {
    colors
        .chunks(FRAME_LEDS_PER_MSG)
        .enumerate()
        .map(|(i, chunk)| {
            let mut payload = Vec::with_capacity(2 + chunk.len() * 3);
            payload.push((i * FRAME_LEDS_PER_MSG) as u8);
            payload.push(chunk.len() as u8);
            for c in chunk {
                payload.extend_from_slice(c);
            }
            set(ValueId::Frame, &payload)
        })
        .collect()
}

pub fn key_events_request() -> Report {
    get(ValueId::KeyEvents, &[])
}

/// LED indices of keys pressed since the last poll.
pub fn parse_key_events(payload: &[u8]) -> Vec<u8> {
    let n = (payload[0] as usize).min(KEYS_PER_MSG);
    payload[1..1 + n].to_vec()
}

// ---------------------------------------------------------------------------
// Effect names
// ---------------------------------------------------------------------------

/// Names of QMK's built-in effects in the order Framework's firmware enables
/// them (keyboards/framework/config.h, Framework QMK v0.3.1). Index = effect id.
pub const EFFECT_NAMES: &[&str] = &[
    "Off",
    "Solid color",
    "Alphas / mods",
    "Gradient up-down",
    "Gradient left-right",
    "Breathing",
    "Band (saturation)",
    "Band (brightness)",
    "Pinwheel band (saturation)",
    "Pinwheel band (brightness)",
    "Spiral band (saturation)",
    "Spiral band (brightness)",
    "Cycle all",
    "Cycle left-right",
    "Cycle up-down",
    "Rainbow chevron",
    "Cycle out-in",
    "Cycle out-in dual",
    "Cycle pinwheel",
    "Cycle spiral",
    "Dual beacon",
    "Rainbow beacon",
    "Rainbow pinwheels",
    "Raindrops",
    "Jellybean raindrops",
    "Hue breathing",
    "Hue pendulum",
    "Hue wave",
    "Pixel rain",
    "Pixel flow",
    "Pixel fractal",
    "Typing heatmap",
    "Digital rain",
    "Reactive (simple)",
    "Reactive",
    "Reactive wide",
    "Reactive multi-wide",
    "Reactive cross",
    "Reactive multi-cross",
    "Reactive nexus",
    "Reactive multi-nexus",
    "Splash",
    "Multi-splash",
    "Solid splash",
    "Solid multi-splash",
];

pub fn effect_name(mode: u8, host_effect: u8) -> String {
    if mode == host_effect {
        return "PC controlled (Linked)".to_string();
    }
    EFFECT_NAMES
        .get(mode as usize)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Effect {mode}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_have_via_custom_header() {
        let r = get(ValueId::Info, &[]);
        assert_eq!(&r[..3], &[0x08, 0x00, 0x01]);
        let r = set_hotkey_lock(true);
        assert_eq!(&r[..4], &[0x07, 0x00, 0x03, 0x01]);
        assert_eq!(r.len(), 32);
    }

    #[test]
    fn bootloader_is_framework_factory_command() {
        let r = bootloader_request();
        assert_eq!(&r[..2], &[0x0B, 0xFE]);
        assert!(r[2..].iter().all(|&b| b == 0));
    }

    #[test]
    fn response_checks() {
        let req = info_request();
        let mut ok = req;
        ok[3] = b'F';
        assert_eq!(check_response(&req, &ok).unwrap()[0], b'F');

        let mut unhandled = req;
        unhandled[0] = 0xFF;
        assert_eq!(
            check_response(&req, &unhandled),
            Err(ProtocolError::Unsupported)
        );

        let other = state_request();
        assert!(matches!(
            check_response(&req, &other),
            Err(ProtocolError::Mismatch { .. })
        ));
    }

    #[test]
    fn parses_info() {
        let mut p = [0u8; 29];
        p[..10].copy_from_slice(&[b'F', b'X', 1, 97, 46, 45, 1, 1, 0, 255]);
        let info = parse_info(&p).unwrap();
        assert_eq!(info.led_count, 97);
        assert_eq!(info.effect_count, 46);
        assert_eq!(info.host_effect, 45);
        assert_eq!(info.kind, DeviceKind::Macropad);
        assert!(info.hotkey_lock);
        assert_eq!(info.layer, Some(0));

        p[6] = 0;
        p[8] = 0xFF;
        let info = parse_info(&p).unwrap();
        assert_eq!(info.kind, DeviceKind::Keyboard);
        assert_eq!(info.layer, None);

        p[0] = 0;
        assert_eq!(parse_info(&p), Err(ProtocolError::NotCustomFirmware));
        p[0] = b'F';
        p[2] = 9;
        assert_eq!(parse_info(&p), Err(ProtocolError::VersionMismatch(9)));
    }

    #[test]
    fn state_round_trip() {
        let s = LightState {
            enabled: true,
            mode: 12,
            hue: 10,
            sat: 20,
            val: 30,
            speed: 40,
        };
        let r = set_state(&s, true, 46).unwrap();
        assert_eq!(&r[..10], &[0x07, 0, 0x02, 1, 12, 10, 20, 30, 40, 1]);
        // The device answers a GET with the same field layout (+ streaming flag)
        let mut payload = [0u8; 29];
        payload[..7].copy_from_slice(&r[3..10]);
        let (parsed, streaming) = parse_state(&payload);
        assert_eq!(parsed, s);
        assert!(streaming); // the save byte sits where "streaming" is reported
    }

    #[test]
    fn set_state_rejects_bad_effect() {
        let mut s = LightState {
            mode: 0,
            ..Default::default()
        };
        assert!(set_state(&s, false, 46).is_err());
        s.mode = 46;
        assert!(set_state(&s, false, 46).is_err());
        s.mode = 45;
        assert!(set_state(&s, false, 46).is_ok());
    }

    #[test]
    fn frame_is_chunked_in_eights() {
        let colors: Vec<[u8; 3]> = (0..97u8)
            .map(|i| [i, i.wrapping_add(1), i.wrapping_add(2)])
            .collect();
        let msgs = encode_frame(&colors);
        assert_eq!(msgs.len(), 13); // ceil(97 / 8)
        assert_eq!(&msgs[0][..5], &[0x07, 0, 0x05, 0, 8]);
        assert_eq!(&msgs[0][5..8], &[0, 1, 2]);
        assert_eq!(&msgs[1][3..5], &[8, 8]);
        assert_eq!(&msgs[1][5..8], &[8, 9, 10]);
        let last = msgs.last().unwrap();
        assert_eq!(&last[3..5], &[96, 1]);
        assert_eq!(&last[5..8], &[96, 97, 98]);
        // the last byte is never used (the RP2040 may not receive it)
        assert!(msgs
            .iter()
            .all(|m| m.len() == REPORT_LEN && m[29..].iter().all(|&b| b == 0)));
        // the macropad's 24 LEDs fit in 3 messages
        assert_eq!(encode_frame(&[[0; 3]; 24]).len(), 3);
    }

    #[test]
    fn led_info_requests_cover_all_leds() {
        let reqs = led_info_requests(97);
        assert_eq!(reqs.len(), 11);
        assert_eq!(reqs[10][3], 90);
        let mut payload = [0u8; 29];
        payload[0] = 90;
        payload[1] = 2;
        payload[2..8].copy_from_slice(&[10, 20, 4, 30, 40, 1]);
        assert_eq!(
            parse_led_info(&payload),
            vec![
                LedPoint {
                    x: 10,
                    y: 20,
                    flags: 4
                },
                LedPoint {
                    x: 30,
                    y: 40,
                    flags: 1
                }
            ]
        );
    }

    #[test]
    fn key_events_are_clamped() {
        let mut p = [0u8; 29];
        p[0] = 3;
        p[1..4].copy_from_slice(&[5, 6, 7]);
        assert_eq!(parse_key_events(&p), vec![5, 6, 7]);
        p[0] = 200; // garbage count must not panic
        assert_eq!(parse_key_events(&p).len(), KEYS_PER_MSG);
    }

    #[test]
    fn effect_names_match_firmware_numbering() {
        // firmware: RGB_MATRIX_TYPING_HEATMAP == 31, SOLID_MULTISPLASH == 44, host effect == 45
        assert_eq!(EFFECT_NAMES[31], "Typing heatmap");
        assert_eq!(EFFECT_NAMES.len(), 45);
        assert_eq!(effect_name(45, 45), "PC controlled (Linked)");
        assert_eq!(effect_name(60, 45), "Effect 60");
    }
}
