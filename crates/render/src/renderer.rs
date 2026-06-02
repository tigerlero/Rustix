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
    pub fn frame_index(&self) -> usize { self.frame_index }
    pub fn device(&self) -> &GpuDevice { &self.device }

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

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod tests;
