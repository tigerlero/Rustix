//! Tests for terrain editor state and modes.

use rustix_terrain::sculpt::BrushMode;
use crate::terrain_editor::*;

#[test]
fn terrain_edit_mode_variants() {
    assert_ne!(TerrainEditMode::Sculpt, TerrainEditMode::Paint);
    assert_ne!(TerrainEditMode::Paint, TerrainEditMode::Vegetation);
    assert_ne!(TerrainEditMode::Vegetation, TerrainEditMode::Smooth);
    assert_ne!(TerrainEditMode::Smooth, TerrainEditMode::Flatten);
}

#[test]
fn terrain_editor_state_new() {
    let state = TerrainEditorState::new();
    assert_eq!(state.mode, TerrainEditMode::Sculpt);
    assert_eq!(state.paint_layer, 0);
    assert_eq!(state.paint_strength, 0.5);
    assert_eq!(state.vegetation_density, 1.0);
}

#[test]
fn terrain_editor_state_default() {
    let state: TerrainEditorState = Default::default();
    assert_eq!(state.mode, TerrainEditMode::Sculpt);
}

#[test]
fn terrain_editor_set_brush_mode() {
    let mut state = TerrainEditorState::new();
    state.set_brush_mode(BrushMode::Raise);
    assert_eq!(state.brush.mode, BrushMode::Raise);
}

#[test]
fn terrain_editor_set_brush_radius() {
    let mut state = TerrainEditorState::new();
    state.set_brush_radius(10.0);
    assert_eq!(state.brush.radius, 10.0);
}

#[test]
fn terrain_editor_set_brush_strength() {
    let mut state = TerrainEditorState::new();
    state.set_brush_strength(0.8);
    assert_eq!(state.brush.strength, 0.8);
}
