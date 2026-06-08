//! Tests for editor metadata and state.

use hecs::World;
use crate::editor_meta::{EditorMetadata, EditorState, EditorLayer, GizmoMode};

fn make_entities(count: usize) -> (World, Vec<hecs::Entity>) {
    let mut world = World::new();
    let entities: Vec<_> = (0..count).map(|_| world.spawn(())).collect();
    (world, entities)
}

#[test]
fn editor_metadata_default() {
    let meta = EditorMetadata::default();
    assert!(meta.visible);
    assert!(!meta.locked);
    assert!(!meta.selected);
    assert_eq!(meta.layer, EditorLayer::Default);
    assert_eq!(meta.gizmo_mode, GizmoMode::Translate);
}

#[test]
fn editor_state_new_is_empty() {
    let state = EditorState::new();
    assert!(state.selected_entities.is_empty());
    assert!(state.visible_layers.is_empty());
    assert!(!state.snap_enabled);
    assert_eq!(state.snap_size, 0.0);
}

#[test]
fn editor_state_select() {
    let mut state = EditorState::new();
    let (_world, entities) = make_entities(1);
    let e = entities[0];
    state.select(e);
    assert_eq!(state.selected_entities.len(), 1);
    assert_eq!(state.selected_entities[0], e);
}

#[test]
fn editor_state_select_deduplicates() {
    let mut state = EditorState::new();
    let (_world, entities) = make_entities(1);
    let e = entities[0];
    state.select(e);
    state.select(e);
    assert_eq!(state.selected_entities.len(), 1);
}

#[test]
fn editor_state_deselect() {
    let mut state = EditorState::new();
    let (_world, entities) = make_entities(2);
    let e1 = entities[0];
    let e2 = entities[1];
    state.select(e1);
    state.select(e2);
    state.deselect(e1);
    assert_eq!(state.selected_entities.len(), 1);
    assert_eq!(state.selected_entities[0], e2);
}

#[test]
fn editor_state_clear_selection() {
    let mut state = EditorState::new();
    let (_world, entities) = make_entities(2);
    state.select(entities[0]);
    state.select(entities[1]);
    state.clear_selection();
    assert!(state.selected_entities.is_empty());
}

#[test]
fn editor_state_toggle_layer() {
    let mut state = EditorState::new();
    state.toggle_layer(EditorLayer::Gizmos);
    assert!(state.is_layer_visible(EditorLayer::Gizmos));
    state.toggle_layer(EditorLayer::Gizmos);
    assert!(!state.is_layer_visible(EditorLayer::Gizmos));
}

#[test]
fn editor_state_is_layer_visible_default_false() {
    let state = EditorState::new();
    assert!(!state.is_layer_visible(EditorLayer::Default));
}
