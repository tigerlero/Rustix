use ash::vk;

use crate::RenderError;

pub struct ShaderModule {
    pub module: vk::ShaderModule,
    pub stage: vk::ShaderStageFlags,
    pub entry_point: std::ffi::CString,
    device: *const ash::Device,
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
        Ok(Self { module, stage, entry_point: std::ffi::CString::new("main").unwrap(), device })
    }

    pub fn from_glsl(device: &ash::Device, source: &str, stage: vk::ShaderStageFlags) -> Result<Self, RenderError> {
        let spv = compile_glsl_to_spirv(source, vk_stage_to_naga(stage))?;
        Self::from_spirv(device, &spv, stage)
    }

    pub fn from_wgsl(device: &ash::Device, source: &str, stage: vk::ShaderStageFlags) -> Result<Self, RenderError> {
        let spv = compile_wgsl_to_spirv(source, vk_stage_to_naga(stage))?;
        Self::from_spirv(device, &spv, stage)
    }

    pub fn stage_create_info(&self) -> vk::PipelineShaderStageCreateInfo<'_> {
        vk::PipelineShaderStageCreateInfo::default().stage(self.stage).module(self.module).name(&self.entry_point)
    }
}

fn vk_stage_to_naga(s: vk::ShaderStageFlags) -> naga::ShaderStage {
    if s.contains(vk::ShaderStageFlags::VERTEX) { naga::ShaderStage::Vertex }
    else if s.contains(vk::ShaderStageFlags::FRAGMENT) { naga::ShaderStage::Fragment }
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
    float closestDepth = texture(sampler2D(shadowMapTex, shadowMapSamp), projCoords.xy).r;
    float currentDepth = projCoords.z;
    float bias = 0.005;
    return currentDepth - bias > closestDepth ? 0.0 : 1.0;
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
        ShaderModule::from_glsl(device, VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
    pub fn fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
        ShaderModule::from_glsl(device, FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
    pub fn shadow_vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
        ShaderModule::from_glsl(device, SHADOW_VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
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
        ShaderModule::from_glsl(device, VERTEX_2D_GLSL, vk::ShaderStageFlags::VERTEX)
    }
    pub fn fragment_2d_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
        ShaderModule::from_glsl(device, FRAGMENT_2D_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}

#[cfg(test)]
mod tests {
    use glam::{Vec3, Vec4};

    /// Replicates the fragment shader's `blinn_phong` + `ambient + shadow * lit` logic.
    fn blinn_phong(
        n: Vec3, l: Vec3, v: Vec3, light_color: Vec3, base: Vec3, roughness: f32, metallic: f32,
    ) -> Vec3 {
        let h = (l + v).normalize();
        let ndotl = n.dot(l).max(0.0);
        let ndoth = n.dot(h).max(0.0);
        let spec_pow = 32.0 / (roughness * roughness + 0.001);
        let spec = ndoth.powf(spec_pow);

        let f0 = Vec3::splat(0.04).lerp(base, metallic);
        let specular = spec * light_color * f0 * (1.0 - roughness) * 0.5;
        let diffuse = ndotl * light_color * base * (1.0 - metallic);

        diffuse + specular
    }

    fn shade(base: Vec3, shadow: f32, n: Vec3, l: Vec3, v: Vec3, light_color: Vec3, roughness: f32, metallic: f32) -> Vec4 {
        let lit = blinn_phong(n, l, v, light_color, base, roughness, metallic);
        let ambient = base * 0.1;
        let color = ambient + shadow * lit;
        Vec4::new(color.x, color.y, color.z, 1.0)
    }

    #[test]
    fn fragment_ambient_lit_blending() {
        let n = Vec3::new(0.0, 0.0, 1.0);
        let l = Vec3::new(0.0, 0.0, 1.0);
        let v = Vec3::new(0.0, 0.0, 1.0);
        let light_color = Vec3::new(1.0, 1.0, 1.0);
        let base = Vec3::new(0.5, 0.5, 0.5);
        let roughness = 0.5;
        let metallic = 0.0;

        // When shadow = 0.0, only ambient remains.
        let unlit = shade(base, 0.0, n, l, v, light_color, roughness, metallic);
        let ambient = base * 0.1;
        assert!((unlit.x - ambient.x).abs() < 0.001, "unlit R should be ambient");
        assert!((unlit.y - ambient.y).abs() < 0.001, "unlit G should be ambient");
        assert!((unlit.z - ambient.z).abs() < 0.001, "unlit B should be ambient");

        // When shadow = 1.0, result is ambient + lit.
        let lit = shade(base, 1.0, n, l, v, light_color, roughness, metallic);
        let expected_lit = blinn_phong(n, l, v, light_color, base, roughness, metallic);
        let expected = ambient + expected_lit;
        assert!((lit.x - expected.x).abs() < 0.001, "lit R should be ambient + lit");
        assert!((lit.y - expected.y).abs() < 0.001, "lit G should be ambient + lit");
        assert!((lit.z - expected.z).abs() < 0.001, "lit B should be ambient + lit");

        // Sanity: lit color should be noticeably brighter than ambient-only.
        assert!(lit.x > unlit.x * 2.0, "lit color should be much brighter than ambient-only");
    }
}

