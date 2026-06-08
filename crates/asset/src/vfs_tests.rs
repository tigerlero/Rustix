//! Tests for virtual file system.

use std::collections::HashMap;
use crate::vfs::{Vfs, MountPoint, build_archive};

fn temp_dir(suffix: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("rustix_vfs_test_{}_{}", std::process::id(), suffix));
    p
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn vfs_new_is_empty() {
    let vfs = Vfs::new();
    assert!(vfs.read("anything").is_none());
}

#[test]
fn vfs_mount_directory_read() {
    let dir = temp_dir("mount_read");
    cleanup(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.txt"), b"hello").unwrap();

    let mut vfs = Vfs::new();
    vfs.mount("test", MountPoint::directory(&dir));
    let data = vfs.read("test.txt").unwrap();
    assert_eq!(data, b"hello");
    cleanup(&dir);
}

#[test]
fn vfs_mount_directory_exists() {
    let dir = temp_dir("mount_exists");
    cleanup(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("exists.txt"), b"").unwrap();

    let mut vfs = Vfs::new();
    vfs.mount("test", MountPoint::directory(&dir));
    assert!(vfs.exists("exists.txt"));
    assert!(!vfs.exists("missing.txt"));
    cleanup(&dir);
}

#[test]
fn vfs_later_mount_shadows_earlier() {
    let dir1 = temp_dir("shadow_a").join("a");
    let dir2 = temp_dir("shadow_b").join("b");
    cleanup(&dir1.parent().unwrap());
    std::fs::create_dir_all(&dir1).unwrap();
    std::fs::create_dir_all(&dir2).unwrap();
    std::fs::write(dir1.join("file.txt"), b"first").unwrap();
    std::fs::write(dir2.join("file.txt"), b"second").unwrap();

    let mut vfs = Vfs::new();
    vfs.mount("a", MountPoint::directory(&dir1));
    vfs.mount("b", MountPoint::directory(&dir2));
    let data = vfs.read("file.txt").unwrap();
    assert_eq!(data, b"second");
    cleanup(&dir1.parent().unwrap());
}

#[test]
fn vfs_unmount() {
    let dir = temp_dir("unmount");
    cleanup(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("file.txt"), b"data").unwrap();

    let mut vfs = Vfs::new();
    vfs.mount("test", MountPoint::directory(&dir));
    assert!(vfs.exists("file.txt"));
    vfs.unmount("test");
    assert!(!vfs.exists("file.txt"));
    cleanup(&dir);
}

#[test]
fn vfs_archive_read() {
    let mut files = HashMap::new();
    files.insert("a.txt".to_string(), b"alpha".to_vec());
    files.insert("b.txt".to_string(), b"beta".to_vec());

    let archive = build_archive("test", files);
    let mut vfs = Vfs::new();
    vfs.mount("archive", archive);

    assert_eq!(vfs.read("a.txt").unwrap(), b"alpha");
    assert_eq!(vfs.read("b.txt").unwrap(), b"beta");
}

#[test]
fn vfs_archive_exists() {
    let mut files = HashMap::new();
    files.insert("a.txt".to_string(), b"alpha".to_vec());

    let archive = build_archive("test", files);
    let mut vfs = Vfs::new();
    vfs.mount("archive", archive);

    assert!(vfs.exists("a.txt"));
    assert!(!vfs.exists("missing.txt"));
}

#[test]
fn vfs_list_directory_mount() {
    let dir = temp_dir("list_dir");
    cleanup(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), b"").unwrap();
    std::fs::write(dir.join("b.txt"), b"").unwrap();

    let mut vfs = Vfs::new();
    vfs.mount("test", MountPoint::directory(&dir));
    let mut list = vfs.list("");
    list.sort();
    assert_eq!(list, vec!["a.txt", "b.txt"]);
    cleanup(&dir);
}

#[test]
fn vfs_list_archive_mount() {
    let mut files = HashMap::new();
    files.insert("dir/a.txt".to_string(), b"".to_vec());
    files.insert("dir/b.txt".to_string(), b"".to_vec());

    let archive = build_archive("test", files);
    let mut vfs = Vfs::new();
    vfs.mount("archive", archive);

    let mut list = vfs.list("dir");
    list.sort();
    assert_eq!(list, vec!["a.txt", "b.txt"]);
}

#[test]
fn vfs_resolve_directory() {
    let dir = temp_dir("resolve");
    cleanup(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("file.txt"), b"").unwrap();

    let mut vfs = Vfs::new();
    vfs.mount("test", MountPoint::directory(&dir));
    let path = vfs.resolve("file.txt").unwrap();
    assert!(path.ends_with("file.txt"));
    cleanup(&dir);
}

#[test]
fn vfs_resolve_archive_returns_none() {
    let mut files = HashMap::new();
    files.insert("a.txt".to_string(), b"".to_vec());

    let archive = build_archive("test", files);
    let mut vfs = Vfs::new();
    vfs.mount("archive", archive);

    assert!(vfs.resolve("a.txt").is_none());
}

#[test]
fn vfs_read_with_path_directory() {
    let dir = temp_dir("read_with_path");
    cleanup(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("file.txt"), b"data").unwrap();

    let mut vfs = Vfs::new();
    vfs.mount("test", MountPoint::directory(&dir));
    let (data, path) = vfs.read_with_path("file.txt").unwrap();
    assert_eq!(data, b"data");
    assert!(path.is_some());
    cleanup(&dir);
}

#[test]
fn vfs_read_with_path_archive() {
    let mut files = HashMap::new();
    files.insert("a.txt".to_string(), b"data".to_vec());

    let archive = build_archive("test", files);
    let mut vfs = Vfs::new();
    vfs.mount("archive", archive);

    let (data, path) = vfs.read_with_path("a.txt").unwrap();
    assert_eq!(data, b"data");
    assert!(path.is_none());
}
