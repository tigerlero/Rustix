use std::time::Instant;

use crate::input::InputEvent;

/// A single input event with its timestamp (seconds from recording start).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimedEvent {
    pub time: f64,
    pub event: InputEvent,
}

/// Serialized recording of a sequence of input events.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct InputRecording {
    pub events: Vec<TimedEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderMode {
    Idle,
    Recording,
    Playing,
    Paused,
}

/// Records input events to a file and replays them later.
pub struct InputRecorder {
    mode: RecorderMode,
    /// Events being recorded or played back.
    recording: InputRecording,
    /// Monotonic start time for recording or playback.
    start: Option<Instant>,
    /// Current playback position (seconds).
    playback_time: f64,
    /// Next event index to emit during playback.
    playback_idx: usize,
}

impl Default for InputRecorder {
    fn default() -> Self { Self::new() }
}

impl InputRecorder {
    pub fn new() -> Self {
        Self {
            mode: RecorderMode::Idle,
            recording: InputRecording::default(),
            start: None,
            playback_time: 0.0,
            playback_idx: 0,
        }
    }

    pub fn mode(&self) -> RecorderMode { self.mode }

    /// Start a new recording, discarding any previous data.
    pub fn start_recording(&mut self) {
        self.mode = RecorderMode::Recording;
        self.recording.events.clear();
        self.start = Some(Instant::now());
    }

    /// Record an event with the current timestamp.
    /// Call this every time a raw input event is received.
    pub fn record(&mut self, event: InputEvent) {
        if self.mode != RecorderMode::Recording { return; }
        if let Some(start) = self.start {
            let t = start.elapsed().as_secs_f64();
            self.recording.events.push(TimedEvent { time: t, event });
        }
    }

    /// Stop recording and return the captured recording.
    pub fn stop_recording(&mut self) -> InputRecording {
        self.mode = RecorderMode::Idle;
        self.start = None;
        self.recording.clone()
    }

    /// Load a recording and prepare for playback from the beginning.
    pub fn start_playback(&mut self, recording: InputRecording) {
        self.mode = RecorderMode::Playing;
        self.recording = recording;
        self.start = Some(Instant::now());
        self.playback_time = 0.0;
        self.playback_idx = 0;
    }

    /// Poll for events that should fire now during playback.
    /// Returns `Vec<InputEvent>` to inject into `InputManager::push_event`.
    pub fn poll_playback(&mut self) -> Vec<InputEvent> {
        if self.mode != RecorderMode::Playing { return Vec::new(); }
        let elapsed = self.start.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
        let mut out = Vec::new();
        while self.playback_idx < self.recording.events.len() {
            let te = &self.recording.events[self.playback_idx];
            if te.time <= elapsed {
                out.push(te.event.clone());
                self.playback_idx += 1;
            } else {
                break;
            }
        }
        if self.playback_idx >= self.recording.events.len() {
            self.mode = RecorderMode::Idle;
        }
        out
    }

    /// Pause playback (retain position).
    pub fn pause_playback(&mut self) {
        if self.mode == RecorderMode::Playing {
            self.mode = RecorderMode::Paused;
            self.playback_time = self.start.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
        }
    }

    /// Resume paused playback.
    pub fn resume_playback(&mut self) {
        if self.mode == RecorderMode::Paused {
            self.mode = RecorderMode::Playing;
            self.start = Instant::now().checked_sub(std::time::Duration::from_secs_f64(self.playback_time));
        }
    }

    /// Stop playback entirely.
    pub fn stop_playback(&mut self) {
        self.mode = RecorderMode::Idle;
        self.playback_idx = 0;
        self.playback_time = 0.0;
    }
}

/// Save a recording to a JSON file path.
pub fn save_recording(path: &std::path::Path, recording: &InputRecording) -> Option<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(recording).ok()?;
    std::fs::write(path, json).ok()
}

/// Load a recording from a JSON file path.
pub fn load_recording(path: &std::path::Path) -> Option<InputRecording> {
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}
