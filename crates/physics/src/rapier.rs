//! Rapier3D physics backend integration.
//!
//! `RapierPhysicsWorld` wraps `rapier3d` and synchronizes rigid bodies and
//! colliders with the engine's ECS.  The existing `RigidBody` and `Collider`
//! components are kept as the ECS-facing API; this module maps them to
//! rapier's internal representation.

use std::collections::HashMap;
use std::sync::Mutex;

use rapier3d::prelude::*;
use rustix_core::math::Vec3;

use crate::{BodyType, CharacterController, Collider, ColliderShape, Joint, JointType, PhysicsDebugLine, PhysicsWorld, RigidBody};

/// Serialized state of the physics world for deterministic replay.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PhysicsSnapshot {
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    ccd_solver: CCDSolver,
    integration_parameters: IntegrationParameters,
    gravity: nalgebra::Vector3<f32>,

    entity_to_body: HashMap<hecs::Entity, RigidBodyHandle>,
    body_to_entity: HashMap<RigidBodyHandle, hecs::Entity>,
    entity_to_collider: HashMap<hecs::Entity, ColliderHandle>,
    character_controllers: HashMap<hecs::Entity, rapier3d::control::KinematicCharacterController>,
    joints: HashMap<hecs::Entity, ImpulseJointHandle>,
}

/// Buffers rapier collision events so they can be read after `step()`.
struct CollisionEventCollector {
    events: Mutex<Vec<CollisionEvent>>,
}

impl CollisionEventCollector {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    fn drain(&self) -> Vec<CollisionEvent> {
        if let Ok(mut guard) = self.events.lock() {
            std::mem::take(&mut *guard)
        } else {
            Vec::new()
        }
    }
}

impl EventHandler for CollisionEventCollector {
    fn handle_collision_event(
        &self,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        event: CollisionEvent,
        _contact_pair: Option<&ContactPair>,
    ) {
        if let Ok(mut guard) = self.events.lock() {
            guard.push(event);
        }
    }

    fn handle_contact_force_event(
        &self,
        _dt: Real,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        _contact_pair: &ContactPair,
        _total_force_magnitude: Real,
    ) {
    }
}

/// Wraps a full rapier3D simulation world and provides ECS sync helpers.
pub struct RapierPhysicsWorld {
    pub pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub impulse_joints: ImpulseJointSet,
    pub multibody_joints: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub integration_parameters: IntegrationParameters,
    pub query_pipeline: QueryPipeline,
    pub physics_hooks: (),
    event_handler: CollisionEventCollector,
    gravity: nalgebra::Vector3<f32>,

    entity_to_body: HashMap<hecs::Entity, RigidBodyHandle>,
    body_to_entity: HashMap<RigidBodyHandle, hecs::Entity>,
    entity_to_collider: HashMap<hecs::Entity, ColliderHandle>,
    character_controllers: HashMap<hecs::Entity, rapier3d::control::KinematicCharacterController>,
    joints: HashMap<hecs::Entity, ImpulseJointHandle>,
}

impl RapierPhysicsWorld {
    pub fn new() -> Self {
        Self {
            pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            integration_parameters: IntegrationParameters::default(),
            query_pipeline: QueryPipeline::new(),
            physics_hooks: (),
            event_handler: CollisionEventCollector::new(),
            gravity: nalgebra::Vector3::new(0.0, -9.81, 0.0),
            entity_to_body: HashMap::new(),
            body_to_entity: HashMap::new(),
            entity_to_collider: HashMap::new(),
            character_controllers: HashMap::new(),
            joints: HashMap::new(),
        }
    }

    /// Configure simulation parameters from engine `PhysicsWorld` settings.
    pub fn configure(&mut self, settings: &PhysicsWorld) {
        // Gravity is passed directly to pipeline.step(); stored here for convenience.
        self.gravity = vec3_to_nalgebra(settings.gravity);
    }

    /// Register an ECS entity as a dynamic/static/kinematic rigid body.
    pub fn add_entity(
        &mut self,
        entity: hecs::Entity,
        body: &RigidBody,
        collider: &Collider,
        position: Vec3,
        rotation: [f32; 3], // Euler angles XYZ in radians
    ) {
        self.remove_entity(entity);

        let rb = build_rapier_body(body, position, rotation);
        let handle = self.rigid_body_set.insert(rb);

        let co = build_rapier_collider(collider);
        let co_handle = self.collider_set.insert_with_parent(
            co,
            handle,
            &mut self.rigid_body_set,
        );

        self.entity_to_body.insert(entity, handle);
        self.body_to_entity.insert(handle, entity);
        self.entity_to_collider.insert(entity, co_handle);
    }

    /// Remove an entity from the rapier world.
    pub fn remove_entity(&mut self, entity: hecs::Entity) {
        if let Some(handle) = self.entity_to_body.remove(&entity) {
            self.body_to_entity.remove(&handle);
            self.rigid_body_set.remove(
                handle,
                &mut self.island_manager,
                &mut self.collider_set,
                &mut self.impulse_joints,
                &mut self.multibody_joints,
                true,
            );
        }
        self.entity_to_collider.remove(&entity);
    }

    /// Step the physics simulation.
    ///
    /// `dt` is the frame delta time in seconds.
    pub fn step(&mut self, dt: f32) {
        self.integration_parameters.dt = dt;
        self.pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            None,
            &self.physics_hooks,
            &self.event_handler,
        );
        self.query_pipeline.update(&self.collider_set);
    }

    /// Read back the position and rotation of a body for an entity.
    pub fn transform_of(&self, entity: hecs::Entity) -> Option<(Vec3, [f32; 3])> {
        let handle = self.entity_to_body.get(&entity)?;
        let rb = self.rigid_body_set.get(*handle)?;
        let pos = nalgebra_vec_to_vec3(*rb.translation());
        let rot = rb.rotation().euler_angles();
        Some((pos, [rot.0, rot.1, rot.2]))
    }

    /// Apply a force (in world space) to an entity's rigid body.
    pub fn apply_force(&mut self, entity: hecs::Entity, force: Vec3) {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(rb) = self.rigid_body_set.get_mut(handle) {
                rb.add_force(vec3_to_nalgebra(force), true);
            }
        }
    }

    /// Apply an impulse (instant velocity change) to an entity's rigid body.
    pub fn apply_impulse(&mut self, entity: hecs::Entity, impulse: Vec3) {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(rb) = self.rigid_body_set.get_mut(handle) {
                rb.apply_impulse(vec3_to_nalgebra(impulse), true);
            }
        }
    }

    /// Set the linear velocity of a body directly.
    pub fn set_velocity(&mut self, entity: hecs::Entity, velocity: Vec3) {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(rb) = self.rigid_body_set.get_mut(handle) {
                rb.set_linvel(vec3_to_nalgebra(velocity), true);
            }
        }
    }

    /// Set the angular velocity of a body directly.
    pub fn set_angular_velocity(&mut self, entity: hecs::Entity, velocity: Vec3) {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(rb) = self.rigid_body_set.get_mut(handle) {
                rb.set_angvel(vec3_to_nalgebra(velocity), true);
            }
        }
    }

    /// Perform a raycast and return the first hit entity and distance.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_toi: f32) -> Option<(hecs::Entity, f32)> {
        let ray = Ray::new(
            nalgebra::Point3::new(origin.x, origin.y, origin.z),
            nalgebra::Vector3::new(direction.x, direction.y, direction.z),
        );
        let filter = QueryFilter::default();
        self.query_pipeline
            .cast_ray(&self.rigid_body_set, &self.collider_set, &ray, max_toi, true, filter)
            .map(|(handle, toi)| {
                let parent = self.collider_set.get(handle)?.parent()?;
                let entity = self.body_to_entity.get(&parent).copied()?;
                Some((entity, toi))
            })
            .flatten()
    }

    /// Register an entity as a kinematic character controller.
    ///
    /// The entity must already have been added via `add_entity`.
    /// The `Collider` should have a capsule shape for best results.
    pub fn add_character(&mut self, entity: hecs::Entity, controller: &CharacterController) {
        use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};

        let rapier_controller = KinematicCharacterController {
            up: nalgebra::Vector::y_axis(),
            offset: CharacterLength::Relative(0.01),
            slide: true,
            autostep: if controller.step_height > 0.0 {
                Some(CharacterAutostep {
                    max_height: CharacterLength::Absolute(controller.step_height),
                    min_width: CharacterLength::Relative(0.5),
                    include_dynamic_bodies: true,
                })
            } else {
                None
            },
            max_slope_climb_angle: controller.slope_limit_degrees.to_radians(),
            min_slope_slide_angle: (controller.slope_limit_degrees - 5.0).to_radians().max(0.0),
            snap_to_ground: if controller.snap_to_ground {
                Some(CharacterLength::Relative(0.2))
            } else {
                None
            },
            normal_nudge_factor: 0.01,
        };

        self.character_controllers.insert(entity, rapier_controller);
    }

    /// Unregister a character controller.
    pub fn remove_character(&mut self, entity: hecs::Entity) {
        self.character_controllers.remove(&entity);
    }

    /// Move a character by `desired_translation` and return the effective movement + grounded state.
    ///
    /// `dt` is the frame delta time. The entity must be a registered character.
    pub fn move_character(
        &mut self,
        entity: hecs::Entity,
        desired_translation: Vec3,
        dt: f32,
    ) -> Option<(Vec3, bool)> {
        let controller = self.character_controllers.get(&entity)?;
        let body_handle = *self.entity_to_body.get(&entity)?;
        let collider_handle = *self.entity_to_collider.get(&entity)?;

        let body = self.rigid_body_set.get(body_handle)?;
        let collider = self.collider_set.get(collider_handle)?;
        let shape = collider.shape();
        let pos = *body.position();

        let filter = QueryFilter::default().exclude_rigid_body(body_handle);
        let movement = controller.move_shape(
            dt,
            &self.rigid_body_set,
            &self.collider_set,
            &self.query_pipeline,
            shape,
            &pos,
            vec3_to_nalgebra(desired_translation),
            filter,
            |_| {},
        );

        let new_pos = pos * nalgebra::Isometry::from(nalgebra::Translation3::from(movement.translation));
        let body = self.rigid_body_set.get_mut(body_handle)?;
        body.set_position(new_pos, true);

        Some((nalgebra_vec_to_vec3(movement.translation), movement.grounded))
    }

    /// Drain collision events produced by the last `step()`.
    ///
    /// Returns `(entity_a, entity_b, started)` where `started` is `true`
    /// for collision enter and `false` for collision exit.
    pub fn collision_events(&self) -> Vec<(hecs::Entity, hecs::Entity, bool)> {
        self.event_handler
            .drain()
            .into_iter()
            .filter_map(|event| {
                let entity_a = self.collider_entity(event.collider1())?;
                let entity_b = self.collider_entity(event.collider2())?;
                let started = event.started();
                Some((entity_a, entity_b, started))
            })
            .collect()
    }

    fn collider_entity(&self, handle: ColliderHandle) -> Option<hecs::Entity> {
        let parent = self.collider_set.get(handle)?.parent()?;
        self.body_to_entity.get(&parent).copied()
    }

    /// Total number of active rigid bodies.
    pub fn body_count(&self) -> usize {
        self.rigid_body_set.len()
    }

    /// Total number of colliders.
    pub fn collider_count(&self) -> usize {
        self.collider_set.len()
    }

    /// Create a joint between two entities.
    ///
    /// `entity_a` is the entity this joint is attached to; `entity_b` is the
    /// connected entity (from `joint.connected_entity`).  Both must already
    /// have been added to the physics world.
    pub fn add_joint(&mut self, entity_a: hecs::Entity, joint: &Joint) -> Option<ImpulseJointHandle> {
        let body_a = *self.entity_to_body.get(&entity_a)?;
        let body_b = *self.entity_to_body.get(&joint.connected_entity)?;

        let rapier_joint: GenericJoint = match joint.joint_type {
            JointType::Fixed => {
                FixedJointBuilder::new()
                    .local_anchor1(nalgebra::Point3::new(joint.local_anchor1.x, joint.local_anchor1.y, joint.local_anchor1.z))
                    .local_anchor2(nalgebra::Point3::new(joint.local_anchor2.x, joint.local_anchor2.y, joint.local_anchor2.z))
                    .contacts_enabled(joint.contacts_enabled)
                    .build()
                    .into()
            }
            JointType::Revolute { axis } => {
                let axis = nalgebra::Vector3::new(axis.x, axis.y, axis.z);
                let axis = nalgebra::Unit::new_normalize(axis);
                RevoluteJointBuilder::new(axis)
                    .local_anchor1(nalgebra::Point3::new(joint.local_anchor1.x, joint.local_anchor1.y, joint.local_anchor1.z))
                    .local_anchor2(nalgebra::Point3::new(joint.local_anchor2.x, joint.local_anchor2.y, joint.local_anchor2.z))
                    .contacts_enabled(joint.contacts_enabled)
                    .build()
                    .into()
            }
            JointType::Spherical => {
                SphericalJointBuilder::new()
                    .local_anchor1(nalgebra::Point3::new(joint.local_anchor1.x, joint.local_anchor1.y, joint.local_anchor1.z))
                    .local_anchor2(nalgebra::Point3::new(joint.local_anchor2.x, joint.local_anchor2.y, joint.local_anchor2.z))
                    .contacts_enabled(joint.contacts_enabled)
                    .build()
                    .into()
            }
            JointType::Prismatic { axis } => {
                let axis = nalgebra::Vector3::new(axis.x, axis.y, axis.z);
                let axis = nalgebra::Unit::new_normalize(axis);
                PrismaticJointBuilder::new(axis)
                    .local_anchor1(nalgebra::Point3::new(joint.local_anchor1.x, joint.local_anchor1.y, joint.local_anchor1.z))
                    .local_anchor2(nalgebra::Point3::new(joint.local_anchor2.x, joint.local_anchor2.y, joint.local_anchor2.z))
                    .contacts_enabled(joint.contacts_enabled)
                    .build()
                    .into()
            }
        };

        let handle = self.impulse_joints.insert(body_a, body_b, rapier_joint, true);
        self.joints.insert(entity_a, handle);
        Some(handle)
    }

    /// Remove a joint by the owning entity's handle.
    pub fn remove_joint(&mut self, entity_a: hecs::Entity) {
        if let Some(handle) = self.joints.remove(&entity_a) {
            self.impulse_joints.remove(handle, true);
        }
    }

    /// Total number of impulse joints.
    pub fn joint_count(&self) -> usize {
        self.impulse_joints.len()
    }

    /// Wake up a rigid body manually.
    pub fn wake_up(&mut self, entity: hecs::Entity, strong: bool) {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.wake_up(strong);
            }
        }
    }

    /// Query whether a rigid body is currently sleeping.
    pub fn is_sleeping(&self, entity: hecs::Entity) -> bool {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(body) = self.rigid_body_set.get(handle) {
                return body.is_sleeping();
            }
        }
        false
    }

    /// Number of active (non-sleeping) rigid bodies.
    pub fn active_body_count(&self) -> usize {
        self.rigid_body_set.iter().filter(|(_, b)| !b.is_sleeping()).count()
    }

    /// Save the entire physics world state into a `PhysicsSnapshot`.
    ///
    /// The snapshot captures all bodies, colliders, joints, and internal
    /// bookkeeping so the world can be restored later for deterministic replay.
    pub fn save_snapshot(&self) -> PhysicsSnapshot {
        PhysicsSnapshot {
            rigid_body_set: self.rigid_body_set.clone(),
            collider_set: self.collider_set.clone(),
            impulse_joints: self.impulse_joints.clone(),
            multibody_joints: self.multibody_joints.clone(),
            island_manager: self.island_manager.clone(),
            broad_phase: self.broad_phase.clone(),
            narrow_phase: self.narrow_phase.clone(),
            ccd_solver: self.ccd_solver.clone(),
            integration_parameters: self.integration_parameters,
            gravity: self.gravity,
            entity_to_body: self.entity_to_body.clone(),
            body_to_entity: self.body_to_entity.clone(),
            entity_to_collider: self.entity_to_collider.clone(),
            character_controllers: self.character_controllers.clone(),
            joints: self.joints.clone(),
        }
    }

    /// Restore the physics world from a `PhysicsSnapshot`.
    ///
    /// After restoring, the `QueryPipeline` is rebuilt from the collider set.
    pub fn restore_snapshot(&mut self, snapshot: &PhysicsSnapshot) {
        self.rigid_body_set = snapshot.rigid_body_set.clone();
        self.collider_set = snapshot.collider_set.clone();
        self.impulse_joints = snapshot.impulse_joints.clone();
        self.multibody_joints = snapshot.multibody_joints.clone();
        self.island_manager = snapshot.island_manager.clone();
        self.broad_phase = snapshot.broad_phase.clone();
        self.narrow_phase = snapshot.narrow_phase.clone();
        self.ccd_solver = snapshot.ccd_solver.clone();
        self.integration_parameters = snapshot.integration_parameters;
        self.gravity = snapshot.gravity;
        self.entity_to_body = snapshot.entity_to_body.clone();
        self.body_to_entity = snapshot.body_to_entity.clone();
        self.entity_to_collider = snapshot.entity_to_collider.clone();
        self.character_controllers = snapshot.character_controllers.clone();
        self.joints = snapshot.joints.clone();
        self.query_pipeline.update(&self.collider_set);
    }

    /// Serialize the snapshot to a byte vector using `bincode`.
    pub fn serialize_snapshot(snapshot: &PhysicsSnapshot) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(snapshot)
    }

    /// Deserialize a snapshot from a byte vector using `bincode`.
    pub fn deserialize_snapshot(bytes: &[u8]) -> Result<PhysicsSnapshot, bincode::Error> {
        bincode::deserialize(bytes)
    }

    /// Generate debug lines for colliders, contacts, and velocities.
    ///
    /// Returns a list of line segments with colors. The caller maps these
    /// to the engine's debug-line renderer (e.g. `GizmoLine`).
    pub fn debug_draw(&self) -> Vec<PhysicsDebugLine> {
        let mut backend = DebugLineCollector::new();
        let mut pipeline = DebugRenderPipeline::new(DebugRenderStyle::default(), DebugRenderMode::default());
        pipeline.render(
            &mut backend,
            &self.rigid_body_set,
            &self.collider_set,
            &self.impulse_joints,
            &self.multibody_joints,
            &self.narrow_phase,
        );
        backend.lines
    }
}

impl Default for RapierPhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

// ── debug render backend ──

struct DebugLineCollector {
    lines: Vec<PhysicsDebugLine>,
}

impl DebugLineCollector {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }
}

impl DebugRenderBackend for DebugLineCollector {
    fn draw_line(
        &mut self,
        _object: DebugRenderObject,
        a: Point<Real>,
        b: Point<Real>,
        color: DebugColor,
    ) {
        self.lines.push(PhysicsDebugLine {
            start: Vec3::new(a.x, a.y, a.z),
            end: Vec3::new(b.x, b.y, b.z),
            color,
        });
    }
}

// ── internal builders ──

fn build_rapier_body(body: &RigidBody, pos: Vec3, rot: [f32; 3]) -> rapier3d::dynamics::RigidBody {
    let mut builder = match body.body_type {
        BodyType::Static => RigidBodyBuilder::fixed(),
        BodyType::Dynamic => RigidBodyBuilder::dynamic(),
        BodyType::Kinematic => RigidBodyBuilder::kinematic_position_based(),
    };

    builder = builder
        .translation(nalgebra::Vector3::new(pos.x, pos.y, pos.z))
        .rotation(nalgebra::Vector3::new(rot[0], rot[1], rot[2]))
        .linvel(nalgebra::Vector3::new(body.velocity.x, body.velocity.y, body.velocity.z))
        .angvel(nalgebra::Vector3::new(body.angular_velocity.x, body.angular_velocity.y, body.angular_velocity.z))
        .additional_mass(body.mass)
        .gravity_scale(body.gravity_scale)
        .linear_damping(body.drag)
        .angular_damping(body.angular_drag);

    if !body.use_gravity {
        builder = builder.gravity_scale(0.0);
    }

    builder
        .can_sleep(body.can_sleep)
        .sleeping(body.sleeping)
        .build()
}

fn build_rapier_collider(collider: &Collider) -> rapier3d::geometry::Collider {
    let shape: SharedShape = match collider.shape {
        ColliderShape::Sphere { radius } => SharedShape::ball(radius),
        ColliderShape::Box { half_extents } => {
            SharedShape::cuboid(half_extents.x, half_extents.y, half_extents.z)
        }
        ColliderShape::Capsule { radius, height } => {
            SharedShape::capsule(
                nalgebra::Vector3::new(0.0, -height * 0.5, 0.0).into(),
                nalgebra::Vector3::new(0.0, height * 0.5, 0.0).into(),
                radius,
            )
        }
    };

    ColliderBuilder::new(shape)
        .restitution(collider.restitution)
        .friction(collider.friction)
        .sensor(collider.is_trigger)
        .active_events(ActiveEvents::COLLISION_EVENTS)
        .build()
}

// ── glam ↔ nalgebra conversions ──

fn vec3_to_nalgebra(v: Vec3) -> nalgebra::Vector3<f32> {
    nalgebra::Vector3::new(v.x, v.y, v.z)
}

fn nalgebra_vec_to_vec3(v: nalgebra::Vector3<f32>) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}
