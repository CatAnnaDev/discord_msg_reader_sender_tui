use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ForceJoinD {
    pub guild_id: String,
    pub channel_id: String,
    pub self_mute: bool,
    pub self_deaf: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ForceJoinVocal {
    pub op: i64,

    pub d: ForceJoinD,
}
