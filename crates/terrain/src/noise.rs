//! Noise functions for procedural terrain generation.
//!
//! Includes value noise, Perlin noise, FBM, and domain warping.

use std::f32;

fn hash(n: u32) -> u32 {
    let mut x = n;
    x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3bu32);
    x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3bu32);
    (x >> 16) ^ x
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn val(x: i32, z: i32, seed: u32) -> f32 {
    let h = hash(
        (x.wrapping_add(0x9e3779b9u32 as i32) as u32)
            .wrapping_mul(0x85ebca6bu32)
            .wrapping_add(z.wrapping_add(0x9e3779b9u32 as i32) as u32)
            .wrapping_add(seed),
    );
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

pub fn value(x: f32, z: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let fx = smooth(x - ix as f32);
    let fz = smooth(z - iz as f32);

    let v00 = val(ix, iz, seed);
    let v10 = val(ix + 1, iz, seed);
    let v01 = val(ix, iz + 1, seed);
    let v11 = val(ix + 1, iz + 1, seed);

    let x0 = v00 + (v10 - v00) * fx;
    let x1 = v01 + (v11 - v01) * fx;
    x0 + (x1 - x0) * fz
}

pub fn fbm(
    x: f32,
    z: f32,
    seed: u32,
    octaves: u32,
    persistence: f32,
    lacunarity: f32,
) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    for _ in 0..octaves {
        total += value(x * frequency, z * frequency, seed) * amplitude;
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    total / max_value
}

// ── Perlin noise ──

const PERM_SIZE: usize = 256;

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn grad(hash: u8, x: f32, y: f32) -> f32 {
    let h = hash & 0xF;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 { y } else { if h == 12 || h == 14 { x } else { 0.0 } };
    let s = if (h & 1) == 0 { u } else { -u };
    let t = if (h & 2) == 0 { v } else { -v };
    s + t
}

/// Classic Perlin noise (2D).
pub struct Perlin {
    perm: [u8; PERM_SIZE * 2],
}

impl Default for Perlin {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Perlin {
    pub fn new(seed: u32) -> Self {
        let mut base: [u8; PERM_SIZE] = std::array::from_fn(|i| i as u8);
        // Shuffle with seeded RNG
        let mut state = seed;
        for i in (1..PERM_SIZE).rev() {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (state % (i as u32 + 1)) as usize;
            base.swap(i, j);
        }
        let mut perm = [0u8; PERM_SIZE * 2];
        for i in 0..PERM_SIZE {
            perm[i] = base[i];
            perm[i + PERM_SIZE] = base[i];
        }
        Self { perm }
    }

    pub fn noise(&self, x: f32, y: f32) -> f32 {
        let xi = x.floor() as i32 & 0xFF;
        let yi = y.floor() as i32 & 0xFF;
        let xf = x - x.floor();
        let yf = y - y.floor();

        let u = fade(xf);
        let v = fade(yf);

        let aa = self.perm[self.perm[xi as usize] as usize + yi as usize];
        let ab = self.perm[self.perm[xi as usize] as usize + yi as usize + 1];
        let ba = self.perm[self.perm[xi as usize + 1] as usize + yi as usize];
        let bb = self.perm[self.perm[xi as usize + 1] as usize + yi as usize + 1];

        let x1 = lerp(grad(aa, xf, yf), grad(ba, xf - 1.0, yf), u);
        let x2 = lerp(grad(ab, xf, yf - 1.0), grad(bb, xf - 1.0, yf - 1.0), u);

        lerp(x1, x2, v)
    }

    pub fn fbm(&self, x: f32, y: f32, octaves: u32, persistence: f32, lacunarity: f32) -> f32 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 1.0;
        let mut max_value = 0.0;
        for _ in 0..octaves {
            total += self.noise(x * frequency, y * frequency) * amplitude;
            max_value += amplitude;
            amplitude *= persistence;
            frequency *= lacunarity;
        }
        total / max_value
    }
}

/// Domain warp: distort input coordinates with low-frequency noise
/// before sampling the main noise function.
pub fn domain_warp<F1: Fn(f32, f32) -> f32, F2: Fn(f32, f32) -> f32>(
    x: f32,
    y: f32,
    warp_amplitude: f32,
    warp_frequency: f32,
    warp_fn: F1,
    main_fn: F2,
) -> f32 {
    let offset_x = warp_fn(x * warp_frequency, y * warp_frequency) * warp_amplitude;
    let offset_y = warp_fn(x * warp_frequency + 5.2, y * warp_frequency + 1.3) * warp_amplitude;
    main_fn(x + offset_x, y + offset_y)
}
