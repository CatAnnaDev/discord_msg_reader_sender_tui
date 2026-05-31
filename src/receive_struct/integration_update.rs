use serde_derive::Deserialize;
use serde_json::Value;

use crate::utils::SnowflakeID;

#[derive(Deserialize, Debug)]
pub struct IntegrationUpdate {
    pub id: SnowflakeID,
    pub guild_id: SnowflakeID,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub enabled: Option<bool>,
    pub syncing: Option<bool>,
    pub role_id: Option<SnowflakeID>,
    pub enable_emoticons: Option<bool>,
    pub expire_behavior: Option<i64>,
    pub expire_grace_period: Option<i64>,
    pub user: Option<Value>,
    pub account: Option<Account>,
    pub synced_at: Option<String>,
    pub subscriber_count: Option<i64>,
    pub revoked: Option<bool>,
    pub application: Option<Value>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct Account {
    pub id: String,
    pub name: String,
}
