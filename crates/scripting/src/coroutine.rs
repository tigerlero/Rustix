//! Coroutine support for cutscenes and async scripts.
//!
//! Lightweight cooperative multitasking for script sequences
//! such as cutscenes, scripted events, and timed animations.

use std::collections::VecDeque;

/// State of a script coroutine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutineState {
    Running,
    Suspended,
    Completed,
}

/// A single yield instruction from a coroutine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum YieldReason {
    WaitSeconds(f32),
    WaitFrames(u32),
    WaitForSignal(&'static str),
}

/// A script coroutine that can be resumed each frame.
pub trait ScriptCoroutine: Send {
    fn resume(&mut self, dt: f32) -> Option<YieldReason>;
    fn state(&self) -> CoroutineState;
    fn name(&self) -> &str;
}

/// Scheduler that manages and ticks active coroutines.
pub struct CoroutineScheduler {
    pub active: VecDeque<Box<dyn ScriptCoroutine>>,
    pub waiting: VecDeque<(Box<dyn ScriptCoroutine>, f32)>, // (coroutine, remaining_seconds)
    pub waiting_frames: VecDeque<(Box<dyn ScriptCoroutine>, u32)>,
}

impl Default for CoroutineScheduler {
    fn default() -> Self {
        Self {
            active: VecDeque::new(),
            waiting: VecDeque::new(),
            waiting_frames: VecDeque::new(),
        }
    }
}

impl std::fmt::Debug for CoroutineScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoroutineScheduler")
            .field("active", &self.active.len())
            .field("waiting", &self.waiting.len())
            .field("waiting_frames", &self.waiting_frames.len())
            .finish()
    }
}

impl CoroutineScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self, coroutine: Box<dyn ScriptCoroutine>) {
        self.active.push_back(coroutine);
    }

    /// Tick all active coroutines. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        // Tick time-waiting coroutines
        let mut ready = Vec::new();
        let mut i = 0;
        while i < self.waiting.len() {
            self.waiting[i].1 -= dt;
            if self.waiting[i].1 <= 0.0 {
                ready.push(i);
            }
            i += 1;
        }
        for idx in ready.into_iter().rev() {
            let (co, _) = self.waiting.remove(idx).unwrap();
            self.active.push_back(co);
        }

        // Tick frame-waiting coroutines
        let mut ready_frames = Vec::new();
        let mut i = 0;
        while i < self.waiting_frames.len() {
            self.waiting_frames[i].1 -= 1;
            if self.waiting_frames[i].1 == 0 {
                ready_frames.push(i);
            }
            i += 1;
        }
        for idx in ready_frames.into_iter().rev() {
            let (co, _) = self.waiting_frames.remove(idx).unwrap();
            self.active.push_back(co);
        }

        // Resume active coroutines
        let count = self.active.len();
        for _ in 0..count {
            if let Some(mut co) = self.active.pop_front() {
                match co.resume(dt) {
                    Some(YieldReason::WaitSeconds(t)) => {
                        self.waiting.push_back((co, t));
                    }
                    Some(YieldReason::WaitFrames(f)) => {
                        self.waiting_frames.push_back((co, f));
                    }
                    Some(YieldReason::WaitForSignal(_)) => {
                        // For now, put back in active; signal handling would need a registry
                        self.active.push_back(co);
                    }
                    None => {
                        // Coroutine completed
                    }
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty() && self.waiting.is_empty() && self.waiting_frames.is_empty()
    }

    pub fn clear(&mut self) {
        self.active.clear();
        self.waiting.clear();
        self.waiting_frames.clear();
    }
}

/// A simple coroutine that runs a sequence of timed steps.
pub struct CutsceneCoroutine {
    pub name: String,
    pub steps: Vec<Box<dyn FnMut() -> Option<YieldReason> + Send>>,
    pub current: usize,
    pub state: CoroutineState,
}

impl std::fmt::Debug for CutsceneCoroutine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CutsceneCoroutine")
            .field("name", &self.name)
            .field("current", &self.current)
            .field("state", &self.state)
            .field("steps", &self.steps.len())
            .finish()
    }
}

impl CutsceneCoroutine {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
            current: 0,
            state: CoroutineState::Running,
        }
    }

    pub fn wait_seconds(&mut self, seconds: f32) {
        self.steps.push(Box::new(move || Some(YieldReason::WaitSeconds(seconds))));
    }

    pub fn wait_frames(&mut self, frames: u32) {
        self.steps.push(Box::new(move || Some(YieldReason::WaitFrames(frames))));
    }

    pub fn action(&mut self, mut f: impl FnMut() + Send + 'static) {
        self.steps.push(Box::new(move || {
            f();
            None
        }));
    }
}

impl ScriptCoroutine for CutsceneCoroutine {
    fn resume(&mut self, _dt: f32) -> Option<YieldReason> {
        if self.current >= self.steps.len() {
            self.state = CoroutineState::Completed;
            return None;
        }
        let result = self.steps[self.current]();
        self.current += 1;
        if result.is_none() && self.current >= self.steps.len() {
            self.state = CoroutineState::Completed;
        }
        result
    }

    fn state(&self) -> CoroutineState {
        self.state
    }

    fn name(&self) -> &str {
        &self.name
    }
}
