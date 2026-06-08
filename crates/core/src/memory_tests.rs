//! Tests for memory allocators.

use std::alloc::Layout;
use crate::memory::{FrameAllocator, PoolAllocator, Aligned};

#[test]
fn test_frame_allocator_basic() {
    let alloc = FrameAllocator::new(1024);
    let ptr = alloc.allocate(Layout::new::<u32>()).unwrap();
    assert!(!ptr.is_null());
    assert!(alloc.used() >= 4);
    alloc.reset();
    assert_eq!(alloc.used(), 0);
}

#[test]
fn test_frame_allocator_oom() {
    let alloc = FrameAllocator::new(4);
    let ptr = alloc.allocate(Layout::new::<u32>());
    assert!(ptr.is_some());
    let ptr2 = alloc.allocate(Layout::new::<u32>());
    assert!(ptr2.is_none());
}

#[test]
fn test_pool_allocator() {
    let pool = PoolAllocator::new(64, 8);
    let ptr = pool.alloc();
    assert!(!ptr.is_null());
    pool.free(ptr);
    // After free, the next alloc should reuse the same slot
    let ptr2 = pool.alloc();
    assert_eq!(ptr, ptr2);
}

#[test]
fn test_aligned() {
    let a = Aligned::new(42u32);
    assert_eq!(a.value, 42);
    assert_eq!(std::mem::align_of_val(&a), 64);
}
