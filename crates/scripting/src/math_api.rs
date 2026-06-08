//! Math API exposed to scripts: `vec3`, `quat`, `lerp`, `dot`, `cross`.

use rustix_core::math::{Vec3, Quat};

/// Create a Vec3 from x, y, z components.
pub fn vec3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Linear interpolation between two floats.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Dot product of two Vec3s.
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a.dot(b)
}

/// Cross product of two Vec3s.
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    a.cross(b)
}

/// Normalize a Vec3.
pub fn normalize(v: Vec3) -> Vec3 {
    v.normalize_or_zero()
}

/// Distance between two Vec3s.
pub fn distance(a: Vec3, b: Vec3) -> f32 {
    a.distance(b)
}

/// Create a Quat from euler angles (yaw, pitch, roll) in degrees.
pub fn quat_from_euler(yaw: f32, pitch: f32, roll: f32) -> Quat {
    Quat::from_euler(
        glam::EulerRot::YXZ,
        yaw.to_radians(),
        pitch.to_radians(),
        roll.to_radians(),
    )
}
