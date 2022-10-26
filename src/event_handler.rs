//! Handle docker daemon events

use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use docker_api::{models::EventMessage, opts::EventsOpts, Docker};
use futures_util::StreamExt;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::container_manager::{ContainerManager, Health};

/// Handler for docker daemon events
pub struct EventHandler {
    /// Docker daemon interface
    docker: Docker,

    /// Reference to the container manager to which container updates are to be reported
    container_manager: Arc<RwLock<ContainerManager>>,
}

impl EventHandler {
    /// Create a new docker event handler
    pub fn new(docker: Docker, container_manager: Arc<RwLock<ContainerManager>>) -> Self {
        Self {
            docker,
            container_manager,
        }
    }

    /// Subscribe to the docker event stream and handle all events
    pub async fn handle_events(&mut self) {
        let mut stream = self.docker.events(&EventsOpts::default());
        while let Some(event) = stream.next().await {
            if let Err(err) = async {
                self.handle_event(event.context("could not get event data")?)
                    .await
                    .context("could not handle event")
            }
            .await
            {
                error!("{err:#}");
            }
        }
    }

    /// Handle a event from the docker daemon
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
        self.container_manager
            .write()
            .await
            .container_started(id)
            .await?;
        Ok(())
    }

    /// Handle a container die event
    async fn handle_container_die(&self, event: EventMessage) -> Result<()> {
        let id = get_container_id(&event)?;
        info!("container died: {:?}", id);
        self.container_manager
            .write()
            .await
            .container_died(id)
            .await?;
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
            .write()
            .await
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
