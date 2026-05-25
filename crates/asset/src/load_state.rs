use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::Notify;

use crate::handle::Asset;

/// Loading state for an async asset operation.
pub enum LoadState<T: Asset> {
    /// Asset is queued for loading.
    Pending,
    /// Asset is currently being loaded.
    Loading,
    /// Asset loaded successfully.
    Loaded(Arc<T>),
    /// Asset failed to load.
    Failed(String),
}

impl<T: Asset> Clone for LoadState<T> {
    fn clone(&self) -> Self {
        match self {
            LoadState::Pending => LoadState::Pending,
            LoadState::Loading => LoadState::Loading,
            LoadState::Loaded(arc) => LoadState::Loaded(Arc::clone(arc)),
            LoadState::Failed(s) => LoadState::Failed(s.clone()),
        }
    }
}

impl<T: Asset> LoadState<T> {
    pub fn is_loaded(&self) -> bool {
        matches!(self, LoadState::Loaded(_))
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, LoadState::Failed(_))
    }
}

/// A future that resolves to an asset handle.
pub struct LoadHandle<T: Asset> {
    inner: Arc<RwLock<LoadState<T>>>,
    notify: Arc<Notify>,
}

impl<T: Asset> LoadHandle<T> {
    pub fn new(state: LoadState<T>) -> Self {
        Self { inner: Arc::new(RwLock::new(state)), notify: Arc::new(Notify::new()) }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(*self.inner.read(), LoadState::Loaded(_))
    }

    pub fn is_failed(&self) -> bool {
        matches!(*self.inner.read(), LoadState::Failed(_))
    }

    pub fn resolve(self, value: T) {
        *self.inner.write() = LoadState::Loaded(Arc::new(value));
        self.notify.notify_waiters();
    }

    pub fn fail(self, error: String) {
        *self.inner.write() = LoadState::Failed(error);
        self.notify.notify_waiters();
    }
}

impl<T: Asset> Clone for LoadHandle<T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), notify: self.notify.clone() }
    }
}

impl<T: Asset> std::fmt::Debug for LoadHandle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadHandle").finish_non_exhaustive()
    }
}

/// A pending async asset load operation.
pub struct AsyncLoad<T: Asset> {
    handle: LoadHandle<T>,
}

impl<T: Asset> AsyncLoad<T> {
    pub fn new() -> Self {
        Self { handle: LoadHandle::new(LoadState::Pending) }
    }

    pub fn handle(&self) -> LoadHandle<T> {
        self.handle.clone()
    }
}

impl<T: Asset> Default for AsyncLoad<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Asset> Future for AsyncLoad<T> {
    type Output = Result<Arc<T>, String>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let state = self.handle.inner.read();
        match state.clone() {
            LoadState::Loaded(arc) => std::task::Poll::Ready(Ok(arc)),
            LoadState::Failed(err) => std::task::Poll::Ready(Err(err)),
            LoadState::Pending | LoadState::Loading => {
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        }
    }
}