use crate::spec::{Dockerfile, Instruction};
use crate::Result;
use object_store::path::Path as ObjectPath;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LayerReference {
    Local(PathBuf),
    Remote { store_url: String, path: String },
}

impl LayerReference {
    pub fn object_store_path(&self) -> Option<ObjectPath> {
        match self {
            LayerReference::Local(_) => None,
            LayerReference::Remote { path, .. } => Some(ObjectPath::from(path.as_str())),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    parent: Option<Arc<Layer>>,
    reference: LayerReference,
    checksum: String,
}

impl Layer {
    pub fn new(
        parent: Option<Arc<Layer>>,
        reference: LayerReference,
        checksum: impl Into<String>,
    ) -> Self {
        Self {
            parent,
            reference,
            checksum: checksum.into(),
        }
    }

    pub fn local(path: impl Into<PathBuf>, checksum: impl Into<String>) -> Self {
        Self::new(None, LayerReference::Local(path.into()), checksum)
    }

    pub fn parent(&self) -> Option<&Arc<Layer>> {
        self.parent.as_ref()
    }

    pub fn reference(&self) -> &LayerReference {
        &self.reference
    }

    pub fn checksum(&self) -> &str {
        &self.checksum
    }

    pub fn content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        if let Some(parent) = &self.parent {
            hasher.update(parent.content_hash());
        }
        hasher.update(self.checksum.as_bytes());
        hex::encode(hasher.finalize())
    }
}

impl PartialEq for Layer {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent && self.checksum == other.checksum
    }
}

impl Eq for Layer {}

impl Hash for Layer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.parent.hash(state);
        self.checksum.hash(state);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub cpus: u32,
    pub gpus: u32,
    pub memory_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub resources: Resources,
    pub time_limit: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostConfig {
    pub name: Option<String>,
    pub partition: Option<String>,
    pub resources: Resources,
    pub time_limit: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub id: String,
    pub name: Option<String>,
    pub resources: Resources,
    pub time_limit: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub working_dir: Option<PathBuf>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            env: BTreeMap::new(),
            working_dir: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessExit {
    pub code: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessOutput {
    pub exit: ProcessExit,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerStatus {
    Created,
    Running,
    Finished(ProcessExit),
    Failed(String),
}

pub trait AttachedProcess: Send {
    fn wait(&mut self) -> Result<ProcessExit>;
    fn send_stdin(&mut self, bytes: &[u8]) -> Result<usize>;
}

pub trait Container: Send + Sync {
    fn id(&self) -> &str;
    fn host_id(&self) -> &str;
    fn base_layer(&self) -> &Layer;
    fn status(&self) -> Result<ContainerStatus>;
    fn attach_root(&self) -> Result<Box<dyn AttachedProcess>>;
    fn exec(&self, command: CommandSpec) -> Result<ProcessOutput>;
    fn final_layer(&self) -> Result<Option<Layer>>;
}

pub trait Host: Send + Sync {
    fn id(&self) -> &str;
    fn resources(&self) -> &Resources;
    fn time_limit(&self) -> Option<Duration>;
    fn launch_container(&self, layer: Layer, limits: ResourceLimits) -> Result<Box<dyn Container>>;
    fn launch_instruction(
        &self,
        instruction: &Instruction,
        layer: Layer,
        limits: ResourceLimits,
    ) -> Result<Box<dyn Container>>;

    fn launch_dockerfile(
        &self,
        dockerfile: &Dockerfile,
        mut layer: Layer,
        limits: ResourceLimits,
    ) -> Result<Vec<Box<dyn Container>>> {
        let mut containers = Vec::new();
        for instruction in dockerfile.instructions() {
            let container = self.launch_instruction(instruction, layer.clone(), limits.clone())?;
            if let Some(final_layer) = container.final_layer()? {
                layer = final_layer;
            }
            containers.push(container);
        }
        Ok(containers)
    }
}

pub trait Scheduler: Send + Sync {
    fn id(&self) -> &str;
    fn hosts(&self) -> Result<Vec<HostInfo>>;
    fn create_host(&self, config: HostConfig) -> Result<Box<dyn Host>>;
}

#[derive(Clone, Debug)]
pub struct ContainerLaunch {
    pub host_id: String,
    pub layer: Layer,
    pub command: Option<CommandSpec>,
    pub limits: ResourceLimits,
}

#[derive(Clone, Debug)]
pub struct EngineContainer {
    pub id: String,
    pub base_layer: Layer,
    pub status: ContainerStatus,
}

pub trait ContainerEngine: Send + Sync {
    fn name(&self) -> &str;
    fn launch(&self, request: ContainerLaunch) -> Result<EngineContainer>;
    fn attach(&self, container_id: &str) -> Result<Box<dyn AttachedProcess>>;
    fn exec(&self, container_id: &str, command: CommandSpec) -> Result<ProcessOutput>;
    fn diff(&self, container_id: &str) -> Result<Option<Layer>>;
}
