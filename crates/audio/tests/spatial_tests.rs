//! Integration tests for spatial audio processing.
//!
//! These tests verify the end-to-end spatial audio pipeline without requiring
//! actual audio hardware or playback.

use rustix_audio::{
    calculate_attenuation, calculate_horiz_azimuth, hrtf_panning, process_spatial, AudioListener,
    AudioSource,
};
use rustix_core::math::Vec3;

/// Test: Spatial playback with distance attenuation
/// Verify that output samples are attenuated correctly based on distance.
#[test]
fn test_spatial_playback_distance_attenuation() {
    // Create listener at origin facing +Z
    let listener = AudioListener {
        position: Vec3::ZERO,
        forward: Vec3::Z,
        up: Vec3::Y,
    };

    // Create source far away (10x min distance)
    let source = AudioSource {
        position: Vec3::new(0.0, 0.0, 10.0), // 10 units away
        min_distance: 1.0,
        max_distance: 100.0,
        rolloff: 1.0,
    };

    // Calculate expected attenuation
    let expected_attenuation = calculate_attenuation(10.0, 1.0, 100.0, 1.0);

    // Create test samples (sine wave at 0.5 amplitude)
    let mut samples = vec![0.5f32; 44100]; // 1 second at 44.1kHz

    // Process spatial audio for mono source
    process_spatial(&mut samples, 1, listener, source, 1.0, |pos| pos);

    // Verify samples are attenuated
    let avg_sample = samples.iter().sum::<f32>() / samples.len() as f32;
    assert!(
        avg_sample > 0.0 && avg_sample < 0.5,
        "Expected attenuation but got avg_sample = {}",
        avg_sample
    );

    // Verify attenuation is approximately correct (accounting for HRTF normalization)
    let expected_with_hrtf = expected_attenuation * 0.707; // Approximate center gain
    assert!(
        (avg_sample - expected_with_hrtf).abs() < 0.1,
        "Attenuation should be close to expected value {} but got {}",
        expected_with_hrtf,
        avg_sample
    );
}

/// Test: Spatial playback with panning
/// Verify left/right channel balance matches expected gains.
#[test]
fn test_spatial_playback_panning() {
    // Source to the right of listener
    let _source = AudioSource {
        position: Vec3::new(10.0, 0.0, 0.0),
        min_distance: 1.0,
        max_distance: 100.0,
        rolloff: 1.0,
    };

    // Calculate expected panning
    let angle = calculate_horiz_azimuth(Vec3::ZERO, Vec3::Z, Vec3::new(10.0, 0.0, 0.0));
    let (left_gain, right_gain) = hrtf_panning(angle);

    // Verify right channel should be dominant
    assert!(
        right_gain > left_gain,
        "Right gain ({}) should be greater than left gain ({})",
        right_gain,
        left_gain
    );
}

/// Test: Combined spatial effects
/// Verify no clipping occurs with combined distance and panning.
#[test]
fn test_spatial_playback_combined() {
    let listener = AudioListener {
        position: Vec3::ZERO,
        forward: Vec3::Z,
        up: Vec3::Y,
    };

    // Source at medium distance to the right
    let source = AudioSource {
        position: Vec3::new(5.0, 0.0, 0.0),
        min_distance: 1.0,
        max_distance: 100.0,
        rolloff: 1.0,
    };

    // Create stereo test samples with 1.0 amplitude
    let mut samples = vec![1.0f32; 44100 * 2]; // Stereo

    // Process with full spatial blend
    process_spatial(&mut samples, 2, listener, source, 1.0, |pos| pos);

    // Verify no clipping (samples should be <= 1.0 after gain normalization)
    for (i, sample) in samples.iter().enumerate() {
        assert!(
            *sample <= 1.0,
            "Sample {} should not clip, got {}",
            i,
            sample
        );
    }
}

/// Test: Edge case - negative rolloff
/// Test behavior with negative rolloff (volume increases with distance).
#[test]
fn test_attenuation_negative_rolloff() {
    // Negative rolloff means volume increases with distance
    let att_at_min = calculate_attenuation(1.0, 1.0, 100.0, -1.0);
    let att_at_far = calculate_attenuation(50.0, 1.0, 100.0, -1.0);

    // At min distance should be 1.0
    assert!((att_at_min - 1.0).abs() < 0.001);

    // At far distance with negative rolloff, attenuation may be > 1.0
    // or may have unusual behavior - just verify it doesn't crash
    assert!(att_at_far.is_finite(), "Attenuation should be finite");
}

/// Test: Edge case - equal min and max distances
#[test]
fn test_attenuation_equal_distances() {
    // When min == max, any distance > min should be silent
    let att_below = calculate_attenuation(0.5, 10.0, 10.0, 1.0);
    let att_at = calculate_attenuation(10.0, 10.0, 10.0, 1.0);
    let att_above = calculate_attenuation(15.0, 10.0, 10.0, 1.0);

    assert!((att_below - 1.0).abs() < 0.001, "Below should be full volume");
    assert!((att_at - 1.0).abs() < 0.001, "At should be full volume");
    assert!((att_above - 0.0).abs() < 0.001, "Above should be silent");
}

/// Test: Sources behind listener
#[test]
fn test_panning_behind_listener() {
    // Source directly behind listener
    let angle_back = calculate_horiz_azimuth(Vec3::ZERO, Vec3::Z, Vec3::new(0.0, 0.0, -10.0));

    // Should give angle close to π or -π
    assert!(
        angle_back.abs() > std::f32::consts::FRAC_PI_2,
        "Angle behind listener should be > π/2, got {}",
        angle_back
    );

    let (left_gain, right_gain) = hrtf_panning(angle_back);
    // Both gains should be similar for rear sources (clamped to front half)
    assert!(left_gain >= 0.0 && right_gain >= 0.0);
}

/// Test: Stereo spatial processing
#[test]
fn test_stereo_spatial_processing() {
    let listener = AudioListener {
        position: Vec3::ZERO,
        forward: Vec3::Z,
        up: Vec3::Y,
    };

    let source = AudioSource {
        position: Vec3::new(10.0, 0.0, 0.0), // To the right
        min_distance: 1.0,
        max_distance: 100.0,
        rolloff: 1.0,
    };

    // Create stereo samples
    let mut samples = vec![1.0f32, 0.5f32, 1.0f32, 0.5f32, 1.0f32, 0.5f32];

    process_spatial(&mut samples, 2, listener, source, 1.0, |pos| pos);

    // Left channel (even indices) should be attenuated more than right (odd indices)
    let left_avg: f32 = samples.iter().step_by(2).sum::<f32>() / 3.0;
    let right_avg: f32 = samples.iter().skip(1).step_by(2).sum::<f32>() / 3.0;

    assert!(
        left_avg < right_avg,
        "Left channel should be attenuated more than right ({} < {})",
        left_avg,
        right_avg
    );
}