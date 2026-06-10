use ash::vk;
use crate::renderer::Renderer;

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

/// A GPU image allocated by the graph for a transient resource.
/// Multiple transient images with non-overlapping lifetimes may be
/// bound to the same backing memory at different offsets.
pub(crate) struct TransientImage {
    pub(crate) image: vk::Image,
    pub(crate) view: vk::ImageView,
}

/// A lightweight snapshot of the graph structure for visualization.
/// The snapshot contains no Vulkan handles or callbacks and is safe to keep across frames.
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
