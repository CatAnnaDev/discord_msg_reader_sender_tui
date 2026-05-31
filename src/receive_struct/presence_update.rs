use crate::utils::SnowflakeID;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceData {
    pub user: User,
    pub status: String,
    #[serde(rename = "processed_at_timestamp")]
    pub processed_at_timestamp: i64,
    #[serde(rename = "guild_id")]
    pub guild_id: Option<String>,
    #[serde(rename = "client_status")]
    pub client_status: ClientStatus,
    pub activities: Vec<Activity>,
    #[serde(rename = "restricted_application")]
    pub restricted_application: Option<Value>,
    #[serde(rename = "hidden_activities")]
    #[serde(default)]
    pub hidden_activities: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: SnowflakeID,
    pub username: Option<String>,
    #[serde(rename = "primary_guild")]
    pub primary_guild: Option<PrimaryGuild>,
    #[serde(rename = "global_name")]
    pub global_name: Option<String>,
    #[serde(rename = "display_name_styles")]
    pub display_name_styles: Option<DisplayNameStyles>,
    pub discriminator: Option<String>,
    pub collectibles: Option<Collectibles>,
    pub clan: Option<Clan>,
    pub bot: Option<bool>,
    #[serde(rename = "avatar_decoration_data")]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    pub avatar: Option<String>,
    #[serde(rename = "public_flags")]
    pub public_flags: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimaryGuild {
    pub tag: Option<String>,
    #[serde(rename = "identity_guild_id")]
    pub identity_guild_id: Option<String>,
    #[serde(rename = "identity_enabled")]
    pub identity_enabled: bool,
    pub badge: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayNameStyles {
    #[serde(rename = "font_id")]
    pub font_id: i64,
    #[serde(rename = "effect_id")]
    pub effect_id: i64,
    pub colors: Vec<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collectibles {
    pub nameplate: Nameplate,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Nameplate {
    #[serde(rename = "sku_id")]
    pub sku_id: String,
    pub palette: String,
    pub label: String,
    #[serde(rename = "expires_at")]
    pub expires_at: Value,
    pub asset: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clan {
    pub tag: Option<String>,
    #[serde(rename = "identity_guild_id")]
    pub identity_guild_id: Option<String>,
    #[serde(rename = "identity_enabled")]
    pub identity_enabled: bool,
    pub badge: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarDecorationData {
    #[serde(rename = "sku_id")]
    pub sku_id: String,
    #[serde(rename = "expires_at")]
    pub expires_at: Value,
    pub asset: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStatus {
    pub desktop: Option<String>,
    pub mobile: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    #[serde(rename = "type")]
    pub type_field: i64,
    pub timestamps: Option<Timestamps>,
    #[serde(rename = "session_id")]
    pub session_id: Option<String>,
    pub name: String,
    pub id: String,
    #[serde(rename = "created_at")]
    pub created_at: i64,
    #[serde(rename = "application_id")]
    pub application_id: Option<String>,
    pub state: Option<String>,
    pub platform: Option<String>,
    pub assets: Option<Assets>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timestamps {
    pub start: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assets {
    #[serde(rename = "small_text")]
    pub small_text: Option<String>,
    #[serde(rename = "small_image")]
    pub small_image: Option<String>,
    #[serde(rename = "large_text")]
    pub large_text: Option<String>,
    #[serde(rename = "large_image")]
    pub large_image: Option<String>,
}
