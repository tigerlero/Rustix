//! Tests for Rapier3D physics backend.

use rustix_core::math::Vec3;
use rustix_core::ecs::EcsWorld;
use crate::rapier::RapierPhysicsWorld;
use crate::{RigidBody, Collider, ColliderShape, BodyType, Joint, JointType, CharacterController, PhysicsWorld};

fn spawn_entity() -> hecs::Entity {
    let mut world = EcsWorld::new();
    world.spawn(())
}

fn default_body() -> RigidBody {
    RigidBody {
        body_type: BodyType::Dynamic,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        mass: 1.0,
        drag: 0.0,
        angular_drag: 0.0,
        gravity_scale: 1.0,
        use_gravity: true,
        can_sleep: true,
        sleeping: false,
    }
}

fn default_collider() -> Collider {
    Collider {
        shape: ColliderShape::Sphere { radius: 0.5 },
        restitution: 0.5,
        friction: 0.5,
        is_trigger: false,
    }
}

#[test]
fn rapier_world_new() {
    let world = RapierPhysicsWorld::new();
    assert_eq!(world.body_count(), 0);
    assert_eq!(world.collider_count(), 0);
}

#[test]
fn rapier_world_default() {
    let world: RapierPhysicsWorld = Default::default();
    assert_eq!(world.body_count(), 0);
}

#[test]
fn rapier_add_entity() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    assert_eq!(world.body_count(), 1);
    assert_eq!(world.collider_count(), 1);
}

#[test]
fn rapier_remove_entity() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    world.remove_entity(entity);
    assert_eq!(world.body_count(), 0);
    assert_eq!(world.collider_count(), 0);
}

#[test]
fn rapier_transform_of() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::new(1.0, 2.0, 3.0), [0.0; 3]);
    let (pos, _rot) = world.transform_of(entity).unwrap();
    assert!((pos - Vec3::new(1.0, 2.0, 3.0)).length() < 0.01);
}

#[test]
fn rapier_step() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::Y, [0.0; 3]);
    world.step(1.0 / 60.0);
    assert_eq!(world.active_body_count(), 1);
}

#[test]
fn rapier_apply_force() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    world.apply_force(entity, Vec3::Y * 10.0);
}

#[test]
fn rapier_apply_impulse() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    world.apply_impulse(entity, Vec3::X * 5.0);
}

#[test]
fn rapier_set_velocity() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    world.set_velocity(entity, Vec3::Z * 2.0);
}

#[test]
fn rapier_set_angular_velocity() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    world.set_angular_velocity(entity, Vec3::Y * 1.0);
}

#[test]
fn rapier_wake_up_and_is_sleeping() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    let body = RigidBody {
        sleeping: true,
        ..default_body()
    };
    world.add_entity(entity, &body, &default_collider(), Vec3::ZERO, [0.0; 3]);
    assert!(world.is_sleeping(entity));
    world.wake_up(entity, false);
    assert!(!world.is_sleeping(entity));
}

#[test]
fn rapier_raycast_miss() {
    let world = RapierPhysicsWorld::new();
    let hit = world.raycast(Vec3::ZERO, Vec3::X, 100.0);
    assert!(hit.is_none());
}

#[test]
fn rapier_add_character() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    world.add_character(entity, &CharacterController::default());
}

#[test]
fn rapier_remove_character() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    world.add_character(entity, &CharacterController::default());
    world.remove_character(entity);
}

#[test]
fn rapier_joint_add_and_remove() {
    let mut world = RapierPhysicsWorld::new();
    let e1 = spawn_entity();
    let e2 = spawn_entity();
    world.add_entity(e1, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    world.add_entity(e2, &default_body(), &default_collider(), Vec3::X, [0.0; 3]);

    let joint = Joint {
        connected_entity: e2,
        joint_type: JointType::Fixed,
        local_anchor1: Vec3::ZERO,
        local_anchor2: Vec3::ZERO,
        contacts_enabled: true,
    };

    let handle = world.add_joint(e1, &joint);
    assert!(handle.is_some());
    assert_eq!(world.joint_count(), 1);

    world.remove_joint(e1);
    assert_eq!(world.joint_count(), 0);
}

#[test]
fn rapier_snapshot_roundtrip() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::Y, [0.0; 3]);

    let snapshot = world.save_snapshot();
    let mut world2 = RapierPhysicsWorld::new();
    world2.restore_snapshot(&snapshot);
    assert_eq!(world2.body_count(), 1);
}

#[test]
fn rapier_snapshot_serialize_roundtrip() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::Y, [0.0; 3]);

    let snapshot = world.save_snapshot();
    let bytes = RapierPhysicsWorld::serialize_snapshot(&snapshot).unwrap();
    let restored = RapierPhysicsWorld::deserialize_snapshot(&bytes).unwrap();

    let mut world2 = RapierPhysicsWorld::new();
    world2.restore_snapshot(&restored);
    assert_eq!(world2.body_count(), 1);
}

#[test]
fn rapier_debug_draw() {
    let mut world = RapierPhysicsWorld::new();
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::ZERO, [0.0; 3]);
    let lines = world.debug_draw();
    // Should produce at least some debug lines for the sphere collider
    assert!(!lines.is_empty());
}

#[test]
fn rapier_configure_gravity() {
    let mut world = RapierPhysicsWorld::new();
    let settings = PhysicsWorld {
        gravity: Vec3::new(0.0, -5.0, 0.0),
        simulation_speed: 1.0,
        max_substeps: 4,
    };
    world.configure(&settings);
    let entity = spawn_entity();
    world.add_entity(entity, &default_body(), &default_collider(), Vec3::Y, [0.0; 3]);
    world.step(1.0 / 60.0);
}
