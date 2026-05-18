pub mod apptainer;
pub mod cli;
pub mod runtime;
pub mod slurm;
pub mod spec;
pub mod state;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("dockerfile parse error: {0}")]
    DockerfileParse(String),
    #[error("slurm parse error: {0}")]
    SlurmParse(String),
    #[error("container engine error: {0}")]
    ContainerEngine(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}
