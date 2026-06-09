use ash::vk;
use parking_lot::Mutex;
use std::collections::HashMap;

use crate::device::GpuDevice;
use crate::shader::ShaderModule;
use crate::spec_constants::SpecConstantMap;
use crate::RenderError;

pub const PUSH_CONSTANT_SIZE: u32 = 128; // Mat4(64) + dir_light(16) + dir_color(16) + base_color(16) + rough_metal(16)
pub const UBO_SCENE_SIZE: u64 = 432; // view_proj(64)+cam(16)+count(4)+pad(12)+8*PointLight(256)+fog(16)+light_view_proj(64)
pub const PUSH_CONSTANT_SIZE_2D: u32 = 64;
pub const DEFERRED_PUSH_CONSTANT_SIZE: u32 = 128; // inv_view_proj(64) + cam_pos(16) + dir_light(16) + dir_color(16) + light_count(4) + max_lights_per_tile(4) + pad(4)

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

/// Hashable key for a pipeline layout configuration.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PipelineLayoutKey {
    set_layouts: Vec<vk::DescriptorSetLayout>,
    push_ranges: Vec<PushConstantKey>,
}

/// Caches `vk::PipelineLayout` objects keyed by their set layouts and push constant ranges.
///
/// Use `get_or_create` to obtain a layout. The cache retains the Vulkan handle until
/// the cache itself is dropped.
pub struct PipelineLayoutCache {
    device: *const ash::Device,
    cache: Mutex<HashMap<PipelineLayoutKey, vk::PipelineLayout>>,
}

unsafe impl Send for PipelineLayoutCache {}
unsafe impl Sync for PipelineLayoutCache {}

impl PipelineLayoutCache {
    pub fn new(device: &ash::Device) -> Self {
        Self {
            device: device as *const ash::Device,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a `vk::PipelineLayout` for the given configuration.
    pub fn get_or_create(
        &self,
        set_layouts: &[vk::DescriptorSetLayout],
        push_ranges: &[vk::PushConstantRange],
    ) -> Result<vk::PipelineLayout, RenderError> {
        let key = PipelineLayoutKey {
            set_layouts: set_layouts.to_vec(),
            push_ranges: push_ranges.iter().copied().map(PushConstantKey::from).collect(),
        };

        {
            let cache = self.cache.lock();
            if let Some(&layout) = cache.get(&key) {
                return Ok(layout);
            }
        }

        let layout = unsafe {
            (*self.device)
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(set_layouts)
                        .push_constant_ranges(push_ranges),
                    None,
                )
                .map_err(|e| RenderError::DeviceCreation(format!("pipeline layout: {e}")))?
        };

        let mut cache = self.cache.lock();
        if let Some(&existing) = cache.get(&key) {
            unsafe {
                (*self.device).destroy_pipeline_layout(layout, None);
            }
            return Ok(existing);
        }

        cache.insert(key, layout);
        Ok(layout)
    }

    pub fn len(&self) -> usize {
        self.cache.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.lock().is_empty()
    }
}

impl Drop for PipelineLayoutCache {
    fn drop(&mut self) {
        unsafe {
            if !self.device.is_null() {
                let dev = &*self.device;
                let cache = self.cache.get_mut();
                for &layout in cache.values() {
                    if layout != vk::PipelineLayout::null() {
                        dev.destroy_pipeline_layout(layout, None);
                    }
                }
            }
        }
    }
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
    /// Specialization constants applied to the fragment shader.
    pub spec_constants: SpecConstantMap,
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
            spec_constants: SpecConstantMap::new(),
        }
    }
}


pub mod scene;
pub mod postprocess;
pub mod oit;
pub mod special;

pub use scene::*;
pub use postprocess::*;
pub use oit::*;
pub use special::*;

#[cfg(test)]
#[path = "../pipeline_tests.rs"]
mod tests;

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
        device: &GpuDevice,
        shader: &ShaderModule,
        set_layouts: &[vk::DescriptorSetLayout],
        push_ranges: &[vk::PushConstantRange],
    ) -> Result<(vk::Pipeline, vk::PipelineLayout), RenderError> {
        let key = ComputePipelineKey {
            shader: shader.module,
            set_layouts: set_layouts.to_vec(),
            push_ranges: push_ranges.iter().copied().map(PushConstantKey::from).collect(),
        };

        {
            let cache = self.cache.lock();
            if let Some(entry) = cache.get(&key) {
                return Ok((entry.pipeline, entry.layout));
            }
        }

        let (pipeline, layout) = unsafe {
            let layout = device.pipeline_layout_cache()
                .get_or_create(set_layouts, push_ranges)
                .map_err(|e| RenderError::PipelineCreation(format!("compute layout: {e}")))?;

            let stage = shader.stage_create_info();
            let ci = vk::ComputePipelineCreateInfo::default()
                .stage(stage)
                .layout(layout);

            let pipeline = (*self.device)
                .create_compute_pipelines(vk::PipelineCache::null(), &[ci], None)
                .map_err(|(_, e)| {
                    // Layout is owned by PipelineLayoutCache; do not destroy here.
                    RenderError::PipelineCreation(format!("compute pipeline: {e}"))
                })?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    RenderError::PipelineCreation("no compute pipeline created".into())
                })?;

            (pipeline, layout)
        };

        let mut cache = self.cache.lock();
        // Another thread may have raced us.
        if let Some(entry) = cache.get(&key) {
            unsafe {
                (*self.device).destroy_pipeline(pipeline, None);
                // Layout is owned by PipelineLayoutCache; do not destroy here.
            }
            return Ok((entry.pipeline, entry.layout));
        }

        cache.insert(key, ComputePipeline { pipeline, layout });
        Ok((pipeline, layout))
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
                    // Pipeline layouts are owned by PipelineLayoutCache; do not destroy here.
                }
            }
        }
    }
}
