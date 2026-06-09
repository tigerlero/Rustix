use ash::vk;
use crate::shader::ShaderModule;
use crate::RenderError;
use super::try_override;

pub const VERTEX_GLSL: &str = r#"#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; } ubo;
layout(push_constant) uniform PC { mat4 model; vec4 dir_light; vec4 dir_color; vec4 base_color; vec4 material; } pc;
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec3 fragWorldPos;
layout(location = 2) out vec4 fragPosLightSpace;
void main() {
    vec4 worldPos = pc.model * vec4(inPosition, 1.0);
    gl_Position = ubo.view_proj * worldPos;
    fragWorldPos = worldPos.xyz;
    fragNormal = mat3(pc.model) * inNormal;
    fragPosLightSpace = ubo.light_view_proj * worldPos;
}
"#;

pub const FRAGMENT_GLSL: &str = r#"#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; } ubo;
layout(push_constant) uniform PC { mat4 model; vec4 dir_light; vec4 dir_color; vec4 base_color; vec4 material; } pc;
layout(binding = 1) uniform texture2D shadowMapTex;
layout(binding = 2) uniform sampler shadowMapSamp;
layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 2) in vec4 fragPosLightSpace;
layout(location = 0) out vec4 outColor;

const int SHADOW_PCF_RADIUS = 1;

vec3 blinn_phong(vec3 N, vec3 L, vec3 V, vec3 light_color, vec3 base, float roughness, float metallic) {
    vec3 H = normalize(L + V);
    float NdotL = max(dot(N, L), 0.0);
    float NdotH = max(dot(N, H), 0.0);
    float spec_pow = 32.0 / (roughness * roughness + 0.001);
    float spec = pow(NdotH, spec_pow);

    vec3 f0 = mix(vec3(0.04), base, metallic);
    vec3 specular = spec * light_color * f0 * (1.0 - roughness) * 0.5;
    vec3 diffuse = NdotL * light_color * base * (1.0 - metallic);

    return diffuse + specular;
}

float shadowFactor(vec4 fragLightSpace) {
    vec3 projCoords = fragLightSpace.xyz / fragLightSpace.w;
    projCoords = projCoords * 0.5 + 0.5;
    if (projCoords.z > 1.0 || projCoords.x < 0.0 || projCoords.x > 1.0 || projCoords.y < 0.0 || projCoords.y > 1.0) return 1.0;
    float currentDepth = projCoords.z;
    float bias = 0.005;
    vec2 texelSize = vec2(1.0 / 1024.0);
    float shadow = 0.0;
    for(int x = -SHADOW_PCF_RADIUS; x <= SHADOW_PCF_RADIUS; ++x) {
        for(int y = -SHADOW_PCF_RADIUS; y <= SHADOW_PCF_RADIUS; ++y) {
            float pcfDepth = texture(sampler2D(shadowMapTex, shadowMapSamp), projCoords.xy + vec2(x, y) * texelSize).r;
            shadow += currentDepth - bias > pcfDepth ? 0.0 : 1.0;
        }
    }
    int samples = (2 * SHADOW_PCF_RADIUS + 1) * (2 * SHADOW_PCF_RADIUS + 1);
    return shadow / float(samples);
}

void main() {
    vec3 N = normalize(fragNormal);
    vec3 L = normalize(pc.dir_light.xyz);
    vec3 V = normalize(ubo.cam_pos.xyz - fragWorldPos);
    vec3 lightCol = pc.dir_color.rgb;
    vec3 base = pc.base_color.rgb;
    float rough = pc.material.x;
    float metal = pc.material.y;

    vec3 lit = blinn_phong(N, L, V, lightCol, base, rough, metal);
    vec3 ambient = base * 0.1;
    float shadow = shadowFactor(fragPosLightSpace);
    outColor = vec4(ambient + shadow * lit, 1.0);
}
"#;

pub const SHADOW_VERTEX_GLSL: &str = r#"#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; } ubo;
layout(push_constant) uniform PC { mat4 model; vec4 dir_light; vec4 dir_color; vec4 base_color; vec4 material; } pc;
layout(location = 0) in vec3 inPosition;
void main() {
    gl_Position = ubo.light_view_proj * pc.model * vec4(inPosition, 1.0);
}
"#;

pub fn vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "pbr.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        ShaderModule::from_glsl(device, VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "pbr.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        ShaderModule::from_glsl(device, FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
pub fn shadow_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "shadow.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        ShaderModule::from_glsl(device, SHADOW_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}

/// Load the PBR vertex shader, preferring a file override if present (debug only).
pub fn vertex_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "pbr.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "pbr.vert", VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
/// Load the PBR fragment shader, preferring a file override if present (debug only).
pub fn fragment_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "pbr.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "pbr.frag", FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
/// Load the shadow vertex shader, preferring a file override if present (debug only).
pub fn shadow_vertex_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "shadow.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "shadow.vert", SHADOW_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
