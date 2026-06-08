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

    /// Create a `ShaderModule` from a `ShaderAsset`.
    /// Uses pre-compiled SPIR-V if available; otherwise compiles from stored source.
    pub fn from_asset(
        device: &ash::Device,
        asset: &rustix_asset::shader::ShaderAsset,
    ) -> Result<Self, RenderError> {
        let stage = asset_stage_to_vk(asset.stage);
        if !asset.compiled_spv.is_empty() {
            Self::from_spirv(device, &asset.compiled_spv, stage)
        } else if asset.language == rustix_asset::shader::ShaderLanguage::Glsl {
            Self::from_glsl(device, &asset.source, stage)
        } else if asset.language == rustix_asset::shader::ShaderLanguage::Wgsl {
            Self::from_wgsl(device, &asset.source, stage)
        } else {
            Err(RenderError::ShaderCompile(
                "ShaderAsset has no compiled SPIR-V and no recognized source language".into(),
            ))
        }
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
            ("mesh", _) => vk::ShaderStageFlags::MESH_NV,
            ("task", _) => vk::ShaderStageFlags::TASK_NV,
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
            "mesh" | "task" => {
                let spv = compile_mesh_glsl_to_spirv(&source, stage)?;
                Self::from_spirv(device, &spv, stage)
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

fn asset_stage_to_vk(stage: rustix_asset::shader::ShaderStage) -> vk::ShaderStageFlags {
    match stage {
        rustix_asset::shader::ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
        rustix_asset::shader::ShaderStage::Fragment => vk::ShaderStageFlags::FRAGMENT,
        rustix_asset::shader::ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
        rustix_asset::shader::ShaderStage::Mesh => vk::ShaderStageFlags::MESH_NV,
        rustix_asset::shader::ShaderStage::Task => vk::ShaderStageFlags::TASK_NV,
    }
}

fn spv_options() -> naga::back::spv::Options<'static> {
    naga::back::spv::Options {
        flags: naga::back::spv::WriterFlags::LABEL_VARYINGS | naga::back::spv::WriterFlags::CLAMP_FRAG_DEPTH,
        ..Default::default()
    }
}

fn compile_wgsl_to_spirv(source: &str, _stage: naga::ShaderStage) -> Result<Vec<u32>, RenderError> {
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

/// Compile GLSL mesh/task shader to SPIR-V using shaderc (requires `mesh-shader` feature).
/// Falls back to naga if stage is a standard one.
pub fn compile_mesh_glsl_to_spirv(source: &str, stage: vk::ShaderStageFlags) -> Result<Vec<u32>, RenderError> {
    if stage == vk::ShaderStageFlags::MESH_NV || stage == vk::ShaderStageFlags::TASK_NV {
        #[cfg(feature = "shaderc")]
        {
            let shader_kind = match stage {
                vk::ShaderStageFlags::MESH_NV => shaderc::ShaderKind::Mesh,
                vk::ShaderStageFlags::TASK_NV => shaderc::ShaderKind::Task,
                _ => unreachable!(),
            };
            let compiler = shaderc::Compiler::new()
                .ok_or_else(|| RenderError::ShaderCompile("shaderc compiler not available".into()))?;
            let mut options = shaderc::CompileOptions::new()
                .ok_or_else(|| RenderError::ShaderCompile("shaderc options creation failed".into()))?;
            options.add_macro_definition("NV_mesh_shader", Some("1"));
            let binary = compiler.compile_into_spirv(source, shader_kind, "mesh.glsl", "main", Some(&options))
                .map_err(|e| RenderError::ShaderCompile(format!("shaderc: {e}")))?;
            let bytes = binary.as_binary_u8();
            let u32s: Vec<u32> = bytes.chunks_exact(4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            Ok(u32s)
        }
        #[cfg(not(feature = "shaderc"))]
        {
            Err(RenderError::ShaderCompile(
                "mesh/task shader compilation requires the `shaderc` feature or pre-compiled SPIR-V".into(),
            ))
        }
    } else {
        let naga_stage = vk_stage_to_naga(stage);
        compile_glsl_to_spirv(source, naga_stage)
    }
}

pub mod builtin;

#[cfg(test)]
#[path = "shader_tests.rs"]
mod tests;
