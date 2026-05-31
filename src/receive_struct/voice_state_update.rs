use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

use crate::utils::SnowflakeID;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStateUpdateData {
    pub member: Option<Member>,
    #[serde(rename = "user_id")]
    pub user_id: SnowflakeID,
    pub suppress: bool,
    #[serde(rename = "session_id")]
    pub session_id: String,
    #[serde(rename = "self_video")]
    pub self_video: bool,
    #[serde(rename = "self_mute")]
    pub self_mute: bool,
    #[serde(rename = "self_deaf")]
    pub self_deaf: bool,
    #[serde(rename = "request_to_speak_timestamp")]
    pub request_to_speak_timestamp: Value,
    pub mute: bool,
    #[serde(rename = "guild_id")]
    pub guild_id: Option<SnowflakeID>,
    pub deaf: bool,
    #[serde(rename = "channel_id")]
    pub channel_id: Option<SnowflakeID>,
    #[serde(rename = "self_stream")]
    pub self_stream: Option<bool>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub user: User,
    pub roles: Vec<Value>,
    #[serde(rename = "premium_since")]
    pub premium_since: Value,
    pub pending: bool,
    pub nick: Value,
    pub mute: bool,
    #[serde(rename = "joined_at")]
    pub joined_at: String,
    pub flags: i64,
    pub deaf: bool,
    #[serde(rename = "communication_disabled_until")]
    pub communication_disabled_until: Value,
    pub avatar: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    #[serde(rename = "public_flags")]
    pub public_flags: i64,
    pub id: SnowflakeID,
    #[serde(rename = "global_name")]
    pub global_name: String,
    #[serde(rename = "display_name")]
    pub display_name: String,
    pub discriminator: String,
    pub bot: bool,
    #[serde(rename = "avatar_decoration_data")]
    pub avatar_decoration_data: Value,
    pub avatar: Value,
}
