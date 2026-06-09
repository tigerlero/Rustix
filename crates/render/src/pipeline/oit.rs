use ash::vk;
use crate::device::GpuDevice;
use crate::shader::ShaderModule;
use crate::RenderError;
use super::PUSH_CONSTANT_SIZE;

/// Weighted blended OIT accumulation pipeline.
/// Renders transparent geometry into two color attachments:
///   RT0: accumulation (RGBA16F) with blend ONE, ONE
///   RT1: revealage (R16F) with blend ZERO, ONE_MINUS_SRC_COLOR
/// Depth test is read-only (LESS_EQUAL, no write).
pub struct OitAccumulatePipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl OitAccumulatePipeline {
    pub fn create(
        device: &GpuDevice,
        vs: &ShaderModule,
        fs: &ShaderModule,
        bindless_layout: vk::DescriptorSetLayout,
    ) -> Result<Self, RenderError> {
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(vs.stage_create_info()) },
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(fs.stage_create_info()) },
        ];

        let set_layouts = [bindless_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(PUSH_CONSTANT_SIZE);
        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &[push_range])
            .map_err(|e| RenderError::PipelineCreation(format!("oit accumulate layout: {e}")))?;

        let stride = 24u32;
        let vb = vk::VertexInputBindingDescription::default().binding(0).stride(stride).input_rate(vk::VertexInputRate::VERTEX);
        let va = [
            vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(0).location(1).format(vk::Format::R32G32B32_SFLOAT).offset(12),
        ];
        let vbs = [vb];
        let vi = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&vbs).vertex_attribute_descriptions(&va);
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(false)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);

        // RT0: accumulation - additive blend for weighted colors
        let ba0 = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE)
            .alpha_blend_op(vk::BlendOp::ADD);
        // RT1: revealage - accumulate alpha coverage
        let ba1 = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::R)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ZERO)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_COLOR)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD);
        let ba = [ba0, ba1];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let cfs = [vk::Format::R16G16B16A16_SFLOAT, vk::Format::R16_SFLOAT];
        let depth_fmt = vk::Format::D32_SFLOAT;
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default()
            .color_attachment_formats(&cfs)
            .depth_attachment_format(depth_fmt);

        let ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia)
            .viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms)
            .depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy)
            .layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None)
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("oit accumulate pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no oit accumulate pipeline created".into()))?
        };

        tracing::info!("oit accumulate pipeline created");
        Ok(Self { pipeline, layout, descriptor_set_layout: bindless_layout })
    }
}

/// Weighted blended OIT composite pipeline.
/// Fullscreen pass that reads accumulation + revealage + opaque HDR,
/// blends transparent layer over opaque, and writes to a new HDR target.
pub struct OitCompositePipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl OitCompositePipeline {
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
            vk::DescriptorSetLayoutBinding::default().binding(4).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default().binding(5).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default().binding(6).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = device.descriptor_layout_cache().get_or_create_simple(&bindings)
            .map_err(|e| RenderError::PipelineCreation(format!("oit composite desc_layout: {e}")))?;
        let push_range = vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::FRAGMENT).offset(0).size(16);
        let set_layouts = [desc_layout];
        let layout = device.pipeline_layout_cache().get_or_create(&set_layouts, &[push_range])
            .map_err(|e| RenderError::PipelineCreation(format!("oit composite layout: {e}")))?;

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

        let ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia)
            .viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms)
            .depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy)
            .layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None)
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("oit composite pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no oit composite pipeline created".into()))?
        };

        tracing::info!("oit composite pipeline created");
        Ok(Self { pipeline, layout, desc_layout })
    }
}
