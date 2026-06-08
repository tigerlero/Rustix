use ash::vk;

use crate::descriptor_cache::DescriptorSetLayoutCache;
use crate::pipeline::PipelineLayoutCache;
use crate::sampler_cache::SamplerCache;
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
    logical: Box<ash::Device>,
    physical: vk::PhysicalDevice,
    physical_properties: vk::PhysicalDeviceProperties,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    queue_families: QueueFamilies,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    transfer_queue: vk::Queue,
    compute_queue: vk::Queue,
    pipeline_cache: vk::PipelineCache,
    descriptor_layout_cache: DescriptorSetLayoutCache,
    pipeline_layout_cache: PipelineLayoutCache,
    sampler_cache: SamplerCache,
    mesh_shader_supported: bool,
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

        let available_extensions = unsafe {
            instance.inner().enumerate_device_extension_properties(physical)
                .unwrap_or_default()
        };
        let has_nv_mesh_shader = available_extensions.iter().any(|ext| {
            let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
            name.to_bytes() == ash::nv::mesh_shader::NAME.to_bytes()
        });
        if has_nv_mesh_shader {
            tracing::info!("VK_NV_mesh_shader extension available");
        }

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

        let mut device_ext_names: Vec<*const i8> = vec![
            ash::khr::swapchain::NAME.as_ptr(),
            ash::khr::dynamic_rendering::NAME.as_ptr(),
            ash::khr::synchronization2::NAME.as_ptr(),
            ash::ext::descriptor_indexing::NAME.as_ptr(),
        ];
        if has_nv_mesh_shader {
            device_ext_names.push(ash::nv::mesh_shader::NAME.as_ptr());
        }

        let mut descriptor_indexing_features =
            vk::PhysicalDeviceDescriptorIndexingFeatures::default()
                .shader_sampled_image_array_non_uniform_indexing(true)
                .descriptor_binding_partially_bound(true)
                .descriptor_binding_update_unused_while_pending(true)
                .descriptor_binding_sampled_image_update_after_bind(true)
                .descriptor_binding_variable_descriptor_count(true);

        let mut mesh_shader_features =
            vk::PhysicalDeviceMeshShaderFeaturesNV::default()
                .mesh_shader(true)
                .task_shader(true);

        let create_info = if has_nv_mesh_shader {
            vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&device_ext_names)
                .enabled_features(&enabled_features)
                .push_next(&mut dynamic_rendering_features)
                .push_next(&mut timeline_semaphore_features)
                .push_next(&mut synchronization2_features)
                .push_next(&mut descriptor_indexing_features)
                .push_next(&mut mesh_shader_features)
        } else {
            vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&device_ext_names)
                .enabled_features(&enabled_features)
                .push_next(&mut dynamic_rendering_features)
                .push_next(&mut timeline_semaphore_features)
                .push_next(&mut synchronization2_features)
                .push_next(&mut descriptor_indexing_features)
        };

        let logical = Box::new(unsafe {
            instance
                .inner()
                .create_device(physical, &create_info, None)
                .map_err(|e| RenderError::DeviceCreation(format!("device: {e}")))?
        });

        let graphics_queue =
            unsafe { logical.get_device_queue(queue_families.graphics.ok_or_else(|| RenderError::DeviceCreation("no graphics queue".into()))?, 0) };
        let present_queue =
            unsafe { logical.get_device_queue(queue_families.present.ok_or_else(|| RenderError::DeviceCreation("no present queue".into()))?, 0) };
        let transfer_queue =
            unsafe { logical.get_device_queue(queue_families.transfer.ok_or_else(|| RenderError::DeviceCreation("no transfer queue".into()))?, 0) };
        let compute_queue =
            unsafe { logical.get_device_queue(queue_families.compute.ok_or_else(|| RenderError::DeviceCreation("no compute queue".into()))?, 0) };

        let pc_create_info = vk::PipelineCacheCreateInfo::default();
        let pipeline_cache = unsafe {
            logical
                .create_pipeline_cache(&pc_create_info, None)
                .map_err(|e| RenderError::DeviceCreation(format!("pipeline cache: {e}")))?
        };

        let descriptor_layout_cache = DescriptorSetLayoutCache::new(&*logical);
        let pipeline_layout_cache = PipelineLayoutCache::new(&*logical);
        let sampler_cache = SamplerCache::new(&*logical);
        Ok(Self {
            logical,
            physical,
            physical_properties,
            memory_properties,
            queue_families,
            graphics_queue,
            present_queue,
            transfer_queue,
            compute_queue,
            pipeline_cache,
            descriptor_layout_cache,
            pipeline_layout_cache,
            sampler_cache,
            mesh_shader_supported: has_nv_mesh_shader,
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

    pub fn logical(&self) -> &ash::Device { self.logical.as_ref() }
    pub fn physical(&self) -> vk::PhysicalDevice { self.physical }
    pub fn physical_properties(&self) -> &vk::PhysicalDeviceProperties { &self.physical_properties }
    pub fn memory_properties(&self) -> &vk::PhysicalDeviceMemoryProperties { &self.memory_properties }
    pub fn queue_families(&self) -> &QueueFamilies { &self.queue_families }
    pub fn graphics_queue(&self) -> vk::Queue { self.graphics_queue }
    pub fn present_queue(&self) -> vk::Queue { self.present_queue }
    pub fn transfer_queue(&self) -> vk::Queue { self.transfer_queue }
    pub fn compute_queue(&self) -> vk::Queue { self.compute_queue }
    pub fn graphics_queue_family_index(&self) -> u32 {
        self.queue_families.graphics.unwrap_or(0)
    }
    pub fn transfer_queue_family_index(&self) -> u32 {
        self.queue_families.transfer.unwrap_or(self.queue_families.graphics.unwrap_or(0))
    }
    pub fn compute_queue_family_index(&self) -> u32 {
        self.queue_families.compute.unwrap_or(self.queue_families.graphics.unwrap_or(0))
    }
    pub fn pipeline_cache(&self) -> vk::PipelineCache { self.pipeline_cache }
    pub fn descriptor_layout_cache(&self) -> &DescriptorSetLayoutCache { &self.descriptor_layout_cache }
    pub fn pipeline_layout_cache(&self) -> &PipelineLayoutCache { &self.pipeline_layout_cache }
    pub fn sampler_cache(&self) -> &SamplerCache { &self.sampler_cache }
    pub fn mesh_shader_supported(&self) -> bool { self.mesh_shader_supported }
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
            // descriptor_layout_cache must be dropped before the logical device
            // so its layouts are destroyed while the device is still valid.
            std::ptr::drop_in_place(&mut self.descriptor_layout_cache);
            std::ptr::drop_in_place(&mut self.pipeline_layout_cache);
            std::ptr::drop_in_place(&mut self.sampler_cache);
            self.logical.destroy_pipeline_cache(self.pipeline_cache, None);
            self.logical.destroy_device(None);
        }
    }
}
