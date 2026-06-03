use ash::vk;
use parking_lot::Mutex;
use crate::RenderError;

/// Maximum number of textures in the bindless heap.
pub const MAX_BINDLESS_TEXTURES: u32 = 4096;
/// Maximum number of samplers in the bindless heap.
pub const MAX_BINDLESS_SAMPLERS: u32 = 128;

/// Global bindless descriptor heap.
///
/// Holds a single descriptor set with:
/// - Binding 0: Scene UBO (1 descriptor, traditional)
/// - Binding 1: Sampled image array (MAX_BINDLESS_TEXTURES entries, partially bound, update-after-bind)
/// - Binding 2: Sampler array (MAX_BINDLESS_SAMPLERS entries, partially bound, update-after-bind)
///
/// Texture and sampler slots can be allocated and updated at any time.
/// The descriptor set uses `PARTIALLY_BOUND` so unwritten slots are valid.
pub struct BindlessDescriptorHeap {
    device: *const ash::Device,
    pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    set: vk::DescriptorSet,
    texture_slots: Mutex<Vec<Option<vk::ImageView>>>,
    free_texture_slots: Mutex<Vec<u32>>,
    sampler_slots: Mutex<Vec<Option<vk::Sampler>>>,
    free_sampler_slots: Mutex<Vec<u32>>,
}

unsafe impl Send for BindlessDescriptorHeap {}
unsafe impl Sync for BindlessDescriptorHeap {}

impl BindlessDescriptorHeap {
    pub fn new(device: &ash::Device) -> Result<Self, RenderError> {
        let max_textures = MAX_BINDLESS_TEXTURES;
        let max_samplers = MAX_BINDLESS_SAMPLERS;

        // --- Descriptor pool with UPDATE_AFTER_BIND ---
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLED_IMAGE,
                descriptor_count: max_textures,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLER,
                descriptor_count: max_samplers,
            },
        ];
        let pool = unsafe {
            device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .pool_sizes(&pool_sizes)
                        .max_sets(1)
                        .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND),
                    None,
                )
                .map_err(|e| {
                    RenderError::DeviceCreation(format!("bindless pool: {e}"))
                })?
        };

        // --- Descriptor set layout ---
        let bindings = [
            // Binding 0: Scene UBO
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
            // Binding 1: Sampled image array (bindless)
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(max_textures)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 2: Sampler array (bindless)
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(max_samplers)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];

        let mut binding_flags = [
            vk::DescriptorBindingFlags::empty(), // UBO: traditional
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
        ];
        let mut binding_flags_info =
            vk::DescriptorSetLayoutBindingFlagsCreateInfo::default()
                .binding_flags(&binding_flags);

        let layout = unsafe {
            device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default()
                        .bindings(&bindings)
                        .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
                        .push_next(&mut binding_flags_info),
                    None,
                )
                .map_err(|e| {
                    RenderError::DeviceCreation(format!("bindless layout: {e}"))
                })?
        };

        // --- Allocate descriptor set ---
        let layouts = [layout];
        let mut sets = unsafe {
            device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pool)
                        .set_layouts(&layouts),
                )
                .map_err(|e| {
                    RenderError::DeviceCreation(format!("bindless set alloc: {e}"))
                })?
        };
        let set = sets.remove(0);

        // Pre-allocate free lists
        let mut free_texture_slots: Vec<u32> = (0..max_textures).collect();
        free_texture_slots.reverse(); // pop() gives 0 first
        let mut free_sampler_slots: Vec<u32> = (0..max_samplers).collect();
        free_sampler_slots.reverse();

        Ok(Self {
            device: device as *const ash::Device,
            pool,
            layout,
            set,
            texture_slots: Mutex::new(vec![None; max_textures as usize]),
            free_texture_slots: Mutex::new(free_texture_slots),
            sampler_slots: Mutex::new(vec![None; max_samplers as usize]),
            free_sampler_slots: Mutex::new(free_sampler_slots),
        })
    }

    pub fn set(&self) -> vk::DescriptorSet {
        self.set
    }

    pub fn layout(&self) -> vk::DescriptorSetLayout {
        self.layout
    }

    /// Allocate a texture slot and write the image descriptor.
    /// Returns the slot index (e.g., for push constants).
    pub fn alloc_texture(
        &self,
        view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> u32 {
        let slot = self
            .free_texture_slots
            .lock()
            .pop()
            .expect("bindless texture heap exhausted");
        self.texture_slots.lock()[slot as usize] = Some(view);

        let img_info = [vk::DescriptorImageInfo::default()
            .image_view(view)
            .image_layout(image_layout)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.set)
            .dst_binding(1)
            .dst_array_element(slot)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .image_info(&img_info);

        unsafe {
            (*self.device).update_descriptor_sets(&[write], &[]);
        }
        slot
    }

    /// Free a texture slot so it can be reused.
    pub fn free_texture(&self, slot: u32) {
        let mut slots = self.texture_slots.lock();
        assert!((slot as usize) < slots.len());
        if slots[slot as usize].is_some() {
            slots[slot as usize] = None;
            self.free_texture_slots.lock().push(slot);
        }
    }

    /// Allocate a sampler slot and write the sampler descriptor.
    pub fn alloc_sampler(&self, sampler: vk::Sampler) -> u32 {
        let slot = self
            .free_sampler_slots
            .lock()
            .pop()
            .expect("bindless sampler heap exhausted");
        self.sampler_slots.lock()[slot as usize] = Some(sampler);

        let samp_info = [vk::DescriptorImageInfo::default().sampler(sampler)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.set)
            .dst_binding(2)
            .dst_array_element(slot)
            .descriptor_type(vk::DescriptorType::SAMPLER)
            .image_info(&samp_info);

        unsafe {
            (*self.device).update_descriptor_sets(&[write], &[]);
        }
        slot
    }

    /// Free a sampler slot.
    pub fn free_sampler(&self, slot: u32) {
        let mut slots = self.sampler_slots.lock();
        assert!((slot as usize) < slots.len());
        if slots[slot as usize].is_some() {
            slots[slot as usize] = None;
            self.free_sampler_slots.lock().push(slot);
        }
    }

    /// Write the scene UBO descriptor (binding 0).
    pub fn write_ubo(&self, buffer: vk::Buffer, size: u64) {
        let bi = [vk::DescriptorBufferInfo::default()
            .buffer(buffer)
            .offset(0)
            .range(size)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(&bi);
        unsafe {
            (*self.device).update_descriptor_sets(&[write], &[]);
        }
    }

    // ---- Batch variants (queue writes into a DescriptorUpdateBatch) ----

    /// Allocate a texture slot and queue the descriptor write into `batch`.
    pub fn alloc_texture_into(
        &self,
        batch: &mut crate::descriptor_batch::DescriptorUpdateBatch,
        view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> u32 {
        let slot = self
            .free_texture_slots
            .lock()
            .pop()
            .expect("bindless texture heap exhausted");
        self.texture_slots.lock()[slot as usize] = Some(view);
        batch.write_sampled_image(1, slot, view, image_layout);
        slot
    }

    /// Allocate a sampler slot and queue the descriptor write into `batch`.
    pub fn alloc_sampler_into(
        &self,
        batch: &mut crate::descriptor_batch::DescriptorUpdateBatch,
        sampler: vk::Sampler,
    ) -> u32 {
        let slot = self
            .free_sampler_slots
            .lock()
            .pop()
            .expect("bindless sampler heap exhausted");
        self.sampler_slots.lock()[slot as usize] = Some(sampler);
        batch.write_sampler(2, slot, sampler);
        slot
    }

    /// Queue a UBO write into `batch` instead of flushing immediately.
    pub fn write_ubo_into(
        &self,
        batch: &mut crate::descriptor_batch::DescriptorUpdateBatch,
        buffer: vk::Buffer,
        size: u64,
    ) {
        batch.write_uniform_buffer(0, buffer, 0, size);
    }
}

impl Drop for BindlessDescriptorHeap {
    fn drop(&mut self) {
        unsafe {
            if !self.device.is_null() {
                let dev = &*self.device;
                if self.layout != vk::DescriptorSetLayout::null() {
                    dev.destroy_descriptor_set_layout(self.layout, None);
                }
                if self.pool != vk::DescriptorPool::null() {
                    dev.destroy_descriptor_pool(self.pool, None);
                }
            }
        }
    }
}
