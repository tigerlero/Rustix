use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// World-space transform: translation, rotation, scale.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_translation(t: Vec3) -> Self {
        Self { translation: t, ..Default::default() }
    }

    pub fn from_translation_rotation_scale(t: Vec3, yaw: f32, scale: f32) -> Self {
        Self {
            translation: t,
            rotation: Quat::from_rotation_y(yaw),
            scale: Vec3::splat(scale),
        }
    }

    /// Compute the model matrix from this transform.
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

/// Parent component: `None` for a root entity, `Some(entity)` for a child.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parent(pub Option<hecs::Entity>);

impl Default for Parent {
    fn default() -> Self {
        Self(None)
    }
}

/// Cached world-space transform matrix.
/// Written by the hierarchy system each frame.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LocalToWorld {
    pub matrix: Mat4,
}

impl Default for LocalToWorld {
    fn default() -> Self {
        Self {
            matrix: Mat4::IDENTITY,
        }
    }
}

/// Script component attached to entities for behavior logic.
/// Contains inline Rhai script source for simplicity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScriptComponent {
    pub source: String,
    pub enabled: bool,
}
