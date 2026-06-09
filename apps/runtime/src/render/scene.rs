use std::collections::HashMap;
use ash::vk;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec2, Vec3, Vec4, Mat4, Frustum};
use rustix_render::Renderer;
use rustix_render::mesh::Mesh;
use rustix_render::pipeline::GraphicsPipeline;
use rustix_render::memory::GpuBuffer;
use rustix_render::DepthBuffer;
use rustix_render::{PointLight, SpotLight, DirectionalLight};
use crate::camera::EditorCamera;
use crate::scene::{Transform, MeshComponent, Material, world_transform};
use super::{CsmResources, PointShadowResources, SpotShadowResources, ForwardPlusResources, collect_lights, directional_light_dir_from_euler};

pub fn render_3d_scene(
    renderer: &Renderer,
    cmd: vk::CommandBuffer,
    pipeline: &GraphicsPipeline,
    shadow_pipeline: Option<&rustix_render::pipeline::ShadowPipeline>,
    depth_buf: &DepthBuffer,
    csm: Option<&CsmResources>,
    point_shadow: Option<&PointShadowResources>,
    spot_shadow: Option<&mut SpotShadowResources>,
    mut shadow_layout: Option<vk::ImageLayout>,
    ubo: &GpuBuffer,
    meshes: &HashMap<String, Mesh>,
    ecs_world: &EcsWorld,
    cam: &EditorCamera,
    offscreen: Option<&rustix_render::Framebuffer>,
    hdr_fb: Option<&rustix_render::HdrFramebuffer>,
    fwd_plus: Option<&ForwardPlusResources>,
) -> Option<vk::ImageLayout> {
    let (aspect, target_extent, color_image, color_view, fb_depth) = if let Some(fb) = offscreen {
        let ext = fb.extent;
        (
            ext.width as f32 / ext.height as f32,
            Some(ext),
            Some(fb.color_image),
            Some(fb.color_view),
            Some(&fb.depth_buffer),
        )
    } else if let Some(hfb) = hdr_fb {
        let ext = hfb.extent;
        (
            ext.width as f32 / ext.height as f32,
            Some(ext),
            Some(hfb.color_image),
            Some(hfb.color_view),
            Some(&hfb.depth_buffer),
        )
    } else {
        let sw = renderer.swapchain.lock();
        let ext = sw.extent();
        (
            ext.width as f32 / ext.height as f32,
            None,
            None,
            None,
            None,
        )
    };
    let depth_buf = fb_depth.unwrap_or(depth_buf);
    let view_proj = cam.view_proj(aspect);
    let eye = cam.eye_pos();

    let point_lights = collect_lights(ecs_world);
    let light_count = point_lights.len().min(8) as u32;

    let (light_dir, light_color) = {
        let mut d = Vec3::new(0.5, 0.8, 0.3);
        let mut c = Vec3::new(1.0, 0.95, 0.8);
        for (dirlight, xform) in ecs_world.query::<(&DirectionalLight, &Transform)>().iter() {
            d = directional_light_dir_from_euler(xform.rotation);
            c = Vec3::new(dirlight.color.x * dirlight.intensity, dirlight.color.y * dirlight.intensity, dirlight.color.z * dirlight.intensity);
            break;
        }
        (Vec4::new(d.x, d.y, d.z, 0.2), Vec4::new(c.x, c.y, c.z, 1.0))
    };

    let light_view_proj = csm.map(|c| c.ubo_data.light_view_proj[0]).unwrap_or(Mat4::IDENTITY);

    // Determine screen dimensions for fog field (used by fragment shader for tile calculation)
    let screen_dims = if let Some(ext) = target_extent {
        Vec2::new(ext.width as f32, ext.height as f32)
    } else {
        let sw = renderer.swapchain.lock();
        let ext = sw.extent();
        Vec2::new(ext.width as f32, ext.height as f32)
    };

    let mut ubo_data = [0u8; 432];
    ubo_data[0..64].copy_from_slice(bytemuck::bytes_of(&view_proj));
    ubo_data[64..80].copy_from_slice(bytemuck::bytes_of(&Vec4::new(eye.x, eye.y, eye.z, 0.0)));
    ubo_data[80..84].copy_from_slice(&light_count.to_ne_bytes());
    for (i, (pos, radius, color, _)) in point_lights.iter().take(8).enumerate() {
        let off = 96 + i * 32;
        ubo_data[off..off+16].copy_from_slice(bytemuck::bytes_of(&Vec4::new(pos.x, pos.y, pos.z, *radius)));
        ubo_data[off+16..off+32].copy_from_slice(bytemuck::bytes_of(&Vec4::new(color.x, color.y, color.z, 1.0)));
    }
    // fog.zw stores screen width/height for Forward+ tile calculation
    let fog_color = Vec4::new(0.15, 0.18, screen_dims.x, screen_dims.y);
    ubo_data[352..368].copy_from_slice(bytemuck::bytes_of(&fog_color));
    ubo_data[368..432].copy_from_slice(bytemuck::bytes_of(&light_view_proj));
    ubo.write(&ubo_data);

    // Populate Forward+ light buffer if available
    if let Some(fwd) = fwd_plus {
        let total_gpu_lights = point_lights.len().min(ForwardPlusResources::MAX_LIGHTS);
        let mut light_data = vec![0u8; total_gpu_lights * 32];
        for (i, (pos, radius, color, _)) in point_lights.iter().take(total_gpu_lights).enumerate() {
            let off = i * 32;
            light_data[off..off+16].copy_from_slice(bytemuck::bytes_of(&Vec4::new(pos.x, pos.y, pos.z, *radius)));
            light_data[off+16..off+32].copy_from_slice(bytemuck::bytes_of(&Vec4::new(color.x, color.y, color.z, 1.0)));
        }
        fwd.light_buffer.write(&light_data);
    }

    // --- CSM Shadow passes ---
    if let (Some(sp), Some(ref mut c), Some(layout)) = (shadow_pipeline, csm, shadow_layout) {
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
                    renderer.draw_shadow_in_pass(
                        cmd, sp,
                        &mesh.vertex_buffer,
                        mesh.index_buffer.as_ref(), mesh.index_count,
                        &pc_data,
                    );
                }
            }

            renderer.end_shadow_pass(cmd);
            renderer.transition_image_layout(cmd, sm.image, vk::ImageAspectFlags::DEPTH, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        }
        shadow_layout = Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }

    // --- Point light shadow passes (cubemap faces) ---
    if let (Some(sp), Some(ps), Some(layout)) = (shadow_pipeline, point_shadow, shadow_layout) {
        let face_size = ps.face_size;
        let mut light_query = ecs_world.query::<(Entity, &PointLight, &Transform)>();
        for (light_idx, (_e, _pl, xform)) in light_query.iter().enumerate().take(ps.max_lights as usize) {
            for face in 0..6 {
                let layer = (light_idx * 6 + face) as u32;
                let view = ps.face_views[layer as usize];
                // Cube face projection matrices
                let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 25.0);
                let view_mat = match face {
                    0 => Mat4::look_at_rh(xform.position, xform.position + Vec3::X, -Vec3::Y),   // +X
                    1 => Mat4::look_at_rh(xform.position, xform.position - Vec3::X, -Vec3::Y),   // -X
                    2 => Mat4::look_at_rh(xform.position, xform.position + Vec3::Y, Vec3::Z),     // +Y
                    3 => Mat4::look_at_rh(xform.position, xform.position - Vec3::Y, -Vec3::Z),  // -Y
                    4 => Mat4::look_at_rh(xform.position, xform.position + Vec3::Z, -Vec3::Y),   // +Z
                    5 => Mat4::look_at_rh(xform.position, xform.position - Vec3::Z, -Vec3::Y),   // -Z
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
        shadow_layout = Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }

    // --- Spot light shadow passes (2D array layers) ---
    if let (Some(sp), Some(ss), Some(layout)) = (shadow_pipeline, spot_shadow, shadow_layout) {
        let size = ss.size;
        let mut light_query = ecs_world.query::<(Entity, &SpotLight, &Transform)>();
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
        shadow_layout = Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }

    let clear_color = [0.04, 0.04, 0.08, 1.0f32];
    if let (Some(ext), Some(ci), Some(cv)) = (target_extent, color_image, color_view) {
        if hdr_fb.is_some() {
            tracing::trace!("render_3d_scene: using HDR pass {}x{}", ext.width, ext.height);
            hdr_fb.unwrap().begin_rendering(cmd, renderer.device(), &renderer.instance, clear_color);
        } else {
            tracing::trace!("render_3d_scene: using offscreen pass {}x{}", ext.width, ext.height);
            renderer.begin_scene_pass_offscreen(cmd, ci, cv, depth_buf, ext, clear_color);
        }
    } else {
        tracing::trace!("render_3d_scene: using swapchain pass");
        renderer.begin_scene_pass(cmd, depth_buf, clear_color);
    }

    let frustum = Frustum::from_view_proj(&view_proj);
    let mut entity_count = 0u32;
    let mut drawn_count = 0u32;
    let mut missing_mesh_count = 0u32;
    let mut mesh_signature = 0u64;

    for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
        entity_count += 1;
        for b in mesh_comp.0.bytes() {
            mesh_signature = mesh_signature.wrapping_mul(31).wrapping_add(b as u64);
        }
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
                cmd, pipeline,
                &mesh.vertex_buffer,
                mesh.index_buffer.as_ref(), mesh.index_count,
                &pc_data,
            );
            drawn_count += 1;
        } else {
            missing_mesh_count += 1;
            tracing::warn!("render_3d_scene: no mesh found for '{}'", mesh_comp.0);
        }
    }

    // Only log when the counts actually change (entity inserted/deleted/mesh loaded)
    {
        use std::cell::RefCell;
        thread_local! {
            static LAST_COUNTS: RefCell<(u32, u32, u32, u64)> = RefCell::new((u32::MAX, u32::MAX, u32::MAX, u64::MAX));
        }
        LAST_COUNTS.with(|last| {
            let prev = *last.borrow();
            let curr = (entity_count, drawn_count, missing_mesh_count, mesh_signature);
            if curr != prev {
                tracing::info!("render_3d_scene: {} entities, {} drawn, {} missing mesh", entity_count, drawn_count, missing_mesh_count);
                *last.borrow_mut() = curr;
            }
        });
    }

    if let Some(ci) = color_image {
        if hdr_fb.is_some() {
            hdr_fb.unwrap().end_rendering(cmd, renderer.device(), &renderer.instance);
        } else {
            renderer.end_scene_pass_offscreen(cmd, ci);
        }
    } else {
        renderer.end_scene_pass(cmd);
    }
    shadow_layout
}
