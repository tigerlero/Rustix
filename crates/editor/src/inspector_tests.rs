//! Tests for inspector panel state and component descriptions.

use std::any::TypeId;
use crate::inspector::{FieldValue, FieldDesc, ComponentDesc, InspectorState};

#[test]
fn field_value_variants() {
    let f1 = FieldValue::Float(1.0);
    let f2 = FieldValue::Int(1);
    assert_ne!(std::mem::discriminant(&f1), std::mem::discriminant(&f2));
}

#[test]
fn field_value_equality() {
    assert_eq!(FieldValue::Float(1.0), FieldValue::Float(1.0));
    assert_ne!(FieldValue::Float(1.0), FieldValue::Float(2.0));
}

#[test]
fn component_desc_new() {
    let desc = ComponentDesc::new("Position");
    assert_eq!(desc.type_name, "Position");
    assert_eq!(desc.type_id, TypeId::of::<()>());
    assert!(desc.fields.is_empty());
}

#[test]
fn component_desc_with_field() {
    let desc = ComponentDesc::new("Position")
        .with_field("x", FieldValue::Float(1.0))
        .with_field("y", FieldValue::Float(2.0));
    assert_eq!(desc.fields.len(), 2);
    assert_eq!(desc.fields[0].name, "x");
    assert_eq!(desc.fields[0].value, FieldValue::Float(1.0));
}

#[test]
fn inspector_state_new() {
    let state = InspectorState::new();
    assert!(state.components.is_empty());
    assert!(!state.component_menu_open);
}

#[test]
fn inspector_state_default() {
    let state: InspectorState = Default::default();
    assert!(state.components.is_empty());
}

#[test]
fn inspector_set_components() {
    let mut state = InspectorState::new();
    state.set_components(vec![
        ComponentDesc::new("A"),
        ComponentDesc::new("B"),
    ]);
    assert_eq!(state.components.len(), 2);
}

#[test]
fn inspector_add_component() {
    let mut state = InspectorState::new();
    state.add_component(ComponentDesc::new("Velocity"));
    assert_eq!(state.components.len(), 1);
    assert_eq!(state.components[0].type_name, "Velocity");
}

#[test]
fn inspector_remove_component() {
    let mut state = InspectorState::new();
    let tid_a = TypeId::of::<i32>();
    let tid_b = TypeId::of::<f32>();

    let mut a = ComponentDesc::new("A");
    a.type_id = tid_a;
    let mut b = ComponentDesc::new("B");
    b.type_id = tid_b;

    state.add_component(a);
    state.add_component(b);
    assert_eq!(state.components.len(), 2);

    state.remove_component(tid_a);
    assert_eq!(state.components.len(), 1);
    assert_eq!(state.components[0].type_name, "B");
}
