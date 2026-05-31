use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Struct {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: i64,
    pub state: String,
    pub id: String,
    pub emoji: Emoji,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Emoji {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct D {
    pub since: i64,
    pub activities: Vec<Struct>,
    pub status: String,
    pub afk: bool,
}

#[derive(Serialize, Deserialize)]
pub struct PresenceUpdate {
    pub op: i64,

    pub d: D,
}
