//! Tests for disk cache metadata and file operations.

use std::io::Write;
use crate::cache::DiskCache;

fn temp_dir(suffix: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("rustix_asset_cache_test_{}_{}", std::process::id(), suffix));
    p
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn cache_new_creates_directory() {
    let dir = temp_dir("new");
    cleanup(&dir);
    let cache = DiskCache::new(&dir).unwrap();
    assert!(cache.root().exists());
    cleanup(&dir);
}

#[test]
fn cache_write_and_read() {
    let dir = temp_dir("write_read");
    cleanup(&dir);
    let cache = DiskCache::new(&dir).unwrap();

    let source = dir.join("source.txt");
    std::fs::write(&source, "hello").unwrap();

    cache.write(&source, b"cached_data").unwrap();
    let data = cache.read(&source).unwrap();
    assert_eq!(data, b"cached_data");
    cleanup(&dir);
}

#[test]
fn cache_is_cached_true_after_write() {
    let dir = temp_dir("cached_true");
    cleanup(&dir);
    let cache = DiskCache::new(&dir).unwrap();

    let source = dir.join("source.txt");
    std::fs::write(&source, "hello").unwrap();

    cache.write(&source, b"data").unwrap();
    assert!(cache.is_cached(&source));
    cleanup(&dir);
}

#[test]
fn cache_is_cached_false_for_missing() {
    let dir = temp_dir("cached_false");
    cleanup(&dir);
    let cache = DiskCache::new(&dir).unwrap();

    let source = dir.join("nonexistent.txt");
    assert!(!cache.is_cached(&source));
    cleanup(&dir);
}

#[test]
fn cache_read_returns_none_for_missing() {
    let dir = temp_dir("read_none");
    cleanup(&dir);
    let cache = DiskCache::new(&dir).unwrap();

    let source = dir.join("nonexistent.txt");
    assert!(cache.read(&source).is_none());
    cleanup(&dir);
}

#[test]
fn cache_invalidate_removes_entry() {
    let dir = temp_dir("invalidate");
    cleanup(&dir);
    let cache = DiskCache::new(&dir).unwrap();

    let source = dir.join("source.txt");
    std::fs::write(&source, "hello").unwrap();

    cache.write(&source, b"data").unwrap();
    assert!(cache.is_cached(&source));
    cache.invalidate(&source).unwrap();
    assert!(!cache.is_cached(&source));
    cleanup(&dir);
}

#[test]
fn cache_entry_count() {
    let dir = temp_dir("entry_count");
    cleanup(&dir);
    let cache = DiskCache::new(&dir).unwrap();

    let source1 = dir.join("a.txt");
    let source2 = dir.join("b.txt");
    std::fs::write(&source1, "a").unwrap();
    std::fs::write(&source2, "b").unwrap();

    cache.write(&source1, b"data1").unwrap();
    cache.write(&source2, b"data2").unwrap();
    assert_eq!(cache.entry_count(), 2);
    cleanup(&dir);
}

#[test]
fn cache_total_size() {
    let dir = temp_dir("total_size");
    cleanup(&dir);
    let cache = DiskCache::new(&dir).unwrap();

    let source = dir.join("source.txt");
    std::fs::write(&source, "hello").unwrap();

    cache.write(&source, b"12345").unwrap();
    assert_eq!(cache.total_size(), 5);
    cleanup(&dir);
}

#[test]
fn cache_clear_removes_all() {
    let dir = temp_dir("clear");
    cleanup(&dir);
    let cache = DiskCache::new(&dir).unwrap();

    let source = dir.join("source.txt");
    std::fs::write(&source, "hello").unwrap();

    cache.write(&source, b"data").unwrap();
    cache.clear().unwrap();
    assert_eq!(cache.entry_count(), 0);
    assert_eq!(cache.total_size(), 0);
    cleanup(&dir);
}
