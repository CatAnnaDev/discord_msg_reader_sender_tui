use std::collections::{HashMap, VecDeque};

use crate::utils::SnowflakeID;

pub struct MessageBuffer {
    message: HashMap<SnowflakeID, String>,
    key_order: VecDeque<SnowflakeID>,
    max_size: usize,
}

impl MessageBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            message: HashMap::with_capacity(max_size),
            key_order: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    pub fn add_message(&mut self, id: SnowflakeID, message: String) {
        if self.message.len() >= self.max_size {
            if let Some(oldest) = self.key_order.pop_back() {
                self.message.remove(&oldest);
            }
        }
        self.message.insert(id, message);
        self.key_order.push_front(id);
    }

    pub fn get_message(&self, id: SnowflakeID) -> &str {
        self.message
            .get(&id)
            .map(String::as_str)
            .unwrap_or("Message not found!")
    }
}
