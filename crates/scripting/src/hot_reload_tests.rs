//! Tests for script hot reload watcher.

use std::path::PathBuf;
use crate::hot_reload::HotReloadWatcher;

#[test]
fn hot_reload_watcher_new() {
    let watcher = HotReloadWatcher::new();
    assert!(watcher.tracked.is_empty());
}

#[test]
fn hot_reload_watcher_default() {
    let watcher: HotReloadWatcher = Default::default();
    assert!(watcher.tracked.is_empty());
}

#[test]
fn hot_reload_watcher_track_and_untrack() {
    let mut watcher = HotReloadWatcher::new();
    let path = PathBuf::from("test.rhai");
    watcher.track(path.clone());
    assert_eq!(watcher.tracked.len(), 1);

    watcher.untrack(&path);
    assert!(watcher.tracked.is_empty());
}

#[test]
fn hot_reload_watcher_check_no_changes_for_untracked() {
    let mut watcher = HotReloadWatcher::new();
    let changed = watcher.check();
    assert!(changed.is_empty());
}
