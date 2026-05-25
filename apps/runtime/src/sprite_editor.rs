//! Sprite editor for creating and editing 2D sprites with variable sizes and RGB coloring.

use egui::{Color32, Context, Rect};
use rustix_render::Sprite;

/// Drawing mode for the sprite editor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawMode {
    Fill,
    Outline,
    Rect,
    Circle,
}

/// State for the sprite editor.
pub struct SpriteEditor {
    sprite: Sprite,
    fill_color: [u8; 4],
    outline_color: [u8; 4],
    brush_size: u32,
    visible: bool,
    last_mouse_pos: Option<(u32, u32)>,
    draw_mode: DrawMode,
    outline_thickness: u32,
    dirty: bool,
}

impl Default for SpriteEditor {
    fn default() -> Self {
        Self {
            sprite: Sprite::empty(64, 64),
            fill_color: [255, 255, 255, 255],
            outline_color: [0, 0, 0, 255],
            brush_size: 1,
            visible: false,
            last_mouse_pos: None,
            draw_mode: DrawMode::Fill,
            outline_thickness: 1,
            dirty: false,
        }
    }
}

impl SpriteEditor {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            sprite: Sprite::empty(width, height),
            ..Default::default()
        }
    }

    /// Show the sprite editor UI.
    pub fn show(&mut self, ctx: &Context) {
        let mut open = self.visible;
        egui::Window::new("Sprite Editor")
            .default_width(400.0)
            .open(&mut open)
            .show(ctx, |ui| {
                self.visible = true;
                ui.label(format!("Sprite: {}x{}", self.sprite.width, self.sprite.height));
                
                ui.separator();
                ui.label("Draw Mode:");
                ui.horizontal(|ui| {
                    use DrawMode::*;
                    if ui.selectable_label(self.draw_mode == Fill, "Fill").clicked() { self.draw_mode = Fill; }
                    if ui.selectable_label(self.draw_mode == Outline, "Outline").clicked() { self.draw_mode = Outline; }
                    if ui.selectable_label(self.draw_mode == Rect, "Rect").clicked() { self.draw_mode = Rect; }
                    if ui.selectable_label(self.draw_mode == Circle, "Circle").clicked() { self.draw_mode = Circle; }
                });
                
                ui.separator();
                ui.label("Fill Color:");
                let mut fill32 = Color32::from_rgba_unmultiplied(
                    self.fill_color[0], self.fill_color[1], self.fill_color[2], self.fill_color[3]
                );
                if ui.color_edit_button_srgba(&mut fill32).changed() {
                    self.fill_color = [fill32.r(), fill32.g(), fill32.b(), fill32.a()];
                }
                
                ui.separator();
                ui.label("Outline Color:");
                let mut outline32 = Color32::from_rgba_unmultiplied(
                    self.outline_color[0], self.outline_color[1], self.outline_color[2], self.outline_color[3]
                );
                if ui.color_edit_button_srgba(&mut outline32).changed() {
                    self.outline_color = [outline32.r(), outline32.g(), outline32.b(), outline32.a()];
                }
                
                ui.separator();
                ui.label("Brush/Outline Size:");
                ui.add(egui::Slider::new(&mut self.brush_size, 1..=10u32));
                ui.add(egui::Slider::new(&mut self.outline_thickness, 0..=5u32));
                
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Clear").clicked() {
                        self.sprite.clear();
                    }
                    if ui.button("Fill").clicked() {
                        self.sprite.fill(self.fill_color);
                    }
                    if ui.button("Checkerboard").clicked() {
                        self.sprite = Sprite::checkerboard(self.sprite.width, self.sprite.height, 8);
                    }
                });
                
                ui.separator();
                ui.label("Pixel Editor:");
                
                let (response, painter) = ui.allocate_painter(
                    egui::vec2(256.0, 256.0),
                    egui::Sense::drag(),
                );
                
                let sprite_size = egui::vec2(
                    self.sprite.width as f32,
                    self.sprite.height as f32
                );
                let scale = 256.0 / sprite_size.x.max(sprite_size.y).max(1.0);
                
                for y in 0..self.sprite.height {
                    for x in 0..self.sprite.width {
                        if let Some(pixel) = self.sprite.get_pixel(x, y) {
                            if pixel[3] > 0 {
                                let color = Color32::from_rgba_unmultiplied(
                                    pixel[0], pixel[1], pixel[2], pixel[3]
                                );
                                let rect = Rect::from_min_max(
                                    response.rect.min + egui::vec2(x as f32, y as f32) * scale,
                                    response.rect.min + egui::vec2(x as f32 + 1.0, y as f32 + 1.0) * scale,
                                );
                                painter.rect_filled(rect, 0.0, color);
                            }
                        }
                    }
                }
                
                if response.hovered() && ui.input(|i| i.pointer.primary_down()) {
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.interact_pos()) {
                        let local_pos = (mouse_pos - response.rect.min) / scale;
                        let px = local_pos.x as u32;
                        let py = local_pos.y as u32;
                        if px < self.sprite.width && py < self.sprite.height {
                            match self.draw_mode {
                                DrawMode::Fill => {
                                    for dy in 0..self.brush_size {
                                        for dx in 0..self.brush_size {
                                            let x = (px as i32 + dx as i32 - self.brush_size as i32 / 2).max(0) as u32;
                                            let y = (py as i32 + dy as i32 - self.brush_size as i32 / 2).max(0) as u32;
                                            if x < self.sprite.width && y < self.sprite.height {
                                                self.sprite.set_pixel(x, y, self.fill_color);
                                            }
                                        }
                                    }
                                }
                                DrawMode::Outline => {
                                    self.sprite.set_pixel(px, py, self.outline_color);
                                }
                                DrawMode::Rect => {
                                    let hw = self.brush_size * 4;
                                    let hh = self.brush_size * 4;
                                    self.sprite.draw_rect(
                                        px as i32 - hw as i32 / 2,
                                        py as i32 - hh as i32 / 2,
                                        hw as i32,
                                        hh as i32,
                                        self.fill_color,
                                        self.outline_color,
                                        self.outline_thickness,
                                    );
                                }
                                DrawMode::Circle => {
                                    let radius = self.brush_size * 4;
                                    self.sprite.draw_circle(
                                        px as i32,
                                        py as i32,
                                        radius as i32,
                                        self.fill_color,
                                        self.outline_color,
                                        self.outline_thickness,
                                    );
                                }
                            }
                        }
                    }
                }
            });
        
        if !open {
            self.visible = false;
        }
    }
    
    /// Get the current sprite.
    pub fn sprite(&self) -> &Sprite {
        &self.sprite
    }
    
    /// Get mutable access to the sprite.
    pub fn sprite_mut(&mut self) -> &mut Sprite {
        &mut self.sprite
    }
    
    /// Get the sprite pixels as a byte slice.
    pub fn pixels(&self) -> &[u8] {
        &self.sprite.pixels
    }
    
    /// Get the sprite dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.sprite.width, self.sprite.height)
    }
    
    /// Set visibility.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    
    /// Check if visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    
    /// Check if the sprite has been modified since last check.
    pub fn mark_clean(&mut self) {
        // Could track a dirty flag here in the future
        let _ = &self.last_mouse_pos;
    }
}