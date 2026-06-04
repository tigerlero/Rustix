use std::collections::HashMap;
use std::time::Instant;
use ash::vk;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4, Quat, EulerRot, Aabb, Frustum};
use rustix_render::Renderer;
use rustix_render::mesh::Mesh;
use rustix_render::pipeline::GraphicsPipeline;
use rustix_render::pipeline::GraphicsPipeline2D;
use rustix_render::memory::GpuBuffer;
use rustix_render::GpuTexture;
use rustix_render::DepthBuffer;

use crate::camera::EditorCamera;
use crate::scene::{Transform, MeshComponent, Material, world_transform};
use rustix_render::{PointLight, SpotLight, DirectionalLight};

/// Compute the light view-projection matrix for shadow mapping.
/// The light is placed behind the target center looking toward it.
pub fn compute_light_view_proj(light_dir: Vec3, center: Vec3) -> Mat4 {
    let light_dir = light_dir.normalize();
    let light_pos = center - light_dir * 20.0;
    let light_view = Mat4::look_at_rh(light_pos, center, Vec3::Y);
    let light_proj = Mat4::orthographic_rh_gl(-15.0, 15.0, -15.0, 15.0, 0.1, 50.0);
    light_proj * light_view
}

/// Compute directional light direction from euler rotation (XYZ order).
/// The light points along -Z in local space, rotated by the given euler angles.
pub fn directional_light_dir_from_euler(rotation: Vec3) -> Vec3 {
    let rot = Quat::from_euler(EulerRot::XYZ, rotation.x, rotation.y, rotation.z);
    (rot * Vec3::NEG_Z).normalize()
}

pub fn render_3d_scene(
    renderer: &Renderer,
    cmd: vk::CommandBuffer,
    pipeline: &GraphicsPipeline,
    shadow_pipeline: Option<&rustix_render::pipeline::ShadowPipeline>,
    depth_buf: &DepthBuffer,
    shadow_map: Option<&rustix_render::GpuTexture>,
    mut shadow_layout: Option<vk::ImageLayout>,
    ubo: &GpuBuffer,
    meshes: &HashMap<String, Mesh>,
    ecs_world: &EcsWorld,
    cam: &EditorCamera,
    offscreen: Option<&rustix_render::Framebuffer>,
    hdr_fb: Option<&rustix_render::HdrFramebuffer>,
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

    let mut point_lights: Vec<(Vec3, f32, Vec3, f32)> = Vec::new();
    for (_e, pl, xform) in ecs_world.query::<(Entity, &PointLight, &Transform)>().iter() {
        point_lights.push((
            xform.position,
            pl.radius.max(0.1),
            Vec3::new(pl.color.x * pl.intensity, pl.color.y * pl.intensity, pl.color.z * pl.intensity),
            pl.intensity,
        ));
    }
    for (_e, sl, xform) in ecs_world.query::<(Entity, &SpotLight, &Transform)>().iter() {
        point_lights.push((
            xform.position,
            sl.radius.max(0.1),
            Vec3::new(sl.color.x * sl.intensity, sl.color.y * sl.intensity, sl.color.z * sl.intensity),
            sl.intensity,
        ));
    }
    point_lights.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
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

    let light_view_proj = compute_light_view_proj(light_dir.truncate(), cam.center);

    let mut ubo_data = [0u8; 432];
    ubo_data[0..64].copy_from_slice(bytemuck::bytes_of(&view_proj));
    ubo_data[64..80].copy_from_slice(bytemuck::bytes_of(&Vec4::new(eye.x, eye.y, eye.z, 0.0)));
    ubo_data[80..84].copy_from_slice(&light_count.to_ne_bytes());
    for (i, (pos, radius, color, _)) in point_lights.iter().take(8).enumerate() {
        let off = 96 + i * 32;
        ubo_data[off..off+16].copy_from_slice(bytemuck::bytes_of(&Vec4::new(pos.x, pos.y, pos.z, *radius)));
        ubo_data[off+16..off+32].copy_from_slice(bytemuck::bytes_of(&Vec4::new(color.x, color.y, color.z, 1.0)));
    }
    let fog_color = Vec4::new(0.15, 0.18, 0.25, cam.distance * 3.0 + 10.0);
    ubo_data[352..368].copy_from_slice(bytemuck::bytes_of(&fog_color));
    ubo_data[368..432].copy_from_slice(bytemuck::bytes_of(&light_view_proj));
    ubo.write(&ubo_data);

    // --- Shadow pass ---
    if let (Some(sp), Some(sm), Some(layout)) = (shadow_pipeline, shadow_map, shadow_layout) {
        let shadow_size = 1024u32;
        renderer.update_descriptor_set(vk::DescriptorSet::null(), ubo);
        renderer.transition_image_layout(cmd, sm.image, vk::ImageAspectFlags::DEPTH, layout, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        shadow_layout = Some(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        renderer.begin_shadow_pass(cmd, sm, shadow_size);

        for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
            if let Some(mesh) = meshes.get(&mesh_comp.0) {
                let model = world_transform(ecs_world, entity);
                let mut pc_data = [0u8; 128];
                pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&model));
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
        shadow_layout = Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

        renderer.update_descriptor_set_with_shadow(vk::DescriptorSet::null(), ubo, sm);
    } else {
        renderer.update_descriptor_set(vk::DescriptorSet::null(), ubo);
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

    for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
        if let Some(mesh) = meshes.get(&mesh_comp.0) {
            let model = world_transform(ecs_world, entity);
            let world_aabb = mesh.aabb.transform(model);
            if !frustum.intersects_aabb(&world_aabb) {
                continue;
            }

            let mat: Option<(Vec4, f32)> = ecs_world.get::<&Material>(entity).ok()
                .map(|m| (Vec4::new(m.base_color.x, m.base_color.y, m.base_color.z, m.roughness), m.metallic));
            let (mat_v, metallic) = mat.unwrap_or((Vec4::new(0.7, 0.7, 0.7, 0.5), 0.0));

            let mut pc_data = [0u8; 128];
            pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&model));
            pc_data[64..80].copy_from_slice(bytemuck::bytes_of(&light_dir));
            pc_data[80..96].copy_from_slice(bytemuck::bytes_of(&light_color));
            pc_data[96..112].copy_from_slice(bytemuck::bytes_of(&mat_v));
            pc_data[112..128].copy_from_slice(bytemuck::bytes_of(&Vec4::new(metallic, 0.0, 0.0, 0.0)));

            renderer.draw_indexed_in_pass(
                cmd, pipeline,
                &mesh.vertex_buffer,
                mesh.index_buffer.as_ref(), mesh.index_count,
                &pc_data,
            );
        }
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

pub fn render_2d_overlay(
    renderer: &Renderer,
    cmd: vk::CommandBuffer,
    pipeline: &GraphicsPipeline2D,
    quad_buffer: &GpuBuffer,
    ubo_2d: &GpuBuffer,
    texture: &GpuTexture,
    desc_set: vk::DescriptorSet,
    start_time: Instant,
) {
    let sw = renderer.swapchain.lock();
    let w = sw.extent().width as f32;
    let h = sw.extent().height as f32;
    drop(sw);
    let ortho = Mat4::orthographic_rh_gl(0.0, w, h, 0.0, -1.0, 1.0);
    ubo_2d.write(bytemuck::bytes_of(&ortho));
    renderer.update_2d_descriptor_set(desc_set, ubo_2d, texture);

    let t = start_time.elapsed().as_secs_f32();
    let pulse = (t * 2.0).sin() * 0.3 + 0.7;
    let s = 100.0 * pulse;
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(s, s, 1.0),
        Quat::from_rotation_z(t * 1.5),
        Vec3::new(w * 0.5, h * 0.5, 0.0),
    );
    let mut pc = [0u8; 64];
    pc.copy_from_slice(bytemuck::bytes_of(&model));
    renderer.draw_2d(cmd, pipeline, quad_buffer, 4, &pc, desc_set);
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
