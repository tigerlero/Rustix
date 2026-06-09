use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;
use crate::scene::Transform;

/// Hit points for any combatant (player or enemy).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn take_damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }
}

/// Basic combat stats shared by players and enemies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatStats {
    /// Damage dealt per basic attack.
    pub attack_damage: f32,
    /// How far away the entity can strike.
    pub attack_range: f32,
    /// Seconds between attacks.
    pub attack_cooldown: f32,
    /// Current cooldown remaining (decreased each frame).
    pub current_cooldown: f32,
}

impl CombatStats {
    pub fn can_attack(&self) -> bool {
        self.current_cooldown <= 0.0
    }

    pub fn reset_cooldown(&mut self) {
        self.current_cooldown = self.attack_cooldown;
    }

    pub fn tick(&mut self, dt: f32) {
        self.current_cooldown = (self.current_cooldown - dt).max(0.0);
    }
}

/// A special ability with its own cooldown.
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub damage: f32,
    pub range: f32,
    pub cooldown: f32,
    pub current_cooldown: f32,
}

impl Skill {
    pub fn can_use(&self) -> bool {
        self.current_cooldown <= 0.0
    }

    pub fn reset_cooldown(&mut self) {
        self.current_cooldown = self.cooldown;
    }

    pub fn tick(&mut self, dt: f32) {
        self.current_cooldown = (self.current_cooldown - dt).max(0.0);
    }
}

/// Marker for entities that have been killed and should be cleaned up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dead;

/// Damage event queued during a frame and applied at the end.
#[derive(Debug, Clone, Copy)]
pub struct DamageEvent {
    pub target: hecs::Entity,
    pub amount: f32,
    pub source: hecs::Entity,
}

/// Apply queued damage, check for death, and mark dead entities.
pub fn resolve_damage(world: &mut EcsWorld, events: &[DamageEvent]) {
    for ev in events {
        let is_dead = {
            if let Ok(mut health) = world.get::<&mut Health>(ev.target) {
                if health.is_alive() {
                    health.take_damage(ev.amount);
                    tracing::debug!("entity {:?} took {:.1} damage from {:?}", ev.target, ev.amount, ev.source);
                    health.is_dead()
                } else {
                    false
                }
            } else {
                false
            }
        };
        if is_dead {
            tracing::info!("entity {:?} was killed by {:?}", ev.target, ev.source);
            let _ = world.insert(ev.target, (Dead,));
        }
    }
}

/// Remove entities marked as Dead from the world.
pub fn cleanup_dead(world: &mut EcsWorld) -> Vec<hecs::Entity> {
    let mut dead = Vec::new();
    for (e, _) in world.query_mut::<(&hecs::Entity, &Dead)>() {
        dead.push(*e);
    }
    for e in &dead {
        let _ = world.despawn(*e);
    }
    dead
}

/// Tick all cooldowns on entities with CombatStats and Skill.
pub fn tick_cooldowns(world: &mut EcsWorld, dt: f32) {
    for stats in world.query_mut::<&mut CombatStats>() {
        stats.tick(dt);
    }
    for skill in world.query_mut::<&mut Skill>() {
        skill.tick(dt);
    }
}

/// Find the nearest living target of a different type within max_distance.
/// `source_filter` should match true for potential targets.
pub fn find_nearest_target(
    world: &EcsWorld,
    from_pos: Vec3,
    max_distance: f32,
    source_entity: hecs::Entity,
) -> Option<(hecs::Entity, Vec3, f32)> {
    let mut nearest: Option<(hecs::Entity, Vec3, f32)> = None;
    for (e, transform, health) in world.query::<(hecs::Entity, &Transform, &Health)>().iter() {
        if e == source_entity || health.is_dead() {
            continue;
        }
        let dist = (transform.position - from_pos).length();
        if dist > max_distance {
            continue;
        }
        if nearest.map_or(true, |(_, _, d)| dist < d) {
            nearest = Some((e, transform.position, dist));
        }
    }
    nearest
}

/// Spawn floating damage text / combat feedback (placeholder for now).
pub fn spawn_damage_text(world: &mut EcsWorld, _position: Vec3, _amount: f32) {
    // Future: spawn a temporary floating text entity.
    let _ = world;
}
