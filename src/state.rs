use crate::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliState {
    pub schedulers: Vec<SchedulerRecord>,
    pub hosts: Vec<HostRecord>,
}

impl Default for CliState {
    fn default() -> Self {
        Self {
            schedulers: vec![SchedulerRecord {
                id: "local-slurm".to_string(),
                kind: SchedulerKind::Slurm,
                config: serde_json::json!({}),
                created_at_unix_secs: now_unix_secs(),
            }],
            hosts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchedulerRecord {
    pub id: String,
    pub kind: SchedulerKind,
    pub config: serde_json::Value,
    pub created_at_unix_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SchedulerKind {
    Slurm,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostRecord {
    pub id: String,
    pub scheduler_id: String,
    pub slurm_job_id: Option<u64>,
    pub resources: crate::runtime::Resources,
    pub time_limit: Option<Duration>,
    pub created_at_unix_secs: u64,
}

#[derive(Clone, Debug)]
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("slurm-docker")
            .join("state.json")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<CliState> {
        if !self.path.exists() {
            return Ok(CliState::default());
        }
        let contents = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&contents)?)
    }

    pub fn save(&self, state: &CliState) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(state)?;
        fs::write(&self.path, contents)?;
        Ok(())
    }

    pub fn add_host(&self, host: HostRecord) -> Result<CliState> {
        let mut state = self.load()?;
        state.hosts.retain(|record| record.id != host.id);
        state.hosts.push(host);
        self.save(&state)?;
        Ok(state)
    }
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_state() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = StateStore::new(tempdir.path().join("state.json"));
        let state = CliState::default();
        store.save(&state).unwrap();
        assert_eq!(store.load().unwrap(), state);
    }
}
