//! VkDebugUtils labeling for objects and command buffer regions.
//!
//! Stub: real implementation requires ash API compatibility layer.

use ash::vk;

/// Label a Vulkan object for debugging.
pub unsafe fn label_object(
    _device: &ash::Device,
    _object: vk::ObjectType,
    _handle: u64,
    _name: &str,
) {
    // Stub: requires ash::ext::debug_utils::Device
}

/// Begin a labeled command buffer region for debugging/profiling.
pub unsafe fn begin_label(
    _cmd: vk::CommandBuffer,
    _name: &str,
    _color: [f32; 4],
) {
    // Stub: requires ash::ext::debug_utils::Device
}

/// End a labeled command buffer region.
pub unsafe fn end_label(
    _cmd: vk::CommandBuffer,
) {
    // Stub: requires ash::ext::debug_utils::Device
}
