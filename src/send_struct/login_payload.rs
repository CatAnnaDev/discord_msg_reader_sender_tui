use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct Properties {
    pub os: String,
    pub browser: String,
    pub device: String,
    pub system_locale: String,
    pub browser_user_agent: String,
    pub browser_version: String,
    pub os_version: String,
    pub referrer: String,
    pub referring_domain: String,
    pub referrer_current: String,
    pub referring_domain_current: String,
    pub release_channel: String,
    pub client_build_number: u32,
    pub client_event_source: Option<Value>,
}

impl Properties {
    pub fn default_mac() -> Self {
        Self {
            os: "Mac OS X".to_string(),
            browser: "Chrome".to_string(),
            device: String::new(),
            system_locale: "en-US".to_string(),
            browser_user_agent:
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like \
                 Gecko) Chrome/131.0.0.0 Safari/537.36"
                    .to_string(),
            browser_version: "131.0.0.0".to_string(),
            os_version: "10.15.7".to_string(),
            referrer: String::new(),
            referring_domain: String::new(),
            referrer_current: String::new(),
            referring_domain_current: String::new(),
            release_channel: "stable".to_string(),
            client_build_number: 350_000,
            client_event_source: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Presence {
    pub activities: Vec<Value>,
    pub status: String,
    pub since: i64,
    pub afk: bool,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ClientState {
    pub guild_versions: serde_json::Map<String, Value>,
}

#[derive(Serialize, Deserialize)]
pub struct D {
    pub token: String,

    pub properties: Properties,
    pub presence: Presence,
    pub compress: bool,
    pub client_state: ClientState,
}

#[derive(Serialize, Deserialize)]
pub struct PayloadLogin {
    pub op: u8,
    pub d: D,
}
