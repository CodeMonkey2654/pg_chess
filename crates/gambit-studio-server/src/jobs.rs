//! Background ingest job tracking.

use crate::types::JobStatus;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static JOB_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Shared job manager (one active job at a time).
#[derive(Clone, Default)]
pub struct JobManager {
    inner: Arc<Mutex<Option<JobStatus>>>,
}

impl JobManager {
    /// Create a new job manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start tracking a new job, replacing any prior job state.
    pub fn start(&self, total_shards: usize, message: impl Into<String>) -> u64 {
        let id = JOB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let status = JobStatus {
            id,
            status: "running".to_string(),
            message: message.into(),
            current_shard: 0,
            total_shards,
            games_loaded: 0,
            games_per_min: None,
        };
        *self.inner.lock().expect("job lock") = Some(status);
        id
    }

    /// Update progress for the active job.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut JobStatus),
    {
        if let Some(job) = self.inner.lock().expect("job lock").as_mut() {
            f(job);
        }
    }

    /// Mark the active job complete.
    pub fn complete(&self, message: impl Into<String>) {
        if let Some(job) = self.inner.lock().expect("job lock").as_mut() {
            job.status = "complete".to_string();
            job.message = message.into();
        }
    }

    /// Mark the active job failed.
    pub fn fail(&self, message: impl Into<String>) {
        if let Some(job) = self.inner.lock().expect("job lock").as_mut() {
            job.status = "failed".to_string();
            job.message = message.into();
        }
    }

    /// Fetch the active job, if any.
    pub fn active(&self) -> Option<JobStatus> {
        self.inner
            .lock()
            .expect("job lock")
            .clone()
            .filter(|j| j.status == "running")
    }

    /// Fetch job status by id.
    pub fn get(&self, job_id: u64) -> Option<JobStatus> {
        self.inner
            .lock()
            .expect("job lock")
            .as_ref()
            .filter(|j| j.id == job_id)
            .cloned()
    }

    /// Whether a job is currently running.
    pub fn is_running(&self) -> bool {
        self.inner
            .lock()
            .expect("job lock")
            .as_ref()
            .is_some_and(|j| j.status == "running")
    }
}
