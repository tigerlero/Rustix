//! Tests for lighting editor state and editable lights.

use rustix_core::math::Vec3;
use crate::lighting_editor::*;

#[test]
fn editable_light_type_variants() {
    assert_ne!(EditableLightType::Directional, EditableLightType::Point);
    assert_ne!(EditableLightType::Point, EditableLightType::Spot);
    assert_ne!(EditableLightType::Spot, EditableLightType::Area);
}

#[test]
fn editable_light_new() {
    let light = EditableLight::new("Sun", EditableLightType::Directional);
    assert_eq!(light.name, "Sun");
    assert_eq!(light.light_type, EditableLightType::Directional);
    assert_eq!(light.position, Vec3::ZERO);
    assert_eq!(light.color, Vec3::ONE);
    assert_eq!(light.intensity, 1.0);
    assert_eq!(light.range, 10.0);
    assert_eq!(light.spot_angle_deg, 45.0);
    assert!(light.cast_shadows);
}

#[test]
fn editable_light_point_constructor() {
    let light = EditableLight::point("Lamp");
    assert_eq!(light.name, "Lamp");
    assert_eq!(light.light_type, EditableLightType::Point);
}

#[test]
fn editable_light_directional_constructor() {
    let light = EditableLight::directional("Sun");
    assert_eq!(light.name, "Sun");
    assert_eq!(light.light_type, EditableLightType::Directional);
}

#[test]
fn ibl_probe_default() {
    let probe = IblProbe {
        position: Vec3::new(1.0, 2.0, 3.0),
        radius: 5.0,
        cubemap_path: Some("sky.hdr".to_string()),
    };
    assert_eq!(probe.radius, 5.0);
}

#[test]
fn lighting_editor_state_new() {
    let state = LightingEditorState::new();
    assert!(state.lights.is_empty());
    assert!(state.selected_light.is_none());
    assert!(!state.bake_in_progress);
}

#[test]
fn lighting_editor_add_light() {
    let mut state = LightingEditorState::new();
    let light = EditableLight::point("Lamp");
    let idx = state.add_light(light);
    assert_eq!(idx, 0);
    assert_eq!(state.lights.len(), 1);
    assert_eq!(state.selected_light, Some(0));
}

#[test]
fn lighting_editor_remove_light() {
    let mut state = LightingEditorState::new();
    state.add_light(EditableLight::point("A"));
    state.add_light(EditableLight::point("B"));
    state.remove_light(0);
    assert_eq!(state.lights.len(), 1);
    assert_eq!(state.selected_light, None);
}

#[test]
fn lighting_editor_remove_light_out_of_bounds() {
    let mut state = LightingEditorState::new();
    state.add_light(EditableLight::point("A"));
    state.remove_light(10);
    assert_eq!(state.lights.len(), 1);
}

#[test]
fn lighting_editor_add_ibl_probe() {
    let mut state = LightingEditorState::new();
    state.add_ibl_probe(IblProbe {
        position: Vec3::ZERO,
        radius: 10.0,
        cubemap_path: None,
    });
    assert_eq!(state.ibl_probes.len(), 1);
}
