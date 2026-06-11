//! Builtin GLSL shaders for debug render modes.

use ash::vk;
use crate::shader::ShaderModule;
use crate::RenderError;

const DEBUG_VERTEX_GLSL: &str = r#"
#version 460
layout(location = 0) out vec2 fragUv;
const vec2 verts[3] = vec2[](
    vec2(-1.0, -1.0),
    vec2( 3.0, -1.0),
    vec2(-1.0,  3.0)
);
void main() {
    fragUv = verts[gl_VertexIndex] * 0.5 + 0.5;
    gl_Position = vec4(verts[gl_VertexIndex], 0.0, 1.0);
}
"#;

const DEBUG_OVERDRAW_FRAGMENT_GLSL: &str = r#"
#version 460
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(binding = 1) uniform texture2D uSceneTex;
layout(binding = 2) uniform sampler uSamp;
layout(push_constant) uniform Push { vec4 color; } uPush;
void main() {
    vec3 scene = texture(sampler2D(uSceneTex, uSamp), fragUv).rgb;
    outColor = vec4(scene + uPush.color.rgb * 0.05, 1.0);
}
"#;

const DEBUG_LIGHT_COMPLEXITY_FRAGMENT_GLSL: &str = r#"
#version 460
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(binding = 1) uniform texture2D uSceneTex;
layout(binding = 2) uniform sampler uSamp;
layout(push_constant) uniform Push { vec4 params; } uPush;
void main() {
    float lightCount = uPush.params.x;
    vec3 heatmap;
    if (lightCount < 4.0) heatmap = mix(vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 1.0), lightCount / 4.0);
    else if (lightCount < 8.0) heatmap = mix(vec3(0.0, 1.0, 1.0), vec3(0.0, 1.0, 0.0), (lightCount - 4.0) / 4.0);
    else if (lightCount < 16.0) heatmap = mix(vec3(0.0, 1.0, 0.0), vec3(1.0, 1.0, 0.0), (lightCount - 8.0) / 8.0);
    else heatmap = mix(vec3(1.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), min((lightCount - 16.0) / 16.0, 1.0));
    outColor = vec4(heatmap, 1.0);
}
"#;

fn try_override(device: &ash::Device, name: &str, builtin: &str, stage: vk::ShaderStageFlags) -> Result<ShaderModule, RenderError> {
    for dir in super::SHADER_SEARCH_PATHS {
        let path = std::path::Path::new(dir).join(name);
        if path.exists() {
            tracing::info!("loading shader override: {}", path.display());
            return ShaderModule::from_file(device, &path, Some(stage));
        }
    }
    ShaderModule::from_glsl(device, builtin, stage)
}

pub fn debug_vertex_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    try_override(device, "debug.vert", DEBUG_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
}

pub fn debug_overdraw_fragment_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    try_override(device, "debug_overdraw.frag", DEBUG_OVERDRAW_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
}

pub fn debug_light_complexity_fragment_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    try_override(device, "debug_light_complexity.frag", DEBUG_LIGHT_COMPLEXITY_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
}
