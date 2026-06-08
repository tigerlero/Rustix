//! Tests for physics components and simulation.

use rustix_core::math::Vec3;
use crate::*;

#[test]
fn rigid_body_default() {
    let body = RigidBody::default();
    assert_eq!(body.body_type, BodyType::Dynamic);
    assert_eq!(body.mass, 1.0);
    assert_eq!(body.velocity, Vec3::ZERO);
    assert_eq!(body.angular_velocity, Vec3::ZERO);
    assert_eq!(body.gravity_scale, 1.0);
    assert_eq!(body.drag, 0.0);
    assert_eq!(body.angular_drag, 0.05);
    assert!(body.use_gravity);
    assert!(body.can_sleep);
    assert!(!body.sleeping);
}

#[test]
fn collider_default() {
    let c = Collider::default();
    assert_eq!(c.shape, ColliderShape::Sphere { radius: 0.5 });
    assert!(!c.is_trigger);
    assert_eq!(c.restitution, 0.5);
    assert_eq!(c.friction, 0.5);
}

#[test]
fn physics_material_default() {
    let m = PhysicsMaterial::default();
    assert_eq!(m.static_friction, 0.5);
    assert_eq!(m.dynamic_friction, 0.5);
    assert_eq!(m.restitution, 0.5);
    assert_eq!(m.density, 1.0);
}

#[test]
fn character_controller_default() {
    let cc = CharacterController::default();
    assert_eq!(cc.height, 1.75);
    assert_eq!(cc.radius, 0.5);
    assert_eq!(cc.slope_limit_degrees, 45.0);
    assert_eq!(cc.step_height, 0.3);
    assert!(cc.snap_to_ground);
}

#[test]
fn joint_default() {
    let j = Joint::default();
    assert_eq!(j.joint_type, JointType::Fixed);
    assert_eq!(j.local_anchor1, Vec3::ZERO);
    assert_eq!(j.local_anchor2, Vec3::ZERO);
    assert!(j.contacts_enabled);
}

#[test]
fn physics_world_default() {
    let pw = PhysicsWorld::default();
    assert_eq!(pw.gravity, Vec3::new(0.0, -9.81, 0.0));
    assert_eq!(pw.simulation_speed, 1.0);
    assert_eq!(pw.max_substeps, 4);
}

#[test]
fn aabb_from_sphere() {
    let aabb = PhysicsAabb::from_shape(&ColliderShape::Sphere { radius: 1.0 }, Vec3::ZERO);
    assert_eq!(aabb.min, Vec3::new(-1.0, -1.0, -1.0));
    assert_eq!(aabb.max, Vec3::new(1.0, 1.0, 1.0));
}

#[test]
fn aabb_from_box() {
    let aabb = PhysicsAabb::from_shape(&ColliderShape::Box { half_extents: Vec3::new(1.0, 2.0, 3.0) }, Vec3::ZERO);
    assert_eq!(aabb.min, Vec3::new(-1.0, -2.0, -3.0));
    assert_eq!(aabb.max, Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn aabb_from_capsule() {
    let aabb = PhysicsAabb::from_shape(&ColliderShape::Capsule { radius: 0.5, height: 2.0 }, Vec3::ZERO);
    assert_eq!(aabb.min, Vec3::new(-0.5, -1.5, -0.5));
    assert_eq!(aabb.max, Vec3::new(0.5, 1.5, 0.5));
}

#[test]
fn aabb_intersects_overlapping() {
    let a = PhysicsAabb::from_shape(&ColliderShape::Sphere { radius: 1.0 }, Vec3::ZERO);
    let b = PhysicsAabb::from_shape(&ColliderShape::Sphere { radius: 1.0 }, Vec3::X);
    assert!(a.intersects(&b));
    assert!(b.intersects(&a));
}

#[test]
fn aabb_intersects_separated() {
    let a = PhysicsAabb::from_shape(&ColliderShape::Sphere { radius: 0.5 }, Vec3::ZERO);
    let b = PhysicsAabb::from_shape(&ColliderShape::Sphere { radius: 0.5 }, Vec3::new(3.0, 0.0, 0.0));
    assert!(!a.intersects(&b));
}

#[test]
fn aabb_intersects_touching() {
    let a = PhysicsAabb { min: Vec3::ZERO, max: Vec3::X };
    let b = PhysicsAabb { min: Vec3::X, max: Vec3::new(2.0, 0.0, 0.0) };
    assert!(a.intersects(&b));
}

#[test]
fn step_physics_applies_gravity() {
    let mut world = hecs::World::new();
    let e = world.spawn(());
    let mut bodies = vec![(e, RigidBody::default())];
    let physics = PhysicsWorld::default();
    let results = step_physics(&mut bodies, &physics, 1.0);
    assert_eq!(results.len(), 1);
    assert_eq!(bodies[0].1.velocity, Vec3::new(0.0, -9.81, 0.0));
}

#[test]
fn step_physics_skips_static() {
    let mut world = hecs::World::new();
    let e = world.spawn(());
    let mut body = RigidBody::default();
    body.body_type = BodyType::Static;
    let mut bodies = vec![(e, body)];
    let physics = PhysicsWorld::default();
    let results = step_physics(&mut bodies, &physics, 1.0);
    assert!(results.is_empty());
}

#[test]
fn step_physics_applies_drag() {
    let mut world = hecs::World::new();
    let e = world.spawn(());
    let mut body = RigidBody::default();
    body.velocity = Vec3::X * 10.0;
    body.drag = 1.0;
    let mut bodies = vec![(e, body)];
    let physics = PhysicsWorld { gravity: Vec3::ZERO, simulation_speed: 1.0, max_substeps: 4 };
    step_physics(&mut bodies, &physics, 1.0);
    assert_eq!(bodies[0].1.velocity, Vec3::ZERO);
}

#[test]
fn step_physics_kinematic_no_gravity() {
    let mut world = hecs::World::new();
    let e = world.spawn(());
    let mut body = RigidBody::default();
    body.body_type = BodyType::Kinematic;
    let mut bodies = vec![(e, body)];
    let physics = PhysicsWorld::default();
    step_physics(&mut bodies, &physics, 1.0);
    assert_eq!(bodies[0].1.velocity, Vec3::ZERO);
}
