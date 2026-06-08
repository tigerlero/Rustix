//! Tests for waveform visualization.

use crate::waveform::generate_waveform;

#[test]
fn test_sine_wave() {
    // 1 second of 440 Hz sine at 44100 Hz mono
    let sr = 44100u32;
    let samples: Vec<f32> = (0..sr)
        .map(|i| {
            let t = i as f32 / sr as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin()
        })
        .collect();

    let wf = generate_waveform(&samples, 1, sr, 100);
    assert_eq!(wf.len(), 100);
    assert!((wf.duration - 1.0).abs() < 0.01);

    // A full sine cycle should have both positive and negative bars
    let (min, max) = wf.bounds();
    assert!(min < -0.5);
    assert!(max > 0.5);
}

#[test]
fn test_stereo_average() {
    let samples = vec![1.0, -1.0, 1.0, -1.0];
    let wf = generate_waveform(&samples, 2, 44100, 2);
    // Each frame averages to 0.0
    assert_eq!(wf.bars[0].min, 0.0);
    assert_eq!(wf.bars[0].max, 0.0);
}

#[test]
fn test_empty() {
    let wf = generate_waveform(&[], 1, 44100, 100);
    assert!(wf.is_empty());
    assert_eq!(wf.duration, 0.0);
}
