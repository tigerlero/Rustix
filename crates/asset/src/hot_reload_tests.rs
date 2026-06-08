//! Tests for hot reload types.

use crate::hot_reload::*;

#[test]
fn file_change_kind_variants() {
    assert_ne!(FileChangeKind::Created, FileChangeKind::Modified);
    assert_ne!(FileChangeKind::Modified, FileChangeKind::Removed);
}

#[test]
fn file_event_debug() {
    let ev = FileEvent {
        path: std::path::PathBuf::from("test.txt"),
        kind: FileChangeKind::Modified,
    };
    let s = format!("{:?}", ev);
    assert!(s.contains("test.txt"));
}

#[test]
fn hot_reloader_new() {
    let mut reloader = HotReloader::new();
    assert!(reloader.poll().next().is_none());
}

#[test]
fn hot_reloader_default() {
    let mut reloader: HotReloader = Default::default();
    assert!(reloader.poll().next().is_none());
}

#[test]
fn hot_reload_service_new() {
    let mut service = HotReloadService::new();
    assert!(service.reloader.poll().next().is_none());
}

#[test]
fn hot_reload_service_default() {
    let mut service: HotReloadService = Default::default();
    assert!(service.reloader.poll().next().is_none());
}
