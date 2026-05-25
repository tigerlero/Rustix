//! Audio effects: compressor, equalizer, reverb.
//!
//! All effects are pure-Rust DSP, compatible with the `SoundInstance` pipeline.

/// Trait for audio effects that process stereo interleaved f32 samples in-place.
pub trait AudioEffect: Send {
    fn process(&mut self, samples: &mut [f32]);
    fn reset(&mut self);
    fn name(&self) -> &str;
}

// --- Effect Chain ---

/// Chains multiple effects, processing samples through each in order.
pub struct EffectChain {
    effects: Vec<Box<dyn AudioEffect>>,
    enabled: Vec<bool>,
}

impl std::fmt::Debug for EffectChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectChain").field("count", &self.effects.len()).finish()
    }
}

impl EffectChain {
    pub fn new() -> Self { Self { effects: Vec::new(), enabled: Vec::new() } }

    pub fn add(&mut self, effect: Box<dyn AudioEffect>) -> usize {
        let idx = self.effects.len();
        self.effects.push(effect);
        self.enabled.push(true);
        idx
    }

    pub fn enable(&mut self, idx: usize, on: bool) {
        if idx < self.enabled.len() { self.enabled[idx] = on; }
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for (effect, &enabled) in self.effects.iter_mut().zip(&self.enabled) {
            if enabled { effect.process(samples); }
        }
    }

    pub fn reset(&mut self) { for e in &mut self.effects { e.reset(); } }
}

// --- Compressor ---

/// Dynamic range compressor with soft knee.
pub struct Compressor {
    threshold: f32,       // dB, typically -24..0
    ratio: f32,           // e.g. 4.0 = 4:1
    attack: f32,          // seconds
    release: f32,         // seconds
    makeup_gain: f32,     // dB
    sample_rate: u32,
    envelope: f32,
}

impl Compressor {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            threshold: -12.0,
            ratio: 4.0,
            attack: 0.005,
            release: 0.1,
            makeup_gain: 0.0,
            sample_rate,
            envelope: 0.0,
        }
    }

    pub fn set_threshold(&mut self, db: f32) { self.threshold = db; }
    pub fn set_ratio(&mut self, ratio: f32) { self.ratio = ratio.max(1.0); }
    pub fn set_attack(&mut self, secs: f32) { self.attack = secs.max(0.0001); }
    pub fn set_release(&mut self, secs: f32) { self.release = secs.max(0.0001); }
    pub fn set_makeup_gain(&mut self, db: f32) { self.makeup_gain = db; }

    fn gain_reduction(&self, level_db: f32) -> f32 {
        if level_db < self.threshold - 3.0 {
            0.0
        } else if level_db > self.threshold + 3.0 {
            (level_db - self.threshold) * (1.0 - 1.0 / self.ratio)
        } else {
            let excess = level_db - self.threshold + 3.0;
            let knee = excess * excess / (12.0 * self.ratio);
            knee * (1.0 - 1.0 / self.ratio)
        }
    }
}

impl AudioEffect for Compressor {
    fn process(&mut self, samples: &mut [f32]) {
        let attack_coeff = (-1.0 / (self.attack * self.sample_rate as f32)).exp();
        let release_coeff = (-1.0 / (self.release * self.sample_rate as f32)).exp();
        let makeup = 10.0f32.powf(self.makeup_gain / 20.0);

        for sample in samples.iter_mut() {
            let abs_val = sample.abs().max(1e-10);
            let level_db = 20.0 * abs_val.log10();
            let target_gr = self.gain_reduction(level_db);

            let coeff = if target_gr > self.envelope { attack_coeff } else { release_coeff };
            self.envelope = coeff * self.envelope + (1.0 - coeff) * target_gr;

            let gain = 10.0f32.powf(-self.envelope / 20.0) * makeup;
            *sample *= gain;
        }
    }

    fn reset(&mut self) { self.envelope = 0.0; }
    fn name(&self) -> &str { "Compressor" }
}

// --- Biquad Filter (for EQ) ---

enum BiquadType { LowShelf, Peaking, HighShelf }

struct Biquad {
    b0: f32, b1: f32, b2: f32, a1: f32, a2: f32,
    x1: f32, x2: f32, y1: f32, y2: f32,
}

impl Biquad {
    fn new(typ: BiquadType, freq: f32, q: f32, gain_db: f32, sample_rate: u32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w = w0.cos();
        let sin_w = w0.sin();
        let alpha = sin_w / (2.0 * q);
        let a = 10.0f32.powf(gain_db / 40.0);

        let (b0, b1, b2, a0, a1, a2) = match typ {
            BiquadType::LowShelf => {
                let ap1 = a + 1.0; let am1 = a - 1.0;
                let sqrt_a = a.sqrt();
                (a * (ap1 - am1 * cos_w + 2.0 * sqrt_a * alpha),
                 a * 2.0 * (am1 - ap1 * cos_w),
                 a * (ap1 - am1 * cos_w - 2.0 * sqrt_a * alpha),
                 ap1 + am1 * cos_w + 2.0 * sqrt_a * alpha,
                 -2.0 * (am1 + ap1 * cos_w),
                 ap1 + am1 * cos_w - 2.0 * sqrt_a * alpha)
            }
            BiquadType::Peaking => {
                (1.0 + alpha * a, -2.0 * cos_w, 1.0 - alpha * a,
                 1.0 + alpha / a, -2.0 * cos_w, 1.0 - alpha / a)
            }
            BiquadType::HighShelf => {
                let ap1 = a + 1.0; let am1 = a - 1.0;
                let sqrt_a = a.sqrt();
                (a * (ap1 + am1 * cos_w + 2.0 * sqrt_a * alpha),
                 -a * 2.0 * (am1 + ap1 * cos_w),
                 a * (ap1 + am1 * cos_w - 2.0 * sqrt_a * alpha),
                 ap1 - am1 * cos_w + 2.0 * sqrt_a * alpha,
                 2.0 * (am1 - ap1 * cos_w),
                 ap1 - am1 * cos_w - 2.0 * sqrt_a * alpha)
            }
        };

        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 }
    }

    fn process_sample(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = x;
        self.y2 = self.y1; self.y1 = y;
        y
    }
}

// --- Equalizer ---

/// 3-band equalizer: low shelf, peaking (mid), high shelf.
pub struct Equalizer {
    low: Biquad,
    mid: Biquad,
    high: Biquad,
}

impl Equalizer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            low: Biquad::new(BiquadType::LowShelf, 250.0, 0.7, 0.0, sample_rate),
            mid: Biquad::new(BiquadType::Peaking, 1000.0, 0.7, 0.0, sample_rate),
            high: Biquad::new(BiquadType::HighShelf, 4000.0, 0.7, 0.0, sample_rate),
        }
    }

    pub fn set_low_gain(&mut self, db: f32, sample_rate: u32) {
        self.low = Biquad::new(BiquadType::LowShelf, 250.0, 0.7, db, sample_rate);
    }

    pub fn set_mid_gain(&mut self, db: f32, sample_rate: u32) {
        self.mid = Biquad::new(BiquadType::Peaking, 1000.0, 0.7, db, sample_rate);
    }

    pub fn set_high_gain(&mut self, db: f32, sample_rate: u32) {
        self.high = Biquad::new(BiquadType::HighShelf, 4000.0, 0.7, db, sample_rate);
    }
}

impl AudioEffect for Equalizer {
    fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            *sample = self.low.process_sample(*sample);
            *sample = self.mid.process_sample(*sample);
            *sample = self.high.process_sample(*sample);
        }
    }

    fn reset(&mut self) {
        self.low = Biquad { x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0, ..self.low };
        self.mid = Biquad { x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0, ..self.mid };
        self.high = Biquad { x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0, ..self.high };
    }

    fn name(&self) -> &str { "Equalizer" }
}

// --- Reverb ---

/// Freeverb-style reverb: 8 comb filters + 4 allpass filters in series.
pub struct Reverb {
    comb: [CombFilter; 8],
    allpass: [AllpassFilter; 4],
    wet: f32,
    dry: f32,
}

struct CombFilter {
    buffer: Vec<f32>,
    idx: usize,
    feedback: f32,
    damp: f32,
    damp_state: f32,
}

impl CombFilter {
    fn new(delay: usize, feedback: f32, damp: f32) -> Self {
        Self { buffer: vec![0.0; delay + 1], idx: 0, feedback, damp, damp_state: 0.0 }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.idx];
        self.damp_state = output * (1.0 - self.damp) + self.damp_state * self.damp;
        self.buffer[self.idx] = input + self.damp_state * self.feedback;
        self.idx = (self.idx + 1) % self.buffer.len();
        output
    }
}

struct AllpassFilter {
    buffer: Vec<f32>,
    idx: usize,
}

impl AllpassFilter {
    fn new(delay: usize) -> Self {
        Self { buffer: vec![0.0; delay + 1], idx: 0 }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.idx];
        let output = -input + buf_out;
        self.buffer[self.idx] = input + buf_out * 0.5;
        self.idx = (self.idx + 1) % self.buffer.len();
        output
    }
}

impl Reverb {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as usize;
        // Classic Freeverb tuning for 44.1kHz, scaled to sample rate
        let scale = sr as f32 / 44100.0;
        let comb_delays = [
            (1557.0 * scale) as usize, (1617.0 * scale) as usize,
            (1491.0 * scale) as usize, (1422.0 * scale) as usize,
            (1277.0 * scale) as usize, (1356.0 * scale) as usize,
            (1188.0 * scale) as usize, (1116.0 * scale) as usize,
        ];
        let allpass_delays = [
            (225.0 * scale) as usize, (556.0 * scale) as usize,
            (441.0 * scale) as usize, (341.0 * scale) as usize,
        ];

        Self {
            comb: [
                CombFilter::new(comb_delays[0], 0.84, 0.2),
                CombFilter::new(comb_delays[1], 0.84, 0.2),
                CombFilter::new(comb_delays[2], 0.84, 0.2),
                CombFilter::new(comb_delays[3], 0.84, 0.2),
                CombFilter::new(comb_delays[4], 0.84, 0.2),
                CombFilter::new(comb_delays[5], 0.84, 0.2),
                CombFilter::new(comb_delays[6], 0.84, 0.2),
                CombFilter::new(comb_delays[7], 0.84, 0.2),
            ],
            allpass: [
                AllpassFilter::new(allpass_delays[0]),
                AllpassFilter::new(allpass_delays[1]),
                AllpassFilter::new(allpass_delays[2]),
                AllpassFilter::new(allpass_delays[3]),
            ],
            wet: 0.3,
            dry: 0.7,
        }
    }

    pub fn set_wet(&mut self, wet: f32) { self.wet = wet.clamp(0.0, 1.0); self.dry = 1.0 - wet; }
}

impl AudioEffect for Reverb {
    fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            let dry_sample = *sample;

            // Sum comb filters
            let mut wet_out = 0.0;
            for comb in &mut self.comb {
                wet_out += comb.process(dry_sample);
            }
            wet_out *= 0.125; // normalize

            // Series allpass
            for ap in &mut self.allpass {
                wet_out = ap.process(wet_out);
            }

            *sample = dry_sample * self.dry + wet_out * self.wet;
        }
    }

    fn reset(&mut self) {
        for c in &mut self.comb { c.buffer.fill(0.0); c.damp_state = 0.0; c.idx = 0; }
        for a in &mut self.allpass { a.buffer.fill(0.0); a.idx = 0; }
    }

    fn name(&self) -> &str { "Reverb" }
}

#[cfg(test)]
mod tests {
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
}
