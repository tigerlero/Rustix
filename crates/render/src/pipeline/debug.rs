//! Debug render mode pipelines (overdraw, light complexity).

use ash::vk;
use crate::shader::ShaderModule;
use crate::pipeline::RenderError;
use crate::swapchain::Swapchain;
use crate::device::GpuDevice;

/// Pipeline for overdraw heatmap visualization.
/// Uses additive blending so each overlapping fragment brightens the output.
pub struct DebugOverdrawPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl DebugOverdrawPipeline {
    pub fn create(
        device: &GpuDevice,
        swapchain: &Swapchain,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let desc_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&desc_bindings),
                None,
            )
        }.map_err(|e| RenderError::DeviceCreation(format!("debug overdraw desc layout: {e}")))?;

        let push_range = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(16)];
        let layout = unsafe {
            device.logical().create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&[desc_layout])
                    .push_constant_ranges(&push_range),
                None,
            )
        }.map_err(|e| RenderError::DeviceCreation(format!("debug overdraw layout: {e}")))?;

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vs.module)
                .name(c"main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fs.module)
                .name(c"main"),
        ];

        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vp = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(false)
            .depth_write_enable(false);
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE)
            .alpha_blend_op(vk::BlendOp::ADD)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let format = swapchain.format();
        let format_arr = [format];
        let ca = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE)
            .alpha_blend_op(vk::BlendOp::ADD)];
        let mut ca_info = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ca);
        let mut render_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&format_arr);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vi)
            .input_assembly_state(&ia)
            .viewport_state(&vp)
            .rasterization_state(&rs)
            .multisample_state(&ms)
            .depth_stencil_state(&ds)
            .color_blend_state(&mut ca_info)
            .dynamic_state(&dy)
            .layout(layout)
            .push_next(&mut render_info);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[create_info],
                None,
            )
        }
        .map_err(|e| RenderError::DeviceCreation(format!("debug overdraw pipeline: {:?}", e.1)))?[0];

        Ok(Self {
            pipeline,
            layout,
            desc_layout,
        })
    }
}

/// Pipeline for light complexity heatmap visualization.
/// Fullscreen pass that visualizes per-tile light counts from Forward+.
pub struct DebugLightComplexityPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
}

impl DebugLightComplexityPipeline {
    pub fn create(
        device: &GpuDevice,
        swapchain: &Swapchain,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let desc_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&desc_bindings),
                None,
            )
        }.map_err(|e| RenderError::DeviceCreation(format!("debug light complexity desc layout: {e}")))?;

        let push_range = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(16)];
        let layout = unsafe {
            device.logical().create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&[desc_layout])
                    .push_constant_ranges(&push_range),
                None,
            )
        }.map_err(|e| RenderError::DeviceCreation(format!("debug light complexity layout: {e}")))?;

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vs.module)
                .name(c"main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fs.module)
                .name(c"main"),
        ];

        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vp = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(false)
            .depth_write_enable(false);
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let format = swapchain.format();
        let format_arr = [format];
        let mut render_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&format_arr);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vi)
            .input_assembly_state(&ia)
            .viewport_state(&vp)
            .rasterization_state(&rs)
            .multisample_state(&ms)
            .depth_stencil_state(&ds)
            .color_blend_state(&cb)
            .dynamic_state(&dy)
            .layout(layout)
            .push_next(&mut render_info);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[create_info],
                None,
            )
        }
        .map_err(|e| RenderError::DeviceCreation(format!("debug light complexity pipeline: {:?}", e.1)))?[0];

        Ok(Self {
            pipeline,
            layout,
            desc_layout,
        })
    }
}
