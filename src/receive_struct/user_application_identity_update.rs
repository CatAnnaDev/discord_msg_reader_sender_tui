use crate::utils::SnowflakeID;
use serde_derive::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Debug)]
pub struct UserApplicationIdentityUpdate {
    pub username: String,
    pub user_id: SnowflakeID,
    pub metadata: Value,
    pub avatar_hash: Value,
    pub application_id: SnowflakeID,
}
