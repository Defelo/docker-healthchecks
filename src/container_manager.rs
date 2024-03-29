//! Manage monitored docker containers

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::{anyhow, bail, Context, Result};
use docker_api::{models::ContainerInspect200Response, opts::ContainerListOpts, Docker};
use futures_util::future::join_all;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::healthchecks::Healthchecks;

/// Docker container health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Health {
    /// Healthy indicates that the container is running correctly
    Healthy,

    /// Unhealthy indicates that the container has a problem
    Unhealthy,

    /// Starting indicates that the container is not yet ready
    Starting,
}

/// Monitored docker container
#[derive(Debug)]
struct Container {
    /// healthchecks url of the container
    ping_url: String,

    /// health status of the container (`None` if the container has no
    /// healthcheck)
    health: Option<Health>,
}

/// Stores monitored and ignored containers
struct ManagedContainers {
    /// Mapping from container id to container data for monitored containers
    monitored_containers: HashMap<String, Container>,

    /// Set of containers without healthchecks label. These containers can be
    /// safely ignored, as it is not possible to add labels to running
    /// containers.
    ignored_containers: HashSet<String>,
}

/// Manager for monitored docker containers
pub struct ContainerManager {
    /// Docker daemon interface
    docker: Docker,

    /// RwLocked monitored and ignored containers
    containers: Arc<RwLock<ManagedContainers>>,

    /// Healthchecks.io interface
    healthchecks: Healthchecks,
}

impl ContainerManager {
    /// Create a new container manager
    pub fn new(docker: Docker, healthchecks: Healthchecks) -> Self {
        Self {
            docker,
            containers: Arc::new(RwLock::new(ManagedContainers {
                monitored_containers: HashMap::new(),
                ignored_containers: HashSet::new(),
            })),
            healthchecks,
        }
    }

    /// Ping the healthcheck urls of all monitored containers
    pub async fn ping_healthchecks(&self) {
        info!("pinging healthchecks");
        join_all(
            self.get_status_map()
                .await
                .iter()
                .map(|(label, health)| async {
                    if let Err(err) = self
                        .healthchecks
                        .ping(label, health)
                        .await
                        .context("failed to ping healthchecks")
                    {
                        error!("{err:#}");
                    }
                }),
        )
        .await;
    }

    /// Reload all docker containers from the daemon
    pub async fn fetch_containers(&self) -> Result<()> {
        info!("fetching containers");
        let mut containers = HashMap::new();
        let mut ignored_containers = HashSet::new();
        for summary in self
            .docker
            .containers()
            .list(&ContainerListOpts::default())
            .await
            .context("failed to list containers")?
        {
            let id = summary
                .id
                .ok_or_else(|| anyhow!("container summary has no id"))?;
            if let Some(container) = self
                .fetch_container(&id)
                .await
                .context("failed to fetch container")?
            {
                containers.insert(id, container);
            } else {
                ignored_containers.insert(id);
            }
        }
        let mut cont = self.containers.write().await;
        cont.monitored_containers = containers;
        cont.ignored_containers = ignored_containers;
        Ok(())
    }

    /// Handle container start events
    pub async fn container_started(&self, id: String) -> Result<()> {
        // ignore containers without healthchecks label
        if self
            .containers
            .read()
            .await
            .ignored_containers
            .contains(&id)
        {
            return Ok(());
        }

        // try to get information about the new container
        if let Some(container) = self.fetch_container(&id).await? {
            // add the container to the collection of monitored containers
            let label = container.ping_url.clone();
            self.containers
                .write()
                .await
                .monitored_containers
                .insert(id, container);

            // send a ping to the corresponding ping url
            self.ping_one(&label).await?;
        } else {
            // ignore the container if it has no healthchecks label
            self.containers.write().await.ignored_containers.insert(id);
        }

        Ok(())
    }

    /// Handle container die events
    pub async fn container_died(&self, id: &String) -> Result<()> {
        // ignore containers without healthchecks label and remove them from the set of
        // ignored containers
        if self.containers.write().await.ignored_containers.remove(id) {
            return Ok(());
        }

        // remove the container from the collection of monitored containers
        let mut containers = self.containers.write().await;
        if let Some(container) = containers.monitored_containers.remove(id) {
            drop(containers);

            // send an unhealthy ping to the corresponding ping url,
            // if this was the last container with this ping url
            if !self
                .get_status_map()
                .await
                .contains_key(&container.ping_url)
            {
                self.healthchecks
                    .ping(&container.ping_url, &Health::Unhealthy)
                    .await?;
            }
        }
        Ok(())
    }

    /// Handle container health update events
    pub async fn container_health_update(&self, id: String, health: Health) -> Result<()> {
        // ignore containers without healthchecks label
        if self
            .containers
            .read()
            .await
            .ignored_containers
            .contains(&id)
        {
            return Ok(());
        }

        // try to find the container in the collection of monitored containers,
        // otherwise fetch its data from the docker daemon
        let mut containers = self.containers.write().await;
        let label = if let Some(container) = containers.monitored_containers.get_mut(&id) {
            // update the health status
            container.health = Some(health);
            container.ping_url.clone()
        } else if let Some(container) = self.fetch_container(&id).await? {
            // add the container to the collection of monitored containers
            let label = container.ping_url.clone();
            containers.monitored_containers.insert(id, container);
            label
        } else {
            // ignore the container if it has no healthchecks label
            containers.ignored_containers.insert(id);
            return Ok(());
        };
        drop(containers);

        // send a ping to the corresponding ping url
        self.ping_one(&label).await
    }

    /// Return a mapping from ping urls to their current health status
    /// If there are multiple containers with the same ping url,
    /// the 'worst' health status is used.
    async fn get_status_map(&self) -> HashMap<String, Health> {
        let mut status = HashMap::new();
        for container in self.containers.read().await.monitored_containers.values() {
            let health: Health = container.health.unwrap_or(Health::Healthy);
            if let Some(h) = status.get_mut(&container.ping_url) {
                // another container with the same ping url already exists
                // update the health status if the health status of the current containers is
                // 'worse'
                if health > *h {
                    *h = health;
                }
            } else {
                // this is the first container with this ping url
                status.insert(container.ping_url.clone(), health);
            }
        }
        status
    }

    /// Ping one url
    async fn ping_one(&self, ping_url: &String) -> Result<()> {
        let health = self
            .get_status_map()
            .await
            .remove(ping_url)
            .unwrap_or(Health::Unhealthy);
        self.healthchecks.ping(ping_url, &health).await
    }

    /// Fetch information about a container from the docker daemon.
    /// Returns `None` if the container has no `healthchecks.url` label.
    async fn fetch_container(&self, id: &str) -> Result<Option<Container>> {
        let data = self
            .docker
            .containers()
            .get(id)
            .inspect()
            .await
            .with_context(|| format!("failed to inspect container {id}"))?;

        if let Some(label) = get_label(&data).context("failed to get label of container")? {
            Ok(Some(Container {
                ping_url: label,
                health: get_health(&data).context("failed to get health status of container")?,
            }))
        } else {
            Ok(None)
        }
    }
}

/// Extract the health status from a container inspect response
fn get_health(data: &ContainerInspect200Response) -> Result<Option<Health>> {
    let status = data
        .state
        .as_ref()
        .ok_or_else(|| anyhow!("container inspect state object is empty"))?
        .health
        .as_ref()
        .and_then(|health| health.status.as_ref())
        .map(std::string::String::as_str);

    Ok(match status {
        None | Some("none") => None,
        Some("starting") => Some(Health::Starting),
        Some("healthy") => Some(Health::Healthy),
        Some("unhealthy") => Some(Health::Unhealthy),
        Some(status) => bail!("invalid health status: {status}"),
    })
}

/// Extract the `healthchecks.url` label from a container inspect response
fn get_label(data: &ContainerInspect200Response) -> Result<Option<String>> {
    let labels = data
        .config
        .as_ref()
        .ok_or_else(|| anyhow!("container inspect config object is empty"))?
        .labels
        .as_ref()
        .ok_or_else(|| anyhow!("container inspect config labels object is empty"))?;
    Ok(labels.get("healthchecks.url").cloned())
}
