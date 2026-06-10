use ash::vk;
use crate::renderer::Renderer;
use super::{FrameGraph, PassQueue, PassContext};

impl<'a> FrameGraph<'a> {
    /// Execute the compiled graph.
    /// Passes that were merged during compilation share a single dynamic rendering
    /// scope: before-barriers of the first pass, one begin/end rendering, then
    /// after-barriers of the last pass.
    /// Compute passes are recorded into a separate command buffer and submitted to
    /// the async compute queue; the renderer's graphics submit waits on them.
    pub fn execute(&self, renderer: &Renderer, cmd: vk::CommandBuffer) {
        let device = renderer.device().logical();

        // ---- Compute passes: record into the renderer's per-frame compute CB ----
        let compute_indices: Vec<usize> = self.passes.iter().enumerate()
            .filter(|(_, p)| p.desc.queue == PassQueue::Compute)
            .map(|(i, _)| i)
            .collect();
        if !compute_indices.is_empty() {
            let compute_cmd = renderer.compute_cmd();
            unsafe {
                device.reset_command_buffer(compute_cmd, vk::CommandBufferResetFlags::empty()).ok();
                let begin_info = vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
                device.begin_command_buffer(compute_cmd, &begin_info).ok();
            }
            for &pass_idx in &compute_indices {
                if let Some(barrier) = self.barriers.get(pass_idx) {
                    let before = self.build_barriers(&barrier.before);
                    unsafe {
                        if !before.is_empty() {
                            let dep = vk::DependencyInfo::default().image_memory_barriers(&before);
                            device.cmd_pipeline_barrier2(compute_cmd, &dep);
                        }
                    }
                }
                let pass = &self.passes[pass_idx];
                let mut ctx = PassContext {
                    cmd: compute_cmd,
                    renderer,
                    device,
                    color_views: &[],
                    depth_view: None,
                    pass_index: pass_idx,
                    name: pass.desc.name,
                };
                (pass.callback)(&mut ctx);
                if let Some(barrier) = self.barriers.get(pass_idx) {
                    let after = self.build_barriers(&barrier.after);
                    unsafe {
                        if !after.is_empty() {
                            let dep = vk::DependencyInfo::default().image_memory_barriers(&after);
                            device.cmd_pipeline_barrier2(compute_cmd, &dep);
                        }
                    }
                }
            }
            unsafe {
                device.end_command_buffer(compute_cmd).ok();
            }
            let cmds = [compute_cmd];
            let signal_sems = [renderer.compute_sync_semaphore()];
            let submit_info = vk::SubmitInfo::default()
                .command_buffers(&cmds)
                .signal_semaphores(&signal_sems);
            let submits = [submit_info];
            unsafe {
                device.queue_submit(renderer.device().compute_queue(), &submits, vk::Fence::null()).ok();
            }
            renderer.notify_compute_submitted();
        }

        // ---- Graphics passes: merged groups with dynamic rendering ----
        for &(start, end) in &self.merged_groups {
            let first_pass = &self.passes[start];
            let first_desc = &first_pass.desc;
            if first_desc.queue != PassQueue::Graphics {
                continue;
            }

            // Apply before barriers of the first pass in the group
            if let Some(barrier) = self.barriers.get(start) {
                let before = self.build_barriers(&barrier.before);
                unsafe {
                    if !before.is_empty() {
                        let dep = vk::DependencyInfo::default().image_memory_barriers(&before);
                        device.cmd_pipeline_barrier2(cmd, &dep);
                    }
                }
            }

            // Build color attachments using the FIRST pass's clear settings
            let mut color_attachments: Vec<vk::RenderingAttachmentInfoKHR<'_>> = Vec::new();
            for &rid in &first_desc.color_attachments {
                let idx = rid.0 as usize;
                let view = self.views.get(idx).copied().flatten().unwrap_or(vk::ImageView::null());
                let ca = vk::RenderingAttachmentInfoKHR::default()
                    .image_view(view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                let ca = if first_desc.clear_color {
                    ca.load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: first_desc.clear_value } })
                } else {
                    ca.load_op(vk::AttachmentLoadOp::LOAD).store_op(vk::AttachmentStoreOp::STORE)
                };
                color_attachments.push(ca);
            }

            // Build depth attachment using the FIRST pass's clear settings
            let mut depth_attachment_opt: Option<vk::RenderingAttachmentInfoKHR<'_>> = None;
            if let Some(rid) = first_desc.depth_attachment {
                let idx = rid.0 as usize;
                let view = self.views.get(idx).copied().flatten().unwrap_or(vk::ImageView::null());
                let da = vk::RenderingAttachmentInfoKHR::default()
                    .image_view(view)
                    .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
                let da = if first_desc.clear_depth {
                    da.load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } })
                } else {
                    da.load_op(vk::AttachmentLoadOp::LOAD).store_op(vk::AttachmentStoreOp::STORE)
                };
                depth_attachment_opt = Some(da);
            }

            // Compute extent from first color attachment (or depth if no colors)
            let extent = first_desc.color_attachments.first()
                .and_then(|&rid| self.textures.get(rid.0 as usize).map(|t| t.extent))
                .or_else(|| first_desc.depth_attachment
                    .and_then(|rid| self.textures.get(rid.0 as usize).map(|t| t.extent)))
                .unwrap_or(vk::Extent2D { width: 1, height: 1 });

            // Pre-collect color views for the PassContext
            let color_views: Vec<vk::ImageView> = first_desc.color_attachments.iter()
                .map(|&rid| self.views.get(rid.0 as usize).copied().flatten().unwrap_or(vk::ImageView::null()))
                .collect();

            let mut ri = vk::RenderingInfoKHR::default()
                .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent })
                .layer_count(1)
                .color_attachments(&color_attachments);
            if let Some(ref da) = depth_attachment_opt {
                let da_ref: &[vk::RenderingAttachmentInfoKHR] = std::slice::from_ref(da);
                ri = ri.depth_attachment(da_ref.first().unwrap());
            }

            unsafe {
                let dr = ash::khr::dynamic_rendering::Device::new(renderer.instance.inner(), device);
                dr.cmd_begin_rendering(cmd, &ri);
            }

            // Set default viewport / scissor once per merged group
            unsafe {
                device.cmd_set_viewport(cmd, 0, &[vk::Viewport {
                    x: 0.0, y: extent.height as f32,
                    width: extent.width as f32, height: -(extent.height as f32),
                    min_depth: 0.0, max_depth: 1.0,
                }]);
                device.cmd_set_scissor(cmd, 0, &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 }, extent,
                }]);
            }

            // Execute each callback in the merged group
            let depth_view = depth_attachment_opt.map(|_| {
                first_desc.depth_attachment
                    .and_then(|rid| self.views.get(rid.0 as usize).copied().flatten())
                    .unwrap_or(vk::ImageView::null())
            });
            for pass_idx in start..=end {
                let pass = &self.passes[pass_idx];
                let mut ctx = PassContext {
                    cmd,
                    renderer,
                    device,
                    color_views: &color_views,
                    depth_view,
                    pass_index: pass_idx,
                    name: pass.desc.name,
                };
                (pass.callback)(&mut ctx);
            }

            // End dynamic rendering
            unsafe {
                let dr = ash::khr::dynamic_rendering::Device::new(renderer.instance.inner(), device);
                dr.cmd_end_rendering(cmd);
            }

            // Apply after barriers of the LAST pass in the group
            if let Some(barrier) = self.barriers.get(end) {
                let after = self.build_barriers(&barrier.after);
                unsafe {
                    if !after.is_empty() {
                        let dep = vk::DependencyInfo::default().image_memory_barriers(&after);
                        device.cmd_pipeline_barrier2(cmd, &dep);
                    }
                }
            }
        }
    }
}
