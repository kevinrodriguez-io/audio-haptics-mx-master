//! Music Drums core: system-audio onset → MX Master 4 HID++ haptics.

pub mod audio;
pub mod dsp;
pub mod engine;
pub mod ffi;
pub mod hidpp;
pub mod mapper;
pub mod transport;

pub use engine::{Engine, EngineConfig, EngineStatus};
pub use transport::{list_logi_devices, LinkKind};
