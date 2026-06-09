use ash::vk;
use crate::device::GpuDevice;
use crate::instance::VulkanInstance;
use crate::error::RenderError;

const MAX_TIMESTAMPS_PER_FRAME: u32 = 8;
const FRAME_COUNT: u32 = 3;

/// GPU profiler using Vulkan timestamp queries.
pub struct GpuProfiler {
    pub(crate) query_pool: vk::QueryPool,
    period_ns: f64,
    labels: Vec<String>,
    next_idx: u32,
}

impl GpuProfiler {
    pub fn new(instance: &VulkanInstance, device: &GpuDevice) -> Result<Self, RenderError> {
        let props = unsafe { instance.inner().get_physical_device_properties(device.physical()) };
        let period_ns = props.limits.timestamp_period as f64;

        let pool_info = vk::QueryPoolCreateInfo::default()
            .query_type(vk::QueryType::TIMESTAMP)
            .query_count(MAX_TIMESTAMPS_PER_FRAME * FRAME_COUNT);
        let query_pool = unsafe {
            device.logical().create_query_pool(&pool_info, None)
                .map_err(|e| RenderError::DeviceCreation(format!("profiler query pool: {e}")))?
        };

        Ok(Self { query_pool, period_ns, labels: Vec::new(), next_idx: 0 })
    }

    fn base_index(&self, frame_idx: usize) -> u32 {
        (frame_idx % FRAME_COUNT as usize) as u32 * MAX_TIMESTAMPS_PER_FRAME
    }

    /// Reset queries for the given frame slot. Call at the top of the command buffer.
    pub fn reset(&mut self, cmd: vk::CommandBuffer, frame_idx: usize, device: &GpuDevice) {
        let base = self.base_index(frame_idx);
        unsafe {
            device.logical().cmd_reset_query_pool(cmd, self.query_pool, base, MAX_TIMESTAMPS_PER_FRAME);
        }
        self.next_idx = 0;
        self.labels.clear();
    }

    /// Write a timestamp into the command buffer.
    pub fn timestamp(&mut self, cmd: vk::CommandBuffer, frame_idx: usize, label: &str, device: &GpuDevice) {
        if self.next_idx >= MAX_TIMESTAMPS_PER_FRAME {
            return;
        }
        let idx = self.base_index(frame_idx) + self.next_idx;
        self.labels.push(label.to_string());
        unsafe {
            device.logical().cmd_write_timestamp(
                cmd,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.query_pool,
                idx,
            );
        }
        self.next_idx += 1;
    }

    /// Read back results for the previous frame. Returns (label, micros) pairs.
    pub fn readback(&self, frame_idx: usize, device: &GpuDevice) -> Vec<(String, f64)> {
        let base = self.base_index(frame_idx);
        let count = self.next_idx.min(MAX_TIMESTAMPS_PER_FRAME);
        if count < 2 {
            return Vec::new();
        }
        let mut results = vec![0u64; count as usize];
        let ok = unsafe {
            device.logical().get_query_pool_results(
                self.query_pool,
                base,
                &mut results,
                vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT,
            )
        };
        if ok.is_err() {
            return Vec::new();
        }

        let mut pairs = Vec::new();
        for i in 1..count as usize {
            let delta = results[i].saturating_sub(results[i - 1]);
            let us = (delta as f64 * self.period_ns) / 1000.0;
            let label = if i < self.labels.len() {
                format!("{} -> {}", self.labels[i - 1], self.labels[i])
            } else {
                format!("segment_{}", i)
            };
            pairs.push((label, us));
        }
        pairs
    }
}

impl Drop for GpuProfiler {
    fn drop(&mut self) {
        // Query pool destruction is done via renderer drop
    }
}

