use rustix_core::math::Vec4;
use super::scene::{Transform};

#[derive(Clone)]
pub enum EditorAction {
    AddEntity(hecs::Entity),
    DeleteEntity { name: String, transform: Transform, mesh: String, material: Vec4, metallic: f32 },
    RenameEntity { entity: hecs::Entity, old_name: String },
    TransformEntity { entity: hecs::Entity, old_transform: Transform },
}

pub struct UndoHistory {
    actions: Vec<EditorAction>,
    index: usize,
    max_actions: usize,
}

impl UndoHistory {
    pub fn new(max: usize) -> Self {
        Self { actions: Vec::with_capacity(max), index: 0, max_actions: max }
    }

    pub fn push(&mut self, action: EditorAction) {
        self.actions.truncate(self.index);
        if self.actions.len() >= self.max_actions {
            self.actions.remove(0);
        }
        self.actions.push(action);
        self.index = self.actions.len();
    }

    pub fn undo(&mut self) -> Option<&EditorAction> {
        if self.index > 0 {
            self.index -= 1;
            Some(&self.actions[self.index])
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<&EditorAction> {
        if self.index < self.actions.len() {
            let action = &self.actions[self.index];
            self.index += 1;
            Some(action)
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool { self.index > 0 }
    pub fn can_redo(&self) -> bool { self.index < self.actions.len() }
}
