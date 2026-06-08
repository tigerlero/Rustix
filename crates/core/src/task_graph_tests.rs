//! Tests for task graph scheduling.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use crate::job::{JobSystem, JobSystemConfig};
use crate::task_graph::TaskGraph;

fn make_system() -> JobSystem {
    JobSystem::new(&JobSystemConfig {
        thread_count: Some(2),
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn graph_starts_empty() {
    let g = TaskGraph::new();
    assert!(g.is_empty());
    assert_eq!(g.len(), 0);
}

#[test]
fn graph_add_task_returns_incrementing_ids() {
    let mut g = TaskGraph::new();
    let a = g.add_task("A", || {});
    let b = g.add_task("B", || {});
    let c = g.add_task("C", || {});
    assert_eq!(a.0, 0);
    assert_eq!(b.0, 1);
    assert_eq!(c.0, 2);
    assert_eq!(g.len(), 3);
}

#[test]
fn graph_topo_sort_linear_chain() {
    let mut g = TaskGraph::new();
    let a = g.add_task("A", || {});
    let b = g.add_task("B", || {});
    let c = g.add_task("C", || {});
    g.add_dependency(a, b);
    g.add_dependency(b, c);

    let names = g.execution_names().unwrap();
    assert_eq!(names, vec!["A", "B", "C"]);
}

#[test]
fn graph_topo_sort_diamond() {
    let mut g = TaskGraph::new();
    let a = g.add_task("A", || {});
    let b = g.add_task("B", || {});
    let c = g.add_task("C", || {});
    let d = g.add_task("D", || {});
    g.add_dependency(a, b);
    g.add_dependency(a, c);
    g.add_dependency(b, d);
    g.add_dependency(c, d);

    let order = g.topo_sort().unwrap();
    assert_eq!(order[0].0, 0); // A first
    assert_eq!(order[3].0, 3); // D last
    // B and C can be in either order
}

#[test]
fn graph_topo_sort_cycle_returns_none() {
    let mut g = TaskGraph::new();
    let a = g.add_task("A", || {});
    let b = g.add_task("B", || {});
    let c = g.add_task("C", || {});
    g.add_dependency(a, b);
    g.add_dependency(b, c);
    g.add_dependency(c, a);

    assert!(g.topo_sort().is_none());
}

#[test]
fn graph_has_cycle_detects_loop() {
    let mut g = TaskGraph::new();
    let a = g.add_task("A", || {});
    let b = g.add_task("B", || {});
    g.add_dependency(a, b);
    g.add_dependency(b, a);
    assert!(g.has_cycle());
}

#[test]
fn graph_has_cycle_no_cycle() {
    let mut g = TaskGraph::new();
    let a = g.add_task("A", || {});
    let b = g.add_task("B", || {});
    g.add_dependency(a, b);
    assert!(!g.has_cycle());
}

#[test]
fn graph_execute_runs_all_tasks() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut g = TaskGraph::new();

    let c = counter.clone();
    g.add_task("A", move || { c.fetch_add(1, Ordering::SeqCst); });
    let c = counter.clone();
    g.add_task("B", move || { c.fetch_add(1, Ordering::SeqCst); });
    let c = counter.clone();
    g.add_task("C", move || { c.fetch_add(1, Ordering::SeqCst); });

    let system = make_system();
    g.execute(&system);

    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[test]
fn graph_execute_respects_dependencies() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut g = TaskGraph::new();

    let c = counter.clone();
    let a = g.add_task("A", move || { c.fetch_add(1, Ordering::SeqCst); });

    let c = counter.clone();
    let b = g.add_task("B", move || {
        // B reads the counter after A has incremented it
        let val = c.load(Ordering::SeqCst);
        assert_eq!(val, 1, "A must have run before B");
        c.fetch_add(1, Ordering::SeqCst);
    });

    g.add_dependency(a, b);

    let system = make_system();
    g.execute(&system);

    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn graph_execute_parallel_frontier() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut g = TaskGraph::new();

    let c = counter.clone();
    let a = g.add_task("A", move || { c.fetch_add(1, Ordering::SeqCst); });
    let c = counter.clone();
    let b = g.add_task("B", move || { c.fetch_add(1, Ordering::SeqCst); });
    let c = counter.clone();
    let c_task = g.add_task("C", move || { c.fetch_add(1, Ordering::SeqCst); });

    // A and B both feed into C
    g.add_dependency(a, c_task);
    g.add_dependency(b, c_task);

    let system = make_system();
    g.execute(&system);

    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[test]
#[should_panic(expected = "task graph contains a cycle")]
fn graph_execute_panics_on_cycle() {
    let mut g = TaskGraph::new();
    let a = g.add_task("A", || {});
    let b = g.add_task("B", || {});
    g.add_dependency(a, b);
    g.add_dependency(b, a);

    let system = make_system();
    g.execute(&system);
}
