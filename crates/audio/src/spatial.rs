use rustix_core::math::Vec3;

/// Audio listener component (usually on the main camera).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AudioListener {
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
}

impl Default for AudioListener {
    fn default() -> Self {
        Self { position: Vec3::ZERO, forward: Vec3::NEG_Z, up: Vec3::Y }
    }
}

/// Component for spatial audio positioning.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AudioSource {
    pub position: Vec3,
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff: f32,
}

/// Reference distance for rolloff (typically 1.0)
pub const REFERENCE_DISTANCE: f32 = 1.0;

/// Speed of sound in meters per second (for ITD calculation)
pub const SPEED_OF_SOUND: f32 = 343.0;

/// Average human head radius in meters (for ITD calculation)
pub const HEAD_RADIUS: f32 = 0.0875;

/// Maximum ITD in seconds (approximately 0.6ms for humans)
pub const MAX_ITD: f32 = 0.0006;

/// Calculate distance-based attenuation factor.
///
/// Uses inverse distance model with linear taper:
/// - At min_distance: full volume (1.0)
/// - Between min and max: linear falloff based on rolloff factor
/// - Beyond max_distance: silent (0.0)
pub fn calculate_attenuation(distance: f32, min_distance: f32, max_distance: f32, rolloff: f32) -> f32 {
    if distance <= min_distance {
        return 1.0;
    }

    if distance >= max_distance {
        return 0.0;
    }

    let ratio = distance / min_distance;
    let attenuation = 1.0 / (1.0 + rolloff * (ratio - 1.0));

    let taper = 1.0 - ((distance - min_distance) / (max_distance - min_distance));
    attenuation * taper.clamp(0.0, 1.0)
}

/// Calculate HRTF panning for a mono source position.
///
/// Returns (left_gain, right_gain) for stereo output.
/// Uses simplified HRTF model with:
/// - Interaural Level Difference (ILD): amplitude difference based on angle
/// - Interaural Time Difference (ITD): phase shift approximation via delay
pub fn hrtf_panning(source_angle: f32) -> (f32, f32) {
    let half_pi = std::f32::consts::FRAC_PI_2;
    let angle_clamped = source_angle.clamp(-half_pi, half_pi);

    let left = (1.0 - angle_clamped / half_pi).clamp(0.0, 1.0);
    let right = (1.0 + angle_clamped / half_pi).clamp(0.0, 1.0);

    let ild_factor = 0.707;
    let left = if left > 0.5 { left * (1.0 - ild_factor * (1.0 - left)) } else { left };
    let right = if right > 0.5 { right * (1.0 - ild_factor * (1.0 - right)) } else { right };

    let sum = left + right;
    if sum > 0.0 {
        (left / sum, right / sum)
    } else {
        (0.0, 0.0)
    }
}

/// Calculate the angle from listener to source in the horizontal plane.
///
/// Returns angle in radians (-π to π), where 0 is directly in front.
pub fn calculate_horiz_azimuth(listener_pos: Vec3, listener_forward: Vec3, source_pos: Vec3) -> f32 {
    let to_source = (source_pos - listener_pos).normalize();

    let forward_h = Vec3::new(listener_forward.x, 0.0, listener_forward.z).normalize();
    let to_h = Vec3::new(to_source.x, 0.0, to_source.z).normalize();

    let forward_angle = forward_h.z.atan2(forward_h.x);
    let source_angle = to_h.z.atan2(to_h.x);

    let mut angle = source_angle - forward_angle;

    while angle > std::f32::consts::PI {
        angle -= 2.0 * std::f32::consts::PI;
    }
    while angle < -std::f32::consts::PI {
        angle += 2.0 * std::f32::consts::PI;
    }

    -angle
}

/// Process spatial audio for a sound instance.
///
/// Applies distance attenuation and HRTF panning to stereo output.
pub fn process_spatial<A: Fn(Vec3) -> Vec3>(
    samples: &mut [f32],
    channels: u16,
    listener: AudioListener,
    source: AudioSource,
    spatial_blend: f32,
    get_position: A,
) {
    let attenuation = calculate_attenuation(
        get_position(source.position).distance(listener.position),
        source.min_distance,
        source.max_distance,
        source.rolloff,
    );

    if spatial_blend <= 0.0 || attenuation <= 0.0 {
        let vol = attenuation;
        for sample in samples.iter_mut() {
            *sample *= vol;
        }
        return;
    }

    let angle = calculate_horiz_azimuth(listener.position, listener.forward, source.position);
    let (left_gain, right_gain) = hrtf_panning(angle);

    if channels == 1 {
        if left_gain >= right_gain {
            for sample in samples.iter_mut() {
                *sample *= left_gain * attenuation * (1.0 - spatial_blend) + attenuation * spatial_blend;
            }
        } else {
            for sample in samples.iter_mut() {
                *sample *= right_gain * attenuation * (1.0 - spatial_blend) + attenuation * spatial_blend;
            }
        }
    } else if channels == 2 {
        for chunk in samples.chunks_mut(2) {
            if chunk.len() >= 2 {
                chunk[0] *= left_gain * attenuation;
                chunk[1] *= right_gain * attenuation;
            }
        }
    } else {
        let vol = attenuation;
        for sample in samples.iter_mut() {
            *sample *= vol;
        }
    }
}

#[cfg(test)]
mod tests {
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
}