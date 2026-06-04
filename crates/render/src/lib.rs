pub mod instance;
pub mod device;
pub mod surface;
pub mod swapchain;
pub mod shader;
pub mod pipeline;
pub mod memory;
pub mod mesh;
pub mod components;
pub mod graph;
pub mod error;
pub mod texture;
pub mod renderer;
pub mod profiler;
pub mod bindless;
pub mod descriptor_cache;
pub mod sampler_cache;
pub mod descriptor_batch;
pub mod descriptor_allocator;
pub mod spec_constants;
pub mod spv_reflect;
pub mod hot_reload;
pub mod shader_include;
pub mod shader_archive;

// Re-export commonly used component types
pub use components::{Sprite, SpriteRenderer, DirectionalLight, PointLight, SpotLight, Camera};

// Re-export core types for convenience
pub use error::RenderError;
pub use renderer::Renderer;
pub use texture::{DepthBuffer, GpuTexture, Framebuffer, RenderTarget, HdrFramebuffer};

pub use rustix_core::config::RenderConfig;
pub use ash;
