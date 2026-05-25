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
    else { panic!("unsupported stage: {s:?}") }
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
layout(binding = 0) uniform ViewProj { mat4 view_proj; vec4 cam_pos; } ubo;
layout(push_constant) uniform PC { mat4 model; vec4 light_dir; vec4 light_color; vec4 material; } pc;
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec3 fragWorldPos;
void main() {
    vec4 worldPos = pc.model * vec4(inPosition, 1.0);
    gl_Position = ubo.view_proj * worldPos;
    fragWorldPos = worldPos.xyz;
    fragNormal = mat3(pc.model) * inNormal;
}
"#;

    pub const FRAGMENT_GLSL: &str = r#"#version 460
layout(push_constant) uniform PC { mat4 model; vec4 light_dir; vec4 light_color; vec4 material; } pc;
layout(binding = 0) uniform ViewProj { mat4 view_proj; vec4 cam_pos; } ubo;
layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 0) out vec4 outColor;
void main() {
    vec3 N = normalize(fragNormal);
    vec3 L = normalize(-pc.light_dir.xyz);
    vec3 V = normalize(ubo.cam_pos.xyz - fragWorldPos);
    vec3 H = normalize(L + V);

    float NdotL = max(dot(N, L), 0.0);
    float NdotH = max(dot(N, H), 0.0);

    vec3 base = pc.material.rgb;
    float roughness = pc.material.a;

    vec3 diffuse = NdotL * pc.light_color.rgb * base;
    float spec = pow(NdotH, 32.0 / (roughness * roughness + 0.001));
    vec3 specular = spec * pc.light_color.rgb * (1.0 - roughness) * 0.5;

    float ambient = pc.light_dir.w;
    vec3 ambient_color = ambient * base * 0.5;

    outColor = vec4(ambient_color + diffuse + specular, 1.0);
}
"#;

    pub fn vertex_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
        ShaderModule::from_glsl(device, VERTEX_GLSL, vk::ShaderStageFlags::VERTEX)
    }
    pub fn fragment_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
        ShaderModule::from_glsl(device, FRAGMENT_GLSL, vk::ShaderStageFlags::FRAGMENT)
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
layout(binding = 1) uniform sampler2D uTex;
layout(location = 0) in vec2 fragUv;
layout(location = 1) in vec4 fragColor;
layout(location = 0) out vec4 outColor;
void main() {
    outColor = texture(uTex, fragUv) * fragColor;
}
"#;

    pub fn vertex_2d_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
        ShaderModule::from_glsl(device, VERTEX_2D_GLSL, vk::ShaderStageFlags::VERTEX)
    }
    pub fn fragment_2d_shader(device: &ash::Device) -> Result<ShaderModule, RenderError> {
        ShaderModule::from_glsl(device, FRAGMENT_2D_GLSL, vk::ShaderStageFlags::FRAGMENT)
    }
}
