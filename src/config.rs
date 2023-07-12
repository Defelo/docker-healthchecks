//! Global configuration from environment variables

use anyhow::Result;
use serde::Deserialize;

/// Values from environment variables
#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    /// Path of the docker daemon socket
    pub docker_path: String,

    /// Number of seconds between healthcheck pings
    pub ping_interval: u64,

    /// Number of retries for failed healthcheck pings
    pub ping_retries: u8,

    /// Number of seconds after which the ping timeout expires
    pub ping_timeout: u64,

    /// Number of seconds between reloading the full container list from the
    /// docker daemon
    pub fetch_interval: u64,

    /// Number of seconds after which the container fetch timeout expires
    pub fetch_timeout: u64,

    /// Number of seconds after which the timeout for handling a docker event
    /// expires
    pub event_timeout: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            docker_path: "/var/run/docker.sock".to_owned(),
            ping_interval: 60,
            ping_retries: 5,
            ping_timeout: 50,
            fetch_interval: 600,
            fetch_timeout: 300,
            event_timeout: 60,
        }
    }
}

/// load configuration from environment variables
pub fn load() -> Result<Config> {
    Ok(config::Config::builder()
        .add_source(config::Environment::default())
        .build()?
        .try_deserialize()?)
}
