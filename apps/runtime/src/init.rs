use std::collections::HashMap;
use ash::vk;
use rustix_render::{Renderer, mesh::Mesh};
use rustix_terrain::{TerrainParams, generate_heightmap, build_terrain_mesh};

pub fn init_scene_resources(
    renderer: &Renderer,
    meshes: &mut HashMap<String, Mesh>,
    scene_pipeline: &mut Option<rustix_render::pipeline::GraphicsPipeline>,
    _scene_descriptor_pool: &mut Option<vk::DescriptorPool>,
    _scene_descriptor_set: &mut Option<vk::DescriptorSet>,
    scene_uniform_buffer: &mut Option<rustix_render::memory::GpuBuffer>,
    scene_depth_buffer: &mut Option<rustix_render::DepthBuffer>,
    shadow_pipeline: &mut Option<rustix_render::pipeline::ShadowPipeline>,
    _shadow_descriptor_pool: &mut Option<vk::DescriptorPool>,
    _shadow_descriptor_set: &mut Option<vk::DescriptorSet>,
    csm_resources: &mut Option<crate::render::CsmResources>,
    point_shadow_resources: &mut Option<crate::render::PointShadowResources>,
    spot_shadow_resources: &mut Option<crate::render::SpotShadowResources>,
    tonemap_pipeline: &mut Option<rustix_render::pipeline::ToneMapPipeline>,
    tonemap_desc_set: &mut Option<vk::DescriptorSet>,
) {
    if scene_pipeline.is_some() { return; }

    if let Ok(result) = crate::gltf_loader::load_glb(renderer, &crate::gltf_loader::generate_cube_glb(), "Cube") {
        meshes.insert("Cube".into(), result.mesh);
    } else {
        tracing::error!("failed to load default cube mesh");
    }
    if let Ok((sp_verts, sp_idx)) = (|| -> Result<(Vec<rustix_render::mesh::Vertex>, Vec<u16>), rustix_render::RenderError> {
        Ok(rustix_render::mesh::procedural::uv_sphere(0.5, 16, 16))
    })() {
        let vb_slice = bytemuck::cast_slice(&sp_verts);
        if let Ok(sp_mesh) = Mesh::new(renderer, "Sphere", vb_slice, sp_verts.len() as u32, Some((&sp_idx, sp_idx.len() as u32))) {
            meshes.insert("Sphere".into(), sp_mesh);
        }
    }
    if let Ok((t_verts, t_idx)) = (|| -> Result<(Vec<rustix_render::mesh::Vertex>, Vec<u16>), rustix_render::RenderError> {
        Ok(rustix_render::mesh::procedural::torus(0.5, 0.15, 24, 12))
    })() {
        let vb_slice = bytemuck::cast_slice(&t_verts);
        if let Ok(t_mesh) = Mesh::new(renderer, "Torus", vb_slice, t_verts.len() as u32, Some((&t_idx, t_idx.len() as u32))) {
            meshes.insert("Torus".into(), t_mesh);
        }
    }
    if let Ok((c_verts, c_idx)) = (|| -> Result<(Vec<rustix_render::mesh::Vertex>, Vec<u16>), rustix_render::RenderError> {
        Ok(rustix_render::mesh::procedural::capsule(0.3, 0.6, 8, 16))
    })() {
        let vb_slice = bytemuck::cast_slice(&c_verts);
        if let Ok(c_mesh) = Mesh::new(renderer, "Capsule", vb_slice, c_verts.len() as u32, Some((&c_idx, c_idx.len() as u32))) {
            meshes.insert("Capsule".into(), c_mesh);
        }
    }
    if let Ok((ico_verts, ico_idx)) = (|| -> Result<(Vec<rustix_render::mesh::Vertex>, Vec<u16>), rustix_render::RenderError> {
        Ok(rustix_render::mesh::procedural::icosphere(0.5, 2))
    })() {
        let vb_slice = bytemuck::cast_slice(&ico_verts);
        if let Ok(ico_mesh) = Mesh::new(renderer, "Icosphere", vb_slice, ico_verts.len() as u32, Some((&ico_idx, ico_idx.len() as u32))) {
            meshes.insert("Icosphere".into(), ico_mesh);
        }
    }
    if let Ok((p_verts, p_idx)) = (|| -> Result<(Vec<rustix_render::mesh::Vertex>, Vec<u16>), rustix_render::RenderError> {
        Ok(rustix_render::mesh::procedural::quad(1.0, 1))
    })() {
        let vb_slice = bytemuck::cast_slice(&p_verts);
        if let Ok(p_mesh) = Mesh::new(renderer, "Plane", vb_slice, p_verts.len() as u32, Some((&p_idx, p_idx.len() as u32))) {
            meshes.insert("Plane".into(), p_mesh);
        }
    }
    {
        let params = TerrainParams { width: 32, depth: 32, scale: 2.0, height_scale: 4.0, ..Default::default() };
        let hm = generate_heightmap(&params);
        let (t_verts, t_idx) = build_terrain_mesh(&hm, params.scale);
        let mut tr_verts: Vec<rustix_render::mesh::Vertex> = Vec::with_capacity(t_verts.len());
        for v in &t_verts {
            tr_verts.push(rustix_render::mesh::Vertex {
                position: v.position,
                normal: v.normal,
            });
        }
        let vb_slice = bytemuck::cast_slice(&tr_verts);
        if let Ok(t_mesh) = Mesh::new(renderer, "Terrain", vb_slice, tr_verts.len() as u32, Some((&t_idx, t_idx.len() as u32))) {
            meshes.insert("Terrain".into(), t_mesh);
        }
    }

    let bindless_layout = renderer.bindless_heap().layout();
    match (
        rustix_render::shader::builtin::vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            let sw = renderer.swapchain.lock();
            let mut variant_key = rustix_render::pipeline::PipelineVariantKey::default();
            variant_key.spec_constants.set(0, 1);
            match renderer.pipeline_variant_cache().get_or_create(
                &variant_key,
                renderer.device(),
                &sw,
                &vs,
                &fs,
            ) {
                Ok(_pipeline) => {
                    if let Some(gp) = renderer.pipeline_variant_cache().get_pipeline(&variant_key) {
                        *scene_pipeline = Some(gp);
                    }
                }
                Err(e) => tracing::error!("scene pipeline creation failed: {e}"),
            }
            drop(sw);
        }
        (Err(e), _) => tracing::error!("vertex shader compile failed: {e}"),
        (_, Err(e)) => tracing::error!("fragment shader compile failed: {e}"),
    }
    match renderer.create_buffer("scene_ubo", rustix_render::pipeline::UBO_SCENE_SIZE, vk::BufferUsageFlags::UNIFORM_BUFFER, gpu_allocator::MemoryLocation::CpuToGpu) {
        Ok(buf) => *scene_uniform_buffer = Some(buf),
        Err(e) => tracing::error!("scene UBO creation failed: {e}"),
    }

    let sw = renderer.swapchain.lock();
    *scene_depth_buffer = renderer.create_depth_buffer(sw.extent()).ok();
    drop(sw);

    if let Ok(sv) = rustix_render::shader::builtin::shadow_vertex_shader(renderer.device().logical()) {
        match rustix_render::pipeline::ShadowPipeline::create(renderer.device(), &sv, bindless_layout) {
            Ok(p) => {
                *shadow_pipeline = Some(p);
            }
            Err(e) => tracing::error!("shadow pipeline creation failed: {e}"),
        }
    } else {
        tracing::error!("failed to compile shadow vertex shader");
    }
    if csm_resources.is_none() {
        match crate::render::CsmResources::new(renderer, 2048) {
            Ok(csm) => {
                *csm_resources = Some(csm);
            }
            Err(e) => tracing::error!("csm resources creation failed: {e}"),
        }
    }
    if point_shadow_resources.is_none() {
        match crate::render::PointShadowResources::new(renderer, 512, 4) {
            Ok(ps) => {
                *point_shadow_resources = Some(ps);
            }
            Err(e) => tracing::error!("point shadow resources creation failed: {e}"),
        }
    }
    if spot_shadow_resources.is_none() {
        match crate::render::SpotShadowResources::new(renderer, 512, 4) {
            Ok(ss) => {
                *spot_shadow_resources = Some(ss);
            }
            Err(e) => tracing::error!("spot shadow resources creation failed: {e}"),
        }
    }

    // Tone-mapping pipeline (HDR → SDR)
    match (
        rustix_render::shader::builtin::tonemap_vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::tonemap_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            let sw = renderer.swapchain.lock();
            match rustix_render::pipeline::ToneMapPipeline::create(renderer.device(), &sw, &vs, &fs, None) {
                Ok(p) => {
                    match renderer.allocate_descriptor_set(p.desc_layout) {
                        Ok(ds) => *tonemap_desc_set = Some(ds),
                        Err(e) => tracing::error!("tonemap desc set alloc failed: {e}"),
                    }
                    *tonemap_pipeline = Some(p);
                }
                Err(e) => tracing::error!("tone-map pipeline creation failed: {e}"),
            }
            drop(sw);
        }
        (Err(e), _) => tracing::error!("tonemap vertex shader compile failed: {e}"),
        (_, Err(e)) => tracing::error!("tonemap fragment shader compile failed: {e}"),
    }
}

pub fn init_2d_resources(
    renderer: &Renderer,
    pipeline_2d: &mut Option<rustix_render::pipeline::GraphicsPipeline2D>,
    ubo_2d: &mut Option<rustix_render::memory::GpuBuffer>,
    desc_set_2d: &mut Option<vk::DescriptorSet>,
    quad_buffer_2d: &mut Option<rustix_render::memory::GpuBuffer>,
    texture_2d: &mut Option<rustix_render::GpuTexture>,
) {
    if pipeline_2d.is_some() { return; }

    let vs_2d = rustix_render::shader::builtin::vertex_2d_shader(renderer.device().logical());
    let fs_2d = rustix_render::shader::builtin::fragment_2d_shader(renderer.device().logical());
    if let (Ok(vs), Ok(fs)) = (vs_2d, fs_2d) {
        let sw = renderer.swapchain.lock();
        match rustix_render::pipeline::GraphicsPipeline2D::create(renderer.device(), &sw, &vs, &fs) {
            Ok(p) => {
                match renderer.allocate_descriptor_set(p.desc_layout) {
                    Ok(ds) => *desc_set_2d = Some(ds),
                    Err(e) => tracing::error!("2D desc set alloc failed: {e}"),
                }
                *pipeline_2d = Some(p);
            }
            Err(e) => tracing::error!("2D pipeline creation failed: {e}"),
        }
        drop(sw);
    }
    match renderer.create_buffer("ubo_2d", 64, vk::BufferUsageFlags::UNIFORM_BUFFER, gpu_allocator::MemoryLocation::CpuToGpu) {
        Ok(buf) => *ubo_2d = Some(buf),
        Err(e) => tracing::error!("2D UBO creation failed: {e}"),
    }

    let quad: [f32; 32] = [
        -0.5, -0.5,  0.0, 0.0,  1.0, 1.0, 1.0, 1.0,
         0.5, -0.5,  1.0, 0.0,  1.0, 1.0, 1.0, 1.0,
         0.5,  0.5,  1.0, 1.0,  1.0, 1.0, 1.0, 1.0,
        -0.5,  0.5,  0.0, 1.0,  1.0, 1.0, 1.0, 1.0,
    ];
    match renderer.create_buffer("quad_2d", 128, vk::BufferUsageFlags::VERTEX_BUFFER, gpu_allocator::MemoryLocation::CpuToGpu) {
        Ok(buf) => {
            buf.write(bytemuck::bytes_of(&quad));
            *quad_buffer_2d = Some(buf);
        }
        Err(e) => tracing::error!("2D quad buffer creation failed: {e}"),
    }

    let tex_size = 64u32;
    let mut pixels = vec![0u8; (tex_size * tex_size * 4) as usize];
    for y in 0..tex_size {
        for x in 0..tex_size {
            let is_white = (x / 8 + y / 8) % 2 == 0;
            let idx = ((y * tex_size + x) * 4) as usize;
            pixels[idx..idx+4].copy_from_slice(
                if is_white { &[240, 240, 255, 255] } else { &[60, 60, 80, 255] }
            );
        }
    }
    match renderer.create_texture(tex_size, tex_size, &pixels) {
        Ok(tex) => *texture_2d = Some(tex),
        Err(e) => tracing::error!("2D texture creation failed: {e}"),
    }
}

/// Reload scene pipeline shaders from disk overrides (hot-reload).
///
/// Call this when `pbr.vert` or `pbr.frag` changed on disk.
pub fn reload_scene_pipeline(
    renderer: &Renderer,
    scene_pipeline: &mut Option<rustix_render::pipeline::GraphicsPipeline>,
) {
    match (
        rustix_render::shader::builtin::vertex_shader_override(renderer.device().logical()),
        rustix_render::shader::builtin::fragment_shader_override(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            let sw = renderer.swapchain.lock();
            let mut variant_key = rustix_render::pipeline::PipelineVariantKey::default();
            variant_key.spec_constants.set(0, 1);
            renderer.clear_pipeline_cache();
            match renderer.pipeline_variant_cache().get_or_create(
                &variant_key,
                renderer.device(),
                &sw,
                &vs,
                &fs,
            ) {
                Ok(_pipeline) => {
                    if let Some(gp) = renderer.pipeline_variant_cache().get_pipeline(&variant_key) {
                        *scene_pipeline = Some(gp);
                        tracing::info!("scene pipeline hot-reloaded");
                    }
                }
                Err(e) => tracing::error!("scene pipeline reload failed: {e}"),
            }
            drop(sw);
        }
        (Err(e), _) => tracing::error!("vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("fragment shader reload failed: {e}"),
    }
}

/// Reload shadow pipeline shader from disk override (hot-reload).
pub fn reload_shadow_pipeline(
    renderer: &Renderer,
    shadow_pipeline: &mut Option<rustix_render::pipeline::ShadowPipeline>,
    bindless_layout: vk::DescriptorSetLayout,
) {
    match rustix_render::shader::builtin::shadow_vertex_shader_override(renderer.device().logical()) {
        Ok(sv) => {
            match rustix_render::pipeline::ShadowPipeline::create(renderer.device(), &sv, bindless_layout) {
                Ok(p) => {
                    *shadow_pipeline = Some(p);
                    tracing::info!("shadow pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("shadow pipeline reload failed: {e}"),
            }
        }
        Err(e) => tracing::error!("shadow vertex shader reload failed: {e}"),
    }
}

/// Reload tone-map pipeline shaders from disk overrides (hot-reload).
pub fn reload_tonemap_pipeline(
    renderer: &Renderer,
    tonemap_pipeline: &mut Option<rustix_render::pipeline::ToneMapPipeline>,
    tonemap_desc_set: &mut Option<vk::DescriptorSet>,
) {
    match (
        rustix_render::shader::builtin::tonemap_vertex_shader_override(renderer.device().logical()),
        rustix_render::shader::builtin::tonemap_fragment_shader_override(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            let sw = renderer.swapchain.lock();
            match rustix_render::pipeline::ToneMapPipeline::create(renderer.device(), &sw, &vs, &fs, None) {
                Ok(p) => {
                    match renderer.allocate_descriptor_set(p.desc_layout) {
                        Ok(ds) => *tonemap_desc_set = Some(ds),
                        Err(e) => tracing::error!("tonemap desc set alloc failed: {e}"),
                    }
                    *tonemap_pipeline = Some(p);
                    tracing::info!("tonemap pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("tonemap pipeline reload failed: {e}"),
            }
            drop(sw);
        }
        (Err(e), _) => tracing::error!("tonemap vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("tonemap fragment shader reload failed: {e}"),
    }
}

/// Reload 2D sprite pipeline shaders from disk overrides (hot-reload).
pub fn reload_2d_pipeline(
    renderer: &Renderer,
    pipeline_2d: &mut Option<rustix_render::pipeline::GraphicsPipeline2D>,
    desc_set_2d: &mut Option<vk::DescriptorSet>,
) {
    match (
        rustix_render::shader::builtin::vertex_2d_shader_override(renderer.device().logical()),
        rustix_render::shader::builtin::fragment_2d_shader_override(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            let sw = renderer.swapchain.lock();
            match rustix_render::pipeline::GraphicsPipeline2D::create(renderer.device(), &sw, &vs, &fs) {
                Ok(p) => {
                    match renderer.allocate_descriptor_set(p.desc_layout) {
                        Ok(ds) => *desc_set_2d = Some(ds),
                        Err(e) => tracing::error!("2D desc set alloc failed: {e}"),
                    }
                    *pipeline_2d = Some(p);
                    tracing::info!("2D pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("2D pipeline reload failed: {e}"),
            }
            drop(sw);
        }
        (Err(e), _) => tracing::error!("2D vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("2D fragment shader reload failed: {e}"),
    }
}
