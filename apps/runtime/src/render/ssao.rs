use ash::vk;
use rustix_render::Renderer;
use rustix_render::RenderError;

/// Persistent GPU images for screen-space ambient occlusion.
pub struct SsaoResources {
    pub ao_image: vk::Image,
    pub ao_view: vk::ImageView,
    pub blurred_ao_image: vk::Image,
    pub blurred_ao_view: vk::ImageView,
    pub extent: vk::Extent2D,
    pub format: vk::Format,
    _allocations: Vec<gpu_allocator::vulkan::Allocation>,
}

impl SsaoResources {
    pub fn new(renderer: &Renderer, width: u32, height: u32) -> Result<Self, RenderError> {
        let format = vk::Format::R8_UNORM;
        let w = (width / 2).max(1);
        let h = (height / 2).max(1);
        let extent = vk::Extent2D { width: w, height: h };

        let mut allocations = Vec::with_capacity(2);
        let ao = create_ssao_image(renderer, w, h, format, &mut allocations)?;
        let blurred = create_ssao_image(renderer, w, h, format, &mut allocations)?;

        Ok(Self {
            ao_image: ao.0,
            ao_view: ao.1,
            blurred_ao_image: blurred.0,
            blurred_ao_view: blurred.1,
            extent,
            format,
            _allocations: allocations,
        })
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            device.destroy_image_view(self.ao_view, None);
            device.destroy_image_view(self.blurred_ao_view, None);
            device.destroy_image(self.ao_image, None);
            device.destroy_image(self.blurred_ao_image, None);
        }
        for alloc in self._allocations.drain(..) {
            let _ = alloc;
        }
    }
}

fn create_ssao_image(
    renderer: &Renderer,
    width: u32,
    height: u32,
    format: vk::Format,
    allocations: &mut Vec<gpu_allocator::vulkan::Allocation>,
) -> Result<(vk::Image, vk::ImageView), RenderError> {
    let image = unsafe {
        renderer.device().logical().create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D { width, height, depth: 1 })
                .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            None,
        ).map_err(|e| RenderError::DeviceCreation(format!("ssao image: {e}")))?
    };

    let reqs = unsafe { renderer.device().logical().get_image_memory_requirements(image) };
    let alloc = renderer.allocator.lock().allocate("ssao", reqs, gpu_allocator::MemoryLocation::GpuOnly, false)
        .map_err(|e| RenderError::DeviceCreation(format!("ssao alloc: {e}")))?;
    unsafe {
        renderer.device().logical().bind_image_memory(image, alloc.memory(), alloc.offset())
            .map_err(|e| RenderError::DeviceCreation(format!("ssao bind: {e}")))?;
    }

    let view = unsafe {
        renderer.device().logical().create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image).view_type(vk::ImageViewType::TYPE_2D)
                .format(format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0, level_count: 1,
                    base_array_layer: 0, layer_count: 1,
                }),
            None,
        ).map_err(|e| RenderError::DeviceCreation(format!("ssao view: {e}")))?
    };

    allocations.push(alloc);
    Ok((image, view))
}
