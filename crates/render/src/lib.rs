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
pub mod gizmo;
pub mod secondary_cmd;
pub mod msaa;
pub mod renderdoc;
pub mod debug_label;
pub mod tracy_gpu;
pub mod pbr;
pub mod sh;
pub mod wireframe;
pub mod particle;
pub mod particle_gpu;

// Re-export commonly used component types
pub use components::{Sprite, SpriteRenderer, DirectionalLight, PointLight, SpotLight, Camera, ParticleEmitter, PostProcessSettings};
pub use particle::{ParticleInstance, ParticleSimulation, GpuParticle};
pub use particle_gpu::{GpuParticleSimulation, ParticleSimParams, ParticleSortParams};
pub use wireframe::{WireframeMode, DebugOverlay, DebugRenderMode};

// Re-export gizmo types
pub use gizmo::{GizmoVertex, GizmoLine, AudioGizmo, wireframe_sphere, wireframe_cone, wireframe_box, generate_audio_gizmo, flatten_gizmo_lines};

// Re-export core types for convenience
pub use error::RenderError;
pub use renderer::Renderer;
pub use texture::{DepthBuffer, GpuTexture, Framebuffer, RenderTarget, HdrFramebuffer};

pub use rustix_core::config::RenderConfig;
pub use ash;

#[cfg(test)]
pub mod components_tests;
#[cfg(test)]
pub mod wireframe_tests;
#[cfg(test)]
pub mod spec_constants_tests;
#[cfg(test)]
pub mod msaa_tests;
#[cfg(test)]
pub mod error_tests;
#[cfg(test)]
pub mod renderdoc_tests;
#[cfg(test)]
pub mod tracy_gpu_tests;
#[cfg(test)]
pub mod hot_reload_tests;
#[cfg(test)]
pub mod spv_reflect_tests;
#[cfg(test)]
pub mod debug_label_tests;
#[cfg(test)]
pub mod gizmo_tests;
#[cfg(test)]
pub mod shader_include_tests;
#[cfg(test)]
pub mod sampler_cache_tests;
#[cfg(test)]
pub mod gi_tests;
