//! GPU particle simulation via compute shaders.

use ash::vk;
use crate::device::GpuDevice;
use crate::memory::GpuBuffer;
use crate::pipeline::ComputePipelineCache;
use crate::shader::ShaderModule;
use crate::RenderError;

/// Push constants for the particle simulation compute shader.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ParticleSimParams {
    pub dt: f32,
    pub gravity: f32,
    pub ground_plane_y: f32,
    pub bounce: f32,
    pub particle_count: u32,
    pub enable_collision: u32,
    pub cam_pos: [f32; 3],
    pub sort_by_depth: u32,
}

unsafe impl bytemuck::Pod for ParticleSimParams {}
unsafe impl bytemuck::Zeroable for ParticleSimParams {}

impl Default for ParticleSimParams {
    fn default() -> Self {
        Self {
            dt: 0.016,
            gravity: -9.81,
            ground_plane_y: 0.0,
            bounce: 0.6,
            particle_count: 0,
            enable_collision: 0,
            cam_pos: [0.0; 3],
            sort_by_depth: 0,
        }
    }
}

/// Push constants for the particle sort compute shader.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ParticleSortParams {
    pub stage: u32,
    pub step: u32,
    pub particle_count: u32,
    pub _pad: u32,
    pub cam_pos: [f32; 3],
    pub _pad2: u32,
}

unsafe impl bytemuck::Pod for ParticleSortParams {}
unsafe impl bytemuck::Zeroable for ParticleSortParams {}

/// Manages GPU compute pipelines and descriptor sets for particle simulation.
pub struct GpuParticleSimulation {
    pub desc_layout: vk::DescriptorSetLayout,
    pub simulate_pipeline: vk::Pipeline,
    pub simulate_layout: vk::PipelineLayout,
    pub sort_pipeline: vk::Pipeline,
    pub sort_layout: vk::PipelineLayout,
    pub max_particles: u32,
    device: *const ash::Device,
}

unsafe impl Send for GpuParticleSimulation {}
unsafe impl Sync for GpuParticleSimulation {}

impl GpuParticleSimulation {
    pub fn create(
        device: &GpuDevice,
        sim_shader: &ShaderModule,
        sort_shader: &ShaderModule,
        max_particles: u32,
    ) -> Result<Self, RenderError> {
        let desc_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
        ];
        let desc_layout = unsafe {
            device.logical().create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&desc_bindings),
                None,
            )
        }.map_err(|e| RenderError::DeviceCreation(format!("particle compute desc layout: {e}")))?;

        let push_range_sim = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .offset(0)
            .size(std::mem::size_of::<ParticleSimParams>() as u32)];
        let simulate_layout = unsafe {
            device.logical().create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&[desc_layout])
                    .push_constant_ranges(&push_range_sim),
                None,
            )
        }.map_err(|e| RenderError::DeviceCreation(format!("particle sim layout: {e}")))?;

        let push_range_sort = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .offset(0)
            .size(std::mem::size_of::<ParticleSortParams>() as u32)];
        let sort_layout = unsafe {
            device.logical().create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&[desc_layout])
                    .push_constant_ranges(&push_range_sort),
                None,
            )
        }.map_err(|e| RenderError::DeviceCreation(format!("particle sort layout: {e}")))?;

        let sim_stage = sim_shader.stage_create_info();
        let sim_ci = vk::ComputePipelineCreateInfo::default()
            .stage(sim_stage)
            .layout(simulate_layout);
        let simulate_pipeline = unsafe {
            device.logical().create_compute_pipelines(
                vk::PipelineCache::null(),
                &[sim_ci],
                None,
            )
        }
        .map_err(|(_, e)| RenderError::PipelineCreation(format!("particle simulate compute pipeline: {e}")))?
        .into_iter()
        .next()
        .ok_or_else(|| RenderError::PipelineCreation("no particle simulate pipeline".into()))?;

        let sort_stage = sort_shader.stage_create_info();
        let sort_ci = vk::ComputePipelineCreateInfo::default()
            .stage(sort_stage)
            .layout(sort_layout);
        let sort_pipeline = unsafe {
            device.logical().create_compute_pipelines(
                vk::PipelineCache::null(),
                &[sort_ci],
                None,
            )
        }
        .map_err(|(_, e)| RenderError::PipelineCreation(format!("particle sort compute pipeline: {e}")))?
        .into_iter()
        .next()
        .ok_or_else(|| RenderError::PipelineCreation("no particle sort pipeline".into()))?;

        Ok(Self {
            desc_layout,
            simulate_pipeline,
            simulate_layout,
            sort_pipeline,
            sort_layout,
            max_particles,
            device: device.logical() as *const ash::Device,
        })
    }

    /// Update the descriptor set to point at the given particle storage buffer.
    pub fn update_descriptor_set(
        &self,
        device: &GpuDevice,
        desc_set: vk::DescriptorSet,
        buffer: vk::Buffer,
    ) {
        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(buffer)
            .offset(0)
            .range(vk::WHOLE_SIZE)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(desc_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&buffer_info);
        unsafe {
            device.logical().update_descriptor_sets(&[write], &[]);
        }
    }

    /// Dispatch the simulation compute shader.
    pub fn dispatch_simulate(
        &self,
        cmd: vk::CommandBuffer,
        device: &GpuDevice,
        desc_set: vk::DescriptorSet,
        params: &ParticleSimParams,
    ) {
        let groups = ((params.particle_count + 255) / 256).max(1);
        unsafe {
            let logical = device.logical();
            logical.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.simulate_pipeline);
            logical.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.simulate_layout,
                0,
                &[desc_set],
                &[],
            );
            logical.cmd_push_constants(
                cmd,
                self.simulate_layout,
                vk::ShaderStageFlags::COMPUTE,
                0,
                bytemuck::bytes_of(params),
            );
            logical.cmd_dispatch(cmd, groups, 1, 1);
        }
    }

    /// Dispatch bitonic sort passes. Requires `particle_count` to be a power of 2.
    pub fn dispatch_sort(
        &self,
        cmd: vk::CommandBuffer,
        device: &GpuDevice,
        desc_set: vk::DescriptorSet,
        particle_count: u32,
        cam_pos: [f32; 3],
    ) {
        if particle_count < 2 {
            return;
        }
        // Pad to power of 2 for bitonic sort.
        let n = particle_count.next_power_of_two();
        unsafe {
            let logical = device.logical();
            logical.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.sort_pipeline);
            logical.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.sort_layout,
                0,
                &[desc_set],
                &[],
            );
            let groups = ((n / 2 + 255) / 256).max(1);
            let mut stage = 2u32;
            while stage <= n {
                let mut step = stage / 2;
                while step >= 1 {
                    let params = ParticleSortParams {
                        stage,
                        step,
                        particle_count: n,
                        _pad: 0,
                        cam_pos,
                        _pad2: 0,
                    };
                    logical.cmd_push_constants(
                        cmd,
                        self.sort_layout,
                        vk::ShaderStageFlags::COMPUTE,
                        0,
                        bytemuck::bytes_of(&params),
                    );
                    logical.cmd_dispatch(cmd, groups, 1, 1);
                    // Memory barrier between sort passes
                    let barrier = vk::MemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                        .dst_access_mask(vk::AccessFlags::SHADER_READ);
                    logical.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::DependencyFlags::empty(),
                        &[barrier],
                        &[],
                        &[],
                    );
                    step /= 2;
                }
                stage *= 2;
            }
        }
    }

    /// Insert a barrier after compute writes before graphics reads the same buffer.
    pub fn barrier_compute_to_graphics(cmd: vk::CommandBuffer, device: &GpuDevice) {
        let barrier = vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::SHADER_WRITE)
            .dst_access_mask(vk::AccessFlags::VERTEX_ATTRIBUTE_READ);
        unsafe {
            device.logical().cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::VERTEX_INPUT,
                vk::DependencyFlags::empty(),
                &[barrier],
                &[],
                &[],
            );
        }
    }
}

impl Drop for GpuParticleSimulation {
    fn drop(&mut self) {
        if self.device.is_null() { return; }
        unsafe {
            let d = &*self.device;
            d.destroy_pipeline(self.simulate_pipeline, None);
            d.destroy_pipeline(self.sort_pipeline, None);
            d.destroy_pipeline_layout(self.simulate_layout, None);
            d.destroy_pipeline_layout(self.sort_layout, None);
            d.destroy_descriptor_set_layout(self.desc_layout, None);
        }
    }
}
