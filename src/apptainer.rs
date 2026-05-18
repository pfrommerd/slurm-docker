use crate::runtime::{
    AttachedProcess, CommandSpec, ContainerEngine, ContainerLaunch, ContainerStatus,
    EngineContainer, Layer, LayerReference, ProcessExit, ProcessOutput,
};
use crate::slurm::FinishedAttachedProcess;
use crate::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct Apptainer {
    next_id: AtomicU64,
}

impl Apptainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }
}

impl ContainerEngine for Apptainer {
    fn name(&self) -> &str {
        "apptainer"
    }

    fn launch(&self, request: ContainerLaunch) -> Result<EngineContainer> {
        let id = format!(
            "apptainer-stub-{}",
            self.next_id.fetch_add(1, Ordering::Relaxed)
        );
        Ok(EngineContainer {
            id,
            base_layer: request.layer,
            status: ContainerStatus::Finished(ProcessExit { code: Some(0) }),
        })
    }

    fn attach(&self, _container_id: &str) -> Result<Box<dyn AttachedProcess>> {
        Ok(Box::new(FinishedAttachedProcess::success()))
    }

    fn exec(&self, _container_id: &str, _command: CommandSpec) -> Result<ProcessOutput> {
        Ok(ProcessOutput {
            exit: ProcessExit { code: Some(0) },
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn diff(&self, container_id: &str) -> Result<Option<Layer>> {
        Ok(Some(Layer::new(
            None,
            LayerReference::Remote {
                store_url: "apptainer://stub".to_string(),
                path: container_id.to_string(),
            },
            format!("stub-diff:{container_id}"),
        )))
    }
}
