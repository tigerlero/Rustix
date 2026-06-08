//! Tests for procedural noise functions.

use crate::noise::{value, fbm, Perlin, domain_warp};

#[test]
fn value_noise_deterministic() {
    let a = value(1.5, 2.5, 42);
    let b = value(1.5, 2.5, 42);
    assert_eq!(a, b, "value noise should be deterministic for same inputs");
}

#[test]
fn value_noise_range() {
    // Sample at many points; all should be within [-1, 1]
    for x in 0..20 {
        for z in 0..20 {
            let v = value(x as f32 * 0.3, z as f32 * 0.3, 0);
            assert!(
                v >= -1.0 && v <= 1.0,
                "value noise out of range at ({}, {}): {}",
                x, z, v
            );
        }
    }
}

#[test]
fn value_noise_different_seeds() {
    let a = value(3.3, 4.4, 0);
    let b = value(3.3, 4.4, 1);
    assert_ne!(a, b, "different seeds should produce different values");
}

#[test]
fn fbm_range() {
    for x in 0..10 {
        for z in 0..10 {
            let v = fbm(x as f32 * 0.5, z as f32 * 0.5, 0, 4, 0.5, 2.0);
            assert!(
                v >= -1.0 && v <= 1.0,
                "FBM out of range at ({}, {}): {}",
                x, z, v
            );
        }
    }
}

#[test]
fn fbm_deterministic() {
    let a = fbm(1.0, 2.0, 99, 3, 0.5, 2.0);
    let b = fbm(1.0, 2.0, 99, 3, 0.5, 2.0);
    assert_eq!(a, b);
}

#[test]
fn perlin_deterministic() {
    let p = Perlin::new(123);
    let a = p.noise(1.2, 3.4);
    let b = p.noise(1.2, 3.4);
    assert_eq!(a, b);
}

#[test]
fn perlin_range() {
    let p = Perlin::new(0);
    for x in 0..20 {
        for z in 0..20 {
            let v = p.noise(x as f32 * 0.25, z as f32 * 0.25);
            assert!(
                v >= -1.0 && v <= 1.0,
                "Perlin noise out of range at ({}, {}): {}",
                x, z, v
            );
        }
    }
}

#[test]
fn perlin_different_seeds() {
    let p0 = Perlin::new(0);
    let p1 = Perlin::new(1);
    let a = p0.noise(5.5, 6.6);
    let b = p1.noise(5.5, 6.6);
    assert_ne!(a, b);
}

#[test]
fn perlin_fbm_range() {
    let p = Perlin::new(7);
    let v = p.fbm(1.0, 2.0, 4, 0.5, 2.0);
    assert!(v >= -1.0 && v <= 1.0, "Perlin FBM out of range: {}", v);
}

#[test]
fn domain_warp_preserves_range() {
    let warp = |x: f32, y: f32| value(x, y, 0);
    let main = |x: f32, y: f32| value(x, y, 1);
    let v = domain_warp(1.0, 2.0, 0.5, 0.1, warp, main);
    // Warped output should still be in [-1, 1] because both fn outputs are
    assert!(v >= -1.0 && v <= 1.0, "domain warp out of range: {}", v);
}

#[test]
fn domain_warp_deterministic() {
    let warp = |x: f32, y: f32| value(x, y, 0);
    let main = |x: f32, y: f32| value(x, y, 1);
    let a = domain_warp(3.0, 4.0, 0.3, 0.2, warp, main);
    let b = domain_warp(3.0, 4.0, 0.3, 0.2, warp, main);
    assert_eq!(a, b);
}
