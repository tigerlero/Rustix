//! Tests for terrain sculpting brush.

use crate::sculpt::{SculptBrush, BrushMode};
use crate::Heightmap;

#[test]
fn brush_raise_increases_height() {
    let mut hm = Heightmap::flat(10, 10, 0.0);
    let brush = SculptBrush::new().radius(2.0).strength(1.0).mode(BrushMode::Raise);
    brush.apply(&mut hm, 1.0, 5.0, 5.0);
    let center = hm.get(5, 5);
    assert!(center > 0.0, "center height should increase after raise");
}

#[test]
fn brush_lower_decreases_height() {
    let mut hm = Heightmap::flat(10, 10, 5.0);
    let brush = SculptBrush::new().radius(2.0).strength(1.0).mode(BrushMode::Lower);
    brush.apply(&mut hm, 1.0, 5.0, 5.0);
    let center = hm.get(5, 5);
    assert!(center < 5.0, "center height should decrease after lower");
}

#[test]
fn brush_flatten_brings_to_target() {
    let mut hm = Heightmap::flat(10, 10, 0.0);
    hm.heights[55] = 10.0; // center at (5,5)
    let brush = SculptBrush::new().radius(2.0).strength(3.0).mode(BrushMode::Flatten);
    brush.apply(&mut hm, 1.0, 5.0, 5.0);
    let center = hm.get(5, 5);
    assert!((center - 3.0).abs() < 0.5, "center should move toward target height 3.0, got {}", center);
}

#[test]
fn brush_smooth_reduces_variance() {
    let mut hm = Heightmap::flat(10, 10, 0.0);
    hm.heights[55] = 10.0; // center spike
    let before = hm.heights[55];
    let brush = SculptBrush::new().radius(2.0).strength(1.0).mode(BrushMode::Smooth);
    brush.apply(&mut hm, 1.0, 5.0, 5.0);
    let after = hm.heights[55];
    assert!(after < before, "smooth should reduce spike height");
}

#[test]
fn brush_respects_radius() {
    let mut hm = Heightmap::flat(20, 20, 0.0);
    let brush = SculptBrush::new().radius(1.0).strength(1.0).mode(BrushMode::Raise);
    brush.apply(&mut hm, 1.0, 10.0, 10.0);

    let center = hm.get(10, 10);
    let far = hm.get(0, 0);
    assert!(center > 0.0, "center should be affected");
    assert_eq!(far, 0.0, "far corner should be unaffected by small radius");
}

#[test]
fn brush_no_effect_outside_bounds() {
    let mut hm = Heightmap::flat(5, 5, 0.0);
    let brush = SculptBrush::new().radius(10.0).strength(1.0).mode(BrushMode::Raise);
    brush.apply(&mut hm, 1.0, -100.0, -100.0);
    // Should not panic and should leave heights mostly unchanged
    assert!(hm.heights.iter().all(|&h| h >= 0.0));
}
