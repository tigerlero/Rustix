use ash::vk;
use rustix_render::Renderer;
use rustix_render::RenderError;

pub struct SkyboxResources {
    pub skybox_image: vk::Image,
    pub skybox_view: vk::ImageView,
    pub extent: vk::Extent2D,
    pub format: vk::Format,
    _alloc: gpu_allocator::vulkan::Allocation,
}

impl SkyboxResources {
    pub fn new(renderer: &Renderer, width: u32, height: u32) -> Result<Self, RenderError> {
        let format = vk::Format::R16G16B16A16_SFLOAT;
        let extent = vk::Extent2D { width, height };
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
            ).map_err(|e| RenderError::DeviceCreation(format!("skybox image: {e}")))?
        };
        let reqs = unsafe { renderer.device().logical().get_image_memory_requirements(image) };
        let alloc = renderer.allocator.lock().allocate("skybox", reqs, gpu_allocator::MemoryLocation::GpuOnly, false)
            .map_err(|e| RenderError::DeviceCreation(format!("skybox alloc: {e}")))?;
        unsafe { renderer.device().logical().bind_image_memory(image, alloc.memory(), alloc.offset())
            .map_err(|e| RenderError::DeviceCreation(format!("skybox bind: {e}")))?; }
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
            ).map_err(|e| RenderError::DeviceCreation(format!("skybox view: {e}")))?
        };
        Ok(Self { skybox_image: image, skybox_view: view, extent, format, _alloc: alloc })
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            device.destroy_image_view(self.skybox_view, None);
            device.destroy_image(self.skybox_image, None);
        }
    }
}
