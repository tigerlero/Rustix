//! Tests for GPU staging ring buffer.

use crate::gpu_staging::GpuStagingRing;

#[test]
fn ring_new_empty() {
    let ring = GpuStagingRing::new(1024);
    assert_eq!(ring.capacity(), 1024);
    assert_eq!(ring.used(), 0);
    assert_eq!(ring.free(), 1024);
}

#[test]
fn ring_allocate_linear() {
    let ring = GpuStagingRing::new(1024);
    let (off, sz) = ring.allocate(64, 4).unwrap();
    assert_eq!(off, 0);
    assert_eq!(sz, 64);
    assert_eq!(ring.used(), 64);
}

#[test]
fn ring_allocate_aligned() {
    let ring = GpuStagingRing::new(1024);
    ring.allocate(63, 4).unwrap(); // 63 → aligned to 64
    let (off, _) = ring.allocate(16, 4).unwrap();
    assert_eq!(off, 64);
}

#[test]
fn ring_allocate_wrap_around() {
    let ring = GpuStagingRing::new(128);
    // Fill almost to end
    let (off1, _) = ring.allocate(100, 4).unwrap();
    assert_eq!(off1, 0);

    // Mark it complete so tail can move
    ring.set_fence_on_last(1);
    ring.release_completed(1);
    assert_eq!(ring.tail(), 100);

    // Now allocate 40 bytes — should wrap to beginning because
    // only 28 bytes remain after head
    let (off2, sz2) = ring.allocate(40, 4).unwrap();
    assert_eq!(off2, 0); // wrapped
    assert_eq!(sz2, 40);
}

#[test]
fn ring_full_returns_none() {
    let ring = GpuStagingRing::new(64);
    ring.allocate(32, 1).unwrap();
    ring.allocate(32, 1).unwrap();
    assert!(ring.allocate(1, 1).is_none());
}

#[test]
fn ring_release_reclaims_space() {
    let ring = GpuStagingRing::new(256);
    ring.allocate(64, 4).unwrap();
    ring.set_fence_on_last(10);
    ring.allocate(64, 4).unwrap();
    ring.set_fence_on_last(20);

    assert_eq!(ring.used(), 128);

    ring.release_completed(10);
    assert_eq!(ring.tail(), 64);
    assert_eq!(ring.used(), 64);

    ring.release_completed(20);
    assert_eq!(ring.tail(), 128);
    assert_eq!(ring.used(), 0);
}

#[test]
fn ring_release_non_contiguous() {
    let ring = GpuStagingRing::new(256);
    ring.allocate(64, 4).unwrap();
    ring.set_fence_on_last(10);
    ring.allocate(64, 4).unwrap();
    ring.set_fence_on_last(5); // older fence!

    // Fence 5 completes first, but region 1 (fence 10) is still in flight
    ring.release_completed(5);
    // Tail can't advance past region 1 because region 2 is still in flight
    assert_eq!(ring.tail(), 0);
    assert_eq!(ring.used(), 128);

    // Now region 1 completes too
    ring.release_completed(10);
    assert_eq!(ring.tail(), 128);
    assert_eq!(ring.used(), 0);
}

#[test]
fn ring_wait_idle_with_reset() {
    let ring = GpuStagingRing::new(256);
    ring.allocate(64, 4).unwrap();
    ring.set_fence_on_last(1);
    unsafe { ring.reset() };
    assert_eq!(ring.used(), 0);
    assert_eq!(ring.region_count(), 0);
}

#[test]
fn ring_multi_thread_allocators() {
    use std::sync::Arc;
    let ring = Arc::new(GpuStagingRing::new(4096));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let r = ring.clone();
            std::thread::spawn(move || {
                for _ in 0..10 {
                    if let Some((off, sz)) = r.allocate(32, 4) {
                        assert!(off + sz <= r.capacity());
                    }
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(ring.used(), 4 * 10 * 32);
}
