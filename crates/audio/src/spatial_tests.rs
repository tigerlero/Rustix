#[cfg(test)]
use super::*;

#[test]
fn test_attenuation_at_min_distance() {
    let att = calculate_attenuation(1.0, 1.0, 100.0, 1.0);
    assert!((att - 1.0).abs() < 0.001);
}

#[test]
fn test_attenuation_at_max_distance() {
    let att = calculate_attenuation(100.0, 1.0, 100.0, 1.0);
    assert!((att - 0.0).abs() < 0.001);
}

#[test]
fn test_attenuation_between_distances() {
    let att = calculate_attenuation(10.0, 1.0, 100.0, 1.0);
    assert!(att > 0.0 && att < 1.0);
}

#[test]
fn test_hrtf_panning_front() {
    let (left, right) = hrtf_panning(0.0);
    assert!((left - right).abs() < 0.1, "Center should be balanced");
}

#[test]
fn test_hrtf_panning_right() {
    let (left, right) = hrtf_panning(std::f32::consts::FRAC_PI_2);
    assert!(right > left, "Right of listener should have more right gain");
}

#[test]
fn test_hrtf_panning_left() {
    let (left, right) = hrtf_panning(-std::f32::consts::FRAC_PI_2);
    assert!(left > right, "Left of listener should have more left gain");
}

#[test]
fn test_horiz_azimuth_front() {
    let listener_pos = Vec3::ZERO;
    let listener_forward = Vec3::Z;
    let source_pos = Vec3::new(0.0, 0.0, 10.0);
    let angle = calculate_horiz_azimuth(listener_pos, listener_forward, source_pos);
    assert!(angle.abs() < 0.01, "Source directly in front should be angle 0");
}

#[test]
fn test_horiz_azimuth_right() {
    let listener_pos = Vec3::ZERO;
    let listener_forward = Vec3::Z;
    let source_pos = Vec3::new(10.0, 0.0, 0.0);
    let angle = calculate_horiz_azimuth(listener_pos, listener_forward, source_pos);
    let expected = std::f32::consts::FRAC_PI_2;
    assert!((angle - expected).abs() < 0.1, "Source to the right should be +90 degrees");
}

#[test]
fn test_horiz_azimuth_left() {
    let listener_pos = Vec3::ZERO;
    let listener_forward = Vec3::Z;
    let source_pos = Vec3::new(-10.0, 0.0, 0.0);
    let angle = calculate_horiz_azimuth(listener_pos, listener_forward, source_pos);
    let expected = -std::f32::consts::FRAC_PI_2;
    assert!((angle - expected).abs() < 0.1, "Source to the left should be -90 degrees");
}
