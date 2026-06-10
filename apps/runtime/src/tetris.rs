//! Tetris game logic for Rustix.
//!
//! Self-contained game state with 10x20 board, 7 classic tetromino shapes,
//! SRS rotation, line clearing, scoring, and soft/hard drop.

use std::collections::VecDeque;

pub const BOARD_WIDTH: usize = 10;
pub const BOARD_HEIGHT: usize = 20;
pub const VISIBLE_HEIGHT: usize = 20;

/// A single cell on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Filled(TetrominoType),
}

impl Default for Cell {
    fn default() -> Self { Cell::Empty }
}

/// The 7 classic tetromino shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TetrominoType {
    I, O, T, S, Z, J, L,
}

impl TetrominoType {
    /// Color for each piece (for UI rendering).
    pub fn color(self) -> [u8; 3] {
        match self {
            TetrominoType::I => [0, 255, 255],
            TetrominoType::O => [255, 255, 0],
            TetrominoType::T => [128, 0, 128],
            TetrominoType::S => [0, 255, 0],
            TetrominoType::Z => [255, 0, 0],
            TetrominoType::J => [0, 0, 255],
            TetrominoType::L => [255, 165, 0],
        }
    }

    /// Initial shape definition as 4 (x, y) offsets from piece center.
    pub fn blocks(self) -> [(i32, i32); 4] {
        match self {
            TetrominoType::I => [(-1, 0), (0, 0), (1, 0), (2, 0)],
            TetrominoType::O => [(0, 0), (1, 0), (0, 1), (1, 1)],
            TetrominoType::T => [(-1, 0), (0, 0), (1, 0), (0, 1)],
            TetrominoType::S => [(-1, 0), (0, 0), (0, 1), (1, 1)],
            TetrominoType::Z => [(-1, 1), (0, 1), (0, 0), (1, 0)],
            TetrominoType::J => [(-1, 1), (-1, 0), (0, 0), (1, 0)],
            TetrominoType::L => [(-1, 0), (0, 0), (1, 0), (1, 1)],
        }
    }
}

/// Active falling piece.
#[derive(Debug, Clone)]
pub struct Piece {
    pub kind: TetrominoType,
    pub x: i32,
    pub y: i32,
    pub rotation: usize, // 0, 1, 2, 3
}

impl Piece {
    pub fn new(kind: TetrominoType, x: i32, y: i32) -> Self {
        Self { kind, x, y, rotation: 0 }
    }

    /// Get the absolute board positions of this piece's 4 blocks.
    pub fn absolute_blocks(&self) -> [(i32, i32); 4] {
        let base = self.kind.blocks();
        let rotated = rotate_blocks(&base, self.rotation);
        let mut result = [(0, 0); 4];
        for i in 0..4 {
            result[i] = (rotated[i].0 + self.x, rotated[i].1 + self.y);
        }
        result
    }
}

/// Rotate a set of block offsets by 90-degree increments clockwise.
fn rotate_blocks(blocks: &[(i32, i32); 4], rotation: usize) -> [(i32, i32); 4] {
    let mut result = *blocks;
    for _ in 0..(rotation % 4) {
        for i in 0..4 {
            let (x, y) = result[i];
            result[i] = (-y, x);
        }
    }
    result
}

/// Wall-kick data for SRS (Super Rotation System).
/// For J, L, S, T, Z pieces.
const WALL_KICK_JLSTZ: [[(i32, i32); 5]; 8] = [
    [(0,0), (-1,0), (-1,1), (0,-2), (-1,-2)], // 0->1
    [(0,0), (1,0), (1,-1), (0,2), (1,2)],    // 1->0
    [(0,0), (1,0), (1,1), (0,-2), (1,-2)],    // 1->2
    [(0,0), (-1,0), (-1,-1), (0,2), (-1,2)],  // 2->1
    [(0,0), (1,0), (1,-1), (0,2), (1,2)],     // 2->3
    [(0,0), (-1,0), (-1,1), (0,-2), (-1,-2)], // 3->2
    [(0,0), (-1,0), (-1,-1), (0,2), (-1,2)],  // 3->0
    [(0,0), (1,0), (1,1), (0,-2), (1,-2)],    // 0->3
];

/// Wall-kick data for I piece.
const WALL_KICK_I: [[(i32, i32); 5]; 8] = [
    [(0,0), (-2,0), (1,0), (-2,-1), (1,2)],   // 0->1
    [(0,0), (2,0), (-1,0), (2,1), (-1,-2)],   // 1->0
    [(0,0), (-1,0), (2,0), (-1,2), (2,-1)],   // 1->2
    [(0,0), (1,0), (-2,0), (1,-2), (-2,1)],   // 2->1
    [(0,0), (2,0), (-1,0), (2,1), (-1,-2)],   // 2->3
    [(0,0), (-2,0), (1,0), (-2,-1), (1,2)],   // 3->2
    [(0,0), (1,0), (-2,0), (1,-2), (-2,1)],   // 3->0
    [(0,0), (-1,0), (2,0), (-1,2), (2,-1)],   // 0->3
];

/// Complete Tetris game state.
#[derive(Debug, Clone)]
pub struct TetrisGame {
    pub board: [[Cell; BOARD_WIDTH]; VISIBLE_HEIGHT],
    pub active_piece: Option<Piece>,
    pub hold_piece: Option<TetrominoType>,
    pub queue: VecDeque<TetrominoType>,
    pub score: u32,
    pub lines_cleared: u32,
    pub level: u32,
    pub game_over: bool,
    pub paused: bool,

    // Timing
    pub fall_timer: f32,
    pub lock_delay: f32,
    pub lock_resets: u32,
    pub das_timer: f32,      // delayed auto-shift
    pub das_direction: i32,  // -1 left, 1 right, 0 none
    pub soft_drop_timer: f32,

    // Settings
    pub das: f32,
    pub arr: f32, // auto-repeat rate
    pub soft_drop_factor: f32,
    pub gravity: f32, // cells per second
}

impl Default for TetrisGame {
    fn default() -> Self {
        let mut game = Self {
            board: [[Cell::Empty; BOARD_WIDTH]; VISIBLE_HEIGHT],
            active_piece: None,
            hold_piece: None,
            queue: VecDeque::new(),
            score: 0,
            lines_cleared: 0,
            level: 1,
            game_over: false,
            paused: false,
            fall_timer: 0.0,
            lock_delay: 0.0,
            lock_resets: 0,
            das_timer: 0.0,
            das_direction: 0,
            soft_drop_timer: 0.0,
            das: 0.17,
            arr: 0.0,
            soft_drop_factor: 20.0,
            gravity: 1.0,
        };
        game.refill_queue();
        game.spawn_piece();
        game
    }
}

impl TetrisGame {
    /// Create a new game.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the game to starting state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Refill the piece queue with a 7-bag randomizer.
    fn refill_queue(&mut self) {
        use TetrominoType::*;
        let mut bag = [I, O, T, S, Z, J, L];
        // Fisher-Yates shuffle using a simple RNG
        for i in (1..7).rev() {
            let j = (self.score.wrapping_add(i as u32 * 12345) % (i as u32 + 1)) as usize;
            bag.swap(i, j);
        }
        for piece in bag {
            self.queue.push_back(piece);
        }
    }

    fn next_piece_type(&mut self) -> TetrominoType {
        if self.queue.len() < 7 {
            self.refill_queue();
        }
        self.queue.pop_front().unwrap_or(TetrominoType::I)
    }

    pub fn spawn_piece(&mut self) {
        let kind = self.next_piece_type();
        let spawn_x = BOARD_WIDTH as i32 / 2 - 1;
        let spawn_y = if kind == TetrominoType::I { 18 } else { 19 };
        let piece = Piece::new(kind, spawn_x, spawn_y);

        if !self.is_valid_position(&piece) {
            self.game_over = true;
            return;
        }

        self.active_piece = Some(piece);
        self.fall_timer = 0.0;
        self.lock_delay = 0.0;
        self.lock_resets = 0;
    }

    pub fn is_valid_position(&self, piece: &Piece) -> bool {
        for (bx, by) in piece.absolute_blocks() {
            if bx < 0 || bx >= BOARD_WIDTH as i32 || by < 0 {
                return false;
            }
            if by < VISIBLE_HEIGHT as i32 {
                if let Cell::Filled(_) = self.board[by as usize][bx as usize] {
                    return false;
                }
            }
        }
        true
    }

    pub fn is_on_ground(&self, piece: &Piece) -> bool {
        let mut test = piece.clone();
        test.y -= 1;
        !self.is_valid_position(&test)
    }

    /// Lock the active piece into the board.
    pub fn lock_piece(&mut self) {
        if let Some(ref piece) = self.active_piece {
            for (bx, by) in piece.absolute_blocks() {
                if by >= 0 && by < VISIBLE_HEIGHT as i32 && bx >= 0 && bx < BOARD_WIDTH as i32 {
                    self.board[by as usize][bx as usize] = Cell::Filled(piece.kind);
                }
            }
        }
        self.clear_lines();
        self.spawn_piece();
    }

    fn clear_lines(&mut self) {
        let mut lines = 0;
        let mut y = 0;
        while y < VISIBLE_HEIGHT {
            let full = self.board[y].iter().all(|c| matches!(c, Cell::Filled(_)));
            if full {
                // Shift everything down
                for row in (y + 1..VISIBLE_HEIGHT).rev() {
                    self.board[row - 1] = self.board[row];
                }
                self.board[VISIBLE_HEIGHT - 1] = [Cell::Empty; BOARD_WIDTH];
                lines += 1;
                // Don't increment y, check same row again
            } else {
                y += 1;
            }
        }

        if lines > 0 {
            self.lines_cleared += lines;
            // Scoring: 1=100, 2=300, 3=500, 4=800 * level
            let base = match lines {
                1 => 100,
                2 => 300,
                3 => 500,
                _ => 800,
            };
            self.score += base * self.level;
            self.level = (self.lines_cleared / 10) + 1;
        }
    }

    /// Try to move the active piece by (dx, dy). Returns true if successful.
    pub fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        let piece = match self.active_piece.clone() {
            Some(p) => p,
            None => return false,
        };
        let mut test = piece.clone();
        test.x += dx;
        test.y += dy;
        if self.is_valid_position(&test) {
            self.active_piece = Some(test);
            if dy < 0 && self.is_on_ground(self.active_piece.as_ref().unwrap()) {
                self.lock_delay = 0.0;
                self.lock_resets = 0;
            }
            return true;
        }
        false
    }

    /// Try to rotate the active piece. direction: 1 = clockwise, -1 = counter-clockwise.
    pub fn try_rotate(&mut self, direction: i32) -> bool {
        let piece = match self.active_piece.clone() {
            Some(p) => p,
            None => return false,
        };
        let old_rot = piece.rotation;
        let new_rot = ((old_rot as i32 + direction).rem_euclid(4)) as usize;

        if piece.kind == TetrominoType::O {
            return false; // O doesn't rotate
        }

        let kicks = if piece.kind == TetrominoType::I {
            &WALL_KICK_I
        } else {
            &WALL_KICK_JLSTZ
        };

        let kick_idx = if direction == 1 {
            old_rot * 2
        } else {
            old_rot * 2 + 7
        } % 8;

        for &(kx, ky) in &kicks[kick_idx] {
            let mut test = piece.clone();
            test.rotation = new_rot;
            test.x += kx;
            test.y += ky;
            if self.is_valid_position(&test) {
                self.active_piece = Some(test);
                if self.is_on_ground(self.active_piece.as_ref().unwrap()) && self.lock_resets < 15 {
                    self.lock_delay = 0.0;
                    self.lock_resets += 1;
                }
                return true;
            }
        }
        false
    }

    /// Hold the current piece (swap with held piece or store it).
    pub fn hold(&mut self) {
        if self.game_over || self.paused { return; }
        if let Some(piece) = self.active_piece.take() {
            match self.hold_piece {
                Some(kind) => {
                    self.hold_piece = Some(piece.kind);
                    self.active_piece = Some(Piece::new(kind, BOARD_WIDTH as i32 / 2 - 1, 19));
                    if !self.is_valid_position(self.active_piece.as_ref().unwrap()) {
                        self.game_over = true;
                    }
                }
                None => {
                    self.hold_piece = Some(piece.kind);
                    self.spawn_piece();
                }
            }
        }
    }

    /// Hard drop: move piece to bottom and lock immediately.
    pub fn hard_drop(&mut self) {
        if self.game_over || self.paused { return; }
        while self.try_move(0, -1) {}
        self.lock_piece();
    }

    /// Update game state with delta time.
    pub fn update(&mut self, dt: f32) {
        if self.game_over || self.paused { return; }

        let piece = match self.active_piece.clone() {
            Some(p) => p,
            None => return,
        };
        let gravity = self.gravity * (self.level as f32).powf(0.8);

        if self.is_on_ground(&piece) {
            self.lock_delay += dt;
            if self.lock_delay >= 0.5 {
                self.lock_piece();
                return;
            }
        } else {
            self.fall_timer += dt * gravity;
            while self.fall_timer >= 1.0 {
                self.fall_timer -= 1.0;
                if !self.try_move(0, -1) {
                    self.lock_piece();
                    return;
                }
            }
        }
    }

    /// Handle key press events.
    pub fn on_key(&mut self, key: rustix_platform::input::KeyCode, pressed: bool) {
        if self.game_over || self.paused { return; }

        use rustix_platform::input::KeyCode;

        if pressed {
            match key {
                KeyCode::Left => {
                    self.try_move(-1, 0);
                    self.das_direction = -1;
                    self.das_timer = 0.0;
                }
                KeyCode::Right => {
                    self.try_move(1, 0);
                    self.das_direction = 1;
                    self.das_timer = 0.0;
                }
                KeyCode::Down => {
                    self.soft_drop_timer = 0.0;
                    self.try_move(0, -1);
                }
                KeyCode::Up | KeyCode::X => {
                    self.try_rotate(1);
                }
                KeyCode::Z => {
                    self.try_rotate(-1);
                }
                KeyCode::Space => {
                    self.hard_drop();
                }
                KeyCode::C | KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                    self.hold();
                }
                _ => {}
            }
        } else {
            match key {
                KeyCode::Left | KeyCode::Right => {
                    self.das_direction = 0;
                }
                _ => {}
            }
        }
    }

    /// Update DAS / ARR handling (call every frame).
    pub fn update_autoshift(&mut self, dt: f32) {
        if self.game_over || self.paused { return; }
        if self.das_direction == 0 { return; }

        self.das_timer += dt;
        if self.das_timer >= self.das {
            if self.arr <= 0.0 {
                // Instant repeat
                while self.try_move(self.das_direction, 0) {}
            } else {
                let repeats = ((self.das_timer - self.das) / self.arr) as i32;
                for _ in 0..repeats {
                    self.try_move(self.das_direction, 0);
                }
            }
        }
    }

    /// Ghost piece Y position (where the active piece would land).
    pub fn ghost_y(&self) -> Option<i32> {
        let piece = self.active_piece.clone()?;
        let mut ghost = piece.clone();
        while self.is_valid_position(&{ let mut t = ghost.clone(); t.y -= 1; t }) {
            ghost.y -= 1;
        }
        Some(ghost.y)
    }
}

/// Render the Tetris game board using egui.
pub fn render_tetris_ui(ctx: &egui::Context, game: &mut TetrisGame) {
    let cell_size = 28.0;
    let board_w = BOARD_WIDTH as f32 * cell_size;
    let board_h = VISIBLE_HEIGHT as f32 * cell_size;

    egui::Window::new("Tetris")
        .default_size([board_w + 160.0, board_h + 40.0])
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Main board
                let (response, painter) = ui.allocate_painter(egui::vec2(board_w, board_h), egui::Sense::click());
                let rect = response.rect;

                // Background
                painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));

                // Draw locked cells
                for y in 0..VISIBLE_HEIGHT {
                    for x in 0..BOARD_WIDTH {
                        if let Cell::Filled(kind) = game.board[y][x] {
                            let color = kind.color();
                            let cell_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.min.x + x as f32 * cell_size, rect.min.y + (VISIBLE_HEIGHT - 1 - y) as f32 * cell_size),
                                egui::vec2(cell_size - 1.0, cell_size - 1.0),
                            );
                            painter.rect_filled(cell_rect, 2.0, egui::Color32::from_rgb(color[0], color[1], color[2]));
                        }
                    }
                }

                // Draw ghost piece
                if let Some(gy) = game.ghost_y() {
                    if let Some(ref piece) = game.active_piece {
                        let ghost = Piece { y: gy, ..piece.clone() };
                        for (bx, by) in ghost.absolute_blocks() {
                            if by >= 0 && by < VISIBLE_HEIGHT as i32 && bx >= 0 && bx < BOARD_WIDTH as i32 {
                                let color = piece.kind.color();
                                let cell_rect = egui::Rect::from_min_size(
                                    egui::pos2(rect.min.x + bx as f32 * cell_size, rect.min.y + (VISIBLE_HEIGHT - 1 - by as usize) as f32 * cell_size),
                                    egui::vec2(cell_size - 1.0, cell_size - 1.0),
                                );
                                painter.rect_filled(cell_rect, 2.0, egui::Color32::from_rgba_premultiplied(color[0] / 3, color[1] / 3, color[2] / 3, 80));
                            }
                        }
                    }
                }

                // Draw active piece
                if let Some(ref piece) = game.active_piece {
                    let color = piece.kind.color();
                    for (bx, by) in piece.absolute_blocks() {
                        if by >= 0 && by < VISIBLE_HEIGHT as i32 && bx >= 0 && bx < BOARD_WIDTH as i32 {
                            let cell_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.min.x + bx as f32 * cell_size, rect.min.y + (VISIBLE_HEIGHT - 1 - by as usize) as f32 * cell_size),
                                egui::vec2(cell_size - 1.0, cell_size - 1.0),
                            );
                            painter.rect_filled(cell_rect, 2.0, egui::Color32::from_rgb(color[0], color[1], color[2]));
                            // Highlight
                            painter.rect_stroke(cell_rect.shrink(2.0), 2.0, egui::Stroke::new(1.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
                        }
                    }
                }

                // Side panel info
                ui.vertical(|ui| {
                    ui.heading(format!("Score: {}", game.score));
                    ui.label(format!("Level: {}", game.level));
                    ui.label(format!("Lines: {}", game.lines_cleared));
                    ui.add_space(12.0);

                    // Hold piece
                    ui.label("Hold:");
                    if let Some(hold) = game.hold_piece {
                        render_mini_piece(ui, hold, cell_size);
                    } else {
                        ui.add_space(cell_size * 2.0);
                    }

                    ui.add_space(12.0);

                    // Next queue
                    ui.label("Next:");
                    for kind in game.queue.iter().take(5) {
                        render_mini_piece(ui, *kind, cell_size * 0.7);
                    }

                    ui.add_space(20.0);

                    if game.game_over {
                        ui.heading("GAME OVER");
                        if ui.button("Restart").clicked() {
                            game.reset();
                        }
                    } else if game.paused {
                        ui.heading("PAUSED");
                        if ui.button("Resume").clicked() {
                            game.paused = false;
                        }
                    } else {
                        if ui.button("Pause").clicked() {
                            game.paused = true;
                        }
                    }

                    if ui.button("Restart").clicked() {
                        game.reset();
                    }
                });
            });
        });
}

fn render_mini_piece(ui: &mut egui::Ui, kind: TetrominoType, cell_size: f32) {
    let blocks = kind.blocks();
    let (min_x, max_x, min_y, max_y) = blocks.iter().fold(
        (i32::MAX, i32::MIN, i32::MAX, i32::MIN),
        |(mix, max, miy, may), (x, y)| (mix.min(*x), max.max(*x), miy.min(*y), may.max(*y)),
    );
    let w = (max_x - min_x + 1) as f32 * cell_size;
    let h = (max_y - min_y + 1) as f32 * cell_size;
    let (response, painter) = ui.allocate_painter(egui::vec2(w, h), egui::Sense::hover());
    let rect = response.rect;
    let color = kind.color();
    for (x, y) in blocks {
        let cell_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + (x - min_x) as f32 * cell_size, rect.min.y + (max_y - y) as f32 * cell_size),
            egui::vec2(cell_size - 1.0, cell_size - 1.0),
        );
        painter.rect_filled(cell_rect, 2.0, egui::Color32::from_rgb(color[0], color[1], color[2]));
    }
}
