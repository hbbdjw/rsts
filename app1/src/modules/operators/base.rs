use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
pub trait Operator: Debug + Send + Sync {
    async fn execute(&self) -> Result<(), anyhow::Error>;
}
