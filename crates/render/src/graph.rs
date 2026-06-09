//! Frame Graph: declarative render pass scheduling with automatic barrier insertion.
//!
//! Resources (textures, buffers) are declared with their format and usage.
//! Passes declare which resources they read/write. The graph compiles into
//! an ordered pass list with pipeline barriers between passes for correct
//! layout transitions and execution dependencies.

use ash::vk;
use crate::renderer::Renderer;
use crate::RenderError;

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
    /// The Vulkan image handle (required for persistent resources; set by allocate_transient_resources for transient ones)
    pub image: Option<vk::Image>,
    /// Whether this resource persists across frames (swapchain) or is transient
    pub persistent: bool,
}

/// A single layout transition for a resource.
#[derive(Debug, Clone, Copy)]
pub struct BarrierTransition {
    pub resource_idx: usize,
    pub old_layout: vk::ImageLayout,
    pub new_layout: vk::ImageLayout,
    pub src_access: vk::AccessFlags2,
    pub dst_access: vk::AccessFlags2,
    pub src_stage: vk::PipelineStageFlags2,
    pub dst_stage: vk::PipelineStageFlags2,
    pub aspect_mask: vk::ImageAspectFlags,
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

/// Which Vulkan queue a pass executes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassQueue {
    /// Graphics queue with dynamic rendering support.
    Graphics,
    /// Async compute queue (dedicated or shared).
    Compute,
}

impl Default for PassQueue {
    fn default() -> Self { PassQueue::Graphics }
}

/// Description of a single render pass in the graph.
#[derive(Debug, Clone)]
pub struct PassDesc {
    pub name: &'static str,
    /// Which queue this pass executes on.
    pub queue: PassQueue,
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
    /// Layout transitions to insert before this pass (barriers built at execute time with correct image handles)
    pub before: Vec<BarrierTransition>,
    /// Layout transitions to insert after this pass
    pub after: Vec<BarrierTransition>,
}

/// Context provided to a pass callback during graph execution.
pub struct PassContext<'a> {
    pub cmd: vk::CommandBuffer,
    pub renderer: &'a Renderer,
    pub device: &'a ash::Device,
    /// Color attachment views for this pass (in the same order as `PassDesc::color_attachments`).
    pub color_views: &'a [vk::ImageView],
    /// Depth attachment view, if present.
    pub depth_view: Option<vk::ImageView>,
    /// Pass index within the graph.
    pub pass_index: usize,
    /// Pass name.
    pub name: &'static str,
}

/// Trait for pass execution callbacks.
pub trait PassCallback {
    fn execute(&self, ctx: &mut PassContext);
}

impl<F: Fn(&mut PassContext)> PassCallback for F {
    fn execute(&self, ctx: &mut PassContext) { self(ctx); }
}

/// Internal node combining pass description with its callback.
struct PassNode<'a> {
    desc: PassDesc,
    callback: Box<dyn Fn(&mut PassContext) + 'a>,
}

/// A GPU image allocated by the graph for a transient resource.
/// Multiple transient images with non-overlapping lifetimes may be
/// bound to the same backing memory at different offsets.
struct TransientImage {
    image: vk::Image,
    view: vk::ImageView,
}

/// A compiled frame graph ready for execution.
pub struct FrameGraph<'a> {
    /// Resource declarations
    textures: Vec<TextureDesc>,
    /// Per-resource Vulkan image view (set by the user or created for transient resources).
    views: Vec<Option<vk::ImageView>>,
    /// Per-resource Vulkan image handle (set by user or allocate_transient_resources).
    images: Vec<Option<vk::Image>>,
    /// Pass declarations + callbacks
    passes: Vec<PassNode<'a>>,
    /// Compiled barriers per pass
    barriers: Vec<PassBarrier>,
    /// Current state per resource (for incremental compilation)
    current_layouts: Vec<vk::ImageLayout>,
    current_access: Vec<vk::AccessFlags2>,
    /// Resource lifetimes computed during compilation: (first_pass, last_pass).
    lifetimes: Vec<Option<(usize, usize)>>,
    /// Transient images allocated with aliased memory.
    transient_images: Vec<TransientImage>,
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
    pub fn add_texture(&mut self, desc: TextureDesc) -> ResourceId {
        let id = ResourceId(self.textures.len() as u32);
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
    pub fn set_view(&mut self, id: ResourceId, view: vk::ImageView) {
        let idx = id.0 as usize;
        if idx < self.views.len() {
            self.views[idx] = Some(view);
        }
    }

    /// Override the initial tracked layout for a persistent resource.
    /// By default add_texture assumes UNDEFINED; call this after add_texture
    /// for resources that are already in a known layout (e.g. swapchain in PRESENT_SRC_KHR).
    pub fn set_initial_layout(&mut self, id: ResourceId, layout: vk::ImageLayout) {
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
    fn build_barriers(&self, transitions: &[BarrierTransition]) -> Vec<vk::ImageMemoryBarrier2<'static>> {
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
    pub fn add_pass<F>(&mut self, desc: PassDesc, callback: F)
    where
        F: Fn(&mut PassContext) + 'a,
    {
        self.passes.push(PassNode { desc, callback: Box::new(callback) });
    }

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

    /// Get barriers to execute before a pass.
    pub fn pass_barriers_before(&self, pass_idx: usize) -> &[BarrierTransition] {
        self.barriers.get(pass_idx).map(|b| b.before.as_slice()).unwrap_or(&[])
    }

    /// Get barriers to execute after a pass.
    pub fn pass_barriers_after(&self, pass_idx: usize) -> &[BarrierTransition] {
        self.barriers.get(pass_idx).map(|b| b.after.as_slice()).unwrap_or(&[])
    }

    /// Get the pass description.
    pub fn pass_desc(&self, pass_idx: usize) -> Option<&PassDesc> {
        self.passes.get(pass_idx).map(|n| &n.desc)
    }

    /// Get a texture descriptor.
    pub fn texture(&self, id: ResourceId) -> Option<&TextureDesc> {
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
    pub fn pass_descs(&self) -> Vec<&PassDesc> {
        self.passes.iter().map(|p| &p.desc).collect()
    }

    /// Access all compiled barriers (in pass order).
    pub fn barriers(&self) -> &[PassBarrier] {
        &self.barriers
    }

    /// Access all texture/resource descriptors.
    pub fn textures(&self) -> &[TextureDesc] {
        &self.textures
    }

    /// Resource lifetimes computed during compilation: `(first_pass, last_pass)`.
    /// `None` for resources that were never referenced by any pass.
    pub fn resource_lifetimes(&self) -> &[Option<(usize, usize)>] {
        &self.lifetimes
    }

    /// Create a lightweight snapshot of the graph structure for visualization.
    /// The snapshot contains no Vulkan handles or callbacks and is safe to keep across frames.
    pub fn snapshot(&self) -> FrameGraphSnapshot {
        FrameGraphSnapshot {
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

/// Lightweight snapshot of a `FrameGraph` containing only metadata.
/// Safe to store and inspect across frames since it holds no callbacks or GPU handles.
#[derive(Debug, Clone)]
pub struct FrameGraphSnapshot {
    pub passes: Vec<PassDesc>,
    pub textures: Vec<TextureDesc>,
    pub barriers: Vec<PassBarrier>,
    pub lifetimes: Vec<Option<(usize, usize)>>,
    pub merged_groups: Vec<(usize, usize)>,
    pub transient_memory_size: u64,
    pub transient_image_count: usize,
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
#[path = "graph_tests.rs"]
mod tests;
