pub mod audio;
pub mod capture;
#[cfg(target_os = "linux")]
pub mod capture_wayland;
pub mod dave;
pub mod dsp;
pub mod gateway;
pub mod manager;
pub mod udp;
pub mod video;
pub mod vsend;
#[cfg(target_os = "macos")]
#[path = "vtdec.rs"]
pub mod vtdec;
#[cfg(not(target_os = "macos"))]
#[path = "vtdec_stub.rs"]
pub mod vtdec;

pub use manager::{VoiceCommand, VoiceManager, VoiceSignal};
