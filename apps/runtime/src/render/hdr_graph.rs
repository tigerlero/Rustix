use std::collections::HashMap;
use ash::vk;
use rustix_core::ecs::EcsWorld;
use rustix_core::math::{Vec3, Vec4, Mat4, Frustum};
use rustix_render::Renderer;
use rustix_render::mesh::Mesh;
use rustix_render::pipeline::GraphicsPipeline;
use rustix_render::DepthBuffer;
use rustix_render::memory::GpuBuffer;
use crate::camera::EditorCamera;
use super::{CsmResources, PointShadowResources, SpotShadowResources, ForwardPlusResources};

mod shadows;
mod scene;
mod post;
mod setup;
mod resources;

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
    use rustix_render::graph::{FrameGraph, PassDesc, PassQueue};

    // --- Scene setup: UBO, lights, and shadow rendering (must be before frame graph) ---
    let setup = setup::prepare_scene_data(cam, ecs_world, ubo, fwd_plus, &mut csm, spot_shadow.as_deref(), hdr_fb.extent);
    let setup::SceneSetup { view_proj, eye, point_lights, light_dir, light_color, screen_w, screen_h, tile_count_x, tile_count_y, csm_data, spot_data, .. } = setup;

    let new_shadow_layout = shadows::render_shadow_passes(
        renderer, cmd, ecs_world, meshes,
        shadow_pipeline, csm, point_shadow, spot_shadow,
        shadow_layout,
    );

    // --- Frame graph ---
    let mut graph = FrameGraph::new();
    let tex = resources::register_textures(&mut graph, renderer, hdr_fb, depth_buf, bloom, ssao, taa, ssr, fog, skybox, oit_resources, csm_data.as_ref(), point_shadow, spot_data.as_ref());
    let resources::GraphTextures { hdr, depth, swapchain, bloom_res, ssao_res, taa_res, ssr_tex, fog_tex, skybox_tex, oit_tex, csm_res, point_shadow_res, spot_shadow_res } = tex;

    let bloom_enabled = bloom.is_some() && bloom_extract_pipeline.is_some() && bloom_down_pipeline.is_some() && bloom_up_pipeline.is_some() && bloom_desc_set.is_some();
    let ssao_enabled = ssao_enabled && ssao.is_some() && ssao_pipeline.is_some() && ssao_blur_pipeline.is_some() && ssao_desc_set.is_some();
    let taa_enabled = taa_enabled && taa.is_some() && taa_pipeline.is_some() && taa_desc_set.is_some();
    let oit_active = oit_enabled && oit_resources.is_some() && oit_accumulate_pipeline.is_some() && oit_composite_pipeline.is_some() && oit_desc_set.is_some();
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
        let mut _pc_data2 = pc_data;
        _pc_data2[0] = [sun_dir.x, sun_dir.y, sun_dir.z, 1.0];
        _pc_data2[1] = [skybox_rayleigh, skybox_mie, skybox_zenith_shift, skybox_exposure];
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
        sampled_textures: tonemap_sampled,
        clear_color: false,
        clear_depth: false,
        clear_value: [0.0; 4],
    }, move |_ctx| {
        post::execute_tonemap(
            cmd, renderer, tonemap_pipeline, tonemap_desc_set,
            tonemap_hdr_view, bloom, ssao, sampler, &post_settings,
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
