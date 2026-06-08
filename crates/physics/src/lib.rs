use rustix_core::math::Vec3;
use std::collections::HashMap;

/// Type of rigid body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BodyType {
    Static,
    Dynamic,
    Kinematic,
}

impl Default for BodyType {
    fn default() -> Self { BodyType::Dynamic }
}

/// Rigid body component for physics simulation.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RigidBody {
    pub body_type: BodyType,
    pub mass: f32,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub gravity_scale: f32,
    pub drag: f32,
    pub angular_drag: f32,
    pub use_gravity: bool,
    pub can_sleep: bool,
    pub sleeping: bool,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self {
            body_type: BodyType::Dynamic,
            mass: 1.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            gravity_scale: 1.0,
            drag: 0.0,
            angular_drag: 0.05,
            use_gravity: true,
            can_sleep: true,
            sleeping: false,
        }
    }
}

/// Collider shape types.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ColliderShape {
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
    Capsule { radius: f32, height: f32 },
}

/// Collider component defining collision geometry.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Collider {
    pub shape: ColliderShape,
    pub is_trigger: bool,
    pub restitution: f32,
    pub friction: f32,
}

impl Default for Collider {
    fn default() -> Self {
        Self {
            shape: ColliderShape::Sphere { radius: 0.5 },
            is_trigger: false,
            restitution: 0.5,
            friction: 0.5,
        }
    }
}

impl Collider {
    /// Apply a `PhysicsMaterialAsset` to this collider, overriding its
    /// restitution and friction values.
    pub fn apply_material(&mut self, material: &rustix_asset::physics::PhysicsMaterialAsset) {
        self.restitution = material.restitution;
        self.friction = material.dynamic_friction;
    }
}

/// Shared physics surface properties that can be referenced by multiple colliders.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PhysicsMaterial {
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub restitution: f32,
    pub density: f32,
}

impl PhysicsMaterial {
    /// Build a runtime `PhysicsMaterial` from an asset definition.
    pub fn from_asset(asset: &rustix_asset::physics::PhysicsMaterialAsset) -> Self {
        Self {
            static_friction: asset.static_friction,
            dynamic_friction: asset.dynamic_friction,
            restitution: asset.restitution,
            density: asset.density,
        }
    }
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self {
            static_friction: 0.5,
            dynamic_friction: 0.5,
            restitution: 0.5,
            density: 1.0,
        }
    }
}

/// Character controller component for kinematic capsule movement.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CharacterController {
    pub height: f32,
    pub radius: f32,
    pub slope_limit_degrees: f32,
    pub step_height: f32,
    pub snap_to_ground: bool,
}

impl Default for CharacterController {
    fn default() -> Self {
        Self {
            height: 1.75,
            radius: 0.5,
            slope_limit_degrees: 45.0,
            step_height: 0.3,
            snap_to_ground: true,
        }
    }
}

/// Types of joints that can connect two rigid bodies.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum JointType {
    Fixed,
    Revolute { axis: Vec3 },
    Spherical,
    Prismatic { axis: Vec3 },
}

/// Joint component linking this entity to another rigid body.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Joint {
    pub joint_type: JointType,
    pub connected_entity: hecs::Entity,
    pub local_anchor1: Vec3,
    pub local_anchor2: Vec3,
    pub contacts_enabled: bool,
}

impl Default for Joint {
    fn default() -> Self {
        Self {
            joint_type: JointType::Fixed,
            connected_entity: hecs::Entity::DANGLING,
            local_anchor1: Vec3::ZERO,
            local_anchor2: Vec3::ZERO,
            contacts_enabled: true,
        }
    }
}

/// Simple AABB used for broad-phase collision detection.
#[derive(Debug, Clone, Copy)]
pub struct PhysicsAabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl PhysicsAabb {
    pub fn from_shape(shape: &ColliderShape, center: Vec3) -> Self {
        match *shape {
            ColliderShape::Sphere { radius } => Self {
                min: center - Vec3::splat(radius),
                max: center + Vec3::splat(radius),
            },
            ColliderShape::Box { half_extents } => Self {
                min: center - half_extents,
                max: center + half_extents,
            },
            ColliderShape::Capsule { radius, height } => Self {
                min: center - Vec3::new(radius, height * 0.5 + radius, radius),
                max: center + Vec3::new(radius, height * 0.5 + radius, radius),
            },
        }
    }

    pub fn intersects(&self, other: &PhysicsAabb) -> bool {
        self.min.cmple(other.max).all() && self.max.cmpge(other.min).all()
    }
}

/// A single debug line for physics visualization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsDebugLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: [f32; 4],
}

/// Global physics simulation settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsWorld {
    pub gravity: Vec3,
    pub simulation_speed: f32,
    pub max_substeps: u32,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            simulation_speed: 1.0,
            max_substeps: 4,
        }
    }
}

/// Step physics simulation for a set of rigid bodies.
///
/// Returns `(entity, position_delta, rotation_delta)` for each dynamic body.
/// The caller should apply the deltas to its transform components and write
/// the updated `RigidBody` values back to the ECS.
pub fn step_physics(
    bodies: &mut [(hecs::Entity, RigidBody)],
    physics: &PhysicsWorld,
    dt: f32,
) -> Vec<(hecs::Entity, Vec3, Vec3)> {
    let mut results = Vec::new();
    let step_dt = dt * physics.simulation_speed;
    for (entity, body) in bodies.iter_mut() {
        if body.body_type == BodyType::Static {
            continue;
        }
        if body.use_gravity && body.body_type == BodyType::Dynamic {
            body.velocity += physics.gravity * body.gravity_scale * step_dt;
        }
        if body.drag > 0.0 {
            body.velocity *= 1.0 - body.drag * step_dt;
        }
        if body.angular_drag > 0.0 {
            body.angular_velocity *= 1.0 - body.angular_drag * step_dt;
        }
        results.push((*entity, body.velocity * step_dt, body.angular_velocity * step_dt));
    }
    results
}

/// Update rigid bodies using simple Euler integration.
///
/// Callers should supply a closure `get_transform` that returns the current
/// position and rotation of an entity, and a closure `set_transform` that
/// applies the updated values.
pub fn update_physics<F, G>(
    bodies: &mut [(hecs::Entity, &mut RigidBody)],
    colliders: &HashMap<hecs::Entity, Collider>,
    physics: &PhysicsWorld,
    dt: f32,
    mut get_transform: F,
    mut set_transform: G,
) where
    F: FnMut(hecs::Entity) -> (Vec3, Vec3),
    G: FnMut(hecs::Entity, Vec3, Vec3),
{
    let step_dt = dt * physics.simulation_speed;

    for (entity, body) in bodies.iter_mut() {
        if body.body_type == BodyType::Static {
            continue;
        }

        let (mut position, rotation) = get_transform(*entity);

        if body.use_gravity && body.body_type == BodyType::Dynamic {
            body.velocity += physics.gravity * body.gravity_scale * step_dt;
        }

        // Apply drag
        if body.drag > 0.0 {
            body.velocity *= 1.0 - body.drag * step_dt;
        }
        if body.angular_drag > 0.0 {
            body.angular_velocity *= 1.0 - body.angular_drag * step_dt;
        }

        position += body.velocity * step_dt;
        let new_rotation = rotation + body.angular_velocity * step_dt;

        set_transform(*entity, position, new_rotation);
    }

    // Simple broad-phase AABB collision detection
    let mut aabbs: Vec<(hecs::Entity, PhysicsAabb, Collider)> = Vec::new();
    for (entity, _) in bodies.iter() {
        if let Some(collider) = colliders.get(entity) {
            if collider.is_trigger {
                continue;
            }
            let (pos, _) = get_transform(*entity);
            let aabb = PhysicsAabb::from_shape(&collider.shape, pos);
            aabbs.push((*entity, aabb, *collider));
        }
    }

    for i in 0..aabbs.len() {
        for j in (i + 1)..aabbs.len() {
            let (e1, aabb1, c1) = &aabbs[i];
            let (e2, aabb2, c2) = &aabbs[j];
            if aabb1.intersects(aabb2) {
                resolve_collision(e1, e2, bodies, c1, c2, &mut get_transform, &mut set_transform);
            }
        }
    }
}

fn resolve_collision<F, G>(
    e1: &hecs::Entity,
    e2: &hecs::Entity,
    bodies: &mut [(hecs::Entity, &mut RigidBody)],
    c1: &Collider,
    c2: &Collider,
    get_transform: &mut F,
    set_transform: &mut G,
) where
    F: FnMut(hecs::Entity) -> (Vec3, Vec3),
    G: FnMut(hecs::Entity, Vec3, Vec3),
{
    let (pos1, _) = get_transform(*e1);
    let (pos2, _) = get_transform(*e2);

    let normal = (pos2 - pos1).normalize_or_zero();
    if normal == Vec3::ZERO {
        return;
    }

    let mut idx1 = None;
    let mut idx2 = None;
    for (i, (e, _)) in bodies.iter().enumerate() {
        if e == e1 { idx1 = Some(i); }
        if e == e2 { idx2 = Some(i); }
    }

    let combined_restitution = (c1.restitution + c2.restitution) * 0.5;

    if let (Some(i1), Some(i2)) = (idx1, idx2) {
        let (b1, b2) = if i1 < i2 {
            let (left, right) = bodies.split_at_mut(i2);
            (&mut left[i1].1, &mut right[0].1)
        } else {
            let (left, right) = bodies.split_at_mut(i1);
            (&mut right[0].1, &mut left[i2].1)
        };

        let relative_velocity = b2.velocity - b1.velocity;
        let velocity_along_normal = relative_velocity.dot(normal);
        if velocity_along_normal > 0.0 {
            return;
        }

        let impulse = -(1.0 + combined_restitution) * velocity_along_normal
            / (1.0 / b1.mass.max(0.001) + 1.0 / b2.mass.max(0.001));

        if b1.body_type == BodyType::Dynamic {
            b1.velocity -= normal * impulse / b1.mass.max(0.001);
        }
        if b2.body_type == BodyType::Dynamic {
            b2.velocity += normal * impulse / b2.mass.max(0.001);
        }
    }

    // Separate overlapping bodies slightly
    let penetration = 0.01;
    if let Some(i1) = idx1 {
        if bodies[i1].1.body_type == BodyType::Dynamic {
            let (p, r) = get_transform(*e1);
            set_transform(*e1, p - normal * penetration, r);
        }
    }
    if let Some(i2) = idx2 {
        if bodies[i2].1.body_type == BodyType::Dynamic {
            let (p, r) = get_transform(*e2);
            set_transform(*e2, p + normal * penetration, r);
        }
    }
}

#[cfg(test)]
pub mod lib_tests;

pub mod rapier;
pub use rapier::RapierPhysicsWorld;
