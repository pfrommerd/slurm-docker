// A SandboxEngine  is a low-level utility used for constructing containers on a host.
// Whereas a layer is just a reference to the content (and may be in the cloud), a sandbox is 
// always constructed on the physical host where the container will run. Routing the sandbox
// output back to the user (to satisfy the Container trait) is handled by the host implementation.

use super::{ContainerStatus, Resources};
use crate::Result;

use std::time::Duration;
use std::path::PathBuf;
use std::collections::BTreeMap;

use uuid::Uuid;
use serde::{Deserialize, Serialize};

pub trait Sandbox: Send + Sync {
    type Process: Process;

    fn id(&self) -> Uuid;
    fn status(&self) -> Result<ContainerStatus>;
    fn root(&self) -> Result<Self::Process>;
    async fn spawn(&self, command: Command) -> Result<Self::Process>;
    // Wait for the root process to exit.
    async fn wait(&self) -> Result<ProcessExit>;
}

pub struct SandboxSpec {
    // The layers to overlay for the sandbox.
    layers: Vec<PathBuf>,
    environment: BTreeMap<String, String>,
    resources: Resources,
    time_limit: Option<Duration>,
    command: Option<Command>,
}

pub trait SandboxEngine: Send + Sync {
    type Sandbox: Sandbox;

    fn name(&self) -> &str;
    fn create(&self, request: SandboxSpec) -> Result<Self::Sandbox>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub working_dir: Option<PathBuf>,
}

impl Command {
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

pub trait Process: Send + Sync {
    fn exit(&self) -> Result<ProcessExit>;
    fn stdout(&self) -> Result<Vec<u8>>;
    fn stderr(&self) -> Result<Vec<u8>>;
}

pub trait AttachedProcess: Process {
    fn wait(&mut self) -> Result<ProcessExit>;
    fn send_stdin(&mut self, bytes: &[u8]) -> Result<usize>;
}