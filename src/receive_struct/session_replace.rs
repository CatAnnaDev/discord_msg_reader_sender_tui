use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientInfo {
    pub version: i64,
    pub os: String,
    pub client: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SessionReplaceData {
    pub status: String,
    pub session_id: String,
    pub client_info: ClientInfo,
    pub activities: Vec<Activities>,
    pub active: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Activities {
    #[serde(rename = "type")]
    pub a_type: i64,
    pub state: String,
    pub name: String,
    pub id: String,
    pub emoji: Option<Emoji>,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Emoji {
    pub name: String,
}
