use ash::vk;
use crate::device::GpuDevice;
use crate::instance::VulkanInstance;
use crate::error::RenderError;
use crate::renderer::Renderer;

pub struct DepthBuffer {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub(crate) _allocation: gpu_allocator::vulkan::Allocation,
}

pub struct GpuTexture {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub(crate) _allocation: gpu_allocator::vulkan::Allocation,
}

pub struct Framebuffer {
    pub color_image: vk::Image,
    pub color_view: vk::ImageView,
    pub depth_buffer: DepthBuffer,
    pub extent: vk::Extent2D,
    pub(crate) _color_allocation: gpu_allocator::vulkan::Allocation,
}

impl Framebuffer {
    pub fn new(renderer: &Renderer, width: u32, height: u32, format: vk::Format) -> Result<Self, RenderError> {
        let extent = vk::Extent2D { width, height };

        let color_img = unsafe {
            renderer.device.logical().create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(format)
                    .extent(vk::Extent3D { width, height, depth: 1 })
                    .mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            ).map_err(|e| RenderError::DeviceCreation(format!("color img: {e}")))?
        };

        let color_req = unsafe { renderer.device.logical().get_image_memory_requirements(color_img) };
        let color_alloc = renderer.allocator.lock().allocate("framebuffer_color", color_req, gpu_allocator::MemoryLocation::GpuOnly, false)?;
        unsafe { renderer.device.logical().bind_image_memory(color_img, color_alloc.memory(), color_alloc.offset())?; }

        let color_view = unsafe {
            renderer.device.logical().create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(color_img).view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0, level_count: 1,
                        base_array_layer: 0, layer_count: 1,
                    }),
                None,
            ).map_err(|e| RenderError::DeviceCreation(format!("color view: {e}")))?
        };

        let depth = renderer.create_depth_buffer(extent)?;

        Ok(Self {
            color_image: color_img,
            color_view,
            depth_buffer: depth,
            extent,
            _color_allocation: color_alloc,
        })
    }

    pub fn prepare_rendering(&self, cmd: vk::CommandBuffer, device: &GpuDevice, instance: &VulkanInstance) {
        unsafe {
            let barrier = vk::ImageMemoryBarrier2::default()
                .image(self.color_image)
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .src_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(vk::AccessFlags2::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            let barriers = [barrier];
            let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
            device.logical().cmd_pipeline_barrier2(cmd, &dep);
        }
        self.begin_rendering(cmd, device, instance);
    }

    pub fn begin_rendering(&self, cmd: vk::CommandBuffer, device: &GpuDevice, instance: &VulkanInstance) {
        let ca = vk::RenderingAttachmentInfoKHR::default()
            .image_view(self.color_view).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.1, 0.1, 0.12, 1.0] } });
        let da = vk::RenderingAttachmentInfoKHR::default()
            .image_view(self.depth_buffer.view).image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let cas = [ca];

        let ri = vk::RenderingInfoKHR::default()
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: self.extent })
            .layer_count(1).color_attachments(&cas).depth_attachment(&da);

        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&instance.inner(), device.logical());
            dr.cmd_begin_rendering(cmd, &ri);
            device.logical().cmd_set_viewport(cmd, 0, &[vk::Viewport {
                x: 0.0, y: self.extent.height as f32,
                width: self.extent.width as f32,
                height: -(self.extent.height as f32),
                min_depth: 0.0, max_depth: 1.0,
            }]);
            device.logical().cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: self.extent }]);
        }
    }

    pub fn end_rendering(&self, cmd: vk::CommandBuffer, device: &GpuDevice, instance: &VulkanInstance) {
        unsafe {
            let dr = ash::khr::dynamic_rendering::Device::new(&instance.inner(), device.logical());
            dr.cmd_end_rendering(cmd);
            let barrier = vk::ImageMemoryBarrier2::default()
                .image(self.color_image)
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags2::TRANSFER_READ)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            let barriers = [barrier];
            let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
            device.logical().cmd_pipeline_barrier2(cmd, &dep);
        }
    }

    pub fn copy_to_buffer(&self, device: &GpuDevice, cmd: vk::CommandBuffer, dst_buffer: vk::Buffer) -> Result<(), RenderError> {
        let extent = self.extent;
        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 });
        unsafe {
            device.logical().cmd_copy_image_to_buffer(cmd, self.color_image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, dst_buffer, &[region]);
        }
        Ok(())
    }
}
