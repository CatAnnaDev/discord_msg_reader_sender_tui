use crate::utils::SnowflakeID;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct GuildIntegrationsUpdate {
    pub guild_id: SnowflakeID,
}
