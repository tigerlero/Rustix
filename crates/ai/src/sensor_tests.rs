//! Tests for AI sensors (vision cone and hearing radius).

use rustix_core::math::Vec3;
use crate::sensor::{VisionCone, HearingRadius, AgentSensor};

#[test]
fn vision_sees_target_in_front() {
    let cone = VisionCone::new(Vec3::ZERO, Vec3::X, 90.0, 10.0);
    assert!(cone.can_see(Vec3::new(5.0, 0.0, 0.0)), "target directly in front should be visible");
}

#[test]
fn vision_ignores_target_behind() {
    let cone = VisionCone::new(Vec3::ZERO, Vec3::X, 90.0, 10.0);
    assert!(!cone.can_see(Vec3::new(-5.0, 0.0, 0.0)), "target behind should not be visible");
}

#[test]
fn vision_ignores_target_outside_fov() {
    let cone = VisionCone::new(Vec3::ZERO, Vec3::X, 45.0, 10.0);
    // 90 degrees to the side is outside 45-degree half-fov
    assert!(!cone.can_see(Vec3::new(0.0, 5.0, 0.0)), "target at 90 deg should be outside 45 deg FOV");
}

#[test]
fn vision_ignores_target_too_far() {
    let cone = VisionCone::new(Vec3::ZERO, Vec3::X, 90.0, 5.0);
    assert!(!cone.can_see(Vec3::new(10.0, 0.0, 0.0)), "target beyond max_distance should be invisible");
}

#[test]
fn vision_sees_target_at_origin() {
    let cone = VisionCone::new(Vec3::ZERO, Vec3::X, 90.0, 10.0);
    assert!(cone.can_see(Vec3::ZERO), "target at origin is always seen");
}

#[test]
fn vision_filters_candidates() {
    let cone = VisionCone::new(Vec3::ZERO, Vec3::X, 90.0, 10.0);
    let candidates = vec![
        Vec3::new(5.0, 0.0, 0.0),
        Vec3::new(-5.0, 0.0, 0.0),
        Vec3::new(20.0, 0.0, 0.0),
    ];
    let visible = cone.visible_candidates(&candidates);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], Vec3::new(5.0, 0.0, 0.0));
}

#[test]
fn vision_wide_fov_sees_sideways() {
    let cone = VisionCone::new(Vec3::ZERO, Vec3::X, 180.0, 10.0);
    assert!(cone.can_see(Vec3::new(0.0, 5.0, 0.0)), "180 deg FOV should see sideways target");
}

#[test]
fn hearing_hears_within_radius() {
    let hear = HearingRadius::new(Vec3::ZERO, 5.0);
    assert!(hear.can_hear(Vec3::new(3.0, 0.0, 0.0), 0.0), "sound within radius should be heard");
}

#[test]
fn hearing_ignores_outside_radius() {
    let hear = HearingRadius::new(Vec3::ZERO, 5.0);
    assert!(!hear.can_hear(Vec3::new(10.0, 0.0, 0.0), 0.0), "sound outside radius should not be heard");
}

#[test]
fn hearing_combines_radii() {
    let hear = HearingRadius::new(Vec3::ZERO, 5.0);
    // sound source radius extends reach
    assert!(hear.can_hear(Vec3::new(7.0, 0.0, 0.0), 3.0), "combined radii should reach");
}

#[test]
fn hearing_filters_candidates() {
    let hear = HearingRadius::new(Vec3::ZERO, 5.0);
    let candidates = vec![
        (Vec3::new(3.0, 0.0, 0.0), 0.0f32),
        (Vec3::new(10.0, 0.0, 0.0), 0.0f32),
    ];
    let audible = hear.audible_candidates(&candidates);
    assert_eq!(audible.len(), 1);
    assert_eq!(audible[0], Vec3::new(3.0, 0.0, 0.0));
}

#[test]
fn agent_sensor_default_empty() {
    let sensor = AgentSensor::new();
    assert!(sensor.vision.is_none());
    assert!(sensor.hearing.is_none());
}

#[test]
fn agent_sensor_builder() {
    let sensor = AgentSensor::new()
        .with_vision(Vec3::ZERO, Vec3::X, 90.0, 10.0)
        .with_hearing(Vec3::ZERO, 5.0);
    assert!(sensor.vision.is_some());
    assert!(sensor.hearing.is_some());
}

#[test]
fn agent_sensor_set_position_updates_both() {
    let mut sensor = AgentSensor::new()
        .with_vision(Vec3::ZERO, Vec3::X, 90.0, 10.0)
        .with_hearing(Vec3::ZERO, 5.0);
    sensor.set_position(Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(sensor.vision.unwrap().origin, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(sensor.hearing.unwrap().origin, Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn agent_sensor_set_forward_updates_vision() {
    let mut sensor = AgentSensor::new()
        .with_vision(Vec3::ZERO, Vec3::X, 90.0, 10.0);
    sensor.set_forward(Vec3::Y);
    assert_eq!(sensor.vision.unwrap().forward, Vec3::Y);
}

#[test]
fn agent_sensor_default_impl() {
    let sensor: AgentSensor = Default::default();
    assert!(sensor.vision.is_none());
    assert!(sensor.hearing.is_none());
}
