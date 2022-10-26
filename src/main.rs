//! Healthchecks.io Integration for Docker Healthchecks

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
    clippy::missing_docs_in_private_items,
    clippy::self_named_module_files,
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::wildcard_enum_match_arm
)]

use std::{sync::Arc, time::Duration};

use anyhow::{ensure, Context, Result};
use docker_api::Docker;
use tokio::{
    spawn,
    sync::RwLock,
    time::{interval, sleep, timeout},
};
use tracing::{debug, error};

use self::{
    container_manager::ContainerManager, event_handler::EventHandler, healthchecks::Healthchecks,
};

mod config;
mod container_manager;
mod event_handler;
mod healthchecks;

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing subscriber
    tracing_subscriber::fmt::init();

    // load config from environment variables
    let config = config::load().context("could not load environment variables")?;
    ensure!(
        config.ping_interval >= 1,
        "ping_interval must be at least one second"
    );
    ensure!(
        config.fetch_interval >= 1,
        "fetch_interval must be at least one second"
    );

    // connect to docker daemon
    let docker = Docker::unix(&config.docker_path);
    debug!(
        "connected to docker: {:?}",
        docker
            .ping()
            .await
            .context("could not ping docker daemon")?
    );

    // create container manager and load container list from docker daemon
    let mut containers =
        ContainerManager::new(docker.clone(), Healthchecks::new(config.ping_retries));
    containers.fetch_containers().await?;

    // create event handler
    let containers = Arc::new(RwLock::new(containers));
    let events = EventHandler::new(containers.clone());

    // handle docker events in a new task
    spawn(async move {
        events
            .handle_events(docker, Duration::from_secs(config.event_timeout))
            .await;
    });

    // periodically refresh docker container list in case we miss some events
    let cont = containers.clone();
    spawn(async move {
        let duration = Duration::from_secs(config.fetch_interval);
        loop {
            sleep(duration).await;
            if let Err(err) = timeout(Duration::from_secs(config.fetch_timeout), async {
                cont.write()
                    .await
                    .fetch_containers()
                    .await
                    .context("failed to fetch containers")
            })
            .await
            .context("failed to fetch containers in time")
            .and_then(|res| res)
            {
                error!("{err:#}");
            }
        }
    });

    // periodically ping the healthcheck urls of the monitored containers
    let mut interval = interval(Duration::from_secs(config.ping_interval));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        if let Err(err) = timeout(Duration::from_secs(config.ping_timeout), async {
            containers.write().await.ping_healthchecks().await;
        })
        .await
        .context("failed to ping healthchecks in time")
        {
            error!("{err:#}");
        }
    }
}
