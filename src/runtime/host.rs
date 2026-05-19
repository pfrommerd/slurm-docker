use super::{Layer, ProcessExit, ProcessOutput, Command};
use crate::spec::Instruction;

use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::sync::Arc;
use std::collections::BTreeMap;
use uuid::Uuid;

pub type ResourceId = Arc<String>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resource {
    pub id: ResourceId,
    pub amount: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub resources: BTreeMap<ResourceId, Resource>,
}

// A configuration for launching a host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostConfig {
    pub name: Option<String>,
    pub partition: Option<String>,
    pub resources: Resources,
    pub time_limit: Option<Duration>,
}

// Information about a host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub id: Uuid,
    pub scheduler_id: Uuid,
    pub name: Option<String>,
    pub resources: Resources,
    pub time_limit: Option<Duration>,
}

pub trait Host: Send + Sync {
    fn id(&self) -> Uuid;
    fn name(&self) -> Option<&str>;
    fn resources(&self) -> &Resources;
    fn time_limit(&self) -> Option<Duration>;
    async fn launch_container(&self, layer: Layer, resources: &Resources) -> Result<Box<dyn Container>>;
    async fn launch_instruction(
        &self, instruction: &Instruction,
        layer: Layer, resources: &Resources,
    ) -> Result<Box<dyn Container>>;
    // Query all containers on the host.
    async fn containers(&self) -> Result<Vec<Box<dyn Container>>>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerStatus {
    Created,
    Running,
    Finished(ProcessExit),
    Failed(String),
}

pub trait Container: Send + Sync {
    fn id(&self) -> &str;
    fn host_id(&self) -> &str;
    fn base_layer(&self) -> &Layer;
    fn status(&self) -> Result<ContainerStatus>;
    fn attach_root(&self) -> Result<Box<dyn AttachedProcess>>;
    fn exec(&self, command: Command) -> Result<ProcessOutput>;
    fn final_layer(&self) -> Result<Option<Layer>>;
}