use ash::vk;

use crate::device::GpuDevice;
use crate::shader::ShaderModule;
use crate::swapchain::Swapchain;
use crate::RenderError;

pub const PUSH_CONSTANT_SIZE: u32 = 128; // Mat4(64) + dir_light(16) + dir_color(16) + base_color(16) + rough_metal(16)
pub const UBO_SCENE_SIZE: u64 = 432; // view_proj(64)+cam(16)+count(4)+pad(12)+8*PointLight(256)+fog(16)+light_view_proj(64)
pub const PUSH_CONSTANT_SIZE_2D: u32 = 64;

pub struct GraphicsPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl GraphicsPipeline {
    pub fn create(
        device: &GpuDevice,
        swapchain: &Swapchain,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let stages = [vs.stage_create_info(), fs.stage_create_info()];

        let ubo_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1).stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT);
        let shadow_tex_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT);
        let shadow_samp_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(2).descriptor_type(vk::DescriptorType::SAMPLER)
            .descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT);
        let bindings = [ubo_binding, shadow_tex_binding, shadow_samp_binding];
        let desc_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("desc_layout: {e}")))?
        };

        let set_layouts = [desc_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(PUSH_CONSTANT_SIZE);
        let push_ranges = [push_range];

        let layout = unsafe {
            device.logical().create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push_ranges), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("layout: {e}")))?
        };

        let stride = 24u32; // pos(12) + normal(12)
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
        let rs = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::CLOCKWISE).line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(true).depth_write_enable(true).depth_compare_op(vk::CompareOp::LESS);
        let ba = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let cfs = [swapchain.format()];
        let depth_fmt = vk::Format::D32_SFLOAT; // D32 is widely supported; fallback to D24 if needed
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cfs).depth_attachment_format(depth_fmt);

        let ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia)
            .viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms)
            .depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy)
            .layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None)
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no pipeline created".into()))?
        };

        tracing::info!("graphics pipeline created (UBO + push constants + depth + culling)");

        Ok(Self { pipeline, layout, descriptor_set_layout: desc_layout })
    }
}

pub struct ShadowPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl ShadowPipeline {
    pub fn create(
        device: &GpuDevice,
        vs: &ShaderModule,
    ) -> Result<Self, RenderError> {
        let stages = [vs.stage_create_info()];

        // Same descriptor set layout as main pipeline (UBO binding 0 with light_view_proj)
        let ubo_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1).stage_flags(vk::ShaderStageFlags::VERTEX);
        let bindings = [ubo_binding];
        let desc_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("shadow desc_layout: {e}")))?
        };

        let set_layouts = [desc_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size(PUSH_CONSTANT_SIZE);
        let push_ranges = [push_range];

        let layout = unsafe {
            device.logical().create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push_ranges), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("shadow layout: {e}")))?
        };

        let stride = 24u32; // pos(12) + normal(12)
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
            .depth_test_enable(true).depth_write_enable(true).depth_compare_op(vk::CompareOp::LESS);
        let cb = vk::PipelineColorBlendStateCreateInfo::default();
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let depth_fmt = vk::Format::D32_SFLOAT;
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().depth_attachment_format(depth_fmt);

        let ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia)
            .viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms)
            .depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy)
            .layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None)
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("shadow pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no shadow pipeline created".into()))?
        };

        tracing::info!("shadow depth pipeline created");

        Ok(Self { pipeline, layout, descriptor_set_layout: desc_layout })
    }
}

pub struct GraphicsPipeline2D {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
    pub desc_pool: vk::DescriptorPool,
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
        let desc_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("2d desc_layout: {e}")))?
        };

        // Descriptor pool: 1 UBO + 1 sampled image + 1 sampler
        let pool_sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 1 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLED_IMAGE, descriptor_count: 1 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLER, descriptor_count: 1 },
        ];
        let desc_pool = unsafe {
            device.logical().create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default().pool_sizes(&pool_sizes).max_sets(1), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("2d desc pool: {e}")))?
        };

        let set_layouts = [desc_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size(PUSH_CONSTANT_SIZE_2D);
        let push_ranges = [push_range];

        let layout = unsafe {
            device.logical().create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push_ranges), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("2d layout: {e}")))?
        };

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

        Ok(Self { pipeline, layout, desc_layout, desc_pool })
    }
}
