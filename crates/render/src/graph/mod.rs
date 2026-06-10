//! Frame Graph: declarative render pass scheduling with automatic barrier insertion.
//!
//! Resources (textures, buffers) are declared with their format and usage.
//! Passes declare which resources they read/write. The graph compiles into
//! an ordered pass list with pipeline barriers between passes for correct
//! layout transitions and execution dependencies.

use ash::vk;
use crate::renderer::Renderer;
use crate::RenderError;

mod types;
mod compile;
mod execute;
mod transient;

pub use types::*;

/// Internal node combining pass description with its callback.
struct PassNode<'a> {
    desc: types::PassDesc,
    callback: Box<dyn Fn(&mut types::PassContext) + 'a>,
}

/// A compiled frame graph ready for execution.
pub struct FrameGraph<'a> {
    /// Resource declarations
    textures: Vec<types::TextureDesc>,
    /// Per-resource Vulkan image view (set by the user or created for transient resources).
    views: Vec<Option<vk::ImageView>>,
    /// Per-resource Vulkan image handle (set by user or allocate_transient_resources).
    images: Vec<Option<vk::Image>>,
    /// Pass declarations + callbacks
    passes: Vec<PassNode<'a>>,
    /// Compiled barriers per pass
    barriers: Vec<types::PassBarrier>,
    /// Current state per resource (for incremental compilation)
    current_layouts: Vec<vk::ImageLayout>,
    current_access: Vec<vk::AccessFlags2>,
    /// Resource lifetimes computed during compilation: (first_pass, last_pass).
    lifetimes: Vec<Option<(usize, usize)>>,
    /// Transient images allocated with aliased memory.
    transient_images: Vec<types::TransientImage>,
    /// Shared device memory block backing all transient images.
    transient_memory: Option<vk::DeviceMemory>,
    /// Total size of the aliased memory block.
    transient_memory_size: u64,
    /// Device pointer for cleanup.
    device: *const ash::Device,
    /// Merged pass groups: each tuple is (start_pass, end_pass) inclusive.
    merged_groups: Vec<(usize, usize)>,
}

impl<'a> FrameGraph<'a> {
    pub fn new() -> Self {
        Self {
            textures: Vec::new(),
            views: Vec::new(),
            images: Vec::new(),
            passes: Vec::new(),
            barriers: Vec::new(),
            current_layouts: Vec::new(),
            current_access: Vec::new(),
            lifetimes: Vec::new(),
            transient_images: Vec::new(),
            transient_memory: None,
            transient_memory_size: 0,
            device: std::ptr::null(),
            merged_groups: Vec::new(),
        }
    }

    /// Add a texture resource. Returns its handle.
    pub fn add_texture(&mut self, desc: types::TextureDesc) -> types::ResourceId {
        let id = types::ResourceId(self.textures.len() as u32);
        self.current_layouts.push(vk::ImageLayout::UNDEFINED);
        self.current_access.push(vk::AccessFlags2::empty());
        self.views.push(desc.view);
        self.images.push(desc.image);
        self.lifetimes.push(None);
        self.textures.push(desc);
        id
    }

    /// Bind a Vulkan image view to a graph resource. Must be called before
    /// `execute()` for every resource used as an attachment.
    pub fn set_view(&mut self, id: types::ResourceId, view: vk::ImageView) {
        let idx = id.0 as usize;
        if idx < self.views.len() {
            self.views[idx] = Some(view);
        }
    }

    /// Override the initial tracked layout for a persistent resource.
    /// By default add_texture assumes UNDEFINED; call this after add_texture
    /// for resources that are already in a known layout (e.g. swapchain in PRESENT_SRC_KHR).
    pub fn set_initial_layout(&mut self, id: types::ResourceId, layout: vk::ImageLayout) {
        let idx = id.0 as usize;
        if idx < self.current_layouts.len() {
            self.current_layouts[idx] = layout;
            self.current_access[idx] = match layout {
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
                vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL => vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => vk::AccessFlags2::SHADER_READ,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL => vk::AccessFlags2::TRANSFER_READ,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL => vk::AccessFlags2::TRANSFER_WRITE,
                _ => vk::AccessFlags2::NONE,
            };
        }
    }

    /// Build Vulkan image memory barriers from transition descriptions using the stored image handles.
    pub(crate) fn build_barriers(&self, transitions: &[types::BarrierTransition]) -> Vec<vk::ImageMemoryBarrier2<'static>> {
        let mut barriers = Vec::with_capacity(transitions.len());
        for bt in transitions.iter() {
            let image = self.images.get(bt.resource_idx).copied().flatten().unwrap_or(vk::Image::null());
            if image == vk::Image::null() { continue; }
            barriers.push(
                vk::ImageMemoryBarrier2::default()
                    .image(image)
                    .old_layout(bt.old_layout)
                    .new_layout(bt.new_layout)
                    .src_access_mask(bt.src_access)
                    .dst_access_mask(bt.dst_access)
                    .src_stage_mask(bt.src_stage)
                    .dst_stage_mask(bt.dst_stage)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: bt.aspect_mask,
                        base_mip_level: 0, level_count: 1,
                        base_array_layer: 0, layer_count: 1,
                    }),
            );
        }
        barriers
    }

    /// Declare a render pass with an execution callback.
    pub fn add_pass<F>(&mut self, desc: types::PassDesc, callback: F)
    where
        F: Fn(&mut types::PassContext) + 'a,
    {
        self.passes.push(PassNode { desc, callback: Box::new(callback) });
    }

    /// Get barriers to execute before a pass.
    pub fn pass_barriers_before(&self, pass_idx: usize) -> &[types::BarrierTransition] {
        self.barriers.get(pass_idx).map(|b| b.before.as_slice()).unwrap_or(&[])
    }

    /// Get barriers to execute after a pass.
    pub fn pass_barriers_after(&self, pass_idx: usize) -> &[types::BarrierTransition] {
        self.barriers.get(pass_idx).map(|b| b.after.as_slice()).unwrap_or(&[])
    }

    /// Get the pass description.
    pub fn pass_desc(&self, pass_idx: usize) -> Option<&types::PassDesc> {
        self.passes.get(pass_idx).map(|n| &n.desc)
    }

    /// Get a texture descriptor.
    pub fn texture(&self, id: types::ResourceId) -> Option<&types::TextureDesc> {
        self.textures.get(id.0 as usize)
    }

    /// Number of passes.
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Merged pass groups from compilation (start, end) inclusive.
    pub fn merged_groups(&self) -> &[(usize, usize)] {
        &self.merged_groups
    }

    /// Total bytes allocated for transient aliased memory (0 if none).
    pub fn transient_memory_size(&self) -> u64 {
        self.transient_memory_size
    }

    /// Number of transient images allocated by the graph.
    pub fn transient_image_count(&self) -> usize {
        self.transient_images.len()
    }

    /// Access all pass descriptors (in declaration order).
    pub fn pass_descs(&self) -> Vec<&types::PassDesc> {
        self.passes.iter().map(|p| &p.desc).collect()
    }

    /// Access all compiled barriers (in pass order).
    pub fn barriers(&self) -> &[types::PassBarrier] {
        &self.barriers
    }

    /// Access all texture/resource descriptors.
    pub fn textures(&self) -> &[types::TextureDesc] {
        &self.textures
    }

    /// Resource lifetimes computed during compilation: `(first_pass, last_pass)`.
    /// `None` for resources that were never referenced by any pass.
    pub fn resource_lifetimes(&self) -> &[Option<(usize, usize)>] {
        &self.lifetimes
    }

    /// Create a lightweight snapshot of the graph structure for visualization.
    /// The snapshot contains no Vulkan handles or callbacks and is safe to keep across frames.
    pub fn snapshot(&self) -> types::FrameGraphSnapshot {
        types::FrameGraphSnapshot {
            passes: self.passes.iter().map(|p| p.desc.clone()).collect(),
            textures: self.textures.clone(),
            barriers: self.barriers.clone(),
            lifetimes: self.lifetimes.clone(),
            merged_groups: self.merged_groups.clone(),
            transient_memory_size: self.transient_memory_size,
            transient_image_count: self.transient_images.len(),
        }
    }
}

impl<'a> Drop for FrameGraph<'a> {
    fn drop(&mut self) {
        self.destroy_transient_resources();
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
mod tests;
