//! Lighting editor: place lights, bake lightmaps, IBL probes.

use rustix_core::math::{Vec3, Vec4};

/// Editable light types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditableLightType {
    Directional,
    Point,
    Spot,
    Area,
}

/// An editable light instance.
#[derive(Debug, Clone, PartialEq)]
pub struct EditableLight {
    pub name: String,
    pub light_type: EditableLightType,
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub spot_angle_deg: f32,
    pub cast_shadows: bool,
}

impl EditableLight {
    pub fn new(name: impl Into<String>, light_type: EditableLightType) -> Self {
        Self {
            name: name.into(),
            light_type,
            position: Vec3::ZERO,
            direction: Vec3::NEG_Z,
            color: Vec3::ONE,
            intensity: 1.0,
            range: 10.0,
            spot_angle_deg: 45.0,
            cast_shadows: true,
        }
    }

    pub fn point(name: impl Into<String>) -> Self {
        Self::new(name, EditableLightType::Point)
    }

    pub fn directional(name: impl Into<String>) -> Self {
        Self::new(name, EditableLightType::Directional)
    }
}

/// IBL probe configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct IblProbe {
    pub position: Vec3,
    pub radius: f32,
    pub cubemap_path: Option<String>,
}

/// Lighting editor state.
#[derive(Debug, Clone, Default)]
pub struct LightingEditorState {
    pub lights: Vec<EditableLight>,
    pub selected_light: Option<usize>,
    pub ibl_probes: Vec<IblProbe>,
    pub bake_in_progress: bool,
}

impl LightingEditorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_light(&mut self, light: EditableLight) -> usize {
        let idx = self.lights.len();
        self.lights.push(light);
        self.selected_light = Some(idx);
        idx
    }

    pub fn remove_light(&mut self, index: usize) {
        if index < self.lights.len() {
            self.lights.remove(index);
            self.selected_light = None;
        }
    }

    pub fn add_ibl_probe(&mut self, probe: IblProbe) {
        self.ibl_probes.push(probe);
    }
}
