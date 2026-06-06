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

/// Render the primary viewport using a declarative frame graph.
/// This replaces the manual HDR scene + tonemap sequence with a graph
/// that automatically inserts layout barriers between passes.
pub fn render_hdr_with_graph(
    renderer: &Renderer,
    cmd: vk::CommandBuffer,
    scene_pipeline: &GraphicsPipeline,
    shadow_pipeline: Option<&rustix_render::pipeline::ShadowPipeline>,
    depth_buf: &DepthBuffer,
    mut csm: Option<&mut CsmResources>,
    point_shadow: Option<&PointShadowResources>,
    spot_shadow: Option<&mut SpotShadowResources>,
    shadow_layout: Option<vk::ImageLayout>,
    ubo: &GpuBuffer,
    meshes: &HashMap<String, Mesh>,
    ecs_world: &EcsWorld,
    cam: &EditorCamera,
    hdr_fb: &rustix_render::HdrFramebuffer,
    tonemap_pipeline: &rustix_render::pipeline::ToneMapPipeline,
    tonemap_desc_set: vk::DescriptorSet,
    sampler: vk::Sampler,
    fwd_plus: Option<&ForwardPlusResources>,
) -> (Option<vk::ImageLayout>, Option<rustix_render::graph::FrameGraphSnapshot>) {
    use rustix_render::graph::{FrameGraph, PassDesc, TextureDesc, ResourceId, PassQueue};

    // --- Scene setup: UBO, lights, and shadow rendering (must be before frame graph) ---
    let aspect = hdr_fb.extent.width as f32 / hdr_fb.extent.height as f32;
    let view_proj = cam.view_proj(aspect);
    let eye = cam.eye_pos();
    let point_lights = collect_lights(ecs_world);
    let light_count = point_lights.len().min(8) as u32;
    let screen_w = hdr_fb.extent.width;
    let screen_h = hdr_fb.extent.height;
    let tile_count_x = (screen_w + ForwardPlusResources::TILE_SIZE - 1) / ForwardPlusResources::TILE_SIZE;
    let tile_count_y = (screen_h + ForwardPlusResources::TILE_SIZE - 1) / ForwardPlusResources::TILE_SIZE;

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

    let screen_dims = Vec2::new(screen_w as f32, screen_h as f32);
    let light_view_proj = csm.as_ref().map(|c| c.ubo_data.light_view_proj[0]).unwrap_or(Mat4::IDENTITY);

    let mut ubo_data = [0u8; 432];
    ubo_data[0..64].copy_from_slice(bytemuck::bytes_of(&view_proj));
    ubo_data[64..80].copy_from_slice(bytemuck::bytes_of(&Vec4::new(eye.x, eye.y, eye.z, 0.0)));
    ubo_data[80..84].copy_from_slice(&light_count.to_ne_bytes());
    for (i, (pos, radius, color, _)) in point_lights.iter().take(8).enumerate() {
        let off = 96 + i * 32;
        ubo_data[off..off+16].copy_from_slice(bytemuck::bytes_of(&Vec4::new(pos.x, pos.y, pos.z, *radius)));
        ubo_data[off+16..off+32].copy_from_slice(bytemuck::bytes_of(&Vec4::new(color.x, color.y, color.z, 1.0)));
    }
    let fog_color = Vec4::new(0.15, 0.18, screen_dims.x, screen_dims.y);
    ubo_data[352..368].copy_from_slice(bytemuck::bytes_of(&fog_color));
    ubo_data[368..432].copy_from_slice(bytemuck::bytes_of(&light_view_proj));
    ubo.write(&ubo_data);

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

    // Compute CSM cascades
    if let Some(ref mut c) = csm {
        let cam_view = match cam.mode {
            crate::camera::CameraMode::Orbit => Mat4::look_at_rh(cam.eye_pos(), cam.center, Vec3::Y),
            crate::camera::CameraMode::FirstPerson => {
                let forward = Vec3::new(cam.pitch.cos() * cam.yaw.sin(), cam.pitch.sin(), cam.pitch.cos() * cam.yaw.cos());
                Mat4::look_at_rh(cam.position, cam.position + forward, Vec3::Y)
            }
        };
        let cam_proj = Mat4::perspective_rh_gl(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);
        c.compute_cascades(&cam_view, &cam_proj, Vec3::new(light_dir.x, light_dir.y, light_dir.z));
        c.upload_ubo();
    }

    // Save CSM/spot data before consuming mutable borrows for shadow rendering
    let csm_data = csm.as_ref().map(|c| (
        c.shadow_map_size,
        c.shadow_maps.iter().map(|sm| (sm.view, sm.image)).collect::<Vec<_>>(),
    ));
    let spot_data = spot_shadow.as_ref().map(|ss| ((ss.array.view, ss.array.image), ss.size));

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
        let mut light_query = ecs_world.query::<(Entity, &PointLight, &Transform)>();
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
        new_shadow_layout = Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }

    // --- Frame graph ---
    let mut graph = FrameGraph::new();
    let (sw_extent, swapchain_image, swapchain_format, swapchain_view) = {
        let sw = renderer.swapchain.lock();
        let extent = sw.extent();
        let image = sw.current_image();
        let format = sw.format();
        let view = sw.current_image_view();
        (extent, image, format, view)
    };

    let hdr = graph.add_texture(TextureDesc {
        format: vk::Format::R16G16B16A16_SFLOAT,
        extent: hdr_fb.extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: Some(hdr_fb.color_view),
        image: Some(hdr_fb.color_image),
        persistent: true,
    });
    graph.set_initial_layout(hdr, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    let depth = graph.add_texture(TextureDesc {
        format: vk::Format::D32_SFLOAT,
        extent: hdr_fb.extent,
        usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        view: Some(depth_buf.view),
        image: Some(depth_buf.image),
        persistent: true,
    });
    graph.set_initial_layout(depth, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let swapchain = graph.add_texture(TextureDesc {
        format: swapchain_format,
        extent: sw_extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        view: Some(swapchain_view),
        image: Some(swapchain_image),
        persistent: true,
    });
    graph.set_initial_layout(swapchain, vk::ImageLayout::PRESENT_SRC_KHR);

    let csm_res: Vec<ResourceId> = if let Some((size, views)) = csm_data {
        views.iter().map(|&(view, image)| {
            let id = graph.add_texture(TextureDesc {
                format: vk::Format::D32_SFLOAT,
                extent: vk::Extent2D { width: size, height: size },
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                view: Some(view),
                image: Some(image),
                persistent: true,
            });
            graph.set_initial_layout(id, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            id
        }).collect()
    } else {
        vec![]
    };

    let point_shadow_res = point_shadow.map(|ps| {
        let id = graph.add_texture(TextureDesc {
            format: vk::Format::D32_SFLOAT,
            extent: vk::Extent2D { width: ps.face_size, height: ps.face_size },
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(ps.cubemap.view),
            image: Some(ps.cubemap.image),
            persistent: true,
        });
        graph.set_initial_layout(id, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        id
    });
    let spot_shadow_res = spot_data.map(|((view, image), size)| {
        let id = graph.add_texture(TextureDesc {
            format: vk::Format::D32_SFLOAT,
            extent: vk::Extent2D { width: size, height: size },
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(view),
            image: Some(image),
            persistent: true,
        });
        graph.set_initial_layout(id, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        id
    });
    let mut sampled = csm_res.clone();
    if let Some(r) = point_shadow_res { sampled.push(r); }
    if let Some(r) = spot_shadow_res { sampled.push(r); }

    // Forward+ light culling compute pass
    if let Some(fwd) = fwd_plus {
        let light_count_gpu = point_lights.len().min(ForwardPlusResources::MAX_LIGHTS) as u32;
        let compute_pipeline = fwd.compute_pipeline;
        let compute_layout = fwd.compute_layout;
        let push_view_proj = view_proj;
        let push_cam_pos = Vec4::new(eye.x, eye.y, eye.z, 0.0);
        let push_screen_size = vk::Extent2D { width: screen_w, height: screen_h };
        let push_tile_count = vk::Extent2D { width: tile_count_x, height: tile_count_y };
        let push_light_count = light_count_gpu;
        let push_max_lights = ForwardPlusResources::MAX_LIGHTS_PER_TILE;

        graph.add_pass(PassDesc {
            name: "light_cull",
            queue: PassQueue::Compute,
            color_attachments: vec![],
            depth_attachment: None,
            sampled_textures: vec![],
            clear_color: false,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |ctx| {
            unsafe {
                let device = ctx.renderer.device().logical();
                let bindless_set = ctx.renderer.bindless_heap().set();
                device.cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::COMPUTE, compute_pipeline);
                device.cmd_bind_descriptor_sets(ctx.cmd, vk::PipelineBindPoint::COMPUTE, compute_layout, 0, &[bindless_set], &[]);
                let mut pc_data = [0u8; 104];
                pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&push_view_proj));
                pc_data[64..80].copy_from_slice(bytemuck::bytes_of(&push_cam_pos));
                pc_data[80..84].copy_from_slice(bytemuck::bytes_of(&push_screen_size.width));
                pc_data[84..88].copy_from_slice(bytemuck::bytes_of(&push_screen_size.height));
                pc_data[88..92].copy_from_slice(bytemuck::bytes_of(&push_tile_count.width));
                pc_data[92..96].copy_from_slice(bytemuck::bytes_of(&push_tile_count.height));
                pc_data[96..100].copy_from_slice(bytemuck::bytes_of(&push_light_count));
                pc_data[100..104].copy_from_slice(bytemuck::bytes_of(&push_max_lights));
                device.cmd_push_constants(ctx.cmd, compute_layout, vk::ShaderStageFlags::COMPUTE, 0, &pc_data);
                device.cmd_dispatch(ctx.cmd, tile_count_x, tile_count_y, 1);
            }
        });
    }

    // Scene pass: only draw meshes (shadows + UBO already handled above)
    graph.add_pass(PassDesc {
        name: "scene",
        queue: rustix_render::graph::PassQueue::Graphics,
        color_attachments: vec![hdr],
        depth_attachment: Some(depth),
        sampled_textures: sampled,
        clear_color: true,
        clear_depth: true,
        clear_value: [0.04, 0.04, 0.08, 1.0],
    }, move |_ctx| {
        let frustum = Frustum::from_view_proj(&view_proj);
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
    });

    // Tonemap pass: writes swapchain, reads HDR
    let sw_w = sw_extent.width;
    let sw_h = sw_extent.height;
    graph.add_pass(PassDesc {
        name: "tonemap",
        queue: rustix_render::graph::PassQueue::Graphics,
        color_attachments: vec![swapchain],
        depth_attachment: None,
        sampled_textures: vec![hdr],
        clear_color: false,
        clear_depth: false,
        clear_value: [0.0; 4],
    }, move |_ctx| {
        renderer.update_tonemap_descriptor_set(tonemap_desc_set, hdr_fb.color_view, sampler);
        unsafe {
            let device = renderer.device().logical();
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, tonemap_pipeline.pipeline);
            device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, tonemap_pipeline.layout, 0, &[tonemap_desc_set], &[]);
            device.cmd_draw(cmd, 3, 1, 0, 0);
        }
    });

    graph.compile();
    let snapshot = graph.snapshot();
    if let Err(e) = graph.allocate_transient_resources(renderer) {
        tracing::warn!("transient resource allocation failed: {e}");
    }
    graph.execute(renderer, cmd);
    // Transient resources are automatically destroyed when graph drops.

    (new_shadow_layout, Some(snapshot))
}
