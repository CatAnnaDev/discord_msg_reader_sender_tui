use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Struct {
    #[serde(rename = "type")]
    pub r#type: String,
    pub rid: String,
    pub quality: i64,
}

#[derive(Serialize, Deserialize)]
pub struct StartVideoData {
    pub server_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
    pub max_secure_frames_version: i64,
    pub video: bool,
    pub streams: Vec<Struct>,
}

#[derive(Serialize, Deserialize)]
pub struct StartVideo {
    pub op: i64,
    pub d: StartVideoData,
}

impl StartVideo {
    pub fn _new(serv_id: String, user_id: String, session_id: String, token: String) -> Self {
        Self {
            op: 0,
            d: StartVideoData {
                server_id: serv_id,
                user_id,
                session_id,
                token,
                max_secure_frames_version: 0,
                video: true,
                streams: vec![Struct {
                    r#type: "video".to_string(),
                    rid: "100".to_string(),
                    quality: 100,
                }],
            },
        }
    }
}
