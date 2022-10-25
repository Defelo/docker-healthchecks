use anyhow::{anyhow, bail, Context, Result};
use docker_api::{models::ContainerInspect200Response, opts::ContainerListOpts, Docker};
use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{debug, error, info};

use crate::monitoring::Monitoring;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Health {
    Healthy,
    Unhealthy,
    Starting,
}

#[derive(Debug)]
pub struct Container {
    label: String,
    health: Option<Health>,
}

pub struct Containers {
    docker: Docker,
    containers: HashMap<String, Container>,
    ignored_containers: HashSet<String>,
    last_ping: SystemTime,
    ping_interval: u64,
    last_fetch: SystemTime,
    fetch_interval: u64,
    monitoring: Box<dyn Monitoring>,
}

impl Containers {
    pub fn new(
        docker: Docker,
        ping_interval: u64,
        fetch_interval: u64,
        monitoring: Box<dyn Monitoring>,
    ) -> Self {
        Self {
            docker,
            containers: HashMap::new(),
            ignored_containers: HashSet::new(),
            last_ping: UNIX_EPOCH,
            ping_interval,
            last_fetch: UNIX_EPOCH,
            fetch_interval,
            monitoring,
        }
    }

    pub async fn tick(&mut self) -> Result<()> {
        if SystemTime::now().duration_since(self.last_ping)?.as_secs() > self.ping_interval {
            info!("pinging monitoring");
            if let Err(err) = self.ping().await.context("failed to ping monitoring") {
                error!("{err:#}");
            }
            self.last_ping = SystemTime::now();
        }
        if SystemTime::now().duration_since(self.last_fetch)?.as_secs() > self.fetch_interval {
            info!("fetching containers");
            if let Err(err) = self.fetch().await.context("failed to fetch containers") {
                error!("{err:#}");
            }
            self.last_fetch = SystemTime::now();
        }
        debug!("{} containers running", self.containers.len());
        for container in &self.containers {
            debug!("  - {container:?}");
        }
        Ok(())
    }

    fn get_status_map(&self) -> HashMap<String, Health> {
        let mut status = HashMap::new();
        for container in self.containers.values() {
            let health: Health = container.health.clone().unwrap_or(Health::Healthy);
            if let Some(h) = status.get_mut(&container.label) {
                if health > *h {
                    *h = health;
                }
            } else {
                status.insert(container.label.clone(), health);
            }
        }
        status
    }

    async fn ping(&mut self) -> Result<()> {
        for (label, health) in self.get_status_map() {
            self.monitoring.ping(&label, &health).await?;
        }
        Ok(())
    }

    async fn fetch(&mut self) -> Result<()> {
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
        self.containers = containers;
        self.ignored_containers = ignored_containers;
        Ok(())
    }

    async fn fetch_container(&mut self, id: &str) -> Result<Option<Container>> {
        let data = self
            .docker
            .containers()
            .get(id)
            .inspect()
            .await
            .with_context(|| format!("failed to inspect container {id}"))?;
        Ok(
            if let Some(label) = get_label(&data).context("failed to get label of container")? {
                Some(Container {
                    label,
                    health: get_health(&data)
                        .context("failed to get health status of container")?,
                })
            } else {
                None
            },
        )
    }

    async fn ping_label(&mut self, label: &String) -> Result<()> {
        let health = self
            .get_status_map()
            .remove(label)
            .unwrap_or(Health::Unhealthy);
        self.monitoring.ping(label, &health).await
    }

    pub async fn container_started(&mut self, id: String) -> Result<()> {
        if self.ignored_containers.contains(&id) {
            return Ok(());
        }
        if let Some(container) = self.fetch_container(&id).await? {
            let label = container.label.clone();
            self.containers.insert(id, container);
            self.ping_label(&label).await?;
        } else {
            self.ignored_containers.insert(id);
        }
        Ok(())
    }

    pub async fn container_died(&mut self, id: &String) -> Result<()> {
        if self.ignored_containers.remove(id) {
            return Ok(());
        }
        if let Some(container) = self.containers.remove(id) {
            if !self.get_status_map().contains_key(&container.label) {
                self.monitoring
                    .ping(&container.label, &Health::Unhealthy)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn container_health_update(&mut self, id: String, health: Health) -> Result<()> {
        if self.ignored_containers.contains(&id) {
            return Ok(());
        }
        let label = if let Some(container) = self.containers.get_mut(&id) {
            container.health = Some(health);
            container.label.clone()
        } else if let Some(container) = self.fetch_container(&id).await? {
            let label = container.label.clone();
            self.containers.insert(id, container);
            label
        } else {
            self.ignored_containers.insert(id);
            return Ok(());
        };
        self.ping_label(&label).await?;
        Ok(())
    }
}

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

fn get_label(data: &ContainerInspect200Response) -> Result<Option<String>> {
    let labels = data
        .config
        .as_ref()
        .ok_or_else(|| anyhow!("container inspect config object is empty"))?
        .labels
        .as_ref()
        .ok_or_else(|| anyhow!("container inspect config labels object is empty"))?;
    Ok(labels.get("healthchecks.id").cloned())
}
