use ash::vk;
use rustix_render::Renderer;
use rustix_render::memory::GpuBuffer;
use rustix_render::RenderError;

/// GPU buffers and pipeline state for Forward+ tiled light culling.
pub struct ForwardPlusResources {
    pub light_buffer: GpuBuffer,
    pub tile_buffer: GpuBuffer,
    pub light_slot: u32,
    pub tile_slot: u32,
    pub compute_pipeline: vk::Pipeline,
    pub compute_layout: vk::PipelineLayout,
}

impl ForwardPlusResources {
    pub const MAX_LIGHTS: usize = 256;
    pub const TILE_SIZE: u32 = 16;
    pub const MAX_LIGHTS_PER_TILE: u32 = 32;

    pub fn new(renderer: &Renderer) -> Result<Self, RenderError> {
        let light_size = (Self::MAX_LIGHTS * 32) as u64; // vec4 + vec4 per light
        let max_tiles_x = (3840u32 + Self::TILE_SIZE - 1) / Self::TILE_SIZE;
        let max_tiles_y = (2160u32 + Self::TILE_SIZE - 1) / Self::TILE_SIZE;
        let tile_struct_size = (Self::MAX_LIGHTS_PER_TILE + 1) * 4; // count + indices
        let tile_size = (max_tiles_x * max_tiles_y * tile_struct_size) as u64;

        let light_buffer = renderer.create_buffer(
            "fwd_plus_lights",
            light_size,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        let tile_buffer = renderer.create_buffer(
            "fwd_plus_tiles",
            tile_size,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            gpu_allocator::MemoryLocation::GpuOnly,
        )?;

        let heap = renderer.bindless_heap();
        let light_slot = heap.alloc_storage_buffer(3, light_buffer.buffer, light_size);
        let tile_slot = heap.alloc_storage_buffer(4, tile_buffer.buffer, tile_size);

        let cs = rustix_render::shader::builtin::light_cull_compute_shader(renderer.device().logical())?;
        let bindless_layout = renderer.bindless_heap().layout();
        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .offset(0)
            .size(104); // mat4(64) + vec4(16) + uvec2(8) + uvec2(8) + uint(4) + uint(4)
        let (compute_pipeline, compute_layout) = renderer.compute_pipeline_cache().get_or_create(
            renderer.device(),
            &cs,
            &[bindless_layout],
            &[push_range],
        )?;

        Ok(Self {
            light_buffer,
            tile_buffer,
            light_slot,
            tile_slot,
            compute_pipeline,
            compute_layout,
        })
    }
}
