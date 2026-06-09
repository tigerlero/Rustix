use ash::vk;
use rustix_render::Renderer;
use rustix_render::RenderError;
use rustix_render::DepthBuffer;

/// G-buffer attachments and pipeline state for deferred shading.
#[allow(dead_code)]
pub struct GBufferResources {
    pub albedo_image: vk::Image,
    pub albedo_view: vk::ImageView,
    pub albedo_slot: u32,
    pub normal_image: vk::Image,
    pub normal_view: vk::ImageView,
    pub normal_slot: u32,
    pub material_image: vk::Image,
    pub material_view: vk::ImageView,
    pub material_slot: u32,
    pub depth_slot: u32,
    pub sampler_slot: u32,
    pub extent: vk::Extent2D,
    pub gbuffer_pipeline: rustix_render::pipeline::GBufferPipeline,
    pub deferred_pipeline: rustix_render::pipeline::DeferredLightingPipeline,
    _albedo_alloc: gpu_allocator::vulkan::Allocation,
    _normal_alloc: gpu_allocator::vulkan::Allocation,
    _material_alloc: gpu_allocator::vulkan::Allocation,
}

#[allow(dead_code)]
impl GBufferResources {
    pub fn new(renderer: &Renderer, extent: vk::Extent2D, depth_buf: &DepthBuffer) -> Result<Self, RenderError> {
        let device = renderer.device().logical();

        let create_gbuffer_image = |format: vk::Format, name: &str| -> Result<(vk::Image, vk::ImageView, gpu_allocator::vulkan::Allocation), RenderError> {
            let image = unsafe {
                device.create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(format)
                        .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                        .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                ).map_err(|e| RenderError::DeviceCreation(format!("{name} img: {e}")))?
            };
            let reqs = unsafe { device.get_image_memory_requirements(image) };
            let alloc = renderer.allocator.lock().allocate(&format!("gbuffer_{name}"), reqs, gpu_allocator::MemoryLocation::GpuOnly, false)?;
            unsafe { device.bind_image_memory(image, alloc.memory(), alloc.offset())?; }
            let view = unsafe {
                device.create_image_view(
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
        };

        let (albedo_img, albedo_view, albedo_alloc) = create_gbuffer_image(
            vk::Format::R8G8B8A8_UNORM, "albedo",
        )?;
        let (normal_img, normal_view, normal_alloc) = create_gbuffer_image(
            vk::Format::R16G16B16A16_SFLOAT, "normal",
        )?;
        let (material_img, material_view, material_alloc) = create_gbuffer_image(
            vk::Format::R8G8B8A8_UNORM, "material",
        )?;

        let heap = renderer.bindless_heap();
        let albedo_slot = heap.alloc_texture(albedo_view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let normal_slot = heap.alloc_texture(normal_view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let material_slot = heap.alloc_texture(material_view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let depth_slot = heap.alloc_texture(depth_buf.view, vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);

        // Create a point-clamp sampler for GBuffer reads
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE);
        let sampler = unsafe { device.create_sampler(&sampler_info, None).map_err(|e| RenderError::DeviceCreation(format!("gbuffer sampler: {e}")))? };
        let sampler_slot = heap.alloc_sampler(sampler);

        // Write GBuffer textures to fixed bindless bindings (5-9)
        let heap = renderer.bindless_heap();
        heap.write_fixed_sampled_image(5, albedo_view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        heap.write_fixed_sampled_image(6, normal_view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        heap.write_fixed_sampled_image(7, material_view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        heap.write_fixed_sampled_image(8, depth_buf.view, vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);
        heap.write_fixed_sampler(9, sampler);

        let bindless_layout = renderer.bindless_heap().layout();
        let gbp = {
            let vs = rustix_render::shader::builtin::gbuffer_vertex_shader(device)?;
            let fs = rustix_render::shader::builtin::gbuffer_fragment_shader(device)?;
            rustix_render::pipeline::GBufferPipeline::create(renderer.device(), &vs, &fs, bindless_layout)?
        };
        let dp = {
            let vs = rustix_render::shader::builtin::deferred_vertex_shader(device)?;
            let fs = rustix_render::shader::builtin::deferred_fragment_shader(device)?;
            rustix_render::pipeline::DeferredLightingPipeline::create(renderer.device(), &vs, &fs, bindless_layout)?
        };

        Ok(Self {
            albedo_image: albedo_img,
            albedo_view,
            albedo_slot,
            normal_image: normal_img,
            normal_view,
            normal_slot,
            material_image: material_img,
            material_view,
            material_slot,
            depth_slot,
            sampler_slot,
            extent,
            gbuffer_pipeline: gbp,
            deferred_pipeline: dp,
            _albedo_alloc: albedo_alloc,
            _normal_alloc: normal_alloc,
            _material_alloc: material_alloc,
        })
    }
}
