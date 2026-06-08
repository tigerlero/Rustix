//! RenderDoc capture trigger integration.

use std::sync::atomic::{AtomicBool, Ordering};

/// RenderDoc capture controller.
#[derive(Debug)]
pub struct RenderDocCapture {
    pub enabled: AtomicBool,
    pub capture_next_frame: AtomicBool,
}

impl Default for RenderDocCapture {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            capture_next_frame: AtomicBool::new(false),
        }
    }
}

impl RenderDocCapture {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle RenderDoc capture on/off.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Request a capture on the next frame (e.g., bound to F12).
    pub fn trigger(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.capture_next_frame.store(true, Ordering::Relaxed);
        }
    }

    /// Check if a capture was requested and clear the flag.
    pub fn consume_trigger(&self) -> bool {
        self.capture_next_frame.swap(false, Ordering::Relaxed)
    }
}
