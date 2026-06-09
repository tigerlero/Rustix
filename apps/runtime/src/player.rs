use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;
use rustix_physics::{RigidBody, BodyType, Collider, ColliderShape};
use rustix_platform::input::{InputManager, KeyCode};
use crate::camera::EditorCamera;
use crate::scene::{Transform, Name, MeshComponent, Material};
use crate::combat::{Health, CombatStats, DamageEvent};

/// Marker component for player-controlled entities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Player {
    pub index: u32,
}

/// Manages multiple player entities and input routing.
#[derive(Debug, Clone)]
pub struct PlayerManager {
    pub players: Vec<hecs::Entity>,
    pub active_index: usize,
    /// When true, keyboard input applies to ALL players simultaneously.
    pub control_all: bool,
}

impl PlayerManager {
    pub fn new() -> Self {
        Self {
            players: Vec::new(),
            active_index: 0,
            control_all: false,
        }
    }

    pub fn active_entity(&self) -> Option<hecs::Entity> {
        if self.control_all {
            None
        } else {
            self.players.get(self.active_index).copied()
        }
    }

    pub fn cycle_active(&mut self) {
        if self.players.is_empty() { return; }
        self.active_index = (self.active_index + 1) % self.players.len();
        self.control_all = false;
    }

    pub fn toggle_control_all(&mut self) {
        self.control_all = !self.control_all;
    }
}

/// Spawn a player entity with capsule collider and visual mesh.
pub fn spawn_player(
    world: &mut EcsWorld,
    position: Vec3,
    index: u32,
    mesh_name: &str,
) -> hecs::Entity {
    let color = match index % 3 {
        0 => Vec3::new(0.8, 0.3, 0.3),
        1 => Vec3::new(0.3, 0.8, 0.3),
        _ => Vec3::new(0.3, 0.3, 0.8),
    };
    world.spawn((
        Transform {
            position,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
        },
        Name(format!("Player {}", index + 1)),
        MeshComponent(mesh_name.into()),
        Material {
            base_color: color,
            alpha: 1.0,
            roughness: 0.5,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
        },
        RigidBody {
            body_type: BodyType::Dynamic,
            mass: 70.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            gravity_scale: 1.0,
            drag: 2.0,
            angular_drag: 0.05,
            use_gravity: true,
            can_sleep: true,
            sleeping: false,
        },
        Collider {
            shape: ColliderShape::Capsule { radius: 0.5, height: 1.75 },
            is_trigger: false,
            restitution: 0.0,
            friction: 0.5,
        },
        Player { index },
        Health::new(100.0),
        CombatStats {
            attack_damage: 15.0,
            attack_range: 2.5,
            attack_cooldown: 0.5,
            current_cooldown: 0.0,
        },
    ))
}

/// Spawn a 2D player entity with quad mesh and flat box collider (no gravity).
pub fn spawn_player_2d(
    world: &mut EcsWorld,
    position: Vec3,
    index: u32,
) -> hecs::Entity {
    let color = match index % 3 {
        0 => Vec3::new(0.8, 0.3, 0.3),
        1 => Vec3::new(0.3, 0.8, 0.3),
        _ => Vec3::new(0.3, 0.3, 0.8),
    };
    world.spawn((
        Transform {
            position,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
        },
        Name(format!("Player {}", index + 1)),
        MeshComponent("Quad".into()),
        Material {
            base_color: color,
            alpha: 1.0,
            roughness: 0.5,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
        },
        RigidBody {
            body_type: BodyType::Dynamic,
            mass: 50.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            gravity_scale: 0.0,
            drag: 2.0,
            angular_drag: 0.05,
            use_gravity: false,
            can_sleep: true,
            sleeping: false,
        },
        Collider {
            shape: ColliderShape::Box { half_extents: Vec3::new(0.5, 0.5, 0.05) },
            is_trigger: false,
            restitution: 0.0,
            friction: 0.5,
        },
        Player { index },
        Health::new(100.0),
        CombatStats {
            attack_damage: 15.0,
            attack_range: 2.5,
            attack_cooldown: 0.5,
            current_cooldown: 0.0,
        },
    ))
}

/// Update player movement and combat from input.
///
/// - **Tab** cycles the active player.
/// - **F** toggles "control all" mode (WASD moves every player together).
/// - **W/A/S/D** move the active player(s) relative to camera yaw.
/// - **Space** jumps.
/// - **Mouse Left** attack nearest enemy in range.
pub fn update_players(
    world: &mut EcsWorld,
    manager: &mut PlayerManager,
    camera: &EditorCamera,
    input: &InputManager,
    dt: f32,
    damage_events: &mut Vec<DamageEvent>,
) {
    // Tab to cycle active player
    if input.keyboard().just_pressed(KeyCode::Tab) {
        manager.cycle_active();
        if let Some(_) = manager.active_entity() {
            tracing::info!("switched to player {}", manager.active_index + 1);
        }
    }

    // F to toggle control-all mode
    if input.keyboard().just_pressed(KeyCode::F) {
        manager.toggle_control_all();
        let mode = if manager.control_all { "all" } else { "single" };
        tracing::info!("player control mode: {}", mode);
    }

    // Tick player cooldowns
    for (_, stats) in world.query_mut::<(&Player, &mut CombatStats)>() {
        stats.tick(dt);
    }

    let k = input.keyboard();
    let move_speed = 6.0;
    let jump_force = 8.0;

    // Compute movement direction from camera yaw (horizontal only)
    let yaw = camera.yaw;
    let forward = Vec3::new(yaw.sin(), 0.0, yaw.cos()).normalize();
    let right = Vec3::new(forward.z, 0.0, -forward.x).normalize();

    let mut move_input = Vec3::ZERO;
    if k.down(KeyCode::W) { move_input += forward; }
    if k.down(KeyCode::S) { move_input -= forward; }
    if k.down(KeyCode::A) { move_input -= right; }
    if k.down(KeyCode::D) { move_input += right; }
    if move_input != Vec3::ZERO {
        move_input = move_input.normalize();
    }

    let jump = k.just_pressed(KeyCode::Space);

    // Apply velocity and handle attack input for targeted player(s)
    let targets: Vec<hecs::Entity> = if manager.control_all {
        manager.players.clone()
    } else {
        manager.active_entity().into_iter().collect()
    };
    if targets.is_empty() {
        tracing::debug!("update_players: no active players to control");
    }

    let attack_input = input.mouse().just_pressed(rustix_platform::input::MouseButton::Left);

    for entity in targets {
        if let Ok(mut body) = world.get::<&mut RigidBody>(entity) {
            body.velocity.x = move_input.x * move_speed;
            body.velocity.z = move_input.z * move_speed;
            if jump {
                body.velocity.y = jump_force;
            }
        }

        // Player attack
        if attack_input {
            let mut can_attack = false;
            let mut attack_range = 0.0;
            let mut attack_damage = 0.0;
            let pos = if let Ok(t) = world.get::<&Transform>(entity) {
                if let Ok(stats) = world.get::<&CombatStats>(entity) {
                    if stats.can_attack() {
                        can_attack = true;
                        attack_range = stats.attack_range;
                        attack_damage = stats.attack_damage;
                    }
                }
                t.position
            } else {
                Vec3::ZERO
            };

            if can_attack {
                // Find nearest enemy
                let mut nearest: Option<(hecs::Entity, f32)> = None;
                for (e, transform, health) in world.query::<(hecs::Entity, &Transform, &Health)>().iter() {
                    if health.is_dead() { continue; }
                    if world.get::<&super::enemy::Enemy>(e).is_ok() {
                        let dist = (transform.position - pos).length();
                        if dist <= attack_range {
                            if nearest.map_or(true, |(_, d)| dist < d) {
                                nearest = Some((e, dist));
                            }
                        }
                    }
                }
                if let Some((target, _)) = nearest {
                    if let Ok(mut stats) = world.get::<&mut CombatStats>(entity) {
                        stats.reset_cooldown();
                    }
                    damage_events.push(DamageEvent {
                        target,
                        amount: attack_damage,
                        source: entity,
                    });
                    tracing::info!("player {:?} attacks enemy {:?} for {:.1} damage", entity, target, attack_damage);
                }
            }
        }
    }
}

/// Return the world position of the active player (for camera follow).
pub fn active_player_position(world: &EcsWorld, manager: &PlayerManager) -> Option<Vec3> {
    let entity = manager.active_entity()?;
    world.get::<&Transform>(entity).ok().map(|t| t.position)
}
