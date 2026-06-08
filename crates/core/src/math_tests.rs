//! Tests for game math utilities (Aabb, Sphere, Plane, Frustum, Ray, Color).

use crate::math::{Aabb, Sphere, Plane, Frustum, Ray, Color, lerp, smoothstep, smootherstep};
use glam::{Vec3, Mat4};

#[test]
fn aabb_from_points() {
    let points = [
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::new(-1.0, 0.0, 4.0),
        Vec3::new(0.0, 5.0, -2.0),
    ];
    let aabb = Aabb::from_points(&points);
    assert_eq!(aabb.min, Vec3::new(-1.0, 0.0, -2.0));
    assert_eq!(aabb.max, Vec3::new(1.0, 5.0, 4.0));
}

#[test]
fn aabb_center_and_extents() {
    let aabb = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 4.0, 6.0));
    assert_eq!(aabb.center(), Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(aabb.extents(), Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(aabb.size(), Vec3::new(2.0, 4.0, 6.0));
}

#[test]
fn aabb_contains() {
    let aabb = Aabb::new(Vec3::ZERO, Vec3::ONE);
    assert!(aabb.contains(Vec3::new(0.5, 0.5, 0.5)));
    assert!(aabb.contains(Vec3::ZERO));
    assert!(aabb.contains(Vec3::ONE));
    assert!(!aabb.contains(Vec3::new(1.5, 0.5, 0.5)));
    assert!(!aabb.contains(Vec3::new(-0.1, 0.5, 0.5)));
}

#[test]
fn aabb_intersects() {
    let a = Aabb::new(Vec3::ZERO, Vec3::ONE);
    let b = Aabb::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.5, 1.5, 1.5));
    let c = Aabb::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(3.0, 3.0, 3.0));
    assert!(a.intersects(&b));
    assert!(b.intersects(&a));
    assert!(!a.intersects(&c));
}

#[test]
fn aabb_union() {
    let a = Aabb::new(Vec3::ZERO, Vec3::ONE);
    let b = Aabb::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(2.0, 2.0, 2.0));
    let u = a.union(&b);
    assert_eq!(u.min, Vec3::ZERO);
    assert_eq!(u.max, Vec3::new(2.0, 2.0, 2.0));
}

#[test]
fn aabb_surface_area_and_volume() {
    let aabb = Aabb::new(Vec3::ZERO, Vec3::new(2.0, 3.0, 4.0));
    assert_eq!(aabb.surface_area(), 2.0 * (2.0 * 3.0 + 3.0 * 4.0 + 4.0 * 2.0));
    assert_eq!(aabb.volume(), 2.0 * 3.0 * 4.0);
}

#[test]
fn sphere_from_points() {
    let points = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, -1.0),
    ];
    let s = Sphere::from_points(&points);
    assert!((s.center - Vec3::ZERO).length() < 1e-4);
    assert!((s.radius - 1.0).abs() < 1e-4);
}

#[test]
fn sphere_contains() {
    let s = Sphere::new(Vec3::ZERO, 1.0);
    assert!(s.contains(Vec3::ZERO));
    assert!(s.contains(Vec3::X));
    assert!(!s.contains(Vec3::new(1.1, 0.0, 0.0)));
}

#[test]
fn sphere_intersects() {
    let a = Sphere::new(Vec3::ZERO, 1.0);
    let b = Sphere::new(Vec3::new(1.5, 0.0, 0.0), 1.0);
    let c = Sphere::new(Vec3::new(3.0, 0.0, 0.0), 1.0);
    assert!(a.intersects(&b));
    assert!(!a.intersects(&c));
}

#[test]
fn plane_distance_to() {
    let plane = Plane::new(Vec3::Y, 0.0);
    assert!((plane.distance_to(Vec3::new(0.0, 2.0, 0.0)) - 2.0).abs() < 1e-4);
    assert!(plane.distance_to(Vec3::ZERO).abs() < 1e-4);
    assert!((plane.distance_to(Vec3::new(0.0, -3.0, 0.0)) + 3.0).abs() < 1e-4);
}

#[test]
fn plane_from_point_normal() {
    let plane = Plane::from_point_normal(Vec3::new(0.0, 5.0, 0.0), Vec3::Y);
    assert!((plane.distance_to(Vec3::new(0.0, 5.0, 0.0))).abs() < 1e-4);
    assert!((plane.distance_to(Vec3::new(0.0, 6.0, 0.0)) - 1.0).abs() < 1e-4);
}

#[test]
fn ray_intersect_aabb_hit() {
    let ray = Ray::new(Vec3::new(-2.0, 0.5, 0.5), Vec3::X);
    let aabb = Aabb::new(Vec3::ZERO, Vec3::ONE);
    let t = ray.intersect_aabb(&aabb).unwrap();
    assert!(t >= 0.0);
    let hit = ray.point_at(t);
    assert!(hit.x >= 0.0 && hit.x <= 1.0);
    assert!(hit.y >= 0.0 && hit.y <= 1.0);
    assert!(hit.z >= 0.0 && hit.z <= 1.0);
}

#[test]
fn ray_intersect_aabb_miss() {
    let ray = Ray::new(Vec3::new(-2.0, 5.0, 0.5), Vec3::X);
    let aabb = Aabb::new(Vec3::ZERO, Vec3::ONE);
    assert!(ray.intersect_aabb(&aabb).is_none());
}

#[test]
fn ray_intersect_plane() {
    let plane = Plane::new(Vec3::Y, 0.0);
    let ray = Ray::new(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y);
    let t = ray.intersect_plane(&plane).unwrap();
    assert!((t - 5.0).abs() < 1e-4);
    assert!(ray.point_at(t).y.abs() < 1e-4);
}

#[test]
fn ray_parallel_to_plane() {
    let plane = Plane::new(Vec3::Y, 0.0);
    let ray = Ray::new(Vec3::ZERO, Vec3::X);
    assert!(ray.intersect_plane(&plane).is_none());
}

#[test]
fn color_linear_srgb_roundtrip() {
    let c = Color::rgb(0.5, 0.5, 0.5);
    let srgb = c.linear_to_srgb();
    let back = srgb.srgb_to_linear();
    assert!((c.r - back.r).abs() < 1e-3);
    assert!((c.g - back.g).abs() < 1e-3);
    assert!((c.b - back.b).abs() < 1e-3);
}

#[test]
fn color_rgba_bytes() {
    let c = Color::rgba_bytes(255, 128, 0, 255);
    assert!((c.r - 1.0).abs() < 1e-4);
    assert!((c.g - 0.50196).abs() < 1e-3);
    assert!(c.b < 1e-4);
    assert!((c.a - 1.0).abs() < 1e-4);
}

#[test]
fn lerp_basic() {
    assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
    assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
    assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
}

#[test]
fn smoothstep_basic() {
    assert_eq!(smoothstep(0.0, 1.0, 0.0), 0.0);
    assert_eq!(smoothstep(0.0, 1.0, 1.0), 1.0);
    assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < 1e-4);
}

#[test]
fn smootherstep_basic() {
    assert_eq!(smootherstep(0.0, 1.0, 0.0), 0.0);
    assert_eq!(smootherstep(0.0, 1.0, 1.0), 1.0);
    assert!((smootherstep(0.0, 1.0, 0.5) - 0.5).abs() < 1e-4);
}
