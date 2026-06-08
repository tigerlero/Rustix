//! Tracy GPU profiling zones stub.
//!
//! Real Tracy GPU integration requires the `tracy-client` crate
//! and GPU timestamp calibration. This module provides the API
//! surface so call sites can be instrumented immediately.

/// A Tracy GPU zone scope (no-op without tracy-client).
#[derive(Debug)]
pub struct TracyGpuZone {
    pub name: &'static str,
}

impl TracyGpuZone {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

/// Begin a Tracy GPU zone. In a real build this would emit
/// a Tracy GPU timestamp query begin.
pub fn begin_zone(name: &'static str) -> TracyGpuZone {
    TracyGpuZone::new(name)
}

/// End a Tracy GPU zone. In a real build this would emit
/// the matching end timestamp.
pub fn end_zone(_zone: TracyGpuZone) {
    // No-op stub
}

/// Collect GPU timestamps and submit them to Tracy.
pub fn collect_timestamps() {
    // No-op stub
}
