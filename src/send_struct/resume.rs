use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DiscordResumeConnection {
    pub op: i8,

    pub d: DiscordResumeData,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordResumeData {
    pub token: String,
    pub session_id: String,
    pub seq: u64,
}
