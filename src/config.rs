use std::error::Error;
use std::path::Path;

use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::config::config::Config;
use crate::error;

pub mod config;

pub async fn make_config_file<P: AsRef<Path> + Copy>(path: P) -> Result<Config, Box<dyn Error>> {
    match fs::read_to_string(path).await {
        Ok(file) => load_config_file(&file),
        Err(_) => {
            let json = serde_json::to_string(&Config::default())?;
            let mut file = File::create(&path).await?;
            file.write_all(json.as_bytes()).await?;
            file.flush().await?;
            error!("Setup config file");
            load_config_file(&json)
        }
    }
}

fn load_config_file(file: &str) -> Result<Config, Box<dyn Error>> {
    serde_json::from_str::<Config>(file).map_err(|e| {
        error!("Failed to parse config.json: {}", e);
        e.into()
    })
}
