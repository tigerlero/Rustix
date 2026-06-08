//! Tests for task priority system.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use crate::task_priority::{PriorityTaskSystem, TaskPriority};

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
