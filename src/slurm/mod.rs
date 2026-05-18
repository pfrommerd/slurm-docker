pub mod parser;

use crate::runtime::{
    AttachedProcess, CommandSpec, Container, ContainerEngine, ContainerLaunch, ContainerStatus,
    EngineContainer, Host, HostConfig, HostInfo, Layer, ProcessExit, ProcessOutput, ResourceLimits,
    Resources, Scheduler,
};
use crate::spec::{Healthcheck, Instruction, ShellOrExec};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlurmPartition {
    #[serde(rename = "PartitionName")]
    pub name: String,
    #[serde(rename = "State")]
    pub state: PartitionState,
    #[serde(rename = "Nodes", default)]
    pub nodes: Option<String>,
    #[serde(rename = "TotalCPUs", default)]
    pub total_cpus: Option<u32>,
    #[serde(rename = "TotalNodes", default)]
    pub total_nodes: Option<u32>,
    #[serde(rename = "TRES", default)]
    pub tres: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionState {
    #[serde(rename = "UP")]
    Up,
    #[serde(rename = "DOWN")]
    Down,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlurmNode {
    #[serde(rename = "NodeName")]
    pub name: String,
    #[serde(rename = "State")]
    pub state: NodeState,
    #[serde(rename = "CPUAlloc", default)]
    pub cpu_alloc: Option<u32>,
    #[serde(rename = "CPUTot", default)]
    pub cpus: Option<u32>,
    #[serde(rename = "RealMemory", default)]
    pub real_memory_mb: Option<u64>,
    #[serde(rename = "AllocMem", default)]
    pub alloc_memory_mb: Option<u64>,
    #[serde(rename = "FreeMem", default)]
    pub free_memory_mb: Option<u64>,
    #[serde(rename = "Gres", default)]
    pub gres: Option<String>,
    #[serde(rename = "Partitions", default)]
    pub partitions: Vec<String>,
    #[serde(rename = "CfgTRES", default)]
    pub configured_tres: BTreeMap<String, String>,
    #[serde(rename = "AllocTRES", default)]
    pub allocated_tres: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    #[serde(rename = "IDLE")]
    Idle,
    #[serde(rename = "ALLOCATED")]
    Allocated,
    #[serde(rename = "MIX")]
    Mixed,
    #[serde(rename = "DOWN")]
    Down,
    #[serde(rename = "DRAIN")]
    Drain,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlurmJob {
    #[serde(rename = "JobId")]
    pub job_id: u64,
    #[serde(rename = "JobName")]
    pub name: String,
    #[serde(rename = "Partition", default)]
    pub partition: Option<String>,
    #[serde(rename = "UserId", default)]
    pub user: Option<String>,
    #[serde(rename = "JobState")]
    pub state: JobState,
    #[serde(rename = "NumCPUs", default)]
    pub num_cpus: Option<u32>,
    #[serde(rename = "NumNodes", default)]
    pub num_nodes: Option<String>,
    #[serde(rename = "NodeList", default)]
    pub node_list: Vec<String>,
    #[serde(rename = "ReqTRES", default)]
    pub requested_tres: BTreeMap<String, String>,
    #[serde(rename = "AllocTRES", default)]
    pub allocated_tres: BTreeMap<String, String>,
    #[serde(rename = "Command", default)]
    pub command: Option<String>,
    #[serde(rename = "WorkDir", default)]
    pub work_dir: Option<String>,
    #[serde(rename = "TimeLimit", default)]
    pub time_limit: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "COMPLETED")]
    Completed,
    #[serde(rename = "FAILED")]
    Failed,
    #[serde(rename = "CANCELLED")]
    Cancelled,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlurmCluster {
    pub nodes: BTreeMap<String, SlurmNode>,
    pub partitions: BTreeMap<String, SlurmPartition>,
    pub jobs: BTreeMap<u64, SlurmJob>,
}

impl SlurmCluster {
    pub fn parse(raw: &RawSlurmInfo) -> Result<Self> {
        Ok(Self {
            nodes: parse_nodes(&raw.nodes)?
                .into_iter()
                .map(|node| (node.name.clone(), node))
                .collect(),
            partitions: parse_partitions(&raw.partitions)?
                .into_iter()
                .map(|partition| (partition.name.clone(), partition))
                .collect(),
            jobs: parse_jobs(&raw.jobs)?
                .into_iter()
                .map(|job| (job.job_id, job))
                .collect(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSlurmInfo {
    pub nodes: String,
    pub partitions: String,
    pub jobs: String,
}

impl RawSlurmInfo {
    pub fn from_local_system() -> Result<Self> {
        Ok(Self {
            nodes: run_slurm_command(["show", "nodes"])?,
            jobs: run_slurm_command(["show", "jobs", "--details"])?,
            partitions: run_slurm_command(["show", "partitions"])?,
        })
    }
}

pub fn parse_partitions(input: &str) -> Result<Vec<SlurmPartition>> {
    parse_records(input)
}

pub fn parse_nodes(input: &str) -> Result<Vec<SlurmNode>> {
    parse_records(input)
}

pub fn parse_jobs(input: &str) -> Result<Vec<SlurmJob>> {
    parse_records(input)
}

fn parse_records<'de, T>(input: &'de str) -> Result<Vec<T>>
where
    T: Deserialize<'de>,
{
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }
    parser::from_str(input).map_err(|err| Error::SlurmParse(err.to_string()))
}

fn run_slurm_command<const N: usize>(args: [&str; N]) -> Result<String> {
    let output = Command::new("scontrol").args(args).output()?;
    if !output.status.success() {
        return Err(Error::SlurmParse(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub struct SlurmScheduler {
    id: String,
    engine: Arc<dyn ContainerEngine>,
    hosts: Arc<Mutex<Vec<HostInfo>>>,
}

impl SlurmScheduler {
    pub fn new(id: impl Into<String>, engine: Arc<dyn ContainerEngine>) -> Self {
        Self {
            id: id.into(),
            engine,
            hosts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn cluster(&self) -> Result<SlurmCluster> {
        SlurmCluster::parse(&RawSlurmInfo::from_local_system()?)
    }
}

impl Scheduler for SlurmScheduler {
    fn id(&self) -> &str {
        &self.id
    }

    fn hosts(&self) -> Result<Vec<HostInfo>> {
        Ok(self
            .hosts
            .lock()
            .map_err(|_| Error::SlurmParse("host registry lock poisoned".to_string()))?
            .clone())
    }

    fn create_host(&self, config: HostConfig) -> Result<Box<dyn Host>> {
        let id = config
            .name
            .clone()
            .unwrap_or_else(|| format!("slurm-host-{}", unix_millis()));
        let info = HostInfo {
            id: id.clone(),
            name: config.name.clone(),
            resources: config.resources.clone(),
            time_limit: config.time_limit,
        };
        self.hosts
            .lock()
            .map_err(|_| Error::SlurmParse("host registry lock poisoned".to_string()))?
            .push(info);

        Ok(Box::new(SlurmHost {
            id,
            resources: config.resources,
            time_limit: config.time_limit,
            partition: config.partition,
            engine: self.engine.clone(),
        }))
    }
}

pub struct SlurmHost {
    id: String,
    resources: Resources,
    time_limit: Option<Duration>,
    partition: Option<String>,
    engine: Arc<dyn ContainerEngine>,
}

impl SlurmHost {
    pub fn partition(&self) -> Option<&str> {
        self.partition.as_deref()
    }
}

impl Host for SlurmHost {
    fn id(&self) -> &str {
        &self.id
    }

    fn resources(&self) -> &Resources {
        &self.resources
    }

    fn time_limit(&self) -> Option<Duration> {
        self.time_limit
    }

    fn launch_container(&self, layer: Layer, limits: ResourceLimits) -> Result<Box<dyn Container>> {
        let engine_container = self.engine.launch(ContainerLaunch {
            host_id: self.id.clone(),
            layer,
            command: None,
            limits,
        })?;
        Ok(Box::new(SlurmContainer {
            host_id: self.id.clone(),
            engine: self.engine.clone(),
            engine_container,
        }))
    }

    fn launch_instruction(
        &self,
        instruction: &Instruction,
        layer: Layer,
        limits: ResourceLimits,
    ) -> Result<Box<dyn Container>> {
        let command = command_for_instruction(instruction)?;
        let engine_container = self.engine.launch(ContainerLaunch {
            host_id: self.id.clone(),
            layer,
            command,
            limits,
        })?;
        Ok(Box::new(SlurmContainer {
            host_id: self.id.clone(),
            engine: self.engine.clone(),
            engine_container,
        }))
    }
}

pub struct SlurmContainer {
    host_id: String,
    engine: Arc<dyn ContainerEngine>,
    engine_container: EngineContainer,
}

impl Container for SlurmContainer {
    fn id(&self) -> &str {
        &self.engine_container.id
    }

    fn host_id(&self) -> &str {
        &self.host_id
    }

    fn base_layer(&self) -> &Layer {
        &self.engine_container.base_layer
    }

    fn status(&self) -> Result<ContainerStatus> {
        Ok(self.engine_container.status.clone())
    }

    fn attach_root(&self) -> Result<Box<dyn AttachedProcess>> {
        self.engine.attach(&self.engine_container.id)
    }

    fn exec(&self, command: CommandSpec) -> Result<ProcessOutput> {
        self.engine.exec(&self.engine_container.id, command)
    }

    fn final_layer(&self) -> Result<Option<Layer>> {
        self.engine.diff(&self.engine_container.id)
    }
}

pub struct FinishedAttachedProcess {
    exit: ProcessExit,
}

impl FinishedAttachedProcess {
    pub fn success() -> Self {
        Self {
            exit: ProcessExit { code: Some(0) },
        }
    }
}

impl AttachedProcess for FinishedAttachedProcess {
    fn wait(&mut self) -> Result<ProcessExit> {
        Ok(self.exit.clone())
    }

    fn send_stdin(&mut self, _bytes: &[u8]) -> Result<usize> {
        Err(Error::Unsupported(
            "cannot send stdin to a completed process".to_string(),
        ))
    }
}

fn command_for_instruction(instruction: &Instruction) -> Result<Option<CommandSpec>> {
    match instruction {
        Instruction::Run(command)
        | Instruction::Cmd(command)
        | Instruction::Entrypoint(command) => Ok(Some(command_spec(command))),
        Instruction::Healthcheck(Healthcheck::Command { command, .. }) => {
            Ok(Some(command_spec(command)))
        }
        Instruction::Stage { dockerfile, .. } => {
            let joined = dockerfile
                .instructions()
                .iter()
                .filter_map(|instruction| match command_for_instruction(instruction) {
                    Ok(Some(command)) => Some(command.program),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" && ");
            if joined.is_empty() {
                Ok(Some(noop_command()))
            } else {
                Ok(Some(shell_command(joined)))
            }
        }
        _ => Ok(Some(noop_command())),
    }
}

fn command_spec(command: &ShellOrExec) -> CommandSpec {
    match command {
        ShellOrExec::Shell(command) => shell_command(command.clone()),
        ShellOrExec::Exec(parts) => {
            let mut spec =
                CommandSpec::new(parts.first().cloned().unwrap_or_else(|| "true".into()));
            spec.args = parts.iter().skip(1).cloned().collect();
            spec
        }
    }
}

fn shell_command(command: String) -> CommandSpec {
    let mut spec = CommandSpec::new("/bin/sh");
    spec.args = vec!["-lc".to_string(), command];
    spec
}

fn noop_command() -> CommandSpec {
    command_spec(&ShellOrExec::Shell("true".to_string()))
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_slurm_nodes() {
        let input = "NodeName=node1 State=IDLE CPUAlloc=0 CPUTot=64 RealMemory=128000 Partitions=debug CfgTRES=cpu=64,mem=128000M AllocTRES=\n\nNodeName=node2 State=ALLOCATED CPUAlloc=4 CPUTot=64 RealMemory=128000 Partitions=debug";
        let nodes = parse_nodes(input).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "node1");
        assert_eq!(
            nodes[0].configured_tres.get("cpu").map(String::as_str),
            Some("64")
        );
    }

    #[test]
    fn parses_slurm_jobs() {
        let input = "JobId=42 JobName=build JobState=RUNNING NumCPUs=2 NumNodes=1 NodeList=node1 ReqTRES=cpu=2,mem=8G AllocTRES=cpu=2";
        let jobs = parse_jobs(input).unwrap();
        assert_eq!(jobs[0].job_id, 42);
        assert_eq!(jobs[0].state, JobState::Running);
    }
}
