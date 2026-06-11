
use std::collections::HashMap;
use ash::vk;
use rustix_render::{Renderer, mesh::Mesh};
use rustix_terrain::{TerrainParams, generate_heightmap, build_terrain_mesh};

pub fn init_scene_resources(
    renderer: &Renderer,
    meshes: &mut HashMap<String, Mesh>,
    scene_pipeline: &mut Option<rustix_render::pipeline::GraphicsPipeline>,
    wireframe_scene_pipeline: &mut Option<rustix_render::pipeline::GraphicsPipeline>,
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
        tracing::trace!("init_scene_resources: already initialized, {} meshes in registry", meshes.len());
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
        rustix_render::shader::builtin::vertex_shader_override(renderer.device().logical()),
        rustix_render::shader::builtin::fragment_shader_override(renderer.device().logical()),
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
            // Wireframe variant of scene pipeline
            let mut wf_key = rustix_render::pipeline::PipelineVariantKey::default();
            wf_key.polygon_mode = vk::PolygonMode::LINE;
            wf_key.spec_constants.set(0, 1);
            match renderer.pipeline_variant_cache().get_or_create(
                &wf_key,
                renderer.device(),
                &sw,
                &vs,
                &fs,
            ) {
                Ok(_) => {
                    if let Some(gp) = renderer.pipeline_variant_cache().get_pipeline(&wf_key) {
                        *wireframe_scene_pipeline = Some(gp);
                    }
                }
                Err(e) => tracing::error!("wireframe pipeline creation failed: {e}"),
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
