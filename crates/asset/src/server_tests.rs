//! Tests for asset server storage and handle management.

use std::path::PathBuf;
use crate::server::AssetServer;
use crate::handle::{Asset, AssetTypeId, Handle};

#[derive(Debug, Clone, PartialEq)]
struct TestAsset(String);

impl Asset for TestAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("test_asset")
    }
}

#[test]
fn asset_server_new() {
    let server = AssetServer::new();
    assert_eq!(server.asset_count(), 0);
}

#[test]
fn asset_server_default() {
    let server: AssetServer = Default::default();
    assert_eq!(server.asset_count(), 0);
}

#[test]
fn asset_server_insert() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("hello".to_string()));
    assert_eq!(handle.index(), 0);
    assert_eq!(handle.generation(), 0);
    assert_eq!(server.asset_count(), 1);
}

#[test]
fn asset_server_get() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("world".to_string()));
    let asset = server.get(handle).unwrap();
    assert_eq!(asset.0, "world");
}

#[test]
fn asset_server_get_stale() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("a".to_string()));
    server.remove::<TestAsset>(handle);
    assert!(server.get(handle).is_none());
}

#[test]
fn asset_server_resolve() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("resolve".to_string()));
    let asset_ref = server.resolve(handle).unwrap();
    assert_eq!(asset_ref.0, "resolve");
}

#[test]
fn asset_server_remove() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("b".to_string()));
    assert!(server.remove::<TestAsset>(handle));
    assert!(server.get(handle).is_none());
}

#[test]
fn asset_server_remove_stale() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("c".to_string()));
    server.remove::<TestAsset>(handle);
    assert!(!server.remove::<TestAsset>(handle));
}

#[test]
fn asset_server_replace() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("old".to_string()));
    let new_handle = server.replace(handle, TestAsset("new".to_string())).unwrap();
    assert!(server.get(handle).is_none());
    assert_eq!(server.get(new_handle).unwrap().0, "new");
}

#[test]
fn asset_server_insert_with_path() {
    let mut server = AssetServer::new();
    let handle = server.insert_with_path("assets/test.rxmesh", TestAsset("path".to_string()));
    let untyped = crate::handle::UntypedHandle::new(handle.index(), handle.generation());
    assert_eq!(server.path_for(untyped), Some(&PathBuf::from("assets/test.rxmesh")));
    assert_eq!(server.get_by_path("assets/test.rxmesh"), Some(untyped));
}

#[test]
fn asset_server_is_referenced() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("ref".to_string()));
    assert!(!server.is_referenced(handle));
    let _arc = server.get(handle).unwrap();
    assert!(server.is_referenced(handle));
}

#[test]
fn asset_server_drain_unreferenced() {
    let mut server = AssetServer::new();
    let h1 = server.insert(TestAsset("keep".to_string()));
    let h2 = server.insert(TestAsset("drop".to_string()));
    let _arc = server.get(h1).unwrap(); // hold reference to h1
    let removed = server.drain_unreferenced::<TestAsset>();
    assert_eq!(removed, 1);
    assert!(server.get(h1).is_some());
    assert!(server.get(h2).is_none());
}

#[test]
fn asset_server_drain_unreferenced_all() {
    let mut server = AssetServer::new();
    let h1 = server.insert(TestAsset("a".to_string()));
    let h2 = server.insert(TestAsset("b".to_string()));
    let _arc = server.get(h1).unwrap();
    let removed = server.drain_unreferenced_all();
    assert_eq!(removed, 1);
}

#[test]
fn asset_server_declare_dependencies() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("main".to_string()));
    server.declare_dependencies(handle, &["dep1", "dep2"]);
    let deps = server.dependency_paths(handle).unwrap();
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&PathBuf::from("dep1")));
}

#[test]
fn asset_server_are_dependencies_loaded_false() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("main".to_string()));
    server.declare_dependencies(handle, &["dep1"]);
    assert!(!server.are_dependencies_loaded(handle));
}

#[test]
fn asset_server_are_dependencies_loaded_true() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("main".to_string()));
    server.declare_dependencies(handle, &["dep1"]);
    server.insert_with_path("dep1", TestAsset("dep".to_string()));
    assert!(server.are_dependencies_loaded(handle));
}

#[test]
fn asset_server_dependents_of() {
    let mut server = AssetServer::new();
    let handle = server.insert(TestAsset("main".to_string()));
    server.declare_dependencies(handle, &["dep1"]);
    let dependents = server.dependents_of("dep1").unwrap();
    assert_eq!(dependents.len(), 1);
}
