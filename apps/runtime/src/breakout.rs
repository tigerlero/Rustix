//! 2D Breakout game logic and UI rendering.

use rustix_audio::{AudioEngine, SoundInstance};

/// Playfield width in game units.
pub const PLAY_WIDTH: f32 = 10.0;
/// Playfield height in game units.
pub const PLAY_HEIGHT: f32 = 14.0;
/// Paddle half-width.
pub const PADDLE_HALF_W: f32 = 1.2;
/// Paddle height.
pub const PADDLE_H: f32 = 0.25;
/// Ball radius.
pub const BALL_RADIUS: f32 = 0.18;
/// Base ball speed.
pub const BASE_BALL_SPEED: f32 = 6.0;
/// Max ball speed.
pub const MAX_BALL_SPEED: f32 = 14.0;
/// Brick width.
pub const BRICK_W: f32 = 1.2;
/// Brick height.
pub const BRICK_H: f32 = 0.4;
/// Rows of bricks per level.
pub const BRICK_ROWS: usize = 5;
/// Columns of bricks per level.
pub const BRICK_COLS: usize = 7;

/// Brick type / strength.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrickType {
    Normal,
    Hard,
    Unbreakable,
}

impl BrickType {
    pub fn color(&self) -> [u8; 3] {
        match self {
            BrickType::Normal => [230, 80, 60],
            BrickType::Hard => [255, 180, 40],
            BrickType::Unbreakable => [80, 80, 90],
        }
    }

    pub fn hits(&self) -> u32 {
        match self {
            BrickType::Normal => 1,
            BrickType::Hard => 2,
            BrickType::Unbreakable => 999,
        }
    }
}

/// A single brick.
#[derive(Debug, Clone)]
pub struct Brick {
    pub x: f32,
    pub y: f32,
    pub kind: BrickType,
    pub hp: u32,
    pub alive: bool,
}

/// Power-up type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PowerUpType {
    Expand,
    MultiBall,
    SlowBall,
}

impl PowerUpType {
    pub fn color(&self) -> [u8; 3] {
        match self {
            PowerUpType::Expand => [100, 255, 100],
            PowerUpType::MultiBall => [100, 200, 255],
            PowerUpType::SlowBall => [255, 200, 100],
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PowerUpType::Expand => "EXPAND",
            PowerUpType::MultiBall => "MULTI",
            PowerUpType::SlowBall => "SLOW",
        }
    }
}

/// A falling power-up.
#[derive(Debug, Clone)]
pub struct PowerUp {
    pub x: f32,
    pub y: f32,
    pub kind: PowerUpType,
    pub active: bool,
}

/// A single ball.
#[derive(Debug, Clone)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub active: bool,
}

impl Ball {
    pub fn speed(&self) -> f32 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }

    pub fn set_speed(&mut self, target: f32) {
        let current = self.speed();
        if current > 0.0 {
            self.vx = self.vx / current * target;
            self.vy = self.vy / current * target;
        }
    }
}

fn config_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .map(|d| d.join("rustix"))
        .unwrap_or_else(|| std::path::PathBuf::from("rustix_config"))
}

fn save_high_score(score: u64) {
    let path = config_dir().join("breakout_highscore.json");
    let data = serde_json::json!({ "high_score": score });
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")));
    let _ = std::fs::write(&path, data.to_string());
}

fn load_high_score() -> u64 {
    let path = config_dir().join("breakout_highscore.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            return val.get("high_score").and_then(|v| v.as_u64()).unwrap_or(0);
        }
    }
    0
}

/// Breakout game state.
#[derive(Debug)]
pub struct BreakoutGame {
    pub score: u64,
    pub high_score: u64,
    pub lives: u32,
    pub level: u32,
    pub paddle_x: f32,
    pub paddle_half_w: f32,
    pub balls: Vec<Ball>,
    pub bricks: Vec<Brick>,
    pub powerups: Vec<PowerUp>,
    pub game_over: bool,
    pub paused: bool,
    pub ball_launched: bool,
    pub screen_shake: f32,
    pub expand_timer: f32,
    pub slow_timer: f32,
    pub sfx_instances: Vec<SoundInstance>,
    pub particles: Vec<Particle>,
}

#[derive(Debug, Clone)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub color: [u8; 3],
    pub size: f32,
}

impl Default for BreakoutGame {
    fn default() -> Self {
        Self::new()
    }
}

impl BreakoutGame {
    pub fn new() -> Self {
        let mut game = Self {
            score: 0,
            high_score: load_high_score(),
            lives: 3,
            level: 1,
            paddle_x: 0.0,
            paddle_half_w: PADDLE_HALF_W,
            balls: Vec::new(),
            bricks: Vec::new(),
            powerups: Vec::new(),
            game_over: false,
            paused: false,
            ball_launched: false,
            screen_shake: 0.0,
            expand_timer: 0.0,
            slow_timer: 0.0,
            sfx_instances: Vec::new(),
            particles: Vec::new(),
        };
        game.reset_level();
        game
    }

    fn play_sfx(sfx_instances: &mut Vec<SoundInstance>, engine: &AudioEngine, name: &str) {
        let path = std::path::PathBuf::from("assets/sounds").join(name);
        if let Ok(inst) = engine.play_sound_file(&path) {
            sfx_instances.push(inst);
        }
    }

    pub fn reset_level(&mut self) {
        self.balls.clear();
        self.balls.push(Ball {
            x: 0.0,
            y: -5.0,
            vx: 0.0,
            vy: 0.0,
            active: true,
        });
        self.ball_launched = false;
        self.paddle_x = 0.0;
        self.powerups.clear();
        self.particles.clear();
        self.expand_timer = 0.0;
        self.slow_timer = 0.0;
        self.paddle_half_w = PADDLE_HALF_W;

        self.bricks.clear();
        let start_x = -((BRICK_COLS as f32) * BRICK_W) / 2.0 + BRICK_W / 2.0;
        let start_y = 4.5;
        for row in 0..BRICK_ROWS {
            for col in 0..BRICK_COLS {
                let kind = match (row, col) {
                    (0, _) => BrickType::Hard,
                    (r, _) if r == BRICK_ROWS - 1 => BrickType::Unbreakable,
                    _ => BrickType::Normal,
                };
                let hp = kind.hits();
                self.bricks.push(Brick {
                    x: start_x + col as f32 * BRICK_W,
                    y: start_y - row as f32 * BRICK_H,
                    kind,
                    hp,
                    alive: true,
                });
            }
        }
    }

    pub fn reset(&mut self) {
        self.score = 0;
        self.lives = 3;
        self.level = 1;
        self.game_over = false;
        self.paused = false;
        self.screen_shake = 0.0;
        self.sfx_instances.clear();
        self.reset_level();
    }

    pub fn launch_ball(&mut self, engine: Option<&AudioEngine>) {
        if self.ball_launched || self.game_over || self.paused { return; }
        self.ball_launched = true;
        for ball in &mut self.balls {
            if !ball.active { continue; }
            ball.vx = rand_f32(-2.0, 2.0);
            ball.vy = BASE_BALL_SPEED;
            ball.set_speed(BASE_BALL_SPEED);
        }
        if let Some(e) = engine {
            Self::play_sfx(&mut self.sfx_instances, e, "click.wav");
        }
    }

    pub fn move_paddle(&mut self, delta: f32) {
        if self.game_over || self.paused { return; }
        self.paddle_x = (self.paddle_x + delta).clamp(
            -PLAY_WIDTH / 2.0 + self.paddle_half_w,
            PLAY_WIDTH / 2.0 - self.paddle_half_w,
        );
        if !self.ball_launched {
            for ball in &mut self.balls {
                if ball.active {
                    ball.x = self.paddle_x;
                }
            }
        }
    }

    /// Update game state with delta time.
    pub fn update(&mut self, dt: f32, audio_engine: Option<&AudioEngine>) {
        // Clean up finished sounds
        self.sfx_instances.retain(|inst| inst.is_playing());

        if self.game_over || self.paused { return; }

        // Decay screen shake
        if self.screen_shake > 0.0 {
            self.screen_shake -= dt * 4.0;
            if self.screen_shake < 0.0 { self.screen_shake = 0.0; }
        }

        // Update expand timer
        if self.expand_timer > 0.0 {
            self.expand_timer -= dt;
            if self.expand_timer <= 0.0 {
                self.paddle_half_w = PADDLE_HALF_W;
            }
        }

        // Update slow timer
        if self.slow_timer > 0.0 {
            self.slow_timer -= dt;
            if self.slow_timer <= 0.0 {
                for ball in &mut self.balls {
                    if ball.active && ball.speed() > BASE_BALL_SPEED {
                        ball.set_speed(BASE_BALL_SPEED.max(ball.speed() * 0.7));
                    }
                }
            }
        }

        // Update particles
        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);

        // Update balls — pre-borrow fields to satisfy borrow checker
        let mut lost_ball = false;
        {
            let paddle_x = self.paddle_x;
            let paddle_half_w = self.paddle_half_w;
            let slow_timer = self.slow_timer;
            let balls = &mut self.balls;
            let bricks = &mut self.bricks;
            let particles = &mut self.particles;
            let powerups_list = &mut self.powerups;
            let score = &mut self.score;
            let screen_shake = &mut self.screen_shake;
            let sfx_instances = &mut self.sfx_instances;

            for ball in balls {
                if !ball.active { continue; }

                let speed = if slow_timer > 0.0 {
                    ball.speed().min(BASE_BALL_SPEED * 0.7)
                } else {
                    ball.speed()
                };
                ball.set_speed(speed);

                ball.x += ball.vx * dt;
                ball.y += ball.vy * dt;

                // Wall collisions
                if ball.x - BALL_RADIUS < -PLAY_WIDTH / 2.0 {
                    ball.x = -PLAY_WIDTH / 2.0 + BALL_RADIUS;
                    ball.vx = ball.vx.abs();
                    if let Some(e) = audio_engine { Self::play_sfx(sfx_instances, e, "beep.wav"); }
                }
                if ball.x + BALL_RADIUS > PLAY_WIDTH / 2.0 {
                    ball.x = PLAY_WIDTH / 2.0 - BALL_RADIUS;
                    ball.vx = -ball.vx.abs();
                    if let Some(e) = audio_engine { Self::play_sfx(sfx_instances, e, "beep.wav"); }
                }
                if ball.y + BALL_RADIUS > PLAY_HEIGHT / 2.0 {
                    ball.y = PLAY_HEIGHT / 2.0 - BALL_RADIUS;
                    ball.vy = -ball.vy.abs();
                    if let Some(e) = audio_engine { Self::play_sfx(sfx_instances, e, "beep.wav"); }
                }

                // Paddle collision
                let paddle_y = -PLAY_HEIGHT / 2.0 + 1.0;
                if ball.y - BALL_RADIUS < paddle_y + PADDLE_H / 2.0
                    && ball.y + BALL_RADIUS > paddle_y - PADDLE_H / 2.0
                    && ball.x > paddle_x - paddle_half_w - BALL_RADIUS
                    && ball.x < paddle_x + paddle_half_w + BALL_RADIUS
                    && ball.vy < 0.0
                {
                    ball.y = paddle_y + PADDLE_H / 2.0 + BALL_RADIUS;
                    let hit_offset = (ball.x - paddle_x) / paddle_half_w;
                    let angle = hit_offset * 0.8;
                    let spd = ball.speed().min(MAX_BALL_SPEED);
                    ball.vx = angle * spd;
                    ball.vy = spd * (1.0 - angle * angle).sqrt();
                    if let Some(e) = audio_engine { Self::play_sfx(sfx_instances, e, "whoosh.wav"); }
                }

                // Brick collisions
                for brick in bricks.iter_mut() {
                    if !brick.alive { continue; }
                    if ball.x + BALL_RADIUS > brick.x - BRICK_W / 2.0
                        && ball.x - BALL_RADIUS < brick.x + BRICK_W / 2.0
                        && ball.y + BALL_RADIUS > brick.y - BRICK_H / 2.0
                        && ball.y - BALL_RADIUS < brick.y + BRICK_H / 2.0
                    {
                        let dx = (ball.x - brick.x).abs() - BRICK_W / 2.0;
                        let dy = (ball.y - brick.y).abs() - BRICK_H / 2.0;
                        if dx > dy {
                            ball.vx = -ball.vx;
                        } else {
                            ball.vy = -ball.vy;
                        }

                        if brick.kind != BrickType::Unbreakable {
                            brick.hp -= 1;
                            if brick.hp == 0 {
                                brick.alive = false;
                                *score += match brick.kind {
                                    BrickType::Normal => 10,
                                    BrickType::Hard => 30,
                                    BrickType::Unbreakable => 0,
                                };
                                for _ in 0..8 {
                                    particles.push(Particle {
                                        x: brick.x,
                                        y: brick.y,
                                        vx: rand_f32(-3.0, 3.0),
                                        vy: rand_f32(-3.0, 3.0),
                                        life: rand_f32(0.3, 0.8),
                                        color: brick.kind.color(),
                                        size: rand_f32(2.0, 5.0),
                                    });
                                }
                                *screen_shake = 0.15;
                                if let Some(e) = audio_engine { Self::play_sfx(sfx_instances, e, "thump.wav"); }

                                if rand_f32(0.0, 1.0) < 0.15 {
                                    powerups_list.push(PowerUp {
                                        x: brick.x,
                                        y: brick.y,
                                        kind: match rand_i32(0, 3) {
                                            0 => PowerUpType::Expand,
                                            1 => PowerUpType::MultiBall,
                                            _ => PowerUpType::SlowBall,
                                        },
                                        active: true,
                                    });
                                }
                            } else {
                                if let Some(e) = audio_engine { Self::play_sfx(sfx_instances, e, "beep.wav"); }
                            }
                        } else {
                            if let Some(e) = audio_engine { Self::play_sfx(sfx_instances, e, "beep.wav"); }
                        }
                        break;
                    }
                }

                // Ball lost
                if ball.y + BALL_RADIUS < -PLAY_HEIGHT / 2.0 {
                    ball.active = false;
                    lost_ball = true;
                }
            }
        }

        if lost_ball {
            let active_count = self.balls.iter().filter(|b| b.active).count();
            if active_count == 0 {
                self.lives -= 1;
                self.screen_shake = 0.5;
                if let Some(e) = audio_engine { Self::play_sfx(&mut self.sfx_instances, e, "thump.wav"); }
                if self.lives == 0 {
                    self.game_over = true;
                    if self.score > self.high_score {
                        self.high_score = self.score;
                        save_high_score(self.high_score);
                    }
                } else {
                    self.balls.clear();
                    self.balls.push(Ball {
                        x: self.paddle_x,
                        y: -5.0,
                        vx: 0.0,
                        vy: 0.0,
                        active: true,
                    });
                    self.ball_launched = false;
                }
            }
        }

        // Update powerups falling — pre-borrow fields
        {
            let paddle_x = self.paddle_x;
            let paddle_half_w = self.paddle_half_w;
            let powerups = &mut self.powerups;
            let balls = &mut self.balls;
            let sfx_instances = &mut self.sfx_instances;
            let expand_timer = &mut self.expand_timer;
            let slow_timer = &mut self.slow_timer;
            let paddle_half_w_ref = &mut self.paddle_half_w;

            for pup in powerups.iter_mut() {
                if !pup.active { continue; }
                pup.y -= 3.0 * dt;
                let paddle_y = -PLAY_HEIGHT / 2.0 + 1.0;
                if pup.y < paddle_y + PADDLE_H / 2.0
                    && pup.y > paddle_y - PADDLE_H / 2.0
                    && pup.x > paddle_x - paddle_half_w
                    && pup.x < paddle_x + paddle_half_w
                {
                    pup.active = false;
                    if let Some(e) = audio_engine { Self::play_sfx(sfx_instances, e, "beep.wav"); }
                    match pup.kind {
                        PowerUpType::Expand => {
                            *paddle_half_w_ref = PADDLE_HALF_W * 1.6;
                            *expand_timer = 8.0;
                        }
                        PowerUpType::MultiBall => {
                            let mut new_balls: Vec<Ball> = Vec::new();
                            for ball in balls.iter() {
                                if ball.active {
                                    for angle in [-0.5f32, 0.5f32] {
                                        let spd = ball.speed();
                                        new_balls.push(Ball {
                                            x: ball.x,
                                            y: ball.y,
                                            vx: ball.vx * angle.cos() - ball.vy * angle.sin(),
                                            vy: ball.vx * angle.sin() + ball.vy * angle.cos(),
                                            active: true,
                                        });
                                        new_balls.last_mut().unwrap().set_speed(spd);
                                    }
                                }
                            }
                            balls.extend(new_balls);
                        }
                        PowerUpType::SlowBall => {
                            *slow_timer = 5.0;
                            for ball in balls.iter_mut() {
                                if ball.active {
                                    ball.set_speed(BASE_BALL_SPEED * 0.6);
                                }
                            }
                        }
                    }
                }
            }
        }
        self.powerups.retain(|p| p.active && p.y > -PLAY_HEIGHT / 2.0 - 1.0);

        // Check level clear
        let breakable_left = self.bricks.iter().any(|b| b.alive && b.kind != BrickType::Unbreakable);
        if !breakable_left {
            self.level += 1;
            self.score += 100 * self.level as u64;
            self.reset_level();
            if let Some(e) = audio_engine { Self::play_sfx(&mut self.sfx_instances, e, "beep.wav"); }
        }
    }

    fn spawn_particles(&mut self, x: f32, y: f32, color: [u8; 3]) {
        for _ in 0..8 {
            self.particles.push(Particle {
                x,
                y,
                vx: rand_f32(-3.0, 3.0),
                vy: rand_f32(-3.0, 3.0),
                life: rand_f32(0.3, 0.8),
                color,
                size: rand_f32(2.0, 5.0),
            });
        }
    }

    pub fn on_key(&mut self, key: rustix_platform::input::KeyCode, audio_engine: Option<&AudioEngine>) {
        use rustix_platform::input::KeyCode;
        match key {
            KeyCode::Space => self.launch_ball(audio_engine),
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

/// Render the Breakout game using egui.
pub fn render_breakout_ui(ctx: &egui::Context, game: &mut BreakoutGame) {
    let panel_w = 520.0;
    let panel_h = 680.0;

    egui::Window::new("Breakout 2D")
        .default_pos([60.0, 40.0])
        .default_size([panel_w, panel_h])
        .collapsible(false)
        .show(ctx, |ui| {
            let avail = ui.available_size();
            let w = avail.x;
            let h = avail.y;

            let rect = ui.available_rect_before_wrap();
            let painter = ui.painter_at(rect);

            // Screen shake
            let shake_x = if game.screen_shake > 0.0 {
                rand_f32(-game.screen_shake * 8.0, game.screen_shake * 8.0)
            } else { 0.0 };
            let shake_y = if game.screen_shake > 0.0 {
                rand_f32(-game.screen_shake * 8.0, game.screen_shake * 8.0)
            } else { 0.0 };

            // Background
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));

            // Playfield border
            let margin = 16.0;
            let play_w = w - margin * 2.0;
            let play_h = h - margin * 2.0 - 60.0;
            let play_x = rect.min.x + margin;
            let play_y = rect.min.y + margin;
            let play_rect = egui::Rect::from_min_size(egui::pos2(play_x, play_y), egui::vec2(play_w, play_h));
            painter.rect_filled(play_rect, 4.0, egui::Color32::from_rgb(30, 30, 45));
            painter.rect_stroke(play_rect, 4.0, egui::Stroke::new(2.0, egui::Color32::from_rgb(60, 60, 90)), egui::StrokeKind::Inside);

            let scale_x = play_w / PLAY_WIDTH;
            let scale_y = play_h / PLAY_HEIGHT;
            let center_x = play_x + play_w / 2.0;
            let center_y = play_y + play_h / 2.0;

            let to_screen = |gx: f32, gy: f32| -> egui::Pos2 {
                egui::pos2(center_x + (gx + shake_x) * scale_x, center_y - (gy + shake_y) * scale_y)
            };

            // Draw bricks
            for brick in &game.bricks {
                if !brick.alive { continue; }
                let pos = to_screen(brick.x - BRICK_W / 2.0, brick.y + BRICK_H / 2.0);
                let bw = BRICK_W * scale_x - 2.0;
                let bh = BRICK_H * scale_y - 2.0;
                let color = brick.kind.color();
                let brect = egui::Rect::from_min_size(pos, egui::vec2(bw, bh));
                painter.rect_filled(brect, 3.0, egui::Color32::from_rgb(color[0], color[1], color[2]));
                painter.rect_stroke(brect, 3.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 255)), egui::StrokeKind::Inside);
                if brick.kind == BrickType::Hard && brick.hp == 2 {
                    painter.rect_stroke(brect.shrink(3.0), 2.0, egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 220, 100)), egui::StrokeKind::Inside);
                }
            }

            // Draw paddle
            let paddle_y = -PLAY_HEIGHT / 2.0 + 1.0;
            let paddle_pos = to_screen(game.paddle_x - game.paddle_half_w, paddle_y + PADDLE_H / 2.0);
            let pw = game.paddle_half_w * 2.0 * scale_x;
            let ph = PADDLE_H * scale_y;
            let paddle_rect = egui::Rect::from_min_size(paddle_pos, egui::vec2(pw, ph));
            painter.rect_filled(paddle_rect, 4.0, egui::Color32::from_rgb(100, 200, 255));
            painter.rect_stroke(paddle_rect, 4.0, egui::Stroke::new(2.0, egui::Color32::WHITE), egui::StrokeKind::Inside);

            // Draw balls
            for ball in &game.balls {
                if !ball.active { continue; }
                let bpos = to_screen(ball.x, ball.y);
                let br = BALL_RADIUS * scale_x.min(scale_y);
                painter.circle_filled(bpos, br, egui::Color32::WHITE);
                // Ball trail
                painter.circle_stroke(bpos, br * 1.4, egui::Stroke::new(2.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 80)));
            }

            // Draw powerups
            for pup in &game.powerups {
                if !pup.active { continue; }
                let ppos = to_screen(pup.x, pup.y);
                let pr = 0.25 * scale_x.min(scale_y);
                let color = pup.kind.color();
                painter.circle_filled(ppos, pr, egui::Color32::from_rgb(color[0], color[1], color[2]));
                painter.circle_stroke(ppos, pr, egui::Stroke::new(2.0, egui::Color32::WHITE));
            }

            // Draw particles
            for p in &game.particles {
                let ppos = to_screen(p.x, p.y);
                let size = p.size * (p.life / 0.8).min(1.0);
                painter.circle_filled(ppos, size, egui::Color32::from_rgba_premultiplied(p.color[0], p.color[1], p.color[2], (p.life * 255.0) as u8));
            }

            // HUD
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading(format!("Score: {}", game.score));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Level: {}", game.level));
                    ui.label(format!("Lives: {}", game.lives));
                });
            });
            if !game.ball_launched && !game.game_over {
                ui.label(egui::RichText::new("Press SPACE to launch!").color(egui::Color32::YELLOW).strong());
            }
            if game.expand_timer > 0.0 {
                ui.label(egui::RichText::new("EXPAND ACTIVE").color(egui::Color32::GREEN).strong());
            }
            if game.slow_timer > 0.0 {
                ui.label(egui::RichText::new("SLOW ACTIVE").color(egui::Color32::from_rgb(255, 200, 100)).strong());
            }

            // Game Over overlay
            if game.game_over {
                let overlay = egui::Rect::from_min_size(rect.min, egui::vec2(w, h));
                painter.rect_filled(overlay, 0.0, egui::Color32::from_rgba_premultiplied(0, 0, 0, 180));
                let center = overlay.center();
                painter.text(
                    center - egui::vec2(0.0, 40.0),
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

            // Controls
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
                ui.label("Mouse = Move paddle, Space = Launch");
            });
        });
}
