use crate::utils::SnowflakeID;
use serde_derive::Deserialize;
use serde_derive::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceChannelStatusUpdateData {
    pub status: Option<String>,
    pub id: SnowflakeID,
    #[serde(rename = "guild_id")]
    pub guild_id: SnowflakeID,
}
