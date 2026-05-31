use serde_derive::Deserialize;

use crate::utils::SnowflakeID;

#[derive(Deserialize, Debug)]
pub struct ChannelUnreadUpdate {
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub channel_unread_updates: Vec<ChannelUnreadEntry>,
}

#[derive(Deserialize, Debug)]
pub struct ChannelUnreadEntry {
    pub id: SnowflakeID,
    pub last_message_id: Option<SnowflakeID>,
}
