//! Tests for memory tracking.

use std::alloc::Layout;
use std::sync::Arc;
use crate::memory_tracker::MemoryTracker;

#[test]
fn tracker_tracks_alloc_and_free() {
    let t = MemoryTracker::new();
    let layout = Layout::new::<u64>();
    let ptr = 0x1000 as *mut u8;
    t.track_alloc(ptr, layout, Some("test"));
    assert_eq!(t.current_used(), 8);
    assert_eq!(t.total_allocated(), 8);
    assert_eq!(t.active_count(), 1);

    assert!(t.track_free(ptr));
    assert_eq!(t.current_used(), 0);
    assert_eq!(t.total_freed(), 8);
    assert_eq!(t.active_count(), 0);
}

#[test]
fn tracker_double_free_returns_false() {
    let t = MemoryTracker::new();
    let layout = Layout::new::<u64>();
    let ptr = 0x2000 as *mut u8;
    t.track_alloc(ptr, layout, None);
    assert!(t.track_free(ptr));
    assert!(!t.track_free(ptr));
}

#[test]
fn tracker_unknown_free_returns_false() {
    let t = MemoryTracker::new();
    let ptr = 0x3000 as *mut u8;
    assert!(!t.track_free(ptr));
}

#[test]
fn tracker_peak_updates() {
    let t = MemoryTracker::new();
    t.track_alloc(0x1000 as *mut u8, Layout::new::<[u8; 100]>(), None);
    assert_eq!(t.peak_used(), 100);
    t.track_alloc(0x2000 as *mut u8, Layout::new::<[u8; 50]>(), None);
    assert_eq!(t.peak_used(), 150);
    t.track_free(0x1000 as *mut u8);
    t.track_free(0x2000 as *mut u8);
    assert_eq!(t.peak_used(), 150); // peak stays
}

#[test]
fn tracker_leak_report_no_leaks() {
    let t = MemoryTracker::new();
    let ptr = 0x4000 as *mut u8;
    t.track_alloc(ptr, Layout::new::<u32>(), None);
    t.track_free(ptr);
    // Should not panic
    t.leak_report();
    assert_eq!(t.active_count(), 0);
}

#[test]
fn tracker_leak_report_detects_leak() {
    let t = MemoryTracker::new();
    let ptr = 0x5000 as *mut u8;
    t.track_alloc(ptr, Layout::new::<u32>(), Some("leaked_block"));
    assert_eq!(t.active_count(), 1);
    t.leak_report();
    // Still active after report
    assert_eq!(t.active_count(), 1);
}

#[test]
fn tracker_reset_clears_everything() {
    let t = MemoryTracker::new();
    t.track_alloc(0x6000 as *mut u8, Layout::new::<u64>(), None);
    t.track_alloc(0x7000 as *mut u8, Layout::new::<u64>(), None);
    t.reset();
    assert_eq!(t.current_used(), 0);
    assert_eq!(t.total_allocated(), 0);
    assert_eq!(t.total_freed(), 0);
    assert_eq!(t.peak_used(), 0);
    assert_eq!(t.active_count(), 0);
}

#[test]
fn tracker_multi_thread() {
    let t = Arc::new(MemoryTracker::new());
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let tracker = t.clone();
            std::thread::spawn(move || {
                let ptr = (0x8000 + i * 0x100) as *mut u8;
                tracker.track_alloc(ptr, Layout::new::<u64>(), None);
                tracker.track_free(ptr);
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(t.current_used(), 0);
    assert_eq!(t.total_allocated(), 32);
    assert_eq!(t.total_freed(), 32);
}
