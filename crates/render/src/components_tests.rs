//! Tests for render components: materials, lights, camera, sprite drawing.

use rustix_core::math::Vec3;
use crate::components::*;

#[test]
fn material_default() {
    let m = Material::default();
    assert_eq!(m.base_color, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(m.metallic, 0.0);
    assert_eq!(m.roughness, 0.5);
    assert_eq!(m.normal_scale, 1.0);
    assert_eq!(m.occlusion_strength, 1.0);
}

#[test]
fn camera_default() {
    let c = Camera::default();
    assert_eq!(c.fov_degrees, 60.0);
    assert_eq!(c.near, 0.1);
    assert_eq!(c.far, 100.0);
}

#[test]
fn directional_light_default() {
    let l = DirectionalLight::default();
    assert_eq!(l.color, Vec3::splat(1.0));
    assert_eq!(l.intensity, 1.0);
}

#[test]
fn point_light_default() {
    let l = PointLight::default();
    assert_eq!(l.color, Vec3::splat(1.0));
    assert_eq!(l.intensity, 1.0);
    assert_eq!(l.radius, 5.0);
}

#[test]
fn spot_light_default() {
    let l = SpotLight::default();
    assert_eq!(l.color, Vec3::splat(1.0));
    assert_eq!(l.intensity, 1.0);
    assert_eq!(l.inner_angle, 0.5);
    assert_eq!(l.outer_angle, 0.78);
    assert_eq!(l.radius, 10.0);
}

#[test]
fn visible_new() {
    let v = Visible::new(true);
    assert!(v.enabled);
    let v = Visible::new(false);
    assert!(!v.enabled);
}

#[test]
fn cast_shadows_new() {
    let c = CastShadows::new(true);
    assert!(c.enabled);
    let c = CastShadows::new(false);
    assert!(!c.enabled);
}

#[test]
fn sprite_renderer_default() {
    let s = SpriteRenderer::default();
    assert_eq!(s.width, 64.0);
    assert_eq!(s.height, 64.0);
    assert_eq!(s.color, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(s.outline_color, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(s.outline_thickness, 0.0);
}

#[test]
fn sprite_new_dimensions() {
    let s = Sprite::new(4, 4, [255, 0, 0, 255]);
    assert_eq!(s.width, 4);
    assert_eq!(s.height, 4);
    assert_eq!(s.pixels.len(), 64);
    assert_eq!(s.get_pixel(0, 0), Some([255, 0, 0, 255]));
}

#[test]
fn sprite_empty_is_transparent() {
    let s = Sprite::empty(4, 4);
    assert_eq!(s.get_pixel(0, 0), Some([0, 0, 0, 0]));
}

#[test]
fn sprite_set_and_get_pixel() {
    let mut s = Sprite::empty(4, 4);
    s.set_pixel(1, 1, [255, 0, 0, 255]);
    assert_eq!(s.get_pixel(1, 1), Some([255, 0, 0, 255]));
    assert_eq!(s.get_pixel(5, 5), None);
}

#[test]
fn sprite_clear() {
    let mut s = Sprite::new(4, 4, [255, 0, 0, 255]);
    s.clear();
    assert_eq!(s.get_pixel(0, 0), Some([0, 0, 0, 0]));
}

#[test]
fn sprite_fill() {
    let mut s = Sprite::empty(4, 4);
    s.fill([0, 255, 0, 255]);
    assert_eq!(s.get_pixel(2, 2), Some([0, 255, 0, 255]));
}

#[test]
fn sprite_fill_rect() {
    let mut s = Sprite::empty(8, 8);
    s.fill_rect(2, 2, 4, 4, [255, 0, 0, 255]);
    assert_eq!(s.get_pixel(3, 3), Some([255, 0, 0, 255]));
    assert_eq!(s.get_pixel(0, 0), Some([0, 0, 0, 0]));
}

#[test]
fn sprite_checkerboard() {
    let s = Sprite::checkerboard(8, 8, 4);
    let c1 = s.get_pixel(0, 0).unwrap();
    let c2 = s.get_pixel(4, 0).unwrap();
    assert_ne!(c1, c2);
}

#[test]
fn sprite_draw_line() {
    let mut s = Sprite::empty(8, 8);
    s.draw_line(0, 0, 7, 7, [255, 0, 0, 255]);
    assert_eq!(s.get_pixel(0, 0), Some([255, 0, 0, 255]));
    assert_eq!(s.get_pixel(7, 7), Some([255, 0, 0, 255]));
}

#[test]
fn sprite_fill_circle() {
    let mut s = Sprite::empty(16, 16);
    s.fill_circle(8, 8, 4, [255, 0, 0, 255]);
    assert_eq!(s.get_pixel(8, 8), Some([255, 0, 0, 255]));
    assert_eq!(s.get_pixel(0, 0), Some([0, 0, 0, 0]));
}

#[test]
fn sprite_nine_patch_corners() {
    let s = Sprite::nine_patch([255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], 2);
    assert_eq!(s.width, 6);
    assert_eq!(s.height, 6);
    assert_eq!(s.get_pixel(0, 0), Some([255, 0, 0, 255]));       // corner
    assert_eq!(s.get_pixel(2, 2), Some([0, 0, 255, 255]));      // center
    assert_eq!(s.get_pixel(2, 0), Some([0, 255, 0, 255]));      // top edge
}

#[test]
fn sprite_draw_rect() {
    let mut s = Sprite::empty(16, 16);
    s.draw_rect(2, 2, 12, 12, [0, 255, 0, 255], [255, 0, 0, 255], 2);
    // fill in the interior
    assert_eq!(s.get_pixel(8, 8), Some([0, 255, 0, 255]));
    // border
    assert_eq!(s.get_pixel(2, 2), Some([255, 0, 0, 255]));
}

#[test]
fn sprite_draw_circle() {
    let mut s = Sprite::empty(16, 16);
    s.draw_circle(8, 8, 6, [0, 255, 0, 255], [255, 0, 0, 255], 2);
    assert_eq!(s.get_pixel(8, 8), Some([0, 255, 0, 255]));
}

#[test]
fn sprite_draw_rect_outline() {
    let mut s = Sprite::empty(16, 16);
    s.draw_rect_outline(2, 2, 12, 12, [255, 0, 0, 255], 2);
    assert_eq!(s.get_pixel(2, 2), Some([255, 0, 0, 255]));
    // interior should not be filled
    assert_eq!(s.get_pixel(8, 8), Some([0, 0, 0, 0]));
}
