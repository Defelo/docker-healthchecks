use anyhow::Result;
use serde::Deserialize;
use url::Url;

#[derive(Deserialize)]
pub struct Config {
    pub ping_interval: u64,
    pub ping_retries: u8,
    pub fetch_interval: u64,
    pub healthchecks_url: Url,
    pub docker_path: String,
}

pub fn load() -> Result<Config> {
    Ok(config::Config::builder()
        .add_source(config::Environment::default())
        .build()?
        .try_deserialize()?)
}
