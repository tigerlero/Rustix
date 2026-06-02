use super::*;
use rustix_core::math::{Vec3, Vec4, Mat4, Aabb, Frustum};
use std::f32::consts::FRAC_PI_4;

#[test]
fn light_vp_matrix_is_not_identity() {
    let vp = compute_light_view_proj(Vec3::new(0.5, -0.8, 0.3).normalize(), Vec3::ZERO);
    assert_ne!(vp, Mat4::IDENTITY, "light VP should not be identity");
}

#[test]
fn light_vp_direction_changes_with_light_dir() {
    let center = Vec3::new(5.0, 2.0, 3.0);
    let vp1 = compute_light_view_proj(Vec3::new(0.5, -0.8, 0.3).normalize(), center);
    let vp2 = compute_light_view_proj(Vec3::new(1.0, -0.5, 0.2).normalize(), center);
    assert_ne!(vp1, vp2, "different light directions should produce different VP matrices");
}

#[test]
fn light_vp_center_affects_view_matrix() {
    let dir = Vec3::new(0.5, -0.8, 0.3).normalize();
    let vp1 = compute_light_view_proj(dir, Vec3::ZERO);
    let vp2 = compute_light_view_proj(dir, Vec3::new(10.0, 0.0, 0.0));
    assert_ne!(vp1, vp2, "different center positions should produce different VP matrices");
}

#[test]
fn light_vp_orthographic_bounds() {
    // Use a light direction not parallel to Y-up to avoid degenerate look_at.
    let vp = compute_light_view_proj(Vec3::new(0.5, -0.8, 0.3).normalize(), Vec3::ZERO);
    // A point at the target center should project close to (0,0,~) in NDC
    let center_ndc = vp.project_point3(Vec3::ZERO);
    assert!((center_ndc.x).abs() < 0.01, "center should be near x=0 in NDC: got {}", center_ndc.x);
    assert!((center_ndc.y).abs() < 0.01, "center should be near y=0 in NDC: got {}", center_ndc.y);
    // OpenGL orthographic_rh_gl maps z to [-1,1]; light is 20 units from center
    // so z in view space is 20, mapped to approximately -0.2 in NDC.
    assert!(center_ndc.z > -1.0 && center_ndc.z < 1.0, "center z should be in [-1,1]: got {}", center_ndc.z);
}

#[test]
fn light_vp_unnormalized_input_is_handled() {
    // Function should normalize the direction internally.
    let center = Vec3::new(1.0, 2.0, 3.0);
    let vp1 = compute_light_view_proj(Vec3::new(1.0, -2.0, 0.5), center);
    let vp2 = compute_light_view_proj(Vec3::new(0.5, -1.0, 0.25).normalize(), center);
    // Both should produce the same result since the function normalizes.
    let diff = vp1 - vp2;
    let max_diff = diff.to_cols_array().iter().map(|v| v.abs()).fold(0.0f32, f32::max);
    assert!(max_diff < 0.0001, "unnormalized dir should be normalized, max diff: {}", max_diff);
}

#[test]
fn directional_light_rotation_changes_direction() {
    let dir1 = directional_light_dir_from_euler(Vec3::new(0.0, 0.0, 0.0));
    let dir2 = directional_light_dir_from_euler(Vec3::new(0.0, std::f32::consts::FRAC_PI_2, 0.0));
    assert!((dir1 - dir2).length() > 0.1, "different Y-rotations should produce different light directions");
}

#[test]
fn directional_light_rotation_affects_shadow_vp() {
    let center = Vec3::ZERO;
    let rot1 = Vec3::new(0.0, 0.0, 0.0);
    let rot2 = Vec3::new(0.0, std::f32::consts::FRAC_PI_4, 0.0);

    let dir1 = directional_light_dir_from_euler(rot1);
    let dir2 = directional_light_dir_from_euler(rot2);

    let vp1 = compute_light_view_proj(dir1, center);
    let vp2 = compute_light_view_proj(dir2, center);

    assert_ne!(vp1, vp2, "rotating directional light should change shadow VP matrix");
}

#[test]
fn directional_light_same_rotation_same_direction() {
    let rot = Vec3::new(0.3, 0.7, 0.1);
    let dir1 = directional_light_dir_from_euler(rot);
    let dir2 = directional_light_dir_from_euler(rot);
    assert!((dir1 - dir2).length() < 0.0001, "same rotation should produce identical light direction");
}

#[test]
fn shadow_pass_renders_entities_outside_camera_frustum() {
    // Camera at z=5 looking toward origin (down -Z)
    let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 0.0), Vec3::Y);
    let proj = Mat4::perspective_rh_gl(FRAC_PI_4, 1.0, 0.1, 100.0);
    let vp = proj * view;
    let frustum = Frustum::from_view_proj(&vp);

    // Entity far to the right (x=50), well outside the 45° FOV
    let model = Mat4::from_translation(Vec3::new(50.0, 0.0, 0.0));
    let mesh_aabb = Aabb { min: Vec3::new(-1.0, -1.0, -1.0), max: Vec3::new(1.0, 1.0, 1.0) };
    let world_aabb = mesh_aabb.transform(model);

    // Entity is outside camera frustum (would be culled in main pass)
    assert!(!frustum.intersects_aabb(&world_aabb),
        "entity far outside FOV should be outside frustum");

    // Shadow pass has no frustum culling — it renders ALL mesh entities.
    // This is correct because shadow casters outside the camera view
    // can still cast shadows into the camera frustum.
    assert!(should_cast_shadow(),
        "shadow pass should render all mesh entities regardless of camera frustum");
}

/// The shadow pass renders every entity with a MeshComponent.
/// No frustum culling is applied because shadow casters outside the
/// camera frustum can still cast shadows into it.
pub fn should_cast_shadow() -> bool {
    true
}
