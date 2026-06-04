use std::sync::Arc;
use ash::vk;
use parking_lot::Mutex;
use crate::instance::VulkanInstance;
use crate::device::GpuDevice;
use crate::memory::GpuMemoryAllocator;
use crate::swapchain::Swapchain;
use crate::surface;
use crate::pipeline;
use crate::texture::Framebuffer;
use crate::error::RenderError;
use crate::profiler::GpuProfiler;
use crate::bindless::BindlessDescriptorHeap;
use crate::pipeline::GraphicsPipelineVariantCache;
use crate::descriptor_allocator::DescriptorSetAllocator;
use rustix_core::config::RenderConfig;

mod resource;
mod texture;
mod draw;

pub struct Renderer {
    pub instance: Arc<VulkanInstance>,
    pub device: Arc<GpuDevice>,
    pub swapchain: Arc<Mutex<Swapchain>>,
    pub allocator: Arc<Mutex<GpuMemoryAllocator>>,
    command_pool: vk::CommandPool,
    transfer_command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    frame_complete_semaphore: vk::Semaphore,
    profiler: Option<GpuProfiler>,
    bindless_heap: BindlessDescriptorHeap,
    pipeline_variant_cache: GraphicsPipelineVariantCache,
    descriptor_allocator: Mutex<DescriptorSetAllocator>,
    hot_reloader: Option<crate::hot_reload::ShaderHotReloader>,
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
        let mut timeline_create = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        let sem_create = vk::SemaphoreCreateInfo::default().push_next(&mut timeline_create);
        let frame_complete_semaphore = unsafe {
            device.logical().create_semaphore(&sem_create, None).expect("timeline semaphore")
        };
        let profiler = GpuProfiler::new(&instance, &device).ok();
        let bindless_heap = BindlessDescriptorHeap::new(device.logical())?;
        let pipeline_variant_cache = GraphicsPipelineVariantCache::new(
            device.logical(),
            bindless_heap.layout(),
        );
        let desc_pool_sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 128 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLED_IMAGE, descriptor_count: 128 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLER, descriptor_count: 128 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: 64 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: 32 },
        ];
        let descriptor_allocator = Mutex::new(DescriptorSetAllocator::new(device.logical(), &desc_pool_sizes, 256)?);
        let hot_reloader = crate::hot_reload::ShaderHotReloader::new().ok();
        if hot_reloader.is_none() {
            tracing::warn!("shader hot-reload disabled: could not start file watcher");
        }
        Ok(Self { instance, device, swapchain: Arc::new(Mutex::new(Swapchain::new())), allocator: Arc::new(Mutex::new(allocator)), command_pool: cmd_pool, transfer_command_pool: transfer_pool, command_buffers: cmd_bufs, frame_complete_semaphore, profiler, bindless_heap, pipeline_variant_cache, descriptor_allocator, hot_reloader, frame_index: 0, initialized: false })
    }

    pub fn init_surface(&mut self, rw: raw_window_handle::RawWindowHandle, rd: raw_window_handle::RawDisplayHandle, w: u32, h: u32) -> Result<(), RenderError> {
        let s = surface::create_surface(&self.instance, rw, rd)?;
        self.swapchain.lock().init(&self.instance, &self.device, s, w, h)?;
        self.initialized = true;
        Ok(())
    }

    pub fn begin_frame(&mut self) -> Result<bool, RenderError> {
        if !self.initialized { return Ok(false); }
        // Recycle descriptor pools from previous frames.
        self.reset_descriptor_pools();
        // Read back profiler results for the frame we just waited on.
        if self.frame_index >= 3 {
            let wait_value = (self.frame_index - 2) as u64;
            let wait_sems = [self.frame_complete_semaphore];
            let wait_values = [wait_value];
            let wait_info = vk::SemaphoreWaitInfo::default()
                .semaphores(&wait_sems)
                .values(&wait_values);
            unsafe { self.device.logical().wait_semaphores(&wait_info, u64::MAX)?; }
            if let Some(ref profiler) = self.profiler {
                let results = profiler.readback(self.frame_index - 2, &self.device);
                for (label, us) in results {
                    tracing::trace!(target: "gpu_profile", "{label}: {us:.2} µs");
                }
            }
        }
        self.swapchain.lock().acquire_next_image(&self.device, self.frame_index)?;
        Ok(true)
    }

    pub fn profiler_begin(&mut self, cmd: vk::CommandBuffer) {
        if let Some(ref mut profiler) = self.profiler {
            profiler.reset(cmd, self.frame_index, &self.device);
            profiler.timestamp(cmd, self.frame_index, "frame_begin", &self.device);
        }
    }

    pub fn end_frame(&mut self) -> Result<(), RenderError> {
        if !self.initialized { return Ok(()); }
        let cmd = self.current_cmd();
        if let Some(ref mut profiler) = self.profiler {
            profiler.timestamp(cmd, self.frame_index, "frame_end", &self.device);
        }
        let frame_idx = self.frame_index;
        // Transition swapchain image to PRESENT_SRC_KHR before presenting.
        unsafe {
            let sw = self.swapchain.lock();
            let dst_image = sw.current_image();
            drop(sw);
            let barrier = vk::ImageMemoryBarrier2::default()
                .image(dst_image)
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags2::BOTTOM_OF_PIPE)
                .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags2::NONE)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0, level_count: 1,
                    base_array_layer: 0, layer_count: 1,
                });
            let barriers = [barrier];
            let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
            self.device.logical().cmd_pipeline_barrier2(cmd, &dep);
        }
        unsafe { self.device.logical().end_command_buffer(cmd)?; }
        let sw = self.swapchain.lock();
        let ws = [sw.image_available_semaphore(frame_idx)];
        let ss = [sw.render_finished_semaphore(frame_idx)];
        drop(sw);
        let wst = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT]; let cmds = [cmd];
        let signal_value = (frame_idx + 1) as u64;
        let signal_values = [0u64, signal_value]; // 0 for binary semaphore, value for timeline
        let mut timeline_si = vk::TimelineSemaphoreSubmitInfo::default()
            .signal_semaphore_values(&signal_values);
        let signal_sems = [ss[0], self.frame_complete_semaphore];
        let si = vk::SubmitInfo::default()
            .wait_semaphores(&ws)
            .wait_dst_stage_mask(&wst)
            .command_buffers(&cmds)
            .signal_semaphores(&signal_sems)
            .push_next(&mut timeline_si);
        let subs = [si];
        unsafe { self.device.logical().queue_submit(self.device.graphics_queue(), &subs, vk::Fence::null())?; }
        let ok = self.swapchain.lock().present(&self.device, &ss)?;
        if !ok { self.swapchain.lock().recreate(&self.instance, &self.device)?; }
        self.frame_index = self.frame_index.wrapping_add(1);
        Ok(())
    }

    pub fn current_cmd(&self) -> vk::CommandBuffer { self.command_buffers[self.frame_index % 3] }
    pub fn frame_index(&self) -> usize { self.frame_index }
    pub fn device(&self) -> &GpuDevice { &self.device }
    pub fn bindless_heap(&self) -> &BindlessDescriptorHeap { &self.bindless_heap }
    pub fn bindless_heap_mut(&mut self) -> &mut BindlessDescriptorHeap { &mut self.bindless_heap }
    pub fn pipeline_variant_cache(&self) -> &GraphicsPipelineVariantCache { &self.pipeline_variant_cache }
    pub fn hot_reloader(&self) -> Option<&crate::hot_reload::ShaderHotReloader> { self.hot_reloader.as_ref() }

    /// Clear all cached graphics pipeline variants. Call after shader hot-reload.
    pub fn clear_pipeline_cache(&self) {
        self.pipeline_variant_cache.clear();
    }

    /// Allocate a descriptor set through the shared pool recycler.
    pub fn allocate_descriptor_set(&self, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet, RenderError> {
        self.descriptor_allocator.lock().allocate(layout)
    }

    /// Reset all descriptor pools so they can be reused next frame.
    pub fn reset_descriptor_pools(&self) {
        self.descriptor_allocator.lock().reset_pools();
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
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe { self.device.logical().device_wait_idle().ok(); }
        self.swapchain.lock().destroy(&self.device);
        unsafe {
            if let Some(ref profiler) = self.profiler {
                self.device.logical().destroy_query_pool(profiler.query_pool, None);
            }
            self.device.logical().free_command_buffers(self.transfer_command_pool, &[]);
            self.device.logical().destroy_command_pool(self.transfer_command_pool, None);
            self.device.logical().free_command_buffers(self.command_pool, &[]);
            self.device.logical().destroy_command_pool(self.command_pool, None);
            self.device.logical().destroy_semaphore(self.frame_complete_semaphore, None);
        }
    }
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod tests;
