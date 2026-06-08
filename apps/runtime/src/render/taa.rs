use ash::vk;
use rustix_render::Renderer;
use rustix_render::RenderError;

/// Persistent GPU images for Temporal Anti-Aliasing.
pub struct TaaResources {
    pub history_image: vk::Image,
    pub history_view: vk::ImageView,
    pub resolved_image: vk::Image,
    pub resolved_view: vk::ImageView,
    pub extent: vk::Extent2D,
    pub format: vk::Format,
    _history_alloc: gpu_allocator::vulkan::Allocation,
    _resolved_alloc: gpu_allocator::vulkan::Allocation,
}

impl TaaResources {
    pub fn new(renderer: &Renderer, width: u32, height: u32) -> Result<Self, RenderError> {
        let format = vk::Format::R16G16B16A16_SFLOAT;
        let extent = vk::Extent2D { width, height };
        let (history_img, history_view, history_alloc) = create_taa_image(
            renderer, width, height, format,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            "taa_history",
        )?;
        let (resolved_img, resolved_view, resolved_alloc) = create_taa_image(
            renderer, width, height, format,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC,
            "taa_resolved",
        )?;

        Ok(Self {
            history_image: history_img,
            history_view: history_view,
            resolved_image: resolved_img,
            resolved_view: resolved_view,
            extent,
            format,
            _history_alloc: history_alloc,
            _resolved_alloc: resolved_alloc,
        })
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            device.destroy_image_view(self.history_view, None);
            device.destroy_image_view(self.resolved_view, None);
            device.destroy_image(self.history_image, None);
            device.destroy_image(self.resolved_image, None);
        }
    }
}

fn create_taa_image(
    renderer: &Renderer,
    width: u32,
    height: u32,
    format: vk::Format,
    usage: vk::ImageUsageFlags,
    name: &str,
) -> Result<(vk::Image, vk::ImageView, gpu_allocator::vulkan::Allocation), RenderError> {
    let image = unsafe {
        renderer.device().logical().create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D { width, height, depth: 1 })
                .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            None,
        ).map_err(|e| RenderError::DeviceCreation(format!("{name} image: {e}")))?
    };

    let reqs = unsafe { renderer.device().logical().get_image_memory_requirements(image) };
    let alloc = renderer.allocator.lock().allocate(name, reqs, gpu_allocator::MemoryLocation::GpuOnly, false)
        .map_err(|e| RenderError::DeviceCreation(format!("{name} alloc: {e}")))?;
    unsafe {
        renderer.device().logical().bind_image_memory(image, alloc.memory(), alloc.offset())
            .map_err(|e| RenderError::DeviceCreation(format!("{name} bind: {e}")))?;
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
        ).map_err(|e| RenderError::DeviceCreation(format!("{name} view: {e}")))?
    };

    Ok((image, view, alloc))
}
