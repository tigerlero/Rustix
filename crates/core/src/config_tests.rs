//! Tests for config watcher.

use std::io::Write;
use std::time::Duration;
use crate::config::{ConfigWatcher, EngineConfig};

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("rustix_config_test_{}_{}", std::process::id(), name));
    p
}

fn make_toml(title: &str) -> String {
    format!(
        r#"[window]
title = "{title}"
width = 1920
height = 1080
fullscreen = false
vsync = false
backend = "auto"
"#
    )
}

#[test]
fn watcher_first_update_loads_config() {
    let path = temp_path("first");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", make_toml("Test")).unwrap();
    }

    let mut reloaded = false;
    {
        let mut watcher = ConfigWatcher::new(&path, |cfg: &EngineConfig| {
            assert_eq!(cfg.window.title, "Test");
            reloaded = true;
        });
        watcher.set_interval(Duration::from_millis(0));
        assert!(watcher.update().unwrap());
    }
    assert!(reloaded);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn watcher_no_change_returns_false() {
    let path = temp_path("no_change");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", make_toml("Test")).unwrap();
    }

    let mut call_count = 0usize;
    let mut watcher = ConfigWatcher::new(&path, |_cfg: &EngineConfig| {
        call_count += 1;
    });
    watcher.set_interval(Duration::from_millis(0));

    assert!(watcher.update().unwrap());   // first load
    assert!(!watcher.update().unwrap());  // no change
    assert_eq!(call_count, 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn watcher_detects_file_change() {
    let path = temp_path("detect");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", make_toml("First")).unwrap();
    }

    let titles = std::cell::RefCell::new(Vec::new());
    let mut watcher = ConfigWatcher::new(&path, |cfg: &EngineConfig| {
        titles.borrow_mut().push(cfg.window.title.clone());
    });
    watcher.set_interval(Duration::from_millis(0));

    assert!(watcher.update().unwrap());
    assert_eq!(titles.borrow().as_slice(), &["First"]);

    // Wait a tiny bit so the mtime actually changes
    std::thread::sleep(Duration::from_millis(50));

    {
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", make_toml("Second")).unwrap();
    }

    assert!(watcher.update().unwrap());
    assert_eq!(titles.borrow().as_slice(), &["First", "Second"]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn watcher_missing_file_returns_false() {
    let path = std::path::PathBuf::from("/tmp/nonexistent_config_12345_rustix.toml");
    let mut watcher = ConfigWatcher::new(&path, |_cfg: &EngineConfig| {
        panic!("should not be called");
    });
    watcher.set_interval(Duration::from_millis(0));
    assert!(!watcher.update().unwrap());
}

#[test]
fn watcher_request_refresh_forces_check() {
    let path = temp_path("refresh");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", make_toml("A")).unwrap();
    }

    let titles = std::cell::RefCell::new(Vec::new());
    let mut watcher = ConfigWatcher::new(&path, |cfg: &EngineConfig| {
        titles.borrow_mut().push(cfg.window.title.clone());
    });
    watcher.set_interval(Duration::from_secs(3600)); // very long interval

    assert!(watcher.update().unwrap());
    assert_eq!(titles.borrow().as_slice(), &["A"]);

    std::thread::sleep(Duration::from_millis(50));
    {
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", make_toml("B")).unwrap();
    }

    // Without request_refresh this would wait 1 hour
    assert!(!watcher.update().unwrap());
    watcher.request_refresh();
    assert!(watcher.update().unwrap());
    assert_eq!(titles.borrow().as_slice(), &["A", "B"]);
    let _ = std::fs::remove_file(&path);
}
