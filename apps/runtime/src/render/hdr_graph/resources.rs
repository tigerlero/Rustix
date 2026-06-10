use ash::vk;
use rustix_render::graph::{FrameGraph, TextureDesc, ResourceId};
use rustix_render::DepthBuffer;
use rustix_render::Renderer;

pub struct GraphTextures {
    pub hdr: ResourceId,
    pub depth: ResourceId,
    pub swapchain: ResourceId,
    pub bloom_res: Option<(ResourceId, ResourceId, ResourceId, ResourceId, ResourceId, ResourceId, ResourceId)>,
    pub ssao_res: Option<(ResourceId, ResourceId)>,
    pub taa_res: Option<(ResourceId, ResourceId)>,
    pub ssr_tex: Option<ResourceId>,
    pub fog_tex: Option<ResourceId>,
    pub skybox_tex: Option<ResourceId>,
    pub oit_tex: Option<(ResourceId, ResourceId, ResourceId)>,
    pub csm_res: Vec<ResourceId>,
    pub point_shadow_res: Option<ResourceId>,
    pub spot_shadow_res: Option<ResourceId>,
}

pub fn register_textures(
    graph: &mut FrameGraph,
    renderer: &Renderer,
    hdr_fb: &rustix_render::HdrFramebuffer,
    depth_buf: &DepthBuffer,
    bloom: Option<&crate::render::BloomResources>,
    ssao: Option<&crate::render::SsaoResources>,
    taa: Option<&crate::render::TaaResources>,
    ssr: Option<&crate::render::SsrResources>,
    fog: Option<&crate::render::VolumetricFogResources>,
    skybox: Option<&crate::render::SkyboxResources>,
    oit_resources: Option<&crate::render::OitResources>,
    csm_data: Option<&(u32, Vec<(vk::ImageView, vk::Image)>)>,
    point_shadow: Option<&crate::render::PointShadowResources>,
    spot_data: Option<&((vk::ImageView, vk::Image), u32)>,
) -> GraphTextures {
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
        views.iter().map(|(view, image)| {
            let id = graph.add_texture(TextureDesc {
                format: vk::Format::D32_SFLOAT,
                extent: vk::Extent2D { width: *size, height: *size },
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                view: Some(*view),
                image: Some(*image),
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
            extent: vk::Extent2D { width: *size, height: *size },
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            view: Some(*view),
            image: Some(*image),
            persistent: true,
        });
        graph.set_initial_layout(id, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        id
    });

    GraphTextures {
        hdr,
        depth,
        swapchain,
        bloom_res,
        ssao_res,
        taa_res,
        ssr_tex,
        fog_tex,
        skybox_tex,
        oit_tex,
        csm_res,
        point_shadow_res,
        spot_shadow_res,
    }
}
