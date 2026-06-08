//! Asset decoding on worker threads (image, mesh, audio).
//!
//! `AssetDecoderPool` wraps `rustix_core::task_priority::PriorityTaskSystem`
//! and submits asset import work to Low-priority worker threads so that
//! heavy decode work (image decompression, mesh parsing, audio decoding)
//! does not block the main / render thread.

use std::any::Any;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rustix_core::task_priority::{PriorityTaskSystem, TaskPriority};

use crate::importer::Importer;

/// Result of an asynchronous asset decode job.
pub struct DecodeResult {
    /// The file path that was decoded.
    pub path: PathBuf,
    /// The decoded asset boxed as `Any` for later downcasting, or a dummy
    /// unit if decoding failed.
    pub asset: Box<dyn Any + Send>,
    /// Error message if decoding failed.
    pub error: Option<String>,
}

/// A pool of worker threads dedicated to decoding assets off the main thread.
pub struct AssetDecoderPool {
    tasks: PriorityTaskSystem,
    results: Arc<Mutex<Vec<DecodeResult>>>,
}

impl AssetDecoderPool {
    /// Create a new decoder pool with the given number of worker threads.
    pub fn new(thread_count: usize) -> Self {
        Self {
            tasks: PriorityTaskSystem::new(thread_count),
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Submit an asset import to run on a worker thread.
    ///
    /// The importer is executed at `TaskPriority::Low` (background work).
    /// Call `poll_completed()` later to collect finished results.
    pub fn submit_import<I: Importer + Send + Sync + 'static>(
        &self,
        importer: I,
        bytes: Vec<u8>,
        hint: Option<String>,
        path: PathBuf,
    ) {
        let results = self.results.clone();
        let name = format!(
            "decode_{}",
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        self.tasks.submit_named(TaskPriority::Low, name, move || {
            let fut = importer.import(&bytes, hint.as_deref());
            let result = futures::executor::block_on(fut);
            let mut lock = results.lock().unwrap();
            match result {
                Ok(asset) => {
                    lock.push(DecodeResult {
                        path,
                        asset: Box::new(asset),
                        error: None,
                    });
                }
                Err(e) => {
                    lock.push(DecodeResult {
                        path,
                        asset: Box::new(()),
                        error: Some(e),
                    });
                }
            }
        });
    }

    /// Drain and return all completed decode results since the last poll.
    pub fn poll_completed(&self) -> Vec<DecodeResult> {
        self.results.lock().unwrap().drain(..).collect()
    }

    /// Block until all currently-submitted decode tasks have finished.
    ///
    /// This is useful for synchronous asset loading points (e.g. level load).
    pub fn wait_for_all(&self) {
        self.tasks.wait_for_all();
    }

    /// Number of worker threads in the pool.
    pub fn thread_count(&self) -> usize {
        self.tasks.thread_count()
    }
}
