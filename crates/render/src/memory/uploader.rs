use ash::vk;
use crate::device::GpuDevice;
use crate::RenderError;

/// Batches GPU uploads and submits them to the dedicated transfer queue.
///
/// On GPUs with a dedicated transfer queue family (e.g. NVIDIA/AMD discrete),
/// this offloads buffer and texture uploads from the graphics queue so
/// render work is not stalled by large asset uploads.
pub struct GpuUploader {
    transfer_pool: vk::CommandPool,
    transfer_queue: vk::Queue,
    /// In-flight command buffers waiting on their fences.
    pending: Vec<(vk::CommandBuffer, vk::Fence)>,
}

impl GpuUploader {
    pub fn new(device: &GpuDevice) -> Result<Self, RenderError> {
        let pool = unsafe {
            device.logical().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(device.transfer_queue_family_index())
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            ).map_err(|e| RenderError::DeviceCreation(format!("transfer pool: {e}")))?
        };
        Ok(Self {
            transfer_pool: pool,
            transfer_queue: device.transfer_queue(),
            pending: Vec::new(),
        })
    }

    /// Allocate and begin a one-time-submit command buffer for transfer work.
    pub fn begin(&self, device: &ash::Device) -> Result<vk::CommandBuffer, RenderError> {
        let info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.transfer_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd = unsafe {
            device.allocate_command_buffers(&info)?
                .into_iter()
                .next()
                .ok_or_else(|| RenderError::DeviceCreation("no cmd buffer allocated".into()))?
        };
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { device.begin_command_buffer(cmd, &begin_info)?; }
        Ok(cmd)
    }

    /// End the command buffer, create a fence, and submit to the transfer queue.
    /// Returns the fence so the caller can wait or poll.
    pub fn submit(&mut self, device: &ash::Device, cmd: vk::CommandBuffer) -> Result<vk::Fence, RenderError> {
        unsafe { device.end_command_buffer(cmd)?; }
        let fence = unsafe {
            device.create_fence(&vk::FenceCreateInfo::default(), None)
                .map_err(|e| RenderError::DeviceCreation(format!("upload fence: {e}")))?
        };
        let cmds = [cmd];
        let si = vk::SubmitInfo::default().command_buffers(&cmds);
        unsafe {
            device.queue_submit(self.transfer_queue, &[si], fence)
                .map_err(|e| RenderError::DeviceCreation(format!("transfer submit: {e}")))?;
        }
        self.pending.push((cmd, fence));
        Ok(fence)
    }

    /// Wait for all in-flight uploads to complete and reclaim command buffers.
    pub fn wait_idle(&mut self, device: &ash::Device) {
        for &(cmd, fence) in &self.pending {
            unsafe {
                let _ = device.wait_for_fences(&[fence], true, u64::MAX);
                device.destroy_fence(fence, None);
                device.free_command_buffers(self.transfer_pool, &[cmd]);
            }
        }
        self.pending.clear();
    }

    /// Poll fences and reclaim completed command buffers. Returns number completed.
    pub fn poll_completed(&mut self, device: &ash::Device) -> usize {
        let mut completed = 0usize;
        let mut remaining = Vec::new();
        for (cmd, fence) in self.pending.drain(..) {
            let done = unsafe {
                device.wait_for_fences(&[fence], true, 0).is_ok()
            };
            if done {
                unsafe {
                    device.destroy_fence(fence, None);
                    device.free_command_buffers(self.transfer_pool, &[cmd]);
                }
                completed += 1;
            } else {
                remaining.push((cmd, fence));
            }
        }
        self.pending = remaining;
        completed
    }

    /// Number of in-flight uploads.
    pub fn in_flight_count(&self) -> usize {
        self.pending.len()
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        self.wait_idle(device);
        unsafe { device.destroy_command_pool(self.transfer_pool, None); }
    }
}

impl Drop for GpuUploader {
    fn drop(&mut self) {
        // Non-owning drop — caller must call destroy() or wait_idle() before drop.
        // The command pool is owned by this struct, but we can't access ash::Device
        // safely in a raw-pointer drop. In practice the renderer calls destroy().
    }
}
