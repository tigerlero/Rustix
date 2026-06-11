use ash::vk;
use crate::shader::ShaderModule;
use crate::RenderError;

pub const SHADER_SEARCH_PATHS: &[&str] = &["shaders", "../shaders", "../../shaders"];

fn try_override(device: &ash::Device, name: &str, builtin: &str, stage: vk::ShaderStageFlags) -> Result<ShaderModule, RenderError> {
    for dir in SHADER_SEARCH_PATHS {
        let path = std::path::Path::new(dir).join(name);
        if path.exists() {
            tracing::info!("loading shader override: {}", path.display());
            return ShaderModule::from_file(device, &path, Some(stage));
        }
    }
    ShaderModule::from_glsl(device, builtin, stage)
}

pub mod forward;
pub mod instanced;
pub mod oit;
pub mod sprite;
pub mod postprocess;
pub mod particle;
pub mod particle_compute;
pub mod debug;

pub use forward::*;
pub use instanced::*;
pub use oit::*;
pub use sprite::*;
pub use postprocess::*;
pub use particle::*;
pub use particle_compute::*;
pub use debug::*;
