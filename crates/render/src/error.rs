use ash::vk;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Vulkan: {0}")]
    Vulkan(#[from] vk::Result),
    #[error("instance: {0}")]
    InstanceCreation(String),
    #[error("device: {0}")]
    DeviceCreation(String),
    #[error("surface: {0}")]
    SurfaceCreation(String),
    #[error("swapchain: {0}")]
    SwapchainCreation(String),
    #[error("shader: {0}")]
    ShaderCompile(String),
    #[error("pipeline: {0}")]
    PipelineCreation(String),
}
