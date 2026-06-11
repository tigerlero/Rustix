use ash::vk;
use crate::shader::ShaderModule;
use crate::RenderError;
use super::try_override;

pub const VERTEX_GLSL: &str = r#"#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; vec4 sh_r; vec4 sh_g; vec4 sh_b; } ubo;
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
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; vec4 sh_r; vec4 sh_g; vec4 sh_b; } ubo;
layout(push_constant) uniform PC { mat4 model; vec4 dir_light; vec4 dir_color; vec4 base_color; vec4 material; } pc;
layout(binding = 1) uniform texture2D shadowMapTex;
layout(binding = 2) uniform sampler shadowMapSamp;
layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 2) in vec4 fragPosLightSpace;
layout(location = 0) out vec4 outColor;

const int SHADOW_PCF_RADIUS = 1;
const float PI = 3.14159265359;

float D_GGX(float NdotH, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float denom = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

float G_SmithGGX(float NdotV, float roughness) {
    float a = roughness * roughness;
    return (2.0 * NdotV) / (NdotV + sqrt(a + (1.0 - a) * NdotV * NdotV));
}

float G_Smith(float NdotV, float NdotL, float roughness) {
    return G_SmithGGX(NdotV, roughness) * G_SmithGGX(NdotL, roughness);
}

vec3 F_Schlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
}

vec3 pbrDirect(vec3 N, vec3 L, vec3 V, vec3 light_color, vec3 base, float roughness, float metallic) {
    float NdotL = max(dot(N, L), 0.0);
    if (NdotL <= 0.0) return vec3(0.0);
    float NdotV = max(dot(N, V), 0.0);
    if (NdotV <= 0.0) return vec3(0.0);
    vec3 H = normalize(L + V);
    float NdotH = max(dot(N, H), 0.0);
    float HdotV = max(dot(H, V), 0.0);
    vec3 F0 = mix(vec3(0.04), base, metallic);
    float D = D_GGX(NdotH, roughness);
    float G = G_Smith(NdotV, NdotL, roughness);
    vec3 F = F_Schlick(HdotV, F0);
    vec3 spec = (D * G * F) / (4.0 * NdotV * NdotL + 0.0001);
    vec3 kD = (1.0 - F) * (1.0 - metallic);
    vec3 diff = base * kD / PI;
    return (diff + spec) * light_color * NdotL;
}

float pcfFilter(vec2 uv, float currentDepth, float bias, vec2 texelSize, int radius) {
    float shadow = 0.0;
    int count = 0;
    for(int x = -radius; x <= radius; ++x) {
        for(int y = -radius; y <= radius; ++y) {
            vec2 offset = vec2(x, y) * texelSize;
            float pcfDepth = texture(sampler2D(shadowMapTex, shadowMapSamp), uv + offset).r;
            shadow += currentDepth - bias > pcfDepth ? 0.0 : 1.0;
            count++;
        }
    }
    return shadow / float(count);
}

float findBlockerDistance(vec2 uv, float currentDepth, float bias, vec2 texelSize, int searchRadius) {
    float blockerSum = 0.0;
    int blockerCount = 0;
    for(int x = -searchRadius; x <= searchRadius; ++x) {
        for(int y = -searchRadius; y <= searchRadius; ++y) {
            vec2 offset = vec2(x, y) * texelSize;
            float sampleDepth = texture(sampler2D(shadowMapTex, shadowMapSamp), uv + offset).r;
            if (currentDepth - bias > sampleDepth) {
                blockerSum += sampleDepth;
                blockerCount++;
            }
        }
    }
    if (blockerCount == 0) return -1.0;
    return blockerSum / float(blockerCount);
}

float shadowFactor(vec4 fragLightSpace) {
    vec3 projCoords = fragLightSpace.xyz / fragLightSpace.w;
    projCoords = projCoords * 0.5 + 0.5;
    if (projCoords.z > 1.0 || projCoords.x < 0.0 || projCoords.x > 1.0 || projCoords.y < 0.0 || projCoords.y > 1.0) return 1.0;
    float currentDepth = projCoords.z;
    float bias = 0.005;
    vec2 texelSize = vec2(1.0 / 1024.0);

    // PCSS: estimate penumbra size from blocker distance
    float blockerDist = findBlockerDistance(projCoords.xy, currentDepth, bias, texelSize, 2);
    if (blockerDist < 0.0) return 1.0; // fully lit, no blockers nearby

    float penumbra = (currentDepth - blockerDist) / blockerDist;
    int pcfRadius = int(clamp(penumbra * 4.0, 1.0, 4.0));

    return pcfFilter(projCoords.xy, currentDepth, bias, texelSize, pcfRadius);
}

vec3 evalShL1(vec3 n, vec4 shR, vec4 shG, vec4 shB) {
    return vec3(
        shR.x + shR.y * n.y + shR.z * n.z + shR.w * n.x,
        shG.x + shG.y * n.y + shG.z * n.z + shG.w * n.x,
        shB.x + shB.y * n.y + shB.z * n.z + shB.w * n.x
    );
}

void main() {
    vec3 N = normalize(fragNormal);
    vec3 L = normalize(pc.dir_light.xyz);
    vec3 V = normalize(ubo.cam_pos.xyz - fragWorldPos);
    vec3 lightCol = pc.dir_color.rgb;
    vec3 base = pc.base_color.rgb;
    float rough = pc.material.x;
    float metal = pc.material.y;

    vec3 lit = pbrDirect(N, L, V, lightCol, base, rough, metal);
    vec3 ambient = evalShL1(N, ubo.sh_r, ubo.sh_g, ubo.sh_b) * base * (1.0 / PI);
    float shadow = shadowFactor(fragPosLightSpace);
    outColor = vec4(ambient + shadow * lit, 1.0);
}
"#;

pub const SHADOW_VERTEX_GLSL: &str = r#"#version 460
struct PointLight { vec4 position; vec4 color; };
layout(binding = 0) uniform SceneUBO { mat4 view_proj; vec4 cam_pos; uint light_count; PointLight lights[8]; vec4 fog; mat4 light_view_proj; vec4 sh_r; vec4 sh_g; vec4 sh_b; } ubo;
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
