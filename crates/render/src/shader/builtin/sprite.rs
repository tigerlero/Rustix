use ash::vk;
use crate::shader::ShaderModule;
use crate::RenderError;
use super::try_override;

// --- 2D sprite shaders ---
pub const VERTEX_2D_GLSL: &str = r#"#version 460
layout(binding = 0) uniform ViewProj { mat4 view_proj; } ubo;
layout(push_constant) uniform PC { mat4 model; } pc;
layout(location = 0) in vec2 inPosition;
layout(location = 1) in vec2 inUv;
layout(location = 2) in vec4 inColor;
layout(location = 0) out vec2 fragUv;
layout(location = 1) out vec4 fragColor;
void main() {
    gl_Position = ubo.view_proj * pc.model * vec4(inPosition, 0.0, 1.0);
    fragUv = inUv;
    fragColor = inColor;
}
"#;

pub const FRAGMENT_2D_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D uTex;
layout(binding = 2) uniform sampler uSamp;
layout(location = 0) in vec2 fragUv;
layout(location = 1) in vec4 fragColor;
layout(location = 0) out vec4 outColor;
void main() {
    outColor = texture(sampler2D(uTex, uSamp), fragUv) * fragColor;
}
"#;

pub fn vertex_2d_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "sprite.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        ShaderModule::from_glsl(device, VERTEX_2D_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn fragment_2d_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "sprite.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        ShaderModule::from_glsl(device, FRAGMENT_2D_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

/// Load the 2D sprite vertex shader, preferring a file override if present (debug only).
pub fn vertex_2d_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "sprite.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "sprite.vert", VERTEX_2D_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
/// Load the 2D sprite fragment shader, preferring a file override if present (debug only).
pub fn fragment_2d_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "sprite.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "sprite.frag", FRAGMENT_2D_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
