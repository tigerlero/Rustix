use rustix_render::{DirectionalLight, PointLight, SpotLight, Camera};
use rustix_audio::{AudioSource, AudioListener};
use rustix_scripting::ScriptComponent;
use rustix_physics::{RigidBody, Collider};
use super::scene::{Transform, Material, MeshComponent, SceneEntity};

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
}

pub struct UndoHistory {
    pub(crate) actions: Vec<EditorAction>,
    pub(crate) index: usize,
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

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool { self.index > 0 }
    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool { self.index < self.actions.len() }
}
