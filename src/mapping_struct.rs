use serde_derive::{Deserialize, Serialize};

use crate::receive_struct::ready::Recipient;
use crate::utils::SnowflakeID;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerMapping {
    pub size: u16,
    pub server_id: SnowflakeID,
    pub server_name: String,
    pub server_channels: Vec<ServerChannel>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerChannel {
    pub channel_type: String,
    pub channel_id: SnowflakeID,
    pub channel_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DMMapping {
    pub channel_id: SnowflakeID,
    pub channel_name: Option<String>,
    pub participant: Vec<Recipient>,
}
