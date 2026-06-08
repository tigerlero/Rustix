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
    bloom_extract_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
    bloom_down_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
    bloom_up_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
    bloom_desc_set: &mut Option<vk::DescriptorSet>,
    ssao_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
    ssao_blur_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
    ssao_desc_set: &mut Option<vk::DescriptorSet>,
    taa_pipeline: &mut Option<rustix_render::pipeline::TaaPipeline>,
    taa_desc_set: &mut Option<vk::DescriptorSet>,
    ssr_pipeline: &mut Option<rustix_render::pipeline::SsrPipeline>,
    ssr_desc_set: &mut Option<vk::DescriptorSet>,
    fog_pipeline: &mut Option<rustix_render::pipeline::VolumetricFogPipeline>,
    fog_desc_set: &mut Option<vk::DescriptorSet>,
    skybox_pipeline: &mut Option<rustix_render::pipeline::SkyboxPipeline>,
    skybox_desc_set: &mut Option<vk::DescriptorSet>,
    instanced_pipeline: &mut Option<rustix_render::pipeline::InstancedGraphicsPipeline>,
    instanced_gbuffer_pipeline: &mut Option<rustix_render::pipeline::InstancedGBufferPipeline>,
    mesh_shader_pipeline: &mut Option<rustix_render::pipeline::MeshShaderPipeline>,
    oit_accumulate_pipeline: &mut Option<rustix_render::pipeline::OitAccumulatePipeline>,
    oit_composite_pipeline: &mut Option<rustix_render::pipeline::OitCompositePipeline>,
    oit_desc_set: &mut Option<vk::DescriptorSet>,
) {
    if scene_pipeline.is_some() && scene_uniform_buffer.is_some() && scene_depth_buffer.is_some()
        && bloom_extract_pipeline.is_some() && bloom_down_pipeline.is_some() && bloom_up_pipeline.is_some()
        && ssao_pipeline.is_some() && ssao_blur_pipeline.is_some()
        && taa_pipeline.is_some() && ssr_pipeline.is_some() && fog_pipeline.is_some() && skybox_pipeline.is_some()
        && instanced_pipeline.is_some() && instanced_gbuffer_pipeline.is_some() {
        tracing::info!("init_scene_resources: already initialized, {} meshes in registry", meshes.len());
        return;
    }

    if let Ok(result) = crate::gltf_loader::load_glb(renderer, &crate::gltf_loader::generate_cube_glb(), "Cube") {
        meshes.insert("Cube".into(), result.mesh);
        tracing::info!("loaded default Cube mesh");
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
        Ok(buf) => {
            renderer.update_descriptor_set(vk::DescriptorSet::null(), &buf);
            *scene_uniform_buffer = Some(buf);
        }
        Err(e) => tracing::error!("scene UBO creation failed: {e}"),
    }

    let sw = renderer.swapchain.lock();
    *scene_depth_buffer = renderer.create_depth_buffer(sw.extent()).ok();
    drop(sw);

    if shadow_pipeline.is_none() {
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
    if tonemap_pipeline.is_none() {
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

    // Bloom pipelines
    if bloom_extract_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::bloom_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::bloom_extract_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                    Ok(p) => {
                        match renderer.allocate_descriptor_set(p.desc_layout) {
                            Ok(ds) => *bloom_desc_set = Some(ds),
                            Err(e) => tracing::error!("bloom desc set alloc failed: {e}"),
                        }
                        *bloom_extract_pipeline = Some(p);
                    }
                    Err(e) => tracing::error!("bloom extract pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("bloom vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("bloom extract fragment shader compile failed: {e}"),
        }
    }
    if bloom_down_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::bloom_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::bloom_down_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                    Ok(p) => *bloom_down_pipeline = Some(p),
                    Err(e) => tracing::error!("bloom down pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("bloom vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("bloom down fragment shader compile failed: {e}"),
        }
    }
    if bloom_up_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::bloom_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::bloom_up_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                    Ok(p) => *bloom_up_pipeline = Some(p),
                    Err(e) => tracing::error!("bloom up pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("bloom vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("bloom up fragment shader compile failed: {e}"),
        }
    }

    // SSAO pipelines
    if ssao_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::ssao_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::ssao_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R8_UNORM, &vs, &fs) {
                    Ok(p) => {
                        match renderer.allocate_descriptor_set(p.desc_layout) {
                            Ok(ds) => *ssao_desc_set = Some(ds),
                            Err(e) => tracing::error!("ssao desc set alloc failed: {e}"),
                        }
                        *ssao_pipeline = Some(p);
                    }
                    Err(e) => tracing::error!("ssao pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("ssao vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("ssao fragment shader compile failed: {e}"),
        }
    }
    if ssao_blur_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::ssao_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::ssao_blur_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R8_UNORM, &vs, &fs) {
                    Ok(p) => *ssao_blur_pipeline = Some(p),
                    Err(e) => tracing::error!("ssao blur pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("ssao vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("ssao blur fragment shader compile failed: {e}"),
        }
    }

    // TAA pipeline
    if taa_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::taa_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::taa_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::TaaPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                    Ok(p) => {
                        match renderer.allocate_descriptor_set(p.desc_layout) {
                            Ok(ds) => *taa_desc_set = Some(ds),
                            Err(e) => tracing::error!("taa desc set alloc failed: {e}"),
                        }
                        *taa_pipeline = Some(p);
                    }
                    Err(e) => tracing::error!("taa pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("taa vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("taa fragment shader compile failed: {e}"),
        }
    }

    // SSR pipeline
    if ssr_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::ssr_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::ssr_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::SsrPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                    Ok(p) => {
                        match renderer.allocate_descriptor_set(p.desc_layout) {
                            Ok(ds) => *ssr_desc_set = Some(ds),
                            Err(e) => tracing::error!("ssr desc set alloc failed: {e}"),
                        }
                        *ssr_pipeline = Some(p);
                    }
                    Err(e) => tracing::error!("ssr pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("ssr vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("ssr fragment shader compile failed: {e}"),
        }
    }

    // Volumetric fog pipeline
    if fog_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::volumetric_fog_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::volumetric_fog_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::VolumetricFogPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                    Ok(p) => {
                        match renderer.allocate_descriptor_set(p.desc_layout) {
                            Ok(ds) => *fog_desc_set = Some(ds),
                            Err(e) => tracing::error!("fog desc set alloc failed: {e}"),
                        }
                        *fog_pipeline = Some(p);
                    }
                    Err(e) => tracing::error!("fog pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("fog vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("fog fragment shader compile failed: {e}"),
        }
    }

    // Skybox pipeline
    if skybox_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::skybox_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::skybox_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::SkyboxPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                    Ok(p) => {
                        match renderer.allocate_descriptor_set(p.desc_layout) {
                            Ok(ds) => *skybox_desc_set = Some(ds),
                            Err(e) => tracing::error!("skybox desc set alloc failed: {e}"),
                        }
                        *skybox_pipeline = Some(p);
                    }
                    Err(e) => tracing::error!("skybox pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("skybox vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("skybox fragment shader compile failed: {e}"),
        }
    }

    // Instanced pipeline
    if instanced_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::pbr_instanced_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::pbr_instanced_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                let mut variant = rustix_render::pipeline::PipelineVariantKey::default();
                variant.render_path = rustix_render::pipeline::RenderPath::Forward;
                variant.quality_level = rustix_render::pipeline::QualityLevel::High;
                variant.polygon_mode = vk::PolygonMode::FILL;
                variant.cull_mode = vk::CullModeFlags::BACK;
                variant.depth_test = true;
                variant.depth_write = true;
                variant.blend_enable = false;
                match rustix_render::pipeline::InstancedGraphicsPipeline::create(
                    renderer.device(), &*renderer.swapchain.lock(), &vs, &fs,
                    renderer.bindless_heap().layout(), &variant,
                ) {
                    Ok(p) => {
                        *instanced_pipeline = Some(p);
                        tracing::info!("instanced pipeline created");
                    }
                    Err(e) => tracing::error!("instanced pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("instanced vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("instanced fragment shader compile failed: {e}"),
        }
    }

    // Instanced GBuffer pipeline
    if instanced_gbuffer_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::gbuffer_instanced_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::gbuffer_instanced_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::InstancedGBufferPipeline::create(
                    renderer.device(), &vs, &fs, renderer.bindless_heap().layout(),
                ) {
                    Ok(p) => {
                        *instanced_gbuffer_pipeline = Some(p);
                        tracing::info!("instanced gbuffer pipeline created");
                    }
                    Err(e) => tracing::error!("instanced gbuffer pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("instanced gbuffer vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("instanced gbuffer fragment shader compile failed: {e}"),
        }
    }

    // Mesh shader pipeline (only if extension is supported)
    if renderer.device().mesh_shader_supported() && mesh_shader_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::pbr_mesh_shader(renderer.device().logical()),
            rustix_render::shader::builtin::pbr_instanced_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(ms), Ok(fs)) => {
                let mut variant = rustix_render::pipeline::PipelineVariantKey::default();
                variant.render_path = rustix_render::pipeline::RenderPath::Forward;
                variant.quality_level = rustix_render::pipeline::QualityLevel::Medium;
                variant.cull_mode = vk::CullModeFlags::BACK;
                variant.polygon_mode = vk::PolygonMode::FILL;
                variant.depth_test = true;
                variant.depth_write = true;
                variant.blend_enable = false;
                match rustix_render::pipeline::MeshShaderPipeline::create(
                    renderer.device(), &*renderer.swapchain.lock(), &ms, &fs,
                    renderer.bindless_heap().layout(), &variant,
                ) {
                    Ok(p) => {
                        *mesh_shader_pipeline = Some(p);
                        tracing::info!("mesh shader pipeline created");
                    }
                    Err(e) => tracing::error!("mesh shader pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("mesh shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("mesh shader fragment shader compile failed: {e}"),
        }
    }

    // OIT pipelines
    if oit_accumulate_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::oit_accumulate_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::oit_accumulate_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                match rustix_render::pipeline::OitAccumulatePipeline::create(
                    renderer.device(), &vs, &fs, renderer.bindless_heap().layout(),
                ) {
                    Ok(p) => {
                        *oit_accumulate_pipeline = Some(p);
                        tracing::info!("oit accumulate pipeline created");
                    }
                    Err(e) => tracing::error!("oit accumulate pipeline creation failed: {e}"),
                }
            }
            (Err(e), _) => tracing::error!("oit accumulate vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("oit accumulate fragment shader compile failed: {e}"),
        }
    }
    if oit_composite_pipeline.is_none() {
        match (
            rustix_render::shader::builtin::oit_composite_vertex_shader(renderer.device().logical()),
            rustix_render::shader::builtin::oit_composite_fragment_shader(renderer.device().logical()),
        ) {
            (Ok(vs), Ok(fs)) => {
                let sw = renderer.swapchain.lock();
                match rustix_render::pipeline::OitCompositePipeline::create(
                    renderer.device(), sw.format(), &vs, &fs,
                ) {
                    Ok(p) => {
                        match renderer.allocate_descriptor_set(p.desc_layout) {
                            Ok(ds) => *oit_desc_set = Some(ds),
                            Err(e) => tracing::error!("oit composite desc set alloc failed: {e}"),
                        }
                        *oit_composite_pipeline = Some(p);
                        tracing::info!("oit composite pipeline created");
                    }
                    Err(e) => tracing::error!("oit composite pipeline creation failed: {e}"),
                }
                drop(sw);
            }
            (Err(e), _) => tracing::error!("oit composite vertex shader compile failed: {e}"),
            (_, Err(e)) => tracing::error!("oit composite fragment shader compile failed: {e}"),
        }
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

/// Reload bloom extract pipeline shaders from disk overrides (hot-reload).
pub fn reload_bloom_extract_pipeline(
    renderer: &Renderer,
    bloom_extract_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
    bloom_desc_set: &mut Option<vk::DescriptorSet>,
) {
    match (
        rustix_render::shader::builtin::bloom_vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::bloom_extract_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                Ok(p) => {
                    match renderer.allocate_descriptor_set(p.desc_layout) {
                        Ok(ds) => *bloom_desc_set = Some(ds),
                        Err(e) => tracing::error!("bloom desc set alloc failed: {e}"),
                    }
                    *bloom_extract_pipeline = Some(p);
                    tracing::info!("bloom extract pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("bloom extract pipeline reload failed: {e}"),
            }
        }
        (Err(e), _) => tracing::error!("bloom vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("bloom extract fragment shader reload failed: {e}"),
    }
}

/// Reload bloom downsample pipeline fragment shader from disk overrides (hot-reload).
pub fn reload_bloom_down_pipeline(
    renderer: &Renderer,
    bloom_down_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
) {
    if let Ok(fs) = rustix_render::shader::builtin::bloom_down_fragment_shader(renderer.device().logical()) {
        if let Ok(vs) = rustix_render::shader::builtin::bloom_vertex_shader(renderer.device().logical()) {
            match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                Ok(p) => {
                    *bloom_down_pipeline = Some(p);
                    tracing::info!("bloom down pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("bloom down pipeline reload failed: {e}"),
            }
        }
    }
}

/// Reload bloom upsample pipeline fragment shader from disk overrides (hot-reload).
pub fn reload_bloom_up_pipeline(
    renderer: &Renderer,
    bloom_up_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
) {
    if let Ok(fs) = rustix_render::shader::builtin::bloom_up_fragment_shader(renderer.device().logical()) {
        if let Ok(vs) = rustix_render::shader::builtin::bloom_vertex_shader(renderer.device().logical()) {
            match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                Ok(p) => {
                    *bloom_up_pipeline = Some(p);
                    tracing::info!("bloom up pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("bloom up pipeline reload failed: {e}"),
            }
        }
    }
}

/// Reload SSAO pipeline shaders from disk overrides (hot-reload).
pub fn reload_ssao_pipeline(
    renderer: &Renderer,
    ssao_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
    ssao_desc_set: &mut Option<vk::DescriptorSet>,
) {
    match (
        rustix_render::shader::builtin::ssao_vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::ssao_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R8_UNORM, &vs, &fs) {
                Ok(p) => {
                    match renderer.allocate_descriptor_set(p.desc_layout) {
                        Ok(ds) => *ssao_desc_set = Some(ds),
                        Err(e) => tracing::error!("ssao desc set alloc failed: {e}"),
                    }
                    *ssao_pipeline = Some(p);
                    tracing::info!("ssao pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("ssao pipeline reload failed: {e}"),
            }
        }
        (Err(e), _) => tracing::error!("ssao vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("ssao fragment shader reload failed: {e}"),
    }
}

/// Reload SSAO blur pipeline fragment shader from disk overrides (hot-reload).
pub fn reload_ssao_blur_pipeline(
    renderer: &Renderer,
    ssao_blur_pipeline: &mut Option<rustix_render::pipeline::BloomPipeline>,
) {
    if let Ok(fs) = rustix_render::shader::builtin::ssao_blur_fragment_shader(renderer.device().logical()) {
        if let Ok(vs) = rustix_render::shader::builtin::ssao_vertex_shader(renderer.device().logical()) {
            match rustix_render::pipeline::BloomPipeline::create(renderer.device(), vk::Format::R8_UNORM, &vs, &fs) {
                Ok(p) => {
                    *ssao_blur_pipeline = Some(p);
                    tracing::info!("ssao blur pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("ssao blur pipeline reload failed: {e}"),
            }
        }
    }
}

/// Reload TAA pipeline shaders from disk overrides (hot-reload).
pub fn reload_taa_pipeline(
    renderer: &Renderer,
    taa_pipeline: &mut Option<rustix_render::pipeline::TaaPipeline>,
    taa_desc_set: &mut Option<vk::DescriptorSet>,
) {
    match (
        rustix_render::shader::builtin::taa_vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::taa_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            match rustix_render::pipeline::TaaPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                Ok(p) => {
                    match renderer.allocate_descriptor_set(p.desc_layout) {
                        Ok(ds) => *taa_desc_set = Some(ds),
                        Err(e) => tracing::error!("taa desc set alloc failed: {e}"),
                    }
                    *taa_pipeline = Some(p);
                    tracing::info!("taa pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("taa pipeline reload failed: {e}"),
            }
        }
        (Err(e), _) => tracing::error!("taa vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("taa fragment shader reload failed: {e}"),
    }
}

/// Reload SSR pipeline shaders from disk overrides (hot-reload).
pub fn reload_ssr_pipeline(
    renderer: &Renderer,
    ssr_pipeline: &mut Option<rustix_render::pipeline::SsrPipeline>,
    ssr_desc_set: &mut Option<vk::DescriptorSet>,
) {
    match (
        rustix_render::shader::builtin::ssr_vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::ssr_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            match rustix_render::pipeline::SsrPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                Ok(p) => {
                    match renderer.allocate_descriptor_set(p.desc_layout) {
                        Ok(ds) => *ssr_desc_set = Some(ds),
                        Err(e) => tracing::error!("ssr desc set alloc failed: {e}"),
                    }
                    *ssr_pipeline = Some(p);
                    tracing::info!("ssr pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("ssr pipeline reload failed: {e}"),
            }
        }
        (Err(e), _) => tracing::error!("ssr vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("ssr fragment shader reload failed: {e}"),
    }
}

/// Reload volumetric fog pipeline shaders from disk overrides (hot-reload).
pub fn reload_fog_pipeline(
    renderer: &Renderer,
    fog_pipeline: &mut Option<rustix_render::pipeline::VolumetricFogPipeline>,
    fog_desc_set: &mut Option<vk::DescriptorSet>,
) {
    match (
        rustix_render::shader::builtin::volumetric_fog_vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::volumetric_fog_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            match rustix_render::pipeline::VolumetricFogPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                Ok(p) => {
                    match renderer.allocate_descriptor_set(p.desc_layout) {
                        Ok(ds) => *fog_desc_set = Some(ds),
                        Err(e) => tracing::error!("fog desc set alloc failed: {e}"),
                    }
                    *fog_pipeline = Some(p);
                    tracing::info!("fog pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("fog pipeline reload failed: {e}"),
            }
        }
        (Err(e), _) => tracing::error!("fog vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("fog fragment shader reload failed: {e}"),
    }
}

/// Reload skybox pipeline shaders from disk overrides (hot-reload).
pub fn reload_skybox_pipeline(
    renderer: &Renderer,
    skybox_pipeline: &mut Option<rustix_render::pipeline::SkyboxPipeline>,
    skybox_desc_set: &mut Option<vk::DescriptorSet>,
) {
    match (
        rustix_render::shader::builtin::skybox_vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::skybox_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            match rustix_render::pipeline::SkyboxPipeline::create(renderer.device(), vk::Format::R16G16B16A16_SFLOAT, &vs, &fs) {
                Ok(p) => {
                    match renderer.allocate_descriptor_set(p.desc_layout) {
                        Ok(ds) => *skybox_desc_set = Some(ds),
                        Err(e) => tracing::error!("skybox desc set alloc failed: {e}"),
                    }
                    *skybox_pipeline = Some(p);
                    tracing::info!("skybox pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("skybox pipeline reload failed: {e}"),
            }
        }
        (Err(e), _) => tracing::error!("skybox vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("skybox fragment shader reload failed: {e}"),
    }
}

/// Reload instanced forward pipeline shaders from disk overrides (hot-reload).
pub fn reload_instanced_pipeline(
    renderer: &Renderer,
    instanced_pipeline: &mut Option<rustix_render::pipeline::InstancedGraphicsPipeline>,
) {
    match (
        rustix_render::shader::builtin::pbr_instanced_vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::pbr_instanced_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            let mut variant = rustix_render::pipeline::PipelineVariantKey::default();
            variant.render_path = rustix_render::pipeline::RenderPath::Forward;
            variant.quality_level = rustix_render::pipeline::QualityLevel::High;
            variant.polygon_mode = vk::PolygonMode::FILL;
            variant.cull_mode = vk::CullModeFlags::BACK;
            variant.depth_test = true;
            variant.depth_write = true;
            variant.blend_enable = false;
            match rustix_render::pipeline::InstancedGraphicsPipeline::create(
                renderer.device(), &*renderer.swapchain.lock(), &vs, &fs,
                renderer.bindless_heap().layout(), &variant,
            ) {
                Ok(p) => {
                    *instanced_pipeline = Some(p);
                    tracing::info!("instanced pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("instanced pipeline reload failed: {e}"),
            }
        }
        (Err(e), _) => tracing::error!("instanced vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("instanced fragment shader reload failed: {e}"),
    }
}

/// Reload instanced gbuffer pipeline shaders from disk overrides (hot-reload).
pub fn reload_instanced_gbuffer_pipeline(
    renderer: &Renderer,
    instanced_gbuffer_pipeline: &mut Option<rustix_render::pipeline::InstancedGBufferPipeline>,
) {
    match (
        rustix_render::shader::builtin::gbuffer_instanced_vertex_shader(renderer.device().logical()),
        rustix_render::shader::builtin::gbuffer_instanced_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(vs), Ok(fs)) => {
            match rustix_render::pipeline::InstancedGBufferPipeline::create(
                renderer.device(), &vs, &fs, renderer.bindless_heap().layout(),
            ) {
                Ok(p) => {
                    *instanced_gbuffer_pipeline = Some(p);
                    tracing::info!("instanced gbuffer pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("instanced gbuffer pipeline reload failed: {e}"),
            }
        }
        (Err(e), _) => tracing::error!("instanced gbuffer vertex shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("instanced gbuffer fragment shader reload failed: {e}"),
    }
}

/// Reload mesh shader pipeline from disk overrides (hot-reload).
pub fn reload_mesh_shader_pipeline(
    renderer: &Renderer,
    mesh_shader_pipeline: &mut Option<rustix_render::pipeline::MeshShaderPipeline>,
) {
    if !renderer.device().mesh_shader_supported() {
        return;
    }
    match (
        rustix_render::shader::builtin::pbr_mesh_shader(renderer.device().logical()),
        rustix_render::shader::builtin::pbr_instanced_fragment_shader(renderer.device().logical()),
    ) {
        (Ok(ms), Ok(fs)) => {
            let mut variant = rustix_render::pipeline::PipelineVariantKey::default();
            variant.render_path = rustix_render::pipeline::RenderPath::Forward;
            variant.quality_level = rustix_render::pipeline::QualityLevel::Medium;
            variant.cull_mode = vk::CullModeFlags::BACK;
            variant.polygon_mode = vk::PolygonMode::FILL;
            variant.depth_test = true;
            variant.depth_write = true;
            variant.blend_enable = false;
            match rustix_render::pipeline::MeshShaderPipeline::create(
                renderer.device(), &*renderer.swapchain.lock(), &ms, &fs,
                renderer.bindless_heap().layout(), &variant,
            ) {
                Ok(p) => {
                    *mesh_shader_pipeline = Some(p);
                    tracing::info!("mesh shader pipeline hot-reloaded");
                }
                Err(e) => tracing::error!("mesh shader pipeline reload failed: {e}"),
            }
        }
        (Err(e), _) => tracing::error!("mesh shader reload failed: {e}"),
        (_, Err(e)) => tracing::error!("mesh shader fragment shader reload failed: {e}"),
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
