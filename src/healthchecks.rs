use std::{collections::HashSet, time::Duration};

use anyhow::{Context, Result};
use reqwest::Client;
use tokio::time::sleep;
use tracing::{debug, warn};
use url::Url;

use crate::containers::Health;

pub struct Healthchecks {
    url: Url,
    ping_retries: u8,
    starting: HashSet<String>,
}

impl Healthchecks {
    pub fn new(url: Url, ping_retries: u8) -> Self {
        Self {
            url,
            ping_retries,
            starting: HashSet::new(),
        }
    }

    pub async fn ping(&mut self, id: &str, health: &Health) -> Result<()> {
        if self.starting.contains(id) {
            if health == &Health::Starting {
                debug!("not sending another starting ping to healthchecks for {id}");
                return Ok(());
            }
            self.starting.remove(id);
        } else if health == &Health::Starting {
            self.starting.insert(id.to_owned());
        }

        debug!("sending {health:?} ping to healthchecks for {id}");

        let url = match health {
            Health::Healthy => self.url.join(id),
            Health::Unhealthy => self.url.join(&format!("{id}/fail")),
            Health::Starting => self.url.join(&format!("{id}/start")),
        }
        .context("could not build healthchecks ping url for {health:?} {id}")?;

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

async fn try_ping(url: &Url) -> Result<()> {
    Client::new()
        .post(url.as_str())
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
