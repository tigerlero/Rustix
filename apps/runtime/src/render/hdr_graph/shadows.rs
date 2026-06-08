use std::collections::HashMap;
use ash::vk;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4};
use rustix_render::Renderer;
use rustix_render::mesh::Mesh;
use crate::scene::{Transform, MeshComponent, world_transform};
use crate::render::{CsmResources, PointShadowResources, SpotShadowResources};
use crate::render::directional_light_dir_from_euler;

/// Render CSM, point, and spot shadow maps.
/// Returns the new layout of shadow images (usually SHADER_READ_ONLY_OPTIMAL).
pub fn render_shadow_passes(
    renderer: &Renderer,
    cmd: vk::CommandBuffer,
    ecs_world: &EcsWorld,
    meshes: &HashMap<String, Mesh>,
    shadow_pipeline: Option<&rustix_render::pipeline::ShadowPipeline>,
    csm: Option<&mut CsmResources>,
    point_shadow: Option<&PointShadowResources>,
    spot_shadow: Option<&mut SpotShadowResources>,
    shadow_layout: Option<vk::ImageLayout>,
) -> Option<vk::ImageLayout> {
    let mut new_shadow_layout = shadow_layout;

    // CSM shadows
    if let (Some(sp), Some(c), Some(layout)) = (shadow_pipeline, csm, new_shadow_layout) {
        let shadow_size = c.shadow_map_size;
        for i in 0..3 {
            let sm = &c.shadow_maps[i];
            let light_matrix = c.ubo_data.light_view_proj[i];
            renderer.transition_image_layout(cmd, sm.image, vk::ImageAspectFlags::DEPTH, layout, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
            renderer.begin_shadow_pass(cmd, sm.view, shadow_size);
            for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
                if let Some(mesh) = meshes.get(&mesh_comp.0) {
                    let model = world_transform(ecs_world, entity);
                    let mut pc_data = [0u8; 128];
                    pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&model));
                    pc_data[64..128].copy_from_slice(bytemuck::bytes_of(&light_matrix));
                    renderer.draw_shadow_in_pass(cmd, sp, &mesh.vertex_buffer, mesh.index_buffer.as_ref(), mesh.index_count, &pc_data);
                }
            }
            renderer.end_shadow_pass(cmd);
            renderer.transition_image_layout(cmd, sm.image, vk::ImageAspectFlags::DEPTH, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        }
        new_shadow_layout = Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }

    // Point shadows
    if let (Some(sp), Some(ps), Some(layout)) = (shadow_pipeline, point_shadow, new_shadow_layout) {
        let face_size = ps.face_size;
        let mut light_query = ecs_world.query::<(Entity, &rustix_render::PointLight, &Transform)>();
        for (light_idx, (_e, _pl, xform)) in light_query.iter().enumerate().take(ps.max_lights as usize) {
            for face in 0..6 {
                let layer = (light_idx * 6 + face) as u32;
                let view = ps.face_views[layer as usize];
                let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 25.0);
                let view_mat = match face {
                    0 => Mat4::look_at_rh(xform.position, xform.position + Vec3::X, -Vec3::Y),
                    1 => Mat4::look_at_rh(xform.position, xform.position - Vec3::X, -Vec3::Y),
                    2 => Mat4::look_at_rh(xform.position, xform.position + Vec3::Y, Vec3::Z),
                    3 => Mat4::look_at_rh(xform.position, xform.position - Vec3::Y, -Vec3::Z),
                    4 => Mat4::look_at_rh(xform.position, xform.position + Vec3::Z, -Vec3::Y),
                    5 => Mat4::look_at_rh(xform.position, xform.position - Vec3::Z, -Vec3::Y),
                    _ => unreachable!(),
                };
                let light_matrix = proj * view_mat;
                renderer.transition_image_layout(cmd, ps.cubemap.image, vk::ImageAspectFlags::DEPTH, layout, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
                renderer.begin_shadow_pass(cmd, view, face_size);
                for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
                    if let Some(mesh) = meshes.get(&mesh_comp.0) {
                        let model = world_transform(ecs_world, entity);
                        let mut pc_data = [0u8; 128];
                        pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&model));
                        pc_data[64..128].copy_from_slice(bytemuck::bytes_of(&light_matrix));
                        renderer.draw_shadow_in_pass(cmd, sp, &mesh.vertex_buffer, mesh.index_buffer.as_ref(), mesh.index_count, &pc_data);
                    }
                }
                renderer.end_shadow_pass(cmd);
            }
        }
        renderer.transition_image_layout(cmd, ps.cubemap.image, vk::ImageAspectFlags::DEPTH, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        new_shadow_layout = Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }

    // Spot shadows
    if let (Some(sp), Some(ss), Some(layout)) = (shadow_pipeline, spot_shadow, new_shadow_layout) {
        let size = ss.size;
        let mut light_query = ecs_world.query::<(Entity, &rustix_render::SpotLight, &Transform)>();
        for (light_idx, (_e, sl, xform)) in light_query.iter().enumerate().take(ss.max_lights as usize) {
            let layer = light_idx as u32;
            let view = ss.layer_views[layer as usize];
            let proj = Mat4::perspective_rh(sl.outer_angle, 1.0, 0.1, sl.radius.max(0.1));
            let forward = directional_light_dir_from_euler(xform.rotation);
            let view_mat = Mat4::look_at_rh(xform.position, xform.position + forward, Vec3::Y);
            let light_matrix = proj * view_mat;
            ss.ubo_data.view_proj[light_idx] = light_matrix;
            ss.ubo_data.params[light_idx] = Vec4::new(xform.position.x, xform.position.y, xform.position.z, layer as f32);
            renderer.transition_image_layout(cmd, ss.array.image, vk::ImageAspectFlags::DEPTH, layout, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
            renderer.begin_shadow_pass(cmd, view, size);
            for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
                if let Some(mesh) = meshes.get(&mesh_comp.0) {
                    let model = world_transform(ecs_world, entity);
                    let mut pc_data = [0u8; 128];
                    pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&model));
                    pc_data[64..128].copy_from_slice(bytemuck::bytes_of(&light_matrix));
                    renderer.draw_shadow_in_pass(cmd, sp, &mesh.vertex_buffer, mesh.index_buffer.as_ref(), mesh.index_count, &pc_data);
                }
            }
            renderer.end_shadow_pass(cmd);
        }
        ss.upload_ubo();
        renderer.transition_image_layout(cmd, ss.array.image, vk::ImageAspectFlags::DEPTH, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        new_shadow_layout = Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }

    new_shadow_layout
}
