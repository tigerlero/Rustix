use ash::vk;
use crate::RenderError;

/// Pending descriptor write stored in a lifetime-safe form.
enum PendingWrite {
    SampledImage {
        binding: u32,
        array_element: u32,
        view: vk::ImageView,
        layout: vk::ImageLayout,
    },
    Sampler {
        binding: u32,
        array_element: u32,
        sampler: vk::Sampler,
    },
    UniformBuffer {
        binding: u32,
        buffer: vk::Buffer,
        offset: u64,
        range: u64,
    },
}

/// Accumulates descriptor writes and flushes them in a single
/// `vkUpdateDescriptorSets` call.
///
/// Typical usage:
/// ```ignore
/// let mut batch = DescriptorUpdateBatch::new(&device, bindless_set);
/// heap.alloc_texture_into(&mut batch, view, layout);
/// heap.alloc_sampler_into(&mut batch, sampler);
/// heap.write_ubo_into(&mut batch, buffer, size);
/// batch.flush()?;
/// ```
pub struct DescriptorUpdateBatch {
    device: *const ash::Device,
    dst_set: vk::DescriptorSet,
    pending: Vec<PendingWrite>,
}

unsafe impl Send for DescriptorUpdateBatch {}

impl DescriptorUpdateBatch {
    pub fn new(device: &ash::Device, dst_set: vk::DescriptorSet) -> Self {
        Self {
            device: device as *const ash::Device,
            dst_set,
            pending: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Queue a sampled-image descriptor write.
    pub fn write_sampled_image(
        &mut self,
        binding: u32,
        array_element: u32,
        view: vk::ImageView,
        layout: vk::ImageLayout,
    ) {
        self.pending.push(PendingWrite::SampledImage {
            binding,
            array_element,
            view,
            layout,
        });
    }

    /// Queue a sampler descriptor write.
    pub fn write_sampler(
        &mut self,
        binding: u32,
        array_element: u32,
        sampler: vk::Sampler,
    ) {
        self.pending.push(PendingWrite::Sampler {
            binding,
            array_element,
            sampler,
        });
    }

    /// Queue a uniform-buffer descriptor write.
    pub fn write_uniform_buffer(
        &mut self,
        binding: u32,
        buffer: vk::Buffer,
        offset: u64,
        range: u64,
    ) {
        self.pending.push(PendingWrite::UniformBuffer {
            binding,
            buffer,
            offset,
            range,
        });
    }

    /// Submit all queued writes in a single `update_descriptor_sets` call.
    /// The batch is cleared after a successful flush.
    pub fn flush(&mut self) -> Result<(), RenderError> {
        if self.pending.is_empty() {
            return Ok(());
        }

        // Pre-allocate info storage to avoid pointer invalidation.
        let image_count = self
            .pending
            .iter()
            .filter(|p| matches!(p, PendingWrite::SampledImage { .. } | PendingWrite::Sampler { .. }))
            .count();
        let buffer_count = self
            .pending
            .iter()
            .filter(|p| matches!(p, PendingWrite::UniformBuffer { .. }))
            .count();

        let mut image_infos: Vec<vk::DescriptorImageInfo> = Vec::with_capacity(image_count);
        let mut buffer_infos: Vec<vk::DescriptorBufferInfo> = Vec::with_capacity(buffer_count);

        // First pass: build info arrays and record (type, binding, array_element, info_index).
        let mut write_meta: Vec<(vk::DescriptorType, u32, u32, usize)> =
            Vec::with_capacity(self.pending.len());

        for pending in &self.pending {
            match *pending {
                PendingWrite::SampledImage {
                    binding,
                    array_element,
                    view,
                    layout,
                } => {
                    let idx = image_infos.len();
                    image_infos.push(
                        vk::DescriptorImageInfo::default()
                            .image_view(view)
                            .image_layout(layout),
                    );
                    write_meta.push((vk::DescriptorType::SAMPLED_IMAGE, binding, array_element, idx));
                }
                PendingWrite::Sampler {
                    binding,
                    array_element,
                    sampler,
                } => {
                    let idx = image_infos.len();
                    image_infos.push(vk::DescriptorImageInfo::default().sampler(sampler));
                    write_meta.push((vk::DescriptorType::SAMPLER, binding, array_element, idx));
                }
                PendingWrite::UniformBuffer {
                    binding,
                    buffer,
                    offset,
                    range,
                } => {
                    let idx = buffer_infos.len();
                    buffer_infos.push(
                        vk::DescriptorBufferInfo::default()
                            .buffer(buffer)
                            .offset(offset)
                            .range(range),
                    );
                    write_meta.push((vk::DescriptorType::UNIFORM_BUFFER, binding, 0, idx));
                }
            }
        }

        // Second pass: build WriteDescriptorSet structs with stable pointers.
        let mut writes: Vec<vk::WriteDescriptorSet> = Vec::with_capacity(write_meta.len());
        for (ty, binding, array_element, info_idx) in &write_meta {
            let write = match *ty {
                vk::DescriptorType::SAMPLED_IMAGE | vk::DescriptorType::SAMPLER => {
                    vk::WriteDescriptorSet::default()
                        .dst_set(self.dst_set)
                        .dst_binding(*binding)
                        .dst_array_element(*array_element)
                        .descriptor_type(*ty)
                        .image_info(std::slice::from_ref(&image_infos[*info_idx]))
                }
                vk::DescriptorType::UNIFORM_BUFFER => {
                    vk::WriteDescriptorSet::default()
                        .dst_set(self.dst_set)
                        .dst_binding(*binding)
                        .dst_array_element(0)
                        .descriptor_type(*ty)
                        .buffer_info(std::slice::from_ref(&buffer_infos[*info_idx]))
                }
                _ => unreachable!(),
            };
            writes.push(write);
        }

        unsafe {
            (*self.device).update_descriptor_sets(&writes, &[]);
        }

        self.pending.clear();
        Ok(())
    }
}
