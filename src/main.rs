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

use anyhow::{Context, Result};
use docker_api::Docker;
use tokio::{spawn, sync::RwLock, time::sleep};
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

    let docker = Docker::unix(&config.docker_path);
    debug!(
        "connected to docker: {:?}",
        docker
            .ping()
            .await
            .context("could not ping docker daemon")?
    );

    let healthchecks = Healthchecks::new(config.ping_retries);
    let containers = Arc::new(RwLock::new(Containers::new(
        docker.clone(),
        config.ping_interval,
        config.fetch_interval,
        healthchecks,
    )));
    let mut events = Events::new(docker, containers.clone());

    spawn(async move {
        events.handle_events().await;
    });

    loop {
        if let Err(err) = containers.write().await.tick().await.context("tick failed") {
            error!("{err:#}");
        }
        sleep(Duration::from_millis(1000)).await;
    }
}
