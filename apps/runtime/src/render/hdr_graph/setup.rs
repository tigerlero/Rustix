use ash::vk;
use rustix_core::ecs::EcsWorld;
use rustix_core::math::{Vec2, Vec3, Vec4, Mat4};
use rustix_render::memory::GpuBuffer;
use rustix_render::DirectionalLight;
use crate::camera::EditorCamera;
use crate::scene::Transform;
use crate::render::{CsmResources, ForwardPlusResources, collect_lights, directional_light_dir_from_euler};

pub struct SceneSetup {
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub point_lights: Vec<(Vec3, f32, Vec3, f32)>,
    pub light_dir: Vec4,
    pub light_color: Vec4,
    pub screen_w: u32,
    pub screen_h: u32,
    pub tile_count_x: u32,
    pub tile_count_y: u32,
    pub csm_data: Option<(u32, Vec<(vk::ImageView, vk::Image)>)>,
    pub spot_data: Option<((vk::ImageView, vk::Image), u32)>,
}

pub fn prepare_scene_data(
    cam: &EditorCamera,
    ecs_world: &EcsWorld,
    ubo: &GpuBuffer,
    fwd_plus: Option<&ForwardPlusResources>,
    csm: &mut Option<&mut CsmResources>,
    spot_shadow: Option<&crate::render::SpotShadowResources>,
    hdr_extent: vk::Extent2D,
) -> SceneSetup {
    let aspect = hdr_extent.width as f32 / hdr_extent.height as f32;
    let view_proj = cam.view_proj(aspect);
    let eye = cam.eye_pos();
    let point_lights = collect_lights(ecs_world);
    let light_count = point_lights.len().min(8) as u32;
    let screen_w = hdr_extent.width;
    let screen_h = hdr_extent.height;
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

    let mut ubo_data = [0u8; 480];
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

    // Compute L1 SH from directional light + ambient for GI ambient term.
    let sh = rustix_render::sh::ShIrradianceL1::from_directional_and_ambient(
        Vec3::new(light_dir.x, light_dir.y, light_dir.z),
        Vec3::new(light_color.x, light_color.y, light_color.z),
        Vec3::new(0.03, 0.03, 0.03),
    );
    ubo_data[432..448].copy_from_slice(bytemuck::bytes_of(&Vec4::new(sh.r.c[0], sh.r.c[1], sh.r.c[2], sh.r.c[3])));
    ubo_data[448..464].copy_from_slice(bytemuck::bytes_of(&Vec4::new(sh.g.c[0], sh.g.c[1], sh.g.c[2], sh.g.c[3])));
    ubo_data[464..480].copy_from_slice(bytemuck::bytes_of(&Vec4::new(sh.b.c[0], sh.b.c[1], sh.b.c[2], sh.b.c[3])));

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

    let csm_data = csm.as_ref().map(|c| (
        c.shadow_map_size,
        c.shadow_maps.iter().map(|sm| (sm.view, sm.image)).collect::<Vec<_>>(),
    ));
    let spot_data = spot_shadow.map(|ss| ((ss.array.view, ss.array.image), ss.size));

    SceneSetup {
        view_proj,
        eye,
        point_lights,
        light_dir,
        light_color,
        screen_w,
        screen_h,
        tile_count_x,
        tile_count_y,
        csm_data,
        spot_data,
    }
}
