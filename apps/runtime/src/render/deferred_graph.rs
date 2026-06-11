use std::collections::HashMap;
use ash::vk;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4};
use rustix_render::Renderer;
use rustix_render::mesh::Mesh;
use rustix_render::pipeline::GraphicsPipeline;
use rustix_render::memory::GpuBuffer;
use rustix_render::DepthBuffer;
use rustix_render::{PointLight, SpotLight, DirectionalLight};
use crate::camera::EditorCamera;
use crate::scene::{Transform, MeshComponent, Material, world_transform};
use super::{CsmResources, PointShadowResources, SpotShadowResources, ForwardPlusResources, GBufferResources, collect_lights, directional_light_dir_from_euler};

/// Deferred shading render path using frame graph.
/// Replaces the forward scene pass with a GBuffer geometry pass + deferred lighting pass.
pub fn render_deferred_with_graph(
    renderer: &Renderer,
    cmd: vk::CommandBuffer,
    _scene_pipeline: &GraphicsPipeline,
    shadow_pipeline: Option<&rustix_render::pipeline::ShadowPipeline>,
    depth_buf: &DepthBuffer,
    mut csm: Option<&mut CsmResources>,
    point_shadow: Option<&PointShadowResources>,
    mut spot_shadow: Option<&mut SpotShadowResources>,
    shadow_layout: Option<vk::ImageLayout>,
    _ubo: &GpuBuffer,
    meshes: &HashMap<String, Mesh>,
    ecs_world: &EcsWorld,
    cam: &EditorCamera,
    hdr_fb: &rustix_render::HdrFramebuffer,
    tonemap_pipeline: &rustix_render::pipeline::ToneMapPipeline,
    tonemap_desc_set: vk::DescriptorSet,
    sampler: vk::Sampler,
    gbuf: &GBufferResources,
    fwd_plus: Option<&ForwardPlusResources>,
) -> (Option<vk::ImageLayout>, Option<rustix_render::graph::FrameGraphSnapshot>, Mat4) {
    use rustix_render::graph::{FrameGraph, ResourceId, PassDesc, PassQueue, TextureDesc};

    let sw = renderer.swapchain.lock();
    let sw_extent = sw.extent();
    drop(sw);

    let shadow_layout_cell = std::cell::Cell::new(shadow_layout);
    let shadow_layout_cell2 = shadow_layout_cell.clone();

    let mut graph = FrameGraph::new();
    let swapchain_image = renderer.swapchain.lock().current_image();
    let swapchain_format = renderer.swapchain.lock().format();

    // HDR color buffer (deferred lighting output)
    let hdr = graph.add_texture(TextureDesc {
        format: vk::Format::R16G16B16A16_SFLOAT,
        extent: hdr_fb.extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: Some(hdr_fb.color_view),
        image: Some(hdr_fb.color_image),
        persistent: true,
    });
    graph.set_initial_layout(hdr, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    // Swapchain image
    let swapchain_view = renderer.swapchain.lock().current_image_view();
    let swapchain = graph.add_texture(TextureDesc {
        format: swapchain_format,
        extent: sw_extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        view: Some(swapchain_view),
        image: Some(swapchain_image),
        persistent: true,
    });
    graph.set_initial_layout(swapchain, vk::ImageLayout::PRESENT_SRC_KHR);
    // GBuffer albedo+metallic
    let gbuf_albedo = graph.add_texture(TextureDesc {
        format: vk::Format::R8G8B8A8_UNORM,
        extent: gbuf.extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: Some(gbuf.albedo_view),
        image: Some(gbuf.albedo_image),
        persistent: true,
    });
    graph.set_initial_layout(gbuf_albedo, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    // GBuffer normal
    let gbuf_normal = graph.add_texture(TextureDesc {
        format: vk::Format::R16G16B16A16_SFLOAT,
        extent: gbuf.extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: Some(gbuf.normal_view),
        image: Some(gbuf.normal_image),
        persistent: true,
    });
    graph.set_initial_layout(gbuf_normal, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    // GBuffer material
    let gbuf_material = graph.add_texture(TextureDesc {
        format: vk::Format::R8G8B8A8_UNORM,
        extent: gbuf.extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: Some(gbuf.material_view),
        image: Some(gbuf.material_image),
        persistent: true,
    });
    graph.set_initial_layout(gbuf_material, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    // Depth
    let depth = graph.add_texture(TextureDesc {
        format: vk::Format::D32_SFLOAT,
        extent: hdr_fb.extent,
        usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: Some(depth_buf.view),
        image: Some(depth_buf.image),
        persistent: true,
    });
    graph.set_initial_layout(depth, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

    // Compute CSM cascades before frame graph execution
    if let Some(ref mut c) = csm {
        let cam_view = match cam.mode {
            crate::camera::CameraMode::Orbit => Mat4::look_at_rh(cam.eye_pos(), cam.center, Vec3::Y),
            crate::camera::CameraMode::FirstPerson => {
                let forward = Vec3::new(cam.pitch.cos() * cam.yaw.sin(), cam.pitch.sin(), cam.pitch.cos() * cam.yaw.cos());
                Mat4::look_at_rh(cam.position, cam.position + forward, Vec3::Y)
            }
        };
        let aspect = hdr_fb.extent.width as f32 / hdr_fb.extent.height as f32;
        let cam_proj = Mat4::perspective_rh_gl(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);
        let light_dir = {
            let mut d = Vec3::new(0.5, 0.8, 0.3);
            for (_dirlight, xform) in ecs_world.query::<(&DirectionalLight, &Transform)>().iter() {
                d = directional_light_dir_from_euler(xform.rotation);
                break;
            }
            d
        };
        c.compute_cascades(&cam_view, &cam_proj, light_dir);
        c.upload_ubo();
    }

    // Inline point/spot shadow rendering before frame graph (deferred path)
    if let (Some(sp), Some(ps)) = (shadow_pipeline, point_shadow) {
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
                renderer.transition_image_layout(cmd, ps.cubemap.image, vk::ImageAspectFlags::DEPTH, vk::ImageLayout::UNDEFINED, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
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
    }
    if let (Some(sp), Some(ss)) = (shadow_pipeline, spot_shadow.as_mut()) {
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
            renderer.transition_image_layout(cmd, ss.array.image, vk::ImageAspectFlags::DEPTH, vk::ImageLayout::UNDEFINED, vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
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
    }

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
    let spot_shadow_res = spot_shadow.as_ref().map(|ss| {
        let id = graph.add_texture(TextureDesc {
            format: vk::Format::D32_SFLOAT,
            extent: vk::Extent2D { width: ss.size, height: ss.size },
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(ss.array.view),
            image: Some(ss.array.image),
            persistent: true,
        });
        graph.set_initial_layout(id, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        id
    });

    let csm_res: Vec<ResourceId> = if let Some(ref c) = csm {
        c.shadow_maps.iter().map(|sm| {
            let id = graph.add_texture(TextureDesc {
                format: vk::Format::D32_SFLOAT,
                extent: vk::Extent2D { width: c.shadow_map_size, height: c.shadow_map_size },
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                view: Some(sm.view),
                image: Some(sm.image),
                persistent: true,
            });
            graph.set_initial_layout(id, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            id
        }).collect()
    } else {
        vec![]
    };

    // CSM shadow passes — one per cascade
    if let (Some(sp), Some(ref mut c)) = (shadow_pipeline, csm) {
        let meshes_clone = meshes;
        let shadow_layout_cell_ref = shadow_layout_cell.clone();
        let size = c.shadow_map_size;
        let views = [c.shadow_maps[0].view, c.shadow_maps[1].view, c.shadow_maps[2].view];
        let matrices = c.ubo_data.light_view_proj;
        let sp_clone = rustix_render::pipeline::ShadowPipeline {
            pipeline: sp.pipeline,
            layout: sp.layout,
            descriptor_set_layout: sp.descriptor_set_layout,
        };
        for cascade_idx in 0..3 {
            let csm_res_id = csm_res[cascade_idx];
            let sm_view = views[cascade_idx];
            let light_matrix = matrices[cascade_idx];
            let meshes_c = meshes_clone;
            let sp_c = sp_clone.clone();
            let sl_ref = shadow_layout_cell_ref.clone();
            let cascade_names = ["shadow0", "shadow1", "shadow2"];
            graph.add_pass(PassDesc {
                name: cascade_names[cascade_idx],
                queue: PassQueue::Graphics,
                color_attachments: vec![],
                depth_attachment: Some(csm_res_id),
                sampled_textures: vec![],
                clear_color: false,
                clear_depth: true,
                clear_value: [0.0; 4],
            }, move |ctx| {
                ctx.renderer.begin_shadow_pass(ctx.cmd, sm_view, size);
                for (_, shape) in meshes_c.iter() {
                    let model = Mat4::IDENTITY;
                    let mut pc = [0u8; 128];
                    pc[0..64].copy_from_slice(bytemuck::bytes_of(&model));
                    pc[64..128].copy_from_slice(bytemuck::bytes_of(&light_matrix));
                    ctx.renderer.draw_shadow_in_pass(ctx.cmd, &sp_c, &shape.vertex_buffer, shape.index_buffer.as_ref(), shape.index_count as u32, &pc);
                }
                ctx.renderer.end_shadow_pass(ctx.cmd);
                sl_ref.set(Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL));
            });
        }
    }

    let aspect = hdr_fb.extent.width as f32 / hdr_fb.extent.height as f32;
    let view_proj = cam.view_proj(aspect);
    let eye = cam.eye_pos();
    let point_lights = collect_lights(ecs_world);
    let screen_w = hdr_fb.extent.width;
    let screen_h = hdr_fb.extent.height;
    let tile_count_x = (screen_w + ForwardPlusResources::TILE_SIZE - 1) / ForwardPlusResources::TILE_SIZE;
    let tile_count_y = (screen_h + ForwardPlusResources::TILE_SIZE - 1) / ForwardPlusResources::TILE_SIZE;

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

    // GBuffer geometry pass
    let gbuffer_pipeline = &gbuf.gbuffer_pipeline;
    let gbuffer_pipeline_clone = gbuffer_pipeline;

    graph.add_pass(PassDesc {
        name: "gbuffer",
        queue: PassQueue::Graphics,
        color_attachments: vec![gbuf_albedo, gbuf_normal, gbuf_material],
        depth_attachment: Some(depth),
        sampled_textures: csm_res.clone(),
        clear_color: true,
        clear_depth: true,
        clear_value: [0.0, 0.0, 0.0, 1.0],
    }, move |ctx| {
        let bindless_set = ctx.renderer.bindless_heap().set();
        unsafe {
            let device = ctx.renderer.device().logical();
            device.cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::GRAPHICS, gbuffer_pipeline_clone.pipeline);
            device.cmd_bind_descriptor_sets(ctx.cmd, vk::PipelineBindPoint::GRAPHICS, gbuffer_pipeline_clone.layout, 0, &[bindless_set], &[]);
        }
        let mesh_entities: Vec<(hecs::Entity, String)> = ecs_world.query::<(Entity, &MeshComponent)>().iter()
            .map(|(e, mc)| (e, mc.0.clone())).collect();
        for (entity, mesh_name) in mesh_entities {
            if let Some(shape) = meshes.get(&mesh_name) {
                let model = world_transform(ecs_world, entity);
                let mat = ecs_world.get::<&Material>(entity).ok();
                let base_color = mat.as_ref().map(|m| Vec4::new(m.base_color.x, m.base_color.y, m.base_color.z, 1.0))
                    .unwrap_or(Vec4::new(0.7, 0.7, 0.7, 1.0));
                let material = mat.as_ref().map(|m| Vec4::new(m.roughness, m.metallic, m.ao, m.emissive))
                    .unwrap_or(Vec4::new(0.5, 0.0, 1.0, 0.0));
                let mut pc = [0u8; 128];
                pc[0..64].copy_from_slice(bytemuck::bytes_of(&model));
                pc[64..80].copy_from_slice(bytemuck::bytes_of(&Vec4::new(0.3, -1.0, 0.2, 1.0).normalize()));
                pc[80..96].copy_from_slice(bytemuck::bytes_of(&Vec4::new(0.9, 0.85, 0.8, 1.0)));
                pc[96..112].copy_from_slice(bytemuck::bytes_of(&base_color));
                pc[112..128].copy_from_slice(bytemuck::bytes_of(&material));
                unsafe {
                    ctx.renderer.device().logical().cmd_push_constants(
                        ctx.cmd, gbuffer_pipeline_clone.layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0, &pc,
                    );
                    ctx.renderer.device().logical().cmd_bind_vertex_buffers(ctx.cmd, 0, &[shape.vertex_buffer.buffer], &[0u64]);
                    if let Some(ref ib) = shape.index_buffer {
                        ctx.renderer.device().logical().cmd_bind_index_buffer(ctx.cmd, ib.buffer, 0, vk::IndexType::UINT16);
                        ctx.renderer.device().logical().cmd_draw_indexed(ctx.cmd, shape.index_count as u32, 1, 0, 0, 0);
                    } else {
                        ctx.renderer.device().logical().cmd_draw(ctx.cmd, shape.vertex_count as u32, 1, 0, 0);
                    }
                }
            }
        }
    });

    // Deferred lighting pass
    let deferred_pipeline = &gbuf.deferred_pipeline;
    let deferred_pipeline_ref = deferred_pipeline;
    let inv_view_proj = view_proj.inverse();
    let light_count = point_lights.len().min(ForwardPlusResources::MAX_LIGHTS) as u32;
    let max_lights_per_tile = ForwardPlusResources::MAX_LIGHTS_PER_TILE;
    let dir_light_dir = Vec3::new(0.3, -1.0, 0.2).normalize();
    let dir_color = Vec3::new(0.9, 0.85, 0.8);

    graph.add_pass(PassDesc {
        name: "deferred",
        queue: PassQueue::Graphics,
        color_attachments: vec![hdr],
        depth_attachment: None,
        sampled_textures: {
            let mut v = vec![gbuf_albedo, gbuf_normal, gbuf_material, depth, swapchain];
            if let Some(r) = point_shadow_res { v.push(r); }
            if let Some(r) = spot_shadow_res { v.push(r); }
            v
        },
        clear_color: true,
        clear_depth: false,
        clear_value: [0.04, 0.04, 0.08, 1.0],
    }, move |ctx| {
        let bindless_set = ctx.renderer.bindless_heap().set();
        unsafe {
            let device = ctx.renderer.device().logical();
            device.cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::GRAPHICS, deferred_pipeline_ref.pipeline);
            device.cmd_bind_descriptor_sets(ctx.cmd, vk::PipelineBindPoint::GRAPHICS, deferred_pipeline_ref.layout, 0, &[bindless_set], &[]);
            let mut pc = [0u8; 128];
            pc[0..64].copy_from_slice(bytemuck::bytes_of(&inv_view_proj));
            pc[64..80].copy_from_slice(bytemuck::bytes_of(&Vec4::new(eye.x, eye.y, eye.z, 0.0)));
            pc[80..96].copy_from_slice(bytemuck::bytes_of(&Vec4::new(dir_light_dir.x, dir_light_dir.y, dir_light_dir.z, 1.0)));
            pc[96..112].copy_from_slice(bytemuck::bytes_of(&Vec4::new(dir_color.x, dir_color.y, dir_color.z, 1.0)));
            pc[112..116].copy_from_slice(bytemuck::bytes_of(&light_count));
            pc[116..120].copy_from_slice(bytemuck::bytes_of(&max_lights_per_tile));
            device.cmd_push_constants(ctx.cmd, deferred_pipeline_ref.layout, vk::ShaderStageFlags::FRAGMENT, 0, &pc);
            device.cmd_draw(ctx.cmd, 3, 1, 0, 0);
        }
    });

    // Tonemap pass
    let _sw_w = sw_extent.width;
    let _sw_h = sw_extent.height;
    let post_settings: rustix_render::PostProcessSettings = {
        let mut settings = rustix_render::PostProcessSettings::default();
        for s in ecs_world.query::<&rustix_render::PostProcessSettings>().iter() {
            settings = *s;
            break;
        }
        settings
    };

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
        renderer.update_tonemap_descriptor_set(tonemap_desc_set, hdr_fb.color_view, hdr_fb.color_view, hdr_fb.color_view, sampler);
        let pc_data: [f32; 20] = [
            post_settings.grain_intensity,
            post_settings.chromatic_aberration,
            post_settings.vignette_intensity,
            post_settings.vignette_smoothness,
            post_settings.contrast,
            post_settings.saturation,
            post_settings.gamma,
            0.0,
            post_settings.tint_shadows[0], post_settings.tint_shadows[1], post_settings.tint_shadows[2], post_settings.tint_shadows[3],
            post_settings.tint_midtones[0], post_settings.tint_midtones[1], post_settings.tint_midtones[2], post_settings.tint_midtones[3],
            post_settings.tint_highlights[0], post_settings.tint_highlights[1], post_settings.tint_highlights[2], post_settings.tint_highlights[3],
        ];
        unsafe {
            let device = renderer.device().logical();
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, tonemap_pipeline.pipeline);
            device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, tonemap_pipeline.layout, 0, &[tonemap_desc_set], &[]);
            device.cmd_push_constants(cmd, tonemap_pipeline.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc_data));
            device.cmd_draw(cmd, 3, 1, 0, 0);
        }
    });

    graph.compile();
    let snapshot = graph.snapshot();
    if let Err(e) = graph.allocate_transient_resources(renderer) {
        tracing::warn!("transient resource allocation failed: {e}");
    }
    graph.execute(renderer, cmd);

    (shadow_layout_cell2.get(), Some(snapshot), view_proj)
}
