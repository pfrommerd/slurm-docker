use crate::apptainer::Apptainer;
use crate::runtime::{
    CommandSpec, HostConfig, Layer, LayerReference, ResourceLimits, Resources, Scheduler,
};
use crate::slurm::{RawSlurmInfo, SlurmCluster, SlurmScheduler};
use crate::spec::Dockerfile;
use crate::state::{now_unix_secs, HostRecord, StateStore};
use crate::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(name = "slurm-docker")]
#[command(about = "A Docker-style CLI for building and running layered containers on Slurm")]
pub struct Cli {
    #[arg(long)]
    state: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Build(BuildArgs),
    Run(RunArgs),
    Ps,
    SlurmInfo,
}

#[derive(Debug, Parser)]
pub struct BuildArgs {
    #[arg(short = 'f', long, default_value = "Dockerfile")]
    file: PathBuf,
    #[arg(default_value = ".")]
    context: PathBuf,
}

#[derive(Debug, Parser)]
pub struct RunArgs {
    #[arg(long, default_value_t = 1)]
    cpus: u32,
    #[arg(long, default_value_t = 0)]
    gpus: u32,
    #[arg(long, default_value_t = 0)]
    memory_bytes: u64,
    #[arg(long)]
    time_limit_secs: Option<u64>,
    #[arg(long)]
    partition: Option<String>,
    image: String,
    command: Vec<String>,
}

pub fn main() -> Result<()> {
    run(Cli::parse())
}

pub fn run(cli: Cli) -> Result<()> {
    let state = StateStore::new(cli.state.unwrap_or_else(StateStore::default_path));
    match cli.command {
        Command::Build(args) => build(args),
        Command::Run(args) => run_container(state, args),
        Command::Ps => list_hosts(state),
        Command::SlurmInfo => slurm_info(),
    }
}

fn build(args: BuildArgs) -> Result<()> {
    let contents = std::fs::read_to_string(&args.file)?;
    let dockerfile = Dockerfile::parse(&contents)?;
    println!(
        "parsed {} root instruction(s) from {} using context {}",
        dockerfile.instructions().len(),
        args.file.display(),
        args.context.display()
    );
    Ok(())
}

fn run_container(state: StateStore, args: RunArgs) -> Result<()> {
    let engine = Apptainer::shared();
    let scheduler = SlurmScheduler::new("local-slurm", engine);
    let resources = Resources {
        cpus: args.cpus,
        gpus: args.gpus,
        memory_bytes: args.memory_bytes,
    };
    let limits = ResourceLimits {
        resources: resources.clone(),
        time_limit: args.time_limit_secs.map(Duration::from_secs),
    };
    let host = scheduler.create_host(HostConfig {
        name: None,
        partition: args.partition,
        resources: resources.clone(),
        time_limit: limits.time_limit,
    })?;
    let layer = Layer::new(
        None,
        LayerReference::Remote {
            store_url: "image".to_string(),
            path: args.image.clone(),
        },
        args.image,
    );
    let container = host.launch_container(layer, limits)?;

    if !args.command.is_empty() {
        let mut command = CommandSpec::new(args.command[0].clone());
        command.args = args.command[1..].to_vec();
        let _ = container.exec(command)?;
    }

    state.add_host(HostRecord {
        id: host.id().to_string(),
        scheduler_id: scheduler.id().to_string(),
        slurm_job_id: None,
        resources,
        time_limit: host.time_limit(),
        created_at_unix_secs: now_unix_secs(),
    })?;

    println!("{}", container.id());
    Ok(())
}

fn list_hosts(state: StateStore) -> Result<()> {
    let state = state.load()?;
    for host in state.hosts {
        println!(
            "{}\t{}\t{} cpu(s)\t{} gpu(s)\t{} bytes",
            host.id,
            host.scheduler_id,
            host.resources.cpus,
            host.resources.gpus,
            host.resources.memory_bytes
        );
    }
    Ok(())
}

fn slurm_info() -> Result<()> {
    let raw = RawSlurmInfo::from_local_system()?;
    let cluster = SlurmCluster::parse(&raw)?;
    println!(
        "{} node(s), {} partition(s), {} job(s)",
        cluster.nodes.len(),
        cluster.partitions.len(),
        cluster.jobs.len()
    );
    Ok(())
}
