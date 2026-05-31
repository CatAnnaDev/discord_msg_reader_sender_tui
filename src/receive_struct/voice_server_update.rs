use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceServerUpdateData {
    pub token: String,
    #[serde(rename = "guild_id", default)]
    pub guild_id: Option<String>,
    pub endpoint: String,
}
