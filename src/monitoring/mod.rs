use anyhow::Result;
use async_trait::async_trait;

pub use healthchecks::Healthchecks;

use crate::containers::Health;

mod healthchecks;

#[async_trait]
pub trait Monitoring
where
    Self: Send + Sync,
{
    async fn ping(&mut self, id: &str, health: &Health) -> Result<()>;
}
