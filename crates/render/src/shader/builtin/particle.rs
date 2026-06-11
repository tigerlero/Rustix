use ash::vk;
use crate::shader::ShaderModule;
use crate::RenderError;
use super::try_override;

pub const PARTICLE_VERTEX_GLSL: &str = r#"#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; vec4 sh_r; vec4 sh_g; vec4 sh_b; } ubo;
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec4 instancePosSize;
layout(location = 2) in vec4 instanceColor;
layout(location = 0) out vec4 fragColor;
layout(location = 1) out vec2 fragUV;

const vec2 QUAD[4] = vec2[](
    vec2(-1.0, -1.0),
    vec2( 1.0, -1.0),
    vec2(-1.0,  1.0),
    vec2( 1.0,  1.0)
);

void main() {
    vec3 center = instancePosSize.xyz;
    float size = instancePosSize.w;
    vec3 toCam = normalize(ubo.cam_pos.xyz - center);
    vec3 right = normalize(cross(vec3(0.0, 1.0, 0.0), toCam));
    vec3 up = cross(toCam, right);

    vec2 quadPos = QUAD[gl_VertexIndex % 4] * size;
    vec3 worldPos = center + right * quadPos.x + up * quadPos.y;

    gl_Position = ubo.view_proj * vec4(worldPos, 1.0);
    fragColor = instanceColor;
    fragUV = QUAD[gl_VertexIndex % 4] * 0.5 + 0.5;
}
"#;

pub const PARTICLE_FRAGMENT_GLSL: &str = r#"#version 460
layout(location = 0) in vec4 fragColor;
layout(location = 1) in vec2 fragUV;
layout(location = 0) out vec4 outColor;

void main() {
    vec2 center = vec2(0.5, 0.5);
    float d = distance(fragUV, center);
    float alpha = 1.0 - smoothstep(0.3, 0.5, d);
    outColor = vec4(fragColor.rgb, fragColor.a * alpha);
}
"#;

pub fn vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "particle.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        ShaderModule::from_glsl(device, PARTICLE_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}

pub fn fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "particle.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        ShaderModule::from_glsl(device, PARTICLE_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

pub fn vertex_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "particle.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "particle.vert", PARTICLE_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}

pub fn fragment_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "particle.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "particle.frag", PARTICLE_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
