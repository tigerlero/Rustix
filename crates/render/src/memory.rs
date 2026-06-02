
use ash::vk;

use gpu_allocator::MemoryLocation;
use gpu_allocator::vulkan::{
    Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc,
};

use crate::device::GpuDevice;
use crate::instance::VulkanInstance;
use crate::RenderError;

pub struct GpuMemoryAllocator {
    allocator: Allocator,
}

impl GpuMemoryAllocator {
    pub fn new(
        instance: &VulkanInstance,
        device: &GpuDevice,
    ) -> Result<Self, RenderError> {
        let desc = AllocatorCreateDesc {
            instance: instance.inner().clone(),
            device: device.logical().clone(),
            physical_device: device.physical(),
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: Default::default(),
        };

        let allocator = Allocator::new(&desc).map_err(|e| {
            RenderError::DeviceCreation(format!("gpu-allocator: {e}"))
        })?;

        Ok(Self { allocator })
    }

    pub fn allocate(
        &mut self,
        name: &str,
        requirements: vk::MemoryRequirements,
        location: MemoryLocation,
        linear: bool,
    ) -> Result<Allocation, RenderError> {
        let desc = AllocationCreateDesc {
            name,
            requirements,
            location,
            linear,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };
        self.allocator
            .allocate(&desc)
            .map_err(|e| RenderError::DeviceCreation(format!("allocation '{name}': {e}")))
    }

    pub fn free(&mut self, allocation: Allocation) -> Result<(), RenderError> {
        self.allocator
            .free(allocation)
            .map_err(|e| RenderError::DeviceCreation(format!("free: {e}")))
    }
}

/// A GPU buffer with both the Vulkan buffer handle and its backing memory.
pub struct GpuBuffer {
    pub buffer: vk::Buffer,
    pub allocation: Allocation,
    pub size: u64,
    pub mapped_ptr: Option<*mut u8>,
    device: *const ash::Device,
}

impl Drop for GpuBuffer {
    fn drop(&mut self) {
        if !self.device.is_null() {
            unsafe { (*self.device).destroy_buffer(self.buffer, None); }
        }
    }
}

impl GpuBuffer {
    pub fn new(
        device: &GpuDevice,
        allocator: &mut GpuMemoryAllocator,
        name: &str,
        size: u64,
        usage: vk::BufferUsageFlags,
        location: MemoryLocation,
    ) -> Result<Self, RenderError> {
        let create_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe {
            device
                .logical()
                .create_buffer(&create_info, None)
                .map_err(|e| RenderError::DeviceCreation(format!("create buffer: {e}")))?
        };

        let requirements =
            unsafe { device.logical().get_buffer_memory_requirements(buffer) };

        let allocation = allocator.allocate(name, requirements, location, true)?;

        unsafe {
            device.logical().bind_buffer_memory(
                buffer,
                allocation.memory(),
                allocation.offset(),
            )?;
        }

        let mapped_ptr = if let Some(ptr) = allocation.mapped_ptr() {
            Some(ptr.as_ptr() as *mut u8)
        } else {
            None
        };

        tracing::debug!(
            name = name,
            size = size,
            usage = ?usage,
            location = ?location,
            "GPU buffer created"
        );

        Ok(Self {
            buffer,
            allocation,
            size,
            mapped_ptr,
            device: device.logical() as *const ash::Device,
        })
    }

    /// Copy data to a mapped buffer at the given byte offset. Panics if not mapped or out of bounds.
    pub fn write_at(&self, data: &[u8], offset: u64) {
        let ptr = self
            .mapped_ptr
            .expect("attempted to write to unmapped GPU buffer");
        if offset + data.len() as u64 > self.size {
            tracing::error!("GPU buffer write overflow: offset={offset} len={} size={}", data.len(), self.size);
            return;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.add(offset as usize), data.len());
        }
    }

    /// Flush mapped memory range so the GPU sees CPU writes.
    /// Needed when the backing memory is not HOST_COHERENT.
    pub fn flush(&self, offset: u64, size: u64) {
        if self.device.is_null() { return; }
        let memory = unsafe { self.allocation.memory() };
        let range = vk::MappedMemoryRange::default()
            .memory(memory)
            .offset(self.allocation.offset() + offset)
            .size(size);
        unsafe {
            let _ = (*self.device).flush_mapped_memory_ranges(&[range]);
        }
    }

    /// Copy data to a mapped buffer. Panics if not mapped.
    pub fn write(&self, data: &[u8]) {
        self.write_at(data, 0);
    }

    /// Copy data from a mapped buffer. Panics if not mapped.
    pub fn read(&self, dst: &mut [u8]) {
        let ptr = self
            .mapped_ptr
            .expect("attempted to read from unmapped GPU buffer");
        if dst.len() as u64 > self.size {
            tracing::error!("GPU buffer read overflow: len={} size={}", dst.len(), self.size);
            return;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(ptr, dst.as_mut_ptr(), dst.len());
        }
    }

    pub fn mapped_ptr(&self) -> Option<*mut u8> {
        self.mapped_ptr
    }

    pub fn destroy(&self, device: &GpuDevice, _allocator: &mut GpuMemoryAllocator) {
        unsafe {
            device.logical().destroy_buffer(self.buffer, None);
        }
        // Leak allocation — the proper way would be to take ownership.
        // For Phase 1, allocations live for the duration of the program.
    }
}

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
                .queue_submit(device.graphics_queue(), &submits, vk::Fence::null())?;
            device.logical().queue_wait_idle(device.graphics_queue())?;
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
///
/// # Example
///
/// ```rust,ignore
/// let mut staging = GpuStagingBuffer::new(&device, &mut allocator, 16 * 1024 * 1024)?;
/// let (ptr, offset) = staging.allocate(256, 4, fence_value)?;
/// unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, 256); }
/// staging.flush(offset, 256)?;
/// // Later, when GPU finishes the frame:
/// staging.release_completed(newest_completed_fence);
/// ```
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
    /// Only needed when the backing memory is **not** `HOST_COHERENT`.
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
