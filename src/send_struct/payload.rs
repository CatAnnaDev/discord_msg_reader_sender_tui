use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SilentMode {
    Silent = 4096,
    None = 0,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Payload {
    content: String,
    nonce: u32,
    tts: bool,
    flags: i16,
}

impl _PayloadGenerator for Payload {
    fn build_payload(content: String, silent: SilentMode) -> Payload {
        Payload {
            content,
            nonce: nonce_generator(),
            tts: false,
            flags: silent as i16,
        }
    }
}

pub trait _PayloadGenerator {
    fn build_payload(content: String, silent: SilentMode) -> Payload;
}

fn nonce_generator() -> u32 {
    rand::thread_rng().next_u32()
}
