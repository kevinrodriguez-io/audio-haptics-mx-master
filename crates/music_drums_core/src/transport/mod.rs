//! HID transports for Bolt receiver and Bluetooth MX Master 4.

use crate::hidpp::{
    self, haptic_set_config_long_report, haptic_set_config_report, haptic_trigger_long_report,
    haptic_trigger_report, long_report, HapticPulse, HidppError, FEATURE_HAPTIC, LOGI_VID,
    LONG_REPORT_LEN, MX_MASTER_4_PID, SHORT_REPORT_LEN,
};
use hidapi::{DeviceInfo, HidApi, HidDevice};
use serde::Serialize;
use std::ffi::CString;
use std::time::Duration;

/// Classic HID++ vendor page (Bolt / Unifying receivers).
const HIDPP_USAGE_PAGE: u16 = 0xFF00;
/// macOS Bluetooth Logitech HID++ vendor page.
const HIDPP_BT_USAGE_PAGE: u16 = 0xFF43;
const HIDPP_BT_USAGE: u16 = 0x0202;
const SHORT_USAGE: u16 = 0x0001;
const LONG_USAGE: u16 = 0x0002;

/// Bolt paired-device index used by MasterMice for MX4 haptics.
const BOLT_DEVICE_INDEX: u8 = 0x02;
const HAPTIC_INDEX_FALLBACK: u8 = 0x0B;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Bolt,
    Bluetooth,
    None,
}

pub trait HapticTransport: Send {
    fn link_kind(&self) -> LinkKind;
    fn set_haptic(&mut self, enabled: bool, intensity: u8) -> Result<(), HidppError>;
    fn trigger(&mut self, pulse: HapticPulse) -> Result<(), HidppError>;
    fn ping(&mut self) -> Result<(), HidppError>;
}

enum ShortPipe {
    /// Dedicated short collection (Bolt FF00 / usage 1) — report id 0x10.
    Dedicated(HidDevice),
    /// Bluetooth FF43/0202 — typically only accepts long report id 0x11.
    SharedLongOnly,
}

struct Mx4Session {
    short: ShortPipe,
    long: HidDevice,
    device_index: u8,
    haptic_feature_index: u8,
    link: LinkKind,
}

impl Mx4Session {
    fn send_bytes(&self, dedicated: Option<&HidDevice>, data: &[u8]) -> Result<(), HidppError> {
        let dev = dedicated.unwrap_or(&self.long);
        match dev.send_output_report(data) {
            Ok(()) => Ok(()),
            Err(set_err) => dev.write(data).map(|_| ()).map_err(|write_err| {
                HidppError::Io(format!(
                    "HID report failed len={} (set_report: {set_err}; write: {write_err})",
                    data.len()
                ))
            }),
        }
    }

    fn send_short(&self, report: &[u8; SHORT_REPORT_LEN]) -> Result<(), HidppError> {
        match &self.short {
            ShortPipe::Dedicated(d) => self.send_bytes(Some(d), report),
            ShortPipe::SharedLongOnly => Err(HidppError::Io(
                "short reports not used on Bluetooth FF43; use long path".into(),
            )),
        }
    }

    fn send_long(&self, report: &[u8; LONG_REPORT_LEN]) -> Result<(), HidppError> {
        self.send_bytes(None, report)
    }

    fn read_response(&self, timeout_ms: i32) -> Result<Vec<u8>, HidppError> {
        let mut buf = [0u8; 64];
        let n = self
            .long
            .read_timeout(&mut buf, timeout_ms)
            .map_err(|e| HidppError::Io(e.to_string()))?;
        Ok(buf[..n].to_vec())
    }

    fn discover_haptic_index(long: &HidDevice, device_index: u8) -> u8 {
        let req = long_report(
            device_index,
            0x00,
            0x00,
            0x01,
            &[(FEATURE_HAPTIC >> 8) as u8, (FEATURE_HAPTIC & 0xFF) as u8],
        );
        let _ = long
            .send_output_report(&req)
            .or_else(|_| long.write(&req).map(|_| ()));

        let mut buf = [0u8; 64];
        for _ in 0..12 {
            let Ok(n) = long.read_timeout(&mut buf, 250) else {
                continue;
            };
            if n == 0 {
                continue;
            }
            if let Some((idx, feat_type, _)) = hidpp::parse_get_feature_response(&buf[..n]) {
                if idx == 0 && feat_type == 0 {
                    continue;
                }
                if idx > 0 {
                    return idx;
                }
            }
        }
        tracing::warn!(
            "feature discovery timed out; using haptic index 0x{HAPTIC_INDEX_FALLBACK:02X}"
        );
        HAPTIC_INDEX_FALLBACK
    }
}

impl HapticTransport for Mx4Session {
    fn link_kind(&self) -> LinkKind {
        self.link
    }

    fn set_haptic(&mut self, enabled: bool, intensity: u8) -> Result<(), HidppError> {
        match self.short {
            ShortPipe::Dedicated(_) => {
                let report = haptic_set_config_report(
                    self.device_index,
                    self.haptic_feature_index,
                    enabled,
                    intensity,
                );
                self.send_short(&report)
            }
            ShortPipe::SharedLongOnly => {
                let report = haptic_set_config_long_report(
                    self.device_index,
                    self.haptic_feature_index,
                    enabled,
                    intensity,
                );
                self.send_long(&report)
            }
        }
    }

    fn trigger(&mut self, pulse: HapticPulse) -> Result<(), HidppError> {
        match self.short {
            ShortPipe::Dedicated(_) => {
                let report = haptic_trigger_report(
                    self.device_index,
                    self.haptic_feature_index,
                    pulse,
                );
                self.send_short(&report)
            }
            ShortPipe::SharedLongOnly => {
                let report = haptic_trigger_long_report(
                    self.device_index,
                    self.haptic_feature_index,
                    pulse,
                );
                self.send_long(&report)
            }
        }
    }

    fn ping(&mut self) -> Result<(), HidppError> {
        let req = long_report(self.device_index, 0x00, 0x01, 0x01, &[0x00, 0x00, 0xAA]);
        self.send_long(&req)?;
        let _ = self.read_response(150);
        Ok(())
    }
}

fn is_bolt_receiver(info: &DeviceInfo) -> bool {
    let pid = info.product_id();
    if matches!(pid, 0xC548 | 0xC547 | 0xC549 | 0xC54A | 0xC54B | 0xC54D | 0xC552) {
        return true;
    }
    let name = info.product_string().unwrap_or("").to_ascii_lowercase();
    name.contains("bolt")
}

fn looks_like_mx_master_4(info: &DeviceInfo) -> bool {
    if info.vendor_id() != LOGI_VID {
        return false;
    }
    if info.product_id() == MX_MASTER_4_PID {
        return true;
    }
    let name = info.product_string().unwrap_or("").to_ascii_lowercase();
    name.contains("mx master 4") || name.contains("mx master4")
}

fn is_hidpp_short(info: &DeviceInfo) -> bool {
    info.usage_page() == HIDPP_USAGE_PAGE && info.usage() == SHORT_USAGE
}

fn is_hidpp_long(info: &DeviceInfo) -> bool {
    info.usage_page() == HIDPP_USAGE_PAGE && info.usage() == LONG_USAGE
}

fn is_hidpp_bluetooth(info: &DeviceInfo) -> bool {
    info.usage_page() == HIDPP_BT_USAGE_PAGE && info.usage() == HIDPP_BT_USAGE
}

fn open_path(api: &HidApi, path: &std::ffi::CStr) -> Result<HidDevice, HidppError> {
    api.open_path(path)
        .map_err(|e| HidppError::Io(e.to_string()))
}

fn path_owned(info: &DeviceInfo) -> Option<CString> {
    CString::new(info.path().to_bytes()).ok()
}

/// List Logitech HID interfaces (for CLI diagnostics).
pub fn list_logi_devices() -> Result<Vec<String>, HidppError> {
    let api = HidApi::new().map_err(|e| HidppError::Io(e.to_string()))?;
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(false);
    let mut lines = Vec::new();
    for d in api.device_list().filter(|d| d.vendor_id() == LOGI_VID) {
        let tag = if is_hidpp_short(d) {
            " SHORT(FF00/1)"
        } else if is_hidpp_long(d) {
            " LONG(FF00/2)"
        } else if is_hidpp_bluetooth(d) {
            " HIDPP-BT(FF43/202)"
        } else {
            ""
        };
        let bolt = if is_bolt_receiver(d) { " bolt" } else { "" };
        let mx4 = if looks_like_mx_master_4(d) { " mx4" } else { "" };
        lines.push(format!(
            "pid=0x{:04X} page=0x{:04X} usage=0x{:04X} iface={}{}{}{} product={:?}",
            d.product_id(),
            d.usage_page(),
            d.usage(),
            d.interface_number(),
            tag,
            bolt,
            mx4,
            d.product_string()
        ));
    }
    Ok(lines)
}

pub fn open_best_transport() -> Result<Box<dyn HapticTransport>, HidppError> {
    let api = HidApi::new().map_err(|e| HidppError::Io(e.to_string()))?;
    // Critical on macOS: exclusive open seizes the whole BT mouse and freezes the cursor.
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(false);

    match open_bolt(&api) {
        Ok(t) => Ok(t),
        Err(bolt_err) => open_bluetooth(&api).map_err(|bt_err| {
            HidppError::Io(format!(
                "no usable MX4 link (bolt: {bolt_err}; bluetooth: {bt_err}). \
                 Run `music-drums-cli devices` and park Options+ with Scripts/logi-mode.sh disable."
            ))
        }),
    }
}

fn open_bolt(api: &HidApi) -> Result<Box<dyn HapticTransport>, HidppError> {
    let bolt_devs: Vec<&DeviceInfo> = api
        .device_list()
        .filter(|d| d.vendor_id() == LOGI_VID && is_bolt_receiver(d))
        .collect();
    if bolt_devs.is_empty() {
        return Err(HidppError::NotConnected);
    }

    let short_path = bolt_devs
        .iter()
        .find(|d| is_hidpp_short(d))
        .and_then(|d| path_owned(d))
        .ok_or_else(|| {
            HidppError::Io(
                "Bolt HID++ SHORT interface (page=0xFF00 usage=0x0001) not found".into(),
            )
        })?;
    let long_path = bolt_devs
        .iter()
        .find(|d| is_hidpp_long(d))
        .and_then(|d| path_owned(d))
        .ok_or_else(|| {
            HidppError::Io(
                "Bolt HID++ LONG interface (page=0xFF00 usage=0x0002) not found".into(),
            )
        })?;

    let short = open_path(api, short_path.as_c_str())?;
    let long = open_path(api, long_path.as_c_str())?;
    let _ = short.set_blocking_mode(false);
    let _ = long.set_blocking_mode(false);

    let mut short = Some(short);
    let mut long = Some(long);
    let mut last_err = HidppError::Io("no bolt device index worked".into());
    for device_index in [BOLT_DEVICE_INDEX, 0x01u8] {
        let long_ref = long.as_ref().ok_or(HidppError::NotConnected)?;
        let haptic_feature_index = Mx4Session::discover_haptic_index(long_ref, device_index);
        let mut session = Mx4Session {
            short: ShortPipe::Dedicated(short.take().ok_or(HidppError::NotConnected)?),
            long: long.take().ok_or(HidppError::NotConnected)?,
            device_index,
            haptic_feature_index,
            link: LinkKind::Bolt,
        };
        match session.set_haptic(false, 0) {
            Ok(()) => {
                let _ = session.ping();
                return Ok(Box::new(session));
            }
            Err(e) => {
                last_err = e;
                if let ShortPipe::Dedicated(d) = session.short {
                    short = Some(d);
                }
                long = Some(session.long);
            }
        }
    }
    Err(last_err)
}

fn open_bluetooth(api: &HidApi) -> Result<Box<dyn HapticTransport>, HidppError> {
    let candidates: Vec<&DeviceInfo> = api
        .device_list()
        .filter(|d| looks_like_mx_master_4(d))
        .collect();
    if candidates.is_empty() {
        return Err(HidppError::NotConnected);
    }

    // macOS BT exposes HID++ on FF43/0202 (single collection for 0x10 + 0x11).
    let bt_path = candidates
        .iter()
        .find(|d| is_hidpp_bluetooth(d))
        .and_then(|d| path_owned(d))
        .ok_or_else(|| {
            HidppError::Io(
                "Bluetooth HID++ interface (page=0xFF43 usage=0x0202) not found — \
                 is the mouse on Bluetooth? Bolt uses FF00 instead."
                    .into(),
            )
        })?;

    let long = open_path(api, bt_path.as_c_str())?;
    let _ = long.set_blocking_mode(false);

    let mut long = Some(long);
    let mut last_err = HidppError::Io("no bluetooth device index worked".into());
    // BLE direct devices commonly use 0xFF; some stacks use 0x00 / 0x01 / 0x02.
    for device_index in [0xFFu8, 0x00u8, 0x01u8, 0x02u8] {
        let long_ref = long.as_ref().ok_or(HidppError::NotConnected)?;
        let haptic_feature_index = Mx4Session::discover_haptic_index(long_ref, device_index);
        let mut session = Mx4Session {
            short: ShortPipe::SharedLongOnly,
            long: long.take().ok_or(HidppError::NotConnected)?,
            device_index,
            haptic_feature_index,
            link: LinkKind::Bluetooth,
        };
        match session.set_haptic(false, 0) {
            Ok(()) => {
                let _ = session.ping();
                return Ok(Box::new(session));
            }
            Err(e) => {
                last_err = e;
                long = Some(session.long);
            }
        }
    }
    Err(last_err)
}

pub fn open_best_transport_with_retry(
    attempts: u32,
    delay: Duration,
) -> Result<Box<dyn HapticTransport>, HidppError> {
    let mut last = HidppError::NotConnected;
    for i in 0..attempts {
        match open_best_transport() {
            Ok(t) => return Ok(t),
            Err(e) => {
                last = e;
                if i + 1 < attempts {
                    std::thread::sleep(delay);
                }
            }
        }
    }
    Err(last)
}
