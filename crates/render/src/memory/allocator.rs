use ash::vk;
use gpu_allocator::MemoryLocation;
use gpu_allocator::vulkan::{Allocation, Allocator, AllocatorCreateDesc};

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
        let desc = gpu_allocator::vulkan::AllocationCreateDesc {
            name,
            requirements,
            location,
            linear,
            allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
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
