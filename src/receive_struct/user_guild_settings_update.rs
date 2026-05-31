use serde_derive::Deserialize;
use serde_derive::Serialize;

use crate::utils::SnowflakeID;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserGuildSettingsUpdateData {
    pub version: i64,
    #[serde(rename = "suppress_roles")]
    pub suppress_roles: bool,
    #[serde(rename = "suppress_everyone")]
    pub suppress_everyone: bool,
    #[serde(rename = "notify_highlights")]
    pub notify_highlights: i64,
    pub muted: bool,
    #[serde(rename = "mute_scheduled_events")]
    pub mute_scheduled_events: bool,
    #[serde(rename = "mute_config")]
    pub mute_config: Option<MuteConfig>,
    #[serde(rename = "mobile_push")]
    pub mobile_push: bool,
    #[serde(rename = "message_notifications")]
    pub message_notifications: i64,
    #[serde(rename = "hide_muted_channels")]
    pub hide_muted_channels: bool,
    #[serde(rename = "guild_id")]
    pub guild_id: Option<SnowflakeID>,
    pub flags: i64,
    #[serde(rename = "channel_overrides")]
    pub channel_overrides: Vec<ChannelOverride>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MuteConfig {
    #[serde(rename = "selected_time_window")]
    pub selected_time_window: i64,
    #[serde(rename = "end_time")]
    pub end_time: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelOverride {
    pub muted: bool,
    #[serde(rename = "mute_config")]
    pub mute_config: Option<MuteConfig2>,
    #[serde(rename = "message_notifications")]
    pub message_notifications: i64,
    pub flags: Option<i64>,
    pub collapsed: bool,
    #[serde(rename = "channel_id")]
    pub channel_id: SnowflakeID,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MuteConfig2 {
    #[serde(rename = "selected_time_window")]
    pub selected_time_window: i64,
    #[serde(rename = "end_time")]
    pub end_time: Option<String>,
}
