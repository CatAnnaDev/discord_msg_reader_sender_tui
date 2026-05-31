use serde_derive::{Deserialize, Serialize};
use serde_json::Value;

use crate::utils::SnowflakeID;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelCreateData {
    #[serde(rename = "type", default)]
    pub type_field: i64,
    #[serde(default)]
    pub id: SnowflakeID,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub flags: i64,
    pub parent_id: Option<SnowflakeID>,
    pub topic: Option<String>,
    pub nsfw: Option<bool>,
    pub last_message_id: Option<SnowflakeID>,
    pub last_pin_timestamp: Option<String>,
    pub rate_limit_per_user: Option<i64>,
    pub bitrate: Option<i64>,
    pub user_limit: Option<i64>,
    pub rtc_region: Option<String>,
    pub video_quality_mode: Option<i64>,
    #[serde(default)]
    pub permission_overwrites: Vec<PermissionOverwrite>,
    pub theme_color: Option<i64>,
    pub status: Option<Value>,
    pub icon_emoji: Option<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionOverwrite {
    #[serde(rename = "type", default)]
    pub type_field: i64,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub deny: String,
    #[serde(default)]
    pub allow: String,
}
