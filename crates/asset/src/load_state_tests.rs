//! Tests for async load state and handles.

use std::sync::Arc;
use crate::load_state::{LoadState, LoadHandle, AsyncLoad};
use crate::material::MaterialAsset;

#[test]
fn load_state_clone() {
    let state = LoadState::Loaded(Arc::new(MaterialAsset::default()));
    let cloned = state.clone();
    assert!(cloned.is_loaded());
}

#[test]
fn load_state_is_loaded() {
    assert!(!LoadState::<MaterialAsset>::Pending.is_loaded());
    assert!(!LoadState::<MaterialAsset>::Loading.is_loaded());
    assert!(LoadState::Loaded(Arc::new(MaterialAsset::default())).is_loaded());
    assert!(!LoadState::<MaterialAsset>::Failed("err".to_string()).is_loaded());
}

#[test]
fn load_state_is_failed() {
    assert!(!LoadState::<MaterialAsset>::Pending.is_failed());
    assert!(!LoadState::<MaterialAsset>::Loading.is_failed());
    assert!(!LoadState::Loaded(Arc::new(MaterialAsset::default())).is_failed());
    assert!(LoadState::<MaterialAsset>::Failed("err".to_string()).is_failed());
}

#[test]
fn load_handle_new() {
    let handle = LoadHandle::new(LoadState::<MaterialAsset>::Pending);
    assert!(!handle.is_loaded());
    assert!(!handle.is_failed());
}

#[test]
fn load_handle_resolve() {
    let handle = LoadHandle::new(LoadState::<MaterialAsset>::Pending);
    handle.clone().resolve(MaterialAsset::default());
    assert!(handle.is_loaded());
}

#[test]
fn load_handle_fail() {
    let handle = LoadHandle::new(LoadState::<MaterialAsset>::Pending);
    handle.clone().fail("oops".to_string());
    assert!(handle.is_failed());
}

#[test]
fn load_handle_clone() {
    let handle = LoadHandle::new(LoadState::<MaterialAsset>::Pending);
    let cloned = handle.clone();
    assert!(!cloned.is_loaded());
}

#[test]
fn async_load_new() {
    let load = AsyncLoad::<MaterialAsset>::new();
    let handle = load.handle();
    assert!(!handle.is_loaded());
}

#[test]
fn async_load_default() {
    let load: AsyncLoad<MaterialAsset> = Default::default();
    let handle = load.handle();
    assert!(!handle.is_loaded());
}
