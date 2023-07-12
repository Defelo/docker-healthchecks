//! Handle docker daemon events

use std::{sync::Arc, time::Duration};

use anyhow::{anyhow, bail, Context, Result};
use docker_api::{models::EventMessage, opts::EventsOpts, Docker};
use futures_util::StreamExt;
use tokio::{spawn, time::timeout};
use tracing::{error, info};

use crate::container_manager::{ContainerManager, Health};

/// Handler for docker daemon events
pub struct EventHandler {
    /// Reference to the container manager to which container updates are to be
    /// reported
    container_manager: Arc<ContainerManager>,
}

impl EventHandler {
    /// Create a new docker event handler
    pub fn new(container_manager: Arc<ContainerManager>) -> Self {
        Self { container_manager }
    }

    /// Subscribe to the docker event stream and handle all events
    pub async fn handle_events(self, docker: Docker, timeout_duration: Duration) -> ! {
        let handler = Arc::new(self);
        loop {
            info!("subscribing to docker event stream");
            let mut stream = docker.events(&EventsOpts::default());
            while let Some(event) = stream.next().await {
                spawn(Self::handle_raw_event(
                    handler.clone(),
                    event,
                    timeout_duration,
                ));
            }
        }
    }

    /// Handle a raw event from the docker event stream
    async fn handle_raw_event(
        handler: Arc<Self>,
        event: docker_api::Result<EventMessage>,
        timeout_duration: Duration,
    ) {
        if let Err(err) = timeout(timeout_duration, async {
            handler
                .handle_event(event.context("could not get event data")?)
                .await
                .context("could not handle event")
        })
        .await
        .context("failed to handle event in time")
        .and_then(|res| res)
        {
            error!("{err:#}");
        }
    }

    /// Handle an event from the docker daemon
    async fn handle_event(&self, event: EventMessage) -> Result<()> {
        match (event.type_.as_deref(), event.action.as_deref()) {
            // container start
            (Some("container"), Some("start")) => self.handle_container_start(event).await,

            // container die
            (Some("container"), Some("die")) => self.handle_container_die(event).await,

            // container health update
            (Some("container"), Some(action)) => {
                if let Some(status) = action.to_owned().strip_prefix("health_status: ") {
                    self.handle_container_health_status(event, status).await
                } else {
                    Ok(())
                }
            }

            // ignore all other events
            _ => Ok(()),
        }
    }

    /// Handle a container start event
    async fn handle_container_start(&self, event: EventMessage) -> Result<()> {
        let id = get_container_id(&event)?.clone();
        info!("container started: {:?}", id);
        self.container_manager.container_started(id).await?;
        Ok(())
    }

    /// Handle a container die event
    async fn handle_container_die(&self, event: EventMessage) -> Result<()> {
        let id = get_container_id(&event)?;
        info!("container died: {:?}", id);
        self.container_manager.container_died(id).await?;
        Ok(())
    }

    /// Handle a container health update event
    async fn handle_container_health_status(
        &self,
        event: EventMessage,
        status: &str,
    ) -> Result<()> {
        let id = get_container_id(&event)?.clone();
        let status = match status {
            "healthy" => Health::Healthy,
            "unhealthy" => Health::Unhealthy,
            "starting" => Health::Starting,
            status => {
                bail!("container {} has invalid health status: {}", id, status);
            }
        };

        info!("health status update: {} {:?}", id, status);
        self.container_manager
            .container_health_update(id, status)
            .await?;
        Ok(())
    }
}

/// Extract the container id from a docker event
fn get_container_id(event: &EventMessage) -> Result<&String> {
    event
        .actor
        .as_ref()
        .ok_or_else(|| anyhow!("event has no actor"))?
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("event actor is empty"))
}
