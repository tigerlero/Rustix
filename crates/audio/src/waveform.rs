//! Waveform visualization: generate amplitude bar data for UI rendering.
//!
//! `Waveform` holds per-bar min/max amplitudes that can be drawn as
//! vertical bars by any renderer (egui, imgui, custom).

use std::path::Path;

use crate::{AudioError, SoundInstance};
use crate::decoder::decode_audio;

/// One bar of a waveform: minimum and maximum amplitude in that segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveformBar {
    pub min: f32,
    pub max: f32,
}

/// A waveform sampled down to a fixed number of bars.
#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
    pub bars: Vec<WaveformBar>,
    /// Sample rate of the source audio.
    pub sample_rate: u32,
    /// Number of channels in the source audio.
    pub channels: u16,
    /// Duration in seconds.
    pub duration: f32,
}

impl Waveform {
    /// Number of bars in the waveform.
    pub fn len(&self) -> usize {
        self.bars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bars.is_empty()
    }

    /// Overall min and max amplitude across all bars.
    pub fn bounds(&self) -> (f32, f32) {
        let mut min = 0.0f32;
        let mut max = 0.0f32;
        for bar in &self.bars {
            min = min.min(bar.min);
            max = max.max(bar.max);
        }
        (min, max)
    }

    /// Sample rate of the source audio.
    pub fn sample_rate(&self) -> u32 { self.sample_rate }

    /// Duration in seconds.
    pub fn duration(&self) -> f32 { self.duration }
}

/// Generate a `Waveform` from decoded interleaved f32 samples.
///
/// `width` is the number of bars (pixels) to produce.
/// For stereo audio the channels are averaged per sample before
/// computing min/max.
///
/// ```rust
/// let wf = generate_waveform(&samples, channels, sample_rate, 400);
/// for bar in &wf.bars {
///     // draw a vertical line from bar.min to bar.max
/// }
/// ```
pub fn generate_waveform(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    width: usize,
) -> Waveform {
    if samples.is_empty() || width == 0 {
        return Waveform {
            bars: Vec::new(),
            sample_rate,
            channels,
            duration: 0.0,
        };
    }

    let samples_per_bar = (samples.len() / channels as usize).max(1) / width.max(1);
    if samples_per_bar == 0 {
        // More bars than samples: one sample per bar.
        let bars: Vec<WaveformBar> = samples
            .chunks(channels as usize)
            .map(|chunk| {
                let v = if channels > 1 {
                    chunk.iter().sum::<f32>() / channels as f32
                } else {
                    chunk[0]
                };
                WaveformBar { min: v, max: v }
            })
            .collect();
        let duration = samples.len() as f32 / (channels as f32 * sample_rate as f32);
        return Waveform { bars, sample_rate, channels, duration };
    }

    let mut bars = Vec::with_capacity(width);
    let sample_count = samples.len() / channels as usize;

    for bar_idx in 0..width {
        let start = bar_idx * samples_per_bar;
        let end = ((start + samples_per_bar).min(sample_count)).max(start);
        if start >= sample_count {
            break;
        }

        let mut bar_min = f32::MAX;
        let mut bar_max = f32::MIN;

        for s in start..end {
            let frame_start = s * channels as usize;
            let frame_end = (frame_start + channels as usize).min(samples.len());
            let v = if channels > 1 && frame_end > frame_start {
                samples[frame_start..frame_end].iter().sum::<f32>() / channels as f32
            } else if frame_start < samples.len() {
                samples[frame_start]
            } else {
                0.0
            };

            bar_min = bar_min.min(v);
            bar_max = bar_max.max(v);
        }

        bars.push(WaveformBar {
            min: bar_min,
            max: bar_max,
        });
    }

    let duration = sample_count as f32 / sample_rate as f32;
    Waveform { bars, sample_rate, channels, duration }
}

/// Convenience: generate a waveform from an existing `SoundInstance`.
pub fn generate_waveform_from_instance(instance: &SoundInstance, width: usize) -> Waveform {
    generate_waveform(&instance.decoded, instance.channels, instance.sample_rate, width)
}

/// Convenience: decode a file and generate its waveform.
pub fn generate_waveform_from_path(path: &Path, width: usize) -> Result<Waveform, AudioError> {
    let (samples, sample_rate, channels) = decode_audio(path)?;
    Ok(generate_waveform(&samples, channels, sample_rate, width))
}
