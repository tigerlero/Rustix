//! Inspector panel: component fields, add/remove components.

use std::any::{Any, TypeId};

/// A serializable field value for the inspector.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
    Color([f32; 4]),
}

/// Description of a single component field.
#[derive(Debug, Clone)]
pub struct FieldDesc {
    pub name: String,
    pub value: FieldValue,
}

/// Description of a component for the inspector UI.
#[derive(Debug, Clone)]
pub struct ComponentDesc {
    pub type_id: TypeId,
    pub type_name: String,
    pub fields: Vec<FieldDesc>,
}

impl ComponentDesc {
    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_id: TypeId::of::<()>(),
            type_name: type_name.into(),
            fields: Vec::new(),
        }
    }

    pub fn with_field(mut self, name: impl Into<String>, value: FieldValue) -> Self {
        self.fields.push(FieldDesc {
            name: name.into(),
            value,
        });
        self
    }
}

/// Inspector data for a selected entity.
#[derive(Debug, Clone, Default)]
pub struct InspectorState {
    pub components: Vec<ComponentDesc>,
    pub component_menu_open: bool,
}

impl InspectorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_components(&mut self, components: Vec<ComponentDesc>) {
        self.components = components;
    }

    pub fn add_component(&mut self, desc: ComponentDesc) {
        self.components.push(desc);
    }

    pub fn remove_component(&mut self, type_id: TypeId) {
        self.components.retain(|c| c.type_id != type_id);
    }
}
