//! HID++ 2.0 helpers and MX Master 4 haptic feature (0x19B0).

use thiserror::Error;

pub const LOGI_VID: u16 = 0x046D;
/// MX Master 4 wireless PID as seen over Bluetooth / product id space.
pub const MX_MASTER_4_PID: u16 = 0xB042;
pub const FEATURE_ROOT: u16 = 0x0000;
pub const FEATURE_FEATURE_SET: u16 = 0x0001;
pub const FEATURE_HAPTIC: u16 = 0x19B0;

pub const SHORT_REPORT_LEN: usize = 7;
pub const LONG_REPORT_LEN: usize = 20;

#[derive(Debug, Error)]
pub enum HidppError {
    #[error("HID I/O error: {0}")]
    Io(String),
    #[error("feature 0x{0:04X} not found on device")]
    FeatureMissing(u16),
    #[error("invalid HID++ response")]
    BadResponse,
    #[error("device not connected")]
    NotConnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PulseType {
    Reset = 0x00,
    Light = 0x02,
    Tick = 0x04,
    Strong = 0x08,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HapticPulse {
    pub bitmask: u8,
}

impl HapticPulse {
    pub fn single(p: PulseType) -> Self {
        Self { bitmask: p as u8 }
    }

    pub fn compound(a: PulseType, b: PulseType) -> Self {
        Self {
            bitmask: (a as u8) | (b as u8),
        }
    }
}

/// Build a HID++ 2.0 short report.
/// Layout: [report_id=0x10, device_index, feature_index, func/swid, params...]
pub fn short_report(
    device_index: u8,
    feature_index: u8,
    function_id: u8,
    software_id: u8,
    params: &[u8],
) -> [u8; SHORT_REPORT_LEN] {
    let mut buf = [0u8; SHORT_REPORT_LEN];
    buf[0] = 0x10;
    buf[1] = device_index;
    buf[2] = feature_index;
    buf[3] = (function_id << 4) | (software_id & 0x0F);
    for (i, p) in params.iter().take(3).enumerate() {
        buf[4 + i] = *p;
    }
    buf
}

/// Build a HID++ 2.0 long report (report id 0x11).
pub fn long_report(
    device_index: u8,
    feature_index: u8,
    function_id: u8,
    software_id: u8,
    params: &[u8],
) -> [u8; LONG_REPORT_LEN] {
    let mut buf = [0u8; LONG_REPORT_LEN];
    buf[0] = 0x11;
    buf[1] = device_index;
    buf[2] = feature_index;
    buf[3] = (function_id << 4) | (software_id & 0x0F);
    for (i, p) in params.iter().take(16).enumerate() {
        buf[4 + i] = *p;
    }
    buf
}

/// Root feature 0x0000 function 0: getFeature(featureId) → (index, type, version)
pub fn get_feature_request(feature_id: u16) -> [u8; LONG_REPORT_LEN] {
    long_report(
        0xFF, // will be overwritten by transport with real device index when needed
        0x00, // root is always index 0
        0x00,
        0x01,
        &[(feature_id >> 8) as u8, (feature_id & 0xFF) as u8],
    )
}

pub fn parse_get_feature_response(data: &[u8]) -> Option<(u8, u8, u8)> {
    // Expect long response starting with 0x11
    if data.len() < 7 {
        return None;
    }
    let offset = if data[0] == 0x11 || data[0] == 0x10 {
        0
    } else {
        // Some stacks omit report id
        usize::MAX
    };
    let d = if offset == usize::MAX {
        data
    } else {
        &data[1..]
    };
    if d.len() < 6 {
        return None;
    }
    // d: [device, featIdx_echo?, func, featIndex, featType, version] — varies by stack.
    // Standard: response params at bytes 4.. of full report = index, type, version
    if data[0] == 0x11 || data[0] == 0x10 {
        Some((data[4], data[5], data[6]))
    } else {
        Some((d[3], d[4], d[5]))
    }
}

/// Haptic feature function 2: enable + intensity (MasterMice / logiops).
pub fn haptic_set_config_report(
    device_index: u8,
    feature_index: u8,
    enabled: bool,
    intensity: u8,
) -> [u8; SHORT_REPORT_LEN] {
    short_report(
        device_index,
        feature_index,
        0x2,
        0xA, // matches MasterMice 0x2A in byte3 when function=2 swid=0xA
        &[if enabled { 0x01 } else { 0x00 }, intensity.min(100), 0x00],
    )
}

/// Same as [`haptic_set_config_report`] but as a long (0x11) report — required on
/// macOS Bluetooth FF43 interfaces that only expose the long output report.
pub fn haptic_set_config_long_report(
    device_index: u8,
    feature_index: u8,
    enabled: bool,
    intensity: u8,
) -> [u8; LONG_REPORT_LEN] {
    long_report(
        device_index,
        feature_index,
        0x2,
        0xA,
        &[if enabled { 0x01 } else { 0x00 }, intensity.min(100), 0x00],
    )
}

/// Haptic feature function 4: trigger pulse bitmask.
pub fn haptic_trigger_report(
    device_index: u8,
    feature_index: u8,
    pulse: HapticPulse,
) -> [u8; SHORT_REPORT_LEN] {
    short_report(
        device_index,
        feature_index,
        0x4,
        0xA, // 0x4A
        &[pulse.bitmask, 0x00, 0x00],
    )
}

pub fn haptic_trigger_long_report(
    device_index: u8,
    feature_index: u8,
    pulse: HapticPulse,
) -> [u8; LONG_REPORT_LEN] {
    long_report(
        device_index,
        feature_index,
        0x4,
        0xA,
        &[pulse.bitmask, 0x00, 0x00],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_report_function_nibble() {
        let r = short_report(0x01, 0x0B, 0x4, 0xA, &[0x08, 0, 0]);
        assert_eq!(r[0], 0x10);
        assert_eq!(r[3], 0x4A);
        assert_eq!(r[4], 0x08);
    }

    #[test]
    fn compound_pulse_bits() {
        let p = HapticPulse::compound(PulseType::Strong, PulseType::Tick);
        assert_eq!(p.bitmask, 0x0C);
    }
}
