use std::collections::HashMap;
use std::time::Instant;
use ash::vk;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4, Quat, EulerRot};
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

pub fn render_3d_scene(
    renderer: &Renderer,
    cmd: vk::CommandBuffer,
    pipeline: &GraphicsPipeline,
    depth_buf: &DepthBuffer,
    ubo: &GpuBuffer,
    descriptor_set: vk::DescriptorSet,
    meshes: &HashMap<String, Mesh>,
    ecs_world: &EcsWorld,
    cam: &EditorCamera,
) {
    let aspect = {
        let sw = renderer.swapchain.lock();
        sw.extent().width as f32 / sw.extent().height as f32
    };
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

    let mut ubo_data = [0u8; 368];
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
    ubo.write(&ubo_data);

    renderer.update_descriptor_set(descriptor_set, ubo);

    let (light_dir, light_color) = {
        let mut d = Vec3::new(0.5, 0.8, 0.3);
        let mut c = Vec3::new(1.0, 0.95, 0.8);
        for (dirlight, xform) in ecs_world.query::<(&DirectionalLight, &Transform)>().iter() {
            let rot = Quat::from_euler(EulerRot::XYZ, xform.rotation.x, xform.rotation.y, xform.rotation.z);
            d = (rot * Vec3::NEG_Z).normalize();
            c = Vec3::new(dirlight.color.x * dirlight.intensity, dirlight.color.y * dirlight.intensity, dirlight.color.z * dirlight.intensity);
            break;
        }
        (Vec4::new(d.x, d.y, d.z, 0.2), Vec4::new(c.x, c.y, c.z, 1.0))
    };

    let clear_color = [0.04, 0.04, 0.08, 1.0f32];
    renderer.begin_scene_pass(cmd, depth_buf, clear_color);

    for (entity, _transform, mesh_comp) in ecs_world.query::<(Entity, &Transform, &MeshComponent)>().iter() {
        if let Some(mesh) = meshes.get(&mesh_comp.0) {
            let model = world_transform(ecs_world, entity);

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
                descriptor_set,
            );
        }
    }

    renderer.end_scene_pass(cmd);
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
