use ash::vk;

/// Collection of Vulkan specialization constants keyed by `constant_id`.
///
/// Each entry is a 32-bit unsigned integer. Booleans, floats, and signed
/// integers can all be represented by reinterpreting their bit pattern as a
/// `u32` (e.g. `f32::to_bits()`).
///
/// # Example
/// ```ignore
/// let mut spec = SpecConstantMap::new();
/// spec.set(0, 4u32);          // SHADOW_PCF_RADIUS
/// spec.set(1, 1u32);          // ENABLE_SHADOWS (bool as u32)
/// spec.set(2, f32::to_bits(2.2)); // gamma value
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SpecConstantMap {
    entries: Vec<(u32, u32)>, // (constant_id, value)
}

impl SpecConstantMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or overwrite a specialization constant.
    pub fn set(&mut self, id: u32, value: u32) -> &mut Self {
        if let Some(entry) = self.entries.iter_mut().find(|(cid, _)| *cid == id) {
            entry.1 = value;
        } else {
            self.entries.push((id, value));
        }
        self
    }

    /// Retrieve a specialization constant value.
    pub fn get(&self, id: u32) -> Option<u32> {
        self.entries.iter().find(|(cid, _)| *cid == id).map(|(_, v)| *v)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Build Vulkan specialization map entries and raw data.
    ///
    /// The returned vectors must outlive the `vk::SpecializationInfo` and
    /// the pipeline creation call that consumes it.
    pub fn build(&self) -> (Vec<vk::SpecializationMapEntry>, Vec<u8>) {
        let mut map_entries = Vec::with_capacity(self.entries.len());
        let mut data = Vec::with_capacity(self.entries.len() * 4);
        for (id, value) in &self.entries {
            let offset = data.len() as u32;
            map_entries.push(
                vk::SpecializationMapEntry::default()
                    .constant_id(*id)
                    .offset(offset)
                    .size(4),
            );
            data.extend_from_slice(&value.to_ne_bytes());
        }
        (map_entries, data)
    }
}

/// Owned Vulkan specialization data produced from a `SpecConstantMap`.
///
/// Keep this alive for as long as any `vk::PipelineShaderStageCreateInfo`
/// that references it is in use.
pub struct SpecConstantData {
    pub map_entries: Vec<vk::SpecializationMapEntry>,
    pub data: Vec<u8>,
}

impl SpecConstantData {
    pub fn from_map(map: &SpecConstantMap) -> Self {
        let (map_entries, data) = map.build();
        Self { map_entries, data }
    }

    pub fn info(&self) -> vk::SpecializationInfo<'_> {
        vk::SpecializationInfo::default()
            .map_entries(&self.map_entries)
            .data(&self.data)
    }
}
