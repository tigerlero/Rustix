//! 3D Platformer game logic.
//!
//! Uses the ECS world for rendering and a custom kinematic controller for
//! tight platformer physics (WASD move, Space jump, AABB collision).

use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;
use rustix_platform::input::KeyCode;
use rustix_audio::{AudioEngine, SoundInstance};
use crate::scene::{Transform, Name, MeshComponent, Material};
use crate::camera::EditorCamera;

/// Marker component for the platformer player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlatformerPlayer {
    pub velocity: Vec3,
    pub on_ground: bool,
    pub jump_cooldown: f32,
    pub coyote_time: f32,
    pub jump_buffer: f32,
}

impl Default for PlatformerPlayer {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            on_ground: false,
            jump_cooldown: 0.0,
            coyote_time: 0.0,
            jump_buffer: 0.0,
        }
    }
}

/// Marker component for platform geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Platform {
    pub half_extents: Vec3,
}

/// Marker component for collectible coins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlatformerCoin {
    pub collected: bool,
    pub bob_offset: f32,
}

/// Respawn trigger (fall below this Y).
pub const RESPAWN_Y: f32 = -20.0;

/// Player move speed (units/sec).
pub const MOVE_SPEED: f32 = 6.0;
/// Gravity (units/sec^2).
pub const GRAVITY: f32 = -28.0;
/// Jump impulse.
pub const JUMP_VELOCITY: f32 = 12.0;
/// Coyote time after leaving ground (seconds).
pub const COYOTE_TIME_MAX: f32 = 0.1;
/// Jump buffer before landing (seconds).
pub const JUMP_BUFFER_MAX: f32 = 0.1;
/// Player capsule radius.
pub const PLAYER_RADIUS: f32 = 0.4;
/// Player capsule half-height (excluding radius).
pub const PLAYER_HALF_HEIGHT: f32 = 0.6;

/// Platformer game state.
#[derive(Debug)]
pub struct PlatformerGame {
    pub score: u32,
    pub coins_collected: u32,
    pub total_coins: u32,
    pub lives: i32,
    pub game_over: bool,
    pub paused: bool,
    pub respawn_pos: Vec3,
    pub sfx_instances: Vec<SoundInstance>,
}

impl Default for PlatformerGame {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformerGame {
    pub fn new() -> Self {
        Self {
            score: 0,
            coins_collected: 0,
            total_coins: 0,
            lives: 3,
            game_over: false,
            paused: false,
            respawn_pos: Vec3::new(0.0, 2.0, 0.0),
            sfx_instances: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.score = 0;
        self.coins_collected = 0;
        self.lives = 3;
        self.game_over = false;
        self.paused = false;
    }
}

/// Spawn the platformer player entity.
pub fn spawn_platformer_player(
    world: &mut EcsWorld,
    position: Vec3,
) -> hecs::Entity {
    world.spawn((
        Transform {
            position,
            rotation: Vec3::ZERO,
            scale: Vec3::new(0.8, 0.8, 0.8),
        },
        Name("Platformer Player".to_string()),
        MeshComponent("Capsule".into()),
        Material {
            base_color: Vec3::new(0.2, 0.6, 1.0),
            alpha: 1.0,
            roughness: 0.4,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
        },
        PlatformerPlayer::default(),
    ))
}

/// Spawn a platform (box) entity.
pub fn spawn_platform(
    world: &mut EcsWorld,
    position: Vec3,
    half_extents: Vec3,
    color: Vec3,
) -> hecs::Entity {
    world.spawn((
        Transform {
            position,
            rotation: Vec3::ZERO,
            scale: half_extents * 2.0,
        },
        Name("Platform".to_string()),
        MeshComponent("Cube".into()),
        Material {
            base_color: color,
            alpha: 1.0,
            roughness: 0.7,
            metallic: 0.0,
            ao: 1.0,
            emissive: 0.0,
        },
        Platform { half_extents },
    ))
}

/// Spawn a collectible coin.
pub fn spawn_coin(
    world: &mut EcsWorld,
    position: Vec3,
) -> hecs::Entity {
    world.spawn((
        Transform {
            position,
            rotation: Vec3::ZERO,
            scale: Vec3::splat(0.4),
        },
        Name("Coin".to_string()),
        MeshComponent("Sphere".into()),
        Material {
            base_color: Vec3::new(1.0, 0.85, 0.1),
            alpha: 1.0,
            roughness: 0.2,
            metallic: 1.0,
            ao: 1.0,
            emissive: 0.3,
        },
        PlatformerCoin { collected: false, bob_offset: position.x + position.z },
    ))
}

/// Build a sample platformer level in the ECS world.
pub fn spawn_platformer_level(world: &mut EcsWorld) -> Vec<hecs::Entity> {
    let mut entities = Vec::new();

    // Starting platform
    entities.push(spawn_platform(world, Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, 0.5, 3.0), Vec3::new(0.4, 0.35, 0.3)));

    // Stair-step platforms
    entities.push(spawn_platform(world, Vec3::new(5.0, 1.5, 0.0), Vec3::new(2.0, 0.5, 2.0), Vec3::new(0.5, 0.4, 0.3)));
    entities.push(spawn_platform(world, Vec3::new(10.0, 3.0, 0.0), Vec3::new(2.0, 0.5, 2.0), Vec3::new(0.45, 0.45, 0.35)));
    entities.push(spawn_platform(world, Vec3::new(15.0, 4.5, 0.0), Vec3::new(2.0, 0.5, 2.0), Vec3::new(0.4, 0.5, 0.35)));

    // Side platforms
    entities.push(spawn_platform(world, Vec3::new(5.0, 2.0, 4.0), Vec3::new(1.5, 0.5, 1.5), Vec3::new(0.35, 0.4, 0.45)));
    entities.push(spawn_platform(world, Vec3::new(10.0, 3.5, 4.0), Vec3::new(1.5, 0.5, 1.5), Vec3::new(0.4, 0.45, 0.5)));
    entities.push(spawn_platform(world, Vec3::new(5.0, 2.0, -4.0), Vec3::new(1.5, 0.5, 1.5), Vec3::new(0.5, 0.35, 0.4)));
    entities.push(spawn_platform(world, Vec3::new(10.0, 3.5, -4.0), Vec3::new(1.5, 0.5, 1.5), Vec3::new(0.45, 0.4, 0.45)));

    // Long bridge
    entities.push(spawn_platform(world, Vec3::new(20.0, 4.5, 0.0), Vec3::new(4.0, 0.5, 1.5), Vec3::new(0.3, 0.5, 0.4)));

    // Final platform
    entities.push(spawn_platform(world, Vec3::new(28.0, 5.0, 0.0), Vec3::new(3.0, 0.5, 3.0), Vec3::new(0.5, 0.5, 0.3)));

    // Coins
    entities.push(spawn_coin(world, Vec3::new(5.0, 3.0, 0.0)));
    entities.push(spawn_coin(world, Vec3::new(10.0, 4.5, 0.0)));
    entities.push(spawn_coin(world, Vec3::new(15.0, 6.0, 0.0)));
    entities.push(spawn_coin(world, Vec3::new(5.0, 3.5, 4.0)));
    entities.push(spawn_coin(world, Vec3::new(10.0, 5.0, 4.0)));
    entities.push(spawn_coin(world, Vec3::new(5.0, 3.5, -4.0)));
    entities.push(spawn_coin(world, Vec3::new(10.0, 5.0, -4.0)));
    entities.push(spawn_coin(world, Vec3::new(20.0, 6.0, 0.0)));
    entities.push(spawn_coin(world, Vec3::new(28.0, 6.5, 0.0)));

    entities
}

/// Clear all platformer-specific entities from the world.
pub fn clear_platformer_level(world: &mut EcsWorld) {
    let mut to_remove: Vec<hecs::Entity> = Vec::new();
    for (e, _) in world.query_mut::<(hecs::Entity, &PlatformerPlayer)>() {
        to_remove.push(e);
    }
    for (e, _) in world.query_mut::<(hecs::Entity, &Platform)>() {
        to_remove.push(e);
    }
    for (e, _) in world.query_mut::<(hecs::Entity, &PlatformerCoin)>() {
        to_remove.push(e);
    }
    for e in to_remove {
        let _ = world.despawn(e);
    }
}

/// Update the platformer game logic for a frame.
pub fn update_platformer(
    world: &mut EcsWorld,
    game: &mut PlatformerGame,
    input: &rustix_platform::input::InputManager,
    audio: Option<&AudioEngine>,
    cam: &mut EditorCamera,
    dt: f32,
) {
    if game.paused || game.game_over {
        return;
    }

    // Find player entity
    let player_entity = match find_player_entity(world) {
        Some(e) => e,
        None => return,
    };

    // Read input
    let kb = input.keyboard();
    let mut move_input = Vec3::ZERO;
    if kb.down(KeyCode::W) || kb.down(KeyCode::Up) {
        move_input.z -= 1.0;
    }
    if kb.down(KeyCode::S) || kb.down(KeyCode::Down) {
        move_input.z += 1.0;
    }
    if kb.down(KeyCode::A) || kb.down(KeyCode::Left) {
        move_input.x -= 1.0;
    }
    if kb.down(KeyCode::D) || kb.down(KeyCode::Right) {
        move_input.x += 1.0;
    }
    if move_input != Vec3::ZERO {
        move_input = move_input.normalize();
    }

    // Jump input
    let jump_pressed = kb.just_pressed(KeyCode::Space);

    // Update player
    let mut player_pos = Vec3::ZERO;
    {
        let mut player = world.get::<&mut PlatformerPlayer>(player_entity).unwrap();
        let mut transform = world.get::<&mut Transform>(player_entity).unwrap();

        // Coyote time & jump buffer
        if player.on_ground {
            player.coyote_time = COYOTE_TIME_MAX;
        } else {
            player.coyote_time -= dt;
        }

        if jump_pressed {
            player.jump_buffer = JUMP_BUFFER_MAX;
        } else {
            player.jump_buffer -= dt;
        }

        // Horizontal movement
        player.velocity.x = move_input.x * MOVE_SPEED;
        player.velocity.z = move_input.z * MOVE_SPEED;

        // Jump
        if player.jump_buffer > 0.0 && player.coyote_time > 0.0 && player.jump_cooldown <= 0.0 {
            player.velocity.y = JUMP_VELOCITY;
            player.on_ground = false;
            player.jump_buffer = 0.0;
            player.coyote_time = 0.0;
            player.jump_cooldown = 0.15;
            if let Some(a) = audio {
                let _ = a.play_sound(std::path::Path::new("assets/sounds/jump.wav"), 0.6, false);
            }
        }

        if player.jump_cooldown > 0.0 {
            player.jump_cooldown -= dt;
        }

        // Gravity
        player.velocity.y += GRAVITY * dt;

        // Apply velocity with collision
        let mut new_pos = transform.position + player.velocity * dt;

        // Gather platforms
        let platforms: Vec<(Vec3, Vec3)> = {
            let mut v = Vec::new();
            for (t, p) in world.query::<(&Transform, &Platform)>().iter() {
                v.push((t.position, p.half_extents));
            }
            v
        };

        // AABB collision resolution
        let player_half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT + PLAYER_RADIUS, PLAYER_RADIUS);
        player.on_ground = false;

        for (plat_pos, plat_half) in platforms {
            if let Some((push, hit_ground)) = resolve_aabb(new_pos, player_half, plat_pos, plat_half) {
                new_pos += push;
                if hit_ground && push.y > 0.0 {
                    player.on_ground = true;
                    player.velocity.y = 0.0;
                }
                if push.y < 0.0 && player.velocity.y > 0.0 {
                    player.velocity.y = 0.0;
                }
            }
        }

        transform.position = new_pos;
        player_pos = new_pos;

        // Respawn if fell off
        if new_pos.y < RESPAWN_Y {
            game.lives -= 1;
            if game.lives <= 0 {
                game.game_over = true;
            } else {
                transform.position = game.respawn_pos;
                player.velocity = Vec3::ZERO;
                player.on_ground = false;
                if let Some(a) = audio {
                    let _ = a.play_sound(std::path::Path::new("assets/sounds/thump.wav"), 0.5, false);
                }
            }
        }
    }

    // Update coins
    let mut coins_to_collect = Vec::new();
    {
        for (entity, coin, transform) in world.query_mut::<(hecs::Entity, &mut PlatformerCoin, &mut Transform)>() {
            if coin.collected {
                continue;
            }
            // Bob animation
            let bob = (transform.position.x + transform.position.z + game.score as f32 * 0.01).sin() * 0.2;
            transform.position.y += bob * dt * 2.0;

            // Collect check (sphere vs sphere)
            let dist = (player_pos - transform.position).length();
            if dist < 0.8 {
                coin.collected = true;
                coins_to_collect.push(entity);
            }
        }
    }

    for coin_entity in coins_to_collect {
        let _ = world.despawn(coin_entity);
        game.coins_collected += 1;
        game.score += 100;
        if let Some(a) = audio {
            let _ = a.play_sound(std::path::Path::new("assets/sounds/coin.wav"), 0.5, false);
        }
    }

    // Camera follow
    let target = player_pos + Vec3::new(0.0, 3.0, 6.0);
    cam.position = cam.position.lerp(target, 5.0 * dt);
    cam.center = player_pos + Vec3::new(0.0, 1.0, 0.0);
}

fn find_player_entity(world: &EcsWorld) -> Option<hecs::Entity> {
    world.query::<(hecs::Entity, &PlatformerPlayer)>()
        .iter()
        .next()
        .map(|(e, _)| e)
}

/// Resolve AABB overlap between player and platform.
/// Returns (push_vector, true if ground collision).
fn resolve_aabb(
    player_center: Vec3,
    player_half: Vec3,
    plat_center: Vec3,
    plat_half: Vec3,
) -> Option<(Vec3, bool)> {
    let min_a = player_center - player_half;
    let max_a = player_center + player_half;
    let min_b = plat_center - plat_half;
    let max_b = plat_center + plat_half;

    if min_a.x >= max_b.x || max_a.x <= min_b.x ||
       min_a.y >= max_b.y || max_a.y <= min_b.y ||
       min_a.z >= max_b.z || max_a.z <= min_b.z {
        return None;
    }

    let overlap_x = (max_a.x.min(max_b.x) - min_a.x.max(min_b.x)).min((max_b.x - min_b.x) * 0.5);
    let overlap_y = (max_a.y.min(max_b.y) - min_a.y.max(min_b.y)).min((max_b.y - min_b.y) * 0.5);
    let overlap_z = (max_a.z.min(max_b.z) - min_a.z.max(min_b.z)).min((max_b.z - min_b.z) * 0.5);

    // Prefer resolving the smallest overlap axis
    let mut push = Vec3::ZERO;
    let mut is_ground = false;

    if overlap_x < overlap_y && overlap_x < overlap_z {
        push.x = if player_center.x < plat_center.x { -overlap_x } else { overlap_x };
    } else if overlap_y < overlap_z {
        push.y = if player_center.y < plat_center.y { -overlap_y } else { overlap_y };
        is_ground = player_center.y > plat_center.y;
    } else {
        push.z = if player_center.z < plat_center.z { -overlap_z } else { overlap_z };
    }

    Some((push, is_ground))
}

/// Render the platformer HUD using egui.
pub fn render_platformer_ui(ctx: &egui::Context, game: &mut PlatformerGame) {
    if game.paused {
        let panel = egui::CentralPanel::default();
        panel.show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(200.0);
                ui.heading("PAUSED");
                ui.add_space(20.0);
                if ui.button("Resume").clicked() {
                    game.paused = false;
                }
                if ui.button("Restart").clicked() {
                    game.reset();
                }
            });
        });
        return;
    }

    if game.game_over {
        let panel = egui::CentralPanel::default();
        panel.show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(200.0);
                ui.heading("GAME OVER");
                ui.label(format!("Score: {}", game.score));
                ui.label(format!("Coins: {}", game.coins_collected));
                ui.add_space(20.0);
                if ui.button("Restart").clicked() {
                    game.reset();
                }
            });
        });
        return;
    }

    // In-game HUD
    egui::Area::new("platformer_hud".into())
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(10.0, 10.0))
        .show(ctx, |ui| {
            ui.label(format!("Score: {}", game.score));
            ui.label(format!("Coins: {}", game.coins_collected));
            ui.label(format!("Lives: {}", game.lives));
        });
}
