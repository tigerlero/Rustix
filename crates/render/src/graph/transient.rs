use ash::vk;
use crate::renderer::Renderer;
use crate::RenderError;
use super::{FrameGraph, TransientImage};

impl<'a> FrameGraph<'a> {
    /// Create Vulkan images for all transient resources (`view == None && persistent == false`)
    /// and bind them to aliased memory. Resources with non-overlapping lifetimes share
    /// the same physical device memory at different offsets.
    /// Must be called after `compile()` and before `execute()`.
    pub fn allocate_transient_resources(&mut self, renderer: &Renderer) -> Result<(), RenderError> {
        self.destroy_transient_resources();

        let device = renderer.device().logical();
        let memory_props = renderer.device().memory_properties();
        self.device = device;

        // Step 1: identify transient resources and create unbound images.
        let mut t_indices: Vec<usize> = Vec::new();
        let mut images: Vec<vk::Image> = Vec::new();
        let mut reqs: Vec<vk::MemoryRequirements> = Vec::new();

        for (idx, desc) in self.textures.iter().enumerate() {
            if desc.persistent || desc.view.is_some() { continue; }
            if self.lifetimes.get(idx).copied().flatten().is_none() { continue; }

            let info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(desc.format)
                .extent(vk::Extent3D { width: desc.extent.width, height: desc.extent.height, depth: 1 })
                .mip_levels(1).array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(desc.usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let img = unsafe {
                device.create_image(&info, None)
                    .map_err(|e| RenderError::DeviceCreation(format!("transient img {idx}: {e}")))?
            };
            let req = unsafe { device.get_image_memory_requirements(img) };
            t_indices.push(idx);
            images.push(img);
            reqs.push(req);
        }

        if images.is_empty() { return Ok(()); }

        // Step 2: find a memory type compatible with all images.
        let mut type_bits = u32::MAX;
        for r in &reqs { type_bits &= r.memory_type_bits; }
        let mem_type = Self::find_memory_type(memory_props, type_bits)
            .ok_or_else(|| RenderError::DeviceCreation("no compatible memory type".into()))?;

        // Step 3: greedy aliasing allocator.
        // allocations tracks (offset, size, first_pass, last_pass).
        let mut allocated: Vec<(u64, u64, usize, usize)> = Vec::new();
        let mut total_size = 0u64;
        let mut offsets = vec![0u64; images.len()];

        for i in 0..images.len() {
            let size = reqs[i].size;
            let align = reqs[i].alignment;
            let idx = t_indices[i];
            let (first, last) = self.lifetimes[idx].unwrap();

            let mut best = None;
            let mut test = 0u64;
            while test <= total_size {
                let conflicts = allocated.iter().any(|(o, s, f, l)| {
                    let mem_overlap = test < o + s && test + size > *o;
                    let life_overlap = first <= *l && last >= *f;
                    mem_overlap && life_overlap
                });
                if !conflicts { best = Some(test); break; }
                test = ((test / align) + 1) * align;
            }

            let offset = best.unwrap_or_else(|| {
                let end = ((total_size + align - 1) / align) * align;
                end
            });
            offsets[i] = offset;
            allocated.push((offset, size, first, last));
            total_size = total_size.max(offset + size);
        }

        // Step 4: allocate one shared memory block.
        let memory = unsafe {
            device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(total_size)
                    .memory_type_index(mem_type),
                None,
            ).map_err(|e| RenderError::DeviceCreation(format!("transient memory: {e}")))?
        };
        self.transient_memory = Some(memory);
        self.transient_memory_size = total_size;

        // Step 5: bind images and create views.
        for i in 0..images.len() {
            let image = images[i];
            let offset = offsets[i];
            let idx = t_indices[i];
            let desc = &self.textures[idx];

            unsafe {
                device.bind_image_memory(image, memory, offset)
                    .map_err(|e| RenderError::DeviceCreation(format!("bind {idx}: {e}")))?;
            }

            let aspect = Self::aspect_mask_for_format(desc.format);
            let view = unsafe {
                device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image).view_type(vk::ImageViewType::TYPE_2D)
                        .format(desc.format)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: aspect,
                            base_mip_level: 0, level_count: 1,
                            base_array_layer: 0, layer_count: 1,
                        }),
                    None,
                ).map_err(|e| RenderError::DeviceCreation(format!("transient view {idx}: {e}")))?
            };

            self.transient_images.push(TransientImage { image, view });
            self.views[idx] = Some(view);
            self.images[idx] = Some(image);
        }

        Ok(())
    }

    /// Destroy all transient images and free the shared aliased memory block.
    pub fn destroy_transient_resources(&mut self) {
        if self.device.is_null() { return; }
        let device = unsafe { &*self.device };

        for ti in self.transient_images.drain(..) {
            unsafe {
                device.destroy_image_view(ti.view, None);
                device.destroy_image(ti.image, None);
            }
        }
        if let Some(mem) = self.transient_memory.take() {
            unsafe { device.free_memory(mem, None); }
        }

        // Reset views that the graph created.
        for (idx, desc) in self.textures.iter().enumerate() {
            if !desc.persistent && desc.view.is_none() {
                if let Some(v) = self.views.get_mut(idx) { *v = None; }
            }
        }
        self.transient_memory_size = 0;
        self.device = std::ptr::null();
    }

    fn find_memory_type(
        props: &vk::PhysicalDeviceMemoryProperties,
        type_bits: u32,
    ) -> Option<u32> {
        for i in 0..props.memory_type_count {
            let bit = 1u32 << i;
            if (type_bits & bit) != 0 {
                let flags = props.memory_types[i as usize].property_flags;
                if flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL) {
                    return Some(i);
                }
            }
        }
        for i in 0..props.memory_type_count {
            let bit = 1u32 << i;
            if (type_bits & bit) != 0 { return Some(i); }
        }
        None
    }

    fn aspect_mask_for_format(fmt: vk::Format) -> vk::ImageAspectFlags {
        match fmt {
            vk::Format::D32_SFLOAT | vk::Format::D16_UNORM
            | vk::Format::D24_UNORM_S8_UINT | vk::Format::X8_D24_UNORM_PACK32 => {
                vk::ImageAspectFlags::DEPTH
            }
            vk::Format::S8_UINT => vk::ImageAspectFlags::STENCIL,
            vk::Format::D32_SFLOAT_S8_UINT => {
                vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
            }
            _ => vk::ImageAspectFlags::COLOR,
        }
    }
}
