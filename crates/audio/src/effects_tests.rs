#[cfg(test)]
use super::*;

#[test]
fn test_compressor_noop() {
    let mut comp = Compressor::new(44100);
    let mut samples = vec![0.1f32; 64];
    let original = samples.clone();
    comp.process(&mut samples);
    // Should reduce gain somewhat for 0.1 linear (~ -20dB)
    for (i, s) in samples.iter().enumerate() {
        assert!(s.abs() <= original[i].abs() + 0.01);
    }
}

#[test]
fn test_eq_noop() {
    let mut eq = Equalizer::new(44100);
    let mut samples = vec![0.5f32; 64];
    eq.process(&mut samples);
    // With zero gain, output should approximate input
    for &s in &samples {
        assert!((s - 0.5).abs() < 0.01, "sample was {}", s);
    }
}

#[test]
fn test_reverb_noop() {
    let mut rev = Reverb::new(44100);
    rev.set_wet(0.0);
    let mut samples = vec![0.5f32; 64];
    let original = samples.clone();
    rev.process(&mut samples);
    for (i, &s) in samples.iter().enumerate() {
        assert!((s - original[i]).abs() < 0.001);
    }
}

#[test]
fn test_effect_chain() {
    let mut chain = EffectChain::new();
    chain.add(Box::new(Compressor::new(44100)));
    chain.add(Box::new(Reverb::new(44100)));
    chain.enable(1, false); // disable reverb
    let mut samples = vec![0.2f32; 64];
    chain.process(&mut samples);
    for &s in &samples {
        assert!(s.is_finite());
    }
}
