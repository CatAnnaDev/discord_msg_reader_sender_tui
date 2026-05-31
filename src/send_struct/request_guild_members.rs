use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RequestGuildD {
    pub guild_id: String,
    pub query: String,
    pub limit: i64,
}

#[derive(Serialize, Deserialize)]
pub struct RequestGuildMembers {
    pub op: i64,

    pub d: RequestGuildD,
}
