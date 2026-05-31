use crate::utils::SnowflakeID;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct MessageAck {
    pub version: i64,
    pub message_id: SnowflakeID,
    pub last_viewed: i64,
    pub flags: i64,
    pub channel_id: SnowflakeID,
}
