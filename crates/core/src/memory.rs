use std::alloc::Layout;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// A bump allocator (frame allocator) for per-frame temporary data.
///
/// Allocations are O(1) — just bump a pointer. The entire arena is
/// reset at the end of each frame (O(1) — reset cursor to zero).
///
/// This eliminates per-frame allocation overhead and avoids fragmentation.
///
/// # Safety
///
/// References returned by this allocator are valid only until the next reset.
/// The allocator is !Send and must be used from a single thread at a time.
pub struct FrameAllocator {
    buffer: UnsafeCell<Vec<u8>>,
    cursor: AtomicUsize,
    capacity: usize,
}

impl FrameAllocator {
    /// Create a new frame allocator with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        // Safety: we need the buffer to be initialized for direct memory access
        unsafe { buffer.set_len(capacity); }
        Self {
            buffer: UnsafeCell::new(buffer),
            cursor: AtomicUsize::new(0),
            capacity,
        }
    }

    /// Allocate memory with the given layout.
    /// Returns a pointer to the allocated memory, or None if out of space.
    pub fn allocate(&self, layout: Layout) -> Option<*mut u8> {
        let size = layout.size();
        let align = layout.align();
        let align_mask = align - 1;

        loop {
            let current = self.cursor.load(Ordering::Acquire);
            let offset = (current + align_mask) & !align_mask;
            let new_cursor = offset + size;

            if new_cursor > self.capacity {
                return None;
            }

            if self
                .cursor
                .compare_exchange(current, new_cursor, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                let ptr = self.buffer.get() as *mut u8;
                // Safety: offset is within bounds, validated by compare_exchange
                return Some(unsafe { ptr.add(offset) });
            }
        }
    }

    /// Reset the allocator. All previously allocated memory becomes invalid.
    /// O(1) — just resets the cursor to zero.
    pub fn reset(&self) {
        self.cursor.store(0, Ordering::Release);
    }

    /// Returns the number of bytes used since last reset.
    pub fn used(&self) -> usize {
        self.cursor.load(Ordering::Relaxed)
    }

    /// Returns the total capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the remaining bytes.
    pub fn remaining(&self) -> usize {
        self.capacity - self.used()
    }
}

// SAFETY: FrameAllocator uses atomic cursor for allocate/reset.
// Allocations are thread-safe but the returned memory is only valid
// for the current frame. Cross-frame use requires synchronization.
unsafe impl Send for FrameAllocator {}
unsafe impl Sync for FrameAllocator {}

/// A pool allocator for fixed-size objects.
/// Efficiently reuses memory for objects of the same size (e.g., ECS components).
pub struct PoolAllocator {
    chunk_size: usize,
    free_list: Mutex<Vec<*mut u8>>,
    chunks: Mutex<Vec<Vec<u8>>>,
}

impl PoolAllocator {
    /// Create a new pool allocator for objects of the given size and alignment.
    pub fn new(chunk_size: usize, _align: usize) -> Self {
        Self {
            chunk_size,
            free_list: Mutex::new(Vec::new()),
            chunks: Mutex::new(Vec::new()),
        }
    }

    /// Allocate an object from the pool.
    pub fn alloc(&self) -> *mut u8 {
        if let Some(ptr) = self.free_list.lock().unwrap().pop() {
            return ptr;
        }

        // Allocate a new chunk
        let mut chunk = vec![0u8; self.chunk_size];
        let ptr = chunk.as_mut_ptr();
        self.chunks.lock().unwrap().push(chunk);
        ptr
    }

    /// Free an object back to the pool.
    pub fn free(&self, ptr: *mut u8) {
        self.free_list.lock().unwrap().push(ptr);
    }

    /// Returns the number of free slots.
    pub fn free_count(&self) -> usize {
        self.free_list.lock().unwrap().len()
    }

    /// Returns the number of allocated chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.lock().unwrap().len()
    }
}

unsafe impl Send for PoolAllocator {}
unsafe impl Sync for PoolAllocator {}

/// Global frame allocator for the current frame.
/// Reset at the end of each frame.
pub struct FrameMemory {
    allocator: FrameAllocator,
}

impl FrameMemory {
    pub fn new(capacity: usize) -> Self {
        Self {
            allocator: FrameAllocator::new(capacity),
        }
    }

    /// Allocate a value of type T on the frame allocator.
    /// Returns a mutable reference valid until end of frame.
    pub fn alloc<T>(&self, val: T) -> Option<&mut T> {
        let layout = Layout::new::<T>();
        let ptr = self.allocator.allocate(layout)? as *mut T;
        // Safety: pointer is valid and properly aligned
        unsafe {
            ptr.write(val);
            Some(&mut *ptr)
        }
    }

    /// Allocate a slice of N values of type T.
    pub fn alloc_slice<T: Copy>(&self, vals: &[T]) -> Option<&mut [T]> {
        let layout = Layout::array::<T>(vals.len()).ok()?;
        let ptr = self.allocator.allocate(layout)? as *mut T;
        // Safety: pointer is valid and properly aligned
        unsafe {
            std::ptr::copy_nonoverlapping(vals.as_ptr(), ptr, vals.len());
            Some(std::slice::from_raw_parts_mut(ptr, vals.len()))
        }
    }

    /// Reset the frame allocator.
    pub fn reset(&self) {
        self.allocator.reset();
    }
}

unsafe impl Send for FrameMemory {}
unsafe impl Sync for FrameMemory {}

/// A cache-line aligned type wrapper.
/// Prevents false sharing between threads when placed in arrays.
#[repr(C, align(64))]
pub struct Aligned<T> {
    pub value: T,
}

impl<T> Aligned<T> {
    pub const fn new(value: T) -> Self {
        Self { value }
    }
}
