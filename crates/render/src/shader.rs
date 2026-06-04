use ash::vk;

use crate::RenderError;

pub struct ShaderModule {
    pub module: vk::ShaderModule,
    pub stage: vk::ShaderStageFlags,
    pub entry_point: std::ffi::CString,
    device: *const ash::Device,
    /// Stored SPIR-V for reflection and hot-reload.
    spv_code: Vec<u32>,
}

unsafe impl Send for ShaderModule {}
unsafe impl Sync for ShaderModule {}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        if !self.device.is_null() {
            unsafe { (*self.device).destroy_shader_module(self.module, None); }
        }
    }
}

impl ShaderModule {
    pub fn from_spirv(device: &ash::Device, code: &[u32], stage: vk::ShaderStageFlags) -> Result<Self, RenderError> {
        let module = unsafe {
            device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(code), None)
                .map_err(|e| RenderError::ShaderCompile(format!("create module: {e}")))?
        };
        Ok(Self { module, stage, entry_point: std::ffi::CString::new("main").unwrap(), device: device as *const ash::Device, spv_code: code.to_vec() })
    }

    pub fn from_glsl(device: &ash::Device, source: &str, stage: vk::ShaderStageFlags) -> Result<Self, RenderError> {
        let resolved = crate::shader_include::resolve(source, None)?;
        let spv = compile_glsl_to_spirv(&resolved, vk_stage_to_naga(stage))?;
        Self::from_spirv(device, &spv, stage)
    }

    pub fn from_wgsl(device: &ash::Device, source: &str, stage: vk::ShaderStageFlags) -> Result<Self, RenderError> {
        let spv = compile_wgsl_to_spirv(source, vk_stage_to_naga(stage))?;
        Self::from_spirv(device, &spv, stage)
    }

    /// Compile GLSL source with `#include` resolution relative to `base_path`.
    pub fn from_glsl_with_includes(
        device: &ash::Device,
        source: &str,
        stage: vk::ShaderStageFlags,
        base_path: &std::path::Path,
    ) -> Result<Self, RenderError> {
        let resolved = crate::shader_include::resolve(source, Some(base_path))?;
        let spv = compile_glsl_to_spirv(&resolved, vk_stage_to_naga(stage))?;
        Self::from_spirv(device, &spv, stage)
    }

    /// Load a shader from the pre-compiled archive (release builds).
    pub fn from_archive_name(
        device: &ash::Device,
        name: &str,
        stage: vk::ShaderStageFlags,
    ) -> Result<Self, RenderError> {
        let (spv, _stage) = crate::shader_archive::lookup(name)
            .ok_or_else(|| RenderError::ShaderCompile(format!("shader '{name}' not found in archive")))?;
        Self::from_spirv(device, spv, stage)
    }

    pub fn stage_create_info(&self) -> vk::PipelineShaderStageCreateInfo<'_> {
        vk::PipelineShaderStageCreateInfo::default().stage(self.stage).module(self.module).name(&self.entry_point)
    }

    /// Reflect this shader to discover bindings, descriptor sets, and push constants.
    pub fn reflect(&self) -> Result<crate::spv_reflect::ShaderReflection, RenderError> {
        crate::spv_reflect::reflect_spv(&self.spv_code, self.stage)
    }

    /// Load and compile a shader from a file path.
    ///
    /// The source language is inferred from the file extension:
    /// - `.glsl`, `.vert`, `.frag`, `.comp` → GLSL via naga
    /// - `.wgsl` → WGSL via naga
    ///
    /// The shader stage is inferred from the extension when possible:
    /// - `.vert` → Vertex
    /// - `.frag` → Fragment
    /// - `.comp` → Compute
    /// - `.glsl`, `.wgsl` → requires explicit `stage`
    pub fn from_file(
        device: &ash::Device,
        path: &std::path::Path,
        stage_hint: Option<vk::ShaderStageFlags>,
    ) -> Result<Self, RenderError> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| RenderError::ShaderCompile(format!("read {}: {e}", path.display())))?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let stage = match (ext.as_str(), stage_hint) {
            ("vert", _) => vk::ShaderStageFlags::VERTEX,
            ("frag", _) => vk::ShaderStageFlags::FRAGMENT,
            ("comp", _) => vk::ShaderStageFlags::COMPUTE,
            (_, Some(s)) => s,
            _ => {
                return Err(RenderError::ShaderCompile(format!(
                    "cannot infer shader stage from extension '{}' for {}; provide stage_hint",
                    ext,
                    path.display()
                )))
            }
        };

        match ext.as_str() {
            "glsl" | "vert" | "frag" | "comp" => {
                let base = path.parent().unwrap_or_else(|| std::path::Path::new("."));
                Self::from_glsl_with_includes(device, &source, stage, base)
            }
            "wgsl" => Self::from_wgsl(device, &source, stage),
            _ => {
                // Try GLSL as a fallback for unknown extensions.
                let base = path.parent().unwrap_or_else(|| std::path::Path::new("."));
                Self::from_glsl_with_includes(device, &source, stage, base)
            }
        }
    }
}

fn vk_stage_to_naga(s: vk::ShaderStageFlags) -> naga::ShaderStage {
    if s.contains(vk::ShaderStageFlags::VERTEX) { naga::ShaderStage::Vertex }
    else if s.contains(vk::ShaderStageFlags::FRAGMENT) { naga::ShaderStage::Fragment }
    else if s.contains(vk::ShaderStageFlags::COMPUTE) { naga::ShaderStage::Compute }
    else { tracing::warn!("unsupported shader stage {s:?}, defaulting to vertex"); naga::ShaderStage::Vertex }
}

fn spv_options() -> naga::back::spv::Options<'static> {
    naga::back::spv::Options {
        flags: naga::back::spv::WriterFlags::LABEL_VARYINGS | naga::back::spv::WriterFlags::CLAMP_FRAG_DEPTH,
        ..Default::default()
    }
}

fn compile_wgsl_to_spirv(source: &str, stage: naga::ShaderStage) -> Result<Vec<u32>, RenderError> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| RenderError::ShaderCompile(format!("WGSL: {e}")))?;
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module).map_err(|e| RenderError::ShaderCompile(format!("validate: {e}")))?;
    naga::back::spv::write_vec(&module, &info, &spv_options(), None)
        .map_err(|e| RenderError::ShaderCompile(format!("SPIR-V: {e}")))
}

fn compile_glsl_to_spirv(source: &str, stage: naga::ShaderStage) -> Result<Vec<u32>, RenderError> {
    let mut fe = naga::front::glsl::Frontend::default();
    let m = fe.parse(&naga::front::glsl::Options::from(stage), source)
        .map_err(|e| RenderError::ShaderCompile(format!("GLSL: {e}")))?;
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&m).map_err(|e| RenderError::ShaderCompile(format!("validate: {e}")))?;
    naga::back::spv::write_vec(&m, &info, &spv_options(), None)
        .map_err(|e| RenderError::ShaderCompile(format!("SPIR-V: {e}")))
}

pub mod builtin {
    use super::*;

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

    /// Paths searched for runtime shader overrides (editor / debug).
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
layout(location = 0) in vec2 fragUv;
layout(location = 0) out vec4 outColor;

const int TONEMAP_ALGORITHM = 0; // 0=ACES, 1=Reinhard

vec3 reinhard(vec3 v) { return v / (v + vec3(1.0)); }
vec3 aces_fitted(vec3 v) {
    float a = 2.51, b = 0.03, c = 2.43, d = 0.59, e = 0.14;
    return clamp((v * (a * v + b)) / (v * (c * v + d) + e), 0.0, 1.0);
}
void main() {
    vec3 hdr = texture(sampler2D(uHdrTex, uSamp), fragUv).rgb;
    vec3 mapped;
    if (TONEMAP_ALGORITHM == 0) {
        mapped = aces_fitted(hdr);
    } else {
        mapped = reinhard(hdr);
    }
    mapped = pow(mapped, vec3(1.0 / 2.2));
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
}

#[cfg(test)]
#[path = "shader_tests.rs"]
mod tests;
