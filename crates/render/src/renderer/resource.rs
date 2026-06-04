use ash::vk;
use crate::memory::GpuBuffer;
use crate::texture::{DepthBuffer, GpuTexture};
use crate::error::RenderError;

/// Pure function returning the `ImageCreateInfo` for a shadow depth map.
/// Tests can inspect this without a Vulkan device.
pub fn shadow_map_image_info(size: u32) -> vk::ImageCreateInfo<'static> {
    vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::D32_SFLOAT)
        .extent(vk::Extent3D { width: size, height: size, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
}

/// Pure function returning the `SamplerCreateInfo` for a shadow map.
/// Uses NEAREST filtering, CLAMP_TO_BORDER addressing, and FLOAT_OPAQUE_WHITE border.
pub fn shadow_sampler_info() -> vk::SamplerCreateInfo<'static> {
    vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::NEAREST)
        .min_filter(vk::Filter::NEAREST)
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
        .compare_enable(false)
        .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
}

/// Pure function returning pipeline barrier parameters for a given layout transition.
/// Returns (src_stage, dst_stage, src_access_mask, dst_access_mask) using Synchronization2 flags.
pub fn layout_transition_params(
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) -> (vk::PipelineStageFlags2, vk::PipelineStageFlags2, vk::AccessFlags2, vk::AccessFlags2) {
    match (old_layout, new_layout) {
        (vk::ImageLayout::UNDEFINED, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL) => (
            vk::PipelineStageFlags2::TOP_OF_PIPE,
            vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS,
            vk::AccessFlags2::empty(),
            vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
        ),
        (vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
            vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS,
            vk::PipelineStageFlags2::FRAGMENT_SHADER,
            vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
            vk::AccessFlags2::SHADER_READ,
        ),
        (vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL) => (
            vk::PipelineStageFlags2::FRAGMENT_SHADER,
            vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS,
            vk::AccessFlags2::SHADER_READ,
            vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
        ),
        _ => (
            vk::PipelineStageFlags2::ALL_COMMANDS,
            vk::PipelineStageFlags2::ALL_COMMANDS,
            vk::AccessFlags2::empty(),
            vk::AccessFlags2::empty(),
        ),
    }
}

impl super::Renderer {
    pub fn create_buffer(&self, name: &str, size: u64, usage: vk::BufferUsageFlags, location: gpu_allocator::MemoryLocation) -> Result<GpuBuffer, RenderError> {
        GpuBuffer::new(&self.device, &mut self.allocator.lock(), name, size, usage, location)
    }

    pub fn create_depth_buffer(&self, extent: vk::Extent2D) -> Result<DepthBuffer, RenderError> {
        let fmt = vk::Format::D32_SFLOAT;
        let img = unsafe {
            self.device.logical().create_image(
                &vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).format(fmt)
                    .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                    .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL).usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("depth img: {e}")))?
        };
        let req = unsafe { self.device.logical().get_image_memory_requirements(img) };
        let alloc = self.allocator.lock().allocate("depth", req, gpu_allocator::MemoryLocation::GpuOnly, false)?;
        unsafe { self.device.logical().bind_image_memory(img, alloc.memory(), alloc.offset())?; }
        let view = unsafe {
            self.device.logical().create_image_view(
                &vk::ImageViewCreateInfo::default().image(img).view_type(vk::ImageViewType::TYPE_2D).format(fmt)
                    .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::DEPTH, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 }), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("depth view: {e}")))?
        };
        Ok(DepthBuffer { image: img, view, _allocation: alloc })
    }

    /// Deprecated: descriptor pools are now managed by `DescriptorSetAllocator`.
    /// Use `Renderer::allocate_descriptor_set(layout)` instead.
    pub fn create_descriptor_pool(&self) -> Result<vk::DescriptorPool, RenderError> {
        let ps = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 1 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLED_IMAGE, descriptor_count: 1 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLER, descriptor_count: 1 },
        ];
        unsafe { self.device.logical().create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().pool_sizes(&ps).max_sets(1), None) }
            .map_err(|e| RenderError::DeviceCreation(format!("desc pool: {e}")))
    }

    /// Deprecated: use `Renderer::allocate_descriptor_set(layout)` for pool-recycled allocation.
    pub fn alloc_descriptor_set(&self, pool: vk::DescriptorPool, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet, RenderError> {
        let ls = [layout];
        unsafe { self.device.logical().allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&ls)) }
            .map_err(|e| RenderError::DeviceCreation(format!("alloc set: {e}"))).map(|mut s| s.remove(0))
    }

    pub fn update_descriptor_set(&self, _set: vk::DescriptorSet, ubo: &GpuBuffer) {
        self.bindless_heap.write_ubo(ubo.buffer, ubo.size);
    }

    pub fn update_descriptor_set_with_shadow(&self, _set: vk::DescriptorSet, ubo: &GpuBuffer, shadow_map: &GpuTexture) {
        self.bindless_heap.write_ubo(ubo.buffer, ubo.size);
        // Shadow map should already be registered in the bindless heap.
        // If not, this is a no-op; the caller manages heap registration.
    }

    pub fn create_shadow_map(&self, size: u32) -> Result<GpuTexture, RenderError> {
        let info = shadow_map_image_info(size);
        let fmt = info.format;
        let img = unsafe {
            self.device.logical().create_image(&info, None)
                .map_err(|e| RenderError::DeviceCreation(format!("shadow img: {e}")))?
        };
        let req = unsafe { self.device.logical().get_image_memory_requirements(img) };
        let alloc = self.allocator.lock().allocate("shadow_map", req, gpu_allocator::MemoryLocation::GpuOnly, false)?;
        unsafe { self.device.logical().bind_image_memory(img, alloc.memory(), alloc.offset())?; }
        let view = unsafe {
            self.device.logical().create_image_view(
                &vk::ImageViewCreateInfo::default().image(img).view_type(vk::ImageViewType::TYPE_2D).format(fmt)
                    .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::DEPTH, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 }), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("shadow view: {e}")))?
        };
        let sampler = self.device.sampler_cache()
            .get_or_create(&shadow_sampler_info())
            .map_err(|e| RenderError::DeviceCreation(format!("shadow sampler: {e}")))?;
        Ok(GpuTexture { image: img, view, sampler, _allocation: alloc })
    }

    pub fn transition_image_layout(&self, cmd: vk::CommandBuffer, image: vk::Image, aspect: vk::ImageAspectFlags, old_layout: vk::ImageLayout, new_layout: vk::ImageLayout) {
        let (src_stage, dst_stage, src_mask, dst_mask) = layout_transition_params(old_layout, new_layout);
        let barrier = vk::ImageMemoryBarrier2::default()
            .old_layout(old_layout).new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: aspect, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 })
            .src_stage_mask(src_stage)
            .dst_stage_mask(dst_stage)
            .src_access_mask(src_mask)
            .dst_access_mask(dst_mask);
        let barriers = [barrier];
        let dep_info = vk::DependencyInfo::default()
            .image_memory_barriers(&barriers);
        unsafe {
            self.device.logical().cmd_pipeline_barrier2(cmd, &dep_info);
        }
    }
}
