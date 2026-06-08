use ash::vk;
use crate::RenderError;

/// Reflected resource from a SPIR-V shader module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReflectedResource {
    pub set: u32,
    pub binding: u32,
    pub descriptor_type: vk::DescriptorType,
    pub count: u32,
    pub stage: vk::ShaderStageFlags,
    pub name: Option<String>,
}

/// Reflection result for a single shader stage.
#[derive(Clone, Debug, Default)]
pub struct ShaderReflection {
    pub resources: Vec<ReflectedResource>,
    pub push_constant_size: Option<u32>,
    pub push_constant_offset: u32,
}

impl ShaderReflection {
    /// Merge another stage's reflection into this one, adjusting stage flags.
    pub fn merge(&mut self, other: &ShaderReflection, stage: vk::ShaderStageFlags) {
        for res in &other.resources {
            if let Some(existing) = self.resources.iter_mut().find(|r| {
                r.set == res.set && r.binding == res.binding
            }) {
                existing.stage |= stage;
            } else {
                let mut merged = res.clone();
                merged.stage = stage;
                self.resources.push(merged);
            }
        }
        if other.push_constant_size.is_some() {
            // Push constants must match across stages; keep the first seen.
            if self.push_constant_size.is_none() {
                self.push_constant_size = other.push_constant_size;
                self.push_constant_offset = other.push_constant_offset;
            }
        }
    }

    /// Build descriptor set layout bindings grouped by set index.
    pub fn bindings_by_set(&self) -> Vec<(u32, Vec<vk::DescriptorSetLayoutBinding<'_>>)> {
        use std::collections::BTreeMap;
        let mut map: BTreeMap<u32, Vec<vk::DescriptorSetLayoutBinding>> = BTreeMap::new();
        for res in &self.resources {
            map.entry(res.set).or_default().push(
                vk::DescriptorSetLayoutBinding::default()
                    .binding(res.binding)
                    .descriptor_type(res.descriptor_type)
                    .descriptor_count(res.count)
                    .stage_flags(res.stage),
            );
        }
        map.into_iter().collect()
    }

    /// Build a single push constant range if any push constants were found.
    pub fn push_constant_range(&self, stage: vk::ShaderStageFlags) -> Option<vk::PushConstantRange> {
        self.push_constant_size.map(|size| {
            vk::PushConstantRange::default()
                .stage_flags(stage)
                .offset(self.push_constant_offset)
                .size(size)
        })
    }
}

/// Reflect a SPIR-V binary to extract bindings and push constants.
pub fn reflect_spv(spv: &[u32], stage: vk::ShaderStageFlags) -> Result<ShaderReflection, RenderError> {
    let spv_bytes: &[u8] = bytemuck::cast_slice(spv);
    let module = naga::front::spv::parse_u8_slice(
        spv_bytes,
        &naga::front::spv::Options::default(),
    )
    .map_err(|e| RenderError::ShaderCompile(format!("spv reflection parse: {e:?}")))?;

    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .map_err(|e| RenderError::ShaderCompile(format!("spv reflection validate: {e:?}")))?;

    let mut reflection = ShaderReflection::default();

    for (_handle, var) in module.global_variables.iter() {
        let ty = &module.types[var.ty];

        if let Some(naga::ResourceBinding { group, binding }) = var.binding {
            let descriptor_type = map_naga_type_to_descriptor(&ty.inner, &module, &info, var.space)?;
            let count = array_size(&ty.inner, &module);

            reflection.resources.push(ReflectedResource {
                set: group,
                binding,
                descriptor_type,
                count,
                stage,
                name: var.name.clone(),
            });
        }

        // Detect push constants by storage class.
        if var.space == naga::AddressSpace::PushConstant {
            let size = type_size(&ty.inner, &module);
            if size > 0 {
                reflection.push_constant_size = Some(size);
                reflection.push_constant_offset = 0;
            }
        }
    }

    Ok(reflection)
}

fn map_naga_type_to_descriptor(
    inner: &naga::TypeInner,
    module: &naga::Module,
    _info: &naga::valid::ModuleInfo,
    space: naga::AddressSpace,
) -> Result<vk::DescriptorType, RenderError> {
    use naga::TypeInner as T;
    use vk::DescriptorType as D;

    match space {
        naga::AddressSpace::Uniform => Ok(D::UNIFORM_BUFFER),
        naga::AddressSpace::Storage { .. } => Ok(D::STORAGE_BUFFER),
        naga::AddressSpace::PushConstant => Ok(D::UNIFORM_BUFFER), // not a real descriptor, but placeholder
        naga::AddressSpace::Handle => {
            match inner {
                T::Image { class, .. } => {
                    match class {
                        naga::ImageClass::Storage { .. } => Ok(D::STORAGE_IMAGE),
                        _ => Ok(D::SAMPLED_IMAGE),
                    }
                }
                T::Sampler { .. } => Ok(D::SAMPLER),
                T::BindingArray { base, .. } => {
                    let base_ty = &module.types[*base];
                    map_naga_type_to_descriptor(&base_ty.inner, module, _info, space)
                }
                _ => Ok(D::UNIFORM_BUFFER), // fallback
            }
        }
        _ => Ok(D::UNIFORM_BUFFER),
    }
}

fn array_size(inner: &naga::TypeInner, module: &naga::Module) -> u32 {
    use naga::TypeInner as T;
    match inner {
        T::Array { size, .. } => match size {
            naga::ArraySize::Constant(c) => c.get() as u32,
            naga::ArraySize::Dynamic => 1,
        },
        T::BindingArray { size, .. } => match size {
            naga::ArraySize::Constant(c) => c.get() as u32,
            naga::ArraySize::Dynamic => 1,
        },
        T::Struct { members, .. } => {
            // Check if the struct contains a runtime array (SSBO)
            for m in members.iter() {
                let mty = &module.types[m.ty];
                if matches!(mty.inner, T::Array { size: naga::ArraySize::Dynamic, .. }) {
                    return 1; // dynamic array = 1 descriptor entry
                }
            }
            1
        }
        _ => 1,
    }
}

fn type_size(inner: &naga::TypeInner, module: &naga::Module) -> u32 {
    use naga::TypeInner as T;
    match inner {
        T::Scalar(scalar) => scalar.width as u32 / 8,
        T::Vector { size, scalar } => (*size as u32) * (scalar.width as u32 / 8),
        T::Matrix { columns, rows, scalar } => (*columns as u32) * (*rows as u32) * (scalar.width as u32 / 8),
        T::Array { base, size, stride: _ } => {
            let elem_size = type_size(&module.types[*base].inner, module);
            let count = match size {
                naga::ArraySize::Constant(c) => c.get() as u32,
                naga::ArraySize::Dynamic => 0,
            };
            elem_size * count
        }
        T::Struct { members, .. } => {
            members.iter().map(|m| type_size(&module.types[m.ty].inner, module)).sum()
        }
        T::Pointer { .. } => 8,
        _ => 0,
    }
}
