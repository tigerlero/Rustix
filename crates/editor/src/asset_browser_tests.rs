//! Tests for asset browser state and entries.

use std::path::PathBuf;
use crate::asset_browser::{AssetEntry, AssetBrowserState};

#[test]
fn asset_entry_folder_name() {
    let entry = AssetEntry::Folder {
        path: PathBuf::from("/assets"),
        name: "assets".to_string(),
        expanded: true,
    };
    assert_eq!(entry.name(), "assets");
    assert_eq!(entry.path(), &PathBuf::from("/assets"));
}

#[test]
fn asset_entry_file_name() {
    let entry = AssetEntry::File {
        path: PathBuf::from("/assets/mesh.rxmesh"),
        name: "mesh.rxmesh".to_string(),
        asset_type: "mesh".to_string(),
    };
    assert_eq!(entry.name(), "mesh.rxmesh");
    assert_eq!(entry.path(), &PathBuf::from("/assets/mesh.rxmesh"));
}

#[test]
fn asset_browser_state_new() {
    let state = AssetBrowserState::new("/assets");
    assert_eq!(state.root, PathBuf::from("/assets"));
    assert!(state.entries.is_empty());
    assert!(state.selected.is_none());
    assert!(state.search_filter.is_empty());
}

#[test]
fn asset_browser_state_default() {
    let state: AssetBrowserState = Default::default();
    assert!(state.root.as_os_str().is_empty());
}

#[test]
fn asset_browser_set_entries() {
    let mut state = AssetBrowserState::new("/assets");
    state.set_entries(vec![
        AssetEntry::Folder { path: PathBuf::from("/assets/fx"), name: "fx".to_string(), expanded: false },
        AssetEntry::File { path: PathBuf::from("/assets/mesh.rxmesh"), name: "mesh".to_string(), asset_type: "mesh".to_string() },
    ]);
    assert_eq!(state.entries.len(), 2);
}

#[test]
fn asset_browser_filtered_entries_empty_filter() {
    let mut state = AssetBrowserState::new("/assets");
    state.set_entries(vec![
        AssetEntry::File { path: PathBuf::from("/assets/a.rxmesh"), name: "a".to_string(), asset_type: "mesh".to_string() },
        AssetEntry::File { path: PathBuf::from("/assets/b.rxmesh"), name: "b".to_string(), asset_type: "mesh".to_string() },
    ]);
    let filtered = state.filtered_entries();
    assert_eq!(filtered.len(), 2);
}

#[test]
fn asset_browser_filtered_entries_by_name() {
    let mut state = AssetBrowserState::new("/assets");
    state.set_entries(vec![
        AssetEntry::File { path: PathBuf::from("/assets/alpha.rxmesh"), name: "alpha".to_string(), asset_type: "mesh".to_string() },
        AssetEntry::File { path: PathBuf::from("/assets/beta.rxmesh"), name: "beta".to_string(), asset_type: "mesh".to_string() },
    ]);
    state.search_filter = "alp".to_string();
    let filtered = state.filtered_entries();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name(), "alpha");
}

#[test]
fn asset_browser_select() {
    let mut state = AssetBrowserState::new("/assets");
    state.select(PathBuf::from("/assets/mesh.rxmesh"));
    assert_eq!(state.selected, Some(PathBuf::from("/assets/mesh.rxmesh")));
}
