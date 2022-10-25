use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    pub docker_path: String,
    pub ping_interval: u64,
    pub ping_retries: u8,
    pub fetch_interval: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            docker_path: "/var/run/docker.sock".to_owned(),
            ping_interval: 60,
            ping_retries: 5,
            fetch_interval: 600,
        }
    }
}

pub fn load() -> Result<Config> {
    Ok(config::Config::builder()
        .add_source(config::Environment::default())
        .build()?
        .try_deserialize()?)
}
