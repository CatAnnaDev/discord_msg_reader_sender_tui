use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

use crate::utils::SnowflakeID;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageRef {
    pub message_id: SnowflakeID,
    pub guild_id: SnowflakeID,
    pub channel_id: SnowflakeID,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefMessage {
    #[serde(rename = "type")]
    pub type_field: u8,
    pub tts: bool,
    pub timestamp: String,
    pub pinned: bool,

    pub content: String,
    pub author: Author,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageCreateData {
    #[serde(rename = "type")]
    pub type_field: u8,
    pub tts: bool,
    pub timestamp: String,
    pub sticker_items: Option<Vec<Stickers>>,
    pub stickers: Option<Vec<Value>>,
    pub referenced_message: Option<RefMessage>,
    pub pinned: bool,
    pub mentions: Vec<Mentions>,
    pub mention_roles: Vec<String>,
    pub mention_everyone: bool,
    pub mention_channels: Option<Vec<MentionChannel>>,
    pub reactions: Option<Vec<Reactions>>,
    pub nonce: Option<Value>,
    pub webhook_id: Option<SnowflakeID>,
    pub activity: Option<Value>,
    pub application: Option<Value>,
    pub application_id: Option<SnowflakeID>,
    pub interaction_metadata: Option<InteractionMetadata>,
    pub interaction: Option<Interaction>,
    pub thread: Option<Value>,
    pub position: Option<u32>,
    pub role_subscription_data: Option<Value>,
    pub resolved: Option<Value>,
    pub poll: Option<Value>,
    pub member: Option<Member>,
    pub id: SnowflakeID,
    pub flags: i64,
    pub embeds: Vec<Embed>,
    pub edited_timestamp: Option<String>,
    pub content: String,
    pub components: Vec<Value>,
    pub channel_id: SnowflakeID,
    pub call: Option<CallState>,
    pub author: Author,
    pub attachments: Vec<Attachments>,
    pub guild_id: Option<SnowflakeID>,
    pub message_reference: Option<MessageReference>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionMetadata {
    pub user: User,
    #[serde(rename = "type")]
    pub type_field: i64,
    pub name: Option<String>,
    pub id: SnowflakeID,
    #[serde(rename = "authorizing_integration_owners")]
    pub authorizing_integration_owners: AuthorizingIntegrationOwners,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    #[serde(rename = "public_flags")]
    pub public_flags: i64,
    pub primary_guild: Option<Value>,
    pub id: SnowflakeID,
    #[serde(rename = "global_name")]
    pub global_name: String,
    pub discriminator: String,
    pub clan: Value,
    #[serde(rename = "avatar_decoration_data")]
    pub avatar_decoration_data: Option<Value>,
    pub avatar: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizingIntegrationOwners {
    #[serde(rename = "0")]
    pub n0: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interaction {
    pub user: User2,
    #[serde(rename = "type")]
    pub type_field: i64,
    pub name: String,
    pub member: Members,
    pub id: SnowflakeID,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User2 {
    pub username: String,
    #[serde(rename = "public_flags")]
    pub public_flags: i64,
    pub id: SnowflakeID,
    #[serde(rename = "global_name")]
    pub global_name: String,
    pub discriminator: String,
    pub clan: Value,
    #[serde(rename = "avatar_decoration_data")]
    pub avatar_decoration_data: Value,
    pub avatar: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Members {
    pub roles: Vec<SnowflakeID>,
    #[serde(rename = "premium_since")]
    pub premium_since: Option<Value>,
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
pub struct Reactions {
    pub count: u16,
    pub guild_id: SnowflakeID,
    #[serde(rename = "type")]
    pub type_field: u8,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MentionChannel {
    pub id: SnowflakeID,
    pub count_details: CountDetails,
    pub me: bool,
    pub me_burst: bool,
    pub emoji: Value,
    pub burst_colors: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CountDetails {
    pub burst: u16,
    pub normal: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageReference {
    pub channel_id: SnowflakeID,
    pub message_id: SnowflakeID,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallState {
    pub participants: Vec<SnowflakeID>,
    pub ended_timestamp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stickers {
    pub name: String,
    pub id: SnowflakeID,
    pub format_type: i8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attachments {
    pub width: Option<i64>,
    pub url: Option<String>,
    pub size: i64,
    pub proxy_url: Option<String>,
    pub placeholder_version: Option<i64>,
    pub placeholder: Option<String>,
    pub id: String,
    pub height: Option<i64>,
    pub filename: String,
    pub content_type: Option<String>,
    pub content_scan_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mentions {
    pub username: String,
    pub public_flags: Option<i64>,
    pub id: String,
    pub global_name: Option<String>,
    pub discriminator: String,
    pub avatar_decoration_data: Option<AvatarDecoration>,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub roles: Vec<String>,
    pub premium_since: Option<String>,
    pub pending: bool,
    pub nick: Option<String>,
    pub mute: bool,
    pub joined_at: String,
    pub flags: i64,
    pub deaf: bool,
    pub communication_disabled_until: Option<Value>,
    pub banner: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AvatarDecoration {
    pub sku_id: String,
    pub asset: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Author {
    pub username: String,
    pub public_flags: Option<i64>,
    pub premium_type: Option<i64>,
    pub id: SnowflakeID,
    pub global_name: Option<String>,
    pub display_name_styles: Option<Value>,
    pub discriminator: String,
    pub collectibles: Option<Value>,
    pub clan: Option<Clan>,
    pub bot: Option<bool>,
    pub avatar_decoration_data: Option<AvatarDecoration>,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clan {
    pub tag: Option<String>,
    pub identity_guild_id: Option<String>,
    pub identity_enabled: bool,
    pub badge: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embed {
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub title: Option<String>,
    pub thumbnail: Option<Thumbnail>,
    pub image: Option<Image>,
    pub video: Option<Video>,
    pub fields: Option<Vec<Fields>>,
    pub description: Option<String>,
    pub color: Option<i64>,
    pub provider: Option<Provider>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provider {
    pub url: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    pub width: i64,
    pub url: Option<String>,
    pub proxy_url: Option<String>,
    pub placeholder_version: i8,
    pub placeholder: String,
    pub height: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fields {
    pub value: String,
    pub name: String,
    pub inline: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thumbnail {
    pub width: i64,
    pub url: Option<String>,
    pub proxy_url: Option<String>,
    pub height: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub width: i64,
    pub url: Option<String>,
    pub proxy_url: Option<String>,
    pub height: i64,
}
