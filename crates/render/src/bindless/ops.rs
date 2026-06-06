use ash::vk;
use super::BindlessDescriptorHeap;

impl BindlessDescriptorHeap {
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

    /// Write a fixed uniform buffer descriptor to a specific binding (e.g., 10 for CSM UBO).
    pub fn write_fixed_ubo(&self, binding: u32, buffer: vk::Buffer, size: u64) {
        let bi = [vk::DescriptorBufferInfo::default()
            .buffer(buffer)
            .offset(0)
            .range(size)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.set)
            .dst_binding(binding)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(&bi);
        unsafe {
            (*self.device).update_descriptor_sets(&[write], &[]);
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

    /// Write a fixed sampled image descriptor to a specific binding (e.g., 5-8 for GBuffer).
    pub fn write_fixed_sampled_image(&self, binding: u32, view: vk::ImageView, image_layout: vk::ImageLayout) {
        let img_info = [vk::DescriptorImageInfo::default()
            .image_view(view)
            .image_layout(image_layout)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.set)
            .dst_binding(binding)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .image_info(&img_info);
        unsafe {
            (*self.device).update_descriptor_sets(&[write], &[]);
        }
    }

    /// Write a fixed sampler descriptor to a specific binding (e.g., 9 for GBuffer).
    pub fn write_fixed_sampler(&self, binding: u32, sampler: vk::Sampler) {
        let samp_info = [vk::DescriptorImageInfo::default().sampler(sampler)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.set)
            .dst_binding(binding)
            .descriptor_type(vk::DescriptorType::SAMPLER)
            .image_info(&samp_info);
        unsafe {
            (*self.device).update_descriptor_sets(&[write], &[]);
        }
    }

    /// Allocate a storage buffer slot for `binding` (3 or 4) and write the descriptor.
    pub fn alloc_storage_buffer(&self, binding: u32, buffer: vk::Buffer, size: u64) -> u32 {
        assert!(binding == 3 || binding == 4, "storage buffer binding must be 3 or 4");
        let idx = (binding - 3) as usize;
        let slot = self
            .free_storage_slots[idx]
            .lock()
            .pop()
            .expect("bindless storage buffer heap exhausted");
        self.storage_slots[idx].lock()[slot as usize] = Some(buffer);

        let bi = [vk::DescriptorBufferInfo::default()
            .buffer(buffer)
            .offset(0)
            .range(size)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(self.set)
            .dst_binding(binding)
            .dst_array_element(slot)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&bi);
        unsafe {
            (*self.device).update_descriptor_sets(&[write], &[]);
        }
        slot
    }

    /// Free a storage buffer slot for `binding` (3 or 4).
    pub fn free_storage_buffer(&self, binding: u32, slot: u32) {
        assert!(binding == 3 || binding == 4, "storage buffer binding must be 3 or 4");
        let idx = (binding - 3) as usize;
        let mut slots = self.storage_slots[idx].lock();
        assert!((slot as usize) < slots.len());
        if slots[slot as usize].is_some() {
            slots[slot as usize] = None;
            self.free_storage_slots[idx].lock().push(slot);
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
