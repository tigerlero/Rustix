//! Editor-only metadata: gizmos, layer visibility, selection.

use hecs::Entity;
use serde::{Serialize, Deserialize};

/// Editor visibility layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorLayer {
    Default,
    Gizmos,
    UI,
    Terrain,
    Vegetation,
    Custom(u32),
}

/// Per-entity editor metadata (stripped in release builds).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorMetadata {
    pub visible: bool,
    pub locked: bool,
    pub layer: EditorLayer,
    pub selected: bool,
    pub gizmo_mode: GizmoMode,
}

impl Default for EditorMetadata {
    fn default() -> Self {
        Self {
            visible: true,
            locked: false,
            layer: EditorLayer::Default,
            selected: false,
            gizmo_mode: GizmoMode::Translate,
        }
    }
}

impl EditorMetadata {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Active transform gizmo mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

/// Global editor state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EditorState {
    pub selected_entities: Vec<Entity>,
    pub visible_layers: Vec<EditorLayer>,
    pub snap_enabled: bool,
    pub snap_size: f32,
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select(&mut self, entity: Entity) {
        if !self.selected_entities.contains(&entity) {
            self.selected_entities.push(entity);
        }
    }

    pub fn deselect(&mut self, entity: Entity) {
        self.selected_entities.retain(|&e| e != entity);
    }

    pub fn clear_selection(&mut self) {
        self.selected_entities.clear();
    }

    pub fn is_layer_visible(&self, layer: EditorLayer) -> bool {
        self.visible_layers.contains(&layer)
    }

    pub fn toggle_layer(&mut self, layer: EditorLayer) {
        if let Some(pos) = self.visible_layers.iter().position(|&l| l == layer) {
            self.visible_layers.remove(pos);
        } else {
            self.visible_layers.push(layer);
        }
    }
}
