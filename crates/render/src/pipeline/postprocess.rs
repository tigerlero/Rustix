use ash::vk;
use crate::device::GpuDevice;
use crate::shader::ShaderModule;
use crate::swapchain::Swapchain;
use crate::spec_constants::SpecConstantMap;
use crate::RenderError;

/// Full-screen tone-mapping pipeline (HDR → SDR).
///
/// Uses a fullscreen triangle generated from `gl_VertexID` with no vertex
/// buffer. The fragment shader samples an HDR texture (binding 1) through a
/// sampler (binding 2) and applies ACES filmic tone mapping + gamma correction.
pub struct ToneMapPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl ToneMapPipeline {
    pub fn create(
        device: &GpuDevice,
        swapchain: &Swapchain,
        vs: &ShaderModule,
        fs: &ShaderModule,
        spec_constants: Option<&SpecConstantMap>,
    ) -> Result<Self, RenderError> {
        let vs_stage = vs.stage_create_info();
        let fs_stage = if let Some(spec) = spec_constants {
            let spec_data = crate::spec_constants::SpecConstantData::from_map(spec);
            let spec_info = spec_data.info();
            let stage = fs.stage_create_info().specialization_info(&spec_info);
            // SAFETY: `spec_data` lives until the end of this function.
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(stage) }
        } else {
            fs.stage_create_info()
        };
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(vs_stage) },
            fs_stage,
        ];

        // Binding 1: sampled image (HDR texture), Binding 2: sampler, Binding 3: bloom texture, Binding 4: SSAO texture
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2).descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(4).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = device
            .descriptor_layout_cache()
            .get_or_create_simple(&bindings)
            .map_err(|e| RenderError::PipelineCreation(format!("tonemap desc_layout: {e}")))?;

        let set_layouts = [desc_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(80); // 8 floats + 3 vec4 = 32 + 48 = 80
        let push_ranges = [push_range];
        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &push_ranges)
            .map_err(|e| RenderError::PipelineCreation(format!("tonemap layout: {e}")))?;

        // No vertex input — positions generated from gl_VertexID
        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default();
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let cfs = [swapchain.format()];
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cfs);

        let ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia)
            .viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms)
            .depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy)
            .layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None)
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("tonemap pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no tonemap pipeline created".into()))?
        };

        tracing::info!("tone-mapping pipeline created");
        Ok(Self { pipeline, layout, desc_layout })
    }
}

/// Full-screen post-process stack pipeline.
/// Applies film grain, chromatic aberration, vignette, and color grading.
pub struct PostProcessPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl PostProcessPipeline {
    pub fn create(
        device: &GpuDevice,
        swapchain: &Swapchain,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(vs.stage_create_info()) },
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(fs.stage_create_info()) },
        ];

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2).descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = device
            .descriptor_layout_cache()
            .get_or_create_simple(&bindings)
            .map_err(|e| RenderError::PipelineCreation(format!("postprocess desc_layout: {e}")))?;

        let set_layouts = [desc_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(64); // 8 floats + 3 vec4 = 32 + 48 = 80, but padded to 64 for simplicity (we'll pass less)
        let push_ranges = [push_range];
        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &push_ranges)
            .map_err(|e| RenderError::PipelineCreation(format!("postprocess layout: {e}")))?;

        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default();
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let cfs = [swapchain.format()];
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cfs);

        let ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia)
            .viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms)
            .depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy)
            .layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None)
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("postprocess pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no postprocess pipeline created".into()))?
        };

        tracing::info!("post-process pipeline created");
        Ok(Self { pipeline, layout, desc_layout })
    }
}

/// Full-screen bloom pipeline (extract / downsample / upsample).
/// Uses a fullscreen triangle with push constants for texel size and threshold.
pub struct BloomPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl BloomPipeline {
    pub fn create(
        device: &GpuDevice,
        format: vk::Format,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(vs.stage_create_info()) },
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(fs.stage_create_info()) },
        ];

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2).descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = device
            .descriptor_layout_cache()
            .get_or_create_simple(&bindings)
            .map_err(|e| RenderError::PipelineCreation(format!("bloom desc_layout: {e}")))?;

        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(16); // vec4
        let set_layouts = [desc_layout];
        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &[push_range])
            .map_err(|e| RenderError::PipelineCreation(format!("bloom layout: {e}")))?;

        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default();
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let cfs = [format];
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cfs);

        let ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia)
            .viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms)
            .depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy)
            .layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None)
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("bloom pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no bloom pipeline created".into()))?
        };

        tracing::info!("bloom pipeline created");
        Ok(Self { pipeline, layout, desc_layout })
    }
}

/// Temporal Anti-Aliasing resolve pipeline.
/// Reads current frame + history + depth, writes resolved color with neighborhood clamping.
pub struct TaaPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl TaaPipeline {
    pub fn create(
        device: &GpuDevice,
        format: vk::Format,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(vs.stage_create_info()) },
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(fs.stage_create_info()) },
        ];

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2).descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(4).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = device
            .descriptor_layout_cache()
            .get_or_create_simple(&bindings)
            .map_err(|e| RenderError::PipelineCreation(format!("taa desc_layout: {e}")))?;

        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(144); // 2x mat4 + vec4
        let set_layouts = [desc_layout];
        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &[push_range])
            .map_err(|e| RenderError::PipelineCreation(format!("taa layout: {e}")))?;

        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default();
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let cfs = [format];
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cfs);

        let ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia)
            .viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms)
            .depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy)
            .layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None)
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("taa pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no taa pipeline created".into()))?
        };

        tracing::info!("taa pipeline created");
        Ok(Self { pipeline, layout, desc_layout })
    }
}

/// Screen-space reflection pipeline.
pub struct SsrPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl SsrPipeline {
    pub fn create(
        device: &GpuDevice,
        format: vk::Format,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(vs.stage_create_info()) },
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(fs.stage_create_info()) },
        ];
        let bindings = [
            vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default().binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default().binding(4).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = device.descriptor_layout_cache().get_or_create_simple(&bindings)
            .map_err(|e| RenderError::PipelineCreation(format!("ssr desc_layout: {e}")))?;
        let push_range = vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::FRAGMENT).offset(0).size(96);
        let set_layouts = [desc_layout];
        let layout = device.pipeline_layout_cache().get_or_create(&set_layouts, &[push_range])
            .map_err(|e| RenderError::PipelineCreation(format!("ssr layout: {e}")))?;
        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::CLOCKWISE).line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default();
        let ba = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);
        let cfs = [format];
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cfs);
        let ci = vk::GraphicsPipelineCreateInfo::default().stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia).viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms).depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy).layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1).push_next(&mut dr);
        let pipeline = unsafe { device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None).map_err(|(_, e)| RenderError::PipelineCreation(format!("ssr pipeline: {e}")))?.into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no ssr pipeline created".into()))? };
        tracing::info!("ssr pipeline created");
        Ok(Self { pipeline, layout, desc_layout })
    }
}

/// Volumetric fog fullscreen pass pipeline.
pub struct VolumetricFogPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl VolumetricFogPipeline {
    pub fn create(
        device: &GpuDevice,
        format: vk::Format,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(vs.stage_create_info()) },
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(fs.stage_create_info()) },
        ];
        let bindings = [
            vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default().binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = device.descriptor_layout_cache().get_or_create_simple(&bindings)
            .map_err(|e| RenderError::PipelineCreation(format!("fog desc_layout: {e}")))?;
        let push_range = vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::FRAGMENT).offset(0).size(96);
        let set_layouts = [desc_layout];
        let layout = device.pipeline_layout_cache().get_or_create(&set_layouts, &[push_range])
            .map_err(|e| RenderError::PipelineCreation(format!("fog layout: {e}")))?;
        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::CLOCKWISE).line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default();
        let ba = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);
        let cfs = [format];
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cfs);
        let ci = vk::GraphicsPipelineCreateInfo::default().stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia).viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms).depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy).layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1).push_next(&mut dr);
        let pipeline = unsafe { device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None).map_err(|(_, e)| RenderError::PipelineCreation(format!("fog pipeline: {e}")))?.into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no fog pipeline created".into()))? };
        tracing::info!("volumetric fog pipeline created");
        Ok(Self { pipeline, layout, desc_layout })
    }
}

/// Skybox / atmospheric scattering fullscreen pass pipeline.
pub struct SkyboxPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl SkyboxPipeline {
    pub fn create(
        device: &GpuDevice,
        format: vk::Format,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(vs.stage_create_info()) },
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(fs.stage_create_info()) },
        ];
        let bindings = [
            vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default().binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = device.descriptor_layout_cache().get_or_create_simple(&bindings)
            .map_err(|e| RenderError::PipelineCreation(format!("skybox desc_layout: {e}")))?;
        let push_range = vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::FRAGMENT).offset(0).size(96);
        let set_layouts = [desc_layout];
        let layout = device.pipeline_layout_cache().get_or_create(&set_layouts, &[push_range])
            .map_err(|e| RenderError::PipelineCreation(format!("skybox layout: {e}")))?;
        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::CLOCKWISE).line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default();
        let ba = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);
        let cfs = [format];
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cfs);
        let ci = vk::GraphicsPipelineCreateInfo::default().stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia).viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms).depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy).layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1).push_next(&mut dr);
        let pipeline = unsafe { device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None).map_err(|(_, e)| RenderError::PipelineCreation(format!("skybox pipeline: {e}")))?.into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no skybox pipeline created".into()))? };
        tracing::info!("skybox pipeline created");
        Ok(Self { pipeline, layout, desc_layout })
    }
}
