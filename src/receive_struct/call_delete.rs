use serde_derive::{Deserialize, Serialize};

use crate::utils::SnowflakeID;

#[derive(Serialize, Deserialize, Debug)]
pub struct CallDeleteData {
    pub channel_id: SnowflakeID,
}
