//! Tests for time-of-day cycle calculations.

use crate::time_of_day::TimeOfDay;

#[test]
fn new_time_wraps_to_24() {
    let t = TimeOfDay::new(25.0);
    assert!((t.hours - 1.0).abs() < 1e-4);
}

#[test]
fn new_time_negative_wraps() {
    let t = TimeOfDay::new(-1.0);
    assert!((t.hours - 23.0).abs() < 1e-4);
}

#[test]
fn advance_increases_hours() {
    let mut t = TimeOfDay::new(10.0);
    t.advance(2.5);
    assert!((t.hours - 12.5).abs() < 1e-4);
}

#[test]
fn advance_wraps_past_24() {
    let mut t = TimeOfDay::new(23.0);
    t.advance(2.0);
    assert!((t.hours - 1.0).abs() < 1e-4);
}

#[test]
fn sun_direction_noon() {
    let t = TimeOfDay::new(12.0);
    let dir = t.sun_direction();
    assert!(dir.y > 0.0, "sun should be above horizon at noon");
}

#[test]
fn sun_direction_dawn() {
    let t = TimeOfDay::new(6.0);
    let dir = t.sun_direction();
    assert!(dir.y.abs() < 1e-3, "sun should be near horizon at dawn");
}

#[test]
fn sun_direction_night() {
    let t = TimeOfDay::new(0.0);
    let dir = t.sun_direction();
    assert!(dir.y < 0.0, "sun should be below horizon at midnight");
}

#[test]
fn moon_direction_opposes_sun() {
    let t = TimeOfDay::new(12.0);
    let sun = t.sun_direction();
    let moon = t.moon_direction();
    // moon should be roughly opposite sun
    assert!(sun.dot(moon) < -0.9);
}

#[test]
fn ambient_color_night_is_dark() {
    let t = TimeOfDay::new(0.0);
    let c = t.ambient_color();
    assert!(c[0] < 0.1, "night ambient R should be dark");
    assert!(c[1] < 0.1, "night ambient G should be dark");
}

#[test]
fn ambient_color_day_is_bright() {
    let t = TimeOfDay::new(12.0);
    let c = t.ambient_color();
    assert!(c[0] > 0.5, "day ambient R should be bright");
    assert!(c[1] > 0.5, "day ambient G should be bright");
}

#[test]
fn sun_color_noon_is_whiteish() {
    let t = TimeOfDay::new(12.0);
    let c = t.sun_color();
    assert!(c[0] > 0.9, "noon sun should be bright white");
    assert!(c[1] > 0.9);
}

#[test]
fn sun_color_night_is_black() {
    let t = TimeOfDay::new(0.0);
    let c = t.sun_color();
    assert_eq!(c[0], 0.0);
    assert_eq!(c[1], 0.0);
    assert_eq!(c[2], 0.0);
}
