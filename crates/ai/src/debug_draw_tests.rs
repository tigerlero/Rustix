//! Tests for AI debug draw primitives.

use rustix_core::math::Vec3;
use crate::debug_draw::{DebugLine, DebugPoint, DebugLabel, AiDebugDraw};
use crate::influence::InfluenceMap;

#[test]
fn debug_line_new() {
    let line = DebugLine::new(Vec3::ZERO, Vec3::X, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(line.start, Vec3::ZERO);
    assert_eq!(line.end, Vec3::X);
}

#[test]
fn debug_line_colors() {
    let red = DebugLine::red(Vec3::ZERO, Vec3::X);
    assert_eq!(red.color, [1.0, 0.0, 0.0, 1.0]);
    let green = DebugLine::green(Vec3::ZERO, Vec3::X);
    assert_eq!(green.color, [0.0, 1.0, 0.0, 1.0]);
    let blue = DebugLine::blue(Vec3::ZERO, Vec3::X);
    assert_eq!(blue.color, [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn debug_point_new() {
    let point = DebugPoint::new(Vec3::Y, 1.0, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(point.position, Vec3::Y);
    assert_eq!(point.radius, 1.0);
}

#[test]
fn debug_label_new() {
    let label = DebugLabel::new(Vec3::ZERO, "test", [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(label.text, "test");
}

#[test]
fn ai_debug_draw_new_is_empty() {
    let draw = AiDebugDraw::new();
    assert!(draw.lines.is_empty());
    assert!(draw.points.is_empty());
    assert!(draw.labels.is_empty());
}

#[test]
fn ai_debug_draw_clear() {
    let mut draw = AiDebugDraw::new();
    draw.line(Vec3::ZERO, Vec3::X, [1.0, 0.0, 0.0, 1.0]);
    draw.clear();
    assert!(draw.lines.is_empty());
}

#[test]
fn ai_debug_draw_line_adds() {
    let mut draw = AiDebugDraw::new();
    draw.line(Vec3::ZERO, Vec3::X, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(draw.lines.len(), 1);
}

#[test]
fn ai_debug_draw_point_adds() {
    let mut draw = AiDebugDraw::new();
    draw.point(Vec3::ZERO, 1.0, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(draw.points.len(), 1);
}

#[test]
fn ai_debug_draw_label_adds() {
    let mut draw = AiDebugDraw::new();
    draw.label(Vec3::ZERO, "hello", [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(draw.labels.len(), 1);
}

#[test]
fn ai_debug_draw_path() {
    let mut draw = AiDebugDraw::new();
    let waypoints = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
    draw.draw_path(&waypoints);
    assert_eq!(draw.lines.len(), 2);
}

#[test]
fn ai_debug_draw_vision_cone() {
    let mut draw = AiDebugDraw::new();
    draw.draw_vision_cone(Vec3::ZERO, Vec3::X, 90.0, 5.0);
    assert_eq!(draw.lines.len(), 3);
}

#[test]
fn ai_debug_draw_hearing_radius() {
    let mut draw = AiDebugDraw::new();
    draw.draw_hearing_radius(Vec3::ZERO, 3.0);
    assert_eq!(draw.lines.len(), 32);
}

#[test]
fn ai_debug_draw_influence_map() {
    let mut draw = AiDebugDraw::new();
    let mut map = InfluenceMap::new(3, 3, 1.0, [0.0, 0.0]);
    map.set(1, 1, 1.0);
    draw.draw_influence_map(&map, 0.0);
    assert!(draw.points.len() > 0);
}

#[test]
fn ai_debug_draw_fsm_state() {
    let mut draw = AiDebugDraw::new();
    draw.draw_fsm_state(Vec3::ZERO, "idle");
    assert_eq!(draw.labels.len(), 1);
}
