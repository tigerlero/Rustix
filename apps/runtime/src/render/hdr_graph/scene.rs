use std::collections::HashMap;
use ash::vk;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4, Frustum};
use rustix_render::Renderer;
use rustix_render::mesh::Mesh;
use rustix_render::pipeline::GraphicsPipeline;
use rustix_render::graph::PassContext;
use crate::scene::{Transform, MeshComponent, Material, world_transform};
use crate::render::ForwardPlusResources;

pub fn execute_light_cull(
    ctx: &mut PassContext,
    view_proj: Mat4,
    eye: Vec3,
    screen_w: u32,
    screen_h: u32,
    tile_count_x: u32,
    tile_count_y: u32,
    light_count: u32,
    fwd: &ForwardPlusResources,
) {
    unsafe {
        let device = ctx.renderer.device().logical();
        let bindless_set = ctx.renderer.bindless_heap().set();
        device.cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::COMPUTE, fwd.compute_pipeline);
        device.cmd_bind_descriptor_sets(ctx.cmd, vk::PipelineBindPoint::COMPUTE, fwd.compute_layout, 0, &[bindless_set], &[]);
        let mut pc_data = [0u8; 104];
        pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&view_proj));
        pc_data[64..80].copy_from_slice(bytemuck::bytes_of(&Vec4::new(eye.x, eye.y, eye.z, 0.0)));
        pc_data[80..84].copy_from_slice(bytemuck::bytes_of(&screen_w));
        pc_data[84..88].copy_from_slice(bytemuck::bytes_of(&screen_h));
        pc_data[88..92].copy_from_slice(bytemuck::bytes_of(&tile_count_x));
        pc_data[92..96].copy_from_slice(bytemuck::bytes_of(&tile_count_y));
        pc_data[96..100].copy_from_slice(bytemuck::bytes_of(&light_count));
        pc_data[100..104].copy_from_slice(bytemuck::bytes_of(&ForwardPlusResources::MAX_LIGHTS_PER_TILE));
        device.cmd_push_constants(ctx.cmd, fwd.compute_layout, vk::ShaderStageFlags::COMPUTE, 0, &pc_data);
        device.cmd_dispatch(ctx.cmd, tile_count_x, tile_count_y, 1);
    }
}

pub fn execute_gpu_cull(
    ctx: &mut PassContext,
    cull_pipe: vk::Pipeline,
    cull_layout: vk::PipelineLayout,
    cull_set: vk::DescriptorSet,
    instance_count: u32,
    cull_pc: &crate::render::CullPushConstants,
) {
    unsafe {
        let device = ctx.renderer.device().logical();
        device.cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::COMPUTE, cull_pipe);
        device.cmd_bind_descriptor_sets(ctx.cmd, vk::PipelineBindPoint::COMPUTE, cull_layout, 0, &[cull_set], &[]);
        device.cmd_push_constants(ctx.cmd, cull_layout, vk::ShaderStageFlags::COMPUTE, 0, bytemuck::bytes_of(cull_pc));
        let groups = (instance_count + 255) / 256;
        device.cmd_dispatch(ctx.cmd, groups, 1, 1);
    }
}

pub fn execute_gen_draw_cmds(
    ctx: &mut PassContext,
    gen_pipe: vk::Pipeline,
    gen_layout: vk::PipelineLayout,
    gen_set: vk::DescriptorSet,
    batch_count: u32,
    gen_pc: &crate::render::GenDrawPushConstants,
) {
    unsafe {
        let device = ctx.renderer.device().logical();
        device.cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::COMPUTE, gen_pipe);
        device.cmd_bind_descriptor_sets(ctx.cmd, vk::PipelineBindPoint::COMPUTE, gen_layout, 0, &[gen_set], &[]);
        device.cmd_push_constants(ctx.cmd, gen_layout, vk::ShaderStageFlags::COMPUTE, 0, bytemuck::bytes_of(gen_pc));
        let groups = (batch_count + 255) / 256;
        device.cmd_dispatch(ctx.cmd, groups, 1, 1);
    }
}

pub fn execute_scene_pass(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    view_proj: Mat4,
    light_dir: Vec4,
    light_color: Vec4,
    ecs_world: &EcsWorld,
    meshes: &HashMap<String, Mesh>,
    mesh_shader_enabled: bool,
    mesh_shader_pipeline: Option<&rustix_render::pipeline::MeshShaderPipeline>,
    gpu_culling_enabled: bool,
    gpu_culling: Option<&crate::render::GpuCullingResources>,
    instanced_enabled: bool,
    instanced_pipeline: Option<&rustix_render::pipeline::InstancedGraphicsPipeline>,
    instanced_batcher: Option<&crate::render::InstancedMeshBatcher>,
    scene_pipeline: &GraphicsPipeline,
) {
    let frustum = Frustum::from_view_proj(&view_proj);
    let use_mesh_shader = mesh_shader_enabled && mesh_shader_pipeline.is_some();
    if use_mesh_shader {
        let pipe = mesh_shader_pipeline.unwrap();
        for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
            if let Some(_mesh) = meshes.get(&mesh_comp.0) {
                let model = world_transform(ecs_world, entity);
                let world_aabb = _mesh.aabb.transform(model);
                if !frustum.intersects_aabb(&world_aabb) {
                    continue;
                }
                let mat = ecs_world.get::<&Material>(entity).ok();
                let base_color = mat.as_ref().map(|m| Vec4::new(m.base_color.x, m.base_color.y, m.base_color.z, 1.0))
                    .unwrap_or(Vec4::new(0.7, 0.7, 0.7, 1.0));
                let material = mat.as_ref().map(|m| Vec4::new(m.roughness, m.metallic, m.ao, m.emissive))
                    .unwrap_or(Vec4::new(0.5, 0.0, 1.0, 0.0));
                let mut pc_data = [0u8; 128];
                pc_data[0..16].copy_from_slice(bytemuck::bytes_of(&light_dir));
                pc_data[16..32].copy_from_slice(bytemuck::bytes_of(&light_color));
                pc_data[32..96].copy_from_slice(bytemuck::bytes_of(&model));
                pc_data[96..112].copy_from_slice(bytemuck::bytes_of(&base_color));
                pc_data[112..128].copy_from_slice(bytemuck::bytes_of(&material));
                renderer.draw_mesh_tasks_in_pass(cmd, pipe, 1, 0, &pc_data);
            }
        }
    } else if gpu_culling_enabled && gpu_culling.is_some() && instanced_enabled && instanced_pipeline.is_some() && instanced_batcher.is_some() {
        let batcher = instanced_batcher.unwrap();
        let cull_res = gpu_culling.unwrap();
        let pipe = instanced_pipeline.unwrap();
        let mut pc_data = [0u8; 128];
        pc_data[64..80].copy_from_slice(bytemuck::bytes_of(&light_dir));
        pc_data[80..96].copy_from_slice(bytemuck::bytes_of(&light_color));
        for (batch_idx, batch) in batcher.batches.iter().enumerate() {
            if let Some(mesh) = meshes.get(&batch.mesh_name) {
                let offset = (batch.instance_offset as u64) * std::mem::size_of::<crate::render::InstanceData>() as u64;
                renderer.draw_instanced_indexed_indirect_in_pass(
                    cmd, pipe,
                    &mesh.vertex_buffer,
                    mesh.index_buffer.as_ref().unwrap(),
                    &batcher.instance_buffer.buffer, offset,
                    &cull_res.draw_command_buffer,
                    (batch_idx * std::mem::size_of::<vk::DrawIndexedIndirectCommand>()) as u64,
                    &pc_data,
                );
            }
        }
    } else if instanced_enabled && instanced_pipeline.is_some() && instanced_batcher.is_some() {
        let batcher = instanced_batcher.unwrap();
        let pipe = instanced_pipeline.unwrap();
        let mut pc_data = [0u8; 128];
        pc_data[64..80].copy_from_slice(bytemuck::bytes_of(&light_dir));
        pc_data[80..96].copy_from_slice(bytemuck::bytes_of(&light_color));
        for batch in &batcher.batches {
            if let Some(mesh) = meshes.get(&batch.mesh_name) {
                let offset = (batch.instance_offset as u64) * std::mem::size_of::<crate::render::InstanceData>() as u64;
                renderer.draw_instanced_indexed_in_pass(
                    cmd, pipe,
                    &mesh.vertex_buffer,
                    mesh.index_buffer.as_ref(), mesh.index_count,
                    &batcher.instance_buffer.buffer,
                    batch.instance_count, offset,
                    &pc_data,
                );
            }
        }
    } else {
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
                let mut pc_data = [0u8; 128];
                pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&model));
                pc_data[64..80].copy_from_slice(bytemuck::bytes_of(&light_dir));
                pc_data[80..96].copy_from_slice(bytemuck::bytes_of(&light_color));
                pc_data[96..112].copy_from_slice(bytemuck::bytes_of(&base_color));
                pc_data[112..128].copy_from_slice(bytemuck::bytes_of(&material));
                renderer.draw_indexed_in_pass(
                    cmd, scene_pipeline,
                    &mesh.vertex_buffer,
                    mesh.index_buffer.as_ref(), mesh.index_count,
                    &pc_data,
                );
            }
        }
    }
}

pub fn execute_oit_accumulate(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    view_proj: Mat4,
    light_dir: Vec4,
    light_color: Vec4,
    ecs_world: &EcsWorld,
    meshes: &HashMap<String, Mesh>,
    oit_accum_pipe: &rustix_render::pipeline::OitAccumulatePipeline,
) {
    let frustum = Frustum::from_view_proj(&view_proj);
    let bindless_set = renderer.bindless_heap().set();
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, oit_accum_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, oit_accum_pipe.layout, 0, &[bindless_set], &[]);
    }
    for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
        if let Some(mesh) = meshes.get(&mesh_comp.0) {
            let model = world_transform(ecs_world, entity);
            let world_aabb = mesh.aabb.transform(model);
            if !frustum.intersects_aabb(&world_aabb) {
                continue;
            }
            let mat = ecs_world.get::<&Material>(entity).ok();
            let base_color = mat.as_ref().map(|m| Vec4::new(m.base_color.x, m.base_color.y, m.base_color.z, m.alpha))
                .unwrap_or(Vec4::new(0.7, 0.7, 0.7, 1.0));
            if base_color.w >= 1.0 {
                continue;
            }
            let material = mat.as_ref().map(|m| Vec4::new(m.roughness, m.metallic, m.ao, m.emissive))
                .unwrap_or(Vec4::new(0.5, 0.0, 1.0, 0.0));
            let mut pc_data = [0u8; 128];
            pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&model));
            pc_data[64..80].copy_from_slice(bytemuck::bytes_of(&light_dir));
            pc_data[80..96].copy_from_slice(bytemuck::bytes_of(&light_color));
            pc_data[96..112].copy_from_slice(bytemuck::bytes_of(&base_color));
            pc_data[112..128].copy_from_slice(bytemuck::bytes_of(&material));
            unsafe {
                let device = renderer.device().logical();
                device.cmd_push_constants(cmd, oit_accum_pipe.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, &pc_data);
                device.cmd_bind_vertex_buffers(cmd, 0, &[mesh.vertex_buffer.buffer], &[0u64]);
                if let Some(ref ib) = mesh.index_buffer {
                    device.cmd_bind_index_buffer(cmd, ib.buffer, 0, vk::IndexType::UINT16);
                    device.cmd_draw_indexed(cmd, mesh.index_count, 1, 0, 0, 0);
                } else {
                    device.cmd_draw(cmd, mesh.vertex_buffer.size as u32 / 24, 1, 0, 0);
                }
            }
        }
    }
}

pub fn execute_oit_composite(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    oit_comp_pipe: &rustix_render::pipeline::OitCompositePipeline,
    oit_desc_set: vk::DescriptorSet,
    oit_resources: &crate::render::OitResources,
    hdr_fb: &rustix_render::HdrFramebuffer,
) {
    let accum_ii = [vk::DescriptorImageInfo::default().image_view(oit_resources.accum_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let reveal_ii = [vk::DescriptorImageInfo::default().image_view(oit_resources.reveal_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let opaque_ii = [vk::DescriptorImageInfo::default().image_view(hdr_fb.color_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [
        vk::WriteDescriptorSet::default().dst_set(oit_desc_set).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&accum_ii),
        vk::WriteDescriptorSet::default().dst_set(oit_desc_set).dst_binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&reveal_ii),
        vk::WriteDescriptorSet::default().dst_set(oit_desc_set).dst_binding(5).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&opaque_ii),
    ];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, oit_comp_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, oit_comp_pipe.layout, 0, &[oit_desc_set], &[]);
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}
