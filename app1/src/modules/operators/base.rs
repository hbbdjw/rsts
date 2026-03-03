use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
#[allow(dead_code)]
pub trait Operator: Debug + Send + Sync {
    async fn execute(&self) -> Result<(), anyhow::Error>;
}
