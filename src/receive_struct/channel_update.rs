use crate::utils::SnowflakeID;
use serde::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

#[derive(Serialize, Deserialize)]
struct Struct {
    #[serde(rename = "type")]
    pub r#type: i64,
    pub id: String,
    pub deny: String,
    pub allow: String,
}

#[derive(Serialize, Deserialize)]
struct ChannelUpdateData {
    pub version: i64,
    #[serde(rename = "type")]
    pub r#type: i64,
    pub topic: Option<String>,
    pub rate_limit_per_user: i64,
    pub position: i64,
    pub permission_overwrites: Option<Vec<Struct>>,
    pub parent_id: Option<Value>,
    pub nsfw: bool,
    pub name: String,
    pub last_message_id: Option<SnowflakeID>,
    pub id: SnowflakeID,
    pub guild_id: SnowflakeID,
    pub flags: i64,
    pub bitrate: i64,
}
