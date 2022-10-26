//! Healthchecks.io interface

use std::{collections::HashSet, time::Duration};

use anyhow::Result;
use reqwest::{Client, IntoUrl};
use tokio::{sync::RwLock, time::sleep};
use tracing::{debug, warn};

use crate::container_manager::Health;

/// Healthchecks.io interface
pub struct Healthchecks {
    /// Number of retries for failed healthcheck pings
    ping_retries: u8,

    /// Set of ping urls that last received a starting ping
    starting: RwLock<HashSet<String>>,
}

impl Healthchecks {
    /// Create a new Healthchecks.io interface
    pub fn new(ping_retries: u8) -> Self {
        Self {
            ping_retries,
            starting: RwLock::new(HashSet::new()),
        }
    }

    /// Ping a given healthchecks url
    pub async fn ping(&self, url: &str, health: &Health) -> Result<()> {
        // avoid sending multiple consecutive starting pings to the same url
        let mut starting = self.starting.write().await;
        if starting.contains(url) {
            if health == &Health::Starting {
                debug!("not sending another starting ping to healthchecks for {url}");
                return Ok(());
            }
            starting.remove(url);
        } else if health == &Health::Starting {
            starting.insert(url.to_owned());
        }
        drop(starting);

        debug!("sending {health:?} ping to healthchecks for {url}");

        // create url from given health status
        let url = match health {
            Health::Healthy => url.to_owned(),
            Health::Unhealthy => format!("{url}/fail"),
            Health::Starting => format!("{url}/start"),
        };

        // send the ping and retry if it fails
        let mut retries = self.ping_retries;
        while let Err(err) = try_ping(&url).await {
            if retries == 0 {
                // return the last error if all retries are exhausted
                return Err(err.context(format!("healthchecks ping to {url} failed")));
            }
            retries -= 1;
            warn!("healthchecks ping to {url} failed, retrying...");
            sleep(Duration::from_secs(2)).await;
        }

        Ok(())
    }
}

/// Send a post request to the given url
async fn try_ping(url: &impl IntoUrl) -> Result<()> {
    Client::new()
        .post(url.as_str())
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
