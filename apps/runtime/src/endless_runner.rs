//! 3D Endless Runner game logic and UI rendering.

use rustix_core::math::Vec3;
use rustix_audio::{AudioEngine, SoundInstance};

/// Number of lanes.
pub const NUM_LANES: usize = 3;
/// Lane width.
pub const LANE_WIDTH: f32 = 2.0;
/// Jump duration in seconds.
pub const JUMP_DURATION: f32 = 0.5;
/// Jump height.
pub const JUMP_HEIGHT: f32 = 1.5;
/// Initial forward speed.
pub const INITIAL_SPEED: f32 = 8.0;
/// Speed increase per second.
pub const SPEED_INCREMENT: f32 = 0.3;
/// Max speed.
pub const MAX_SPEED: f32 = 30.0;
/// Distance between obstacle spawns.
pub const SPAWN_INTERVAL: f32 = 15.0;
/// Minimum distance between obstacles.
pub const MIN_OBSTACLE_GAP: f32 = 6.0;

/// Obstacle type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ObstacleType {
    Box,
    Barrier,
    LowBarrier,
}

impl ObstacleType {
    pub fn color(&self) -> [u8; 3] {
        match self {
            ObstacleType::Box => [180, 80, 60],
            ObstacleType::Barrier => [160, 40, 40],
            ObstacleType::LowBarrier => [60, 120, 60],
        }
    }

    pub fn height(&self) -> f32 {
        match self {
            ObstacleType::Box => 1.0,
            ObstacleType::Barrier => 1.5,
            ObstacleType::LowBarrier => 0.5,
        }
    }

    pub fn jumpable(&self) -> bool {
        matches!(self, ObstacleType::Box | ObstacleType::LowBarrier)
    }
}

/// A single obstacle on the track.
#[derive(Debug, Clone)]
pub struct Obstacle {
    pub lane: i32,
    pub z: f32,
    pub kind: ObstacleType,
    pub passed: bool,
}

/// Power-up type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PowerUpType {
    Coin,
    SpeedBoost,
    Shield,
    Magnet,
}

impl PowerUpType {
    pub fn color(&self) -> [u8; 3] {
        match self {
            PowerUpType::Coin => [255, 215, 0],
            PowerUpType::SpeedBoost => [100, 200, 255],
            PowerUpType::Shield => [150, 255, 150],
            PowerUpType::Magnet => [255, 100, 200],
        }
    }
}

/// A collectible power-up.
#[derive(Debug, Clone)]
pub struct PowerUp {
    pub lane: i32,
    pub z: f32,
    pub kind: PowerUpType,
    pub collected: bool,
}

/// Parallax cloud for background.
#[derive(Debug, Clone)]
pub struct Cloud {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub size: f32,
    pub speed: f32,
}

fn config_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .map(|d| d.join("rustix"))
        .unwrap_or_else(|| std::path::PathBuf::from("rustix_config"))
}

fn save_high_score(score: u64) {
    let path = config_dir().join("endless_runner_highscore.json");
    let data = serde_json::json!({ "high_score": score });
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")));
    let _ = std::fs::write(&path, data.to_string());
}

fn load_high_score() -> u64 {
    let path = config_dir().join("endless_runner_highscore.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            return val.get("high_score").and_then(|v| v.as_u64()).unwrap_or(0);
        }
    }
    0
}

/// Endless Runner game state.
#[derive(Debug)]
pub struct EndlessRunnerGame {
    pub score: u64,
    pub high_score: u64,
    pub distance: f32,
    pub speed: f32,
    pub lane: i32,
    pub lane_transition: f32,
    pub is_jumping: bool,
    pub jump_timer: f32,
    pub y: f32,
    pub game_over: bool,
    pub paused: bool,
    pub obstacles: Vec<Obstacle>,
    pub powerups: Vec<PowerUp>,
    pub next_spawn_z: f32,
    pub next_powerup_z: f32,
    pub shield_active: bool,
    pub shield_timer: f32,
    pub coin_count: u32,
    pub run_time: f32,

    pub screen_shake: f32,
    pub combo: u32,
    pub combo_timer: f32,
    pub score_multiplier: f32,
    pub magnet_active: bool,
    pub magnet_timer: f32,
    pub difficulty_level: u32,
    pub clouds: Vec<Cloud>,
    pub sfx_instances: Vec<SoundInstance>,
}

impl Default for EndlessRunnerGame {
    fn default() -> Self {
        Self::new()
    }
}

impl EndlessRunnerGame {
    pub fn new() -> Self {
        let mut game = Self {
            score: 0,
            high_score: 0,
            distance: 0.0,
            speed: INITIAL_SPEED,
            lane: 0,
            lane_transition: 0.0,
            is_jumping: false,
            jump_timer: 0.0,
            y: 0.0,
            game_over: false,
            paused: false,
            obstacles: Vec::new(),
            powerups: Vec::new(),
            next_spawn_z: SPAWN_INTERVAL,
            next_powerup_z: SPAWN_INTERVAL * 0.7,
            shield_active: false,
            shield_timer: 0.0,
            coin_count: 0,
            run_time: 0.0,
            screen_shake: 0.0,
            combo: 0,
            combo_timer: 0.0,
            score_multiplier: 1.0,
            magnet_active: false,
            magnet_timer: 0.0,
            difficulty_level: 1,
            clouds: Vec::new(),
            sfx_instances: Vec::new(),
        };
        game.high_score = load_high_score();
        game.init_clouds();
        game
    }

    fn play_sfx(&mut self, engine: &AudioEngine, name: &str) {
        let path = std::path::PathBuf::from("assets/sounds").join(name);
        if let Ok(inst) = engine.play_sound_file(&path) {
            self.sfx_instances.push(inst);
        }
    }

    fn init_clouds(&mut self) {
        self.clouds.clear();
        for _ in 0..6 {
            self.clouds.push(Cloud {
                x: rand_f32(-300.0, 300.0),
                y: rand_f32(20.0, 120.0),
                z: rand_f32(50.0, 200.0),
                size: rand_f32(30.0, 80.0),
                speed: rand_f32(5.0, 15.0),
            });
        }
    }

    pub fn reset(&mut self) {
        self.score = 0;
        self.distance = 0.0;
        self.speed = INITIAL_SPEED;
        self.lane = 0;
        self.lane_transition = 0.0;
        self.is_jumping = false;
        self.jump_timer = 0.0;
        self.y = 0.0;
        self.game_over = false;
        self.paused = false;
        self.obstacles.clear();
        self.powerups.clear();
        self.next_spawn_z = SPAWN_INTERVAL;
        self.next_powerup_z = SPAWN_INTERVAL * 0.7;
        self.shield_active = false;
        self.shield_timer = 0.0;
        self.coin_count = 0;
        self.run_time = 0.0;
        self.screen_shake = 0.0;
        self.combo = 0;
        self.combo_timer = 0.0;
        self.score_multiplier = 1.0;
        self.magnet_active = false;
        self.magnet_timer = 0.0;
        self.difficulty_level = 1;
        self.clouds.clear();
        self.sfx_instances.clear();
        self.init_clouds();
    }

    /// Get world X position for a lane.
    pub fn lane_x(lane: i32) -> f32 {
        (lane as f32) * LANE_WIDTH
    }

    /// Current smoothed player X position.
    pub fn player_x(&self) -> f32 {
        let target = Self::lane_x(self.lane);
        let current = Self::lane_x(if self.lane_transition > 0.0 {
            self.lane - self.lane_transition.signum() as i32
        } else {
            self.lane
        });
        // Simple lerp for lane transition
        if self.lane_transition > 0.0 {
            let t = 1.0 - self.lane_transition;
            current + (target - current) * t
        } else {
            target
        }
    }

    pub fn jump(&mut self, engine: Option<&AudioEngine>) {
        if self.game_over || self.paused || self.is_jumping { return; }
        self.is_jumping = true;
        self.jump_timer = JUMP_DURATION;
        if let Some(e) = engine {
            self.play_sfx(e, "whoosh.wav");
        }
    }

    pub fn move_lane(&mut self, delta: i32, engine: Option<&AudioEngine>) {
        if self.game_over || self.paused { return; }
        let new_lane = (self.lane + delta).clamp(-1, 1);
        if new_lane != self.lane {
            self.lane = new_lane;
            self.lane_transition = 1.0;
            if let Some(e) = engine {
                self.play_sfx(e, "click.wav");
            }
        }
    }

    /// Update game state with delta time.
    pub fn update(&mut self, dt: f32, audio_engine: Option<&AudioEngine>) {
        if self.game_over || self.paused { return; }

        // Clean up finished sound effects
        self.sfx_instances.retain(|inst| inst.is_playing());

        self.run_time += dt;

        // Difficulty scales with distance
        self.difficulty_level = 1 + (self.distance / 200.0) as u32;

        // Increase speed over time
        self.speed = (self.speed + SPEED_INCREMENT * dt).min(MAX_SPEED);

        // Move forward
        let move_dist = self.speed * dt;
        self.distance += move_dist;
        let base_score = (move_dist * 10.0) as u64;
        self.score += (base_score as f32 * self.score_multiplier) as u64;

        // Update lane transition
        if self.lane_transition > 0.0 {
            self.lane_transition -= dt * 6.0;
            if self.lane_transition < 0.0 {
                self.lane_transition = 0.0;
            }
        }

        // Update jump
        if self.is_jumping {
            self.jump_timer -= dt;
            let progress = 1.0 - (self.jump_timer / JUMP_DURATION);
            self.y = (progress * std::f32::consts::PI).sin() * JUMP_HEIGHT;
            if self.jump_timer <= 0.0 {
                self.is_jumping = false;
                self.y = 0.0;
            }
        }

        // Update shield
        if self.shield_active {
            self.shield_timer -= dt;
            if self.shield_timer <= 0.0 {
                self.shield_active = false;
            }
        }

        // Update magnet
        if self.magnet_active {
            self.magnet_timer -= dt;
            if self.magnet_timer <= 0.0 {
                self.magnet_active = false;
            }
        }

        // Update combo timer
        if self.combo_timer > 0.0 {
            self.combo_timer -= dt;
            if self.combo_timer <= 0.0 {
                self.combo = 0;
                self.score_multiplier = 1.0;
            }
        }

        // Decay screen shake
        if self.screen_shake > 0.0 {
            self.screen_shake -= dt * 3.0;
            if self.screen_shake < 0.0 {
                self.screen_shake = 0.0;
            }
        }

        // Update clouds (parallax)
        for cloud in &mut self.clouds {
            cloud.x -= cloud.speed * dt;
            if cloud.x < -400.0 {
                cloud.x = 400.0;
                cloud.y = rand_f32(20.0, 120.0);
                cloud.size = rand_f32(30.0, 80.0);
            }
        }

        // Spawn obstacles
        let spawn_interval = (SPAWN_INTERVAL - self.difficulty_level as f32 * 0.8).max(6.0);
        if self.distance + 60.0 >= self.next_spawn_z {
            self.spawn_obstacles();
            self.next_spawn_z += spawn_interval + rand_f32(0.0, 3.0);
        }

        // Spawn powerups
        if self.distance + 60.0 >= self.next_powerup_z {
            self.spawn_powerups();
            self.next_powerup_z += SPAWN_INTERVAL * 1.2 + rand_f32(0.0, 6.0);
        }

        let player_z = self.distance;
        let px = self.player_x();
        let py = self.y;
        let player_radius = 0.4;

        // Magnet pulls nearby coins
        if self.magnet_active {
            for pup in &mut self.powerups {
                if pup.collected || pup.kind != PowerUpType::Coin { continue; }
                let pup_z = pup.z;
                let dist_z = (player_z - pup_z).abs();
                if dist_z < 15.0 {
                    // Pull coin toward player's lane
                    let target_lane = self.lane;
                    if pup.lane < target_lane { pup.lane += 1; }
                    else if pup.lane > target_lane { pup.lane -= 1; }
                }
            }
        }

        // Check collisions and track near-misses for combo
        let mut near_miss = false;
        for obs in &mut self.obstacles {
            if obs.passed { continue; }
            let obs_x = Self::lane_x(obs.lane);
            let obs_z = obs.z;
            let dist_z = (player_z - obs_z).abs();
            let dist_x = (px - obs_x).abs();

            if dist_z < 1.0 && dist_x < (LANE_WIDTH * 0.4) {
                let obs_top = obs.kind.height();
                if py + player_radius < obs_top {
                    // Jumped over - counts as a near-miss for combo
                    near_miss = true;
                } else {
                    // Collision!
                    if self.shield_active {
                        self.shield_active = false;
                        obs.passed = true;
                        self.screen_shake = 0.3;
                    } else {
                        self.game_over = true;
                        self.screen_shake = 1.0;
                        if self.score > self.high_score {
                            self.high_score = self.score;
                            save_high_score(self.high_score);
                        }
                        if let Some(e) = audio_engine {
                            self.play_sfx(e, "thump.wav");
                        }
                        return;
                    }
                }
            }
            if player_z > obs_z + 2.0 {
                if !obs.passed {
                    near_miss = true;
                }
                obs.passed = true;
            }
        }

        // Update combo on near-miss or successful dodge
        if near_miss {
            self.combo += 1;
            self.combo_timer = 2.0;
            // Multiplier increases every 5 combo
            self.score_multiplier = 1.0 + (self.combo / 5) as f32 * 0.5;
        }

        // Collect powerups
        let mut coin_collected = false;
        for pup in &mut self.powerups {
            if pup.collected { continue; }
            let pup_x = Self::lane_x(pup.lane);
            let pup_z = pup.z;
            let dist_z = (player_z - pup_z).abs();
            let dist_x = (px - pup_x).abs();

            if dist_z < 1.0 && dist_x < (LANE_WIDTH * 0.4) && py < 1.0 {
                pup.collected = true;
                match pup.kind {
                    PowerUpType::Coin => {
                        self.coin_count += 1;
                        self.score += (500.0 * self.score_multiplier) as u64;
                        coin_collected = true;
                    }
                    PowerUpType::SpeedBoost => {
                        self.speed = (self.speed + 5.0).min(MAX_SPEED);
                        self.screen_shake = 0.2;
                    }
                    PowerUpType::Shield => {
                        self.shield_active = true;
                        self.shield_timer = 5.0;
                    }
                    PowerUpType::Magnet => {
                        self.magnet_active = true;
                        self.magnet_timer = 6.0;
                    }
                }
            }
        }
        if coin_collected {
            if let Some(e) = audio_engine {
                self.play_sfx(e, "beep.wav");
            }
        }

        // Clean up far behind obstacles
        self.obstacles.retain(|o| o.z > player_z - 20.0);
        self.powerups.retain(|p| p.z > player_z - 20.0);
    }

    fn spawn_obstacles(&mut self) {
        let spawn_z = self.distance + 60.0;
        let num_obstacles = rand_i32(1, 3);

        for i in 0..num_obstacles {
            let lane = rand_i32(-1, 2);
            let z = spawn_z + i as f32 * MIN_OBSTACLE_GAP;
            let kind = match rand_i32(0, 3) {
                0 => ObstacleType::Box,
                1 => ObstacleType::Barrier,
                _ => ObstacleType::LowBarrier,
            };
            self.obstacles.push(Obstacle {
                lane,
                z,
                kind,
                passed: false,
            });
        }
    }

    fn spawn_powerups(&mut self) {
        let spawn_z = self.distance + 60.0;
        let lane = rand_i32(-1, 2);
        let kind = match rand_i32(0, 4) {
            0 => PowerUpType::Coin,
            1 => PowerUpType::SpeedBoost,
            2 => PowerUpType::Shield,
            _ => PowerUpType::Magnet,
        };
        self.powerups.push(PowerUp {
            lane,
            z: spawn_z,
            kind,
            collected: false,
        });
    }

    pub fn on_key(&mut self, key: rustix_platform::input::KeyCode, audio_engine: Option<&AudioEngine>) {
        use rustix_platform::input::KeyCode;
        match key {
            KeyCode::Left => self.move_lane(-1, audio_engine),
            KeyCode::Right => self.move_lane(1, audio_engine),
            KeyCode::Space => self.jump(audio_engine),
            _ => {}
        }
    }
}

fn rand_f32(min: f32, max: f32) -> f32 {
    use rand::Rng;
    rand::thread_rng().gen_range(min..max)
}

fn rand_i32(min: i32, max: i32) -> i32 {
    use rand::Rng;
    rand::thread_rng().gen_range(min..max)
}

/// Render the Endless Runner game using egui with a pseudo-3D perspective.
pub fn render_endless_runner_ui(ctx: &egui::Context, game: &mut EndlessRunnerGame) {
    let panel_w = 480.0;
    let panel_h = 640.0;

    egui::Window::new("Endless Runner 3D")
        .default_pos([60.0, 60.0])
        .default_size([panel_w, panel_h])
        .collapsible(false)
        .show(ctx, |ui| {
            let avail = ui.available_size();
            let w = avail.x;
            let h = avail.y;

            // Draw game area
            let rect = ui.available_rect_before_wrap();
            let painter = ui.painter_at(rect);

            // Screen shake offset
            let shake_x = if game.screen_shake > 0.0 {
                rand_f32(-game.screen_shake * 10.0, game.screen_shake * 10.0)
            } else { 0.0 };
            let shake_y = if game.screen_shake > 0.0 {
                rand_f32(-game.screen_shake * 10.0, game.screen_shake * 10.0)
            } else { 0.0 };

            // Sky
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(135, 206, 235));

            // Draw parallax clouds
            let center_x = rect.min.x + w * 0.5;
            for cloud in &game.clouds {
                let cx = center_x + cloud.x + shake_x * 0.3;
                let cy = rect.min.y + cloud.y + shake_y * 0.3;
                painter.circle_filled(
                    egui::pos2(cx, cy),
                    cloud.size,
                    egui::Color32::from_rgba_premultiplied(255, 255, 255, 180),
                );
                painter.circle_filled(
                    egui::pos2(cx + cloud.size * 0.6, cy + cloud.size * 0.2),
                    cloud.size * 0.7,
                    egui::Color32::from_rgba_premultiplied(255, 255, 255, 160),
                );
            }

            // Ground area (bottom half)
            let ground_top = rect.min.y + h * 0.35;
            let ground_rect = egui::Rect::from_min_max(
                egui::pos2(rect.min.x, ground_top),
                egui::pos2(rect.max.x, rect.max.y),
            );
            painter.rect_filled(ground_rect, 0.0, egui::Color32::from_rgb(80, 140, 80));

            // Horizon line
            painter.line_segment(
                [egui::pos2(rect.min.x, ground_top), egui::pos2(rect.max.x, ground_top)],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(60, 100, 60)),
            );

            // Draw lanes
            let lane_y = ground_top + h * 0.1;
            let lane_h = ground_rect.height();
            for i in -1..=1 {
                let lx = center_x + (i as f32) * LANE_WIDTH * 30.0;
                painter.line_segment(
                    [egui::pos2(lx, lane_y), egui::pos2(lx, rect.max.y)],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 60)),
                );
            }

            // Draw perspective grid lines on ground
            for i in 0..8 {
                let t = i as f32 / 8.0;
                let py = lane_y + t * lane_h;
                let spread = 50.0 + t * 200.0;
                painter.line_segment(
                    [egui::pos2(center_x - spread, py), egui::pos2(center_x + spread, py)],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 30)),
                );
            }

            // Speed lines (motion blur effect at high speed)
            if game.speed > 15.0 {
                let alpha = ((game.speed - 15.0) / 15.0 * 80.0) as u8;
                for _ in 0..(game.speed as i32 / 5) {
                    let sx = rand_f32(rect.min.x, rect.max.x);
                    let sy = rand_f32(lane_y, rect.max.y);
                    let slen = rand_f32(20.0, 80.0);
                    painter.line_segment(
                        [egui::pos2(sx, sy), egui::pos2(sx - slen, sy)],
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, alpha)),
                    );
                }
            }

            // Draw obstacles
            let player_z = game.distance;
            for obs in &game.obstacles {
                if obs.passed { continue; }
                let rel_z = obs.z - player_z;
                if rel_z < -5.0 || rel_z > 60.0 { continue; }

                let scale = 200.0 / (rel_z + 5.0);
                let obs_x = center_x + (obs.lane as f32) * LANE_WIDTH * 30.0 * (scale / 40.0).min(1.0) + shake_x;
                let obs_y = lane_y + (1.0 - 5.0 / (rel_z + 5.0)) * lane_h * 0.7 + shake_y;
                let size = scale.clamp(10.0, 60.0);

                let color = obs.kind.color();
                let obs_rect = egui::Rect::from_center_size(
                    egui::pos2(obs_x, obs_y - size * 0.3),
                    egui::vec2(size, size * obs.kind.height()),
                );
                painter.rect_filled(obs_rect, 4.0, egui::Color32::from_rgb(color[0], color[1], color[2]));
                painter.rect_stroke(obs_rect, 4.0, egui::Stroke::new(1.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
            }

            // Draw powerups
            for pup in &game.powerups {
                if pup.collected { continue; }
                let rel_z = pup.z - player_z;
                if rel_z < -5.0 || rel_z > 60.0 { continue; }

                let scale = 200.0 / (rel_z + 5.0);
                let pup_x = center_x + (pup.lane as f32) * LANE_WIDTH * 30.0 * (scale / 40.0).min(1.0) + shake_x;
                let pup_y = lane_y + (1.0 - 5.0 / (rel_z + 5.0)) * lane_h * 0.7 + shake_y;
                let size = scale.clamp(10.0, 40.0);

                let color = pup.kind.color();
                let pup_rect = egui::Rect::from_center_size(
                    egui::pos2(pup_x, pup_y - size),
                    egui::vec2(size, size),
                );
                painter.rect_filled(pup_rect, size * 0.5, egui::Color32::from_rgb(color[0], color[1], color[2]));
                painter.rect_stroke(pup_rect, size * 0.5, egui::Stroke::new(2.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
            }

            // Draw player
            let player_x = center_x + game.player_x() * 30.0 + shake_x;
            let player_y = lane_y + lane_h * 0.75 - game.y * 30.0 + shake_y;
            let player_size = 40.0;
            let player_rect = egui::Rect::from_center_size(
                egui::pos2(player_x, player_y - player_size * 0.5),
                egui::vec2(player_size, player_size),
            );
            painter.rect_filled(player_rect, 8.0, egui::Color32::from_rgb(60, 150, 255));
            painter.rect_stroke(player_rect, 8.0, egui::Stroke::new(2.0, egui::Color32::WHITE), egui::StrokeKind::Inside);

            // Shield glow
            if game.shield_active {
                painter.circle_stroke(
                    egui::pos2(player_x, player_y - player_size * 0.5),
                    player_size * 0.7,
                    egui::Stroke::new(3.0, egui::Color32::from_rgb(150, 255, 150)),
                );
            }

            // Magnet glow
            if game.magnet_active {
                painter.circle_stroke(
                    egui::pos2(player_x, player_y - player_size * 0.5),
                    player_size * 1.2,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 100, 200)),
                );
            }

            // UI overlay
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading(format!("Score: {}", game.score));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Coins: {}", game.coin_count));
                });
            });
            ui.label(format!("Speed: {:.1} | Distance: {:.0}m", game.speed, game.distance));
            if game.shield_active {
                ui.label(egui::RichText::new("SHIELD ACTIVE").color(egui::Color32::GREEN).strong());
            }
            if game.magnet_active {
                ui.label(egui::RichText::new("MAGNET ACTIVE").color(egui::Color32::from_rgb(255, 100, 200)).strong());
            }
            if game.combo > 1 {
                ui.label(
                    egui::RichText::new(format!("COMBO x{} (x{:.1})", game.combo, game.score_multiplier))
                        .color(egui::Color32::from_rgb(255, 200, 50))
                        .strong()
                );
            }

            // Game Over overlay
            if game.game_over {
                let overlay = egui::Rect::from_min_size(rect.min, egui::vec2(w, h));
                painter.rect_filled(overlay, 0.0, egui::Color32::from_rgba_premultiplied(0, 0, 0, 180));
                let center = overlay.center();
                let text_pos = center - egui::vec2(0.0, 40.0);
                painter.text(
                    text_pos,
                    egui::Align2::CENTER_CENTER,
                    "GAME OVER",
                    egui::FontId::proportional(36.0),
                    egui::Color32::WHITE,
                );
                painter.text(
                    center + egui::vec2(0.0, 10.0),
                    egui::Align2::CENTER_CENTER,
                    &format!("Score: {}  High: {}", game.score, game.high_score),
                    egui::FontId::proportional(18.0),
                    egui::Color32::from_gray(200),
                );
            }

            // Pause overlay
            if game.paused && !game.game_over {
                let overlay = egui::Rect::from_min_size(rect.min, egui::vec2(w, h));
                painter.rect_filled(overlay, 0.0, egui::Color32::from_rgba_premultiplied(0, 0, 0, 120));
                painter.text(
                    overlay.center(),
                    egui::Align2::CENTER_CENTER,
                    "PAUSED",
                    egui::FontId::proportional(32.0),
                    egui::Color32::WHITE,
                );
            }

            // Controls hint
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if game.game_over {
                    if ui.button("Restart").clicked() {
                        game.reset();
                    }
                } else {
                    if ui.button(if game.paused { "Resume" } else { "Pause" }).clicked() {
                        game.paused = !game.paused;
                    }
                }
                ui.label("Controls: Left/Right = Switch lane, Space = Jump");
            });
        });
}
