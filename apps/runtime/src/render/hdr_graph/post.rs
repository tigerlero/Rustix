use ash::vk;
use rustix_core::math::{Vec3, Vec4, Mat4};
use rustix_render::Renderer;
use rustix_render::DepthBuffer;
use rustix_render::graph::PassContext;

pub fn execute_fog_pass(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    fog_pipe: &rustix_render::pipeline::VolumetricFogPipeline,
    fds: vk::DescriptorSet,
    depth_buf: &DepthBuffer,
    scene_color_view: vk::ImageView,
    inv_vp: Mat4,
    eye: Vec3,
    fog_max_steps: f32,
    fog_density: f32,
    fog_scattering: f32,
    fog_height_falloff: f32,
    fog_max_dist: f32,
    fog_sun_intensity: f32,
) {
    let pc_data: [[f32; 4]; 6] = [
        [inv_vp.x_axis.x, inv_vp.x_axis.y, inv_vp.x_axis.z, inv_vp.x_axis.w],
        [inv_vp.y_axis.x, inv_vp.y_axis.y, inv_vp.y_axis.z, inv_vp.y_axis.w],
        [inv_vp.z_axis.x, inv_vp.z_axis.y, inv_vp.z_axis.z, inv_vp.z_axis.w],
        [inv_vp.w_axis.x, inv_vp.w_axis.y, inv_vp.w_axis.z, inv_vp.w_axis.w],
        [eye.x, eye.y, eye.z, fog_max_steps],
        [fog_density, fog_scattering, fog_height_falloff, fog_max_dist],
    ];
    let light_dir = Vec3::new(0.3, -1.0, 0.2).normalize();
    let mut pc_data2 = pc_data;
    pc_data2[5] = [light_dir.x, light_dir.y, light_dir.z, fog_sun_intensity];
    let depth_ii = [vk::DescriptorImageInfo::default().image_view(depth_buf.view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let color_ii = [vk::DescriptorImageInfo::default().image_view(scene_color_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [
        vk::WriteDescriptorSet::default().dst_set(fds).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&depth_ii),
        vk::WriteDescriptorSet::default().dst_set(fds).dst_binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&color_ii),
    ];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, fog_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, fog_pipe.layout, 0, &[fds], &[]);
        device.cmd_push_constants(cmd, fog_pipe.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc_data2));
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}

pub fn execute_skybox_pass(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    sb_pipe: &rustix_render::pipeline::SkyboxPipeline,
    sds: vk::DescriptorSet,
    depth_buf: &DepthBuffer,
    scene_color_view: vk::ImageView,
    inv_vp: Mat4,
    skybox_rayleigh: f32,
    skybox_mie: f32,
    skybox_zenith_shift: f32,
    skybox_exposure: f32,
) {
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
    let depth_ii = [vk::DescriptorImageInfo::default().image_view(depth_buf.view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let color_ii = [vk::DescriptorImageInfo::default().image_view(scene_color_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [
        vk::WriteDescriptorSet::default().dst_set(sds).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&depth_ii),
        vk::WriteDescriptorSet::default().dst_set(sds).dst_binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&color_ii),
    ];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, sb_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, sb_pipe.layout, 0, &[sds], &[]);
        device.cmd_push_constants(cmd, sb_pipe.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc_data2));
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}

pub fn execute_ssr_pass(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    ssr_pipe: &rustix_render::pipeline::SsrPipeline,
    sds: vk::DescriptorSet,
    depth_buf: &DepthBuffer,
    ssr_color_view: vk::ImageView,
    gbuf: &crate::render::GBufferResources,
    inv_vp: Mat4,
    eye: Vec3,
    ssr_max_steps: f32,
    ssr_stride: f32,
    ssr_max_dist: f32,
    hdr_extent: vk::Extent2D,
) {
    let pc_data: [[f32; 4]; 6] = [
        [inv_vp.x_axis.x, inv_vp.x_axis.y, inv_vp.x_axis.z, inv_vp.x_axis.w],
        [inv_vp.y_axis.x, inv_vp.y_axis.y, inv_vp.y_axis.z, inv_vp.y_axis.w],
        [inv_vp.z_axis.x, inv_vp.z_axis.y, inv_vp.z_axis.z, inv_vp.z_axis.w],
        [inv_vp.w_axis.x, inv_vp.w_axis.y, inv_vp.w_axis.z, inv_vp.w_axis.w],
        [eye.x, eye.y, eye.z, ssr_max_steps],
        [hdr_extent.width as f32, hdr_extent.height as f32, ssr_stride, ssr_max_dist],
    ];
    let depth_ii = [vk::DescriptorImageInfo::default().image_view(depth_buf.view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let color_ii = [vk::DescriptorImageInfo::default().image_view(ssr_color_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let normal_ii = [vk::DescriptorImageInfo::default().image_view(gbuf.normal_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [
        vk::WriteDescriptorSet::default().dst_set(sds).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&depth_ii),
        vk::WriteDescriptorSet::default().dst_set(sds).dst_binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&color_ii),
        vk::WriteDescriptorSet::default().dst_set(sds).dst_binding(4).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&normal_ii),
    ];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, ssr_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, ssr_pipe.layout, 0, &[sds], &[]);
        device.cmd_push_constants(cmd, ssr_pipe.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc_data));
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}

pub fn execute_taa_pass(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    taa_pipe: &rustix_render::pipeline::TaaPipeline,
    tds: vk::DescriptorSet,
    depth_buf: &DepthBuffer,
    taa_source_view: vk::ImageView,
    taa: &crate::render::TaaResources,
    inv_vp: Mat4,
    prev_vp: Mat4,
    blend: f32,
    hdr_extent: vk::Extent2D,
) {
    let pc_blend = [blend, hdr_extent.width as f32, hdr_extent.height as f32, 1.0];
    let pc_data: [[f32; 4]; 9] = [
        [inv_vp.x_axis.x, inv_vp.x_axis.y, inv_vp.x_axis.z, inv_vp.x_axis.w],
        [inv_vp.y_axis.x, inv_vp.y_axis.y, inv_vp.y_axis.z, inv_vp.y_axis.w],
        [inv_vp.z_axis.x, inv_vp.z_axis.y, inv_vp.z_axis.z, inv_vp.z_axis.w],
        [inv_vp.w_axis.x, inv_vp.w_axis.y, inv_vp.w_axis.z, inv_vp.w_axis.w],
        [prev_vp.x_axis.x, prev_vp.x_axis.y, prev_vp.x_axis.z, prev_vp.x_axis.w],
        [prev_vp.y_axis.x, prev_vp.y_axis.y, prev_vp.y_axis.z, prev_vp.y_axis.w],
        [prev_vp.z_axis.x, prev_vp.z_axis.y, prev_vp.z_axis.z, prev_vp.z_axis.w],
        [prev_vp.w_axis.x, prev_vp.w_axis.y, prev_vp.w_axis.z, prev_vp.w_axis.w],
        pc_blend,
    ];
    let current_ii = [vk::DescriptorImageInfo::default().image_view(taa_source_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let history_ii = [vk::DescriptorImageInfo::default().image_view(taa.history_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let depth_ii = [vk::DescriptorImageInfo::default().image_view(depth_buf.view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [
        vk::WriteDescriptorSet::default().dst_set(tds).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&current_ii),
        vk::WriteDescriptorSet::default().dst_set(tds).dst_binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&history_ii),
        vk::WriteDescriptorSet::default().dst_set(tds).dst_binding(4).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&depth_ii),
    ];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, taa_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, taa_pipe.layout, 0, &[tds], &[]);
        device.cmd_push_constants(cmd, taa_pipe.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc_data));
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}

pub fn execute_ssao_generate(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    ssao_pipe: &rustix_render::pipeline::BloomPipeline,
    sds: vk::DescriptorSet,
    depth_buf: &DepthBuffer,
    hdr_extent: vk::Extent2D,
    ssao_radius: f32,
    ssao_bias: f32,
    ssao_power: f32,
    ssao_intensity: f32,
) {
    let near = 0.1;
    let far = 100.0;
    let fov_y = 60.0f32.to_radians();
    let tan_half_fov = (fov_y * 0.5).tan();
    let aspect = hdr_extent.width as f32 / hdr_extent.height as f32;
    let proj_params = [near, far, 1.0 / tan_half_fov, aspect];
    let radius_bias = [ssao_radius, ssao_bias, ssao_power, ssao_intensity];
    let screen_size = [hdr_extent.width as f32, hdr_extent.height as f32, 1.0 / hdr_extent.width as f32, 1.0 / hdr_extent.height as f32];
    let pc_data: [[f32; 4]; 3] = [proj_params, radius_bias, screen_size];
    let tex_ii = [vk::DescriptorImageInfo::default().image_view(depth_buf.view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [vk::WriteDescriptorSet::default().dst_set(sds).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&tex_ii)];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, ssao_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, ssao_pipe.layout, 0, &[sds], &[]);
        device.cmd_push_constants(cmd, ssao_pipe.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc_data));
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}

pub fn execute_ssao_blur(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    blur_pipe: &rustix_render::pipeline::BloomPipeline,
    sds: vk::DescriptorSet,
    ssao: &crate::render::SsaoResources,
) {
    let pc = [1.0 / ssao.extent.width as f32, 1.0 / ssao.extent.height as f32, 0.0, 0.0];
    let tex_ii = [vk::DescriptorImageInfo::default().image_view(ssao.ao_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [vk::WriteDescriptorSet::default().dst_set(sds).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&tex_ii)];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, blur_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, blur_pipe.layout, 0, &[sds], &[]);
        device.cmd_push_constants(cmd, blur_pipe.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc));
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}

pub fn execute_bloom_extract(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    extract_pipe: &rustix_render::pipeline::BloomPipeline,
    bds: vk::DescriptorSet,
    bloom_source_view: vk::ImageView,
    bloom_threshold: f32,
    bloom_intensity: f32,
) {
    let pc = [bloom_threshold, bloom_intensity, 0.0, 0.0];
    let tex_ii = [vk::DescriptorImageInfo::default().image_view(bloom_source_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [vk::WriteDescriptorSet::default().dst_set(bds).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&tex_ii)];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, extract_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, extract_pipe.layout, 0, &[bds], &[]);
        device.cmd_push_constants(cmd, extract_pipe.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc));
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}

pub fn execute_bloom_down(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    down_pipe: &rustix_render::pipeline::BloomPipeline,
    bds: vk::DescriptorSet,
    bloom_view: vk::ImageView,
    inv_w: f32,
    inv_h: f32,
) {
    let pc = [inv_w, inv_h, 0.0, 0.0];
    let tex_ii = [vk::DescriptorImageInfo::default().image_view(bloom_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [vk::WriteDescriptorSet::default().dst_set(bds).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&tex_ii)];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, down_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, down_pipe.layout, 0, &[bds], &[]);
        device.cmd_push_constants(cmd, down_pipe.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc));
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}

pub fn execute_bloom_up(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    up_pipe: &rustix_render::pipeline::BloomPipeline,
    bds: vk::DescriptorSet,
    bloom_view: vk::ImageView,
    inv_w: f32,
    inv_h: f32,
) {
    let pc = [inv_w, inv_h, 0.0, 0.0];
    let tex_ii = [vk::DescriptorImageInfo::default().image_view(bloom_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [vk::WriteDescriptorSet::default().dst_set(bds).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&tex_ii)];
    unsafe { renderer.device().logical().update_descriptor_sets(&writes, &[]); }
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, up_pipe.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, up_pipe.layout, 0, &[bds], &[]);
        device.cmd_push_constants(cmd, up_pipe.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&pc));
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}

pub fn execute_tonemap(
    cmd: vk::CommandBuffer,
    renderer: &Renderer,
    tonemap_pipeline: &rustix_render::pipeline::ToneMapPipeline,
    tonemap_desc_set: vk::DescriptorSet,
    tonemap_hdr_view: vk::ImageView,
    bloom: Option<&crate::render::BloomResources>,
    ssao: Option<&crate::render::SsaoResources>,
    sampler: vk::Sampler,
) {
    let bloom_view = if let Some(b) = bloom { b.mip0b_view } else { tonemap_hdr_view };
    let ssao_view = if let Some(s) = ssao { s.blurred_ao_view } else { tonemap_hdr_view };
    renderer.update_tonemap_descriptor_set(tonemap_desc_set, tonemap_hdr_view, bloom_view, ssao_view, sampler);
    unsafe {
        let device = renderer.device().logical();
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, tonemap_pipeline.pipeline);
        device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, tonemap_pipeline.layout, 0, &[tonemap_desc_set], &[]);
        device.cmd_draw(cmd, 3, 1, 0, 0);
    }
}
