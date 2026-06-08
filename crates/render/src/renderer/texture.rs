use ash::vk;
use crate::texture::GpuTexture;
use crate::error::RenderError;

impl super::Renderer {
    pub fn create_texture(&self, width: u32, height: u32, pixels: &[u8]) -> Result<GpuTexture, RenderError> {
        let extent = vk::Extent3D { width, height, depth: 1 };
        let fmt = vk::Format::R8G8B8A8_UNORM;

        let staging = self.create_buffer("tex_staging", pixels.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, gpu_allocator::MemoryLocation::CpuToGpu)?;
        staging.write(pixels);
        staging.flush(0, pixels.len() as u64);

        let img = unsafe {
            self.device.logical().create_image(
                &vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).format(fmt).extent(extent)
                    .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL).usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("tex image: {e}")))?
        };
        let req = unsafe { self.device.logical().get_image_memory_requirements(img) };
        let alloc = self.allocator.lock().allocate("texture", req, gpu_allocator::MemoryLocation::GpuOnly, false)?;
        unsafe { self.device.logical().bind_image_memory(img, alloc.memory(), alloc.offset())?; }

        let one_time_cmd = {
            let info = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.transfer_command_pool)
                .level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
            unsafe { self.device.logical().allocate_command_buffers(&info)?.remove(0) }
        };
        unsafe {
            self.device.logical().begin_command_buffer(one_time_cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
        }

        let barrier1 = vk::ImageMemoryBarrier2::default()
            .image(img).old_layout(vk::ImageLayout::UNDEFINED).new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::TOP_OF_PIPE).dst_stage_mask(vk::PipelineStageFlags2::TRANSFER)
            .src_access_mask(vk::AccessFlags2::empty()).dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        let barriers1 = [barrier1];
        let dep1 = vk::DependencyInfo::default().image_memory_barriers(&barriers1);
        unsafe {
            self.device.logical().cmd_pipeline_barrier2(one_time_cmd, &dep1);
        }

        let copy_region = vk::BufferImageCopy::default()
            .buffer_offset(0).image_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 }).image_extent(extent);
        unsafe {
            self.device.logical().cmd_copy_buffer_to_image(one_time_cmd, staging.buffer, img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_region]);
        }

        let barrier2 = vk::ImageMemoryBarrier2::default()
            .image(img).old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL).new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::TRANSFER).dst_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE).dst_access_mask(vk::AccessFlags2::SHADER_READ)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        let barriers2 = [barrier2];
        let dep2 = vk::DependencyInfo::default().image_memory_barriers(&barriers2);
        unsafe {
            self.device.logical().cmd_pipeline_barrier2(one_time_cmd, &dep2);
        }

        unsafe { self.device.logical().end_command_buffer(one_time_cmd)?; }
        let cmds = [one_time_cmd];
        let si = vk::SubmitInfo::default().command_buffers(&cmds);
        let subs = [si];
        let upload_fence = unsafe { self.device.logical().create_fence(&vk::FenceCreateInfo::default(), None)? };
        unsafe { self.device.logical().queue_submit(self.device.transfer_queue(), &subs, upload_fence)?; }
        unsafe { self.device.logical().wait_for_fences(&[upload_fence], true, u64::MAX)?; }
        unsafe { self.device.logical().destroy_fence(upload_fence, None); }

        let view = unsafe {
            self.device.logical().create_image_view(
                &vk::ImageViewCreateInfo::default().image(img).view_type(vk::ImageViewType::TYPE_2D).format(fmt)
                    .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 }), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("tex view: {e}")))?
        };

        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR).min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT).address_mode_v(vk::SamplerAddressMode::REPEAT).address_mode_w(vk::SamplerAddressMode::REPEAT)
            .anisotropy_enable(false).max_anisotropy(1.0)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false).compare_enable(false).compare_op(vk::CompareOp::ALWAYS)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR).mip_lod_bias(0.0).min_lod(0.0).max_lod(0.0);
        let sampler = self.device.sampler_cache()
            .get_or_create(&sampler_info)
            .map_err(|e| RenderError::DeviceCreation(format!("sampler: {e}")))?;

        unsafe { self.device.logical().free_command_buffers(self.transfer_command_pool, &[one_time_cmd]); }

        Ok(GpuTexture { image: img, view, sampler, _allocation: alloc })
    }

    /// Create a texture with an explicit Vulkan format. Supports R8G8B8A8_UNORM, R16G16B16A16_SFLOAT, R32G32B32A32_SFLOAT.
    pub fn create_texture_with_format(&self, width: u32, height: u32, pixels: &[u8], format: vk::Format) -> Result<GpuTexture, RenderError> {
        let extent = vk::Extent3D { width, height, depth: 1 };

        let staging = self.create_buffer("tex_staging", pixels.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, gpu_allocator::MemoryLocation::CpuToGpu)?;
        staging.write(pixels);
        staging.flush(0, pixels.len() as u64);

        let img = unsafe {
            self.device.logical().create_image(
                &vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).format(format).extent(extent)
                    .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL).usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("tex image: {e}")))?
        };
        let req = unsafe { self.device.logical().get_image_memory_requirements(img) };
        let alloc = self.allocator.lock().allocate("texture", req, gpu_allocator::MemoryLocation::GpuOnly, false)?;
        unsafe { self.device.logical().bind_image_memory(img, alloc.memory(), alloc.offset())?; }

        let one_time_cmd = {
            let info = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.transfer_command_pool)
                .level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
            unsafe { self.device.logical().allocate_command_buffers(&info)?.remove(0) }
        };
        unsafe {
            self.device.logical().begin_command_buffer(one_time_cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
        }

        let barrier1 = vk::ImageMemoryBarrier2::default()
            .image(img).old_layout(vk::ImageLayout::UNDEFINED).new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::TOP_OF_PIPE).dst_stage_mask(vk::PipelineStageFlags2::TRANSFER)
            .src_access_mask(vk::AccessFlags2::empty()).dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        let barriers1 = [barrier1];
        let dep1 = vk::DependencyInfo::default().image_memory_barriers(&barriers1);
        unsafe { self.device.logical().cmd_pipeline_barrier2(one_time_cmd, &dep1); }

        let copy_region = vk::BufferImageCopy::default()
            .buffer_offset(0).image_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 }).image_extent(extent);
        let regions = [copy_region];
        unsafe { self.device.logical().cmd_copy_buffer_to_image(one_time_cmd, staging.buffer, img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &regions); }

        let barrier2 = vk::ImageMemoryBarrier2::default()
            .image(img).old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL).new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::TRANSFER).dst_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE).dst_access_mask(vk::AccessFlags2::SHADER_READ)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        let barriers2 = [barrier2];
        let dep2 = vk::DependencyInfo::default().image_memory_barriers(&barriers2);
        unsafe { self.device.logical().cmd_pipeline_barrier2(one_time_cmd, &dep2); }

        unsafe { self.device.logical().end_command_buffer(one_time_cmd)?; }
        let cmds = [one_time_cmd];
        let si = vk::SubmitInfo::default().command_buffers(&cmds);
        let upload_fence = unsafe { self.device.logical().create_fence(&vk::FenceCreateInfo::default(), None)? };
        unsafe { self.device.logical().queue_submit(self.device.transfer_queue(), &[si], upload_fence)?; }
        unsafe { self.device.logical().wait_for_fences(&[upload_fence], true, u64::MAX)?; }
        unsafe { self.device.logical().destroy_fence(upload_fence, None); }

        let view = unsafe {
            self.device.logical().create_image_view(
                &vk::ImageViewCreateInfo::default().image(img).view_type(vk::ImageViewType::TYPE_2D).format(format)
                    .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 }), None,
            ).map_err(|e| RenderError::DeviceCreation(format!("tex view: {e}")))?
        };

        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR).min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT).address_mode_v(vk::SamplerAddressMode::REPEAT).address_mode_w(vk::SamplerAddressMode::REPEAT)
            .anisotropy_enable(false).max_anisotropy(1.0)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false).compare_enable(false).compare_op(vk::CompareOp::ALWAYS)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR).mip_lod_bias(0.0).min_lod(0.0).max_lod(0.0);
        let sampler = self.device.sampler_cache()
            .get_or_create(&sampler_info)
            .map_err(|e| RenderError::DeviceCreation(format!("sampler: {e}")))?;

        unsafe { self.device.logical().free_command_buffers(self.transfer_command_pool, &[one_time_cmd]); }

        Ok(GpuTexture { image: img, view, sampler, _allocation: alloc })
    }

    /// Create a `GpuTexture` from a `TextureAsset` by selecting the appropriate Vulkan format.
    pub fn create_texture_from_asset(&self, asset: &rustix_asset::texture::TextureAsset) -> Result<GpuTexture, RenderError> {
        let vk_format = match asset.format {
            rustix_asset::texture::TextureFormat::R8g8b8a8Unorm => vk::Format::R8G8B8A8_UNORM,
            rustix_asset::texture::TextureFormat::R16g16b16a16Sfloat => vk::Format::R16G16B16A16_SFLOAT,
            rustix_asset::texture::TextureFormat::R32g32b32a32Sfloat => vk::Format::R32G32B32A32_SFLOAT,
        };
        self.create_texture_with_format(asset.width, asset.height, &asset.pixels, vk_format)
    }

    pub fn update_texture_pixels(&self, tex: &GpuTexture, width: u32, height: u32, pixels: &[u8]) -> Result<(), RenderError> {
        let extent = vk::Extent3D { width, height, depth: 1 };
        let staging = self.create_buffer("tex_update", pixels.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, gpu_allocator::MemoryLocation::CpuToGpu)?;
        staging.write(pixels);
        staging.flush(0, pixels.len() as u64);

        let one_time_cmd = {
            let info = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.transfer_command_pool)
                .level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
            unsafe { self.device.logical().allocate_command_buffers(&info)?.remove(0) }
        };
        unsafe {
            self.device.logical().begin_command_buffer(one_time_cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
        }

        let barrier = vk::ImageMemoryBarrier2::default()
            .image(tex.image).old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL).new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER).dst_stage_mask(vk::PipelineStageFlags2::TRANSFER)
            .src_access_mask(vk::AccessFlags2::SHADER_READ).dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        let barriers = [barrier];
        let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
        unsafe {
            self.device.logical().cmd_pipeline_barrier2(one_time_cmd, &dep);
        }

        let copy_region = vk::BufferImageCopy::default()
            .buffer_offset(0).image_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 }).image_extent(extent);
        unsafe {
            self.device.logical().cmd_copy_buffer_to_image(one_time_cmd, staging.buffer, tex.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_region]);
        }

        let barrier2 = vk::ImageMemoryBarrier2::default()
            .image(tex.image).old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL).new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::TRANSFER).dst_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE).dst_access_mask(vk::AccessFlags2::SHADER_READ)
            .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
        let barriers2 = [barrier2];
        let dep2 = vk::DependencyInfo::default().image_memory_barriers(&barriers2);
        unsafe {
            self.device.logical().cmd_pipeline_barrier2(one_time_cmd, &dep2);
        }

        unsafe { self.device.logical().end_command_buffer(one_time_cmd)?; }
        let cmds = [one_time_cmd];
        let si = vk::SubmitInfo::default().command_buffers(&cmds);
        let subs = [si];
        let upload_fence = unsafe { self.device.logical().create_fence(&vk::FenceCreateInfo::default(), None)? };
        unsafe { self.device.logical().queue_submit(self.device.transfer_queue(), &subs, upload_fence)?; }
        unsafe { self.device.logical().wait_for_fences(&[upload_fence], true, u64::MAX)?; }
        unsafe { self.device.logical().destroy_fence(upload_fence, None); }

        unsafe { self.device.logical().free_command_buffers(self.transfer_command_pool, &[one_time_cmd]); }
        Ok(())
    }

    pub fn update_texture_subregion(
        &self,
        tex: &GpuTexture,
        offset_x: u32,
        offset_y: u32,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(), RenderError> {
        let extent = vk::Extent3D { width, height, depth: 1 };
        let staging = self.create_buffer("tex_subregion_update", pixels.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, gpu_allocator::MemoryLocation::CpuToGpu)?;
        staging.write(pixels);
        staging.flush(0, pixels.len() as u64);

        unsafe { self.device.logical().device_wait_idle()?; }

        let one_time_cmd = {
            let info = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.transfer_command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            unsafe { self.device.logical().allocate_command_buffers(&info)?.remove(0) }
        };
        unsafe {
            self.device.logical().begin_command_buffer(one_time_cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
        }

        let barrier = vk::ImageMemoryBarrier2::default()
            .image(tex.image)
            .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
            .dst_stage_mask(vk::PipelineStageFlags2::TRANSFER)
            .src_access_mask(vk::AccessFlags2::SHADER_READ)
            .dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let barriers = [barrier];
        let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
        unsafe {
            self.device.logical().cmd_pipeline_barrier2(one_time_cmd, &dep);
        }

        let copy_region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D {
                x: offset_x as i32,
                y: offset_y as i32,
                z: 0,
            })
            .image_extent(extent);
        unsafe {
            self.device.logical().cmd_copy_buffer_to_image(
                one_time_cmd,
                staging.buffer,
                tex.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[copy_region],
            );
        }

        let barrier2 = vk::ImageMemoryBarrier2::default()
            .image(tex.image)
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::TRANSFER)
            .dst_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags2::SHADER_READ)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let barriers2 = [barrier2];
        let dep2 = vk::DependencyInfo::default().image_memory_barriers(&barriers2);
        unsafe {
            self.device.logical().cmd_pipeline_barrier2(one_time_cmd, &dep2);
        }

        unsafe { self.device.logical().end_command_buffer(one_time_cmd)?; }
        let cmds = [one_time_cmd];
        let si = vk::SubmitInfo::default().command_buffers(&cmds);
        let subs = [si];
        let upload_fence = unsafe { self.device.logical().create_fence(&vk::FenceCreateInfo::default(), None)? };
        unsafe { self.device.logical().queue_submit(self.device.transfer_queue(), &subs, upload_fence)?; }
        unsafe { self.device.logical().wait_for_fences(&[upload_fence], true, u64::MAX)?; }
        unsafe { self.device.logical().destroy_fence(upload_fence, None); }

        unsafe { self.device.logical().free_command_buffers(self.transfer_command_pool, &[one_time_cmd]); }
        Ok(())
    }
}
