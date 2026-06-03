use ash::vk;
use parking_lot::Mutex;
use std::collections::HashMap;

use crate::device::GpuDevice;
use crate::shader::ShaderModule;
use crate::swapchain::Swapchain;
use crate::RenderError;

pub const PUSH_CONSTANT_SIZE: u32 = 128; // Mat4(64) + dir_light(16) + dir_color(16) + base_color(16) + rough_metal(16)
pub const UBO_SCENE_SIZE: u64 = 432; // view_proj(64)+cam(16)+count(4)+pad(12)+8*PointLight(256)+fog(16)+light_view_proj(64)
pub const PUSH_CONSTANT_SIZE_2D: u32 = 64;

/// Pure function returning the descriptor set layout binding for the shadow pass UBO.
pub fn shadow_descriptor_set_bindings() -> [vk::DescriptorSetLayoutBinding<'static>; 1] {
    [vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::VERTEX)]
}

/// Pure function returning the descriptor set layout bindings for the main graphics pass.
/// UBO (binding 0), shadow sampled image (binding 1), shadow sampler (binding 2).
pub fn main_descriptor_set_bindings() -> [vk::DescriptorSetLayoutBinding<'static>; 3] {
    [
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
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
    ]
}

/// Pure function returning the push constant range for the shadow pass.
pub fn shadow_push_constant_range() -> vk::PushConstantRange {
    vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size(PUSH_CONSTANT_SIZE)
}

/// Pure function returning the vertex input configuration for the shadow pass.
/// Position (location 0) and normal (location 1), stride 24 bytes.
pub fn shadow_vertex_input_state() -> (
    [vk::VertexInputBindingDescription; 1],
    [vk::VertexInputAttributeDescription; 2],
) {
    let vb = [vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(24)
        .input_rate(vk::VertexInputRate::VERTEX)];
    let va = [
        vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(0),
        vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(12),
    ];
    (vb, va)
}

/// Pure function returning the depth-stencil state for the shadow pass.
pub fn shadow_depth_stencil_state() -> vk::PipelineDepthStencilStateCreateInfo<'static> {
    vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(true)
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS)
}

/// Rendering path strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderPath {
    Forward,
    /// Deferred rendering (G-buffer + lighting pass).
    /// Currently falls back to forward; full G-buffer support is planned.
    Deferred,
}

/// Quality preset affecting MSAA, shadow resolution, and effect quality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QualityLevel {
    Low,
    Medium,
    High,
    Ultra,
}

impl QualityLevel {
    /// MSAA sample count for this quality level.
    pub fn msaa_samples(&self) -> vk::SampleCountFlags {
        match self {
            QualityLevel::Low => vk::SampleCountFlags::TYPE_1,
            QualityLevel::Medium => vk::SampleCountFlags::TYPE_2,
            QualityLevel::High => vk::SampleCountFlags::TYPE_4,
            QualityLevel::Ultra => vk::SampleCountFlags::TYPE_8,
        }
    }
}

/// Key identifying a specific pipeline variant.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PipelineVariantKey {
    pub render_path: RenderPath,
    pub quality_level: QualityLevel,
    pub cull_mode: vk::CullModeFlags,
    pub polygon_mode: vk::PolygonMode,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend_enable: bool,
}

impl Default for PipelineVariantKey {
    fn default() -> Self {
        Self {
            render_path: RenderPath::Forward,
            // Low quality (1x MSAA) is the safe default until the engine
            // creates multisample render targets for higher quality levels.
            quality_level: QualityLevel::Low,
            cull_mode: vk::CullModeFlags::NONE,
            polygon_mode: vk::PolygonMode::FILL,
            depth_test: true,
            depth_write: true,
            blend_enable: false,
        }
    }
}

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
        let stages = [vs.stage_create_info(), fs.stage_create_info()];

        let set_layouts = [bindless_layout];
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

        let layout = unsafe {
            device.logical().create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push_ranges), None,
            ).map_err(|e| RenderError::PipelineCreation(format!("shadow layout: {e}")))?
        };

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
        let desc_layout = device
            .descriptor_layout_cache()
            .get_or_create_simple(&bindings)
            .map_err(|e| RenderError::PipelineCreation(format!("2d desc_layout: {e}")))?;

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

/// Compute pipeline wrapper.
pub struct ComputePipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
}

/// Hashable key for the compute pipeline cache.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ComputePipelineKey {
    shader: vk::ShaderModule,
    set_layouts: Vec<vk::DescriptorSetLayout>,
    push_ranges: Vec<PushConstantKey>,
}

/// Hashable wrapper for `vk::PushConstantRange` fields.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PushConstantKey {
    stage_flags: vk::ShaderStageFlags,
    offset: u32,
    size: u32,
}

impl From<vk::PushConstantRange> for PushConstantKey {
    fn from(r: vk::PushConstantRange) -> Self {
        Self {
            stage_flags: r.stage_flags,
            offset: r.offset,
            size: r.size,
        }
    }
}

/// Caches compute pipelines keyed by shader module + layout configuration.
///
/// Use `get_or_create` to obtain a compute pipeline. The cache owns the
/// Vulkan pipeline and layout handles and destroys them on drop.
pub struct ComputePipelineCache {
    device: *const ash::Device,
    cache: Mutex<HashMap<ComputePipelineKey, ComputePipeline>>,
}

unsafe impl Send for ComputePipelineCache {}
unsafe impl Sync for ComputePipelineCache {}

impl ComputePipelineCache {
    pub fn new(device: &ash::Device) -> Self {
        Self {
            device: device as *const ash::Device,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a compute pipeline for the given shader and layout config.
    ///
    /// `set_layouts` should be borrowed from the descriptor set layout cache
    /// (e.g. `device.descriptor_layout_cache()`). `push_ranges` are the push
    /// constant ranges for the pipeline layout.
    pub fn get_or_create(
        &self,
        shader: &ShaderModule,
        set_layouts: &[vk::DescriptorSetLayout],
        push_ranges: &[vk::PushConstantRange],
    ) -> Result<vk::Pipeline, RenderError> {
        let key = ComputePipelineKey {
            shader: shader.module,
            set_layouts: set_layouts.to_vec(),
            push_ranges: push_ranges.iter().copied().map(PushConstantKey::from).collect(),
        };

        {
            let cache = self.cache.lock();
            if let Some(entry) = cache.get(&key) {
                return Ok(entry.pipeline);
            }
        }

        let (pipeline, layout) = unsafe {
            let layout = (*self.device)
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(set_layouts)
                        .push_constant_ranges(push_ranges),
                    None,
                )
                .map_err(|e| RenderError::PipelineCreation(format!("compute layout: {e}")))?;

            let stage = shader.stage_create_info();
            let ci = vk::ComputePipelineCreateInfo::default()
                .stage(stage)
                .layout(layout);

            let pipeline = (*self.device)
                .create_compute_pipelines(vk::PipelineCache::null(), &[ci], None)
                .map_err(|(_, e)| {
                    (*self.device).destroy_pipeline_layout(layout, None);
                    RenderError::PipelineCreation(format!("compute pipeline: {e}"))
                })?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    (*self.device).destroy_pipeline_layout(layout, None);
                    RenderError::PipelineCreation("no compute pipeline created".into())
                })?;

            (pipeline, layout)
        };

        let mut cache = self.cache.lock();
        // Another thread may have raced us.
        if let Some(entry) = cache.get(&key) {
            unsafe {
                (*self.device).destroy_pipeline(pipeline, None);
                (*self.device).destroy_pipeline_layout(layout, None);
            }
            return Ok(entry.pipeline);
        }

        cache.insert(key, ComputePipeline { pipeline, layout });
        Ok(pipeline)
    }

    /// Retrieve an existing compute pipeline by key without creating one.
    pub fn get(
        &self,
        shader: &ShaderModule,
        set_layouts: &[vk::DescriptorSetLayout],
        push_ranges: &[vk::PushConstantRange],
    ) -> Option<vk::Pipeline> {
        let key = ComputePipelineKey {
            shader: shader.module,
            set_layouts: set_layouts.to_vec(),
            push_ranges: push_ranges.iter().copied().map(PushConstantKey::from).collect(),
        };
        self.cache.lock().get(&key).map(|e| e.pipeline)
    }

    /// Returns the number of cached compute pipelines.
    pub fn len(&self) -> usize {
        self.cache.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.lock().is_empty()
    }
}

impl Drop for ComputePipelineCache {
    fn drop(&mut self) {
        unsafe {
            if !self.device.is_null() {
                let dev = &*self.device;
                let cache = self.cache.get_mut();
                for entry in cache.values() {
                    if entry.pipeline != vk::Pipeline::null() {
                        dev.destroy_pipeline(entry.pipeline, None);
                    }
                    if entry.layout != vk::PipelineLayout::null() {
                        dev.destroy_pipeline_layout(entry.layout, None);
                    }
                }
            }
        }
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
                    if entry.layout != vk::PipelineLayout::null() {
                        dev.destroy_pipeline_layout(entry.layout, None);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
