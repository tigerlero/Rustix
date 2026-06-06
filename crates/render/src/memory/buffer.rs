use ash::vk;
use gpu_allocator::vulkan::Allocation;
use gpu_allocator::MemoryLocation;

use crate::device::GpuDevice;
use crate::memory::allocator::GpuMemoryAllocator;
use crate::RenderError;

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
