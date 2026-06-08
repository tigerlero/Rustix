use ash::vk;
use rustix_render::Renderer;
use rustix_render::RenderError;

/// Persistent GPU images for weighted blended OIT.
/// - accumulation: RGBA16F (weighted premultiplied color)
/// - revealage: R16F (alpha coverage)
/// - composite: RGBA16F (result of blending transparency over opaque)
pub struct OitResources {
    pub accum_image: vk::Image,
    pub accum_view: vk::ImageView,
    pub reveal_image: vk::Image,
    pub reveal_view: vk::ImageView,
    pub composite_image: vk::Image,
    pub composite_view: vk::ImageView,
    pub extent: vk::Extent2D,
    pub format: vk::Format,
    _allocations: Vec<gpu_allocator::vulkan::Allocation>,
}

impl OitResources {
    pub fn new(renderer: &Renderer, width: u32, height: u32) -> Result<Self, RenderError> {
        let accum_format = vk::Format::R16G16B16A16_SFLOAT;
        let reveal_format = vk::Format::R16_SFLOAT;
        let composite_format = vk::Format::R16G16B16A16_SFLOAT;
        let extent = vk::Extent2D { width, height };

        let mut allocations = Vec::with_capacity(3);
        let accum = create_oit_image(renderer, width, height, accum_format, &mut allocations)?;
        let reveal = create_oit_image(renderer, width, height, reveal_format, &mut allocations)?;
        let composite = create_oit_image(renderer, width, height, composite_format, &mut allocations)?;

        Ok(Self {
            accum_image: accum.0,
            accum_view: accum.1,
            reveal_image: reveal.0,
            reveal_view: reveal.1,
            composite_image: composite.0,
            composite_view: composite.1,
            extent,
            format: composite_format,
            _allocations: allocations,
        })
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            device.destroy_image_view(self.accum_view, None);
            device.destroy_image_view(self.reveal_view, None);
            device.destroy_image_view(self.composite_view, None);
            device.destroy_image(self.accum_image, None);
            device.destroy_image(self.reveal_image, None);
            device.destroy_image(self.composite_image, None);
        }
        for alloc in self._allocations.drain(..) {
            let _ = alloc;
        }
    }
}

fn create_oit_image(
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
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            None,
        ).map_err(|e| RenderError::DeviceCreation(format!("oit image: {e}")))?
    };

    let reqs = unsafe { renderer.device().logical().get_image_memory_requirements(image) };
    let alloc = renderer.allocator.lock().allocate("oit", reqs, gpu_allocator::MemoryLocation::GpuOnly, false)
        .map_err(|e| RenderError::DeviceCreation(format!("oit alloc: {e}")))?;
    unsafe {
        renderer.device().logical().bind_image_memory(image, alloc.memory(), alloc.offset())
            .map_err(|e| RenderError::DeviceCreation(format!("oit bind: {e}")))?;
    }

    let view = unsafe {
        renderer.device().logical().create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0, level_count: 1,
                    base_array_layer: 0, layer_count: 1,
                }),
            None,
        ).map_err(|e| RenderError::DeviceCreation(format!("oit view: {e}")))?
    };

    allocations.push(alloc);
    Ok((image, view))
}
