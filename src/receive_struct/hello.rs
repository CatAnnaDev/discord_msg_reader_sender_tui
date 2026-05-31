use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct HelloData {
    pub heartbeat_interval: u64,
}
