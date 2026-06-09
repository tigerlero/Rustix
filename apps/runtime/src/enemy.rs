use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;
use rustix_physics::{RigidBody, BodyType, Collider, ColliderShape};
use crate::scene::{Transform, Name, MeshComponent, Material};
use crate::combat::{Health, CombatStats, Skill, DamageEvent};

/// Marker component for enemy entities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enemy {
    pub enemy_type: EnemyType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnemyType {
    Melee,
    Ranged,
    Boss,
}

/// AI behaviour configuration for an enemy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemyAI {
    /// Whether the enemy will chase the nearest player.
    pub can_follow: bool,
    /// Whether the enemy can attack when in range.
    pub can_attack: bool,
    /// Maximum distance at which the enemy will start following.
    pub follow_range: f32,
    /// Movement speed when chasing.
    pub move_speed: f32,
    /// Stop this far from the target when attacking.
    pub stop_distance: f32,
}

impl Default for EnemyAI {
    fn default() -> Self {
        Self {
            can_follow: true,
            can_attack: true,
            follow_range: 15.0,
            move_speed: 3.5,
            stop_distance: 1.5,
        }
    }
}

/// Spawn a basic melee enemy.
pub fn spawn_enemy(
    world: &mut EcsWorld,
    position: Vec3,
    name: &str,
    mesh_name: &str,
    color: Vec3,
    ai: EnemyAI,
    health: f32,
    attack_damage: f32,
    attack_range: f32,
    attack_cooldown: f32,
) -> hecs::Entity {
    world.spawn((
        Transform {
            position,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
        },
        Name(name.into()),
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
            mass: 50.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            gravity_scale: 1.0,
            drag: 4.0,
            angular_drag: 0.05,
            use_gravity: true,
            can_sleep: true,
            sleeping: false,
        },
        Collider {
            shape: ColliderShape::Capsule { radius: 0.4, height: 1.6 },
            is_trigger: false,
            restitution: 0.0,
            friction: 0.5,
        },
        Enemy { enemy_type: EnemyType::Melee },
        ai,
        Health::new(health),
        CombatStats {
            attack_damage,
            attack_range,
            attack_cooldown,
            current_cooldown: 0.0,
        },
    ))
}

/// Spawn an enemy with a special skill.
pub fn spawn_enemy_with_skill(
    world: &mut EcsWorld,
    position: Vec3,
    name: &str,
    mesh_name: &str,
    color: Vec3,
    ai: EnemyAI,
    health: f32,
    stats: CombatStats,
    skill: Skill,
) -> hecs::Entity {
    world.spawn((
        Transform {
            position,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
        },
        Name(name.into()),
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
            mass: 50.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            gravity_scale: 1.0,
            drag: 4.0,
            angular_drag: 0.05,
            use_gravity: true,
            can_sleep: true,
            sleeping: false,
        },
        Collider {
            shape: ColliderShape::Capsule { radius: 0.4, height: 1.6 },
            is_trigger: false,
            restitution: 0.0,
            friction: 0.5,
        },
        Enemy { enemy_type: EnemyType::Boss },
        ai,
        Health::new(health),
        stats,
        skill,
    ))
}

/// Update all enemies: follow nearest player, attack when in range.
pub fn update_enemies(
    world: &mut EcsWorld,
    _dt: f32,
    damage_events: &mut Vec<DamageEvent>,
) {
    // Gather enemy data first to avoid borrow issues
    let mut enemies: Vec<(hecs::Entity, Vec3, EnemyAI, CombatStats, Health)> = Vec::new();
    for (e, transform, ai, stats, health) in world.query::<(hecs::Entity, &Transform, &EnemyAI, &CombatStats, &Health)>().iter() {
        if health.is_dead() {
            continue;
        }
        enemies.push((e, transform.position, *ai, *stats, *health));
    }

    for (entity, pos, ai, mut stats, _) in enemies {
        if !ai.can_follow && !ai.can_attack {
            continue;
        }

        // Find nearest player (entity with Player component)
        let mut nearest: Option<(hecs::Entity, Vec3, f32)> = None;
        for (e, transform, health) in world.query::<(hecs::Entity, &Transform, &Health)>().iter() {
            if health.is_dead() {
                continue;
            }
            // Skip other enemies - only target players (players also have Health)
            // We distinguish by checking if the entity has the Enemy component.
            if world.get::<&super::player::Player>(e).is_ok() {
                let dist = (transform.position - pos).length();
                if dist <= ai.follow_range {
                    if nearest.map_or(true, |(_, _, d)| dist < d) {
                        nearest = Some((e, transform.position, dist));
                    }
                }
            }
        }

        if let Some((target, target_pos, dist)) = nearest {
            // Movement
            if ai.can_follow && dist > ai.stop_distance {
                let dir = (target_pos - pos).normalize_or_zero();
                if let Ok(mut body) = world.get::<&mut RigidBody>(entity) {
                    body.velocity.x = dir.x * ai.move_speed;
                    body.velocity.z = dir.z * ai.move_speed;
                }
            } else {
                // Stop moving when close enough
                if let Ok(mut body) = world.get::<&mut RigidBody>(entity) {
                    body.velocity.x = 0.0;
                    body.velocity.z = 0.0;
                }
            }

            // Attack
            if ai.can_attack && stats.can_attack() && dist <= stats.attack_range {
                stats.reset_cooldown();
                damage_events.push(DamageEvent {
                    target,
                    amount: stats.attack_damage,
                    source: entity,
                });
                tracing::info!("enemy {:?} attacks player {:?} for {:.1} damage", entity, target, stats.attack_damage);
            }

            // Update cooldown in ECS
            if let Ok(mut ecs_stats) = world.get::<&mut CombatStats>(entity) {
                *ecs_stats = stats;
            }
        } else {
            // No target in range - stop moving
            if let Ok(mut body) = world.get::<&mut RigidBody>(entity) {
                body.velocity.x = 0.0;
                body.velocity.z = 0.0;
            }
        }
    }
}
