use serde_derive::{Deserialize, Serialize};
use serde_json::Value;

use crate::utils::SnowflakeID;

#[derive(Serialize, Deserialize, Debug)]
pub struct VoiceStates {
    pub user_id: SnowflakeID,
    pub suppress: bool,
    pub session_id: String,
    pub self_video: bool,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub request_to_speak_timestamp: Option<Value>,
    pub mute: bool,
    pub deaf: bool,
    pub channel_id: SnowflakeID,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CallCreateData {
    pub voice_states: Vec<VoiceStates>,
    pub ringing: Vec<Value>,
    pub region: String,
    pub message_id: SnowflakeID,
    pub embedded_activities: Vec<Value>,
    pub channel_id: SnowflakeID,
}
