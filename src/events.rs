use anyhow::{anyhow, bail, Result};
use std::sync::Arc;

use docker_api::{models::EventMessage, opts::EventsOpts, Docker};
use futures_util::StreamExt;
use tokio::sync::RwLock;
use tracing::{debug, error};

use crate::containers::{Containers, Health};

pub struct Events {
    docker: Docker,
    containers: Arc<RwLock<Containers>>,
}

impl Events {
    pub fn new(docker: Docker, containers: Arc<RwLock<Containers>>) -> Self {
        Self { docker, containers }
    }

    async fn handle_container_start(&self, event: EventMessage) -> Result<()> {
        let id = get_container_id(&event)?.clone();
        debug!("container started: {:?}", id);
        self.containers.write().await.container_started(id).await?;
        Ok(())
    }

    async fn handle_container_die(&self, event: EventMessage) -> Result<()> {
        let id = get_container_id(&event)?;
        debug!("container died: {:?}", id);
        self.containers.write().await.container_died(id).await?;
        Ok(())
    }

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
        debug!("health status update: {} {:?}", id, status);
        self.containers
            .write()
            .await
            .container_health_update(id, status)
            .await?;
        Ok(())
    }

    async fn handle_event(&self, event: EventMessage) -> Result<()> {
        match (event.type_.as_deref(), event.action.as_deref()) {
            (Some("container"), Some("start")) => self.handle_container_start(event).await,
            (Some("container"), Some("die")) => self.handle_container_die(event).await,
            (Some("container"), Some(action)) => {
                if let Some(status) = action.to_owned().strip_prefix("health_status: ") {
                    self.handle_container_health_status(event, status).await
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    pub async fn handle_events(&mut self) {
        let mut stream = self.docker.events(&EventsOpts::default());
        while let Some(event) = stream.next().await {
            let event = match event {
                Ok(e) => e,
                Err(err) => {
                    error!("error: {err}");
                    continue;
                }
            };

            if let Err(err) = self.handle_event(event).await {
                error!("could not handle event: {err}");
            }
        }
    }
}

fn get_container_id(event: &EventMessage) -> Result<&String> {
    event
        .actor
        .as_ref()
        .ok_or_else(|| anyhow!("event has no actor"))?
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("event actor is empty"))
}
