use std::collections::HashMap;
use ash::vk;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec4, Mat4};
use rustix_render::mesh::Mesh;
use rustix_render::memory::{GpuBuffer, GpuMemoryAllocator};
use rustix_render::device::GpuDevice;
use rustix_render::RenderError;
use rustix_core::math::Frustum;
use crate::scene::{Transform, MeshComponent, Material, world_transform};

/// Per-instance data passed to the GPU via vertex attributes (binding 1, PER_INSTANCE).
/// Layout must match the shader: model matrix (64b) + base_color (16b) + material (16b).
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct InstanceData {
    pub model_col0: [f32; 4],
    pub model_col1: [f32; 4],
    pub model_col2: [f32; 4],
    pub model_col3: [f32; 4],
    pub base_color: [f32; 4],
    pub material: [f32; 4], // x=roughness, y=metallic, z=ao, w=emissive
}

impl InstanceData {
    pub fn from_transform_and_material(model: &Mat4, base_color: &Vec4, material: &Vec4) -> Self {
        Self {
            model_col0: [model.x_axis.x, model.x_axis.y, model.x_axis.z, model.x_axis.w],
            model_col1: [model.y_axis.x, model.y_axis.y, model.y_axis.z, model.y_axis.w],
            model_col2: [model.z_axis.x, model.z_axis.y, model.z_axis.z, model.z_axis.w],
            model_col3: [model.w_axis.x, model.w_axis.y, model.w_axis.z, model.w_axis.w],
            base_color: [base_color.x, base_color.y, base_color.z, base_color.w],
            material: [material.x, material.y, material.z, material.w],
        }
    }
}

/// A GPU buffer that holds per-instance data for one frame.
#[allow(dead_code)]
pub struct InstanceBuffer {
    pub buffer: GpuBuffer,
    pub capacity: usize, // max instances
}

impl InstanceBuffer {
    pub fn new(
        device: &GpuDevice,
        allocator: &mut GpuMemoryAllocator,
        max_instances: usize,
    ) -> Result<Self, RenderError> {
        let size = (max_instances * std::mem::size_of::<InstanceData>()) as u64;
        let buffer = GpuBuffer::new(
            device, allocator, "instance_buffer", size,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        Ok(Self { buffer, capacity: max_instances })
    }

    /// Write instance data into the mapped buffer. Returns number of instances written.
    pub fn write(&mut self, instances: &[InstanceData]) -> usize {
        let count = instances.len().min(self.capacity);
        if count == 0 { return 0; }
        if let Some(ptr) = self.buffer.mapped_ptr {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    instances.as_ptr() as *const u8,
                    ptr,
                    count * std::mem::size_of::<InstanceData>(),
                );
            }
        }
        count
    }

    #[allow(dead_code)]
    pub fn destroy(&mut self, device: &ash::Device) {
        unsafe { device.destroy_buffer(self.buffer.buffer, None); }
    }
}

/// GPU buffer holding VkDrawIndexedIndirectCommand entries, one per mesh batch.
#[allow(dead_code)]
pub struct IndirectDrawBuffer {
    pub buffer: GpuBuffer,
    pub capacity: usize,
}

impl IndirectDrawBuffer {
    pub fn new(
        device: &GpuDevice,
        allocator: &mut GpuMemoryAllocator,
        max_commands: usize,
    ) -> Result<Self, RenderError> {
        let size = (max_commands * std::mem::size_of::<vk::DrawIndexedIndirectCommand>()) as u64;
        let buffer = GpuBuffer::new(
            device, allocator, "indirect_draw_buffer", size,
            vk::BufferUsageFlags::INDIRECT_BUFFER,
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;
        Ok(Self { buffer, capacity: max_commands })
    }

    pub fn write(&mut self, commands: &[vk::DrawIndexedIndirectCommand]) -> usize {
        let count = commands.len().min(self.capacity);
        if count == 0 { return 0; }
        if let Some(ptr) = self.buffer.mapped_ptr {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    commands.as_ptr() as *const u8,
                    ptr,
                    count * std::mem::size_of::<vk::DrawIndexedIndirectCommand>(),
                );
            }
        }
        count
    }

    #[allow(dead_code)]
    pub fn destroy(&mut self, device: &ash::Device) {
        unsafe { device.destroy_buffer(self.buffer.buffer, None); }
    }
}

/// One mesh batch: all entities sharing the same mesh.
#[allow(dead_code)]
pub struct MeshBatch {
    pub mesh_name: String,
    pub instance_offset: u32, // index into the instance buffer
    pub instance_count: u32,
    pub index_count: u32,
}

/// Groups visible scene entities by mesh and builds instance/indirect buffers.
#[allow(dead_code)]
pub struct InstancedMeshBatcher {
    pub instance_buffer: InstanceBuffer,
    pub indirect_buffer: IndirectDrawBuffer,
    pub batches: Vec<MeshBatch>,
    /// CPU-side copy of the last built instances, for GPU culling upload.
    pub cpu_instances: Vec<InstanceData>,
    max_instances: usize,
    max_batches: usize,
}

#[allow(dead_code)]
impl InstancedMeshBatcher {
    pub fn new(
        device: &GpuDevice,
        allocator: &mut GpuMemoryAllocator,
        max_instances: usize,
        max_batches: usize,
    ) -> Result<Self, RenderError> {
        Ok(Self {
            instance_buffer: InstanceBuffer::new(device, allocator, max_instances)?,
            indirect_buffer: IndirectDrawBuffer::new(device, allocator, max_batches)?,
            batches: Vec::with_capacity(max_batches),
            cpu_instances: Vec::new(),
            max_instances,
            max_batches,
        })
    }

    /// Rebuild instance and indirect buffers for the current frame.
    /// Returns number of instances and number of batches.
    pub fn build(
        &mut self,
        ecs_world: &EcsWorld,
        meshes: &HashMap<String, Mesh>,
        frustum: &Frustum,
    ) -> (usize, usize) {
        self.batches.clear();

        // Collect visible entities grouped by mesh
        let mut grouped: HashMap<String, Vec<(Entity, Mat4, Vec4, Vec4)>> = HashMap::new();
        for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
            if let Some(mesh) = meshes.get(&mesh_comp.0) {
                let model = world_transform(ecs_world, entity);
                let world_aabb = mesh.aabb.transform(model);
                if !frustum.intersects_aabb(&world_aabb) {
                    continue;
                }
                let mat = ecs_world.get::<&Material>(entity).ok();
                let base_color = mat.as_ref().map(|m| Vec4::new(m.base_color.x, m.base_color.y, m.base_color.z, 1.0))
                    .unwrap_or(Vec4::new(0.7, 0.7, 0.7, 1.0));
                let material = mat.as_ref().map(|m| Vec4::new(m.roughness, m.metallic, m.ao, m.emissive))
                    .unwrap_or(Vec4::new(0.5, 0.0, 1.0, 0.0));
                grouped.entry(mesh_comp.0.clone()).or_default().push((entity, model, base_color, material));
            }
        }

        let mut instances: Vec<InstanceData> = Vec::with_capacity(self.max_instances);
        let mut commands: Vec<vk::DrawIndexedIndirectCommand> = Vec::with_capacity(self.max_batches);

        for (mesh_name, entity_list) in grouped {
            if self.batches.len() >= self.max_batches { break; }
            if let Some(mesh) = meshes.get(&mesh_name) {
                let offset = instances.len() as u32;
                let count = entity_list.len().min(self.max_instances - instances.len());
                if count == 0 { continue; }
                for (_entity, model, base_color, material) in &entity_list[..count] {
                    instances.push(InstanceData::from_transform_and_material(model, base_color, material));
                }
                let index_count = mesh.index_count;
                commands.push(vk::DrawIndexedIndirectCommand {
                    index_count,
                    instance_count: count as u32,
                    first_index: 0,
                    vertex_offset: 0,
                    first_instance: offset,
                });
                self.batches.push(MeshBatch {
                    mesh_name,
                    instance_offset: offset,
                    instance_count: count as u32,
                    index_count,
                });
            }
        }

        self.instance_buffer.write(&instances);
        self.indirect_buffer.write(&commands);
        self.cpu_instances = instances;

        (self.cpu_instances.len(), self.batches.len())
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        self.instance_buffer.destroy(device);
        self.indirect_buffer.destroy(device);
    }
}
