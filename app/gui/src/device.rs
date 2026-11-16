//! HID access to the keyboard and macropad (hidapi).

use f16_core::protocol::{
    self as proto, DeviceInfo, DeviceKind, LedPoint, ProtocolError, Report, REPORT_LEN,
};
use hidapi::{HidApi, HidDevice};
use std::fmt;

const READ_TIMEOUT_MS: i32 = 250;

#[derive(Debug)]
pub enum DeviceError {
    Hid(hidapi::HidError),
    Protocol(ProtocolError),
    Timeout,
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hid(e) => write!(f, "USB error: {e}"),
            Self::Protocol(e) => write!(f, "{e}"),
            Self::Timeout => write!(f, "device did not answer"),
        }
    }
}

impl From<hidapi::HidError> for DeviceError {
    fn from(e: hidapi::HidError) -> Self {
        Self::Hid(e)
    }
}
impl From<ProtocolError> for DeviceError {
    fn from(e: ProtocolError) -> Self {
        Self::Protocol(e)
    }
}

pub type Result<T> = std::result::Result<T, DeviceError>;

/// A connected keyboard or macropad (VIA raw HID interface).
pub struct Device {
    pub kind: DeviceKind,
    hid: HidDevice,
}

/// What `scan` found for one device type.
pub enum Found {
    /// Running our firmware.
    Ready(Device, DeviceInfo, Vec<LedPoint>),
    /// Present but not running f16custom (stock firmware or other).
    Stock(DeviceKind),
}

impl Device {
    pub fn send(&self, req: &Report) -> Result<()> {
        // hidapi wants the report id (0 = none) in front
        let mut buf = [0u8; REPORT_LEN + 1];
        buf[1..].copy_from_slice(req);
        self.hid.write(&buf)?;
        Ok(())
    }

    fn recv_matching(&self, req: &Report) -> Result<Report> {
        // Skip stale answers (e.g. left over after a timeout) until ours shows up.
        for _ in 0..8 {
            let mut resp = [0u8; REPORT_LEN];
            let n = self.hid.read_timeout(&mut resp, READ_TIMEOUT_MS)?;
            if n == 0 {
                return Err(DeviceError::Timeout);
            }
            match proto::check_response(req, &resp) {
                Err(ProtocolError::Mismatch { .. }) => continue,
                Err(e) => return Err(e.into()),
                Ok(_) => return Ok(resp),
            }
        }
        Err(DeviceError::Timeout)
    }

    /// Sends one request and returns the 29 byte answer payload.
    pub fn transact(&self, req: &Report) -> Result<[u8; REPORT_LEN - 3]> {
        self.send(req)?;
        let resp = self.recv_matching(req)?;
        let mut payload = [0u8; REPORT_LEN - 3];
        payload.copy_from_slice(&resp[3..]);
        Ok(payload)
    }

    /// Sends several requests back to back, then collects the answers.
    /// Much faster than one round trip per message for streaming frames.
    pub fn transact_many(&self, reqs: &[Report]) -> Result<()> {
        for r in reqs {
            self.send(r)?;
        }
        for r in reqs {
            self.recv_matching(r)?;
        }
        Ok(())
    }

    pub fn info(&self) -> Result<DeviceInfo> {
        let p = self.transact(&proto::info_request())?;
        Ok(proto::parse_info(&p)?)
    }

    pub fn led_points(&self, count: u8) -> Result<Vec<LedPoint>> {
        let mut out = Vec::with_capacity(count as usize);
        for req in proto::led_info_requests(count) {
            out.extend(proto::parse_led_info(&self.transact(&req)?));
        }
        out.truncate(count as usize);
        Ok(out)
    }
}

/// Opens the raw HID interface of one device type without talking to it.
pub fn open_raw(api: &mut HidApi, want: DeviceKind) -> Option<Device> {
    let _ = api.refresh_devices();
    api.device_list()
        .filter(|d| {
            d.vendor_id() == proto::VID
                && d.usage_page() == proto::RAW_USAGE_PAGE
                && d.usage() == proto::RAW_USAGE
                && DeviceKind::from_pid(d.product_id()) == Some(want)
        })
        .find_map(|d| d.open_device(api).ok())
        .map(|hid| Device { kind: want, hid })
}

/// Finds the keyboard and macropad raw HID interfaces, skipping kinds in `skip`
/// (devices that are already open).
pub fn scan(api: &mut HidApi, skip: &[DeviceKind]) -> Vec<Found> {
    if let Err(e) = api.refresh_devices() {
        eprintln!("HID refresh failed: {e}");
    }
    let mut found = Vec::new();
    let mut seen = Vec::new();
    for d in api.device_list() {
        if d.vendor_id() != proto::VID
            || d.usage_page() != proto::RAW_USAGE_PAGE
            || d.usage() != proto::RAW_USAGE
        {
            continue;
        }
        let Some(kind) = DeviceKind::from_pid(d.product_id()) else {
            continue;
        };
        if seen.contains(&kind) || skip.contains(&kind) {
            continue; // only one of each
        }
        let Ok(hid) = d.open_device(api) else {
            continue;
        };
        seen.push(kind);
        let dev = Device { kind, hid };
        match dev.info() {
            Ok(info) => match dev.led_points(info.led_count) {
                Ok(leds) => found.push(Found::Ready(dev, info, leds)),
                Err(_) => found.push(Found::Stock(kind)),
            },
            Err(_) => found.push(Found::Stock(kind)),
        }
    }
    found
}
