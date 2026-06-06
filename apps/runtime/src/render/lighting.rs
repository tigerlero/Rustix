use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4, Quat, EulerRot};
use rustix_render::{PointLight, SpotLight};
use crate::scene::Transform;

/// Compute the light view-projection matrix for shadow mapping.
/// The light is placed behind the target center looking toward it.
pub fn compute_light_view_proj(light_dir: Vec3, center: Vec3) -> Mat4 {
    let light_dir = light_dir.normalize();
    let light_pos = center - light_dir * 20.0;
    let light_view = Mat4::look_at_rh(light_pos, center, Vec3::Y);
    let light_proj = Mat4::orthographic_rh_gl(-15.0, 15.0, -15.0, 15.0, 0.1, 50.0);
    light_proj * light_view
}

/// Compute directional light direction from euler rotation (XYZ order).
/// The light points along -Z in local space, rotated by the given euler angles.
pub fn directional_light_dir_from_euler(rotation: Vec3) -> Vec3 {
    let rot = Quat::from_euler(EulerRot::XYZ, rotation.x, rotation.y, rotation.z);
    (rot * Vec3::NEG_Z).normalize()
}

/// Collect point/spot lights from the ECS world, sorted by intensity descending.
pub fn collect_lights(ecs_world: &EcsWorld) -> Vec<(Vec3, f32, Vec3, f32)> {
    let mut lights: Vec<(Vec3, f32, Vec3, f32)> = Vec::new();
    for (_e, pl, xform) in ecs_world.query::<(Entity, &PointLight, &Transform)>().iter() {
        lights.push((
            xform.position,
            pl.radius.max(0.1),
            Vec3::new(pl.color.x * pl.intensity, pl.color.y * pl.intensity, pl.color.z * pl.intensity),
            pl.intensity,
        ));
    }
    for (_e, sl, xform) in ecs_world.query::<(Entity, &SpotLight, &Transform)>().iter() {
        lights.push((
            xform.position,
            sl.radius.max(0.1),
            Vec3::new(sl.color.x * sl.intensity, sl.color.y * sl.intensity, sl.color.z * sl.intensity),
            sl.intensity,
        ));
    }
    lights.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
    lights
}
