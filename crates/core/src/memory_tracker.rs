use std::alloc::Layout;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;

/// Lightweight allocation tracker for debugging and leak detection.
///
/// Tracks every `track_alloc` / `track_free` pair.  When `leak_report`
/// is called, any allocation without a matching free is printed.
///
/// All counters are atomics so the tracker is safe to use from
/// multiple threads.  The active-allocation map is behind a mutex.
pub struct MemoryTracker {
    total_allocated: AtomicUsize,
    total_freed: AtomicUsize,
    peak_used: AtomicUsize,
    current_used: AtomicUsize,
    active: Mutex<HashMap<usize, AllocationRecord>>,
}

#[derive(Debug, Clone)]
struct AllocationRecord {
    size: usize,
    align: usize,
    label: Option<String>,
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryTracker {
    pub fn new() -> Self {
        Self {
            total_allocated: AtomicUsize::new(0),
            total_freed: AtomicUsize::new(0),
            peak_used: AtomicUsize::new(0),
            current_used: AtomicUsize::new(0),
            active: Mutex::new(HashMap::new()),
        }
    }

    /// Register a new allocation.
    pub fn track_alloc(&self, ptr: *mut u8, layout: Layout, label: Option<&str>) {
        let addr = ptr as usize;
        let size = layout.size();
        self.total_allocated.fetch_add(size, Ordering::Relaxed);
        let used = self.current_used.fetch_add(size, Ordering::Relaxed) + size;
        // Update peak
        loop {
            let peak = self.peak_used.load(Ordering::Relaxed);
            if used <= peak {
                break;
            }
            if self
                .peak_used
                .compare_exchange(peak, used, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
        self.active.lock().insert(
            addr,
            AllocationRecord {
                size,
                align: layout.align(),
                label: label.map(|s| s.to_string()),
            },
        );
    }

    /// Register a free.
    ///
    /// Returns `true` if the pointer was known to the tracker.
    pub fn track_free(&self, ptr: *mut u8) -> bool {
        let addr = ptr as usize;
        let maybe_rec = self.active.lock().remove(&addr);
        if let Some(rec) = maybe_rec {
            self.total_freed.fetch_add(rec.size, Ordering::Relaxed);
            self.current_used.fetch_sub(rec.size, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Number of bytes currently tracked as allocated.
    pub fn current_used(&self) -> usize {
        self.current_used.load(Ordering::Relaxed)
    }

    /// Total bytes ever allocated.
    pub fn total_allocated(&self) -> usize {
        self.total_allocated.load(Ordering::Relaxed)
    }

    /// Total bytes ever freed.
    pub fn total_freed(&self) -> usize {
        self.total_freed.load(Ordering::Relaxed)
    }

    /// Peak concurrent allocation size.
    pub fn peak_used(&self) -> usize {
        self.peak_used.load(Ordering::Relaxed)
    }

    /// Number of active (unfreed) allocations.
    pub fn active_count(&self) -> usize {
        self.active.lock().len()
    }

    /// Print a leak report of all unfreed allocations.
    pub fn leak_report(&self) {
        let active = self.active.lock();
        if active.is_empty() {
            tracing::info!("memory tracker: no leaks detected ({} bytes currently used)",
                self.current_used.load(Ordering::Relaxed));
            return;
        }
        tracing::warn!(
            "memory tracker: {} leaked allocation(s), {} bytes",
            active.len(),
            self.current_used.load(Ordering::Relaxed)
        );
        for (addr, rec) in active.iter() {
            let label = rec.label.as_deref().unwrap_or("<unlabeled>");
            tracing::warn!(
                "  leak: 0x{:016x} — {} bytes (align {}), label: {}",
                addr, rec.size, rec.align, label
            );
        }
    }

    /// Reset all counters and clear the active map.
    pub fn reset(&self) {
        self.total_allocated.store(0, Ordering::Relaxed);
        self.total_freed.store(0, Ordering::Relaxed);
        self.peak_used.store(0, Ordering::Relaxed);
        self.current_used.store(0, Ordering::Relaxed);
        self.active.lock().clear();
    }
}

/// A global tracker instance for convenience.
///
/// Use this when you don't want to pass a `MemoryTracker` around.
pub static GLOBAL_MEMORY_TRACKER: std::sync::LazyLock<MemoryTracker> =
    std::sync::LazyLock::new(MemoryTracker::new);

/// Convenience: track an allocation on the global tracker.
pub fn global_track_alloc(ptr: *mut u8, layout: Layout, label: Option<&str>) {
    GLOBAL_MEMORY_TRACKER.track_alloc(ptr, layout, label);
}

/// Convenience: track a free on the global tracker.
pub fn global_track_free(ptr: *mut u8) -> bool {
    GLOBAL_MEMORY_TRACKER.track_free(ptr)
}

/// Convenience: print the global leak report.
pub fn global_leak_report() {
    GLOBAL_MEMORY_TRACKER.leak_report();
}
