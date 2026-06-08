//! Async asset loader using a tokio runtime for IO.
//!
//! `AssetLoader` spawns tokio tasks that read files from disk, run the
//! appropriate `Importer`, and return the result via a oneshot channel.
//! The caller receives a standard `tokio::sync::oneshot::Receiver` future
//! that resolves to the loaded asset or an error string.

use std::path::PathBuf;

use tokio::runtime::Handle as TokioHandle;
use tokio::sync::oneshot;

use crate::importer::Importer;

/// Async asset loader backed by a tokio runtime handle.
pub struct AssetLoader {
    tokio: TokioHandle,
}

impl AssetLoader {
    /// Create a loader using the current tokio runtime.
    ///
    /// Panics if called outside of a tokio runtime.
    pub fn from_current_runtime() -> Self {
        Self {
            tokio: TokioHandle::current(),
        }
    }

    /// Create a loader from an explicit tokio handle.
    pub fn new(tokio: TokioHandle) -> Self {
        Self { tokio }
    }

    /// Load an asset asynchronously.
    ///
    /// Spawns a tokio task that:
    /// 1. Reads the file at `path`.
    /// 2. Runs `importer.import()` on the bytes.
    ///
    /// Returns a oneshot receiver that resolves to `Result<Asset, String>`.
    pub fn load<I: Importer + Send + Sync + 'static>(
        &self,
        path: impl Into<PathBuf>,
        importer: I,
    ) -> oneshot::Receiver<Result<I::Asset, String>> {
        let (tx, rx) = oneshot::channel();
        let path = path.into();
        let hint = path.to_str().map(|s| s.to_string());

        self.tokio.spawn(async move {
            let bytes = match tokio::fs::read(&path).await {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(Err(format!("failed to read {}: {}", path.display(), e)));
                    return;
                }
            };

            match importer.import(&bytes, hint.as_deref()).await {
                Ok(asset) => {
                    tracing::info!("asset loader: loaded {}", path.display());
                    let _ = tx.send(Ok(asset));
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("failed to import {}: {}", path.display(), e)));
                }
            }
        });

        rx
    }
}
