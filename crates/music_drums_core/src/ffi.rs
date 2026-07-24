//! C ABI for the Swift menu-bar app.

use crate::engine::{Engine, EngineConfig};
use crate::transport::LinkKind;
use std::ffi::CString;
use std::os::raw::{c_char, c_float, c_int, c_uint};
use std::ptr;
use std::sync::{Arc, OnceLock};

static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();

fn engine() -> &'static Arc<Engine> {
    ENGINE.get_or_init(|| Engine::new(EngineConfig::default()))
}

fn status_json() -> String {
    serde_json::to_string(&engine().status()).unwrap_or_else(|_| {
        "{\"running\":false,\"link\":\"none\",\"sensitivity\":0.65,\"last_error\":\"serialize\",\"hits_fired\":0}".into()
    })
}

/// Start the haptic engine (HID++). Call after parking Logi Options+.
#[no_mangle]
pub unsafe extern "C" fn md_start() -> c_int {
    md_start_with_error(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn md_start_with_error(out_error: *mut *mut c_char) -> c_int {
    if !out_error.is_null() {
        *out_error = ptr::null_mut();
    }
    match engine().start() {
        Ok(()) => 0,
        Err(e) => {
            if !out_error.is_null() {
                if let Ok(c) = CString::new(e) {
                    *out_error = c.into_raw();
                }
            }
            -1
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn md_stop() {
    engine().stop();
}

#[no_mangle]
pub unsafe extern "C" fn md_set_sensitivity(value: c_float) {
    engine().set_sensitivity(value);
}

/// Push interleaved stereo f32 frames from the Swift Process Tap.
#[no_mangle]
pub unsafe extern "C" fn md_push_audio_frames(frames: *const c_float, count: c_uint) {
    if frames.is_null() || count == 0 {
        return;
    }
    let slice = std::slice::from_raw_parts(frames, count as usize);
    engine().push_audio(slice);
}

/// Returns a heap-allocated JSON status string. Caller must `md_string_free`.
#[no_mangle]
pub unsafe extern "C" fn md_status_json() -> *mut c_char {
    let json = status_json();
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn md_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    drop(CString::from_raw(s));
}

/// Fire a single strong test pulse. Returns 0 on success.
#[no_mangle]
pub unsafe extern "C" fn md_test_pulse(intensity: c_uint) -> c_int {
    md_test_pulse_with_error(intensity, ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn md_test_pulse_with_error(
    intensity: c_uint,
    out_error: *mut *mut c_char,
) -> c_int {
    if !out_error.is_null() {
        *out_error = ptr::null_mut();
    }
    match crate::engine::fire_test_pulse(intensity as u8) {
        Ok(_) => 0,
        Err(e) => {
            if !out_error.is_null() {
                if let Ok(c) = CString::new(e) {
                    *out_error = c.into_raw();
                }
            }
            -1
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn md_link_kind() -> c_int {
    match engine().status().link {
        LinkKind::None => 0,
        LinkKind::Bolt => 1,
        LinkKind::Bluetooth => 2,
    }
}

#[no_mangle]
pub unsafe extern "C" fn md_version() -> *const c_char {
    static V: &[u8] = b"0.1.0\0";
    V.as_ptr() as *const c_char
}
