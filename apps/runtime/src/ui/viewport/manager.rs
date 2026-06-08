use crate::camera::EditorCamera;

pub const MAX_VIEWPORTS: usize = 4;
pub const PRIMARY_VIEWPORT: usize = 0;

/// Per-viewport texture IDs for offscreen rendering.
pub fn viewport_texture_id(index: usize) -> egui::TextureId {
    egui::TextureId::User(index as u64)
}

/// A single editor viewport with its own camera.
#[derive(Clone)]
pub struct Viewport {
    pub camera: EditorCamera,
    pub name: String,
    pub open: bool,
    pub is_primary: bool,
}

impl Viewport {
    pub fn new(name: &str, is_primary: bool) -> Self {
        Self {
            camera: EditorCamera::new(),
            name: name.to_string(),
            open: true,
            is_primary,
        }
    }
}

/// Manages up to `MAX_VIEWPORTS` viewports. Viewport 0 is always the primary.
pub struct ViewportManager {
    pub viewports: Vec<Viewport>,
}

impl ViewportManager {
    pub fn new() -> Self {
        let mut viewports = Vec::with_capacity(MAX_VIEWPORTS);
        viewports.push(Viewport::new("Viewport", true));
        Self { viewports }
    }

    pub fn primary_camera_mut(&mut self) -> &mut EditorCamera {
        &mut self.viewports[PRIMARY_VIEWPORT].camera
    }

    pub fn primary_camera(&self) -> &EditorCamera {
        &self.viewports[PRIMARY_VIEWPORT].camera
    }

    pub fn add_viewport(&mut self) -> Option<usize> {
        if self.viewports.len() >= MAX_VIEWPORTS {
            return None;
        }
        let idx = self.viewports.len();
        let name = match idx {
            1 => "Viewport 2".to_string(),
            2 => "Viewport 3".to_string(),
            3 => "Viewport 4".to_string(),
            _ => format!("Viewport {}", idx + 1),
        };
        self.viewports.push(Viewport::new(&name, false));
        Some(idx)
    }

    pub fn remove_viewport(&mut self, index: usize) {
        if index > PRIMARY_VIEWPORT && index < self.viewports.len() {
            self.viewports.remove(index);
        }
    }
}
