use ash::vk;

use crate::instance::VulkanInstance;
use crate::RenderError;

#[derive(Debug, Clone)]
pub struct QueueFamilies {
    pub graphics: Option<u32>,
    pub compute: Option<u32>,
    pub transfer: Option<u32>,
    pub present: Option<u32>,
}

impl QueueFamilies {
    pub fn is_complete(&self) -> bool {
        self.graphics.is_some() && self.present.is_some()
    }

    pub fn unique_indices(&self) -> Vec<u32> {
        let mut indices = Vec::new();
        for &idx in [self.graphics, self.compute, self.transfer, self.present].iter() {
            if let Some(idx) = idx {
                if !indices.contains(&idx) {
                    indices.push(idx);
                }
            }
        }
        indices
    }
}

fn score_physical_device(device: &vk::PhysicalDeviceProperties) -> u32 {
    let mut score = 0u32;
    if device.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
        score += 1000;
    }
    match device.vendor_id {
        0x10DE => score += 500,
        0x1002 => score += 300,
        0x8086 => score += 100,
        _ => {}
    }
    if device.api_version >= vk::API_VERSION_1_3 {
        score += 200;
    } else if device.api_version >= vk::API_VERSION_1_2 {
        score += 100;
    }
    if device.limits.max_image_dimension2_d >= 16384 {
        score += 100;
    }
    score
}

pub struct GpuDevice {
    logical: ash::Device,
    physical: vk::PhysicalDevice,
    physical_properties: vk::PhysicalDeviceProperties,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    queue_families: QueueFamilies,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    transfer_queue: vk::Queue,
    pipeline_cache: vk::PipelineCache,
}

impl GpuDevice {
    pub fn new(
        instance: &VulkanInstance,
        _config: &crate::RenderConfig,
    ) -> Result<Self, RenderError> {
        let physical_devices = unsafe {
            instance
                .inner()
                .enumerate_physical_devices()
                .map_err(|e| RenderError::DeviceCreation(format!("enumerate: {e}")))?
        };

        if physical_devices.is_empty() {
            return Err(RenderError::DeviceCreation(
                "no Vulkan-capable GPUs found".into(),
            ));
        }

        let mut scored: Vec<(u32, vk::PhysicalDevice)> = physical_devices
            .into_iter()
            .map(|dev| {
                let props = unsafe { instance.inner().get_physical_device_properties(dev) };
                (score_physical_device(&props), dev)
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        let (score, physical) = scored.remove(0);
        let physical_properties =
            unsafe { instance.inner().get_physical_device_properties(physical) };
        let memory_properties =
            unsafe { instance.inner().get_physical_device_memory_properties(physical) };

        let device_name = physical_devices_name(&physical_properties);

        tracing::info!(
            gpu = %device_name,
            score = score,
            type_ = ?physical_properties.device_type,
            "selected physical device"
        );

        let queue_families = Self::find_queue_families(instance, physical);

        if !queue_families.is_complete() {
            return Err(RenderError::DeviceCreation(
                "missing required queue families".into(),
            ));
        }

        let priorities = [1.0f32];
        let unique_families = queue_families.unique_indices();
        let queue_create_infos: Vec<vk::DeviceQueueCreateInfo> = unique_families
            .iter()
            .map(|&index| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(index)
                    .queue_priorities(&priorities)
            })
            .collect();

        let mut dynamic_rendering_features =
            vk::PhysicalDeviceDynamicRenderingFeaturesKHR::default()
                .dynamic_rendering(true);

        let mut timeline_semaphore_features =
            vk::PhysicalDeviceTimelineSemaphoreFeatures::default()
                .timeline_semaphore(true);

        let mut synchronization2_features =
            vk::PhysicalDeviceSynchronization2Features::default()
                .synchronization2(true);

        let enabled_features = vk::PhysicalDeviceFeatures::default();

        let device_ext_names = vec![
            ash::khr::swapchain::NAME.as_ptr(),
            ash::khr::dynamic_rendering::NAME.as_ptr(),
            ash::khr::synchronization2::NAME.as_ptr(),
        ];

        let create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_ext_names)
            .enabled_features(&enabled_features)
            .push_next(&mut dynamic_rendering_features)
            .push_next(&mut timeline_semaphore_features)
            .push_next(&mut synchronization2_features);

        let logical = unsafe {
            instance
                .inner()
                .create_device(physical, &create_info, None)
                .map_err(|e| RenderError::DeviceCreation(format!("device: {e}")))?
        };

        let graphics_queue =
            unsafe { logical.get_device_queue(queue_families.graphics.ok_or_else(|| RenderError::DeviceCreation("no graphics queue".into()))?, 0) };
        let present_queue =
            unsafe { logical.get_device_queue(queue_families.present.ok_or_else(|| RenderError::DeviceCreation("no present queue".into()))?, 0) };
        let transfer_queue =
            unsafe { logical.get_device_queue(queue_families.transfer.ok_or_else(|| RenderError::DeviceCreation("no transfer queue".into()))?, 0) };

        let pc_create_info = vk::PipelineCacheCreateInfo::default();
        let pipeline_cache = unsafe {
            logical
                .create_pipeline_cache(&pc_create_info, None)
                .map_err(|e| RenderError::DeviceCreation(format!("pipeline cache: {e}")))?
        };

        Ok(Self {
            logical,
            physical,
            physical_properties,
            memory_properties,
            queue_families,
            graphics_queue,
            present_queue,
            transfer_queue,
            pipeline_cache,
        })
    }

    fn find_queue_families(
        instance: &VulkanInstance,
        physical: vk::PhysicalDevice,
    ) -> QueueFamilies {
        let families = unsafe {
            instance
                .inner()
                .get_physical_device_queue_family_properties(physical)
        };

        let mut result = QueueFamilies {
            graphics: None,
            compute: None,
            transfer: None,
            present: None,
        };

        for (i, family) in families.iter().enumerate() {
            let index = i as u32;

            if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                result.graphics = Some(index);
            }

            if family.queue_flags.contains(vk::QueueFlags::COMPUTE)
                && !family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            {
                result.compute = Some(index);
            }

            if family.queue_flags.contains(vk::QueueFlags::TRANSFER)
                && !family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                && !family.queue_flags.contains(vk::QueueFlags::COMPUTE)
            {
                result.transfer = Some(index);
            }
        }

        if result.compute.is_none() {
            result.compute = result.graphics;
        }
        if result.transfer.is_none() {
            result.transfer = result.graphics;
        }
        result.present = result.graphics;

        result
    }

    pub fn logical(&self) -> &ash::Device { &self.logical }
    pub fn physical(&self) -> vk::PhysicalDevice { self.physical }
    pub fn physical_properties(&self) -> &vk::PhysicalDeviceProperties { &self.physical_properties }
    pub fn memory_properties(&self) -> &vk::PhysicalDeviceMemoryProperties { &self.memory_properties }
    pub fn queue_families(&self) -> &QueueFamilies { &self.queue_families }
    pub fn graphics_queue(&self) -> vk::Queue { self.graphics_queue }
    pub fn present_queue(&self) -> vk::Queue { self.present_queue }
    pub fn transfer_queue(&self) -> vk::Queue { self.transfer_queue }
    pub fn graphics_queue_family_index(&self) -> u32 {
        self.queue_families.graphics.unwrap_or(0)
    }
    pub fn transfer_queue_family_index(&self) -> u32 {
        self.queue_families.transfer.unwrap_or(self.queue_families.graphics.unwrap_or(0))
    }
    pub fn pipeline_cache(&self) -> vk::PipelineCache { self.pipeline_cache }
}

fn physical_devices_name(props: &vk::PhysicalDeviceProperties) -> String {
    let name = &props.device_name;
    let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    let bytes: Vec<u8> = name[..end].iter().map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).to_string()
}

impl Drop for GpuDevice {
    fn drop(&mut self) {
        unsafe {
            self.logical.destroy_pipeline_cache(self.pipeline_cache, None);
            self.logical.destroy_device(None);
        }
    }
}
