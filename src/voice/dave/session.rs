use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::frame::KeyRatchet;

pub struct DaveSession {
    pub enabled: bool,

    pub tx: Option<KeyRatchet>,

    pub rx: HashMap<u32, KeyRatchet>,

    pub candidates: Vec<(u64, KeyRatchet)>,

    pub tx_nonce: u32,
}

impl DaveSession {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            enabled: false,
            tx: None,
            rx: HashMap::new(),
            candidates: Vec::new(),
            tx_nonce: 0,
        }))
    }
}

pub type SharedDave = Arc<Mutex<DaveSession>>;
