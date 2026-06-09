use ash::vk;
use rustix_render::Renderer;

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
