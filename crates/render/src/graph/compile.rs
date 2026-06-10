use ash::vk;
use super::{FrameGraph, BarrierTransition, PassBarrier};

impl<'a> FrameGraph<'a> {
    /// Compile the graph: compute barriers for each pass based on state transitions.
    /// Must be called exactly once after all passes are added and before `execute()`.
    pub fn compile(&mut self) {
        self.barriers.clear();

        for pass_idx in 0..self.passes.len() {
            let pass = &self.passes[pass_idx].desc;
            let mut before = Vec::new();
            let after = Vec::new();

            // Barrier for each color attachment
            for &rid in &pass.color_attachments {
                let idx = rid.0 as usize;
                if idx >= self.textures.len() { continue; }

                let old_layout = self.current_layouts[idx];
                let old_access = self.current_access[idx];
                let new_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                let new_access = vk::AccessFlags2::COLOR_ATTACHMENT_WRITE;

                if old_layout != new_layout || old_access != new_access {
                    let src_stage = if old_layout == vk::ImageLayout::UNDEFINED || old_layout == vk::ImageLayout::PRESENT_SRC_KHR {
                        vk::PipelineStageFlags2::TOP_OF_PIPE
                    } else {
                        vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT
                    };
                    before.push(BarrierTransition {
                        resource_idx: idx,
                        old_layout, new_layout,
                        src_access: old_access, dst_access: new_access,
                        src_stage, dst_stage: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                    });
                    self.current_layouts[idx] = new_layout;
                    self.current_access[idx] = new_access;
                }
            }

            // Barrier for depth attachment
            if let Some(rid) = pass.depth_attachment {
                let idx = rid.0 as usize;
                if idx < self.textures.len() {
                    let old_layout = self.current_layouts[idx];
                    let old_access = self.current_access[idx];
                    let new_layout = vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
                    let new_access = vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE;

                    if old_layout != new_layout || old_access != new_access {
                        let src_stage = if old_layout == vk::ImageLayout::UNDEFINED {
                            vk::PipelineStageFlags2::TOP_OF_PIPE
                        } else {
                            vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS
                        };
                        before.push(BarrierTransition {
                            resource_idx: idx,
                            old_layout, new_layout,
                            src_access: old_access, dst_access: new_access,
                            src_stage, dst_stage: vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS,
                            aspect_mask: vk::ImageAspectFlags::DEPTH,
                        });
                        self.current_layouts[idx] = new_layout;
                        self.current_access[idx] = new_access;
                    }
                }
            }

            // Barrier for sampled textures (transition to shader read)
            for &rid in &pass.sampled_textures {
                let idx = rid.0 as usize;
                if idx >= self.textures.len() { continue; }

                let old_layout = self.current_layouts[idx];
                let old_access = self.current_access[idx];
                let new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                let new_access = vk::AccessFlags2::SHADER_READ;

                if old_layout != new_layout || old_access != new_access {
                    let aspect = if self.textures[idx].format == vk::Format::D32_SFLOAT {
                        vk::ImageAspectFlags::DEPTH
                    } else {
                        vk::ImageAspectFlags::COLOR
                    };
                    let src_stage = match old_layout {
                        vk::ImageLayout::UNDEFINED => vk::PipelineStageFlags2::TOP_OF_PIPE,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                        vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL => vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS,
                        _ => vk::PipelineStageFlags2::ALL_COMMANDS,
                    };
                    before.push(BarrierTransition {
                        resource_idx: idx,
                        old_layout, new_layout,
                        src_access: old_access, dst_access: new_access,
                        src_stage, dst_stage: vk::PipelineStageFlags2::FRAGMENT_SHADER | vk::PipelineStageFlags2::COMPUTE_SHADER,
                        aspect_mask: aspect,
                    });
                    self.current_layouts[idx] = new_layout;
                    self.current_access[idx] = new_access;
                }
            }

            // NOTE: We intentionally do NOT transition to PRESENT_SRC_KHR here.
            // The caller (e.g. egui manual rendering) may draw after the frame graph
            // on the same command buffer. The final PRESENT_SRC_KHR transition is handled
            // by Renderer::end_frame().

            self.barriers.push(PassBarrier { before, after });
        }

        // Compute resource lifetimes for transient aliasing.
        self.lifetimes.clear();
        self.lifetimes.resize(self.textures.len(), None);
        for pass_idx in 0..self.passes.len() {
            let pass = &self.passes[pass_idx].desc;
            for &rid in pass.color_attachments.iter()
                .chain(pass.depth_attachment.iter())
                .chain(pass.sampled_textures.iter())
            {
                let idx = rid.0 as usize;
                if idx >= self.lifetimes.len() { continue; }
                match self.lifetimes[idx] {
                    None => self.lifetimes[idx] = Some((pass_idx, pass_idx)),
                    Some((first, _)) => self.lifetimes[idx] = Some((first, pass_idx)),
                }
            }
        }

        // Compute merged pass groups.
        // Two consecutive passes are mergeable when:
        //   - they are on the same queue (Graphics or Compute)
        //   - they have the same color attachments (same resources, same order)
        //   - they have the same depth attachment (same resource or both None)
        //   - no after-barriers on the first and no before-barriers on the second
        self.merged_groups.clear();
        if self.passes.is_empty() { return; }

        let mut start = 0usize;
        for i in 0..self.passes.len().saturating_sub(1) {
            let desc_a = &self.passes[i].desc;
            let desc_b = &self.passes[i + 1].desc;
            let can_merge = {
                let same_queue = desc_a.queue == desc_b.queue;
                let same_colors = desc_a.color_attachments.len() == desc_b.color_attachments.len()
                    && desc_a.color_attachments.iter().zip(&desc_b.color_attachments).all(|(a, b)| a == b);
                let same_depth = desc_a.depth_attachment == desc_b.depth_attachment;
                let no_barrier_gap = {
                    let after_empty = self.barriers.get(i).map(|b| b.after.is_empty()).unwrap_or(true);
                    let before_empty = self.barriers.get(i + 1).map(|b| b.before.is_empty()).unwrap_or(true);
                    after_empty && before_empty
                };
                same_queue && same_colors && same_depth && no_barrier_gap
            };
            if !can_merge {
                self.merged_groups.push((start, i));
                start = i + 1;
            }
        }
        self.merged_groups.push((start, self.passes.len() - 1));
    }
}
