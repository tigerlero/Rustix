pub mod instance;
pub mod device;
pub mod surface;
pub mod swapchain;
pub mod shader;
pub mod pipeline;
pub mod memory;
pub mod mesh;
pub mod components;

// Re-export commonly used component types
pub use components::{Sprite, SpriteRenderer, DirectionalLight, PointLight, SpotLight};

use std::sync::Arc;

use ash::vk;
use parking_lot::Mutex;

use instance::VulkanInstance;
use device::GpuDevice;
use memory::{GpuBuffer, GpuMemoryAllocator};
use swapchain::Swapchain;

use rustix_core::config::RenderConfig;

pub use ash;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Vulkan: {0}")]
    Vulkan(#[from] vk::Result),
    #[error("instance: {0}")]
    InstanceCreation(String),
    #[error("device: {0}")]
    DeviceCreation(String),
    #[error("surface: {0}")]
    SurfaceCreation(String),
    #[error("swapchain: {0}")]
    SwapchainCreation(String),
    #[error("shader: {0}")]
    ShaderCompile(String),
    #[error("pipeline: {0}")]
    PipelineCreation(String),
}

pub struct Renderer {
    pub instance: Arc<VulkanInstance>,
    pub device: Arc<GpuDevice>,
    pub swapchain: Arc<Mutex<Swapchain>>,
    pub allocator: Arc<Mutex<GpuMemoryAllocator>>,
    command_pool: vk::CommandPool,
    transfer_command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    in_flight_fences: Vec<vk::Fence>,
    frame_index: usize,
    initialized: bool,
}

impl Renderer {
    pub fn new(config: &RenderConfig) -> Result<Self, RenderError> {
        let instance = Arc::new(VulkanInstance::new(config)?);
        let device = Arc::new(GpuDevice::new(&instance, config)?);
        let allocator = GpuMemoryAllocator::new(&instance, &device)?;
        let cmd_pool = unsafe {
            device.logical().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(device.graphics_queue_family_index())
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER), None,
            )?
        };
        let transfer_pool = unsafe {
            device.logical().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(device.graphics_queue_family_index())
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER), None,
            )?
        };
        let cmd_bufs = unsafe {
            device.logical().allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default().command_pool(cmd_pool)
                    .level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(3),
            )?
        };
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let in_flight_fences: Vec<vk::Fence> = (0..3).map(|_| unsafe {
            device.logical().create_fence(&fence_info, None).expect("fence")
        }).collect();
        Ok(Self { instance, device, swapchain: Arc::new(Mutex::new(Swapchain::new())), allocator: Arc::new(Mutex::new(allocator)), command_pool: cmd_pool, transfer_command_pool: transfer_pool, command_buffers: cmd_bufs, in_flight_fences, frame_index: 0, initialized: false })
    }

    pub fn init_surface(&mut self, rw: raw_window_handle::RawWindowHandle, rd: raw_window_handle::RawDisplayHandle, w: u32, h: u32) -> Result<(), RenderError> {
        let s = surface::create_surface(&self.instance, rw, rd)?;
        self.swapchain.lock().init(&self.instance, &self.device, s, w, h)?;
        self.initialized = true;
        Ok(())
    }

    pub fn begin_frame(&mut self) -> Result<bool, RenderError> {
        if !self.initialized { return Ok(false); }
        let fence = self.in_flight_fences[self.frame_index % 3];
        unsafe { self.device.logical().wait_for_fences(&[fence], true, u64::MAX)?; }
        unsafe { self.device.logical().reset_fences(&[fence])?; }
        self.swapchain.lock().acquire_next_image(&self.device, self.frame_index)?;
        Ok(true)
    }

    pub fn end_frame(&mut self) -> Result<(), RenderError> {
        if !self.initialized { return Ok(()); }
        let cmd = self.current_cmd();
        let frame_idx = self.frame_index;
        unsafe { self.device.logical().end_command_buffer(cmd)?; }
        let sw = self.swapchain.lock();
        let ws = [sw.image_available_semaphore(frame_idx)];
        let ss = [sw.render_finished_semaphore(frame_idx)];
        drop(sw);
        let wst = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT]; let cmds = [cmd];
        let fence = self.in_flight_fences[frame_idx % 3];
        let si = vk::SubmitInfo::default().wait_semaphores(&ws).wait_dst_stage_mask(&wst).command_buffers(&cmds).signal_semaphores(&ss);
        let subs = [si];
        unsafe { self.device.logical().queue_submit(self.device.graphics_queue(), &subs, fence)?; }
        let ok = self.swapchain.lock().present(&self.device, &ss)?;
        if !ok { self.swapchain.lock().recreate(&self.instance, &self.device)?; }
        self.frame_index = self.frame_index.wrapping_add(1);
        Ok(())
    }

    pub fn current_cmd(&self) -> vk::CommandBuffer { self.command_buffers[self.frame_index % 3] }
    pub fn device(&self) -> &GpuDevice { &self.device }

    pub fn create_buffer(&self, name: &str, size: u64, usage: vk::BufferUsageFlags, location: gpu_allocator::MemoryLocation) -> Result<GpuBuffer, RenderError> {
        GpuBuffer::new(&self.device, &mut self.allocator.lock(), name, size, usage, location)
    }

    pub fn create_depth_buffer(&self, extent: vk::Extent2D) -> Result<DepthBuffer, RenderError> {
        let fmt = vk::Format::D32_SFLOAT;
        let img = unsafe {
            self.device.logical().create_image(
                &vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).format(fmt)
                    .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
                    .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL).usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("depth img: {e}")))?
        };
        let req = unsafe { self.device.logical().get_image_memory_requirements(img) };
        let alloc = self.allocator.lock().allocate("depth", req, gpu_allocator::MemoryLocation::GpuOnly, false)?;
        unsafe { self.device.logical().bind_image_memory(img, alloc.memory(), alloc.offset())?; }
        let view = unsafe {
            self.device.logical().create_image_view(
                &vk::ImageViewCreateInfo::default().image(img).view_type(vk::ImageViewType::TYPE_2D).format(fmt)
                    .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::DEPTH, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 }), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("depth view: {e}")))?
        };
        Ok(DepthBuffer { image: img, view, _allocation: alloc })
    }

    pub fn create_descriptor_pool(&self) -> Result<vk::DescriptorPool, RenderError> {
        let ps = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 1 },
        ];
        unsafe { self.device.logical().create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().pool_sizes(&ps).max_sets(1), None) }
            .map_err(|e| RenderError::DeviceCreation(format!("desc pool: {e}")))
    }

    pub fn alloc_descriptor_set(&self, pool: vk::DescriptorPool, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet, RenderError> {
        let ls = [layout];
        unsafe { self.device.logical().allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&ls)) }
            .map_err(|e| RenderError::DeviceCreation(format!("alloc set: {e}"))).map(|mut s| s.remove(0))
    }

    pub fn update_descriptor_set(&self, set: vk::DescriptorSet, ubo: &GpuBuffer) {
        let bi = [vk::DescriptorBufferInfo::default().buffer(ubo.buffer).offset(0).range(ubo.size)];
        let w = vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&bi);
        unsafe { self.device.logical().update_descriptor_sets(&[w], &[]); }
    }

    pub fn create_texture(&self, width: u32, height: u32, pixels: &[u8]) -> Result<GpuTexture, RenderError> {
        let extent = vk::Extent3D { width, height, depth: 1 };
        let fmt = vk::Format::R8G8B8A8_UNORM;

        // Staging buffer
        let staging = self.create_buffer("tex_staging", pixels.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, gpu_allocator::MemoryLocation::CpuToGpu)?;
        staging.write(pixels);

        // Device-local image
        let img = unsafe {
            self.device.logical().create_image(
                &vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).format(fmt).extent(extent)
                    .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL).usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("tex image: {e}")))?
        };
        let req = unsafe { self.device.logical().get_image_memory_requirements(img) };
        let alloc = self.allocator.lock().allocate("texture", req, gpu_allocator::MemoryLocation::GpuOnly, false)?;
        unsafe { self.device.logical().bind_image_memory(img, alloc.memory(), alloc.offset())?; }

        // Upload via transfer
        let one_time_cmd = {
            let info = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.transfer_command_pool)
                .level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
            unsafe { self.device.logical().allocate_command_buffers(&info)?.remove(0) }
        };
        unsafe {
            self.device.logical().begin_command_buffer(one_time_cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
        }

        // Transition: undefined → transfer dst
        let barrier1 = vk::ImageMemoryBarrier::default()
            .image(img).old_layout(vk::ImageLayout::UNDEFINED).new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_access_mask(vk::AccessFlags::empty()).dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        unsafe {
            self.device.logical().cmd_pipeline_barrier(one_time_cmd, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[barrier1]);
        }

        // Copy buffer to image
        let copy_region = vk::BufferImageCopy::default()
            .buffer_offset(0).image_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 }).image_extent(extent);
        unsafe {
            self.device.logical().cmd_copy_buffer_to_image(one_time_cmd, staging.buffer, img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_region]);
        }

        // Transition: transfer dst → shader read
        let barrier2 = vk::ImageMemoryBarrier::default()
            .image(img).old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL).new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE).dst_access_mask(vk::AccessFlags::SHADER_READ)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        unsafe {
            self.device.logical().cmd_pipeline_barrier(one_time_cmd, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &[barrier2]);
        }

        unsafe { self.device.logical().end_command_buffer(one_time_cmd)?; }
        let cmds = [one_time_cmd];
        let si = vk::SubmitInfo::default().command_buffers(&cmds);
        let subs = [si];
        let upload_fence = unsafe { self.device.logical().create_fence(&vk::FenceCreateInfo::default(), None)? };
        unsafe { self.device.logical().queue_submit(self.device.graphics_queue(), &subs, upload_fence)?; }
        unsafe { self.device.logical().wait_for_fences(&[upload_fence], true, u64::MAX)?; }
        unsafe { self.device.logical().destroy_fence(upload_fence, None); }

        // Image view
        let view = unsafe {
            self.device.logical().create_image_view(
                &vk::ImageViewCreateInfo::default().image(img).view_type(vk::ImageViewType::TYPE_2D).format(fmt)
                    .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 }), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("tex view: {e}")))?
        };

        // Sampler
        let sampler = unsafe {
            self.device.logical().create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR).min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::REPEAT).address_mode_v(vk::SamplerAddressMode::REPEAT).address_mode_w(vk::SamplerAddressMode::REPEAT)
                    .anisotropy_enable(false).max_anisotropy(1.0)
                    .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
                    .unnormalized_coordinates(false).compare_enable(false).compare_op(vk::CompareOp::ALWAYS)
                    .mipmap_mode(vk::SamplerMipmapMode::LINEAR).mip_lod_bias(0.0).min_lod(0.0).max_lod(0.0),
                None,
            ).map_err(|e| RenderError::DeviceCreation(format!("sampler: {e}")))?
        };

        // Free one-time command buffer
        unsafe { self.device.logical().free_command_buffers(self.transfer_command_pool, &[one_time_cmd]); }

        Ok(GpuTexture { image: img, view, sampler, _allocation: alloc })
    }

    pub fn update_texture_pixels(&self, tex: &GpuTexture, width: u32, height: u32, pixels: &[u8]) -> Result<(), RenderError> {
        let extent = vk::Extent3D { width, height, depth: 1 };
        let staging = self.create_buffer("tex_update", pixels.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, gpu_allocator::MemoryLocation::CpuToGpu)?;
        staging.write(pixels);

        let one_time_cmd = {
            let info = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.transfer_command_pool)
                .level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
            unsafe { self.device.logical().allocate_command_buffers(&info)?.remove(0) }
        };
        unsafe {
            self.device.logical().begin_command_buffer(one_time_cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
        }

        // Transition: shader read → transfer dst
        let barrier = vk::ImageMemoryBarrier::default()
            .image(tex.image).old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL).new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_access_mask(vk::AccessFlags::SHADER_READ).dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        unsafe {
            self.device.logical().cmd_pipeline_barrier(one_time_cmd, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[barrier]);
        }

        let copy_region = vk::BufferImageCopy::default()
            .buffer_offset(0).image_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 }).image_extent(extent);
        unsafe {
            self.device.logical().cmd_copy_buffer_to_image(one_time_cmd, staging.buffer, tex.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_region]);
        }

        // Transition: transfer dst → shader read
        let barrier2 = vk::ImageMemoryBarrier::default()
            .image(tex.image).old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL).new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE).dst_access_mask(vk::AccessFlags::SHADER_READ)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        unsafe {
            self.device.logical().cmd_pipeline_barrier(one_time_cmd, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &[barrier2]);
        }

        unsafe { self.device.logical().end_command_buffer(one_time_cmd)?; }
        let cmds = [one_time_cmd];
        let si = vk::SubmitInfo::default().command_buffers(&cmds);
        let subs = [si];
        let upload_fence = unsafe { self.device.logical().create_fence(&vk::FenceCreateInfo::default(), None)? };
        unsafe { self.device.logical().queue_submit(self.device.graphics_queue(), &subs, upload_fence)?; }
        unsafe { self.device.logical().wait_for_fences(&[upload_fence], true, u64::MAX)?; }
        unsafe { self.device.logical().destroy_fence(upload_fence, None); }

        unsafe { self.device.logical().free_command_buffers(self.transfer_command_pool, &[one_time_cmd]); }
        Ok(())
    }

    pub fn command_pool(&self) -> vk::CommandPool {
        self.command_pool
    }

    pub fn transfer_command_pool(&self) -> vk::CommandPool {
        self.transfer_command_pool
    }

    pub fn create_framebuffer(&self, width: u32, height: u32, format: vk::Format) -> Result<Framebuffer, RenderError> {
        Framebuffer::new(self, width, height, format)
    }

    pub fn update_2d_descriptor_set(
        &self, set: vk::DescriptorSet, ubo: &GpuBuffer, texture: &GpuTexture,
    ) {
        let bi = [vk::DescriptorBufferInfo::default().buffer(ubo.buffer).offset(0).range(ubo.size)];
        let ii = [vk::DescriptorImageInfo::default()
            .image_view(texture.view).sampler(texture.sampler)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let writes = [
            vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&bi),
            vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&ii),
        ];
        unsafe { self.device.logical().update_descriptor_sets(&writes, &[]); }
    }

    pub fn draw_2d(
        &self, cmd: vk::CommandBuffer,
        pipeline: &pipeline::GraphicsPipeline2D,
        vertex_buffer: &GpuBuffer, vertex_count: u32,
        push_constants: &[u8], descriptor_set: vk::DescriptorSet,
    ) {
        let sw = self.swapchain.lock();
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(sw.current_image_view()).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 0.0] } });
        let cas = [ca];
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: sw.extent() })
            .layer_count(1).color_attachments(&cas);

        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            self.device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: sw.extent().height as f32, width: sw.extent().width as f32, height: -(sw.extent().height as f32), min_depth: 0.0, max_depth: 1.0 }]);
            self.device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: sw.extent() }]);
            self.device.logical().cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            self.device.logical().cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[descriptor_set], &[]);
            self.device.logical().cmd_push_constants(cmd, pipeline.layout, vk::ShaderStageFlags::VERTEX, 0, push_constants);
            self.device.logical().cmd_bind_vertex_buffers(cmd, 0, &[vertex_buffer.buffer], &[0u64]);
            self.device.logical().cmd_draw(cmd, vertex_count, 1, 0, 0);
            dr.cmd_end_rendering(cmd);
        }
    }

    pub fn draw_mesh(
        &self, cmd: vk::CommandBuffer,
        pipeline: &pipeline::GraphicsPipeline,
        vertex_buffer: &GpuBuffer, index_buffer: Option<&GpuBuffer>, index_count: u32,
        push_constants: &[u8], descriptor_set: vk::DescriptorSet, depth_buffer: &DepthBuffer,
    ) {
        let sw = self.swapchain.lock();
        let extent = sw.extent();
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(sw.current_image_view()).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.04, 0.04, 0.08, 1.0] } });
        let da = vk::RenderingAttachmentInfoKHR::default()
            .image_view(depth_buffer.view).image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let cas = [ca];
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }).layer_count(1).color_attachments(&cas).depth_attachment(&da);

        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            self.device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: extent.height as f32, width: extent.width as f32, height: -(extent.height as f32), min_depth: 0.0, max_depth: 1.0 }]);
            self.device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]);
            self.device.logical().cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            self.device.logical().cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[descriptor_set], &[]);
            self.device.logical().cmd_push_constants(cmd, pipeline.layout, vk::ShaderStageFlags::VERTEX, 0, push_constants);
            self.device.logical().cmd_bind_vertex_buffers(cmd, 0, &[vertex_buffer.buffer], &[0u64]);
            if let Some(ib) = index_buffer {
                self.device.logical().cmd_bind_index_buffer(cmd, ib.buffer, 0, vk::IndexType::UINT16);
                self.device.logical().cmd_draw_indexed(cmd, index_count, 1, 0, 0, 0);
            } else {
                self.device.logical().cmd_draw(cmd, index_count, 1, 0, 0);
            }
            dr.cmd_end_rendering(cmd);
        }
    }

    pub fn begin_scene_pass(&self, cmd: vk::CommandBuffer, depth_buffer: &DepthBuffer, clear_color: [f32; 4]) {
        let sw = self.swapchain.lock();
        let extent = sw.extent();
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(sw.current_image_view()).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: clear_color } });
        let da = vk::RenderingAttachmentInfoKHR::default()
            .image_view(depth_buffer.view).image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let cas = [ca];
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }).layer_count(1).color_attachments(&cas).depth_attachment(&da);
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            self.device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: extent.height as f32, width: extent.width as f32, height: -(extent.height as f32), min_depth: 0.0, max_depth: 1.0 }]);
            self.device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]);
        }
    }

    pub fn draw_indexed_in_pass(
        &self, cmd: vk::CommandBuffer,
        pipeline: &pipeline::GraphicsPipeline,
        vertex_buffer: &GpuBuffer, index_buffer: Option<&GpuBuffer>, index_count: u32,
        push_constants: &[u8], descriptor_set: vk::DescriptorSet,
    ) {
        unsafe {
            self.device.logical().cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            self.device.logical().cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[descriptor_set], &[]);
            self.device.logical().cmd_push_constants(cmd, pipeline.layout, vk::ShaderStageFlags::VERTEX, 0, push_constants);
            self.device.logical().cmd_bind_vertex_buffers(cmd, 0, &[vertex_buffer.buffer], &[0u64]);
            if let Some(ib) = index_buffer {
                self.device.logical().cmd_bind_index_buffer(cmd, ib.buffer, 0, vk::IndexType::UINT16);
                self.device.logical().cmd_draw_indexed(cmd, index_count, 1, 0, 0, 0);
            } else {
                self.device.logical().cmd_draw(cmd, index_count, 1, 0, 0);
            }
        }
    }

    pub fn end_scene_pass(&self, cmd: vk::CommandBuffer) {
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_end_rendering(cmd);
        }
    }
}

pub struct DepthBuffer {
    pub image: vk::Image, pub view: vk::ImageView,
    _allocation: gpu_allocator::vulkan::Allocation,
}

pub struct GpuTexture {
    pub image: vk::Image, pub view: vk::ImageView, pub sampler: vk::Sampler,
    _allocation: gpu_allocator::vulkan::Allocation,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe { self.device.logical().device_wait_idle().ok(); }
        self.swapchain.lock().destroy(&self.device);
        unsafe {
            self.device.logical().free_command_buffers(self.transfer_command_pool, &[]);
            self.device.logical().destroy_command_pool(self.transfer_command_pool, None);
            self.device.logical().free_command_buffers(self.command_pool, &[]);
            self.device.logical().destroy_command_pool(self.command_pool, None);
            for &fence in &self.in_flight_fences {
                self.device.logical().destroy_fence(fence, None);
            }
        }
    }
}

pub struct Framebuffer {
    pub color_image: vk::Image,
    pub color_view: vk::ImageView,
    pub depth_buffer: DepthBuffer,
    extent: vk::Extent2D,
    _color_allocation: gpu_allocator::vulkan::Allocation,
}

impl Framebuffer {
    pub fn new(renderer: &Renderer, width: u32, height: u32, format: vk::Format) -> Result<Self, RenderError> {
        let extent = vk::Extent2D { width, height };

        let color_img = unsafe {
            renderer.device.logical().create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(format)
                    .extent(vk::Extent3D { width, height, depth: 1 })
                    .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            ).map_err(|e| RenderError::DeviceCreation(format!("color img: {e}")))?
        };

        let color_req = unsafe { renderer.device.logical().get_image_memory_requirements(color_img) };
        let color_alloc = renderer.allocator.lock().allocate("framebuffer_color", color_req, gpu_allocator::MemoryLocation::GpuOnly, false)?;
        unsafe { renderer.device.logical().bind_image_memory(color_img, color_alloc.memory(), color_alloc.offset())?; }

        let color_view = unsafe {
            renderer.device.logical().create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(color_img).view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0, level_count: 1,
                        base_array_layer: 0, layer_count: 1,
                    }),
                None,
            ).map_err(|e| RenderError::DeviceCreation(format!("color view: {e}")))?
        };

        let depth = renderer.create_depth_buffer(extent)?;

        Ok(Self {
            color_image: color_img,
            color_view,
            depth_buffer: depth,
            extent,
            _color_allocation: color_alloc,
        })
    }

    pub fn prepare_rendering(&self, cmd: vk::CommandBuffer, device: &GpuDevice, instance: &VulkanInstance) {
        unsafe {
            // Transition from transfer source back to color attachment
            let barrier = vk::ImageMemoryBarrier::default()
                .image(self.color_image)
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            device.logical().cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, vk::DependencyFlags::empty(), &[], &[], &[barrier]);
        }
        self.begin_rendering(cmd, device, instance);
    }

    pub fn begin_rendering(&self, cmd: vk::CommandBuffer, device: &GpuDevice, instance: &VulkanInstance) {
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(self.color_view).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.1, 0.1, 0.12, 1.0] } });
        let da = vk::RenderingAttachmentInfoKHR::default()
            .image_view(self.depth_buffer.view).image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let cas = [ca];

        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: self.extent })
            .layer_count(1).color_attachments(&cas).depth_attachment(&da);

        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&instance.inner(), device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport {
                x: 0.0, y: self.extent.height as f32,
                width: self.extent.width as f32,
                height: -(self.extent.height as f32),
                min_depth: 0.0, max_depth: 1.0,
            }]);
            device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: self.extent }]);
        }
    }

    pub fn end_rendering(&self, cmd: vk::CommandBuffer, device: &GpuDevice, instance: &VulkanInstance) {
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&instance.inner(), device.logical());
            dr.cmd_end_rendering(cmd);
            // Transition to transfer source for readback
            let barrier = vk::ImageMemoryBarrier::default()
                .image(self.color_image)
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            device.logical().cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[barrier]);
        }
    }

    pub fn copy_to_buffer(&self, device: &GpuDevice, cmd: vk::CommandBuffer, dst_buffer: vk::Buffer) -> Result<(), RenderError> {
        let extent = self.extent;
        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 });
        unsafe {
            device.logical().cmd_copy_image_to_buffer(cmd, self.color_image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, dst_buffer, &[region]);
        }
        Ok(())
    }
}
