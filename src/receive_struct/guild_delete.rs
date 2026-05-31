use crate::utils::SnowflakeID;
use serde::Deserialize;
use serde_derive::Serialize;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct GuildDelete {
    pub id: SnowflakeID,
}
