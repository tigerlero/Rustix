use rustix_core::ecs::EcsWorld;
use rustix_core::math::{Vec3, Vec4};
use rustix_render::particle::{GpuParticle};
use rustix_render::particle_gpu::{GpuParticleSimulation, ParticleSimParams};
use rustix_render::components::ParticleEmitter;
use rustix_render::memory::GpuBuffer;
use rustix_render::pipeline::ParticlePipeline;
use rustix_render::Renderer;
use crate::scene::Transform;
use ash::vk;
use std::collections::HashMap;

/// Per-emitter GPU resources with GPU simulation.
pub struct EmitterGpuState {
    pub particles: Vec<GpuParticle>,
    pub particle_buffer: GpuBuffer,
    pub max_count: u32,
    pub alive_count: u32,
    pub desc_set: vk::DescriptorSet,
    pub spawn_accumulator: f32,
}

/// Manages GPU simulation and rendering of particles.
pub struct ParticleSystem {
    pub emitters: HashMap<rustix_core::ecs::Entity, EmitterGpuState>,
    pub pipeline: Option<ParticlePipeline>,
    pub dummy_vertex_buffer: Option<GpuBuffer>,
    pub gpu_sim: Option<GpuParticleSimulation>,
}

impl ParticleSystem {
    pub fn new() -> Self {
        Self {
            emitters: HashMap::new(),
            pipeline: None,
            dummy_vertex_buffer: None,
            gpu_sim: None,
        }
    }

    pub fn init(&mut self, renderer: &Renderer) {
        if self.pipeline.is_some() {
            return;
        }
        let device = renderer.device();
        let swapchain = renderer.swapchain.lock();
        let vs = rustix_render::shader::builtin::particle::vertex_shader_override(device.logical())
            .expect("particle vertex shader");
        let fs = rustix_render::shader::builtin::particle::fragment_shader_override(device.logical())
            .expect("particle fragment shader");
        let bindless = renderer.bindless_heap().layout();
        let pipeline = ParticlePipeline::create(device, &swapchain, &vs, &fs, bindless)
            .expect("particle pipeline");
        drop(swapchain);

        // Create compute pipelines for GPU simulation
        let sim_cs = rustix_render::shader::builtin::particle_compute::particle_simulate_shader_override(device.logical())
            .expect("particle simulate compute shader");
        let sort_cs = rustix_render::shader::builtin::particle_compute::particle_sort_shader_override(device.logical())
            .expect("particle sort compute shader");
        let gpu_sim = GpuParticleSimulation::create(device, &sim_cs, &sort_cs, 65536)
            .expect("gpu particle simulation");

        let quad_verts: [f32; 12] = [
            0.0, 0.0, 0.0,
            0.0, 0.0, 0.0,
            0.0, 0.0, 0.0,
            0.0, 0.0, 0.0,
        ];
        let mut allocator = renderer.allocator.lock();
        let dummy_vb = GpuBuffer::new(
            device,
            &mut allocator,
            "particle_dummy_vb",
            48,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        ).expect("particle dummy vb");
        dummy_vb.write(bytemuck::bytes_of(&quad_verts));
        drop(allocator);

        self.pipeline = Some(pipeline);
        self.dummy_vertex_buffer = Some(dummy_vb);
        self.gpu_sim = Some(gpu_sim);
    }

    pub fn update(&mut self, dt: f32, ecs_world: &EcsWorld, renderer: &Renderer) {
        if self.pipeline.is_none() {
            self.init(renderer);
        }
        let gpu_sim = self.gpu_sim.as_ref().unwrap();

        let device = renderer.device();
        let mut allocator = renderer.allocator.lock();

        // Remove emitters that no longer exist
        self.emitters.retain(|e, _| ecs_world.contains(*e));

        for (entity, emitter, xform) in ecs_world.query::<(rustix_core::ecs::Entity, &ParticleEmitter, &Transform)>().iter() {
            if !emitter.enabled {
                continue;
            }

            let max_particles = emitter.max_particles;
            let buffer_size = (max_particles as u64) * std::mem::size_of::<GpuParticle>() as u64;

            let state = self.emitters.entry(entity).or_insert_with(|| {
                let buf = GpuBuffer::new(
                    device,
                    &mut allocator,
                    "particle_buffer",
                    buffer_size,
                    vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::VERTEX_BUFFER,
                    gpu_allocator::MemoryLocation::CpuToGpu,
                ).expect("particle buffer");
                let desc_set = renderer.allocate_descriptor_set(gpu_sim.desc_layout)
                    .expect("particle desc set");
                gpu_sim.update_descriptor_set(device, desc_set, buf.buffer);
                EmitterGpuState {
                    particles: vec![GpuParticle::default(); max_particles as usize],
                    particle_buffer: buf,
                    max_count: max_particles,
                    alive_count: 0,
                    desc_set,
                    spawn_accumulator: 0.0,
                }
            });

            // Resize buffer if max_particles changed
            if state.max_count != max_particles {
                let new_buf = GpuBuffer::new(
                    device,
                    &mut allocator,
                    "particle_buffer",
                    buffer_size,
                    vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::VERTEX_BUFFER,
                    gpu_allocator::MemoryLocation::CpuToGpu,
                ).expect("particle buffer resize");
                gpu_sim.update_descriptor_set(device, state.desc_set, new_buf.buffer);
                state.particles.resize(max_particles as usize, GpuParticle::default());
                state.particle_buffer = new_buf;
                state.max_count = max_particles;
                state.alive_count = 0;
            }

            // CPU-side spawn: find dead particles and write new ones
            state.spawn_accumulator += emitter.spawn_rate * dt;
            let spawn_count = state.spawn_accumulator.floor() as u32;
            state.spawn_accumulator -= spawn_count as f32;

            let mut spawned = 0u32;
            let mut rng_seed = state.spawn_accumulator.to_bits() ^ state.max_count;
            let mut next_rand = || {
                rng_seed = rng_seed.wrapping_mul(1103515245).wrapping_add(12345);
                (rng_seed as f32 / u32::MAX as f32)
            };

            for i in 0..state.max_count {
                if spawned >= spawn_count {
                    break;
                }
                if !state.particles[i as usize].is_alive() {
                    let spread = emitter.velocity_spread;
                    let vel = Vec3::new(
                        emitter.velocity.x + (next_rand() - 0.5) * spread,
                        emitter.velocity.y + (next_rand() - 0.5) * spread,
                        emitter.velocity.z + (next_rand() - 0.5) * spread,
                    );
                    let max_life = emitter.lifetime + (next_rand() - 0.5) * emitter.lifetime_spread;
                    state.particles[i as usize] = GpuParticle::new(
                        xform.position,
                        emitter.start_size,
                        Vec4::new(emitter.start_color[0], emitter.start_color[1], emitter.start_color[2], emitter.start_color[3]),
                        vel,
                        max_life,
                        max_life,
                        emitter.start_size,
                        emitter.end_size,
                    );
                    spawned += 1;
                    state.alive_count += 1;
                }
            }

            // Write CPU particle data to GPU buffer
            let data = bytemuck::cast_slice(&state.particles);
            state.particle_buffer.write(data);
        }

        drop(allocator);
    }

    pub fn render(&self, cmd: vk::CommandBuffer, renderer: &Renderer, cam_pos: Vec3, enable_sorting: bool) {
        let Some(ref pipeline) = self.pipeline else { return };
        let Some(ref dummy_vb) = self.dummy_vertex_buffer else { return };
        let gpu_sim = self.gpu_sim.as_ref().unwrap();
        let bindless_set = renderer.bindless_heap().set();

        for state in self.emitters.values() {
            if state.alive_count == 0 {
                continue;
            }

            // GPU simulation dispatch
            let sim_params = ParticleSimParams {
                dt: 0.016,
                gravity: -9.81,
                ground_plane_y: 0.0,
                bounce: 0.6,
                particle_count: state.max_count,
                enable_collision: 0,
                cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z],
                sort_by_depth: if enable_sorting { 1 } else { 0 },
            };
            gpu_sim.dispatch_simulate(cmd, renderer.device(), state.desc_set, &sim_params);

            // Barrier before sort or graphics
            GpuParticleSimulation::barrier_compute_to_graphics(cmd, renderer.device());

            if enable_sorting {
                gpu_sim.dispatch_sort(cmd, renderer.device(), state.desc_set, state.max_count, [cam_pos.x, cam_pos.y, cam_pos.z]);
                GpuParticleSimulation::barrier_compute_to_graphics(cmd, renderer.device());
            }

            // Graphics render
            unsafe {
                let device = renderer.device().logical();
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
                device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[bindless_set], &[]);
                device.cmd_bind_vertex_buffers(cmd, 0, &[dummy_vb.buffer], &[0u64]);
                device.cmd_bind_vertex_buffers(cmd, 1, &[state.particle_buffer.buffer], &[0u64]);
                device.cmd_draw(cmd, 4, state.max_count, 0, 0);
            }
        }
    }

    /// Render particles into the swapchain image after the main frame graph.
    /// Sets up dynamic rendering around the particle draw.
    pub fn render_swapchain(
        &self,
        cmd: vk::CommandBuffer,
        renderer: &Renderer,
        color_view: vk::ImageView,
        color_format: vk::Format,
        extent: vk::Extent2D,
        cam_pos: Vec3,
    ) {
        if self.emitters.is_empty() {
            return;
        }
        let ca = [vk::RenderingAttachmentInfo::default()
            .image_view(color_view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE)];
        let rendering_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            })
            .layer_count(1)
            .color_attachments(&ca);
        unsafe {
            renderer.device().logical().cmd_begin_rendering(cmd, &rendering_info);
        }
        let vp = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as f32,
            height: extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let sc = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent,
        }];
        unsafe {
            let device = renderer.device().logical();
            device.cmd_set_viewport(cmd, 0, &vp);
            device.cmd_set_scissor(cmd, 0, &sc);
        }
        self.render(cmd, renderer, cam_pos, true);
        unsafe {
            renderer.device().logical().cmd_end_rendering(cmd);
        }
    }
}
