//! Frame Graph: declarative render pass scheduling with automatic barrier insertion.
//!
//! Resources (textures, buffers) are declared with their format and usage.
//! Passes declare which resources they read/write. The graph compiles into
//! an ordered pass list with pipeline barriers between passes for correct
//! layout transitions and execution dependencies.

use ash::vk;
use std::collections::HashMap;

/// Handle to a graph resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(pub u32);

/// The type of a graph resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// A color attachment (sampled image, color render target, etc.)
    Texture,
    /// A depth/stencil attachment
    DepthBuffer,
}

/// Description of a texture resource in the graph.
#[derive(Debug, Clone)]
pub struct TextureDesc {
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub usage: vk::ImageUsageFlags,
    /// The image view (set before compilation if using an existing image)
    pub view: Option<vk::ImageView>,
    /// Whether this resource persists across frames (swapchain) or is transient
    pub persistent: bool,
}

/// Access mode for a resource in a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Resource is a color attachment written to
    ColorWrite,
    /// Resource is a depth attachment written to
    DepthWrite,
    /// Resource is read as a sampled texture
    ShaderRead,
    /// Resource is not accessed but barriers are needed
    TransferWrite,
}

/// Description of a single render pass in the graph.
#[derive(Debug, Clone)]
pub struct PassDesc {
    pub name: &'static str,
    /// Color attachment resources
    pub color_attachments: Vec<ResourceId>,
    /// Depth attachment (optional)
    pub depth_attachment: Option<ResourceId>,
    /// Resources read via samplers
    pub sampled_textures: Vec<ResourceId>,
    /// Clear the color attachments (true) or load (false)
    pub clear_color: bool,
    /// Clear depth (true) or load (false)
    pub clear_depth: bool,
    /// Clear color value
    pub clear_value: [f32; 4],
}

/// Barrier between passes computed by the graph.
#[derive(Debug, Clone)]
pub struct PassBarrier {
    /// Image memory barriers to insert before this pass
    pub before: Vec<vk::ImageMemoryBarrier2<'static>>,
    /// Image memory barriers to insert after this pass
    pub after: Vec<vk::ImageMemoryBarrier2<'static>>,
}

/// A compiled frame graph ready for execution.
pub struct FrameGraph {
    /// Resource declarations
    textures: Vec<TextureDesc>,
    /// Pass declarations
    passes: Vec<PassDesc>,
    /// Compiled barriers per pass
    barriers: Vec<PassBarrier>,
    /// Current state per resource (for incremental compilation)
    current_layouts: Vec<vk::ImageLayout>,
    current_access: Vec<vk::AccessFlags2>,
}

impl FrameGraph {
    pub fn new() -> Self {
        Self {
            textures: Vec::new(),
            passes: Vec::new(),
            barriers: Vec::new(),
            current_layouts: Vec::new(),
            current_access: Vec::new(),
        }
    }

    /// Add a texture resource. Returns its handle.
    pub fn add_texture(&mut self, desc: TextureDesc) -> ResourceId {
        let id = ResourceId(self.textures.len() as u32);
        self.current_layouts.push(vk::ImageLayout::UNDEFINED);
        self.current_access.push(vk::AccessFlags2::empty());
        self.textures.push(desc);
        id
    }

    /// Declare a render pass. Passes execute in declaration order.
    pub fn add_pass(&mut self, desc: PassDesc) {
        self.passes.push(desc);
    }

    /// Compile the graph: compute barriers for each pass based on state transitions.
    /// Must be called exactly once after all passes are added.
    pub fn compile(&mut self) {
        self.barriers.clear();

        for pass_idx in 0..self.passes.len() {
            let pass = &self.passes[pass_idx];
            let mut before = Vec::new();
            let mut after = Vec::new();

            // Barrier for each color attachment
            for &rid in &pass.color_attachments {
                let idx = rid.0 as usize;
                if idx >= self.textures.len() { continue; }

                let old_layout = self.current_layouts[idx];
                let old_access = self.current_access[idx];
                let new_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                let new_access = vk::AccessFlags2::COLOR_ATTACHMENT_WRITE;

                if old_layout != new_layout || old_access != new_access {
                    let barrier = vk::ImageMemoryBarrier2::default()
                        .old_layout(old_layout).new_layout(new_layout)
                        .src_access_mask(old_access).dst_access_mask(new_access)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0, level_count: 1,
                            base_array_layer: 0, layer_count: 1,
                        });
                    before.push(barrier);
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
                        let barrier = vk::ImageMemoryBarrier2::default()
                            .old_layout(old_layout).new_layout(new_layout)
                            .src_access_mask(old_access).dst_access_mask(new_access)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::DEPTH,
                                base_mip_level: 0, level_count: 1,
                                base_array_layer: 0, layer_count: 1,
                            });
                        before.push(barrier);
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
                    let barrier = vk::ImageMemoryBarrier2::default()
                        .old_layout(old_layout).new_layout(new_layout)
                        .src_access_mask(old_access).dst_access_mask(new_access)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0, level_count: 1,
                            base_array_layer: 0, layer_count: 1,
                        });
                    before.push(barrier);
                    self.current_layouts[idx] = new_layout;
                    self.current_access[idx] = new_access;
                }
            }

            // After a color-attachment pass, the resource stays in COLOR_ATTACHMENT_OPTIMAL
            // until the next pass transitions it elsewhere.
            // If this is the last pass writing to swapchain, transition to PRESENT_SRC.
            let is_last = pass_idx == self.passes.len() - 1;
            if is_last {
                for &rid in &pass.color_attachments {
                    let idx = rid.0 as usize;
                    if idx < self.textures.len() {
                        let barrier = vk::ImageMemoryBarrier2::default()
                            .old_layout(self.current_layouts[idx])
                            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                            .src_access_mask(self.current_access[idx])
                            .dst_access_mask(vk::AccessFlags2::empty())
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                base_mip_level: 0, level_count: 1,
                                base_array_layer: 0, layer_count: 1,
                            });
                        after.push(barrier);
                        self.current_layouts[idx] = vk::ImageLayout::PRESENT_SRC_KHR;
                        self.current_access[idx] = vk::AccessFlags2::empty();
                    }
                }
            }

            self.barriers.push(PassBarrier { before, after });
        }
    }

    /// Get barriers to execute before a pass.
    pub fn pass_barriers_before(&self, pass_idx: usize) -> &[vk::ImageMemoryBarrier2] {
        self.barriers.get(pass_idx).map(|b| b.before.as_slice()).unwrap_or(&[])
    }

    /// Get barriers to execute after a pass.
    pub fn pass_barriers_after(&self, pass_idx: usize) -> &[vk::ImageMemoryBarrier2] {
        self.barriers.get(pass_idx).map(|b| b.after.as_slice()).unwrap_or(&[])
    }

    /// Get the pass description.
    pub fn pass_desc(&self, pass_idx: usize) -> Option<&PassDesc> {
        self.passes.get(pass_idx)
    }

    /// Get a texture descriptor.
    pub fn texture(&self, id: ResourceId) -> Option<&TextureDesc> {
        self.textures.get(id.0 as usize)
    }

    /// Number of passes.
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }
}

/// Execute barriers on a command buffer.
pub unsafe fn execute_barriers(
    device: &ash::Device,
    cmd: vk::CommandBuffer,
    barriers: &[vk::ImageMemoryBarrier2],
) {
    if barriers.is_empty() { return; }
    let dep = vk::DependencyInfo::default().image_memory_barriers(barriers);
    device.cmd_pipeline_barrier2(cmd, &dep);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_graph() {
        let mut graph = FrameGraph::new();

        let swapchain = graph.add_texture(TextureDesc {
            format: vk::Format::B8G8R8A8_SRGB,
            extent: vk::Extent2D { width: 1920, height: 1080 },
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            view: None,
            persistent: true,
        });

        let depth = graph.add_texture(TextureDesc {
            format: vk::Format::D32_SFLOAT,
            extent: vk::Extent2D { width: 1920, height: 1080 },
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            view: None,
            persistent: false,
        });

        // 3D scene pass
        graph.add_pass(PassDesc {
            name: "scene",
            color_attachments: vec![swapchain],
            depth_attachment: Some(depth),
            sampled_textures: vec![],
            clear_color: true,
            clear_depth: true,
            clear_value: [0.04, 0.04, 0.08, 1.0],
        });

        // UI overlay pass
        graph.add_pass(PassDesc {
            name: "ui",
            color_attachments: vec![swapchain],
            depth_attachment: None,
            sampled_textures: vec![],
            clear_color: false,
            clear_depth: false,
            clear_value: [0.0; 4],
        });

        graph.compile();

        assert_eq!(graph.pass_count(), 2);

        // Scene pass should begin with UNDEFINED and transition to COLOR_ATTACHMENT_OPTIMAL
        assert!(!graph.pass_barriers_before(0).is_empty());
    }

    #[test]
    fn test_no_barrier_when_layout_unchanged() {
        let mut graph = FrameGraph::new();
        let tex = graph.add_texture(TextureDesc {
            format: vk::Format::B8G8R8A8_SRGB,
            extent: vk::Extent2D { width: 100, height: 100 },
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            view: None,
            persistent: false,
        });

        graph.add_pass(PassDesc {
            name: "p1", color_attachments: vec![tex], depth_attachment: None,
            sampled_textures: vec![], clear_color: true, clear_depth: false,
            clear_value: [0.0; 4],
        });
        graph.add_pass(PassDesc {
            name: "p2", color_attachments: vec![tex], depth_attachment: None,
            sampled_textures: vec![], clear_color: false, clear_depth: false,
            clear_value: [0.0; 4],
        });
        graph.compile();

        // Second pass should have NO barriers since layout is already COLOR_ATTACHMENT_OPTIMAL
        assert!(graph.pass_barriers_before(1).is_empty());
    }
}
