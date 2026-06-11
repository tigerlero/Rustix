use rustix_core::math::Vec3;
use serde::{Deserialize, Serialize};

/// References a mesh in the renderer's mesh registry by index.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MeshRenderer {
    pub mesh_idx: usize,
}

/// A material with base color texture and PBR properties.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Material {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub normal_scale: f32,
    pub occlusion_strength: f32,
}

impl Material {
    /// Build a renderer `Material` from an asset definition.
    pub fn from_asset(asset: &rustix_asset::material::MaterialAsset) -> Self {
        Self {
            base_color: asset.base_color,
            metallic: asset.metallic,
            roughness: asset.roughness,
            normal_scale: asset.normal_scale,
            occlusion_strength: asset.occlusion_strength,
        }
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
        }
    }
}

/// Texture index for material.
pub type TextureIndex = usize;

/// A material component with optional texture indices.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MaterialComponent {
    pub albedo: Option<TextureIndex>,
    pub normal: Option<TextureIndex>,
    pub metallic_roughness: Option<TextureIndex>,
    pub material: Material,
}

impl MaterialComponent {
    /// Build a `MaterialComponent` from an asset, resolving texture indices externally.
    pub fn from_asset(
        asset: &rustix_asset::material::MaterialAsset,
        albedo: Option<TextureIndex>,
        normal: Option<TextureIndex>,
        metallic_roughness: Option<TextureIndex>,
    ) -> Self {
        Self {
            albedo,
            normal,
            metallic_roughness,
            material: Material::from_asset(asset),
        }
    }
}

/// Marks the entity as the active camera.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub fov_degrees: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { fov_degrees: 60.0, near: 0.1, far: 100.0 }
    }
}

/// Directional light component.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DirectionalLight {
    pub color: Vec3,
    pub intensity: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self { color: Vec3::splat(1.0), intensity: 1.0 }
    }
}

/// Point light component.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointLight {
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self { color: Vec3::splat(1.0), intensity: 1.0, radius: 5.0 }
    }
}

/// Spot light component.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpotLight {
    pub color: Vec3,
    pub intensity: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
    pub radius: f32,
}

impl Default for SpotLight {
    fn default() -> Self {
        Self {
            color: Vec3::splat(1.0),
            intensity: 1.0,
            inner_angle: 0.5,
            outer_angle: 0.78,
            radius: 10.0
        }
    }
}

/// Parent entity for hierarchy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Parent {
    pub entity: hecs::Entity,
}

/// Children entities for hierarchy.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Children {
    pub entities: Vec<hecs::Entity>,
}

/// Visibility flag.
#[derive(Debug, Clone, Copy, Default)]
pub struct Visible {
    pub enabled: bool,
}

impl Visible {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

/// Shadow caster flag.
#[derive(Debug, Clone, Copy, Default)]
pub struct CastShadows {
    pub enabled: bool,
}

impl CastShadows {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

/// 2D sprite component with variable size and RGB coloring.
/// Supports per-pixel color data for sprite creation and editing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sprite {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels  
    pub height: u32,
    /// Pixel data as RGBA values (width * height * 4 bytes)
    pub pixels: Vec<u8>,
}

impl Default for Sprite {
    fn default() -> Self {
        Self::empty(64, 64)
    }
}

/// Component to render a sprite (2D quad with texture).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SpriteRenderer {
    pub width: f32,
    pub height: f32,
    pub color: [f32; 4],
    pub outline_color: [f32; 4],
    pub outline_thickness: f32,
}

impl Default for SpriteRenderer {
    fn default() -> Self {
        Self { width: 64.0, height: 64.0, color: [1.0, 1.0, 1.0, 1.0], outline_color: [0.0, 0.0, 0.0, 1.0], outline_thickness: 0.0 }
    }
}

impl Sprite {
    /// Create a new sprite with given dimensions and solid color.
    pub fn new(width: u32, height: u32, color: [u8; 4]) -> Self {
        let len = (width * height * 4) as usize;
        let mut pixels = Vec::with_capacity(len);
        for _ in 0..width * height {
            pixels.extend_from_slice(&color);
        }
        Self { width, height, pixels }
    }

    /// Create an empty sprite (transparent).
    pub fn empty(width: u32, height: u32) -> Self {
        let pixels = vec![0u8; (width * height * 4) as usize];
        Self { width, height, pixels }
    }

    /// Create a checkerboard pattern for testing.
    pub fn checkerboard(width: u32, height: u32, square_size: u32) -> Self {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let square_x = (x / square_size) % 2;
                let square_y = (y / square_size) % 2;
                let idx = ((y * width + x) * 4) as usize;
                let color = if (square_x + square_y) % 2 == 0 {
                    [255u8, 255, 255, 255]
                } else {
                    [64u8, 64, 64, 255]
                };
                pixels[idx..idx+4].copy_from_slice(&color);
            }
        }
        Self { width, height, pixels }
    }

    /// Set a pixel at (x, y) to the given color.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            self.pixels[idx..idx+4].copy_from_slice(&color);
        }
    }

    /// Get a pixel at (x, y).
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            Some([
                self.pixels[idx],
                self.pixels[idx + 1],
                self.pixels[idx + 2],
                self.pixels[idx + 3],
            ])
        } else {
            None
        }
    }

    /// Clear the sprite to transparent black.
    pub fn clear(&mut self) {
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk.fill(0);
        }
    }

    /// Fill with a solid color.
    pub fn fill(&mut self, color: [u8; 4]) {
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&color);
        }
    }

    /// Fill a rectangle with a color.
    pub fn fill_rect(&mut self, x: u32, y: u32, rect_w: u32, rect_h: u32, color: [u8; 4]) {
        let x_end = (x + rect_w).min(self.width);
        let y_end = (y + rect_h).min(self.height);
        for py in y..y_end {
            for px in x..x_end {
                self.set_pixel(px, py, color);
            }
        }
    }

    /// Draw a line using Bresenham's algorithm.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 4]) {
        let mut x0 = x0;
        let mut y0 = y0;
        let x1 = x1;
        let y1 = y1;
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        loop {
            if x0 >= 0 && y0 >= 0 {
                self.set_pixel(x0 as u32, y0 as u32, color);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 > -(dy as i32) {
                err -= dy as i32;
                x0 += sx;
            }
            if e2 < dx as i32 {
                err += dx as i32;
                y0 += sy;
            }
        }
    }

    /// Fill a circle.
    pub fn fill_circle(&mut self, cx: i32, cy: i32, radius: i32, color: [u8; 4]) {
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= radius * radius {
                    if x >= 0 && y >= 0 {
                        self.set_pixel(x as u32, y as u32, color);
                    }
                }
            }
        }
    }

    /// Create a 9-patch sprite (3x3 grid) from corners and edges.
    pub fn nine_patch(
        corner: [u8; 4],
        edge: [u8; 4],
        center: [u8; 4],
        patch_size: u32,
    ) -> Self {
        let width = patch_size * 3;
        let height = patch_size * 3;
        let mut sprite = Sprite::new(width, height, center);
        
        // Corners
        sprite.fill_rect(0, 0, patch_size, patch_size, corner);
        sprite.fill_rect(patch_size * 2, 0, patch_size, patch_size, corner);
        sprite.fill_rect(0, patch_size * 2, patch_size, patch_size, corner);
        sprite.fill_rect(patch_size * 2, patch_size * 2, patch_size, patch_size, corner);
        
        // Edges
        sprite.fill_rect(patch_size, 0, patch_size, patch_size, edge);
        sprite.fill_rect(0, patch_size, patch_size, patch_size, edge);
        sprite.fill_rect(patch_size * 2, patch_size, patch_size, patch_size, edge);
        sprite.fill_rect(patch_size, patch_size * 2, patch_size, patch_size, edge);
        
        sprite
    }

    /// Draw an outlined rectangle with separate line and fill colors.
    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, fill_color: [u8; 4], line_color: [u8; 4], line_thickness: u32) {
        // Fill the interior
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let x1 = (x + w as i32).max(0) as u32;
        let y1 = (y + h as i32).max(0) as u32;
        
        let inner_x0 = x0 + line_thickness;
        let inner_y0 = y0 + line_thickness;
        let inner_x1 = x1.saturating_sub(line_thickness);
        let inner_y1 = y1.saturating_sub(line_thickness);
        
        if inner_x1 > inner_x0 && inner_y1 > inner_y0 {
            self.fill_rect(inner_x0, inner_y0, inner_x1 - inner_x0, inner_y1 - inner_y0, fill_color);
        }
        
        // Draw border using fill_rect for each edge
        if line_thickness > 0 {
            // Top
            self.fill_rect(x0, y0, x1 - x0, line_thickness, line_color);
            // Bottom
            if y1 > line_thickness {
                self.fill_rect(x0, y1 - line_thickness, x1 - x0, line_thickness, line_color);
            }
            // Left
            self.fill_rect(x0, y0, line_thickness, y1 - y0, line_color);
            // Right
            if x1 > line_thickness {
                self.fill_rect(x1 - line_thickness, y0, line_thickness, y1 - y0, line_color);
            }
        }
    }

    /// Draw an outlined circle with separate fill and line colors.
    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32, fill_color: [u8; 4], line_color: [u8; 4], line_thickness: u32) {
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let dx = x - cx;
                let dy = y - cy;
                let dist_sq = dx * dx + dy * dy;
                let outer_r = radius;
                let inner_r = (radius as i32 - line_thickness as i32).max(0);
                
                if x >= 0 && y >= 0 {
                    if dist_sq <= outer_r as i32 * outer_r as i32 {
                        // Inside the outer circle
                        if dist_sq >= inner_r as i32 * inner_r as i32 {
                            // In the border region
                            self.set_pixel(x as u32, y as u32, line_color);
                        } else {
                            // In the fill region
                            self.set_pixel(x as u32, y as u32, fill_color);
                        }
                    }
                }
            }
        }
    }

    /// Draw an outlined rectangle with line color (uses current brush color as fill).
    pub fn draw_rect_outline(&mut self, x: i32, y: i32, w: i32, h: i32, line_color: [u8; 4], line_thickness: u32) {
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let x1 = (x + w as i32).min(self.width as i32).max(0) as u32;
        let y1 = (y + h as i32).min(self.height as i32).max(0) as u32;
        
        // Top
        self.fill_rect(x0, y0, x1.saturating_sub(x0), line_thickness, line_color);
        // Bottom
        if y1 > line_thickness {
            self.fill_rect(x0, y1.saturating_sub(line_thickness), x1.saturating_sub(x0), line_thickness, line_color);
        }
        // Left
        self.fill_rect(x0, y0, line_thickness, y1.saturating_sub(y0), line_color);
        // Right
        if x1 > line_thickness {
            self.fill_rect(x1.saturating_sub(line_thickness), y0, line_thickness, y1.saturating_sub(y0), line_color);
        }
    }
}

/// Post-process settings component.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PostProcessSettings {
    pub grain_intensity: f32,
    pub chromatic_aberration: f32,
    pub vignette_intensity: f32,
    pub vignette_smoothness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub gamma: f32,
    pub tint_shadows: [f32; 4],
    pub tint_midtones: [f32; 4],
    pub tint_highlights: [f32; 4],
}

impl Default for PostProcessSettings {
    fn default() -> Self {
        Self {
            grain_intensity: 0.03,
            chromatic_aberration: 0.005,
            vignette_intensity: 1.5,
            vignette_smoothness: 0.8,
            contrast: 1.0,
            saturation: 1.0,
            gamma: 2.2,
            tint_shadows: [1.0, 1.0, 1.0, 1.0],
            tint_midtones: [1.0, 1.0, 1.0, 1.0],
            tint_highlights: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// Particle emitter component.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParticleEmitter {
    pub spawn_rate: f32,
    pub max_particles: u32,
    pub velocity: Vec3,
    pub velocity_spread: f32,
    pub lifetime: f32,
    pub lifetime_spread: f32,
    pub start_size: f32,
    pub end_size: f32,
    pub start_color: [f32; 4],
    pub end_color: [f32; 4],
    pub gravity: Vec3,
    pub enabled: bool,
}

impl Default for ParticleEmitter {
    fn default() -> Self {
        Self {
            spawn_rate: 100.0,
            max_particles: 1000,
            velocity: Vec3::Y,
            velocity_spread: 0.5,
            lifetime: 2.0,
            lifetime_spread: 0.5,
            start_size: 0.1,
            end_size: 0.02,
            start_color: [1.0, 1.0, 1.0, 1.0],
            end_color: [1.0, 1.0, 1.0, 0.0],
            gravity: Vec3::new(0.0, -9.81, 0.0),
            enabled: true,
        }
    }
}