#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::dbg_macro, clippy::use_debug)]
#![warn(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![warn(
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::wildcard_enum_match_arm
)]
#![allow(clippy::module_name_repetitions)]

use std::{sync::Arc, time::Duration};

use anyhow::{ensure, Context, Result};
use docker_api::Docker;
use tokio::{
    spawn,
    sync::RwLock,
    time::{interval, sleep},
};
use tracing::{debug, error};

use self::{containers::Containers, events::Events, healthchecks::Healthchecks};

mod config;
mod containers;
mod events;
mod healthchecks;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = config::load().context("could not load environment variables")?;
    ensure!(
        config.ping_interval >= 1,
        "ping_interval must be at least one second"
    );
    ensure!(
        config.fetch_interval >= 1,
        "fetch_interval must be at least one second"
    );

    let docker = Docker::unix(&config.docker_path);
    debug!(
        "connected to docker: {:?}",
        docker
            .ping()
            .await
            .context("could not ping docker daemon")?
    );

    let mut containers = Containers::new(docker.clone(), Healthchecks::new(config.ping_retries));
    containers.fetch_containers().await?;

    let containers = Arc::new(RwLock::new(containers));
    let mut events = Events::new(docker, containers.clone());

    spawn(async move {
        events.handle_events().await;
    });

    let cont = containers.clone();
    spawn(async move {
        let duration = Duration::from_secs(config.fetch_interval);
        loop {
            sleep(duration).await;
            if let Err(err) = cont
                .write()
                .await
                .fetch_containers()
                .await
                .context("failed to fetch containers")
            {
                error!("{err:#}");
            }
        }
    });

    let mut interval = interval(Duration::from_secs(config.ping_interval));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        if let Err(err) = containers
            .write()
            .await
            .ping_healthchecks()
            .await
            .context("failed to ping healthchecks")
        {
            error!("{err:#}");
        }
    }
}
