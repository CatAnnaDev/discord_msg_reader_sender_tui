use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct UserSettingsUpdateData {
    pub custom_status: String,
}
