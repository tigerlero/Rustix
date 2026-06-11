use ash::vk;
use crate::shader::ShaderModule;
use crate::RenderError;
use super::try_override;

// --- Tone mapping shaders (fullscreen triangle, no vertex buffer) ---
pub const TONEMAP_VERTEX_GLSL: &str = r#"#version 460
layout(location = 0) out vec2 fragUv;
const vec2 positions[3] = vec2[](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
const vec2 uvs[3] = vec2[](vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0));
void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragUv = uvs[gl_VertexIndex];
}
"#;

pub const TONEMAP_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D uHdrTex;
layout(binding = 2) uniform sampler uSamp;
layout(binding = 3) uniform texture2D uBloomTex;
layout(binding = 4) uniform texture2D uSsaoTex;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;

layout(push_constant) uniform PC {
    float grainIntensity;
    float chromaticAberration;
    float vignetteIntensity;
    float vignetteSmoothness;
    float contrast;
    float saturation;
    float gamma;
    float _pad;
    vec4 tintShadows;
    vec4 tintMidtones;
    vec4 tintHighlights;
} pc;

const int TONEMAP_ALGORITHM = 0; // 0=ACES, 1=Reinhard

vec3 reinhard(vec3 v) { return v / (v + vec3(1.0)); }
vec3 aces_fitted(vec3 v) {
    float a = 2.51, b = 0.03, c = 2.43, d = 0.59, e = 0.14;
    return clamp((v * (a * v + b)) / (v * (c * v + d) + e), 0.0, 1.0);
}

float rand(vec2 co) {
    return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
}

vec3 applyVignette(vec3 color, vec2 uv) {
    vec2 dist = uv - 0.5;
    float vignette = 1.0 - dot(dist, dist) * pc.vignetteIntensity;
    vignette = smoothstep(0.0, pc.vignetteSmoothness, vignette);
    return color * vignette;
}

vec3 applyChromaticAberration(vec2 uv) {
    vec2 dir = uv - 0.5;
    float dist = length(dir);
    vec2 offset = dir * pc.chromaticAberration * dist;
    float r = texture(sampler2D(uHdrTex, uSamp), uv + offset).r;
    float g = texture(sampler2D(uHdrTex, uSamp), uv).g;
    float b = texture(sampler2D(uHdrTex, uSamp), uv - offset).b;
    return vec3(r, g, b);
}

vec3 applyFilmGrain(vec3 color, vec2 uv) {
    float noise = rand(uv + fract(vec2(1.0, 1.0) * 0.01)) * 2.0 - 1.0;
    return color + noise * pc.grainIntensity;
}

vec3 applyColorGrading(vec3 color) {
    color = (color - 0.5) * pc.contrast + 0.5;
    float luma = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(luma), color, pc.saturation);
    float luminance = luma;
    vec3 tint = mix(pc.tintShadows.rgb, pc.tintMidtones.rgb, smoothstep(0.0, 0.5, luminance));
    tint = mix(tint, pc.tintHighlights.rgb, smoothstep(0.5, 1.0, luminance));
    color *= tint;
    color = pow(max(color, vec3(0.0)), vec3(1.0 / pc.gamma));
    return color;
}

void main() {
    vec3 hdr = applyChromaticAberration(fragUv);
    vec3 bloom = texture(sampler2D(uBloomTex, uSamp), fragUv).rgb;
    float ssao = texture(sampler2D(uSsaoTex, uSamp), fragUv).r;
    hdr += bloom;
    hdr *= ssao;
    vec3 mapped;
    if (TONEMAP_ALGORITHM == 0) {
        mapped = aces_fitted(hdr);
    } else {
        mapped = reinhard(hdr);
    }
    mapped = pow(mapped, vec3(1.0 / 2.2));
    mapped = applyVignette(mapped, fragUv);
    mapped = applyFilmGrain(mapped, fragUv);
    mapped = applyColorGrading(mapped);
    outColor = vec4(mapped, 1.0);
}
"#;

pub fn tonemap_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "tonemap.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        ShaderModule::from_glsl(device, TONEMAP_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn tonemap_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "tonemap.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        ShaderModule::from_glsl(device, TONEMAP_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

/// Load the tone-map vertex shader, preferring a file override if present (debug only).
pub fn tonemap_vertex_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "tonemap.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "tonemap.vert", TONEMAP_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
/// Load the tone-map fragment shader, preferring a file override if present (debug only).
pub fn tonemap_fragment_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "tonemap.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "tonemap.frag", TONEMAP_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

pub const POSTPROCESS_VERTEX_GLSL: &str = r#"#version 460
layout(location = 0) out vec2 fragUv;
const vec2 positions[3] = vec2[](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
const vec2 uvs[3] = vec2[](vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0));
void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragUv = uvs[gl_VertexIndex];
}
"#;

pub const POSTPROCESS_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D uSceneTex;
layout(binding = 2) uniform sampler uSamp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;

layout(push_constant) uniform PC {
    float grainIntensity;
    float chromaticAberration;
    float vignetteIntensity;
    float vignetteSmoothness;
    float contrast;
    float saturation;
    float gamma;
    float _pad;
    vec4 tintShadows;
    vec4 tintMidtones;
    vec4 tintHighlights;
} pc;

float rand(vec2 co) {
    return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
}

vec3 applyVignette(vec3 color, vec2 uv) {
    vec2 dist = uv - 0.5;
    float vignette = 1.0 - dot(dist, dist) * pc.vignetteIntensity;
    vignette = smoothstep(0.0, pc.vignetteSmoothness, vignette);
    return color * vignette;
}

vec3 applyChromaticAberration(vec2 uv) {
    vec2 dir = uv - 0.5;
    float dist = length(dir);
    vec2 offset = dir * pc.chromaticAberration * dist;
    float r = texture(sampler2D(uSceneTex, uSamp), uv + offset).r;
    float g = texture(sampler2D(uSceneTex, uSamp), uv).g;
    float b = texture(sampler2D(uSceneTex, uSamp), uv - offset).b;
    return vec3(r, g, b);
}

vec3 applyFilmGrain(vec3 color, vec2 uv) {
    float noise = rand(uv + fract(vec2(1.0, 1.0) * 0.01)) * 2.0 - 1.0;
    return color + noise * pc.grainIntensity;
}

vec3 applyColorGrading(vec3 color) {
    // Contrast
    color = (color - 0.5) * pc.contrast + 0.5;
    // Saturation
    float luma = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(luma), color, pc.saturation);
    // Shadows / midtones / highlights tint
    float luminance = luma;
    vec3 tint = mix(pc.tintShadows.rgb, pc.tintMidtones.rgb, smoothstep(0.0, 0.5, luminance));
    tint = mix(tint, pc.tintHighlights.rgb, smoothstep(0.5, 1.0, luminance));
    color *= tint;
    // Gamma
    color = pow(max(color, vec3(0.0)), vec3(1.0 / pc.gamma));
    return color;
}

void main() {
    vec3 color = applyChromaticAberration(fragUv);
    color = applyVignette(color, fragUv);
    color = applyFilmGrain(color, fragUv);
    color = applyColorGrading(color);
    outColor = vec4(color, 1.0);
}
"#;

pub fn postprocess_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "postprocess.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        ShaderModule::from_glsl(device, POSTPROCESS_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}

pub fn postprocess_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "postprocess.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        ShaderModule::from_glsl(device, POSTPROCESS_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

pub fn postprocess_vertex_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "postprocess.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "postprocess.vert", POSTPROCESS_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}

pub fn postprocess_fragment_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "postprocess.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "postprocess.frag", POSTPROCESS_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

pub fn light_cull_compute_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "light_cull.comp", vk::ShaderStageFlags::COMPUTE)
    } else {
        ShaderModule::from_file(device, std::path::Path::new("shaders/light_cull.comp"), Some(vk::ShaderStageFlags::COMPUTE))
    }
}

pub fn light_cull_compute_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "light_cull.comp", vk::ShaderStageFlags::COMPUTE)
    } else {
        try_override(device, "light_cull.comp", "", vk::ShaderStageFlags::COMPUTE)
    }
}

pub fn gbuffer_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "gbuffer.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        ShaderModule::from_file(device, std::path::Path::new("shaders/gbuffer.vert"), Some(vk::ShaderStageFlags::VERTEX))
    }
}
pub fn gbuffer_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "gbuffer.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        ShaderModule::from_file(device, std::path::Path::new("shaders/gbuffer.frag"), Some(vk::ShaderStageFlags::FRAGMENT))
    }
}
pub fn deferred_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "deferred.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        ShaderModule::from_file(device, std::path::Path::new("shaders/deferred.vert"), Some(vk::ShaderStageFlags::VERTEX))
    }
}
pub fn deferred_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "deferred.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        ShaderModule::from_file(device, std::path::Path::new("shaders/deferred.frag"), Some(vk::ShaderStageFlags::FRAGMENT))
    }
}

pub fn gbuffer_vertex_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "gbuffer.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "gbuffer.vert", "", vk::ShaderStageFlags::VERTEX)
    }
}
pub fn gbuffer_fragment_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "gbuffer.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "gbuffer.frag", "", vk::ShaderStageFlags::FRAGMENT)
    }
}
pub fn deferred_vertex_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "deferred.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "deferred.vert", "", vk::ShaderStageFlags::VERTEX)
    }
}
pub fn deferred_fragment_shader_override(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "deferred.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "deferred.frag", "", vk::ShaderStageFlags::FRAGMENT)
    }
}

// --- Bloom shaders (fullscreen triangle) ---
pub const BLOOM_VERTEX_GLSL: &str = r#"#version 460
layout(location = 0) out vec2 fragUv;
const vec2 positions[3] = vec2[](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
const vec2 uvs[3] = vec2[](vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0));
void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragUv = uvs[gl_VertexIndex];
}
"#;

pub const BLOOM_EXTRACT_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D uSrc;
layout(binding = 2) uniform sampler uSamp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform Params {
    vec4 thresholdIntensity; // x=threshold, y=intensity, zw=unused
} pc;
void main() {
    vec3 hdr = texture(sampler2D(uSrc, uSamp), fragUv).rgb;
    float lum = dot(hdr, vec3(0.2126, 0.7152, 0.0722));
    float thresh = pc.thresholdIntensity.x;
    vec3 extracted = hdr * max(lum - thresh, 0.0) / max(lum, 0.0001);
    outColor = vec4(extracted * pc.thresholdIntensity.y, 1.0);
}
"#;

pub const BLOOM_DOWN_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D uSrc;
layout(binding = 2) uniform sampler uSamp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform Params {
    vec4 texelSize; // xy = 1.0 / source_size, zw = unused
} pc;
void main() {
    vec2 ts = pc.texelSize.xy;
    vec3 a = texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x, -ts.y)).rgb;
    vec3 b = texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x, -ts.y)).rgb;
    vec3 c = texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x,  ts.y)).rgb;
    vec3 d = texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x,  ts.y)).rgb;
    outColor = vec4((a + b + c + d) * 0.25, 1.0);
}
"#;

pub const BLOOM_UP_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D uSrc;
layout(binding = 2) uniform sampler uSamp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform Params {
    vec4 texelSize; // xy = 1.0 / source_size, zw = unused
} pc;
void main() {
    vec2 ts = pc.texelSize.xy;
    vec3 color = texture(sampler2D(uSrc, uSamp), fragUv).rgb * 4.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x, 0.0)).rgb * 2.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x, 0.0)).rgb * 2.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(0.0, -ts.y)).rgb * 2.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(0.0,  ts.y)).rgb * 2.0;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x, -ts.y)).rgb;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x, -ts.y)).rgb;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2(-ts.x,  ts.y)).rgb;
    color += texture(sampler2D(uSrc, uSamp), fragUv + vec2( ts.x,  ts.y)).rgb;
    outColor = vec4(color / 16.0, 1.0);
}
"#;

pub fn bloom_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "bloom.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "bloom.vert", BLOOM_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn bloom_extract_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "bloom_extract.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "bloom_extract.frag", BLOOM_EXTRACT_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
pub fn bloom_down_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "bloom_down.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "bloom_down.frag", BLOOM_DOWN_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
pub fn bloom_up_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "bloom_up.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "bloom_up.frag", BLOOM_UP_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

// --- SSAO shaders (fullscreen triangle) ---
pub const SSAO_VERTEX_GLSL: &str = r#"#version 460
layout(location = 0) out vec2 fragUv;
const vec2 positions[3] = vec2[](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
const vec2 uvs[3] = vec2[](vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0));
void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragUv = uvs[gl_VertexIndex];
}
"#;

pub const SSAO_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D depthTex;
layout(binding = 2) uniform sampler samp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out float outOcclusion;
layout(push_constant) uniform Params {
    vec4 projParams;
    vec4 radiusBias;
    vec4 screenSize;
} pc;
float linearize_depth(float d) {
    float near = pc.projParams.x;
    float far = pc.projParams.y;
    return near * far / (far - d * (far - near));
}
vec3 view_pos_from_uv_depth(vec2 uv, float depth) {
    float z = linearize_depth(depth);
    float tan_half_fov = 1.0 / pc.projParams.z;
    vec2 ndc = uv * 2.0 - 1.0;
    float aspect = pc.projParams.w;
    vec3 view_pos;
    view_pos.x = ndc.x * tan_half_fov * aspect * z;
    view_pos.y = ndc.y * tan_half_fov * z;
    view_pos.z = -z;
    return view_pos;
}
vec3 reconstruct_normal(vec2 uv, float depth) {
    vec2 texel = pc.screenSize.zw;
    float d1 = texture(sampler2D(depthTex, samp), uv + vec2(texel.x, 0.0)).r;
    float d2 = texture(sampler2D(depthTex, samp), uv + vec2(0.0, texel.y)).r;
    vec3 p0 = view_pos_from_uv_depth(uv, depth);
    vec3 px = view_pos_from_uv_depth(uv + vec2(texel.x, 0.0), d1);
    vec3 py = view_pos_from_uv_depth(uv + vec2(0.0, texel.y), d2);
    vec3 n = normalize(cross(px - p0, py - p0));
    return n;
}
const vec2 poisson[16] = vec2[](
    vec2(-0.94201624, -0.39906216), vec2(0.94558609, -0.76890725),
    vec2(-0.09418410, -0.92938870), vec2(0.34495938,  0.29387760),
    vec2(-0.91588581,  0.45771432), vec2(-0.81544232, -0.87912464),
    vec2(-0.38277543,  0.27676845), vec2(0.97484398,  0.75648379),
    vec2(0.44323325, -0.97511554), vec2(0.53742981, -0.47373420),
    vec2(-0.26496911, -0.41893023), vec2(0.79197514,  0.19090188),
    vec2(-0.24188840,  0.99706507), vec2(-0.81409955,  0.91437590),
    vec2(0.19984126,  0.78641367), vec2(0.14383161, -0.14100790)
);
float rand(vec2 co) {
    return fract(sin(dot(co.xy, vec2(12.9898, 78.233))) * 43758.5453);
}
void main() {
    float depth = texture(sampler2D(depthTex, samp), fragUv).r;
    if (depth >= 1.0) { outOcclusion = 1.0; return; }
    vec3 origin = view_pos_from_uv_depth(fragUv, depth);
    vec3 normal = reconstruct_normal(fragUv, depth);
    float radius = pc.radiusBias.x;
    float bias = pc.radiusBias.y;
    float power = pc.radiusBias.z;
    float intensity = pc.radiusBias.w;
    float occlusion = 0.0;
    float sampleScale = radius / -origin.z;
    float rotAngle = rand(fragUv * pc.screenSize.xy) * 6.28318530718;
    float sin_r = sin(rotAngle); float cos_r = cos(rotAngle);
    for (int i = 0; i < 16; ++i) {
        vec2 offset = poisson[i];
        vec2 rotated = vec2(offset.x * cos_r - offset.y * sin_r, offset.x * sin_r + offset.y * cos_r);
        vec2 sample_uv = fragUv + rotated * sampleScale * pc.screenSize.zw;
        float sample_depth = texture(sampler2D(depthTex, samp), sample_uv).r;
        vec3 sample_pos = view_pos_from_uv_depth(sample_uv, sample_depth);
        vec3 diff = sample_pos - origin;
        float dist = length(diff);
        vec3 sample_dir = diff / dist;
        float angle = max(dot(normal, sample_dir), 0.0);
        float rangeCheck = smoothstep(0.0, 1.0, radius / dist);
        float do_occlude = (sample_pos.z < (origin.z - bias)) ? 1.0 : 0.0;
        occlusion += do_occlude * rangeCheck * angle;
    }
    occlusion = 1.0 - (occlusion / 16.0) * intensity;
    occlusion = pow(clamp(occlusion, 0.0, 1.0), power);
    outOcclusion = occlusion;
}
"#;

pub const SSAO_BLUR_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D ssaoTex;
layout(binding = 2) uniform sampler samp;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out float outOcclusion;
layout(push_constant) uniform Params { vec4 texelSize; } pc;
void main() {
    vec2 ts = pc.texelSize.xy;
    float result = 0.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2(-ts.x, -ts.y)).r;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( ts.x, -ts.y)).r;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2(-ts.x,  ts.y)).r;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( ts.x,  ts.y)).r;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( 0.0, -ts.y)).r * 2.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( 0.0,  ts.y)).r * 2.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2(-ts.x,  0.0)).r * 2.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv + vec2( ts.x,  0.0)).r * 2.0;
    result += texture(sampler2D(ssaoTex, samp), fragUv).r * 4.0;
    outOcclusion = result / 16.0;
}
"#;

pub fn ssao_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "ssao.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "ssao.vert", SSAO_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn ssao_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "ssao.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "ssao.frag", SSAO_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
pub fn ssao_blur_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "ssao_blur.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "ssao_blur.frag", SSAO_BLUR_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

// --- TAA shaders (fullscreen triangle) ---
pub const TAA_VERTEX_GLSL: &str = r#"#version 460
layout(location = 0) out vec2 fragUv;
const vec2 positions[3] = vec2[](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
const vec2 uvs[3] = vec2[](vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0));
void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragUv = uvs[gl_VertexIndex];
}
"#;

pub const TAA_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D currentTex;
layout(binding = 2) uniform sampler samp;
layout(binding = 3) uniform texture2D historyTex;
layout(binding = 4) uniform texture2D depthTex;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform TAAParams {
    mat4 inv_view_proj;
    mat4 prev_view_proj;
    vec4 blendAndSize;
} pc;
vec3 sample_current(vec2 uv) { return texture(sampler2D(currentTex, samp), uv).rgb; }
vec3 sample_history(vec2 uv) { return texture(sampler2D(historyTex, samp), uv).rgb; }
float sample_depth(vec2 uv) { return texture(sampler2D(depthTex, samp), uv).r; }
vec3 world_pos_from_uv_depth(vec2 uv, float depth) {
    vec4 clip = vec4(uv * 2.0 - 1.0, depth, 1.0);
    vec4 world = pc.inv_view_proj * clip;
    return world.xyz / world.w;
}
vec2 prev_uv_from_world(vec3 world_pos) {
    vec4 prev_clip = pc.prev_view_proj * vec4(world_pos, 1.0);
    vec3 prev_ndc = prev_clip.xyz / prev_clip.w;
    return prev_ndc.xy * 0.5 + 0.5;
}
void main() {
    vec2 ts = vec2(1.0 / pc.blendAndSize.z, 1.0 / pc.blendAndSize.w);
    vec3 current = sample_current(fragUv);
    float depth = sample_depth(fragUv);
    vec3 world_pos = world_pos_from_uv_depth(fragUv, depth);
    vec2 prev_uv = prev_uv_from_world(world_pos);
    float off_screen = float(prev_uv.x < 0.0 || prev_uv.x > 1.0 || prev_uv.y < 0.0 || prev_uv.y > 1.0);
    vec3 history = sample_history(prev_uv);
    vec3 min_color = current;
    vec3 max_color = current;
    for (int x = -1; x <= 1; ++x) {
        for (int y = -1; y <= 1; ++y) {
            vec2 offset = vec2(float(x), float(y)) * ts;
            vec3 neighbor = sample_current(fragUv + offset);
            min_color = min(min_color, neighbor);
            max_color = max(max_color, neighbor);
        }
    }
    vec3 clamped_history = clamp(history, min_color, max_color);
    float blend = pc.blendAndSize.x;
    vec2 motion = abs(prev_uv - fragUv);
    float motion_factor = smoothstep(0.001, 0.01, length(motion));
    blend = mix(blend, 0.0, motion_factor);
    blend = mix(blend, 0.0, off_screen);
    vec3 resolved = mix(current, clamped_history, blend);
    outColor = vec4(resolved, 1.0);
}
"#;

pub fn taa_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "taa.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "taa.vert", TAA_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn taa_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "taa.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "taa.frag", TAA_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

// --- SSR shaders ---
pub const SSR_VERTEX_GLSL: &str = r#"#version 460
layout(location = 0) out vec2 fragUv;
const vec2 positions[3] = vec2[](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
const vec2 uvs[3] = vec2[](vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0));
void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragUv = uvs[gl_VertexIndex];
}
"#;

pub const SSR_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D uDepthTex;
layout(binding = 2) uniform sampler uSamp;
layout(binding = 3) uniform texture2D uColorTex;
layout(binding = 4) uniform texture2D uNormalTex;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform SSRParams {
    mat4 inv_view_proj;
    vec4 camPosAndMaxSteps;
    vec4 screenAndStride;
} pc;
float sample_depth(vec2 uv) { return texture(sampler2D(uDepthTex, uSamp), uv).r; }
vec3 world_from_uv_depth(vec2 uv, float d) {
    vec4 clip = vec4(uv * 2.0 - 1.0, d, 1.0);
    vec4 w = pc.inv_view_proj * clip;
    return w.xyz / w.w;
}
vec3 sample_normal(vec2 uv) { return texture(sampler2D(uNormalTex, uSamp), uv).rgb * 2.0 - 1.0; }
vec3 sample_color(vec2 uv) { return texture(sampler2D(uColorTex, uSamp), uv).rgb; }
void main() {
    float depth = sample_depth(fragUv);
    if (depth >= 0.99999) { outColor = vec4(0.0); return; }
    vec3 worldPos = world_from_uv_depth(fragUv, depth);
    vec3 normal = normalize(sample_normal(fragUv));
    vec3 viewDir = normalize(pc.camPosAndMaxSteps.xyz - worldPos);
    vec3 reflectDir = reflect(-viewDir, normal);
    vec3 startPos = worldPos + reflectDir * 0.05;
    vec3 endPos = startPos + reflectDir * pc.screenAndStride.w;
    vec4 startClip = vec4(startPos, 1.0) * pc.inv_view_proj; startClip.xyz /= startClip.w;
    vec4 endClip = vec4(endPos, 1.0) * pc.inv_view_proj; endClip.xyz /= endClip.w;
    vec2 startUV = startClip.xy * 0.5 + 0.5;
    vec2 endUV = endClip.xy * 0.5 + 0.5;
    vec2 delta = endUV - startUV;
    float len = length(delta * vec2(pc.screenAndStride.x, pc.screenAndStride.y));
    int steps = clamp(int(len / pc.screenAndStride.z), 4, int(pc.camPosAndMaxSteps.w));
    vec2 stepUV = delta / float(max(steps, 1));
    vec2 uv = startUV;
    float prevDepth = startClip.z;
    vec3 reflColor = vec3(0.0);
    float fade = 0.0;
    for (int i = 0; i < steps; ++i) {
        uv += stepUV;
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) break;
        float rayDepth = mix(startClip.z, endClip.z, float(i) / float(steps));
        float sceneDepth = sample_depth(uv);
        if (sceneDepth < rayDepth && (prevDepth - sceneDepth) < 0.05) {
            reflColor = sample_color(uv);
            fade = 1.0 - float(i) / float(steps);
            fade *= smoothstep(0.0, 0.1, min(uv.x, uv.y));
            fade *= smoothstep(1.0, 0.9, max(uv.x, uv.y));
            break;
        }
        prevDepth = rayDepth;
    }
    vec3 baseColor = sample_color(fragUv);
    float reflectivity = fade * 0.5;
    outColor = vec4(mix(baseColor, reflColor, reflectivity), 1.0);
}
"#;

pub fn ssr_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "ssr.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "ssr.vert", SSR_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn ssr_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "ssr.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "ssr.frag", SSR_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

// --- Volumetric fog shaders ---
pub const VOLUMETRIC_FOG_VERTEX_GLSL: &str = r#"#version 460
layout(location = 0) out vec2 fragUv;
const vec2 positions[3] = vec2[](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
const vec2 uvs[3] = vec2[](vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0));
void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragUv = uvs[gl_VertexIndex];
}
"#;

pub const VOLUMETRIC_FOG_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D uDepthTex;
layout(binding = 2) uniform sampler uSamp;
layout(binding = 3) uniform texture2D uColorTex;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform FogParams {
    mat4 inv_view_proj;
    vec4 camPosAndMaxSteps;
    vec4 fogAndScattering;
    vec4 lightDirAndIntensity;
} pc;
float hash(vec3 p) {
    p = fract(p * vec3(0.1031, 0.1030, 0.0973));
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}
float noise3d(vec3 p) {
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float n = mix(
        mix(mix(hash(i), hash(i + vec3(1,0,0)), f.x),
            mix(hash(i + vec3(0,1,0)), hash(i + vec3(1,1,0)), f.x), f.y),
        mix(mix(hash(i + vec3(0,0,1)), hash(i + vec3(1,0,1)), f.x),
            mix(hash(i + vec3(0,1,1)), hash(i + vec3(1,1,1)), f.x), f.y),
        f.z
    );
    return n;
}
float sample_depth(vec2 uv) { return texture(sampler2D(uDepthTex, uSamp), uv).r; }
vec3 world_from_uv_depth(vec2 uv, float d) {
    vec4 clip = vec4(uv * 2.0 - 1.0, d, 1.0);
    vec4 w = pc.inv_view_proj * clip;
    return w.xyz / w.w;
}
vec3 sample_color(vec2 uv) { return texture(sampler2D(uColorTex, uSamp), uv).rgb; }
void main() {
    float depth = sample_depth(fragUv);
    vec3 worldPos = world_from_uv_depth(fragUv, depth);
    vec3 camPos = pc.camPosAndMaxSteps.xyz;
    vec3 rayDir = worldPos - camPos;
    float rayLen = length(rayDir);
    rayDir = rayDir / max(rayLen, 0.001);
    float maxDist = min(rayLen, pc.fogAndScattering.w);
    int steps = clamp(int(pc.camPosAndMaxSteps.w), 8, 128);
    float stepSize = maxDist / float(steps);
    float density = pc.fogAndScattering.x;
    float scattering = pc.fogAndScattering.y;
    float heightFalloff = pc.fogAndScattering.z;
    vec3 lightDir = normalize(pc.lightDirAndIntensity.xyz);
    float sunIntensity = pc.lightDirAndIntensity.w;
    vec3 fogColor = vec3(0.6, 0.7, 0.8);
    vec3 sunColor = vec3(1.0, 0.95, 0.8) * sunIntensity;
    float transmittance = 1.0;
    vec3 inScattered = vec3(0.0);
    for (int i = 0; i < steps; ++i) {
        float t = (float(i) + 0.5) * stepSize;
        vec3 pos = camPos + rayDir * t;
        float heightFactor = exp(-max(pos.y, 0.0) * heightFalloff);
        float noise = noise3d(pos * 0.5) * 0.5 + 0.5;
        float localDensity = density * heightFactor * (0.7 + 0.3 * noise);
        float stepTransmittance = exp(-localDensity * stepSize * scattering);
        float cosTheta = max(dot(rayDir, lightDir), 0.0);
        float phase = 0.25 + 0.75 * cosTheta * cosTheta;
        vec3 stepLight = sunColor * phase * localDensity * stepSize;
        inScattered += transmittance * stepLight;
        transmittance *= stepTransmittance;
    }
    vec3 sceneColor = sample_color(fragUv);
    vec3 fogged = sceneColor * transmittance + inScattered * fogColor;
    outColor = vec4(fogged, 1.0);
}
"#;

pub fn volumetric_fog_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "volumetric_fog.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "volumetric_fog.vert", VOLUMETRIC_FOG_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn volumetric_fog_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "volumetric_fog.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "volumetric_fog.frag", VOLUMETRIC_FOG_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

// --- Skybox shaders ---
pub const SKYBOX_VERTEX_GLSL: &str = r#"#version 460
layout(location = 0) out vec2 fragUv;
const vec2 positions[3] = vec2[](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
const vec2 uvs[3] = vec2[](vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0));
void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragUv = uvs[gl_VertexIndex];
}
"#;

pub const SKYBOX_FRAGMENT_GLSL: &str = r#"#version 460
layout(binding = 1) uniform texture2D uDepthTex;
layout(binding = 2) uniform sampler uSamp;
layout(binding = 3) uniform texture2D uColorTex;
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;
layout(push_constant) uniform SkyboxParams {
    mat4 inv_view_proj;
    vec4 sunDirAndIntensity;
    vec4 skyParams;
} pc;
float sample_depth(vec2 uv) { return texture(sampler2D(uDepthTex, uSamp), uv).r; }
vec3 world_dir_from_uv(vec2 uv) {
    vec4 clip = vec4(uv * 2.0 - 1.0, 1.0, 1.0);
    vec4 w = pc.inv_view_proj * clip;
    vec3 p = w.xyz / w.w;
    vec4 camW = pc.inv_view_proj * vec4(0.0, 0.0, -1.0, 1.0);
    vec3 camPos = camW.xyz / camW.w;
    return normalize(p - camPos);
}
vec3 sample_color(vec2 uv) { return texture(sampler2D(uColorTex, uSamp), uv).rgb; }
void main() {
    float depth = sample_depth(fragUv);
    if (depth < 0.99999) { outColor = vec4(sample_color(fragUv), 1.0); return; }
    vec3 viewDir = world_dir_from_uv(fragUv);
    vec3 sunDir = normalize(pc.sunDirAndIntensity.xyz);
    float sunIntensity = pc.sunDirAndIntensity.w;
    float rayleigh = pc.skyParams.x;
    float mie = pc.skyParams.y;
    float zenithShift = pc.skyParams.z;
    float exposure = pc.skyParams.w;
    float cosTheta = viewDir.y + zenithShift;
    float rayleighPhase = 0.0596831 * (1.0 + cosTheta * cosTheta);
    float zenithAngle = max(0.0, cosTheta);
    float zenithDensity = exp(-zenithAngle * 3.0);
    vec3 skyColor = vec3(0.2, 0.5, 1.0) * rayleighPhase * rayleigh * zenithDensity;
    vec3 sunColor = vec3(1.0, 0.95, 0.8) * sunIntensity;
    float cosSunAngle = dot(viewDir, sunDir);
    float sunDisc = smoothstep(0.999, 0.9999, cosSunAngle);
    float sunGlow = pow(max(cosSunAngle, 0.0), 256.0) * mie;
    skyColor += sunColor * (sunDisc * 2.0 + sunGlow * 0.5);
    float horizonGlow = exp(-abs(viewDir.y) * 8.0) * 0.3;
    skyColor += vec3(0.8, 0.5, 0.3) * horizonGlow;
    outColor = vec4(skyColor * exposure, 1.0);
}
"#;

pub fn skybox_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "skybox.vert", vk::ShaderStageFlags::VERTEX)
    } else {
        try_override(device, "skybox.vert", SKYBOX_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
}
pub fn skybox_fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
    if cfg!(not(debug_assertions)) {
        ShaderModule::from_archive_name(device, "skybox.frag", vk::ShaderStageFlags::FRAGMENT)
    } else {
        try_override(device, "skybox.frag", SKYBOX_FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
