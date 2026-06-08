//! Tests for asset streaming system.

use std::path::PathBuf;
use crate::streaming::*;

#[test]
fn streaming_priority_ordering() {
    assert!(StreamingPriority::Critical > StreamingPriority::High);
    assert!(StreamingPriority::High > StreamingPriority::Medium);
    assert!(StreamingPriority::Medium > StreamingPriority::Low);
    assert!(StreamingPriority::Low > StreamingPriority::Background);
}

#[test]
fn streaming_system_new() {
    let sys = StreamingSystem::new(10, 2);
    assert_eq!(sys.max_loaded, 10);
    assert_eq!(sys.budget_per_tick, 2);
    assert_eq!(sys.loaded_count(), 0);
    assert_eq!(sys.pending_load_count(), 0);
    assert_eq!(sys.pending_unload_count(), 0);
}

#[test]
fn streaming_system_default() {
    let sys: StreamingSystem = Default::default();
    assert_eq!(sys.max_loaded, 1024);
    assert_eq!(sys.budget_per_tick, 4);
}

#[test]
fn request_load_queues() {
    let mut sys = StreamingSystem::new(10, 2);
    sys.request_load("a.mesh", StreamingPriority::High);
    sys.request_load("b.mesh", StreamingPriority::Low);
    assert_eq!(sys.pending_load_count(), 2);
}

#[test]
fn request_unload_queues() {
    let mut sys = StreamingSystem::new(10, 2);
    sys.request_unload("a.mesh");
    sys.request_unload("b.mesh");
    assert_eq!(sys.pending_unload_count(), 2);
}

#[test]
fn request_load_dedup_already_loaded() {
    let mut sys = StreamingSystem::new(10, 2);
    sys.request_load("a.mesh", StreamingPriority::Low);
    let (loaded, _) = sys.tick();
    assert_eq!(loaded, 1);

    // Request same path with higher priority — should upgrade in-place
    sys.request_load("a.mesh", StreamingPriority::High);
    assert_eq!(sys.pending_load_count(), 0); // not re-queued
    let asset = sys.loaded().find(|a| a.path == PathBuf::from("a.mesh")).unwrap();
    assert_eq!(asset.priority, StreamingPriority::High);
}

#[test]
fn tick_processes_loads() {
    let mut sys = StreamingSystem::new(10, 2);
    sys.request_load("a.mesh", StreamingPriority::High);
    sys.request_load("b.mesh", StreamingPriority::Low);
    let (loaded, evicted) = sys.tick();
    assert_eq!(loaded, 2);
    assert_eq!(evicted, 0);
    assert_eq!(sys.loaded_count(), 2);
    assert_eq!(sys.pending_load_count(), 0);
}

#[test]
fn tick_respects_budget() {
    let mut sys = StreamingSystem::new(10, 1);
    sys.request_load("a.mesh", StreamingPriority::High);
    sys.request_load("b.mesh", StreamingPriority::Low);
    let (loaded, _) = sys.tick();
    assert_eq!(loaded, 1);
    assert_eq!(sys.pending_load_count(), 1);
}

#[test]
fn tick_processes_unloads() {
    let mut sys = StreamingSystem::new(10, 2);
    sys.request_load("a.mesh", StreamingPriority::High);
    sys.tick();
    assert_eq!(sys.loaded_count(), 1);

    sys.request_unload("a.mesh");
    let (_, _) = sys.tick();
    assert_eq!(sys.loaded_count(), 0);
    assert_eq!(sys.pending_unload_count(), 0);
}

#[test]
fn tick_evicts_lowest_priority() {
    let mut sys = StreamingSystem::new(2, 10);
    sys.request_load("a.mesh", StreamingPriority::Low);
    sys.request_load("b.mesh", StreamingPriority::Medium);
    sys.request_load("c.mesh", StreamingPriority::High);
    let (loaded, evicted) = sys.tick();
    assert_eq!(loaded, 3);
    assert_eq!(evicted, 1); // a.mesh evicted
    assert_eq!(sys.loaded_count(), 2);
    assert!(!sys.loaded().any(|a| a.path == PathBuf::from("a.mesh")));
}

#[test]
fn cancel_removes_pending() {
    let mut sys = StreamingSystem::new(10, 2);
    sys.request_load("a.mesh", StreamingPriority::High);
    sys.request_unload("b.mesh");
    sys.cancel("a.mesh");
    sys.cancel("b.mesh");
    assert_eq!(sys.pending_load_count(), 0);
    assert_eq!(sys.pending_unload_count(), 0);
}

#[test]
fn cancel_does_not_affect_loaded() {
    let mut sys = StreamingSystem::new(10, 2);
    sys.request_load("a.mesh", StreamingPriority::High);
    sys.tick();
    sys.cancel("a.mesh");
    assert_eq!(sys.loaded_count(), 1);
}

#[test]
fn handle_for_and_resolve() {
    let mut sys = StreamingSystem::new(10, 2);
    sys.request_load("a.mesh", StreamingPriority::High);
    sys.tick();

    let path = PathBuf::from("a.mesh");
    let placeholder = sys.handle_for(&path).unwrap();
    assert_eq!(placeholder.index, 0);
    assert_eq!(placeholder.generation, 0);

    let real = crate::handle::UntypedHandle::new(42, 1);
    sys.resolve_handle(&path, real);
    assert_eq!(sys.handle_for(&path).unwrap(), real);
}

#[test]
fn request_kind_variants() {
    assert_ne!(RequestKind::Load, RequestKind::Unload);
}

#[test]
fn streamed_asset_clone() {
    let asset = StreamedAsset {
        path: PathBuf::from("test.mesh"),
        priority: StreamingPriority::Critical,
        handle: crate::handle::UntypedHandle::new(1, 1),
    };
    let cloned = asset.clone();
    assert_eq!(asset.path, cloned.path);
    assert_eq!(asset.priority, cloned.priority);
}
