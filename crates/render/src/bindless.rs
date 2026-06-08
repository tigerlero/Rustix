use ash::vk;
use parking_lot::Mutex;
use crate::RenderError;

/// Maximum number of textures in the bindless heap.
pub const MAX_BINDLESS_TEXTURES: u32 = 4096;
/// Maximum number of samplers in the bindless heap.
pub const MAX_BINDLESS_SAMPLERS: u32 = 128;
/// Maximum number of storage buffers in the bindless heap.
pub const MAX_BINDLESS_STORAGE_BUFFERS: u32 = 16;

/// Global bindless descriptor heap.
///
/// Holds a single descriptor set with:
/// - Binding 0: Scene UBO (1 descriptor, traditional)
/// - Binding 1: Sampled image array (MAX_BINDLESS_TEXTURES entries, partially bound, update-after-bind)
/// - Binding 2: Sampler array (MAX_BINDLESS_SAMPLERS entries, partially bound, update-after-bind)
/// - Binding 3: Storage buffer array (MAX_BINDLESS_STORAGE_BUFFERS entries, partially bound, update-after-bind)
/// - Binding 4: Storage buffer array (MAX_BINDLESS_STORAGE_BUFFERS entries, partially bound, update-after-bind)
///
/// Texture, sampler, and storage buffer slots can be allocated and updated at any time.
/// The descriptor set uses `PARTIALLY_BOUND` so unwritten slots are valid.
pub struct BindlessDescriptorHeap {
    pub(crate) device: *const ash::Device,
    pub(crate) pool: vk::DescriptorPool,
    pub(crate) layout: vk::DescriptorSetLayout,
    pub(crate) set: vk::DescriptorSet,
    pub(crate) texture_slots: Mutex<Vec<Option<vk::ImageView>>>,
    pub(crate) free_texture_slots: Mutex<Vec<u32>>,
    pub(crate) sampler_slots: Mutex<Vec<Option<vk::Sampler>>>,
    pub(crate) free_sampler_slots: Mutex<Vec<u32>>,
    pub(crate) storage_slots: [Mutex<Vec<Option<vk::Buffer>>>; 2],
    pub(crate) free_storage_slots: [Mutex<Vec<u32>>; 2],
}

unsafe impl Send for BindlessDescriptorHeap {}
unsafe impl Sync for BindlessDescriptorHeap {}

impl BindlessDescriptorHeap {
    pub fn new(device: &ash::Device) -> Result<Self, RenderError> {
        let max_textures = MAX_BINDLESS_TEXTURES;
        let max_samplers = MAX_BINDLESS_SAMPLERS;
        let max_storage = MAX_BINDLESS_STORAGE_BUFFERS;

        // --- Descriptor pool with UPDATE_AFTER_BIND ---
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 3, // +1 CSM UBO + 1 spot shadow UBO
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLED_IMAGE,
                descriptor_count: max_textures + 10, // +4 gbuffer + depth + 3 CSM + 1 cubemap + 1 spot array
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLER,
                descriptor_count: max_samplers + 4, // +1 gbuffer + 1 CSM + 1 point + 1 spot
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: max_storage * 2,
            },
        ];
        let pool = unsafe {
            device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .pool_sizes(&pool_sizes)
                        .max_sets(1)
                        .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND),
                    None,
                )
                .map_err(|e| {
                    RenderError::DeviceCreation(format!("bindless pool: {e}"))
                })?
        };

        // --- Descriptor set layout ---
        let bindings = [
            // Binding 0: Scene UBO
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT | vk::ShaderStageFlags::COMPUTE),
            // Binding 1: Sampled image array (bindless)
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(max_textures)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 2: Sampler array (bindless)
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(max_samplers)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 3: Storage buffer array (bindless)
            vk::DescriptorSetLayoutBinding::default()
                .binding(3)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(max_storage)
                .stage_flags(vk::ShaderStageFlags::COMPUTE | vk::ShaderStageFlags::FRAGMENT),
            // Binding 4: Storage buffer array (bindless)
            vk::DescriptorSetLayoutBinding::default()
                .binding(4)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(max_storage)
                .stage_flags(vk::ShaderStageFlags::COMPUTE | vk::ShaderStageFlags::FRAGMENT),
            // Binding 5-8: GBuffer sampled images (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(5)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(6)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(7)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(8)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 9: GBuffer sampler (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(9)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 10: CSM UBO (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(10)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
            // Binding 11-13: CSM shadow map sampled images (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(11)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(12)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(13)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 14: CSM shadow sampler (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(14)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 15: Point light cubemap array shadow (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(15)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 16: Point light shadow sampler (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(16)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 17: Spot light 2D array shadow (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(17)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 18: Spot light shadow sampler (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(18)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            // Binding 19: Spot light shadow UBO (fixed)
            vk::DescriptorSetLayoutBinding::default()
                .binding(19)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];

        let binding_flags = [
            vk::DescriptorBindingFlags::empty(), // UBO: traditional
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::empty(), // CSM UBO: traditional
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND
                | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
            vk::DescriptorBindingFlags::empty(), // Spot shadow UBO: traditional
        ];
        let mut binding_flags_info =
            vk::DescriptorSetLayoutBindingFlagsCreateInfo::default()
                .binding_flags(&binding_flags);

        let layout = unsafe {
            device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default()
                        .bindings(&bindings)
                        .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
                        .push_next(&mut binding_flags_info),
                    None,
                )
                .map_err(|e| {
                    RenderError::DeviceCreation(format!("bindless layout: {e}"))
                })?
        };

        // --- Allocate descriptor set ---
        let layouts = [layout];
        let mut sets = unsafe {
            device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pool)
                        .set_layouts(&layouts),
                )
                .map_err(|e| {
                    RenderError::DeviceCreation(format!("bindless set alloc: {e}"))
                })?
        };
        let set = sets.remove(0);

        // Pre-allocate free lists
        let mut free_texture_slots: Vec<u32> = (0..max_textures).collect();
        free_texture_slots.reverse(); // pop() gives 0 first
        let mut free_sampler_slots: Vec<u32> = (0..max_samplers).collect();
        free_sampler_slots.reverse();
        let mut free_storage_0: Vec<u32> = (0..max_storage).collect();
        free_storage_0.reverse();
        let mut free_storage_1: Vec<u32> = (0..max_storage).collect();
        free_storage_1.reverse();

        Ok(Self {
            device: device as *const ash::Device,
            pool,
            layout,
            set,
            texture_slots: Mutex::new(vec![None; max_textures as usize]),
            free_texture_slots: Mutex::new(free_texture_slots),
            sampler_slots: Mutex::new(vec![None; max_samplers as usize]),
            free_sampler_slots: Mutex::new(free_sampler_slots),
            storage_slots: [
                Mutex::new(vec![None; max_storage as usize]),
                Mutex::new(vec![None; max_storage as usize]),
            ],
            free_storage_slots: [
                Mutex::new(free_storage_0),
                Mutex::new(free_storage_1),
            ],
        })
    }

    pub fn set(&self) -> vk::DescriptorSet {
        self.set
    }

    pub fn layout(&self) -> vk::DescriptorSetLayout {
        self.layout
    }
}

mod ops;

impl Drop for BindlessDescriptorHeap {
    fn drop(&mut self) {
        unsafe {
            if !self.device.is_null() {
                let dev = &*self.device;
                if self.layout != vk::DescriptorSetLayout::null() {
                    dev.destroy_descriptor_set_layout(self.layout, None);
                }
                if self.pool != vk::DescriptorPool::null() {
                    dev.destroy_descriptor_pool(self.pool, None);
                }
            }
        }
    }
}
