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

    pub fn draw_indexed_in_pass(
        &self, cmd: vk::CommandBuffer,
        pipeline: &pipeline::GraphicsPipeline,
        vertex_buffer: &GpuBuffer, index_buffer: Option<&GpuBuffer>, index_count: u32,
        push_constants: &[u8], descriptor_set: vk::DescriptorSet,
    ) {
        unsafe {
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
        }
    }

    pub fn end_scene_pass(&self, cmd: vk::CommandBuffer) {
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&self.instance.inner(), &self.device.logical());
            dr.cmd_end_rendering(cmd);
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
        push_constants: &[u8], descriptor_set: vk::DescriptorSet,
    ) {
        unsafe {
            self.device.logical().cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            self.device.logical().cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[descriptor_set], &[]);
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
}
