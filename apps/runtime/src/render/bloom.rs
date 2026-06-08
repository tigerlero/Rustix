use ash::vk;
use rustix_render::Renderer;
use rustix_render::RenderError;

/// Persistent GPU images for the bloom gaussian pyramid.
/// Mip 0 is half resolution, mip 1 is quarter, mip 2 is eighth, mip 3 is sixteenth.
/// Two ping-pong chains are used: a (downsample) and b (upsample).
pub struct BloomResources {
    pub mip0a_image: vk::Image,
    pub mip0a_view: vk::ImageView,
    pub mip1a_image: vk::Image,
    pub mip1a_view: vk::ImageView,
    pub mip2a_image: vk::Image,
    pub mip2a_view: vk::ImageView,
    pub mip3_image: vk::Image,
    pub mip3_view: vk::ImageView,
    pub mip2b_image: vk::Image,
    pub mip2b_view: vk::ImageView,
    pub mip1b_image: vk::Image,
    pub mip1b_view: vk::ImageView,
    pub mip0b_image: vk::Image,
    pub mip0b_view: vk::ImageView,
    pub extent0: vk::Extent2D,
    pub format: vk::Format,
    _allocations: Vec<gpu_allocator::vulkan::Allocation>,
}

impl BloomResources {
    pub fn new(renderer: &Renderer, width: u32, height: u32) -> Result<Self, RenderError> {
        let format = vk::Format::R16G16B16A16_SFLOAT;
        let w0 = (width / 2).max(1);
        let h0 = (height / 2).max(1);
        let w1 = (w0 / 2).max(1);
        let h1 = (h0 / 2).max(1);
        let w2 = (w1 / 2).max(1);
        let h2 = (h1 / 2).max(1);
        let w3 = (w2 / 2).max(1);
        let h3 = (h2 / 2).max(1);

        let mut allocations = Vec::with_capacity(10);
        let mip0a = create_bloom_image(renderer, w0, h0, format, &mut allocations)?;
        let mip1a = create_bloom_image(renderer, w1, h1, format, &mut allocations)?;
        let mip2a = create_bloom_image(renderer, w2, h2, format, &mut allocations)?;
        let mip3 = create_bloom_image(renderer, w3, h3, format, &mut allocations)?;
        let mip2b = create_bloom_image(renderer, w2, h2, format, &mut allocations)?;
        let mip1b = create_bloom_image(renderer, w1, h1, format, &mut allocations)?;
        let mip0b = create_bloom_image(renderer, w0, h0, format, &mut allocations)?;

        Ok(Self {
            mip0a_image: mip0a.0,
            mip0a_view: mip0a.1,
            mip1a_image: mip1a.0,
            mip1a_view: mip1a.1,
            mip2a_image: mip2a.0,
            mip2a_view: mip2a.1,
            mip3_image: mip3.0,
            mip3_view: mip3.1,
            mip2b_image: mip2b.0,
            mip2b_view: mip2b.1,
            mip1b_image: mip1b.0,
            mip1b_view: mip1b.1,
            mip0b_image: mip0b.0,
            mip0b_view: mip0b.1,
            extent0: vk::Extent2D { width: w0, height: h0 },
            format,
            _allocations: allocations,
        })
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            device.destroy_image_view(self.mip0a_view, None);
            device.destroy_image_view(self.mip1a_view, None);
            device.destroy_image_view(self.mip2a_view, None);
            device.destroy_image_view(self.mip3_view, None);
            device.destroy_image_view(self.mip2b_view, None);
            device.destroy_image_view(self.mip1b_view, None);
            device.destroy_image_view(self.mip0b_view, None);
            device.destroy_image(self.mip0a_image, None);
            device.destroy_image(self.mip1a_image, None);
            device.destroy_image(self.mip2a_image, None);
            device.destroy_image(self.mip3_image, None);
            device.destroy_image(self.mip2b_image, None);
            device.destroy_image(self.mip1b_image, None);
            device.destroy_image(self.mip0b_image, None);
        }
        for alloc in self._allocations.drain(..) {
            let _ = alloc;
        }
    }
}

fn create_bloom_image(
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
                .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            None,
        ).map_err(|e| RenderError::DeviceCreation(format!("bloom image: {e}")))?
    };

    let reqs = unsafe { renderer.device().logical().get_image_memory_requirements(image) };
    let alloc = renderer.allocator.lock().allocate("bloom", reqs, gpu_allocator::MemoryLocation::GpuOnly, false)
        .map_err(|e| RenderError::DeviceCreation(format!("bloom alloc: {e}")))?;
    unsafe {
        renderer.device().logical().bind_image_memory(image, alloc.memory(), alloc.offset())
            .map_err(|e| RenderError::DeviceCreation(format!("bloom bind: {e}")))?;
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
        ).map_err(|e| RenderError::DeviceCreation(format!("bloom view: {e}")))?
    };

    allocations.push(alloc);
    Ok((image, view))
}
