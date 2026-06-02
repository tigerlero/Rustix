use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

/// A GPU staging ring-buffer allocator.
///
/// Manages a single large host-visible, coherent memory block as a
/// circular queue.  Each allocation is tagged with a *fence value*;
/// when the GPU signals that fence, the region becomes reusable.
///
/// # Design
///
/// ```text
///  tail                    head
///   |                       |
///   v                       v
///  [  free  |  in-use  |  free  ]
///            ^            ^
///            |            |
///         oldest      newest
///         alloc       alloc
/// ```
///
/// `head` advances on `allocate`.  `tail` advances when
/// `release_completed(fence_value)` shows that the oldest allocations
/// are done on the GPU.
///
/// # Safety
///
/// The caller must supply the actual mapped CPU pointer and GPU
/// buffer handle.  This allocator only tracks *offsets* and fence
/// lifetimes.
pub struct GpuStagingRing {
    capacity: u64,
    head: AtomicU64,
    tail: AtomicU64,
    regions: parking_lot::Mutex<VecDeque<Region>>,
}

#[derive(Debug, Clone, Copy)]
struct Region {
    start: u64,
    end: u64,
    fence: u64,
}

impl GpuStagingRing {
    /// Create a new ring with the given byte capacity.
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            regions: parking_lot::Mutex::new(VecDeque::new()),
        }
    }

    /// Total byte capacity of the underlying buffer.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Current head offset (next allocation starts here).
    pub fn head(&self) -> u64 {
        self.head.load(Ordering::Relaxed)
    }

    /// Current tail offset (earliest not-yet-reclaimed byte).
    pub fn tail(&self) -> u64 {
        self.tail.load(Ordering::Relaxed)
    }

    /// Bytes currently committed (head − tail, wrapping).
    pub fn used(&self) -> u64 {
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Relaxed);
        if h >= t {
            h - t
        } else {
            self.capacity - (t - h)
        }
    }

    /// Bytes still free.
    pub fn free(&self) -> u64 {
        self.capacity - self.used()
    }

    /// Number of in-flight regions.
    pub fn region_count(&self) -> usize {
        self.regions.lock().len()
    }

    /// Try to allocate `size` bytes with `align` alignment.
    ///
    /// On success returns `(offset, size)` inside the ring.  The
    /// caller is responsible for writing to the mapped pointer at
    /// `base_ptr + offset` and for later calling
    /// `release_completed(fence)` with the actual GPU fence value.
    ///
    /// Returns `None` if the ring is full.
    pub fn allocate(&self, size: u64, align: u64) -> Option<(u64, u64)> {
        let aligned_size = Self::align_up(size, align);

        let mut head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);

        // Fast path: linear space after head
        let aligned_head = Self::align_up(head, align);
        if aligned_head + aligned_size <= self.capacity {
            if aligned_head + aligned_size <= tail || tail <= head {
                self.head.store(aligned_head + aligned_size, Ordering::Relaxed);
                self.regions.lock().push_back(Region {
                    start: aligned_head,
                    end: aligned_head + aligned_size,
                    fence: 0, // set by caller later
                });
                return Some((aligned_head, size));
            }
        }

        // Wrap-around: try from beginning
        if aligned_size <= tail {
            self.head.store(aligned_size, Ordering::Relaxed);
            self.regions.lock().push_back(Region {
                start: 0,
                end: aligned_size,
                fence: 0,
            });
            return Some((0, size));
        }

        None
    }

    /// Tag the most-recently-allocated region with a fence value.
    ///
    /// Must be called immediately after `allocate` for the same
    /// logical upload.
    pub fn set_fence_on_last(&self, fence: u64) {
        let mut regions = self.regions.lock();
        if let Some(last) = regions.back_mut() {
            last.fence = fence;
        }
    }

    /// Reclaim all regions whose fence value is <= `completed_fence`.
    ///
    /// Advances `tail` past contiguous completed regions.
    pub fn release_completed(&self, completed_fence: u64) {
        let mut regions = self.regions.lock();
        while let Some(front) = regions.front() {
            if front.fence <= completed_fence {
                let r = regions.pop_front().unwrap();
                self.tail.store(r.end, Ordering::Relaxed);
            } else {
                break;
            }
        }
    }

    /// Wait until the ring is completely empty.
    ///
    /// Spins until `used() == 0`.  Call with a very large fence
    /// value (e.g. `u64::MAX`) to force reclamation of everything.
    pub fn wait_idle(&self) {
        while self.used() > 0 {
            std::hint::spin_loop();
        }
    }

    /// Reset the ring (for device-lost / shutdown paths).
    ///
    /// # Safety
    /// Caller must ensure the GPU is no longer accessing any region.
    pub unsafe fn reset(&self) {
        self.head.store(0, Ordering::Relaxed);
        self.tail.store(0, Ordering::Relaxed);
        self.regions.lock().clear();
    }

    fn align_up(addr: u64, align: u64) -> u64 {
        if align == 0 {
            return addr;
        }
        (addr + align - 1) & !(align - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
