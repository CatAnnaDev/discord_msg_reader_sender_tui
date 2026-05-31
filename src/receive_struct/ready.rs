use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

use crate::utils::SnowflakeID;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReadyData {
    #[serde(default)]
    pub user_settings: Option<UserSettings>,

    #[serde(default)]
    pub user_guild_settings: UserGuildSettingsContainer,
    pub user: Option<UserData>,
    #[serde(default)]
    pub sessions: Vec<SessionsData>,
    pub session_type: Option<String>,
    pub session_id: Option<String>,
    pub resume_gateway_url: Option<String>,
    pub private_channels: Option<Vec<PrivateChannels>>,

    #[serde(default)]
    pub users: Vec<UserStub>,
    #[serde(default)]
    pub guilds: Vec<Guild>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UserStub {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub bot: bool,

    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct UserGuildSettingsContainer {
    pub entries: Vec<UserGuildSetting>,
    pub version: i64,
    pub partial: bool,
}

impl<'de> serde::Deserialize<'de> for UserGuildSettingsContainer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v: serde_json::Value = serde::Deserialize::deserialize(d)?;
        match v {
            serde_json::Value::Array(arr) => {
                let entries: Vec<UserGuildSetting> = arr
                    .into_iter()
                    .filter_map(|x| serde_json::from_value(x).ok())
                    .collect();
                Ok(UserGuildSettingsContainer {
                    entries,
                    ..Default::default()
                })
            }
            serde_json::Value::Object(_) => {
                #[derive(serde_derive::Deserialize)]
                struct Inner {
                    #[serde(default)]
                    entries: Vec<UserGuildSetting>,
                    #[serde(default)]
                    version: i64,
                    #[serde(default)]
                    partial: bool,
                }
                let inner: Inner = serde_json::from_value(v).map_err(D::Error::custom)?;
                Ok(UserGuildSettingsContainer {
                    entries: inner.entries,
                    version: inner.version,
                    partial: inner.partial,
                })
            }
            serde_json::Value::Null => Ok(UserGuildSettingsContainer::default()),
            _ => Err(D::Error::custom(
                "user_guild_settings: expected array or object",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserSettings {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UserGuildSetting {
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub suppress_roles: bool,
    #[serde(default)]
    pub suppress_everyone: bool,
    #[serde(default)]
    pub notify_highlights: i64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub mute_scheduled_events: bool,
    pub mute_config: Option<MuteConfig>,
    #[serde(default)]
    pub mobile_push: bool,
    #[serde(default)]
    pub message_notifications: i64,
    pub hide_muted_channels: Option<bool>,
    pub guild_id: Option<SnowflakeID>,
    pub flags: Option<i64>,
    #[serde(default)]
    pub channel_overrides: Vec<ChannelOverride>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MuteConfig {
    pub selected_time_window: Option<i64>,
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChannelOverride {
    #[serde(default)]
    pub muted: bool,
    pub mute_config: Option<MuteConfig>,
    #[serde(default)]
    pub message_notifications: i64,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub channel_id: SnowflakeID,
    pub flags: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionsData {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub session_id: String,
    pub client_info: Option<ClientInfo>,
    #[serde(default)]
    pub activities: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ClientInfo {
    #[serde(default)]
    pub client: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub version: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrivateChannels {
    #[serde(rename = "type", default)]
    pub type_field: i64,
    #[serde(default)]
    pub safety_warnings: Option<Vec<Value>>,

    #[serde(default)]
    pub recipients: Vec<Recipient>,
    #[serde(default)]
    pub recipient_ids: Vec<String>,
    pub last_pin_timestamp: Option<String>,
    pub name: Option<String>,
    pub last_message_id: Option<SnowflakeID>,
    pub is_spam: Option<bool>,
    #[serde(default)]
    pub id: SnowflakeID,
    #[serde(default)]
    pub flags: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Recipient {
    #[serde(default)]
    pub username: String,
    pub public_flags: Option<i64>,
    #[serde(default)]
    pub id: String,
    pub global_name: Option<String>,
    #[serde(default)]
    pub discriminator: String,
    pub avatar_decoration_data: Option<Value>,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserData {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub id: SnowflakeID,
    #[serde(default)]
    pub global_name: String,
    pub avatar: Option<String>,
    pub email: Option<String>,
    #[serde(default)]
    pub discriminator: String,
    #[serde(default)]
    pub flags: i64,
    pub premium_type: Option<i64>,
    #[serde(default)]
    pub mfa_enabled: bool,
    #[serde(default)]
    pub verified: bool,
    pub phone: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Guild {
    #[serde(default)]
    pub id: SnowflakeID,

    pub properties: Option<GuildProperties>,

    pub name: Option<String>,

    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub channels: Vec<Channel>,
    #[serde(default)]
    pub threads: Vec<Thread>,
    #[serde(default)]
    pub member_count: i64,
    #[serde(default)]
    pub large: bool,
    #[serde(default)]
    pub lazy: bool,
    pub joined_at: Option<String>,
    #[serde(default)]
    pub emojis: Vec<Value>,
    #[serde(default)]
    pub roles: Vec<Value>,
    #[serde(default)]
    pub stickers: Vec<Value>,

    #[serde(default)]
    pub voice_states: Vec<Value>,
}

impl Guild {
    pub fn display_name(&self) -> &str {
        self.properties
            .as_ref()
            .and_then(|p| p.name.as_deref())
            .or(self.name.as_deref())
            .unwrap_or("unknown")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GuildProperties {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub owner_id: Option<SnowflakeID>,
    pub banner: Option<String>,
    pub splash: Option<String>,
    pub vanity_url_code: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    pub preferred_locale: Option<String>,
    pub afk_channel_id: Option<String>,
    pub afk_timeout: Option<i64>,
    pub max_members: Option<i64>,
    pub verification_level: Option<i64>,
    pub default_message_notifications: Option<i64>,
    pub explicit_content_filter: Option<i64>,
    pub mfa_level: Option<i64>,
    pub nsfw_level: Option<i64>,
    pub premium_tier: Option<i64>,
    pub system_channel_id: Option<String>,
    pub system_channel_flags: Option<i64>,
    pub public_updates_channel_id: Option<String>,
    pub rules_channel_id: Option<String>,
    pub safety_alerts_channel_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Channel {
    #[serde(rename = "type", default)]
    pub type_field: i64,
    #[serde(default)]
    pub id: SnowflakeID,
    #[serde(default)]
    pub name: String,
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
    pub permission_overwrites: Vec<Value>,
    pub theme_color: Option<i64>,
    pub status: Option<Value>,
    pub icon_emoji: Option<Value>,
    pub default_auto_archive_duration: Option<i64>,
    pub default_thread_rate_limit_per_user: Option<i64>,
    pub template: Option<String>,
    pub default_sort_order: Option<i64>,
    pub default_reaction_emoji: Option<Value>,
    pub default_forum_layout: Option<i64>,
    pub default_tag_setting: Option<Value>,
    #[serde(default)]
    pub available_tags: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Thread {
    #[serde(rename = "type", default)]
    pub type_field: i64,
    #[serde(default)]
    pub id: SnowflakeID,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub flags: i64,
    pub parent_id: Option<SnowflakeID>,
    pub owner_id: Option<SnowflakeID>,
    pub guild_id: Option<SnowflakeID>,
    pub last_message_id: Option<SnowflakeID>,
    pub message_count: Option<i64>,
    pub member_count: Option<i64>,
    pub total_message_sent: Option<i64>,
    pub rate_limit_per_user: Option<i64>,
    #[serde(default)]
    pub member_ids_preview: Vec<String>,
    pub thread_metadata: Option<Value>,
    pub member: Option<Value>,
    #[serde(default)]
    pub applied_tags: Vec<String>,
}
