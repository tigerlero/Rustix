//! Secondary command buffers for multi-threaded command recording.

use ash::vk;

/// A pool of secondary command buffers for parallel recording.
pub struct SecondaryCommandPool {
    pub pool: vk::CommandPool,
    pub buffers: Vec<vk::CommandBuffer>,
    pub capacity: u32,
}

impl SecondaryCommandPool {
    pub fn new(device: &ash::Device, queue_family: u32, capacity: u32) -> Self {
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let pool = unsafe { device.create_command_pool(&pool_info, None).unwrap() };

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(pool)
            .level(vk::CommandBufferLevel::SECONDARY)
            .command_buffer_count(capacity);
        let buffers = unsafe { device.allocate_command_buffers(&alloc_info).unwrap() };

        Self {
            pool,
            buffers,
            capacity,
        }
    }

    /// Begin a secondary command buffer for inline rendering.
    pub fn begin_secondary(
        &self,
        device: &ash::Device,
        buffer: vk::CommandBuffer,
        render_pass: vk::RenderPass,
        framebuffer: vk::Framebuffer,
    ) {
        let inheritance = vk::CommandBufferInheritanceInfo::default()
            .render_pass(render_pass)
            .framebuffer(framebuffer)
            .subpass(0);
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT | vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
            .inheritance_info(&inheritance);
        unsafe {
            device.begin_command_buffer(buffer, &begin_info).unwrap();
        }
    }

    pub fn end_secondary(&self, device: &ash::Device, buffer: vk::CommandBuffer) {
        unsafe {
            device.end_command_buffer(buffer).unwrap();
        }
    }

    pub fn execute_secondary(&self, cmd: vk::CommandBuffer, buffer: vk::CommandBuffer, device: &ash::Device) {
        unsafe {
            device.cmd_execute_commands(cmd, &[buffer]);
        }
    }
}
