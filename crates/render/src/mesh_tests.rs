use super::*;
use rustix_core::math::Vec3;

#[test]
fn aabb_from_cube_vertices() {
    let (verts, _) = procedural::cube(2.0);
    let bytes = bytemuck::cast_slice(&verts);
    let aabb = compute_aabb_from_vertices(bytes);

    assert!((aabb.min - Vec3::new(-1.0, -1.0, -1.0)).length() < 0.0001, "cube min should be (-1,-1,-1)");
    assert!((aabb.max - Vec3::new(1.0, 1.0, 1.0)).length() < 0.0001, "cube max should be (1,1,1)");
}

#[test]
fn aabb_from_uv_sphere_vertices() {
    let (verts, _) = procedural::uv_sphere(1.0, 8, 8);
    let bytes = bytemuck::cast_slice(&verts);
    let aabb = compute_aabb_from_vertices(bytes);

    assert!(aabb.min.x >= -1.0 && aabb.min.x <= -0.99, "sphere min.x should be ~-1");
    assert!(aabb.max.x <= 1.0 && aabb.max.x >= 0.99, "sphere max.x should be ~1");
    assert!(aabb.min.y >= -1.0 && aabb.min.y <= -0.99, "sphere min.y should be ~-1");
    assert!(aabb.max.y <= 1.0 && aabb.max.y >= 0.99, "sphere max.y should be ~1");
}

#[test]
fn aabb_empty_for_no_vertices() {
    let aabb = compute_aabb_from_vertices(&[]);
    assert_eq!(aabb.min, Vec3::ZERO);
    assert_eq!(aabb.max, Vec3::ZERO);
}
