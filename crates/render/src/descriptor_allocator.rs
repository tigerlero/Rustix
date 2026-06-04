use ash::vk;
use crate::error::RenderError;

/// Descriptor-set allocator that recycles `VkDescriptorPool` objects instead of
/// creating a new pool for every allocation.
///
/// Pools are created with a fixed capacity. When a pool is exhausted the
/// allocator grabs another pool from the *ready* list or creates a new one.
/// Call [`reset_pools`] once per frame (or once all in-flight frames have
/// finished) to return all used pools to the ready list.
pub struct DescriptorSetAllocator {
    device: *const ash::Device,
    /// Pools that have been handed out this cycle and will be reset next call.
    used_pools: Vec<vk::DescriptorPool>,
    /// Pools that have been reset and are available for allocation.
    ready_pools: Vec<vk::DescriptorPool>,
    /// Sizes used when creating new pools.
    pool_sizes: Vec<vk::DescriptorPoolSize>,
    max_sets_per_pool: u32,
}

impl DescriptorSetAllocator {
    pub fn new(
        device: &ash::Device,
        pool_sizes: &[vk::DescriptorPoolSize],
        max_sets_per_pool: u32,
    ) -> Result<Self, RenderError> {
        let mut alloc = Self {
            device: device as *const ash::Device,
            used_pools: Vec::new(),
            ready_pools: Vec::new(),
            pool_sizes: pool_sizes.to_vec(),
            max_sets_per_pool,
        };
        // Pre-allocate one pool so the first `allocate` never fails.
        let pool = alloc.create_pool()?;
        alloc.ready_pools.push(pool);
        Ok(alloc)
    }

    /// Allocate a single descriptor set with the given layout.
    ///
    /// If the current pool is full a new pool is obtained (from the ready list
    /// or created on the fly) and the allocation is retried.
    pub fn allocate(&mut self, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet, RenderError> {
        let ls = [layout];
        let ai = vk::DescriptorSetAllocateInfo::default()
            .set_layouts(&ls);

        loop {
            let pool = self.current_pool()?;
            let ai = ai.clone().descriptor_pool(pool);
            let result = unsafe {
                (*self.device).allocate_descriptor_sets(&ai)
            };
            match result {
                Ok(mut sets) => return Ok(sets.remove(0)),
                Err(vk::Result::ERROR_OUT_OF_POOL_MEMORY) | Err(vk::Result::ERROR_FRAGMENTED_POOL) => {
                    // Pool is full — move it to used and try again with a fresh pool.
                    tracing::trace!("DescriptorSetAllocator: pool exhausted, grabbing next pool");
                    continue;
                }
                Err(e) => return Err(RenderError::DeviceCreation(format!("descriptor set alloc: {e}"))),
            }
        }
    }

    /// Reset all pools that have been used since the last call and move them
    /// back to the ready list.
    pub fn reset_pools(&mut self) {
        for &pool in &self.used_pools {
            unsafe {
                let _ = (*self.device).reset_descriptor_pool(pool, vk::DescriptorPoolResetFlags::empty());
            }
        }
        self.ready_pools.append(&mut self.used_pools);
    }

    /// Destroy every pool managed by this allocator.
    pub fn destroy(&mut self) {
        unsafe {
            let dev = &*self.device;
            for &pool in self.ready_pools.iter().chain(self.used_pools.iter()) {
                if pool != vk::DescriptorPool::null() {
                    dev.destroy_descriptor_pool(pool, None);
                }
            }
        }
        self.ready_pools.clear();
        self.used_pools.clear();
    }

    // ------------------------------------------------------------------

    fn current_pool(&mut self) -> Result<vk::DescriptorPool, RenderError> {
        if let Some(pool) = self.ready_pools.pop() {
            self.used_pools.push(pool);
            Ok(pool)
        } else {
            let pool = self.create_pool()?;
            self.used_pools.push(pool);
            Ok(pool)
        }
    }

    fn create_pool(&self) -> Result<vk::DescriptorPool, RenderError> {
        unsafe {
            (*self.device)
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .pool_sizes(&self.pool_sizes)
                        .max_sets(self.max_sets_per_pool),
                    None,
                )
                .map_err(|e| RenderError::DeviceCreation(format!("descriptor pool: {e}")))
        }
    }
}

impl Drop for DescriptorSetAllocator {
    fn drop(&mut self) {
        self.destroy();
    }
}
