use ash::vk;
use crate::device::GpuDevice;
use crate::shader::ShaderModule;
use crate::swapchain::Swapchain;
use crate::RenderError;
use super::PUSH_CONSTANT_SIZE_2D;

pub struct GraphicsPipeline2D {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl GraphicsPipeline2D {
    pub fn create(
        device: &GpuDevice,
        swapchain: &Swapchain,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let stages = [vs.stage_create_info(), fs.stage_create_info()];

        // Binding 0: UBO (view-proj), Binding 1: sampled image, Binding 2: sampler
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1).stage_flags(vk::ShaderStageFlags::VERTEX),
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
            .map_err(|e| RenderError::PipelineCreation(format!("2d desc_layout: {e}")))?;

        let set_layouts = [desc_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size(PUSH_CONSTANT_SIZE_2D);
        let push_ranges = [push_range];

        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &push_ranges)
            .map_err(|e| RenderError::PipelineCreation(format!("2d layout: {e}")))?;

        // Vertex: position(vec2) + uv(vec2) + color(vec4) = 32 bytes
        let stride = 32u32;
        let vb = vk::VertexInputBindingDescription::default().binding(0).stride(stride).input_rate(vk::VertexInputRate::VERTEX);
        let va = [
            vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(0).location(1).format(vk::Format::R32G32_SFLOAT).offset(8),
            vk::VertexInputAttributeDescription::default().binding(0).location(2).format(vk::Format::R32G32B32A32_SFLOAT).offset(16),
        ];
        let vbs = [vb];
        let vi = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&vbs).vertex_attribute_descriptions(&va);
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        // No depth
        let ds = vk::PipelineDepthStencilStateCreateInfo::default();
        // Alpha blending
        let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA);
        let ba = [blend_attachment];
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
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("2d pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no 2d pipeline created".into()))?
        };

        tracing::info!("2D graphics pipeline created (UBO + sampler + alpha blend)");

        Ok(Self { pipeline, layout, desc_layout })
    }
}
