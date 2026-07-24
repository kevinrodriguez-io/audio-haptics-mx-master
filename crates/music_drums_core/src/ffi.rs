//! C ABI for the Swift menu-bar app.

use crate::config::{
    list_presets, load_preset_by_id, save_user_preset, DrumsConfig, PresetInfo,
};
use crate::engine::{Engine, EngineConfig};
use crate::transport::LinkKind;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int, c_uint};
use std::ptr;
use std::sync::{Arc, OnceLock};

static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();

fn engine() -> &'static Arc<Engine> {
    ENGINE.get_or_init(|| Engine::new(EngineConfig::default()))
}

fn status_json() -> String {
    serde_json::to_string(&engine().status()).unwrap_or_else(|_| {
        "{\"running\":false,\"link\":\"none\",\"sensitivity\":0.65,\"preset_id\":\"classic\",\"preset_name\":\"Classic\",\"last_error\":\"serialize\",\"hits_fired\":0}".into()
    })
}

fn set_out_error(out_error: *mut *mut c_char, msg: String) {
    if out_error.is_null() {
        return;
    }
    if let Ok(c) = CString::new(msg) {
        unsafe {
            *out_error = c.into_raw();
        }
    }
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
            set_out_error(out_error, e);
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

#[no_mangle]
pub unsafe extern "C" fn md_set_sample_rate(value: c_float) {
    engine().set_sample_rate(value);
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

/// Current drums config as pretty JSON. Caller must `md_string_free`.
#[no_mangle]
pub unsafe extern "C" fn md_config_json() -> *mut c_char {
    match engine().config().to_json_pretty() {
        Ok(json) => CString::new(json)
            .map(|c| c.into_raw())
            .unwrap_or(ptr::null_mut()),
        Err(_) => ptr::null_mut(),
    }
}

/// Apply config JSON. Returns 0 on success.
#[no_mangle]
pub unsafe extern "C" fn md_set_config_json(json: *const c_char, out_error: *mut *mut c_char) -> c_int {
    if !out_error.is_null() {
        *out_error = ptr::null_mut();
    }
    if json.is_null() {
        set_out_error(out_error, "null config".into());
        return -1;
    }
    let s = match CStr::from_ptr(json).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_out_error(out_error, e.to_string());
            return -1;
        }
    };
    match DrumsConfig::from_json(s).and_then(|c| engine().set_config(c)) {
        Ok(()) => 0,
        Err(e) => {
            set_out_error(out_error, e);
            -1
        }
    }
}

/// Builtin + user preset catalog JSON. Caller must `md_string_free`.
#[no_mangle]
pub unsafe extern "C" fn md_list_presets_json() -> *mut c_char {
    let list: Vec<PresetInfo> = list_presets();
    match serde_json::to_string(&list) {
        Ok(json) => CString::new(json)
            .map(|c| c.into_raw())
            .unwrap_or(ptr::null_mut()),
        Err(_) => ptr::null_mut(),
    }
}

/// Load builtin or user preset by id (or file path). Returns 0 on success.
#[no_mangle]
pub unsafe extern "C" fn md_load_preset(id: *const c_char, out_error: *mut *mut c_char) -> c_int {
    if !out_error.is_null() {
        *out_error = ptr::null_mut();
    }
    if id.is_null() {
        set_out_error(out_error, "null preset id".into());
        return -1;
    }
    let s = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_out_error(out_error, e.to_string());
            return -1;
        }
    };
    match load_preset_by_id(s).and_then(|c| engine().set_config(c)) {
        Ok(()) => 0,
        Err(e) => {
            set_out_error(out_error, e);
            -1
        }
    }
}

/// Save current config into Application Support presets as `{id}.json`.
#[no_mangle]
pub unsafe extern "C" fn md_save_preset(out_error: *mut *mut c_char) -> c_int {
    if !out_error.is_null() {
        *out_error = ptr::null_mut();
    }
    let cfg = engine().config();
    match save_user_preset(&cfg) {
        Ok(path) => {
            // Return path via out_error channel as success message? Better: ignore, path in status.
            let _ = path;
            0
        }
        Err(e) => {
            set_out_error(out_error, e);
            -1
        }
    }
}

/// Export current config to an absolute path.
#[no_mangle]
pub unsafe extern "C" fn md_export_config(
    path: *const c_char,
    out_error: *mut *mut c_char,
) -> c_int {
    if !out_error.is_null() {
        *out_error = ptr::null_mut();
    }
    if path.is_null() {
        set_out_error(out_error, "null path".into());
        return -1;
    }
    let s = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_out_error(out_error, e.to_string());
            return -1;
        }
    };
    match engine().config().save_file(std::path::Path::new(s)) {
        Ok(()) => 0,
        Err(e) => {
            set_out_error(out_error, e);
            -1
        }
    }
}

/// Import config from path and make it active.
#[no_mangle]
pub unsafe extern "C" fn md_import_config(
    path: *const c_char,
    out_error: *mut *mut c_char,
) -> c_int {
    if !out_error.is_null() {
        *out_error = ptr::null_mut();
    }
    if path.is_null() {
        set_out_error(out_error, "null path".into());
        return -1;
    }
    let s = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_out_error(out_error, e.to_string());
            return -1;
        }
    };
    match DrumsConfig::load_file(std::path::Path::new(s)).and_then(|c| engine().set_config(c)) {
        Ok(()) => 0,
        Err(e) => {
            set_out_error(out_error, e);
            -1
        }
    }
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
            set_out_error(out_error, e);
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
    static V: &[u8] = b"0.2.0\0";
    V.as_ptr() as *const c_char
}
