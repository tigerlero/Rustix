use std::time::Instant;
use ash::vk;
use rustix_core::math::{Vec3, Mat4, Quat};
use rustix_render::Renderer;
use rustix_render::pipeline::GraphicsPipeline2D;
use rustix_render::memory::GpuBuffer;
use rustix_render::GpuTexture;

#[allow(dead_code)]
pub fn render_2d_overlay(
    renderer: &Renderer,
    cmd: vk::CommandBuffer,
    pipeline: &GraphicsPipeline2D,
    quad_buffer: &GpuBuffer,
    ubo_2d: &GpuBuffer,
    texture: &GpuTexture,
    desc_set: vk::DescriptorSet,
    start_time: Instant,
) {
    let sw = renderer.swapchain.lock();
    let w = sw.extent().width as f32;
    let h = sw.extent().height as f32;
    drop(sw);
    let ortho = Mat4::orthographic_rh_gl(0.0, w, h, 0.0, -1.0, 1.0);
    ubo_2d.write(bytemuck::bytes_of(&ortho));
    renderer.update_2d_descriptor_set(desc_set, ubo_2d, texture);

    let t = start_time.elapsed().as_secs_f32();
    let pulse = (t * 2.0).sin() * 0.3 + 0.7;
    let s = 100.0 * pulse;
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(s, s, 1.0),
        Quat::from_rotation_z(t * 1.5),
        Vec3::new(w * 0.5, h * 0.5, 0.0),
    );
    let mut pc = [0u8; 64];
    pc.copy_from_slice(bytemuck::bytes_of(&model));
    renderer.draw_2d(cmd, pipeline, quad_buffer, 4, &pc, desc_set);
}
