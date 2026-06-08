use ash::vk;
use rustix_render::memory::GpuBuffer;
use rustix_render::device::GpuDevice;
use rustix_render::RenderError;
use rustix_core::math::{Vec3, Vec4, Mat4};

/// GPU culling input format: same as InstanceData plus AABB and mesh index.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CullInstance {
    pub model_col0: [f32; 4],
    pub model_col1: [f32; 4],
    pub model_col2: [f32; 4],
    pub model_col3: [f32; 4],
    pub base_color: [f32; 4],
    pub material: [f32; 4],
    pub aabb_min: [f32; 4], // xyz = min, w = mesh_index
    pub aabb_max: [f32; 4], // xyz = max, w = unused
}

impl CullInstance {
    pub fn from_instance_data(data: &super::InstanceData, aabb_min: Vec3, aabb_max: Vec3, mesh_index: u32) -> Self {
        Self {
            model_col0: data.model_col0,
            model_col1: data.model_col1,
            model_col2: data.model_col2,
            model_col3: data.model_col3,
            base_color: data.base_color,
            material: data.material,
            aabb_min: [aabb_min.x, aabb_min.y, aabb_min.z, mesh_index as f32],
            aabb_max: [aabb_max.x, aabb_max.y, aabb_max.z, 0.0],
        }
    }
}

/// Per-batch info for command generation.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct BatchInfo {
    pub mesh_index: u32,
    pub instance_offset: u32,
    pub instance_count: u32,
    pub index_count: u32,
}

/// Push constants for the compute culling shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CullPushConstants {
    pub view_proj: Mat4,
    pub frustum_planes: [Vec4; 6],
    pub instance_count: u32,
    pub batch_count: u32,
    pub _pad: [u32; 2],
}

/// Push constants for the draw command generation shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GenDrawPushConstants {
    pub batch_count: u32,
    pub _pad: [u32; 3],
}

/// GPU-driven culling resources.
pub struct GpuCullingResources {
    /// Input buffer with CullInstance data (culling binding 0).
    pub input_buffer: GpuBuffer,
    /// Output buffer with per-instance visibility flags (culling binding 1).
    pub visibility_buffer: GpuBuffer,
    /// Atomic counter buffer for per-batch visible counts (culling binding 2).
    pub counter_buffer: GpuBuffer,
    /// Indirect draw command buffer, written by compute (gen binding 1).
    pub draw_command_buffer: GpuBuffer,
    /// Batch info buffer for command generation (gen binding 2).
    pub batch_info_buffer: GpuBuffer,
    /// Culling descriptor set layout.
    pub cull_desc_layout: vk::DescriptorSetLayout,
    /// Culling descriptor set.
    pub cull_desc_set: vk::DescriptorSet,
    /// Gen descriptor set layout.
    pub gen_desc_layout: vk::DescriptorSetLayout,
    /// Gen descriptor set.
    pub gen_desc_set: vk::DescriptorSet,
    /// Culling compute pipeline.
    pub cull_pipeline: vk::Pipeline,
    /// Culling compute pipeline layout.
    pub cull_layout: vk::PipelineLayout,
    /// Gen compute pipeline.
    pub gen_pipeline: vk::Pipeline,
    /// Gen compute pipeline layout.
    pub gen_layout: vk::PipelineLayout,
    pub max_instances: usize,
    pub max_batches: usize,
}

impl GpuCullingResources {
    pub fn new(
        renderer: &rustix_render::Renderer,
        device: &GpuDevice,
        max_instances: usize,
        max_batches: usize,
    ) -> Result<Self, RenderError> {
        let instance_size = (max_instances * std::mem::size_of::<CullInstance>()) as u64;
        let visibility_size = (max_instances * std::mem::size_of::<u32>()) as u64;
        let counter_size = (max_batches * std::mem::size_of::<u32>()) as u64;
        let draw_cmd_size = (max_batches * std::mem::size_of::<vk::DrawIndexedIndirectCommand>()) as u64;
        let batch_info_size = (max_batches * std::mem::size_of::<BatchInfo>()) as u64;

        let input_buffer = renderer.create_buffer(
            "cull_input", instance_size,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        let visibility_buffer = renderer.create_buffer(
            "cull_visibility", visibility_size,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        let counter_buffer = renderer.create_buffer(
            "cull_counters", counter_size,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        let draw_command_buffer = renderer.create_buffer(
            "cull_draw_cmds", draw_cmd_size,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::INDIRECT_BUFFER,
            gpu_allocator::MemoryLocation::GpuOnly,
        )?;
        let batch_info_buffer = renderer.create_buffer(
            "cull_batch_info", batch_info_size,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        // Culling descriptor layout: bindings 0, 1, 2
        let cull_bindings = [
            vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];
        let cull_desc_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&cull_bindings),
                None,
            ).map_err(|e| RenderError::DeviceCreation(format!("cull desc layout: {e}")))?
        };
        let cull_desc_set = renderer.allocate_descriptor_set(cull_desc_layout)
            .map_err(|e| RenderError::DeviceCreation(format!("cull desc set: {e}")))?;

        let cull_bi = [
            vk::DescriptorBufferInfo::default().buffer(input_buffer.buffer).offset(0).range(instance_size),
            vk::DescriptorBufferInfo::default().buffer(visibility_buffer.buffer).offset(0).range(visibility_size),
            vk::DescriptorBufferInfo::default().buffer(counter_buffer.buffer).offset(0).range(counter_size),
        ];
        let cull_writes = [
            vk::WriteDescriptorSet::default().dst_set(cull_desc_set).dst_binding(0).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(std::slice::from_ref(&cull_bi[0])),
            vk::WriteDescriptorSet::default().dst_set(cull_desc_set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(std::slice::from_ref(&cull_bi[1])),
            vk::WriteDescriptorSet::default().dst_set(cull_desc_set).dst_binding(2).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(std::slice::from_ref(&cull_bi[2])),
        ];

        // Gen descriptor layout: bindings 0, 1, 2
        let gen_bindings = [
            vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];
        let gen_desc_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&gen_bindings),
                None,
            ).map_err(|e| RenderError::DeviceCreation(format!("gen desc layout: {e}")))?
        };
        let gen_desc_set = renderer.allocate_descriptor_set(gen_desc_layout)
            .map_err(|e| RenderError::DeviceCreation(format!("gen desc set: {e}")))?;

        let gen_bi = [
            vk::DescriptorBufferInfo::default().buffer(counter_buffer.buffer).offset(0).range(counter_size),
            vk::DescriptorBufferInfo::default().buffer(draw_command_buffer.buffer).offset(0).range(draw_cmd_size),
            vk::DescriptorBufferInfo::default().buffer(batch_info_buffer.buffer).offset(0).range(batch_info_size),
        ];
        let gen_writes = [
            vk::WriteDescriptorSet::default().dst_set(gen_desc_set).dst_binding(0).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(std::slice::from_ref(&gen_bi[0])),
            vk::WriteDescriptorSet::default().dst_set(gen_desc_set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(std::slice::from_ref(&gen_bi[1])),
            vk::WriteDescriptorSet::default().dst_set(gen_desc_set).dst_binding(2).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(std::slice::from_ref(&gen_bi[2])),
        ];

        unsafe {
            device.logical().update_descriptor_sets(&cull_writes, &[]);
            device.logical().update_descriptor_sets(&gen_writes, &[]);
        }

        let cs = rustix_render::shader::builtin::cull_instances_compute_shader(device.logical())?;
        let cull_push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE).offset(0)
            .size(std::mem::size_of::<CullPushConstants>() as u32);
        let (cull_pipeline, cull_layout) = renderer.compute_pipeline_cache().get_or_create(
            device, &cs, &[cull_desc_layout], &[cull_push_range],
        )?;

        let gen_cs = rustix_render::shader::builtin::gen_draw_cmds_compute_shader(device.logical())?;
        let gen_push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE).offset(0)
            .size(std::mem::size_of::<GenDrawPushConstants>() as u32);
        let (gen_pipeline, gen_layout) = renderer.compute_pipeline_cache().get_or_create(
            device, &gen_cs, &[gen_desc_layout], &[gen_push_range],
        )?;

        Ok(Self {
            input_buffer, visibility_buffer, counter_buffer,
            draw_command_buffer, batch_info_buffer,
            cull_desc_layout, cull_desc_set,
            gen_desc_layout, gen_desc_set,
            cull_pipeline, cull_layout,
            gen_pipeline, gen_layout,
            max_instances, max_batches,
        })
    }

    /// Write cull instance data to the GPU input buffer.
    pub fn write_input(&mut self, instances: &[CullInstance]) -> usize {
        let count = instances.len().min(self.max_instances);
        if count == 0 { return 0; }
        if let Some(ptr) = self.input_buffer.mapped_ptr {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    instances.as_ptr() as *const u8, ptr,
                    count * std::mem::size_of::<CullInstance>(),
                );
            }
        }
        count
    }

    /// Write batch info to the GPU batch info buffer.
    pub fn write_batch_info(&mut self, batches: &[BatchInfo]) -> usize {
        let count = batches.len().min(self.max_batches);
        if count == 0 { return 0; }
        if let Some(ptr) = self.batch_info_buffer.mapped_ptr {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    batches.as_ptr() as *const u8, ptr,
                    count * std::mem::size_of::<BatchInfo>(),
                );
            }
        }
        count
    }

    /// Reset atomic counters to zero.
    pub fn reset_counters(&mut self, batch_count: usize) {
        let count = batch_count.min(self.max_batches);
        if let Some(ptr) = self.counter_buffer.mapped_ptr {
            unsafe {
                std::ptr::write_bytes(ptr, 0, count * std::mem::size_of::<u32>());
            }
        }
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            device.destroy_descriptor_set_layout(self.cull_desc_layout, None);
            device.destroy_descriptor_set_layout(self.gen_desc_layout, None);
        }
    }
}
