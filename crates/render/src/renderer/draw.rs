use ash::vk;
use crate::memory::GpuBuffer;
use crate::texture::{DepthBuffer, GpuTexture};
use crate::pipeline;
use crate::error::RenderError;

impl super::Renderer {
    pub fn update_2d_descriptor_set(
        &self, set: vk::DescriptorSet, ubo: &GpuBuffer, texture: &GpuTexture,
    ) {
        let bi = [vk::DescriptorBufferInfo::default().buffer(ubo.buffer).offset(0).range(ubo.size)];
        let tex_ii = [vk::DescriptorImageInfo::default()
            .image_view(texture.view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let samp_ii = [vk::DescriptorImageInfo::default()
            .sampler(texture.sampler)];
        let writes = [
            vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&bi),
            vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&tex_ii),
            vk::WriteDescriptorSet::default().dst_set(set).dst_binding(2).descriptor_type(vk::DescriptorType::SAMPLER).image_info(&samp_ii),
        ];
        unsafe { self.device.logical().update_descriptor_sets(&writes, &[]); }
    }

    pub fn draw_2d(
        &self, cmd: vk::CommandBuffer,
        pipeline: &pipeline::GraphicsPipeline2D,
        vertex_buffer: &GpuBuffer, vertex_count: u32,
        push_constants: &[u8], descriptor_set: vk::DescriptorSet,
    ) {
        let sw = self.swapchain.lock();
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(sw.current_image_view()).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 0.0] } });
        let cas = [ca];
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: sw.extent() })
            .layer_count(1).color_attachments(&cas);

        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            self.device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: sw.extent().height as f32, width: sw.extent().width as f32, height: -(sw.extent().height as f32), min_depth: 0.0, max_depth: 1.0 }]);
            self.device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: sw.extent() }]);
            self.device.logical().cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            self.device.logical().cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[descriptor_set], &[]);
            self.device.logical().cmd_push_constants(cmd, pipeline.layout, vk::ShaderStageFlags::VERTEX, 0, push_constants);
            self.device.logical().cmd_bind_vertex_buffers(cmd, 0, &[vertex_buffer.buffer], &[0u64]);
            self.device.logical().cmd_draw(cmd, vertex_count, 1, 0, 0);
            dr.cmd_end_rendering(cmd);
        }
    }

    pub fn draw_mesh(
        &self, cmd: vk::CommandBuffer,
        pipeline: &pipeline::GraphicsPipeline,
        vertex_buffer: &GpuBuffer, index_buffer: Option<&GpuBuffer>, index_count: u32,
        push_constants: &[u8], descriptor_set: vk::DescriptorSet, depth_buffer: &DepthBuffer,
    ) {
        let sw = self.swapchain.lock();
        let extent = sw.extent();
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(sw.current_image_view()).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.04, 0.04, 0.08, 1.0] } });
        let da = vk::RenderingAttachmentInfoKHR::default()
            .image_view(depth_buffer.view).image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let cas = [ca];
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }).layer_count(1).color_attachments(&cas).depth_attachment(&da);

        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            self.device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: extent.height as f32, width: extent.width as f32, height: -(extent.height as f32), min_depth: 0.0, max_depth: 1.0 }]);
            self.device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]);
            self.device.logical().cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            self.device.logical().cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[descriptor_set], &[]);
            self.device.logical().cmd_push_constants(cmd, pipeline.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, push_constants);
            self.device.logical().cmd_bind_vertex_buffers(cmd, 0, &[vertex_buffer.buffer], &[0u64]);
            if let Some(ib) = index_buffer {
                self.device.logical().cmd_bind_index_buffer(cmd, ib.buffer, 0, vk::IndexType::UINT16);
                self.device.logical().cmd_draw_indexed(cmd, index_count, 1, 0, 0, 0);
            } else {
                self.device.logical().cmd_draw(cmd, index_count, 1, 0, 0);
            }
            dr.cmd_end_rendering(cmd);
        }
    }

    pub fn begin_scene_pass(&self, cmd: vk::CommandBuffer, depth_buffer: &DepthBuffer, clear_color: [f32; 4]) {
        let sw = self.swapchain.lock();
        let extent = sw.extent();
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(sw.current_image_view()).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: clear_color } });
        let da = vk::RenderingAttachmentInfoKHR::default()
            .image_view(depth_buffer.view).image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let cas = [ca];
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }).layer_count(1).color_attachments(&cas).depth_attachment(&da);
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            self.device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: extent.height as f32, width: extent.width as f32, height: -(extent.height as f32), min_depth: 0.0, max_depth: 1.0 }]);
            self.device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]);
        }
    }

    pub fn begin_scene_pass_offscreen(&self, cmd: vk::CommandBuffer, color_image: vk::Image, color_view: vk::ImageView, depth_buffer: &DepthBuffer, extent: vk::Extent2D, clear_color: [f32; 4]) {
        // Color: transition from UNDEFINED — valid for newly-created images and harmless for
        // images that were previously SHADER_READ_ONLY_OPTIMAL (we clear anyway).
        let color_barrier = vk::ImageMemoryBarrier2::default()
            .image(color_image)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::TOP_OF_PIPE)
            .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags2::empty())
            .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0, level_count: 1,
                base_array_layer: 0, layer_count: 1,
            });
        let depth_barrier = vk::ImageMemoryBarrier2::default()
            .image(depth_buffer.image)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .src_stage_mask(vk::PipelineStageFlags2::TOP_OF_PIPE)
            .dst_stage_mask(vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags2::empty())
            .dst_access_mask(vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                base_mip_level: 0, level_count: 1,
                base_array_layer: 0, layer_count: 1,
            });
        let barriers = [color_barrier, depth_barrier];
        let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
        unsafe {
            self.device.logical().cmd_pipeline_barrier2(cmd, &dep);
        }
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(color_view).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: clear_color } });
        let da = vk::RenderingAttachmentInfoKHR::default()
            .image_view(depth_buffer.view).image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let cas = [ca];
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }).layer_count(1).color_attachments(&cas).depth_attachment(&da);
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            self.device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: extent.height as f32, width: extent.width as f32, height: -(extent.height as f32), min_depth: 0.0, max_depth: 1.0 }]);
            self.device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]);
        }
    }

    pub fn draw_indexed_in_pass(
        &self, cmd: vk::CommandBuffer,
        pipeline: &pipeline::GraphicsPipeline,
        vertex_buffer: &GpuBuffer, index_buffer: Option<&GpuBuffer>, index_count: u32,
        push_constants: &[u8],
    ) {
        let bindless_set = self.bindless_heap.set();
        unsafe {
            self.device.logical().cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            self.device.logical().cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[bindless_set], &[]);
            self.device.logical().cmd_push_constants(cmd, pipeline.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, push_constants);
            self.device.logical().cmd_bind_vertex_buffers(cmd, 0, &[vertex_buffer.buffer], &[0u64]);
            if let Some(ib) = index_buffer {
                self.device.logical().cmd_bind_index_buffer(cmd, ib.buffer, 0, vk::IndexType::UINT16);
                self.device.logical().cmd_draw_indexed(cmd, index_count, 1, 0, 0, 0);
            } else {
                self.device.logical().cmd_draw(cmd, index_count, 1, 0, 0);
            }
        }
    }

    pub fn end_scene_pass(&self, cmd: vk::CommandBuffer) {
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_end_rendering(cmd);
        }
    }

    pub fn end_scene_pass_offscreen(&self, cmd: vk::CommandBuffer, color_image: vk::Image) {
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_end_rendering(cmd);
            let barrier = vk::ImageMemoryBarrier2::default()
                .image(color_image)
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
                .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags2::SHADER_READ)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0, level_count: 1,
                    base_array_layer: 0, layer_count: 1,
                });
            let barriers = [barrier];
            let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
            self.device.logical().cmd_pipeline_barrier2(cmd, &dep);
        }
    }

    pub fn begin_shadow_pass(&self, cmd: vk::CommandBuffer, shadow_map: &GpuTexture, size: u32) {
        let extent = vk::Extent2D { width: size, height: size };
        let da = vk::RenderingAttachmentInfoKHR::default()
            .image_view(shadow_map.view).image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }).layer_count(1).depth_attachment(&da);
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            self.device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: size as f32, width: size as f32, height: -(size as f32), min_depth: 0.0, max_depth: 1.0 }]);
            self.device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]);
        }
    }

    pub fn draw_shadow_in_pass(
        &self, cmd: vk::CommandBuffer,
        pipeline: &pipeline::ShadowPipeline,
        vertex_buffer: &GpuBuffer, index_buffer: Option<&GpuBuffer>, index_count: u32,
        push_constants: &[u8],
    ) {
        let bindless_set = self.bindless_heap.set();
        unsafe {
            self.device.logical().cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            self.device.logical().cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[bindless_set], &[]);
            self.device.logical().cmd_push_constants(cmd, pipeline.layout, vk::ShaderStageFlags::VERTEX, 0, push_constants);
            self.device.logical().cmd_bind_vertex_buffers(cmd, 0, &[vertex_buffer.buffer], &[0u64]);
            if let Some(ib) = index_buffer {
                self.device.logical().cmd_bind_index_buffer(cmd, ib.buffer, 0, vk::IndexType::UINT16);
                self.device.logical().cmd_draw_indexed(cmd, index_count, 1, 0, 0, 0);
            } else {
                self.device.logical().cmd_draw(cmd, index_count, 1, 0, 0);
            }
        }
    }

    pub fn end_shadow_pass(&self, cmd: vk::CommandBuffer) {
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_end_rendering(cmd);
        }
    }

    pub fn update_tonemap_descriptor_set(
        &self, set: vk::DescriptorSet, hdr_view: vk::ImageView, sampler: vk::Sampler,
    ) {
        let tex_ii = [vk::DescriptorImageInfo::default().image_view(hdr_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let samp_ii = [vk::DescriptorImageInfo::default().sampler(sampler)];
        let writes = [
            vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&tex_ii),
            vk::WriteDescriptorSet::default().dst_set(set).dst_binding(2).descriptor_type(vk::DescriptorType::SAMPLER).image_info(&samp_ii),
        ];
        unsafe { self.device.logical().update_descriptor_sets(&writes, &[]); }
    }

    pub fn tone_map_pass(
        &self, cmd: vk::CommandBuffer,
        pipeline: &pipeline::ToneMapPipeline,
        desc_set: vk::DescriptorSet,
        extent: vk::Extent2D,
    ) {
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(self.swapchain.lock().current_image_view())
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE);
        let cas = [ca];
        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent })
            .layer_count(1).color_attachments(&cas);

        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            self.device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport {
                x: 0.0, y: extent.height as f32,
                width: extent.width as f32, height: -(extent.height as f32),
                min_depth: 0.0, max_depth: 1.0,
            }]);
            self.device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]);
            self.device.logical().cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            self.device.logical().cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[desc_set], &[]);
            self.device.logical().cmd_draw(cmd, 3, 1, 0, 0);
            dr.cmd_end_rendering(cmd);
        }
    }

    /// Blit a render-target image into the current swapchain image and transition
    /// the swapchain image to `PRESENT_SRC_KHR` so `end_frame` can present it.
    pub fn blit_to_swapchain(
        &self, cmd: vk::CommandBuffer,
        src_image: vk::Image, src_extent: vk::Extent2D,
        src_layout: vk::ImageLayout,
    ) {
        let sw = self.swapchain.lock();
        let dst_image = sw.current_image();
        let dst_extent = sw.extent();
        drop(sw);

        unsafe {
            // 1. Transition src to TRANSFER_SRC if not already.
            if src_layout != vk::ImageLayout::TRANSFER_SRC_OPTIMAL {
                let src_barrier = vk::ImageMemoryBarrier2::default()
                    .image(src_image)
                    .old_layout(src_layout)
                    .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags2::FRAGMENT_SHADER)
                    .dst_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                    .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE | vk::AccessFlags2::SHADER_READ)
                    .dst_access_mask(vk::AccessFlags2::TRANSFER_READ)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0, level_count: 1,
                        base_array_layer: 0, layer_count: 1,
                    });
                let barriers = [src_barrier];
                let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
                self.device.logical().cmd_pipeline_barrier2(cmd, &dep);
            }

            // 2. Transition dst (swapchain) to TRANSFER_DST.
            let dst_barrier = vk::ImageMemoryBarrier2::default()
                .image(dst_image)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0, level_count: 1,
                    base_array_layer: 0, layer_count: 1,
                });
            let barriers = [dst_barrier];
            let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
            self.device.logical().cmd_pipeline_barrier2(cmd, &dep);

            // 3. Blit.
            let blit = vk::ImageBlit::default()
                .src_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0, base_array_layer: 0, layer_count: 1,
                })
                .src_offsets([
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D { x: src_extent.width as i32, y: src_extent.height as i32, z: 1 },
                ])
                .dst_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0, base_array_layer: 0, layer_count: 1,
                })
                .dst_offsets([
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D { x: dst_extent.width as i32, y: dst_extent.height as i32, z: 1 },
                ]);
            self.device.logical().cmd_blit_image(
                cmd,
                src_image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                dst_image, vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[blit], vk::Filter::LINEAR,
            );

            // 4. Transition dst to PRESENT_SRC_KHR.
            let present_barrier = vk::ImageMemoryBarrier2::default()
                .image(dst_image)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .src_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                .dst_stage_mask(vk::PipelineStageFlags2::BOTTOM_OF_PIPE)
                .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags2::NONE)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0, level_count: 1,
                    base_array_layer: 0, layer_count: 1,
                });
            let barriers = [present_barrier];
            let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
            self.device.logical().cmd_pipeline_barrier2(cmd, &dep);
        }
    }
}
