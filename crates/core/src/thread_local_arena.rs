use std::alloc::Layout;
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::memory::FrameAllocator;

/// Per-thread bump arena that eliminates cross-thread contention.
///
/// Each thread that calls `allocate` gets its own [`FrameAllocator`]
/// from a shared pool.  Because threads never touch the same cursor,
/// there is zero contention on the fast path.
///
/// `reset_all()` clears every arena so the memory can be reused next
/// frame.  This is the only point where a lock is taken.
pub struct ThreadLocalArena {
    /// One arena per thread index.  Protected by a light-weight lock
    /// only during `reset_all` and first-access registration.
    arenas: parking_lot::Mutex<Vec<FrameAllocator>>,
    next_index: AtomicUsize,
    capacity_per_arena: usize,
}

impl ThreadLocalArena {
    /// Create an arena pool for up to `max_threads` threads, each
    /// with a bump region of `capacity_per_arena` bytes.
    pub fn new(max_threads: usize, capacity_per_arena: usize) -> Self {
        let mut arenas = Vec::with_capacity(max_threads);
        for _ in 0..max_threads {
            arenas.push(FrameAllocator::new(capacity_per_arena));
        }
        Self {
            arenas: parking_lot::Mutex::new(arenas),
            next_index: AtomicUsize::new(0),
            capacity_per_arena,
        }
    }

    /// Allocate memory from this thread's local arena.
    ///
    /// The first call on a given thread registers it and binds an
    /// arena.  Subsequent calls are lock-free.
    pub fn allocate(&self, layout: Layout) -> Option<*mut u8> {
        TL_STATE.with(|state| {
            let mut st = state.borrow_mut();
            if st.arena.is_none() {
                let idx = self.next_index.fetch_add(1, Ordering::Relaxed);
                let arenas = self.arenas.lock();
                let arena = arenas.get(idx).expect(
                    "ThreadLocalArena ran out of arenas; increase max_threads"
                );
                st.arena = Some(arena as *const FrameAllocator);
            }
            // Safety: arena pointer is valid for the lifetime of `self`
            let arena = unsafe { &*st.arena.unwrap() };
            arena.allocate(layout)
        })
    }

    /// Allocate a value of type `T` in the thread-local arena.
    pub fn alloc<T>(&self, val: T) -> Option<&mut T> {
        let ptr = self.allocate(Layout::new::<T>())? as *mut T;
        unsafe {
            ptr.write(val);
            Some(&mut *ptr)
        }
    }

    /// Reset every arena.  O(arenas) — one atomic write per arena.
    pub fn reset_all(&self) {
        let arenas = self.arenas.lock();
        for arena in arenas.iter() {
            arena.reset();
        }
    }

    /// Total bytes reserved across all arenas.
    pub fn total_capacity(&self) -> usize {
        let arenas = self.arenas.lock();
        arenas.len() * self.capacity_per_arena
    }

    /// Sum of bytes currently used in all arenas.
    pub fn total_used(&self) -> usize {
        let arenas = self.arenas.lock();
        arenas.iter().map(|a| a.used()).sum()
    }

    /// Number of arenas currently bound to threads.
    pub fn bound_threads(&self) -> usize {
        self.next_index.load(Ordering::Relaxed)
    }
}

thread_local! {
    static TL_STATE: RefCell<TlState> = const { RefCell::new(TlState { arena: None }) };
}

struct TlState {
    /// Raw pointer to the thread's bound [`FrameAllocator`].
    arena: Option<*const FrameAllocator>,
}
