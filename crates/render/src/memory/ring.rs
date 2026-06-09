use ash::vk;
use gpu_allocator::MemoryLocation;

use crate::device::GpuDevice;
use crate::memory::allocator::GpuMemoryAllocator;
use crate::memory::buffer::GpuBuffer;
use crate::RenderError;

/// A ring-buffer allocator for per-frame uniform / storage buffer data.
///
/// Wraps a single [`GpuBuffer`] with `UNIFORM_BUFFER | STORAGE_BUFFER` usage
/// and sub-allocates aligned regions using [`GpuStagingRing`]. Each allocation
/// is tagged with a fence value; when the GPU signals completion the space is
/// reclaimed automatically.
///
/// This is intended for data that changes every frame (view-projection matrices,
/// material parameters, draw-indirect structs, etc.). A typical usage pattern is:
pub struct GpuUniformRing {
    buffer: GpuBuffer,
    ring: rustix_core::gpu_staging::GpuStagingRing,
    align: u64,
    #[allow(dead_code)]
    device: *const ash::Device,
}

unsafe impl Send for GpuUniformRing {}
unsafe impl Sync for GpuUniformRing {}

impl GpuUniformRing {
    /// Create a new uniform ring with the given capacity.
    ///
    /// `capacity` is rounded up to the next multiple of the device's
    /// `minUniformBufferOffsetAlignment`.
    pub fn new(
        device: &GpuDevice,
        allocator: &mut GpuMemoryAllocator,
        capacity: u64,
    ) -> Result<Self, RenderError> {
        let align = device.physical_properties().limits.min_uniform_buffer_offset_alignment as u64;
        let aligned_cap = ((capacity + align - 1) / align) * align;
        let buffer = GpuBuffer::new(
            device,
            allocator,
            "gpu_uniform_ring",
            aligned_cap.max(align),
            vk::BufferUsageFlags::UNIFORM_BUFFER | vk::BufferUsageFlags::STORAGE_BUFFER,
            MemoryLocation::CpuToGpu,
        )?;
        Ok(Self {
            device: device.logical() as *const ash::Device,
            ring: rustix_core::gpu_staging::GpuStagingRing::new(aligned_cap.max(align)),
            align: align.max(1),
            buffer,
        })
    }

    /// Allocate `size` bytes tagged with `fence_value`.
    ///
    /// The returned offset is aligned to `minUniformBufferOffsetAlignment`.
    /// Returns `(mapped_ptr, device_offset, allocated_size)` on success,
    /// or `None` if the ring is full.
    pub fn allocate(
        &self,
        size: u64,
        fence_value: u64,
    ) -> Option<(*mut u8, u64, u64)> {
        let aligned_size = ((size + self.align - 1) / self.align) * self.align;
        let (offset, _sz) = self.ring.allocate(aligned_size, self.align)?;
        self.ring.set_fence_on_last(fence_value);
        let ptr = self.buffer.mapped_ptr?;
        let write_ptr = unsafe { ptr.add(offset as usize) };
        Some((write_ptr, offset, aligned_size))
    }

    /// Flush a CPU-written range so the GPU sees it.
    pub fn flush(&self, offset: u64, size: u64) {
        self.buffer.flush(offset, size);
    }

    /// Reclaim all regions whose fence is <= `completed_fence`.
    pub fn release_completed(&self, completed_fence: u64) {
        self.ring.release_completed(completed_fence);
    }

    /// Raw Vulkan buffer handle (for descriptor writes).
    pub fn buffer_handle(&self) -> vk::Buffer {
        self.buffer.buffer
    }

    /// Required alignment for allocations.
    pub fn alignment(&self) -> u64 {
        self.align
    }

    /// Total capacity in bytes.
    pub fn capacity(&self) -> u64 {
        self.ring.capacity()
    }

    /// Bytes currently committed.
    pub fn used(&self) -> u64 {
        self.ring.used()
    }

    /// Bytes still free.
    pub fn free(&self) -> u64 {
        self.ring.free()
    }

    /// Number of in-flight regions.
    pub fn region_count(&self) -> usize {
        self.ring.region_count()
    }

    /// Wait until the ring is completely empty.
    pub fn wait_idle(&self) {
        self.ring.wait_idle();
    }
}
