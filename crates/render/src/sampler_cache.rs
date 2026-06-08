use ash::vk;
use parking_lot::Mutex;
use std::collections::HashMap;

use crate::RenderError;

/// Hashable key derived from `vk::SamplerCreateInfo` fields that affect sampler behaviour.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SamplerKey {
    pub(crate) mag_filter: vk::Filter,
    pub(crate) min_filter: vk::Filter,
    pub(crate) mipmap_mode: vk::SamplerMipmapMode,
    pub(crate) address_mode_u: vk::SamplerAddressMode,
    pub(crate) address_mode_v: vk::SamplerAddressMode,
    pub(crate) address_mode_w: vk::SamplerAddressMode,
    pub(crate) mip_lod_bias: u32, // f32 as bits for hash
    pub(crate) anisotropy_enable: bool,
    pub(crate) max_anisotropy: u32, // f32 as bits for hash
    pub(crate) compare_enable: bool,
    pub(crate) compare_op: vk::CompareOp,
    pub(crate) min_lod: u32, // f32 as bits for hash
    pub(crate) max_lod: u32, // f32 as bits for hash
    pub(crate) border_color: vk::BorderColor,
    pub(crate) unnormalized_coordinates: bool,
}

impl SamplerKey {
    pub(crate) fn from_info(info: &vk::SamplerCreateInfo) -> Self {
        // ash stores f32 fields as f32; we hash the bits for stability.
        Self {
            mag_filter: info.mag_filter,
            min_filter: info.min_filter,
            mipmap_mode: info.mipmap_mode,
            address_mode_u: info.address_mode_u,
            address_mode_v: info.address_mode_v,
            address_mode_w: info.address_mode_w,
            mip_lod_bias: info.mip_lod_bias.to_bits(),
            anisotropy_enable: info.anisotropy_enable != 0,
            max_anisotropy: info.max_anisotropy.to_bits(),
            compare_enable: info.compare_enable != 0,
            compare_op: info.compare_op,
            min_lod: info.min_lod.to_bits(),
            max_lod: info.max_lod.to_bits(),
            border_color: info.border_color,
            unnormalized_coordinates: info.unnormalized_coordinates != 0,
        }
    }
}

/// Caches `vk::Sampler` objects keyed by their creation parameters.
///
/// Use `get_or_create` to obtain a sampler for a given `SamplerCreateInfo`.
/// The cache retains the Vulkan handle until the cache itself is dropped.
pub struct SamplerCache {
    device: *const ash::Device,
    cache: Mutex<HashMap<SamplerKey, vk::Sampler>>,
}

unsafe impl Send for SamplerCache {}
unsafe impl Sync for SamplerCache {}

impl SamplerCache {
    pub fn new(device: &ash::Device) -> Self {
        Self {
            device: device as *const ash::Device,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a `vk::Sampler` matching the given `SamplerCreateInfo`.
    pub fn get_or_create(
        &self,
        info: &vk::SamplerCreateInfo,
    ) -> Result<vk::Sampler, RenderError> {
        let key = SamplerKey::from_info(info);

        {
            let cache = self.cache.lock();
            if let Some(&sampler) = cache.get(&key) {
                return Ok(sampler);
            }
        }

        let sampler = unsafe {
            (*self.device)
                .create_sampler(info, None)
                .map_err(|e| RenderError::DeviceCreation(format!("sampler cache: {e}")))?
        };

        let mut cache = self.cache.lock();
        // Another thread may have created it in the meantime.
        if let Some(&existing) = cache.get(&key) {
            unsafe {
                (*self.device).destroy_sampler(sampler, None);
            }
            return Ok(existing);
        }

        cache.insert(key, sampler);
        Ok(sampler)
    }
}

impl Drop for SamplerCache {
    fn drop(&mut self) {
        unsafe {
            if !self.device.is_null() {
                let dev = &*self.device;
                let cache = self.cache.get_mut();
                for &sampler in cache.values() {
                    if sampler != vk::Sampler::null() {
                        dev.destroy_sampler(sampler, None);
                    }
                }
            }
        }
    }
}
