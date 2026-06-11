use ash::vk;
use crate::device::GpuDevice;
use crate::shader::ShaderModule;
use crate::swapchain::Swapchain;
use crate::RenderError;
use super::PUSH_CONSTANT_SIZE;

/// Particle billboard pipeline with additive blending and no depth write.
#[derive(Clone)]
pub struct ParticlePipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl ParticlePipeline {
    pub fn create(
        device: &GpuDevice,
        swapchain: &Swapchain,
        vs: &ShaderModule,
        fs: &ShaderModule,
        bindless_layout: vk::DescriptorSetLayout,
    ) -> Result<Self, RenderError> {
        let vs_stage = vs.stage_create_info();
        let fs_stage = fs.stage_create_info();
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(vs_stage) },
            fs_stage,
        ];

        let set_layouts = [bindless_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(PUSH_CONSTANT_SIZE);
        let push_ranges = [push_range];

        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &push_ranges)
            .map_err(|e| RenderError::PipelineCreation(format!("particle layout: {e}")))?;

        // Binding 0: dummy vertex position (not used, but needed for pipeline)
        // Binding 1: per-instance data (position + size, color)
        let vb0 = vk::VertexInputBindingDescription::default()
            .binding(0).stride(12).input_rate(vk::VertexInputRate::VERTEX);
        let vb1 = vk::VertexInputBindingDescription::default()
            .binding(1).stride(64).input_rate(vk::VertexInputRate::INSTANCE);
        let va = [
            vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(1).location(1).format(vk::Format::R32G32B32A32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(1).location(2).format(vk::Format::R32G32B32A32_SFLOAT).offset(16),
        ];
        let vbs = [vb0, vb1];
        let vi = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vbs)
            .vertex_attribute_descriptions(&va);
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_STRIP);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(false)
            .depth_compare_op(vk::CompareOp::LESS);
        // Additive blending for glow/particle look
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE)
            .alpha_blend_op(vk::BlendOp::ADD)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let cfs = [swapchain.format()];
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
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("particle pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no particle pipeline created".into()))?
        };

        tracing::info!("particle pipeline created");

        Ok(Self { pipeline, layout, descriptor_set_layout: bindless_layout })
    }
}
