//! Background ingest job tracking with broadcast updates for streaming.

use gambit_proto::JobStatus;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

static JOB_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Shared job manager (one active job at a time).
#[derive(Clone)]
pub struct JobManager {
    inner: Arc<Mutex<Option<JobStatus>>>,
    updates: broadcast::Sender<JobStatus>,
}

impl JobManager {
    /// Create a new job manager.
    pub fn new() -> Self {
        let (updates, _) = broadcast::channel(64);
        Self {
            inner: Arc::new(Mutex::new(None)),
            updates,
        }
    }

    fn publish(&self, status: &JobStatus) {
        let _ = self.updates.send(status.clone());
    }

    /// Subscribe to job status updates.
    pub fn subscribe(&self) -> broadcast::Receiver<JobStatus> {
        self.updates.subscribe()
    }

    /// Start tracking a new job, replacing any prior job state.
    pub fn start(&self, total_shards: usize, message: impl Into<String>) -> u64 {
        let id = JOB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let status = JobStatus {
            id,
            status: "running".to_string(),
            message: message.into(),
            current_shard: 0,
            total_shards: total_shards as u32,
            games_loaded: 0,
            games_per_min: None,
        };
        *self.inner.lock().expect("job lock") = Some(status.clone());
        self.publish(&status);
        id
    }

    /// Update progress for the active job.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut JobStatus),
    {
        if let Some(job) = self.inner.lock().expect("job lock").as_mut() {
            f(job);
            self.publish(job);
        }
    }

    /// Mark the active job complete.
    pub fn complete(&self, message: impl Into<String>) {
        if let Some(job) = self.inner.lock().expect("job lock").as_mut() {
            job.status = "complete".to_string();
            job.message = message.into();
            self.publish(job);
        }
    }

    /// Mark the active job failed.
    pub fn fail(&self, message: impl Into<String>) {
        if let Some(job) = self.inner.lock().expect("job lock").as_mut() {
            job.status = "failed".to_string();
            job.message = message.into();
            self.publish(job);
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

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_job_and_update() {
        let mgr = JobManager::new();
        let id = mgr.start(12, "test");
        assert!(id > 0);
        assert!(mgr.is_running());
        mgr.update(|j| j.games_loaded = 100);
        assert_eq!(mgr.active().unwrap().games_loaded, 100);
        mgr.complete("done");
        assert!(!mgr.is_running());
    }
}
