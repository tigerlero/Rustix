use ash::vk;
use rustix_render::Renderer;

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

