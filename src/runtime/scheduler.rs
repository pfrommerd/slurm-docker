use super::host::{Host, HostConfig, HostInfo};
use crate::Result;

pub trait Scheduler: Send + Sync {
    fn id(&self) -> &str;
    fn hosts(&self) -> Result<Vec<HostInfo>>;
    fn create_host(&self, config: HostConfig) -> Result<Box<dyn Host>>;
}
