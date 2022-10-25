use std::{collections::HashSet, time::Duration};

use anyhow::Result;
use reqwest::{Client, IntoUrl};
use tokio::time::sleep;
use tracing::{debug, warn};

use crate::containers::Health;

pub struct Healthchecks {
    ping_retries: u8,
    starting: HashSet<String>,
}

impl Healthchecks {
    pub fn new(ping_retries: u8) -> Self {
        Self {
            ping_retries,
            starting: HashSet::new(),
        }
    }

    pub async fn ping(&mut self, url: &str, health: &Health) -> Result<()> {
        if self.starting.contains(url) {
            if health == &Health::Starting {
                debug!("not sending another starting ping to healthchecks for {url}");
                return Ok(());
            }
            self.starting.remove(url);
        } else if health == &Health::Starting {
            self.starting.insert(url.to_owned());
        }

        debug!("sending {health:?} ping to healthchecks for {url}");

        let url = match health {
            Health::Healthy => url.to_owned(),
            Health::Unhealthy => format!("{url}/fail"),
            Health::Starting => format!("{url}/start"),
        };

        let mut retries = self.ping_retries;
        while let Err(err) = try_ping(&url).await {
            retries -= 1;
            if retries == 0 {
                return Err(err.context(format!("healthchecks ping to {url} failed")));
            }
            warn!("healthchecks ping to {url} failed, retrying...");
            sleep(Duration::from_secs(2)).await;
        }

        Ok(())
    }
}

async fn try_ping(url: &impl IntoUrl) -> Result<()> {
    Client::new()
        .post(url.as_str())
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
