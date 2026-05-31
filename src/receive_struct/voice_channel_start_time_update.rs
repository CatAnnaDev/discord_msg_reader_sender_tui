use crate::utils::SnowflakeID;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceChannelStartTimeUpdateData {
    pub start_time: Option<u64>,
    pub id: SnowflakeID,
    #[serde(rename = "guild_id")]
    pub guild_id: SnowflakeID,
}
