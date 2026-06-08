//! Profiler panel: frame time breakdown.

use std::collections::VecDeque;

/// A single timing sample.
#[derive(Debug, Clone, Copy)]
pub struct ProfileSample {
    pub name: &'static str,
    pub duration_ms: f32,
}

/// Ring buffer of frame timing data.
#[derive(Debug, Clone)]
pub struct ProfilerState {
    pub frame_times: VecDeque<f32>,
    pub max_samples: usize,
    pub current_frame: Vec<ProfileSample>,
}

impl ProfilerState {
    pub fn new(max_samples: usize) -> Self {
        Self {
            frame_times: VecDeque::with_capacity(max_samples),
            max_samples,
            current_frame: Vec::new(),
        }
    }

    pub fn begin_frame(&mut self) {
        self.current_frame.clear();
    }

    pub fn add_sample(&mut self, name: &'static str, duration_ms: f32) {
        self.current_frame.push(ProfileSample { name, duration_ms });
    }

    pub fn end_frame(&mut self, total_ms: f32) {
        self.frame_times.push_back(total_ms);
        if self.frame_times.len() > self.max_samples {
            self.frame_times.pop_front();
        }
    }

    pub fn average_fps(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let avg_ms: f32 = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
        if avg_ms > 0.0 {
            1000.0 / avg_ms
        } else {
            0.0
        }
    }

    pub fn frame_time_min(&self) -> f32 {
        self.frame_times.iter().copied().fold(f32::INFINITY, f32::min)
    }

    pub fn frame_time_max(&self) -> f32 {
        self.frame_times.iter().copied().fold(0.0f32, f32::max)
    }
}
