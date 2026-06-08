//! Time API exposed to scripts: `dt`, `time`, `frame_count`.

/// Time state accessible from scripts.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScriptTime {
    pub delta_time: f64,
    pub elapsed: f64,
    pub frame_count: u64,
}

impl ScriptTime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self, dt: f64) {
        self.delta_time = dt;
        self.elapsed += dt;
        self.frame_count += 1;
    }
}
