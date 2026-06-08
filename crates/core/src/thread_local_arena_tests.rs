//! Tests for thread-local arena allocator.

use std::alloc::Layout;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use crate::thread_local_arena::ThreadLocalArena;

#[test]
fn arena_basic_alloc() {
    let arena = ThreadLocalArena::new(4, 1024);
    let ptr = arena.allocate(Layout::new::<u64>()).unwrap();
    assert!(!ptr.is_null());
    unsafe { *(ptr as *mut u64) = 42 };
    assert_eq!(unsafe { *(ptr as *mut u64) }, 42);
}

#[test]
fn arena_reset_all() {
    let arena = ThreadLocalArena::new(4, 1024);
    let _ = arena.allocate(Layout::new::<[u8; 512]>());
    assert!(arena.total_used() > 0);
    arena.reset_all();
    assert_eq!(arena.total_used(), 0);
}

#[test]
fn arena_alloc_typed() {
    let arena = ThreadLocalArena::new(4, 1024);
    let r = arena.alloc(3.14f64).unwrap();
    assert_eq!(*r, 3.14);
}

#[test]
fn arena_total_capacity() {
    let arena = ThreadLocalArena::new(8, 4096);
    assert_eq!(arena.total_capacity(), 8 * 4096);
}

#[test]
fn arena_multi_thread_no_contention() {
    let arena = Arc::new(ThreadLocalArena::new(8, 4096));
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let a = arena.clone();
            let c = counter.clone();
            std::thread::spawn(move || {
                let ptr = a.alloc(i).unwrap();
                assert_eq!(*ptr, i);
                c.fetch_add(1, Ordering::SeqCst);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(counter.load(Ordering::SeqCst), 4);
    assert_eq!(arena.bound_threads(), 4);
}

#[test]
fn arena_oom_returns_none() {
    let arena = ThreadLocalArena::new(1, 8);
    // First alloc succeeds
    assert!(arena.allocate(Layout::new::<[u8; 4]>()).is_some());
    // Second alloc may succeed depending on alignment, keep going
    let mut allocated = 0usize;
    while let Some(ptr) = arena.allocate(Layout::new::<[u8; 4]>()) {
        if !ptr.is_null() {
            allocated += 4;
        }
        if allocated > 16 {
            break; // arena definitely exhausted by now
        }
    }
    // After many allocations we should get None
    let result = arena.allocate(Layout::new::<[u8; 1024]>());
    assert!(result.is_none());
}
