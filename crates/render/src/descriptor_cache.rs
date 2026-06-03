use ash::vk;
use parking_lot::Mutex;
use std::collections::HashMap;

use crate::RenderError;

/// Hashable key representing a descriptor set layout configuration.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LayoutBindingKey {
    binding: u32,
    descriptor_type: vk::DescriptorType,
    descriptor_count: u32,
    stage_flags: vk::ShaderStageFlags,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LayoutKey {
    flags: vk::DescriptorSetLayoutCreateFlags,
    bindings: Vec<LayoutBindingKey>,
    binding_flags: Vec<vk::DescriptorBindingFlags>,
}

/// Caches `vk::DescriptorSetLayout` objects keyed by their binding configuration.
///
/// Use `get_or_create` to obtain a layout for a given set of bindings. The cache
/// retains the Vulkan handle until the cache itself is dropped.
pub struct DescriptorSetLayoutCache {
    device: *const ash::Device,
    cache: Mutex<HashMap<LayoutKey, vk::DescriptorSetLayout>>,
}

unsafe impl Send for DescriptorSetLayoutCache {}
unsafe impl Sync for DescriptorSetLayoutCache {}

impl DescriptorSetLayoutCache {
    pub fn new(device: &ash::Device) -> Self {
        Self {
            device: device as *const ash::Device,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a `vk::DescriptorSetLayout` for the given bindings.
    ///
    /// `flags` are the `vk::DescriptorSetLayoutCreateFlags` for the layout.
    /// `binding_flags_opt` is an optional slice of per-binding flags (e.g. `PARTIALLY_BOUND`).
    /// If provided, its length must match `bindings`.
    pub fn get_or_create(
        &self,
        bindings: &[vk::DescriptorSetLayoutBinding],
        flags: vk::DescriptorSetLayoutCreateFlags,
        binding_flags_opt: Option<&[vk::DescriptorBindingFlags]>,
    ) -> Result<vk::DescriptorSetLayout, RenderError> {
        let key = Self::make_key(bindings, flags, binding_flags_opt);

        {
            let cache = self.cache.lock();
            if let Some(&layout) = cache.get(&key) {
                return Ok(layout);
            }
        }

        let layout = unsafe {
            let mut ci = vk::DescriptorSetLayoutCreateInfo::default()
                .bindings(bindings)
                .flags(flags);

            let mut bf: Vec<vk::DescriptorBindingFlags> = Vec::new();
            let mut binding_flags_info = if let Some(binding_flags) = binding_flags_opt {
                bf = binding_flags.to_vec();
                Some(
                    vk::DescriptorSetLayoutBindingFlagsCreateInfo::default()
                        .binding_flags(&bf),
                )
            } else {
                None
            };

            if let Some(ref mut bfi) = binding_flags_info {
                ci = ci.push_next(bfi);
            }

            (*self.device)
                .create_descriptor_set_layout(&ci, None)
                .map_err(|e| {
                    RenderError::DeviceCreation(format!("descriptor set layout cache: {e}"))
                })?
        };

        let mut cache = self.cache.lock();
        // Another thread may have created it in the meantime.
        if let Some(&existing) = cache.get(&key) {
            unsafe {
                (*self.device).destroy_descriptor_set_layout(layout, None);
            }
            return Ok(existing);
        }

        cache.insert(key, layout);
        Ok(layout)
    }

    /// Convenience wrapper without per-binding flags.
    pub fn get_or_create_simple(
        &self,
        bindings: &[vk::DescriptorSetLayoutBinding],
    ) -> Result<vk::DescriptorSetLayout, RenderError> {
        self.get_or_create(bindings, vk::DescriptorSetLayoutCreateFlags::empty(), None)
    }

    fn make_key(
        bindings: &[vk::DescriptorSetLayoutBinding],
        flags: vk::DescriptorSetLayoutCreateFlags,
        binding_flags_opt: Option<&[vk::DescriptorBindingFlags]>,
    ) -> LayoutKey {
        let binding_keys: Vec<LayoutBindingKey> = bindings
            .iter()
            .map(|b| LayoutBindingKey {
                binding: b.binding,
                descriptor_type: b.descriptor_type,
                descriptor_count: b.descriptor_count,
                stage_flags: b.stage_flags,
            })
            .collect();

        let binding_flags = binding_flags_opt
            .map(|bf| bf.to_vec())
            .unwrap_or_default();

        LayoutKey {
            flags,
            bindings: binding_keys,
            binding_flags,
        }
    }
}

impl Drop for DescriptorSetLayoutCache {
    fn drop(&mut self) {
        unsafe {
            if !self.device.is_null() {
                let dev = &*self.device;
                let cache = self.cache.get_mut();
                for &layout in cache.values() {
                    if layout != vk::DescriptorSetLayout::null() {
                        dev.destroy_descriptor_set_layout(layout, None);
                    }
                }
            }
        }
    }
}
