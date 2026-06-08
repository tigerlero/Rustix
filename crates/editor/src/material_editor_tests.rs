//! Tests for material editor state and properties.

use rustix_core::math::Vec3;
use crate::material_editor::{MaterialProperty, MaterialEditorState};

#[test]
fn material_property_variants() {
    let p1 = MaterialProperty::Albedo(Vec3::X);
    let p2 = MaterialProperty::Roughness(0.5);
    assert_ne!(std::mem::discriminant(&p1), std::mem::discriminant(&p2));
}

#[test]
fn material_property_clone() {
    let p = MaterialProperty::Albedo(Vec3::new(1.0, 0.0, 0.0));
    let cloned = p.clone();
    assert_eq!(p, cloned);
}

#[test]
fn material_editor_state_new() {
    let state = MaterialEditorState::new();
    assert!(state.properties.is_empty());
    assert!(state.selected_slot.is_none());
}

#[test]
fn material_editor_state_default() {
    let state: MaterialEditorState = Default::default();
    assert!(state.properties.is_empty());
}

#[test]
fn material_editor_add_property() {
    let mut state = MaterialEditorState::new();
    state.add_property(MaterialProperty::Albedo(Vec3::Y));
    state.add_property(MaterialProperty::Roughness(0.3));
    assert_eq!(state.properties.len(), 2);
}

#[test]
fn material_editor_set_property() {
    let mut state = MaterialEditorState::new();
    state.add_property(MaterialProperty::Roughness(0.0));
    state.set_property(0, MaterialProperty::Roughness(0.8));
    assert_eq!(state.properties[0], MaterialProperty::Roughness(0.8));
}

#[test]
fn material_editor_set_property_out_of_bounds() {
    let mut state = MaterialEditorState::new();
    state.set_property(5, MaterialProperty::Roughness(0.8));
    assert!(state.properties.is_empty());
}
