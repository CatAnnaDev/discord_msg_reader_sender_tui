use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

use crate::utils::SnowflakeID;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingData {
    #[serde(rename = "user_id")]
    pub user_id: SnowflakeID,
    pub timestamp: i64,
    pub member: Option<Member>,
    #[serde(rename = "channel_id")]
    pub channel_id: SnowflakeID,
    #[serde(rename = "guild_id")]
    pub guild_id: Option<SnowflakeID>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub user: User,
    pub roles: Vec<String>,
    #[serde(rename = "premium_since")]
    pub premium_since: Value,
    pub pending: bool,
    pub nick: Option<String>,
    pub mute: bool,
    #[serde(rename = "joined_at")]
    pub joined_at: String,
    pub flags: i64,
    pub deaf: bool,
    #[serde(rename = "communication_disabled_until")]
    pub communication_disabled_until: Value,
    pub avatar: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    #[serde(rename = "public_flags")]
    pub public_flags: i64,
    pub id: SnowflakeID,
    #[serde(rename = "global_name")]
    pub global_name: Option<String>,
    #[serde(rename = "display_name")]
    pub display_name: Option<String>,
    pub discriminator: String,
    pub bot: bool,
    #[serde(rename = "avatar_decoration_data")]
    pub avatar_decoration_data: Option<Value>,
    pub avatar: Option<String>,
}
