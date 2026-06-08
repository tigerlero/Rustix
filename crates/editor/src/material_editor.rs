//! Material editor: node graph or property panel.

use rustix_core::math::Vec3;

/// A property in a material property panel.
#[derive(Debug, Clone, PartialEq)]
pub enum MaterialProperty {
    Albedo(Vec3),
    Roughness(f32),
    Metalness(f32),
    NormalStrength(f32),
    Emissive(Vec3),
    EmissiveIntensity(f32),
    TextureSlot(String, String), // (slot_name, asset_path)
}

/// Editable material state.
#[derive(Debug, Clone, Default)]
pub struct MaterialEditorState {
    pub properties: Vec<MaterialProperty>,
    pub selected_slot: Option<String>,
}

impl MaterialEditorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_property(&mut self, prop: MaterialProperty) {
        self.properties.push(prop);
    }

    pub fn set_property(&mut self, index: usize, value: MaterialProperty) {
        if index < self.properties.len() {
            self.properties[index] = value;
        }
    }
}
