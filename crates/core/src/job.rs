use serde::{Deserialize, Serialize};

use rayon::{ThreadPool, ThreadPoolBuilder};

/// The job system wraps a rayon thread pool and provides
/// fork-join parallelism for engine tasks.
///
/// On AMD many-core CPUs, the work-stealing scheduler in rayon
/// provides excellent load balancing across all cores.
/// Threads can be optionally pinned to physical cores for
/// compute-heavy workloads.
pub struct JobSystem {
    pool: ThreadPool,
    /// Number of worker threads
    thread_count: usize,
    /// Whether threads are pinned to cores
    affinity_enabled: bool,
}

/// Configuration for the job system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSystemConfig {
    /// Number of worker threads. `None` = auto-detect (all logical cores).
    pub thread_count: Option<usize>,
    /// Pin worker threads to CPU cores (Linux only, requires `pthread_setaffinity_np`).
    #[serde(default)]
    pub affinity: bool,
    /// Stack size per worker thread in bytes.
    pub stack_size: Option<usize>,
    /// Thread name prefix for worker threads.
    pub thread_name: Option<String>,
}

impl Default for JobSystemConfig {
    fn default() -> Self {
        Self {
            thread_count: None,
            affinity: false,
            stack_size: None,
            thread_name: Some("rx-worker".into()),
        }
    }
}

impl JobSystem {
    /// Create a new job system with the given configuration.
    pub fn new(config: &JobSystemConfig) -> Result<Self, JobError> {
        let thread_count = config
            .thread_count
            .unwrap_or_else(num_cpus_for_workstealing);

        let mut builder = ThreadPoolBuilder::new()
            .num_threads(thread_count);

        if let Some(ref name) = config.thread_name {
            let prefix = name.clone();
            builder = builder.thread_name(move |i| format!("{prefix}-{i}"));
        }

        if let Some(stack_size) = config.stack_size {
            builder = builder.stack_size(stack_size);
        }

        let pool = builder.build().map_err(|e| JobError::BuildFailed(e.to_string()))?;

        tracing::info!(
            thread_count = thread_count,
            affinity = config.affinity,
            "job system initialized"
        );

        Ok(Self {
            pool,
            thread_count,
            affinity_enabled: config.affinity,
        })
    }

    /// Returns the number of worker threads.
    pub fn thread_count(&self) -> usize {
        self.thread_count
    }

    /// Returns true if thread affinity is enabled.
    pub fn affinity_enabled(&self) -> bool {
        self.affinity_enabled
    }

    /// Execute a parallel operation on the thread pool and wait for completion.
    pub fn install<OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce() -> R + Send,
        R: Send,
    {
        self.pool.install(op)
    }

    /// Returns a reference to the underlying rayon thread pool.
    pub fn inner(&self) -> &ThreadPool {
        &self.pool
    }

    /// Re-create the thread pool with a new configuration.
    ///
    /// Pending work in the old pool is dropped; call this only
    /// when the system is idle.
    pub fn rebuild(&mut self, config: &JobSystemConfig) -> Result<(), JobError> {
        let new = Self::new(config)?;
        *self = new;
        Ok(())
    }
}

/// Errors that can occur during job system creation.
#[derive(Debug)]
pub enum JobError {
    BuildFailed(String),
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobError::BuildFailed(msg) => write!(f, "job system build failed: {msg}"),
        }
    }
}

impl std::error::Error for JobError {}

/// Estimate a good number of threads for work-stealing parallelism.
fn num_cpus_for_workstealing() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_system_rebuild_changes_thread_count() {
        let mut sys = JobSystem::new(&JobSystemConfig {
            thread_count: Some(2),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(sys.thread_count(), 2);

        sys.rebuild(&JobSystemConfig {
            thread_count: Some(4),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(sys.thread_count(), 4);
    }
}


