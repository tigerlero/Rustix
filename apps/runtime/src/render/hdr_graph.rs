use std::collections::HashMap;
use ash::vk;
use rustix_core::ecs::EcsWorld;
use rustix_core::math::{Vec2, Vec3, Vec4, Mat4, Frustum};
use rustix_render::Renderer;
use rustix_render::mesh::Mesh;
use rustix_render::pipeline::GraphicsPipeline;
use rustix_render::DepthBuffer;
use rustix_render::memory::GpuBuffer;
use rustix_render::DirectionalLight;
use crate::camera::EditorCamera;
use crate::scene::Transform;
use super::{CsmResources, PointShadowResources, SpotShadowResources, ForwardPlusResources, collect_lights, directional_light_dir_from_euler};

mod shadows;
mod scene;
mod post;

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
    bloom: Option<&crate::render::BloomResources>,
    bloom_extract_pipeline: Option<&rustix_render::pipeline::BloomPipeline>,
    bloom_down_pipeline: Option<&rustix_render::pipeline::BloomPipeline>,
    bloom_up_pipeline: Option<&rustix_render::pipeline::BloomPipeline>,
    bloom_desc_set: Option<vk::DescriptorSet>,
    bloom_threshold: f32,
    bloom_intensity: f32,
    ssao: Option<&crate::render::SsaoResources>,
    ssao_pipeline: Option<&rustix_render::pipeline::BloomPipeline>,
    ssao_blur_pipeline: Option<&rustix_render::pipeline::BloomPipeline>,
    ssao_desc_set: Option<vk::DescriptorSet>,
    ssao_enabled: bool,
    ssao_radius: f32,
    ssao_bias: f32,
    ssao_power: f32,
    ssao_intensity: f32,
    taa: Option<&crate::render::TaaResources>,
    taa_pipeline: Option<&rustix_render::pipeline::TaaPipeline>,
    taa_desc_set: Option<vk::DescriptorSet>,
    taa_enabled: bool,
    taa_blend_factor: f32,
    prev_view_proj: &mut Option<Mat4>,
    ssr: Option<&crate::render::SsrResources>,
    ssr_pipeline: Option<&rustix_render::pipeline::SsrPipeline>,
    ssr_desc_set: Option<vk::DescriptorSet>,
    ssr_enabled: bool,
    ssr_max_steps: f32,
    ssr_stride: f32,
    ssr_max_dist: f32,
    gbuffer: Option<&crate::render::GBufferResources>,
    fog: Option<&crate::render::VolumetricFogResources>,
    fog_pipeline: Option<&rustix_render::pipeline::VolumetricFogPipeline>,
    fog_desc_set: Option<vk::DescriptorSet>,
    fog_enabled: bool,
    fog_density: f32,
    fog_scattering: f32,
    fog_height_falloff: f32,
    fog_max_dist: f32,
    fog_max_steps: f32,
    fog_sun_intensity: f32,
    skybox: Option<&crate::render::SkyboxResources>,
    skybox_pipeline: Option<&rustix_render::pipeline::SkyboxPipeline>,
    skybox_desc_set: Option<vk::DescriptorSet>,
    skybox_enabled: bool,
    skybox_rayleigh: f32,
    skybox_mie: f32,
    skybox_zenith_shift: f32,
    skybox_exposure: f32,
    instanced_pipeline: Option<&rustix_render::pipeline::InstancedGraphicsPipeline>,
    instanced_batcher: Option<&crate::render::InstancedMeshBatcher>,
    instanced_enabled: bool,
    gpu_culling: Option<&crate::render::GpuCullingResources>,
    gpu_culling_enabled: bool,
    mesh_shader_pipeline: Option<&rustix_render::pipeline::MeshShaderPipeline>,
    mesh_shader_enabled: bool,
    oit_resources: Option<&crate::render::OitResources>,
    oit_enabled: bool,
    oit_accumulate_pipeline: Option<&rustix_render::pipeline::OitAccumulatePipeline>,
    oit_composite_pipeline: Option<&rustix_render::pipeline::OitCompositePipeline>,
    oit_desc_set: Option<vk::DescriptorSet>,
) -> (Option<vk::ImageLayout>, Option<rustix_render::graph::FrameGraphSnapshot>, Mat4) {
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

    let new_shadow_layout = shadows::render_shadow_passes(
        renderer, cmd, ecs_world, meshes,
        shadow_pipeline, csm, point_shadow, spot_shadow,
        shadow_layout,
    );

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

    let bloom_enabled = bloom.is_some() && bloom_extract_pipeline.is_some() && bloom_down_pipeline.is_some() && bloom_up_pipeline.is_some() && bloom_desc_set.is_some();
    let bloom_res = bloom.map(|b| {
        let mip0a = graph.add_texture(TextureDesc {
            format: b.format,
            extent: b.extent0,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(b.mip0a_view),
            image: Some(b.mip0a_image),
            persistent: true,
        });
        let mip1a = graph.add_texture(TextureDesc {
            format: b.format,
            extent: vk::Extent2D { width: (b.extent0.width / 2).max(1), height: (b.extent0.height / 2).max(1) },
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(b.mip1a_view),
            image: Some(b.mip1a_image),
            persistent: true,
        });
        let mip2a = graph.add_texture(TextureDesc {
            format: b.format,
            extent: vk::Extent2D { width: (b.extent0.width / 4).max(1), height: (b.extent0.height / 4).max(1) },
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(b.mip2a_view),
            image: Some(b.mip2a_image),
            persistent: true,
        });
        let mip3 = graph.add_texture(TextureDesc {
            format: b.format,
            extent: vk::Extent2D { width: (b.extent0.width / 8).max(1), height: (b.extent0.height / 8).max(1) },
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(b.mip3_view),
            image: Some(b.mip3_image),
            persistent: true,
        });
        let mip2b = graph.add_texture(TextureDesc {
            format: b.format,
            extent: vk::Extent2D { width: (b.extent0.width / 4).max(1), height: (b.extent0.height / 4).max(1) },
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(b.mip2b_view),
            image: Some(b.mip2b_image),
            persistent: true,
        });
        let mip1b = graph.add_texture(TextureDesc {
            format: b.format,
            extent: vk::Extent2D { width: (b.extent0.width / 2).max(1), height: (b.extent0.height / 2).max(1) },
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(b.mip1b_view),
            image: Some(b.mip1b_image),
            persistent: true,
        });
        let mip0b = graph.add_texture(TextureDesc {
            format: b.format,
            extent: b.extent0,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(b.mip0b_view),
            image: Some(b.mip0b_image),
            persistent: true,
        });
        (mip0a, mip1a, mip2a, mip3, mip2b, mip1b, mip0b)
    });

    let ssao_enabled = ssao_enabled && ssao.is_some() && ssao_pipeline.is_some() && ssao_blur_pipeline.is_some() && ssao_desc_set.is_some();
    let ssao_res = ssao.map(|s| {
        let ao = graph.add_texture(TextureDesc {
            format: s.format,
            extent: s.extent,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(s.ao_view),
            image: Some(s.ao_image),
            persistent: true,
        });
        let blurred = graph.add_texture(TextureDesc {
            format: s.format,
            extent: s.extent,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(s.blurred_ao_view),
            image: Some(s.blurred_ao_image),
            persistent: true,
        });
        (ao, blurred)
    });

    let taa_enabled = taa_enabled && taa.is_some() && taa_pipeline.is_some() && taa_desc_set.is_some();
    let taa_res = taa.map(|t| {
        let history = graph.add_texture(TextureDesc {
            format: t.format,
            extent: t.extent,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            view: Some(t.history_view),
            image: Some(t.history_image),
            persistent: true,
        });
        graph.set_initial_layout(history, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let resolved = graph.add_texture(TextureDesc {
            format: t.format,
            extent: t.extent,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC,
            view: Some(t.resolved_view),
            image: Some(t.resolved_image),
            persistent: true,
        });
        (history, resolved)
    });

    let ssr_tex = ssr.map(|s| graph.add_texture(TextureDesc {
        format: s.format,
        extent: s.extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: Some(s.ssr_view),
        image: Some(s.ssr_image),
        persistent: true,
    }));

    let fog_tex = fog.map(|f| graph.add_texture(TextureDesc {
        format: f.format,
        extent: f.extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: Some(f.fog_view),
        image: Some(f.fog_image),
        persistent: true,
    }));

    let skybox_tex = skybox.map(|s| graph.add_texture(TextureDesc {
        format: s.format,
        extent: s.extent,
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: Some(s.skybox_view),
        image: Some(s.skybox_image),
        persistent: true,
    }));

    let oit_active = oit_enabled && oit_resources.is_some() && oit_accumulate_pipeline.is_some() && oit_composite_pipeline.is_some() && oit_desc_set.is_some();
    let oit_tex = oit_resources.map(|o| {
        let accum = graph.add_texture(TextureDesc {
            format: vk::Format::R16G16B16A16_SFLOAT,
            extent: o.extent,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(o.accum_view),
            image: Some(o.accum_image),
            persistent: true,
        });
        let reveal = graph.add_texture(TextureDesc {
            format: vk::Format::R16_SFLOAT,
            extent: o.extent,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(o.reveal_view),
            image: Some(o.reveal_image),
            persistent: true,
        });
        let composite = graph.add_texture(TextureDesc {
            format: vk::Format::R16G16B16A16_SFLOAT,
            extent: o.extent,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(o.composite_view),
            image: Some(o.composite_image),
            persistent: true,
        });
        (accum, reveal, composite)
    });

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
            scene::execute_light_cull(ctx, view_proj, eye, screen_w, screen_h, tile_count_x, tile_count_y, light_count_gpu, fwd);
        });
    }

    // Scene pass: only draw meshes (shadows + UBO already handled above)
    let use_instanced = instanced_enabled && instanced_pipeline.is_some() && instanced_batcher.is_some();

    // GPU-driven culling compute passes
    let use_gpu_cull = gpu_culling_enabled && gpu_culling.is_some() && use_instanced;
    if use_gpu_cull {
        let cull_res = gpu_culling.unwrap();
        let cull_pipe = cull_res.cull_pipeline;
        let cull_layout = cull_res.cull_layout;
        let cull_set = cull_res.cull_desc_set;
        let gen_pipe = cull_res.gen_pipeline;
        let gen_layout = cull_res.gen_layout;
        let gen_set = cull_res.gen_desc_set;
        let batch_count = instanced_batcher.unwrap().batches.len() as u32;
        let instance_count = instanced_batcher.unwrap().instance_buffer.capacity as u32;
        let frustum = Frustum::from_view_proj(&view_proj);
        let cull_planes: [Vec4; 6] = [
            Vec4::new(frustum.planes[0].normal.x, frustum.planes[0].normal.y, frustum.planes[0].normal.z, frustum.planes[0].d),
            Vec4::new(frustum.planes[1].normal.x, frustum.planes[1].normal.y, frustum.planes[1].normal.z, frustum.planes[1].d),
            Vec4::new(frustum.planes[2].normal.x, frustum.planes[2].normal.y, frustum.planes[2].normal.z, frustum.planes[2].d),
            Vec4::new(frustum.planes[3].normal.x, frustum.planes[3].normal.y, frustum.planes[3].normal.z, frustum.planes[3].d),
            Vec4::new(frustum.planes[4].normal.x, frustum.planes[4].normal.y, frustum.planes[4].normal.z, frustum.planes[4].d),
            Vec4::new(frustum.planes[5].normal.x, frustum.planes[5].normal.y, frustum.planes[5].normal.z, frustum.planes[5].d),
        ];
        let cull_pc = crate::render::CullPushConstants {
            view_proj,
            frustum_planes: cull_planes,
            instance_count,
            batch_count,
            _pad: [0; 2],
        };
        let gen_pc = crate::render::GenDrawPushConstants {
            batch_count,
            _pad: [0; 3],
        };

        // Culling compute pass
        graph.add_pass(PassDesc {
            name: "gpu_cull",
            queue: PassQueue::Compute,
            color_attachments: vec![],
            depth_attachment: None,
            sampled_textures: vec![],
            clear_color: false,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |ctx| {
            scene::execute_gpu_cull(ctx, cull_pipe, cull_layout, cull_set, instance_count, &cull_pc);
        });

        // Command generation compute pass
        graph.add_pass(PassDesc {
            name: "gen_draw_cmds",
            queue: PassQueue::Compute,
            color_attachments: vec![],
            depth_attachment: None,
            sampled_textures: vec![],
            clear_color: false,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |ctx| {
            scene::execute_gen_draw_cmds(ctx, gen_pipe, gen_layout, gen_set, batch_count, &gen_pc);
        });
    }

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
        scene::execute_scene_pass(
            cmd, renderer, view_proj, light_dir, light_color,
            ecs_world, meshes, mesh_shader_enabled, mesh_shader_pipeline,
            gpu_culling_enabled, gpu_culling, instanced_enabled,
            instanced_pipeline, instanced_batcher, scene_pipeline,
        );
    });

    // OIT accumulate pass - renders transparent geometry
    if oit_active {
        let (accum_res, reveal_res, _) = oit_tex.unwrap();
        let oit_accum_pipe = oit_accumulate_pipeline.unwrap();
        graph.add_pass(PassDesc {
            name: "oit_accumulate",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![accum_res, reveal_res],
            depth_attachment: Some(depth),
            sampled_textures: vec![],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            scene::execute_oit_accumulate(
                cmd, renderer, view_proj, light_dir, light_color,
                ecs_world, meshes, oit_accum_pipe,
            );
        });

        // OIT composite pass - blend transparent over opaque
        let (_, _, composite_res) = oit_tex.unwrap();
        let oit_comp_pipe = oit_composite_pipeline.unwrap();
        let ods = oit_desc_set.unwrap();
        graph.add_pass(PassDesc {
            name: "oit_composite",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![composite_res],
            depth_attachment: None,
            sampled_textures: vec![accum_res, reveal_res, hdr],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            scene::execute_oit_composite(
                cmd, renderer, oit_comp_pipe, ods,
                oit_resources.unwrap(), hdr_fb,
            );
        });
    }

    // Choose source color texture for post-effects (OIT composite when active, otherwise raw HDR)
    let scene_color = if oit_active { oit_tex.unwrap().2 } else { hdr };
    let scene_color_view = if oit_active { oit_resources.unwrap().composite_view } else { hdr_fb.color_view };

    // Volumetric fog pass
    let fog_active = fog_enabled && fog.is_some() && fog_pipeline.is_some() && fog_desc_set.is_some();
    if fog_active {
        let fog_tex_res = fog_tex.unwrap();
        let fds = fog_desc_set.unwrap();
        let fog_pipe = fog_pipeline.unwrap();
        graph.add_pass(PassDesc {
            name: "volumetric_fog",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![fog_tex_res],
            depth_attachment: None,
            sampled_textures: vec![depth, scene_color],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_fog_pass(
                cmd, renderer, fog_pipe, fds, depth_buf, scene_color_view,
                view_proj.inverse(), eye, fog_max_steps, fog_density,
                fog_scattering, fog_height_falloff, fog_max_dist, fog_sun_intensity,
            );
        });
    }

    // Skybox pass
    let skybox_active = skybox_enabled && skybox.is_some() && skybox_pipeline.is_some() && skybox_desc_set.is_some();
    if skybox_active {
        let skybox_tex_res = skybox_tex.unwrap();
        let sds = skybox_desc_set.unwrap();
        let sb_pipe = skybox_pipeline.unwrap();
        let inv_vp = view_proj.inverse();
        let pc_data: [[f32; 4]; 4] = [
            [inv_vp.x_axis.x, inv_vp.x_axis.y, inv_vp.x_axis.z, inv_vp.x_axis.w],
            [inv_vp.y_axis.x, inv_vp.y_axis.y, inv_vp.y_axis.z, inv_vp.y_axis.w],
            [inv_vp.z_axis.x, inv_vp.z_axis.y, inv_vp.z_axis.z, inv_vp.z_axis.w],
            [inv_vp.w_axis.x, inv_vp.w_axis.y, inv_vp.w_axis.z, inv_vp.w_axis.w],
        ];
        let sun_dir = Vec3::new(0.3, -1.0, 0.2).normalize();
        let mut pc_data2 = pc_data;
        pc_data2[0] = [sun_dir.x, sun_dir.y, sun_dir.z, 1.0];
        pc_data2[1] = [skybox_rayleigh, skybox_mie, skybox_zenith_shift, skybox_exposure];
        graph.add_pass(PassDesc {
            name: "skybox",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![skybox_tex_res],
            depth_attachment: None,
            sampled_textures: vec![depth, scene_color],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_skybox_pass(
                cmd, renderer, sb_pipe, sds, depth_buf, scene_color_view,
                view_proj.inverse(), skybox_rayleigh, skybox_mie,
                skybox_zenith_shift, skybox_exposure,
            );
        });
    }

    // SSR pass
    let ssr_active = ssr_enabled && ssr.is_some() && ssr_pipeline.is_some() && ssr_desc_set.is_some() && gbuffer.is_some();
    if ssr_active {
        let ssr_tex_res = ssr_tex.unwrap();
        let sds = ssr_desc_set.unwrap();
        let ssr_pipe = ssr_pipeline.unwrap();
        let gbuf = gbuffer.unwrap();
        let ssr_color_tex = if skybox_active { skybox_tex.unwrap() } else if fog_active { fog_tex.unwrap() } else { scene_color };
        let ssr_color_view = if skybox_active { skybox.unwrap().skybox_view } else if fog_active { fog.unwrap().fog_view } else { scene_color_view };
        graph.add_pass(PassDesc {
            name: "ssr",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![ssr_tex_res],
            depth_attachment: None,
            sampled_textures: vec![depth, ssr_color_tex],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_ssr_pass(
                cmd, renderer, ssr_pipe, sds, depth_buf, ssr_color_view,
                gbuf, view_proj.inverse(), eye, ssr_max_steps, ssr_stride,
                ssr_max_dist, hdr_fb.extent,
            );
        });
    }

    // TAA resolve pass
    if taa_enabled {
        let (history_tex, resolved_tex) = taa_res.unwrap();
        let tds = taa_desc_set.unwrap();
        let taa_pipe = taa_pipeline.unwrap();
        let prev_vp = (*prev_view_proj).unwrap_or(view_proj);
        let blend = if (*prev_view_proj).is_none() { 0.0f32 } else { taa_blend_factor };

        let taa_source_tex = if skybox_active { skybox_tex.unwrap() } else if ssr_active { ssr_tex.unwrap() } else if fog_active { fog_tex.unwrap() } else { scene_color };
        let taa_source_view = if skybox_active { skybox.unwrap().skybox_view } else if ssr_active { ssr.unwrap().ssr_view } else if fog_active { fog.unwrap().fog_view } else { scene_color_view };
        graph.add_pass(PassDesc {
            name: "taa_resolve",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![resolved_tex],
            depth_attachment: None,
            sampled_textures: vec![taa_source_tex, history_tex, depth],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_taa_pass(
                cmd, renderer, taa_pipe, tds, depth_buf, taa_source_view,
                taa.unwrap(), view_proj.inverse(),
                prev_vp,
                blend,
                hdr_fb.extent,
            );
        });
    }

    // SSAO passes
    if ssao_enabled {
        let (ao_tex, blurred_tex) = ssao_res.unwrap();
        let sds = ssao_desc_set.unwrap();
        let ssao_pipe = ssao_pipeline.unwrap();
        let blur_pipe = ssao_blur_pipeline.unwrap();

        graph.add_pass(PassDesc {
            name: "ssao_generate",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![ao_tex],
            depth_attachment: None,
            sampled_textures: vec![depth],
            clear_color: true,
            clear_depth: false,
            clear_value: [1.0; 4],
        }, move |_ctx| {
            post::execute_ssao_generate(
                cmd, renderer, ssao_pipe, sds, depth_buf,
                hdr_fb.extent, ssao_radius, ssao_bias,
                ssao_power, ssao_intensity,
            );
        });

        graph.add_pass(PassDesc {
            name: "ssao_blur",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![blurred_tex],
            depth_attachment: None,
            sampled_textures: vec![ao_tex],
            clear_color: true,
            clear_depth: false,
            clear_value: [1.0; 4],
        }, move |_ctx| {
            post::execute_ssao_blur(cmd, renderer, blur_pipe, sds, ssao.unwrap());
        });
    }

    // Bloom passes
    if bloom_enabled {
        let (m0a, m1a, m2a, m3, m2b, m1b, m0b) = bloom_res.unwrap();
        let bds = bloom_desc_set.unwrap();
        let extract_pipe = bloom_extract_pipeline.unwrap();
        let down_pipe = bloom_down_pipeline.unwrap();
        let up_pipe = bloom_up_pipeline.unwrap();

        // Extract
        let bloom_source = if taa_enabled { taa_res.unwrap().1 } else if skybox_active { skybox_tex.unwrap() } else if ssr_active { ssr_tex.unwrap() } else if fog_active { fog_tex.unwrap() } else { scene_color };
        let bloom_source_view = if taa_enabled { taa.unwrap().resolved_view } else if skybox_active { skybox.unwrap().skybox_view } else if ssr_active { ssr.unwrap().ssr_view } else if fog_active { fog.unwrap().fog_view } else { scene_color_view };
        graph.add_pass(PassDesc {
            name: "bloom_extract",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![m0a],
            depth_attachment: None,
            sampled_textures: vec![bloom_source],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_bloom_extract(
                cmd, renderer, extract_pipe, bds, bloom_source_view,
                bloom_threshold, bloom_intensity,
            );
        });

        // Downsample chain
        let e0 = hdr_fb.extent;
        let e1 = vk::Extent2D { width: (e0.width / 2).max(1), height: (e0.height / 2).max(1) };
        let e2 = vk::Extent2D { width: (e1.width / 2).max(1), height: (e1.height / 2).max(1) };
        let e3 = vk::Extent2D { width: (e2.width / 2).max(1), height: (e2.height / 2).max(1) };

        graph.add_pass(PassDesc {
            name: "bloom_down_0",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![m1a],
            depth_attachment: None,
            sampled_textures: vec![m0a],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_bloom_down(
                cmd, renderer, down_pipe, bds, bloom.unwrap().mip0a_view,
                1.0 / e0.width as f32, 1.0 / e0.height as f32,
            );
        });

        graph.add_pass(PassDesc {
            name: "bloom_down_1",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![m2a],
            depth_attachment: None,
            sampled_textures: vec![m1a],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_bloom_down(
                cmd, renderer, down_pipe, bds, bloom.unwrap().mip1a_view,
                1.0 / e1.width as f32, 1.0 / e1.height as f32,
            );
        });

        graph.add_pass(PassDesc {
            name: "bloom_down_2",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![m3],
            depth_attachment: None,
            sampled_textures: vec![m2a],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_bloom_down(
                cmd, renderer, down_pipe, bds, bloom.unwrap().mip2a_view,
                1.0 / e2.width as f32, 1.0 / e2.height as f32,
            );
        });

        // Upsample chain
        graph.add_pass(PassDesc {
            name: "bloom_up_2",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![m2b],
            depth_attachment: None,
            sampled_textures: vec![m3],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_bloom_up(
                cmd, renderer, up_pipe, bds, bloom.unwrap().mip3_view,
                1.0 / e3.width as f32, 1.0 / e3.height as f32,
            );
        });

        graph.add_pass(PassDesc {
            name: "bloom_up_1",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![m1b],
            depth_attachment: None,
            sampled_textures: vec![m2b],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_bloom_up(
                cmd, renderer, up_pipe, bds, bloom.unwrap().mip2b_view,
                1.0 / e2.width as f32, 1.0 / e2.height as f32,
            );
        });

        graph.add_pass(PassDesc {
            name: "bloom_up_0",
            queue: rustix_render::graph::PassQueue::Graphics,
            color_attachments: vec![m0b],
            depth_attachment: None,
            sampled_textures: vec![m1b],
            clear_color: true,
            clear_depth: false,
            clear_value: [0.0; 4],
        }, move |_ctx| {
            post::execute_bloom_up(
                cmd, renderer, up_pipe, bds, bloom.unwrap().mip1b_view,
                1.0 / e1.width as f32, 1.0 / e1.height as f32,
            );
        });
    }

    // Tonemap pass: writes swapchain, reads resolved HDR + bloom + SSAO
    let tonemap_hdr = if taa_enabled { taa_res.unwrap().1 } else if skybox_active { skybox_tex.unwrap() } else if ssr_active { ssr_tex.unwrap() } else if fog_active { fog_tex.unwrap() } else { scene_color };
    let tonemap_hdr_view = if taa_enabled { taa.unwrap().resolved_view } else if skybox_active { skybox.unwrap().skybox_view } else if ssr_active { ssr.unwrap().ssr_view } else if fog_active { fog.unwrap().fog_view } else { scene_color_view };
    let mut tonemap_sampled = vec![tonemap_hdr];
    if bloom_enabled {
        tonemap_sampled.push(bloom_res.unwrap().6);
    }
    if ssao_enabled {
        tonemap_sampled.push(ssao_res.unwrap().1);
    }
    graph.add_pass(PassDesc {
        name: "tonemap",
        queue: rustix_render::graph::PassQueue::Graphics,
        color_attachments: vec![swapchain],
        depth_attachment: None,
        sampled_textures: tonemap_sampled,
        clear_color: false,
        clear_depth: false,
        clear_value: [0.0; 4],
    }, move |_ctx| {
        post::execute_tonemap(
            cmd, renderer, tonemap_pipeline, tonemap_desc_set,
            tonemap_hdr_view, bloom, ssao, sampler,
        );
    });

    graph.compile();
    let snapshot = graph.snapshot();
    if let Err(e) = graph.allocate_transient_resources(renderer) {
        tracing::warn!("transient resource allocation failed: {e}");
    }
    graph.execute(renderer, cmd);
    // Transient resources are automatically destroyed when graph drops.

    let current_view_proj = view_proj;
    *prev_view_proj = Some(current_view_proj);

    (new_shadow_layout, Some(snapshot), current_view_proj)
}
