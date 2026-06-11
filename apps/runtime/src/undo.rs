use rustix_render::{DirectionalLight, PointLight, SpotLight, Camera};
use rustix_audio::{AudioSource, AudioListener};
use rustix_scripting::ScriptComponent;
use rustix_physics::{RigidBody, Collider};
use super::scene::{Transform, Material, MeshComponent, SceneEntity};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Subsystem {
    Transform,
    Hierarchy,
    Rendering,
    Audio,
    Physics,
    Scripting,
    All,
}

#[derive(Clone)]
pub enum EditorAction {
    AddEntity { entity: hecs::Entity, snapshot: SceneEntity },
    DeleteEntity { entity: hecs::Entity, snapshot: SceneEntity },
    RenameEntity { entity: hecs::Entity, old_name: String },
    TransformEntity { entity: hecs::Entity, old_transform: Transform },
    DirectionalLightChanged { entity: hecs::Entity, old: DirectionalLight },
    PointLightChanged { entity: hecs::Entity, old: PointLight },
    SpotLightChanged { entity: hecs::Entity, old: SpotLight },
    MaterialChanged { entity: hecs::Entity, old: Material },
    AudioSourceChanged { entity: hecs::Entity, old: AudioSource },
    ScriptComponentChanged { entity: hecs::Entity, old: ScriptComponent },
    RigidBodyChanged { entity: hecs::Entity, old: RigidBody },
    ColliderChanged { entity: hecs::Entity, old: Collider },
    MeshComponentChanged { entity: hecs::Entity, old: MeshComponent },
    AudioListenerChanged { entity: hecs::Entity, old: AudioListener },
    CameraChanged { entity: hecs::Entity, old: Camera },
    ParentChanged { entity: hecs::Entity, old_parent: Option<hecs::Entity>, new_parent: Option<hecs::Entity> },
    ComponentAdded { entity: hecs::Entity, component: String, old_snapshot: SceneEntity },
    ComponentRemoved { entity: hecs::Entity, component: String, old_snapshot: SceneEntity },
    Compound { name: String, actions: Vec<EditorAction> },
}

impl EditorAction {
    pub fn subsystem(&self) -> Subsystem {
        match self {
            EditorAction::TransformEntity { .. } => Subsystem::Transform,
            EditorAction::AddEntity { .. } | EditorAction::DeleteEntity { .. }
            | EditorAction::RenameEntity { .. } | EditorAction::ParentChanged { .. }
            | EditorAction::ComponentAdded { .. } | EditorAction::ComponentRemoved { .. } => Subsystem::Hierarchy,
            EditorAction::DirectionalLightChanged { .. } | EditorAction::PointLightChanged { .. }
            | EditorAction::SpotLightChanged { .. } | EditorAction::MaterialChanged { .. }
            | EditorAction::MeshComponentChanged { .. } | EditorAction::CameraChanged { .. } => Subsystem::Rendering,
            EditorAction::AudioSourceChanged { .. } | EditorAction::AudioListenerChanged { .. } => Subsystem::Audio,
            EditorAction::RigidBodyChanged { .. } | EditorAction::ColliderChanged { .. } => Subsystem::Physics,
            EditorAction::ScriptComponentChanged { .. } => Subsystem::Scripting,
            EditorAction::Compound { actions, .. } => {
                actions.iter().map(|a| a.subsystem()).reduce(|a, b| if a == b { a } else { Subsystem::All }).unwrap_or(Subsystem::All)
            }
        }
    }
}

pub struct UndoHistory {
    pub(crate) actions: Vec<EditorAction>,
    pub(crate) index: usize,
    max_actions: usize,
    pending_compound: Option<(String, Vec<EditorAction>)>,
    compound_depth: usize,
}

impl UndoHistory {
    pub fn new(max: usize) -> Self {
        Self { actions: Vec::with_capacity(max), index: 0, max_actions: max, pending_compound: None, compound_depth: 0 }
    }

    pub fn begin_compound(&mut self, name: &str) {
        self.compound_depth += 1;
        if self.compound_depth == 1 {
            self.pending_compound = Some((name.to_string(), Vec::new()));
        }
    }

    pub fn end_compound(&mut self) {
        self.compound_depth = self.compound_depth.saturating_sub(1);
        if self.compound_depth == 0 {
            if let Some((name, actions)) = self.pending_compound.take() {
                if !actions.is_empty() {
                    self.push_direct(EditorAction::Compound { name, actions });
                }
            }
        }
    }

    fn push_direct(&mut self, action: EditorAction) {
        self.actions.truncate(self.index);
        if self.actions.len() >= self.max_actions {
            self.actions.remove(0);
            if self.index > 0 { self.index -= 1; }
        }
        self.actions.push(action);
        self.index = self.actions.len();
    }

    pub fn push(&mut self, action: EditorAction) {
        if self.compound_depth > 0 {
            if let Some((_, ref mut actions)) = self.pending_compound {
                actions.push(action);
                return;
            }
        }
        self.push_direct(action);
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

    pub fn undo_subsystem(&mut self, subsystem: Subsystem) -> Option<&EditorAction> {
        if self.index == 0 { return None; }
        for i in (0..self.index).rev() {
            let action = &self.actions[i];
            if action.subsystem() == subsystem || subsystem == Subsystem::All {
                self.index = i;
                return Some(action);
            }
        }
        None
    }

    pub fn redo_subsystem(&mut self, subsystem: Subsystem) -> Option<&EditorAction> {
        if self.index >= self.actions.len() { return None; }
        for i in self.index..self.actions.len() {
            let action = &self.actions[i];
            if action.subsystem() == subsystem || subsystem == Subsystem::All {
                self.index = i + 1;
                return Some(action);
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool { self.index > 0 }
    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool { self.index < self.actions.len() }

    pub fn can_undo_subsystem(&self, subsystem: Subsystem) -> bool {
        if subsystem == Subsystem::All { return self.can_undo(); }
        self.actions[..self.index].iter().any(|a| a.subsystem() == subsystem)
    }

    pub fn can_redo_subsystem(&self, subsystem: Subsystem) -> bool {
        if subsystem == Subsystem::All { return self.can_redo(); }
        self.actions[self.index..].iter().any(|a| a.subsystem() == subsystem)
    }

    pub fn clear(&mut self) {
        self.actions.clear();
        self.index = 0;
        self.pending_compound = None;
        self.compound_depth = 0;
    }
}
