use ash::vk;
use gpu_allocator::MemoryLocation;

use crate::device::GpuDevice;
use crate::memory::allocator::GpuMemoryAllocator;
use crate::memory::buffer::GpuBuffer;
use crate::RenderError;

/// Staging buffer pool: a host-visible ring-buffer for CPU → GPU data uploads.
pub struct StagingBufferPool {
    buffer: Option<GpuBuffer>,
    cursor: u64,
    device: *const ash::Device, // non-owning for destroy
}

unsafe impl Send for StagingBufferPool {}
unsafe impl Sync for StagingBufferPool {}

impl StagingBufferPool {
    pub fn new() -> Self {
        Self {
            buffer: None,
            cursor: 0,
            device: std::ptr::null(),
        }
    }

    pub fn init(
        &mut self,
        device: &GpuDevice,
        allocator: &mut GpuMemoryAllocator,
        size: u64,
    ) -> Result<(), RenderError> {
        self.buffer = Some(GpuBuffer::new(
            device,
            allocator,
            "staging_pool",
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MemoryLocation::CpuToGpu,
        )?);
        self.device = device.logical() as *const ash::Device;
        Ok(())
    }

    pub fn upload<T: Sized>(
        &mut self,
        data: &[T],
    ) -> Result<(*const u8, vk::Buffer, u64), RenderError> {
        let buf = self.buffer.as_ref()
            .ok_or_else(|| RenderError::DeviceCreation("staging pool not initialized".into()))?;
        let byte_size = (data.len() * std::mem::size_of::<T>()) as u64;

        if self.cursor + byte_size > buf.size {
            self.cursor = 0;
        }

        let offset = self.cursor;
        let mapped = buf.mapped_ptr
            .ok_or_else(|| RenderError::DeviceCreation("staging buffer not mapped".into()))?;
        unsafe {
            let dst = mapped.add(offset as usize);
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                dst,
                byte_size as usize,
            );
        }
        self.cursor = offset + byte_size;

        Ok((mapped, buf.buffer, offset))
    }

    pub fn upload_to_device(
        &mut self,
        device: &GpuDevice,
        command_pool: vk::CommandPool,
        src_data: &[u8],
        dst_buffer: vk::Buffer,
        dst_offset: u64,
    ) -> Result<(), RenderError> {
        let (_ptr, src_buffer, src_offset) = {
            // Reinterpret data as &[u32] to avoid alignment issues
            let aligned_len = (src_data.len() + 3) / 4;
            let mut aligned = vec![0u32; aligned_len];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    src_data.as_ptr(),
                    aligned.as_mut_ptr() as *mut u8,
                    src_data.len(),
                );
            }
            self.upload(&aligned)?
        };

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let cmd = unsafe {
            device
                .logical()
                .allocate_command_buffers(&alloc_info)?
                .into_iter()
                .next()
                .ok_or_else(|| RenderError::DeviceCreation("no command buffer allocated".into()))?
        };

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            device.logical().begin_command_buffer(cmd, &begin_info)?;
        }

        let copy_region = vk::BufferCopy::default()
            .src_offset(src_offset)
            .dst_offset(dst_offset)
            .size(src_data.len() as u64);
        unsafe {
            device
                .logical()
                .cmd_copy_buffer(cmd, src_buffer, dst_buffer, &[copy_region]);
            device.logical().end_command_buffer(cmd)?;
        }

        let cmds = [cmd];
        let submit_info = vk::SubmitInfo::default()
            .command_buffers(&cmds);
        let submits = [submit_info];

        unsafe {
            device
                .logical()
                .queue_submit(device.transfer_queue(), &submits, vk::Fence::null())?;
            device.logical().queue_wait_idle(device.transfer_queue())?;
        }

        unsafe {
            device.logical().free_command_buffers(command_pool, &[cmd]);
        }

        Ok(())
    }
}

/// A coherent, mapped ring-buffer for CPU → GPU uploads with fence tracking.
///
/// Wraps a [`GpuBuffer`] created with `MemoryLocation::CpuToGpu` and
/// sub-allocates from it using [`GpuStagingRing`].  Each upload is
/// tagged with a fence value; when the GPU signals completion the
/// space is reclaimed automatically.
pub struct GpuStagingBuffer {
    buffer: GpuBuffer,
    ring: rustix_core::gpu_staging::GpuStagingRing,
    device: *const ash::Device,
}

unsafe impl Send for GpuStagingBuffer {}
unsafe impl Sync for GpuStagingBuffer {}

impl GpuStagingBuffer {
    /// Create a new staging buffer with the given capacity.
    pub fn new(
        device: &GpuDevice,
        allocator: &mut GpuMemoryAllocator,
        capacity: u64,
    ) -> Result<Self, RenderError> {
        let buffer = GpuBuffer::new(
            device,
            allocator,
            "gpu_staging_ring",
            capacity,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MemoryLocation::CpuToGpu,
        )?;
        Ok(Self {
            device: device.logical() as *const ash::Device,
            ring: rustix_core::gpu_staging::GpuStagingRing::new(capacity),
            buffer,
        })
    }

    /// Allocate `size` bytes with `align` alignment, tagged with `fence_value`.
    ///
    /// Returns `(mapped_ptr, device_offset)` on success, or `None` if the
    /// ring is full.
    pub fn allocate(
        &self,
        size: u64,
        align: u64,
        fence_value: u64,
    ) -> Option<(*mut u8, u64)> {
        let (offset, _sz) = self.ring.allocate(size, align)?;
        self.ring.set_fence_on_last(fence_value);
        let ptr = self.buffer.mapped_ptr?;
        let write_ptr = unsafe { ptr.add(offset as usize) };
        Some((write_ptr, offset))
    }

    /// Flush a CPU-written range so the GPU sees it.
    ///
    /// Only needed when the backing memory is not **HOST_COHERENT**.
    /// For `CpuToGpu` memory this is usually a no-op but is provided
    /// for correctness.
    pub fn flush(&self, offset: u64, size: u64) {
        self.buffer.flush(offset, size);
    }

    /// Reclaim all regions whose fence is <= `completed_fence`.
    pub fn release_completed(&self, completed_fence: u64) {
        self.ring.release_completed(completed_fence);
    }

    /// Raw Vulkan buffer handle (for `vkCmdCopyBuffer`).
    pub fn buffer_handle(&self) -> vk::Buffer {
        self.buffer.buffer
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
