use ash::vk;
use parking_lot::Mutex;
use std::collections::HashMap;
use crate::device::GpuDevice;
use crate::shader::ShaderModule;
use crate::swapchain::Swapchain;
use crate::RenderError;
use super::{PUSH_CONSTANT_SIZE, DEFERRED_PUSH_CONSTANT_SIZE, shadow_push_constant_range, shadow_vertex_input_state, shadow_depth_stencil_state, PipelineVariantKey};

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
        bindless_layout: vk::DescriptorSetLayout,
        variant: &PipelineVariantKey,
    ) -> Result<Self, RenderError> {
        let vs_stage = vs.stage_create_info();
        // Build spec constant data unconditionally so it outlives the transmuted stage.
        let spec_data = crate::spec_constants::SpecConstantData::from_map(&variant.spec_constants);
        let spec_info = spec_data.info();
        let fs_stage = if variant.spec_constants.is_empty() {
            fs.stage_create_info()
        } else {
            let stage = fs.stage_create_info().specialization_info(&spec_info);
            // SAFETY: `spec_data` and `spec_info` live until the end of this
            // function, so all pointers remain valid for the synchronous
            // `create_graphics_pipelines` call.
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(stage) }
        };
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
            .map_err(|e| RenderError::PipelineCreation(format!("layout: {e}")))?;

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
            .polygon_mode(variant.polygon_mode)
            .cull_mode(variant.cull_mode)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(variant.quality_level.msaa_samples());
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(variant.depth_test)
            .depth_write_enable(variant.depth_write)
            .depth_compare_op(vk::CompareOp::LESS);
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(variant.blend_enable)];
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
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no pipeline created".into()))?
        };

        tracing::info!("graphics pipeline created (variant={:?}, msaa={:?})", variant.render_path, variant.quality_level.msaa_samples());

        Ok(Self { pipeline, layout, descriptor_set_layout: bindless_layout })
    }
}

/// Forward scene pipeline with instanced rendering support.
/// Same bindless layout and push constants as GraphicsPipeline, but adds
/// a second vertex binding (rate=PER_INSTANCE) for per-instance transforms,
/// base color, and material params.
#[derive(Clone)]
pub struct InstancedGraphicsPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl InstancedGraphicsPipeline {
    pub fn create(
        device: &GpuDevice,
        swapchain: &Swapchain,
        vs: &ShaderModule,
        fs: &ShaderModule,
        bindless_layout: vk::DescriptorSetLayout,
        variant: &PipelineVariantKey,
    ) -> Result<Self, RenderError> {
        let vs_stage = vs.stage_create_info();
        let spec_data = crate::spec_constants::SpecConstantData::from_map(&variant.spec_constants);
        let spec_info = spec_data.info();
        let fs_stage = if variant.spec_constants.is_empty() {
            fs.stage_create_info()
        } else {
            let stage = fs.stage_create_info().specialization_info(&spec_info);
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(stage) }
        };
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
            .map_err(|e| RenderError::PipelineCreation(format!("instanced layout: {e}")))?;

        let vb0 = vk::VertexInputBindingDescription::default()
            .binding(0).stride(24).input_rate(vk::VertexInputRate::VERTEX);
        let vb1 = vk::VertexInputBindingDescription::default()
            .binding(1).stride(96).input_rate(vk::VertexInputRate::INSTANCE);
        let va = [
            vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(0).location(1).format(vk::Format::R32G32B32_SFLOAT).offset(12),
            vk::VertexInputAttributeDescription::default().binding(1).location(2).format(vk::Format::R32G32B32A32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(1).location(3).format(vk::Format::R32G32B32A32_SFLOAT).offset(16),
            vk::VertexInputAttributeDescription::default().binding(1).location(4).format(vk::Format::R32G32B32A32_SFLOAT).offset(32),
            vk::VertexInputAttributeDescription::default().binding(1).location(5).format(vk::Format::R32G32B32A32_SFLOAT).offset(48),
            vk::VertexInputAttributeDescription::default().binding(1).location(6).format(vk::Format::R32G32B32A32_SFLOAT).offset(64),
            vk::VertexInputAttributeDescription::default().binding(1).location(7).format(vk::Format::R32G32B32A32_SFLOAT).offset(80),
        ];
        let vbs = [vb0, vb1];
        let vi = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vbs)
            .vertex_attribute_descriptions(&va);
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(variant.polygon_mode)
            .cull_mode(variant.cull_mode)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(variant.quality_level.msaa_samples());
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(variant.depth_test)
            .depth_write_enable(variant.depth_write)
            .depth_compare_op(vk::CompareOp::LESS);
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(variant.blend_enable)];
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
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("instanced pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no instanced pipeline created".into()))?
        };

        tracing::info!("instanced graphics pipeline created (variant={:?}, msaa={:?})", variant.render_path, variant.quality_level.msaa_samples());

        Ok(Self { pipeline, layout, descriptor_set_layout: bindless_layout })
    }
}

/// Forward scene pipeline using VK_NV_mesh_shader.
/// No vertex input state — the mesh shader generates geometry directly
/// from instance data in an SSBO and outputs the same varyings as the
/// instanced vertex shader. Falls back to InstancedGraphicsPipeline when
/// mesh shaders are not available.
#[derive(Clone)]
pub struct MeshShaderPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl MeshShaderPipeline {
    pub fn create(
        device: &GpuDevice,
        swapchain: &crate::swapchain::Swapchain,
        mesh: &crate::shader::ShaderModule,
        fs: &crate::shader::ShaderModule,
        bindless_layout: vk::DescriptorSetLayout,
        variant: &PipelineVariantKey,
    ) -> Result<Self, RenderError> {
        let mesh_stage = mesh.stage_create_info();
        let spec_data = crate::spec_constants::SpecConstantData::from_map(&variant.spec_constants);
        let spec_info = spec_data.info();
        let fs_stage = if variant.spec_constants.is_empty() {
            fs.stage_create_info()
        } else {
            let stage = fs.stage_create_info().specialization_info(&spec_info);
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(stage) }
        };
        let stages = [
            unsafe { std::mem::transmute::<_, vk::PipelineShaderStageCreateInfo<'static>>(mesh_stage) },
            fs_stage,
        ];

        let set_layouts = [bindless_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::MESH_NV | vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(PUSH_CONSTANT_SIZE);
        let push_ranges = [push_range];

        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &push_ranges)
            .map_err(|e| RenderError::PipelineCreation(format!("mesh layout: {e}")))?;

        // Mesh shaders have no vertex input state
        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(variant.polygon_mode)
            .cull_mode(variant.cull_mode)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(variant.quality_level.msaa_samples());
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(variant.depth_test)
            .depth_write_enable(variant.depth_write)
            .depth_compare_op(vk::CompareOp::LESS);
        let ba = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(variant.blend_enable)];
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
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("mesh pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no mesh pipeline created".into()))?
        };

        tracing::info!("mesh shader pipeline created (variant={:?}, msaa={:?})", variant.render_path, variant.quality_level.msaa_samples());

        Ok(Self { pipeline, layout, descriptor_set_layout: bindless_layout })
    }
}

/// G-buffer geometry pass pipeline with instanced rendering support.
#[derive(Clone)]
pub struct InstancedGBufferPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl InstancedGBufferPipeline {
    pub fn create(
        device: &GpuDevice,
        vs: &ShaderModule,
        fs: &ShaderModule,
        bindless_layout: vk::DescriptorSetLayout,
    ) -> Result<Self, RenderError> {
        let stages = [
            vs.stage_create_info(),
            fs.stage_create_info(),
        ];

        let set_layouts = [bindless_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(PUSH_CONSTANT_SIZE);
        let push_ranges = [push_range];

        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &push_ranges)
            .map_err(|e| RenderError::PipelineCreation(format!("instanced gbuffer layout: {e}")))?;

        let vb0 = vk::VertexInputBindingDescription::default()
            .binding(0).stride(24).input_rate(vk::VertexInputRate::VERTEX);
        let vb1 = vk::VertexInputBindingDescription::default()
            .binding(1).stride(96).input_rate(vk::VertexInputRate::INSTANCE);
        let va = [
            vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(0).location(1).format(vk::Format::R32G32B32_SFLOAT).offset(12),
            vk::VertexInputAttributeDescription::default().binding(1).location(2).format(vk::Format::R32G32B32A32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().binding(1).location(3).format(vk::Format::R32G32B32A32_SFLOAT).offset(16),
            vk::VertexInputAttributeDescription::default().binding(1).location(4).format(vk::Format::R32G32B32A32_SFLOAT).offset(32),
            vk::VertexInputAttributeDescription::default().binding(1).location(5).format(vk::Format::R32G32B32A32_SFLOAT).offset(48),
            vk::VertexInputAttributeDescription::default().binding(1).location(6).format(vk::Format::R32G32B32A32_SFLOAT).offset(64),
            vk::VertexInputAttributeDescription::default().binding(1).location(7).format(vk::Format::R32G32B32A32_SFLOAT).offset(80),
        ];
        let vbs = [vb0, vb1];
        let vi = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vbs)
            .vertex_attribute_descriptions(&va);
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vps = [vk::Viewport::default()]; let scs = [vk::Rect2D::default()];
        let vp = vk::PipelineViewportStateCreateInfo::default().viewports(&vps).scissors(&scs);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS);
        let ba = [
            vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false),
            vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false),
            vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false),
        ];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let cfs = [
            vk::Format::R8G8B8A8_UNORM,
            vk::Format::R16G16B16A16_SFLOAT,
            vk::Format::R8G8B8A8_UNORM,
        ];
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
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("instanced gbuffer pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no instanced gbuffer pipeline created".into()))?
        };

        tracing::info!("instanced gbuffer pipeline created (bindless)");

        Ok(Self { pipeline, layout, descriptor_set_layout: bindless_layout })
    }
}
#[derive(Clone)]
pub struct ShadowPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl ShadowPipeline {
    pub fn create(
        device: &GpuDevice,
        vs: &ShaderModule,
        bindless_layout: vk::DescriptorSetLayout,
    ) -> Result<Self, RenderError> {
        let stages = [vs.stage_create_info()];

        let set_layouts = [bindless_layout];
        let push_range = shadow_push_constant_range();
        let push_ranges = [push_range];

        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &push_ranges)
            .map_err(|e| RenderError::PipelineCreation(format!("shadow layout: {e}")))?;

        let (vbs, va) = shadow_vertex_input_state();
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
        let ds = shadow_depth_stencil_state();
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

        tracing::info!("shadow depth pipeline created (bindless)");

        Ok(Self { pipeline, layout, descriptor_set_layout: bindless_layout })
    }
}


/// G-buffer geometry pass pipeline.
///
/// Writes albedo+metallic, world normal, and roughness/AO/emissive into
/// three color attachments plus depth. Uses the same bindless descriptor
/// set layout and push constants as the forward scene pipeline.
pub struct GBufferPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl GBufferPipeline {
    pub fn create(
        device: &GpuDevice,
        vs: &ShaderModule,
        fs: &ShaderModule,
        bindless_layout: vk::DescriptorSetLayout,
    ) -> Result<Self, RenderError> {
        let stages = [
            vs.stage_create_info(),
            fs.stage_create_info(),
        ];

        let set_layouts = [bindless_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(PUSH_CONSTANT_SIZE);
        let push_ranges = [push_range];

        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &push_ranges)
            .map_err(|e| RenderError::PipelineCreation(format!("gbuffer layout: {e}")))?;

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
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let ds = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS);
        // 3 color attachments, no blending
        let ba = [
            vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false),
            vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false),
            vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA).blend_enable(false),
        ];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&ba);
        let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);

        let cfs = [
            vk::Format::R8G8B8A8_UNORM,   // albedo + metallic
            vk::Format::R16G16B16A16_SFLOAT, // normal
            vk::Format::R8G8B8A8_UNORM,   // roughness + AO + emissive
        ];
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
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("gbuffer pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no gbuffer pipeline created".into()))?
        };

        tracing::info!("GBuffer pipeline created");
        Ok(Self { pipeline, layout, descriptor_set_layout: bindless_layout })
    }
}

/// Deferred lighting pass pipeline.
///
/// Full-screen triangle that samples the G-buffer textures and computes
/// lighting (directional + Forward+ point lights). Outputs HDR color.
pub struct DeferredLightingPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
}

impl DeferredLightingPipeline {
    pub fn create(
        device: &GpuDevice,
        vs: &ShaderModule,
        fs: &ShaderModule,
        bindless_layout: vk::DescriptorSetLayout,
    ) -> Result<Self, RenderError> {
        let stages = [
            vs.stage_create_info(),
            fs.stage_create_info(),
        ];

        let set_layouts = [bindless_layout];
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(DEFERRED_PUSH_CONSTANT_SIZE);
        let push_ranges = [push_range];

        let layout = device.pipeline_layout_cache()
            .get_or_create(&set_layouts, &push_ranges)
            .map_err(|e| RenderError::PipelineCreation(format!("deferred layout: {e}")))?;

        // No vertex input — fullscreen triangle from gl_VertexIndex
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

        let cfs = [vk::Format::R16G16B16A16_SFLOAT];
        let mut dr = vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&cfs);

        let ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages).vertex_input_state(&vi).input_assembly_state(&ia)
            .viewport_state(&vp).rasterization_state(&rs).multisample_state(&ms)
            .depth_stencil_state(&ds).color_blend_state(&cb).dynamic_state(&dy)
            .layout(layout).base_pipeline_handle(vk::Pipeline::null()).base_pipeline_index(-1)
            .push_next(&mut dr);

        let pipeline = unsafe {
            device.logical().create_graphics_pipelines(device.pipeline_cache(), &[ci], None)
                .map_err(|(_, e)| RenderError::PipelineCreation(format!("deferred pipeline: {e}")))?
                .into_iter().next().ok_or_else(|| RenderError::PipelineCreation("no deferred pipeline created".into()))?
        };

        tracing::info!("deferred lighting pipeline created");
        Ok(Self { pipeline, layout, descriptor_set_layout: bindless_layout })
    }
}

/// Caches graphics pipelines by variant key.
///
/// Each variant gets its own `GraphicsPipeline` with potentially different
/// rasterization state, MSAA, depth/blend settings, etc. All variants share
/// the same bindless descriptor set layout.
pub struct GraphicsPipelineVariantCache {
    device: *const ash::Device,
    bindless_layout: vk::DescriptorSetLayout,
    cache: Mutex<HashMap<PipelineVariantKey, GraphicsPipeline>>,
}

unsafe impl Send for GraphicsPipelineVariantCache {}
unsafe impl Sync for GraphicsPipelineVariantCache {}

impl GraphicsPipelineVariantCache {
    pub fn new(device: &ash::Device, bindless_layout: vk::DescriptorSetLayout) -> Self {
        Self {
            device: device as *const ash::Device,
            bindless_layout,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a graphics pipeline for the given variant key.
    pub fn get_or_create(
        &self,
        key: &PipelineVariantKey,
        device: &GpuDevice,
        swapchain: &Swapchain,
        vs: &ShaderModule,
        fs: &ShaderModule,
    ) -> Result<vk::Pipeline, RenderError> {
        {
            let cache = self.cache.lock();
            if let Some(entry) = cache.get(key) {
                return Ok(entry.pipeline);
            }
        }

        let gp = GraphicsPipeline::create(
            device,
            swapchain,
            vs,
            fs,
            self.bindless_layout,
            key,
        )?;
        let pipeline = gp.pipeline;

        let mut cache = self.cache.lock();
        if let Some(entry) = cache.get(key) {
            return Ok(entry.pipeline);
        }

        cache.insert(key.clone(), gp);
        Ok(pipeline)
    }

    /// Look up an existing pipeline without creating one.
    pub fn get(&self, key: &PipelineVariantKey) -> Option<vk::Pipeline> {
        self.cache.lock().get(key).map(|e| e.pipeline)
    }

    /// Retrieve the full `GraphicsPipeline` for a variant.
    pub fn get_pipeline(&self, key: &PipelineVariantKey) -> Option<GraphicsPipeline> {
        // Return a copy with the shared layout handle so callers can use it
        // for binding.  Note: `GraphicsPipeline` does not implement `Clone`,
        // so we return a newly constructed struct with the same handles.
        self.cache.lock().get(key).map(|e| GraphicsPipeline {
            pipeline: e.pipeline,
            layout: e.layout,
            descriptor_set_layout: e.descriptor_set_layout,
        })
    }

    pub fn len(&self) -> usize {
        self.cache.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.lock().is_empty()
    }

    /// Destroy all cached pipelines and clear the cache. Used for shader hot-reload.
    pub fn clear(&self) {
        unsafe {
            if !self.device.is_null() {
                let dev = &*self.device;
                let mut cache = self.cache.lock();
                for entry in cache.values() {
                    if entry.pipeline != vk::Pipeline::null() {
                        dev.destroy_pipeline(entry.pipeline, None);
                    }
                    // Pipeline layouts are owned by PipelineLayoutCache; do not destroy here.
                }
                cache.clear();
            }
        }
    }
}

impl Drop for GraphicsPipelineVariantCache {
    fn drop(&mut self) {
        unsafe {
            if !self.device.is_null() {
                let dev = &*self.device;
                let cache = self.cache.get_mut();
                for entry in cache.values() {
                    if entry.pipeline != vk::Pipeline::null() {
                        dev.destroy_pipeline(entry.pipeline, None);
                    }
                    // Pipeline layouts are owned by PipelineLayoutCache; do not destroy here.
                }
            }
        }
    }
}
