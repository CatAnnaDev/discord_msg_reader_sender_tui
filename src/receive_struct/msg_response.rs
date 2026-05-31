use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MsgResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: i64,
    pub content: String,
    pub channel_id: String,
    pub author: Author,
    pub attachments: Vec<Value>,
    pub embeds: Vec<Value>,
    pub mentions: Vec<Value>,
    pub mention_roles: Vec<Value>,
    pub pinned: bool,
    pub mention_everyone: bool,
    pub tts: bool,
    pub timestamp: String,
    pub edited_timestamp: Value,
    pub flags: i64,
    pub components: Vec<Value>,
    pub nonce: i64,
    pub referenced_message: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Author {
    pub id: String,
    pub username: String,
    pub avatar: String,
    pub discriminator: String,
    pub public_flags: i64,
    pub flags: i64,
    pub banner: Value,
    pub accent_color: i64,
    pub global_name: String,
    pub avatar_decoration_data: Value,
    pub banner_color: String,
}
