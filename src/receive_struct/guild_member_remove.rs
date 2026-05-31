use crate::utils::SnowflakeID;
use serde::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug)]
pub struct Struct {
    pub tag: String,
    pub identity_guild_id: String,
    pub identity_enabled: bool,
    pub badge: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub username: String,
    pub public_flags: i64,
    pub primary_guild: Option<Struct>,
    pub id: String,
    pub global_name: String,
    pub display_name_styles: Option<Value>,
    pub discriminator: String,
    pub collectibles: Option<Value>,
    pub clan: Option<Struct>,
    pub avatar_decoration_data: Option<Value>,
    pub avatar: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct GuildMemberRemove {
    pub user: User,
    pub guild_id: SnowflakeID,
}
