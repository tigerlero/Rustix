use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::thread_priority::{set_current_thread_priority, ThreadPriority};

/// Priority levels for engine tasks.
///
/// The worker thread pool always drains the **high** queue before
/// looking at **medium**, and **medium** before **low**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskPriority {
    /// Background work: asset streaming, LOD generation, etc.
    Low = 0,
    /// Gameplay work: physics, animation, AI.
    Medium = 1,
    /// Frame-critical work: render culling, audio mixing.
    High = 2,
}

/// A thread pool that respects three priority levels.
///
/// Worker threads poll the queues in strict priority order
/// (high → medium → low) so frame-critical tasks are never
/// blocked by background work.
pub struct PriorityTaskSystem {
    high: Arc<Mutex<Vec<(String, Box<dyn FnOnce() + Send>)>>>,
    medium: Arc<Mutex<Vec<(String, Box<dyn FnOnce() + Send>)>>>,
    low: Arc<Mutex<Vec<(String, Box<dyn FnOnce() + Send>)>>>,
    pending: Arc<AtomicUsize>,
    thread_count: usize,
    shutdown: Arc<AtomicUsize>,
    excess: Arc<AtomicUsize>,
    handles: Mutex<Vec<std::thread::JoinHandle<()>>>,
    thread_priority: ThreadPriority,
}

impl PriorityTaskSystem {
    /// Spawn a new priority task system with `thread_count` workers.
    pub fn new(thread_count: usize) -> Self {
        Self::with_priority(thread_count, ThreadPriority::Normal)
    }

    /// Spawn a new priority task system with the given OS-level thread priority.
    pub fn with_priority(thread_count: usize, priority: ThreadPriority) -> Self {
        let high = Arc::new(Mutex::new(Vec::new()));
        let medium = Arc::new(Mutex::new(Vec::new()));
        let low = Arc::new(Mutex::new(Vec::new()));
        let pending = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicUsize::new(0));
        let excess = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(thread_count);

        for i in 0..thread_count {
            let h = high.clone();
            let m = medium.clone();
            let l = low.clone();
            let p = pending.clone();
            let s = shutdown.clone();
            let e = excess.clone();
            let prio = priority;
            let handle = std::thread::Builder::new()
                .name(format!("rx-priority-{i}"))
                .spawn(move || {
                    if let Err(err) = set_current_thread_priority(prio) {
                        tracing::warn!("failed to set priority thread priority: {err}");
                    }
                    worker_loop(h, m, l, p, s, e);
                })
                .expect("failed to spawn priority worker thread");
            handles.push(handle);
        }

        Self {
            high,
            medium,
            low,
            pending,
            thread_count,
            shutdown,
            excess,
            handles: Mutex::new(handles),
            thread_priority: priority,
        }
    }

    /// Submit a named task with the given priority.
    pub fn submit_named<F>(&self, priority: TaskPriority, name: impl Into<String>, func: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let boxed: Box<dyn FnOnce() + Send> = Box::new(func);
        self.pending.fetch_add(1, Ordering::SeqCst);
        match priority {
            TaskPriority::High => self.high.lock().push((name.into(), boxed)),
            TaskPriority::Medium => self.medium.lock().push((name.into(), boxed)),
            TaskPriority::Low => self.low.lock().push((name.into(), boxed)),
        }
    }

    /// Submit a task with the given priority (uses "task" as the name).
    pub fn submit<F>(&self, priority: TaskPriority, func: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit_named(priority, "task", func);
    }

    /// Submit a named task and block until it completes.
    pub fn install_named<F, R>(&self, priority: TaskPriority, name: impl Into<String>, func: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        self.submit_named(priority, name, move || {
            let _ = tx.send(func());
        });
        rx.recv().expect("priority task panicked or channel closed")
    }

    /// Submit a task and block until it completes (uses "task" as the name).
    pub fn install<F, R>(&self, priority: TaskPriority, func: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.install_named(priority, "task", func)
    }

    /// Block until all currently-submitted tasks have finished.
    pub fn wait_for_all(&self) {
        while self.pending.load(Ordering::SeqCst) > 0 {
            std::thread::yield_now();
        }
    }

    /// Number of worker threads.
    pub fn thread_count(&self) -> usize {
        self.thread_count
    }

    /// Grow or shrink the worker thread pool to `new_count`.
    ///
    /// When shrinking, idle threads are asked to exit.  The call
    /// blocks until the target count is reached.
    pub fn resize(&mut self, new_count: usize) {
        if new_count == self.thread_count {
            return;
        }

        if new_count > self.thread_count {
            let start = self.thread_count;
            let prio = self.thread_priority;
            for i in start..new_count {
                let h = self.high.clone();
                let m = self.medium.clone();
                let l = self.low.clone();
                let p = self.pending.clone();
                let s = self.shutdown.clone();
                let e = self.excess.clone();
                let handle = std::thread::Builder::new()
                    .name(format!("rx-priority-{i}"))
                    .spawn(move || {
                        if let Err(err) = set_current_thread_priority(prio) {
                            tracing::warn!("failed to set priority thread priority: {err}");
                        }
                        worker_loop(h, m, l, p, s, e);
                    })
                    .expect("failed to spawn priority worker thread");
                self.handles.lock().push(handle);
            }
            self.thread_count = new_count;
        } else {
            let to_remove = self.thread_count - new_count;
            self.excess.store(to_remove, Ordering::SeqCst);
            self.wait_for_all();
            loop {
                {
                    let mut handles = self.handles.lock();
                    let mut remaining = Vec::with_capacity(handles.len());
                    let mut removed = 0usize;
                    for h in handles.drain(..) {
                        if removed < to_remove && h.is_finished() {
                            let _ = h.join();
                            removed += 1;
                        } else {
                            remaining.push(h);
                        }
                    }
                    *handles = remaining;
                    if removed >= to_remove {
                        self.thread_count = handles.len();
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }

    /// Shut down all worker threads.  Consumes the system.
    pub fn shutdown(self) {
        self.shutdown.store(self.thread_count, Ordering::SeqCst);
        self.wait_for_all();
        for h in self.handles.into_inner() {
            let _ = h.join();
        }
    }
}

fn worker_loop(
    high: Arc<Mutex<Vec<(String, Box<dyn FnOnce() + Send>)>>>,
    medium: Arc<Mutex<Vec<(String, Box<dyn FnOnce() + Send>)>>>,
    low: Arc<Mutex<Vec<(String, Box<dyn FnOnce() + Send>)>>>,
    pending: Arc<AtomicUsize>,
    shutdown: Arc<AtomicUsize>,
    excess: Arc<AtomicUsize>,
) {
    loop {
        let task: Option<(String, Box<dyn FnOnce() + Send>)> = {
            if let Some(t) = high.lock().pop() {
                Some(t)
            } else if let Some(t) = medium.lock().pop() {
                Some(t)
            } else if let Some(t) = low.lock().pop() {
                Some(t)
            } else {
                None
            }
        };

        if let Some((name, t)) = task {
            #[cfg(feature = "profiling")]
            let _zone = tracy_client::span!(name);
            t();
            pending.fetch_sub(1, Ordering::SeqCst);
        } else {
            if shutdown.load(Ordering::SeqCst) > 0 {
                break;
            }
            // Try to claim an excess slot (resize shrink)
            let mut current = excess.load(Ordering::Relaxed);
            while current > 0 {
                match excess.compare_exchange_weak(
                    current,
                    current - 1,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return,
                    Err(actual) => current = actual,
                }
            }
            std::thread::yield_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn priority_system_creation() {
        let sys = PriorityTaskSystem::new(2);
        assert_eq!(sys.thread_count(), 2);
    }

    #[test]
    fn priority_submit_and_wait() {
        let sys = PriorityTaskSystem::new(2);
        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        sys.submit(TaskPriority::High, move || {
            f.store(true, Ordering::SeqCst);
        });
        sys.wait_for_all();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn priority_install_returns_value() {
        let sys = PriorityTaskSystem::new(2);
        let val = sys.install(TaskPriority::Medium, || 42);
        assert_eq!(val, 42);
    }

    #[test]
    fn priority_high_runs_before_medium_and_low() {
        let sys = PriorityTaskSystem::new(1); // single worker for deterministic order
        let order = Arc::new(Mutex::new(Vec::new()));

        let o = order.clone();
        sys.submit(TaskPriority::Low, move || o.lock().push("low"));

        let o = order.clone();
        sys.submit(TaskPriority::Medium, move || o.lock().push("medium"));

        let o = order.clone();
        sys.submit(TaskPriority::High, move || o.lock().push("high"));

        sys.wait_for_all();
        let seq = order.lock().clone();
        assert_eq!(seq, vec!["high", "medium", "low"]);
    }

    #[test]
    fn priority_multiple_tasks_per_level() {
        let sys = PriorityTaskSystem::new(1);
        let order = Arc::new(Mutex::new(Vec::new()));

        for i in 0..3 {
            let o = order.clone();
            sys.submit(TaskPriority::Low, move || o.lock().push(format!("L{i}")));
        }
        for i in 0..3 {
            let o = order.clone();
            sys.submit(TaskPriority::High, move || o.lock().push(format!("H{i}")));
        }

        sys.wait_for_all();
        let seq = order.lock().clone();
        // All high tasks must appear before all low tasks
        let first_low = seq.iter().position(|s| s.starts_with('L')).unwrap();
        let last_high = seq.iter().rposition(|s| s.starts_with('H')).unwrap();
        assert!(last_high < first_low, "high tasks should finish before low tasks");
    }

    #[test]
    fn priority_wait_for_all_empties_queues() {
        let sys = PriorityTaskSystem::new(4);
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..100 {
            let c = counter.clone();
            sys.submit(TaskPriority::Medium, move || {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        sys.wait_for_all();
        assert_eq!(counter.load(Ordering::SeqCst), 100);
    }

    #[test]
    fn priority_system_shutdown() {
        let sys = PriorityTaskSystem::new(2);
        sys.submit(TaskPriority::High, || {});
        sys.wait_for_all();
        sys.shutdown();
    }

    #[test]
    fn priority_system_thread_names() {
        let sys = PriorityTaskSystem::new(2);
        sys.submit(TaskPriority::High, || {
            let name = std::thread::current().name().unwrap().to_string();
            assert!(name.starts_with("rx-priority-"));
        });
        sys.wait_for_all();
        sys.shutdown();
    }

    #[test]
    fn priority_submit_named_runs() {
        let sys = PriorityTaskSystem::new(2);
        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        sys.submit_named(TaskPriority::High, "my_task", move || {
            f.store(true, Ordering::SeqCst);
        });
        sys.wait_for_all();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn priority_install_named_returns_value() {
        let sys = PriorityTaskSystem::new(2);
        let val = sys.install_named(TaskPriority::Medium, "compute", || 42);
        assert_eq!(val, 42);
    }

    #[test]
    fn priority_resize_grows() {
        let mut sys = PriorityTaskSystem::new(1);
        assert_eq!(sys.thread_count(), 1);
        sys.resize(4);
        assert_eq!(sys.thread_count(), 4);
        // verify new threads are functional
        let counter = Arc::new(AtomicUsize::new(0));
        for _ in 0..4 {
            let c = counter.clone();
            sys.submit(TaskPriority::High, move || {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }
        sys.wait_for_all();
        assert_eq!(counter.load(Ordering::SeqCst), 4);
        sys.shutdown();
    }

    #[test]
    fn priority_resize_shrinks() {
        let mut sys = PriorityTaskSystem::new(4);
        assert_eq!(sys.thread_count(), 4);
        sys.resize(1);
        assert_eq!(sys.thread_count(), 1);
        // verify remaining thread is functional
        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        sys.submit(TaskPriority::High, move || {
            f.store(true, Ordering::SeqCst);
        });
        sys.wait_for_all();
        assert!(flag.load(Ordering::SeqCst));
        sys.shutdown();
    }

    #[test]
    fn priority_resize_noop_same_count() {
        let mut sys = PriorityTaskSystem::new(2);
        sys.resize(2);
        assert_eq!(sys.thread_count(), 2);
        sys.shutdown();
    }
}
