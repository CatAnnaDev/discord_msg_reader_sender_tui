use comparable::Comparable;
use serde_derive::{Deserialize, Serialize};

use crate::utils::SnowflakeID;

#[derive(Serialize, Deserialize, Clone, Comparable)]
pub struct Config {
    pub dm_track: bool,
    pub server_track: bool,
    pub print_muted_dm: bool,
    pub message_buffer_size: usize,
    pub token: String,
    pub dm_channel_id_tracking: Vec<SnowflakeID>,
    pub track_myself: bool,
    #[serde(default)]
    pub event: Event,
    pub download_media: bool,
    #[serde(default)]
    pub write_file: WriteFile,
    pub debug: bool,

    #[serde(default)]
    #[comparable_ignore]
    pub voice_input_device: Option<String>,
    #[serde(default)]
    #[comparable_ignore]
    pub voice_output_device: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Comparable)]
pub struct Event {
    pub message_create: bool,
    pub message_delete: bool,
    pub session_replace: bool,
    pub presence_update: bool,
    pub user_settings_update: bool,
    pub call_dm_create: bool,
    pub call_dm_delete: bool,
    pub typing_start: bool,
    pub voice_state_update: bool,
    pub channel_update: bool,
}

impl Default for Event {
    fn default() -> Self {
        Self {
            message_create: true,
            message_delete: true,
            session_replace: true,
            presence_update: true,
            user_settings_update: true,
            call_dm_create: true,
            call_dm_delete: true,
            typing_start: false,
            voice_state_update: false,
            channel_update: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Comparable, Default)]
pub struct WriteFile {
    pub ready: bool,
    pub server_channel: bool,
    pub dm_channel: bool,
    pub todo_dump: bool,
    pub tracking: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dm_track: true,
            server_track: true,
            print_muted_dm: true,
            message_buffer_size: 500,
            token: "".to_string(),
            dm_channel_id_tracking: vec![],
            track_myself: true,
            event: Event::default(),
            download_media: false,
            write_file: Default::default(),
            debug: false,
            voice_input_device: None,
            voice_output_device: None,
        }
    }
}
