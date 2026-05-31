use serde_derive::{Deserialize, Serialize};

use crate::utils::SnowflakeID;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageDeleteData {
    pub id: SnowflakeID,
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
}
