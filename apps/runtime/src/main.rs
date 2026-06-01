use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

mod camera;
mod gltf_loader;
mod project;
mod scene;
mod sprite_editor;
mod ui;
mod ui_renderer;
mod undo;
mod waveform;

use ash::vk;
use rustix_core::config;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec2, Vec3, Vec4, Mat4, Quat, EulerRot};
use rustix_platform::input::InputManager;
use rustix_platform::window::{WindowConfig, WindowHandle};
use rustix_render::{Renderer, DirectionalLight, PointLight, SpotLight};
use rustix_render::mesh::Mesh;
use rustix_audio::{AudioEngine, SoundInstance};

use camera::EditorCamera;
use project::{AppScreen, ConfirmTarget, ProjectType, ProjectInfo, load_project_file, create_project_file, add_recent_project, load_recent_projects};
use scene::{Transform, Name, MeshComponent, Material, world_transform, scene_to_world};
use undo::UndoHistory;

/// Configure egui fonts using bundled Noto fonts embedded via `include_bytes!`.
///
/// Fallback chain:
///   Proportional: noto_sans → [Ubuntu-Light] → noto_emoji
///   Monospace:    noto_mono → [Hack] → noto_emoji
///
/// noto_emoji catches emoji and symbols (▶ ⏹ 🔊); box-drawing (└ ─) and
/// arrows (→) are covered by egui's built-in Ubuntu-Light / Hack.
///
/// Fonts are compiled into the binary, so rendering is deterministic across
/// platforms (no dependency on OS-installed fonts).
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let noto_sans = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
    let noto_mono = include_bytes!("../../../assets/fonts/NotoSansMono-Regular.ttf");
    let noto_emoji = include_bytes!("../../../assets/fonts/NotoEmoji-Regular.ttf");

    fonts.font_data.insert(
        "noto_sans".into(),
        std::sync::Arc::new(egui::FontData::from_owned(noto_sans.to_vec())),
    );
    fonts.font_data.insert(
        "noto_mono".into(),
        std::sync::Arc::new(egui::FontData::from_owned(noto_mono.to_vec())),
    );
    fonts.font_data.insert(
        "noto_emoji".into(),
        std::sync::Arc::new(egui::FontData::from_owned(noto_emoji.to_vec())),
    );

    // Proportional: prefer Noto Sans, fall back to Noto Emoji for symbols/emoji
    if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        fam.insert(0, "noto_sans".into());
        fam.push("noto_emoji".into());
    }

    // Monospace: prefer Noto Mono, fall back to Noto Emoji
    if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        fam.insert(0, "noto_mono".into());
        fam.push("noto_emoji".into());
    }

    ctx.set_fonts(fonts);
}

fn main() {
    let config = config::find_and_load_config();
    let log_buffer = rustix_core::init_log_capture(500);
    rustix_core::diagnostics::init_logging_with_capture(
        &rustix_core::diagnostics::LogConfig {
            level: config.log_level(), crate_filters: vec![], json: false, thread_ids: true, targets: true, tracy_enabled: false,
        },
        Some(log_buffer.clone()),
    );
    tracing::info!("Rustix Editor");

    let el = match winit::event_loop::EventLoop::new() {
        Ok(el) => el,
        Err(e) => { eprintln!("Failed to create event loop: {e}"); std::process::exit(1); }
    };
    let wc = WindowConfig { title: "Rustix Editor".into(), width: 1600, height: 900, fullscreen: false, resizable: true, decorations: true };
    let mut window = match WindowHandle::new(&el, &wc) {
        Ok(w) => w,
        Err(e) => { eprintln!("Failed to create window: {e}"); std::process::exit(1); }
    };
    let (mut ww, mut wh) = window.physical_size();

    let mut input = InputManager::new();

    let egui_ctx = egui::Context::default();
    setup_fonts(&egui_ctx);
    tracing::info!("fonts: bundled NotoSans + NotoMono + NotoEmoji");
    let mut egui_state = egui_winit::State::new(egui_ctx.clone(), egui_ctx.viewport_id(), window.inner(), None, None, None);

    let rc = rustix_core::config::RenderConfig {
        enable_validation: false, preferred_gpu: config.render.preferred_gpu, frame_count: config.render.frame_count,
        shader_cache_path: config.render.shader_cache_path, pipeline_cache_path: config.render.pipeline_cache_path,
    };
    let mut renderer = match Renderer::new(&rc) {
        Ok(r) => r,
        Err(e) => { eprintln!("Failed to create Vulkan renderer: {e}"); std::process::exit(1); }
    };
    if let Err(e) = renderer.init_surface(window.raw_window_handle(), window.raw_display_handle(), ww, wh) {
        eprintln!("Failed to create Vulkan surface: {e}"); std::process::exit(1);
    }
    let sc_format = renderer.swapchain.lock().format();
    let mut egui_r = match ui_renderer::EguiVulkanRenderer::new(&renderer, sc_format) {
        Ok(r) => r,
        Err(e) => { eprintln!("Failed to create egui renderer: {e}"); std::process::exit(1); }
    };

    let mut cam = EditorCamera::new();
    let mut last = Instant::now();

    let mut fc = 0u64;
    let mut ft = Instant::now();
    let mut fps = 0u64;

    let mut screen = AppScreen::Startup;
    let mut recent_projects: Vec<project::ProjectEntry> = load_recent_projects();
    let mut current_project: Option<ProjectInfo> = None;
    let mut project_dir: Option<String> = None;
    let mut ecs_world = EcsWorld::new();

    for i in 0..3 {
        let e = ecs_world.spawn((
            Transform { position: Vec3::new(i as f32 * 2.0, 0.0, 0.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
            Name(format!("Entity {}", i)),
            MeshComponent("Cube".into()),
            Material { base_color: Vec3::new(0.6 + i as f32 * 0.15, 0.4 + i as f32 * 0.1, 0.5), roughness: 0.3 + i as f32 * 0.2, metallic: 0.0 },
        ));
        tracing::info!("created entity {}: {:?}", i, e);
    }

    let mut meshes: HashMap<String, Mesh> = HashMap::new();
    let pending_mesh_load: std::rc::Rc<std::cell::RefCell<Option<String>>> = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut scene_pipeline: Option<rustix_render::pipeline::GraphicsPipeline> = None;
    let mut scene_descriptor_pool: Option<vk::DescriptorPool> = None;
    let mut scene_descriptor_set: Option<vk::DescriptorSet> = None;
    let mut scene_uniform_buffer: Option<rustix_render::memory::GpuBuffer> = None;
    let mut scene_depth_buffer: Option<rustix_render::DepthBuffer> = None;

    let mut pipeline_2d: Option<rustix_render::pipeline::GraphicsPipeline2D> = None;
    let mut ubo_2d: Option<rustix_render::memory::GpuBuffer> = None;
    let mut desc_set_2d: Option<vk::DescriptorSet> = None;
    let mut quad_buffer_2d: Option<rustix_render::memory::GpuBuffer> = None;
    let mut texture_2d: Option<rustix_render::GpuTexture> = None;

    let open_project = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
    let new_project = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
    let selected_entity: std::rc::Rc<std::cell::RefCell<Option<hecs::Entity>>> = std::rc::Rc::new(std::cell::RefCell::new(None));
    let pending_delete: std::rc::Rc<std::cell::RefCell<Option<hecs::Entity>>> = std::rc::Rc::new(std::cell::RefCell::new(None));
    let dirty: std::rc::Rc<std::cell::Cell<bool>> = std::rc::Rc::new(std::cell::Cell::new(false));
    let show_confirm: std::rc::Rc<std::cell::Cell<bool>> = std::rc::Rc::new(std::cell::Cell::new(false));
    let confirm_target: std::rc::Rc<std::cell::Cell<ConfirmTarget>> = std::rc::Rc::new(std::cell::Cell::new(ConfirmTarget::None));
    let show_settings: std::rc::Rc<std::cell::Cell<bool>> = std::rc::Rc::new(std::cell::Cell::new(false));
    let renaming: std::rc::Rc<std::cell::RefCell<Option<hecs::Entity>>> = std::rc::Rc::new(std::cell::RefCell::new(None));
    let rename_buffer: std::rc::Rc<std::cell::RefCell<String>> = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let undo_history: std::rc::Rc<std::cell::RefCell<UndoHistory>> = std::rc::Rc::new(std::cell::RefCell::new(UndoHistory::new(100)));
    let show_new_project_type: std::rc::Rc<std::cell::Cell<bool>> = std::rc::Rc::new(std::cell::Cell::new(false));
    let new_project_type: std::rc::Rc<std::cell::Cell<ProjectType>> = std::rc::Rc::new(std::cell::Cell::new(ProjectType::Dim3));

    let mut sprite_editor = sprite_editor::SpriteEditor::default();
    let mut audio_engine = AudioEngine::new().ok();
    let mut audio_instance: Option<SoundInstance> = None;
    let mut waveform_viewer = waveform::WaveformViewer::new();

    let start_time = Instant::now();
    let mut needs_resize = false;
    tracing::info!("editor ready");

    #[allow(deprecated)]
    let _ = el.run(move |event, target| {
        if let winit::event::Event::WindowEvent { ref event, .. } = event {
            let _ = egui_state.on_window_event(window.inner(), event);
        }

        match event {
            winit::event::Event::WindowEvent { event: e, .. } => {
                input.handle_winit_event(&e);
                window.handle_event(&e);
                match e {
                    winit::event::WindowEvent::CloseRequested => target.exit(),
                    winit::event::WindowEvent::Resized(size) => {
                        if size.width > 0 && size.height > 0 {
                            ww = size.width;
                            wh = size.height;
                            needs_resize = true;
                        }
                    }
                    winit::event::WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = now.duration_since(last).as_secs_f32().min(0.1);
                        last = now;

                        input.poll();
                        cam.update(&input, dt);

                        let follow_pos = selected_entity.borrow().and_then(|sel| {
                            let matrix = world_transform(&ecs_world, sel);
                            let (_scale, _rot, pos) = matrix.to_scale_rotation_translation();
                            Some(pos)
                        });
                        cam.follow(follow_pos);

                        if needs_resize {
                            if let Err(e) = renderer.swapchain.lock().recreate(&renderer.instance, &renderer.device) {
                                tracing::error!("swapchain recreate failed: {e}");
                            }
                            let ext = renderer.swapchain.lock().extent();
                            match renderer.create_depth_buffer(ext) {
                                Ok(db) => scene_depth_buffer = Some(db),
                                Err(e) => tracing::error!("depth buffer recreate failed: {e}"),
                            }
                            needs_resize = false;
                        }

                        match renderer.begin_frame() {
                            Ok(false) => return,
                            Err(e) => { tracing::error!("begin_frame: {e}"); return; }
                            _ => {}
                        }
                        let cmd = renderer.current_cmd();
                        if let Err(e) = unsafe { renderer.device().logical().begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)) } {
                            tracing::error!("begin_command_buffer failed: {e}");
                            return;
                        }

                        if scene_pipeline.is_none() {
                            if let Ok(result) = gltf_loader::load_glb(&renderer, &gltf_loader::generate_cube_glb(), "Cube") {
                                meshes.insert("Cube".into(), result.mesh);
                            } else {
                                tracing::error!("failed to load default cube mesh");
                            }
                            if let Ok((sp_verts, sp_idx)) = (|| -> Result<(Vec<rustix_render::mesh::Vertex>, Vec<u16>), rustix_render::RenderError> {
                                Ok(rustix_render::mesh::procedural::uv_sphere(0.5, 16, 16))
                            })() {
                                let vb_slice = bytemuck::cast_slice(&sp_verts);
                                if let Ok(sp_mesh) = Mesh::new(&renderer, "Sphere", vb_slice, sp_verts.len() as u32, Some((&sp_idx, sp_idx.len() as u32))) {
                                    meshes.insert("Sphere".into(), sp_mesh);
                                }
                            }
                            if let Ok((t_verts, t_idx)) = (|| -> Result<(Vec<rustix_render::mesh::Vertex>, Vec<u16>), rustix_render::RenderError> {
                                Ok(rustix_render::mesh::procedural::torus(0.5, 0.15, 24, 12))
                            })() {
                                let vb_slice = bytemuck::cast_slice(&t_verts);
                                if let Ok(t_mesh) = Mesh::new(&renderer, "Torus", vb_slice, t_verts.len() as u32, Some((&t_idx, t_idx.len() as u32))) {
                                    meshes.insert("Torus".into(), t_mesh);
                                }
                            }
                            let vs = rustix_render::shader::builtin::vertex_shader(renderer.device().logical());
                            let fs = rustix_render::shader::builtin::fragment_shader(renderer.device().logical());
                            if let (Ok(vs), Ok(fs)) = (vs, fs) {
                                let sw = renderer.swapchain.lock();
                                match rustix_render::pipeline::GraphicsPipeline::create(renderer.device(), &sw, &vs, &fs) {
                                    Ok(p) => scene_pipeline = Some(p),
                                    Err(e) => tracing::error!("scene pipeline creation failed: {e}"),
                                }
                                match renderer.create_descriptor_pool() {
                                    Ok(dp) => scene_descriptor_pool = Some(dp),
                                    Err(e) => tracing::error!("scene descriptor pool failed: {e}"),
                                }
                                drop(sw);
                            } else {
                                tracing::error!("failed to compile built-in shaders");
                            }
                            if let Some(ref pipeline) = scene_pipeline {
                                if let Some(dp) = scene_descriptor_pool {
                                    match renderer.alloc_descriptor_set(dp, pipeline.descriptor_set_layout) {
                                        Ok(ds) => scene_descriptor_set = Some(ds),
                                        Err(e) => tracing::error!("scene descriptor set alloc failed: {e}"),
                                    }
                                }
                            }
                            match renderer.create_buffer("scene_ubo", rustix_render::pipeline::UBO_SCENE_SIZE, vk::BufferUsageFlags::UNIFORM_BUFFER, gpu_allocator::MemoryLocation::CpuToGpu) {
                                Ok(buf) => scene_uniform_buffer = Some(buf),
                                Err(e) => tracing::error!("scene UBO creation failed: {e}"),
                            }
                            
                            let sw = renderer.swapchain.lock();
                            scene_depth_buffer = renderer.create_depth_buffer(sw.extent()).ok();
                            drop(sw);
                        }

                        if pipeline_2d.is_none() {
                            let vs_2d = rustix_render::shader::builtin::vertex_2d_shader(renderer.device().logical());
                            let fs_2d = rustix_render::shader::builtin::fragment_2d_shader(renderer.device().logical());
                            if let (Ok(vs), Ok(fs)) = (vs_2d, fs_2d) {
                                let sw = renderer.swapchain.lock();
                                match rustix_render::pipeline::GraphicsPipeline2D::create(renderer.device(), &sw, &vs, &fs) {
                                    Ok(p) => {
                                        match renderer.alloc_descriptor_set(p.desc_pool, p.desc_layout) {
                                            Ok(ds) => desc_set_2d = Some(ds),
                                            Err(e) => tracing::error!("2D desc set alloc failed: {e}"),
                                        }
                                        pipeline_2d = Some(p);
                                    }
                                    Err(e) => tracing::error!("2D pipeline creation failed: {e}"),
                                }
                                drop(sw);
                            }
                            match renderer.create_buffer("ubo_2d", 64, vk::BufferUsageFlags::UNIFORM_BUFFER, gpu_allocator::MemoryLocation::CpuToGpu) {
                                Ok(buf) => ubo_2d = Some(buf),
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
                                    quad_buffer_2d = Some(buf);
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
                                Ok(tex) => texture_2d = Some(tex),
                                Err(e) => tracing::error!("2D texture creation failed: {e}"),
                            }
                        }

                        if let Some(path) = pending_mesh_load.borrow_mut().take() {
                            if let Ok(data) = std::fs::read(&path) {
                                let mesh_name = Path::new(&path)
                                    .file_stem().and_then(|s| s.to_str()).unwrap_or("Imported")
                                    .to_string();
                                if let Ok(result) = gltf_loader::load_glb(&renderer, &data, &mesh_name) {
                                    tracing::info!("loaded mesh {mesh_name} from {path} (base={:?} rough={:.2} metal={:.2})",
                                        result.base_color, result.roughness, result.metallic);
                                    meshes.insert(mesh_name.clone(), result.mesh);
                                    let e = ecs_world.spawn((
                                        Transform { position: Vec3::new(0.0, 1.0, 0.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
                                        Name(mesh_name.clone()),
                                        MeshComponent(mesh_name),
                                        Material { base_color: Vec3::from(result.base_color), roughness: result.roughness, metallic: result.metallic },
                                    ));
                                    *selected_entity.borrow_mut() = Some(e);
                                    dirty.set(true);
                                } else {
                                    tracing::error!("failed to load mesh from {path}");
                                }
                            } else {
                                tracing::error!("failed to read file {path}");
                            }
                        }

                        if let (Some(ref pipeline), Some(depth_buf)) = 
                            (&scene_pipeline, &scene_depth_buffer) 
                        {
                            if let Some(ubo) = &scene_uniform_buffer {
                                let aspect = {
                                    let sw = renderer.swapchain.lock();
                                    sw.extent().width as f32 / sw.extent().height as f32
                                };
                                let view_proj = cam.view_proj(aspect);
                                let eye = cam.eye_pos();

                                let mut point_lights: Vec<(Vec3, f32, Vec3, f32)> = Vec::new();
                                for (_e, pl, xform) in ecs_world.query::<(&Entity, &PointLight, &Transform)>().iter() {
                                    point_lights.push((
                                        xform.position,
                                        pl.radius.max(0.1),
                                        Vec3::new(pl.color.x * pl.intensity, pl.color.y * pl.intensity, pl.color.z * pl.intensity),
                                        pl.intensity,
                                    ));
                                }
                                for (_e, sl, xform) in ecs_world.query::<(&Entity, &SpotLight, &Transform)>().iter() {
                                    point_lights.push((
                                        xform.position,
                                        sl.radius.max(0.1),
                                        Vec3::new(sl.color.x * sl.intensity, sl.color.y * sl.intensity, sl.color.z * sl.intensity),
                                        sl.intensity,
                                    ));
                                }
                                point_lights.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
                                let light_count = point_lights.len().min(8) as u32;

                                let mut ubo_data = [0u8; 368];
                                ubo_data[0..64].copy_from_slice(bytemuck::bytes_of(&view_proj));
                                ubo_data[64..80].copy_from_slice(bytemuck::bytes_of(&Vec4::new(eye.x, eye.y, eye.z, 0.0)));
                                ubo_data[80..84].copy_from_slice(&light_count.to_ne_bytes());
                                for (i, (pos, radius, color, _)) in point_lights.iter().take(8).enumerate() {
                                    let off = 96 + i * 32;
                                    ubo_data[off..off+16].copy_from_slice(bytemuck::bytes_of(&Vec4::new(pos.x, pos.y, pos.z, *radius)));
                                    ubo_data[off+16..off+32].copy_from_slice(bytemuck::bytes_of(&Vec4::new(color.x, color.y, color.z, 1.0)));
                                }
                                let fog_color = Vec4::new(0.15, 0.18, 0.25, cam.distance * 3.0 + 10.0);
                                ubo_data[352..368].copy_from_slice(bytemuck::bytes_of(&fog_color));
                                ubo.write(&ubo_data);
                                
                                if let Some(set) = scene_descriptor_set {
                                    renderer.update_descriptor_set(set, ubo);
                                
                                let (light_dir, light_color) = {
                                    let mut d = Vec3::new(0.5, 0.8, 0.3);
                                    let mut c = Vec3::new(1.0, 0.95, 0.8);
                                    for (dirlight, xform) in ecs_world.query_mut::<(&DirectionalLight, &Transform)>() {
                                        let rot = Quat::from_euler(EulerRot::XYZ, xform.rotation.x, xform.rotation.y, xform.rotation.z);
                                        d = (rot * Vec3::NEG_Z).normalize();
                                        c = Vec3::new(dirlight.color.x * dirlight.intensity, dirlight.color.y * dirlight.intensity, dirlight.color.z * dirlight.intensity);
                                        break;
                                    }
                                    (Vec4::new(d.x, d.y, d.z, 0.2), Vec4::new(c.x, c.y, c.z, 1.0))
                                };
                                
                                let clear_color = [0.04, 0.04, 0.08, 1.0f32];
                                renderer.begin_scene_pass(cmd, depth_buf, clear_color);
                                
                                for (entity, _transform, mesh_comp) in ecs_world.query::<(&Entity, &Transform, &MeshComponent)>().iter() {
                                    if let Some(mesh) = meshes.get(&mesh_comp.0) {
                                        let model = world_transform(&ecs_world, *entity);

                                        let mat: Option<(Vec4, f32)> = ecs_world.get::<&Material>(*entity).ok()
                                            .map(|m| (Vec4::new(m.base_color.x, m.base_color.y, m.base_color.z, m.roughness), m.metallic));
                                        let (mat_v, metallic) = mat.unwrap_or((Vec4::new(0.7, 0.7, 0.7, 0.5), 0.0));
                                        
                                        let mut pc_data = [0u8; 128];
                                        pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&model));
                                        pc_data[64..80].copy_from_slice(bytemuck::bytes_of(&light_dir));
                                        pc_data[80..96].copy_from_slice(bytemuck::bytes_of(&light_color));
                                        pc_data[96..112].copy_from_slice(bytemuck::bytes_of(&mat_v));
                                        pc_data[112..128].copy_from_slice(bytemuck::bytes_of(&Vec2::new(metallic, 0.0)));
                                        
                                        renderer.draw_indexed_in_pass(
                                            cmd, pipeline,
                                            &mesh.vertex_buffer,
                                            mesh.index_buffer.as_ref(), mesh.index_count,
                                            &pc_data,
                                            set,
                                        );
                                    }
                                    let _ = entity;
                                }
                                
                                renderer.end_scene_pass(cmd);
                                } // end if let Some(set)
                            }
                        }

                        if let (Some(ref ppl), Some(ref buf), Some(ref ubo), Some(ref tex), Some(ds)) = 
                            (&pipeline_2d, &quad_buffer_2d, &ubo_2d, &texture_2d, desc_set_2d)
                        {
                            let sw = renderer.swapchain.lock();
                            let w = sw.extent().width as f32;
                            let h = sw.extent().height as f32;
                            drop(sw);
                            let ortho = Mat4::orthographic_rh_gl(0.0, w, h, 0.0, -1.0, 1.0);
                            ubo.write(bytemuck::bytes_of(&ortho));
                            renderer.update_2d_descriptor_set(ds, ubo, tex);

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
                            renderer.draw_2d(cmd, ppl, buf, 4, &pc, ds);
                        }

                        // Process egui and upload textures DURING command buffer recording.
                        // update_textures now waits for GPU idle before partial updates to avoid
                        // layout-transition races with in-flight frames.
                        let mut raw_input = egui_state.take_egui_input(window.inner());
                        if !input.mouse().down(rustix_platform::input::MouseButton::Left) {
                            raw_input.events.push(egui::Event::PointerButton {
                                pos: egui::Pos2::ZERO,
                                button: egui::PointerButton::Primary,
                                pressed: false,
                                modifiers: egui::Modifiers::default(),
                            });
                        }
                        let out = egui_ctx.run(raw_input, |ctx| {
                            match screen {
                                AppScreen::Startup => {
                                    ui::startup_screen(ctx, &recent_projects, &mut screen, &open_project, &new_project, &*show_new_project_type, &*new_project_type);
                                }
                                AppScreen::Editor => {
                                    let proj_name = current_project.as_ref().map(|p| p.name.as_str()).unwrap_or("Untitled");
                                    let proj_name_owned = proj_name.to_string();
                                    ui::editor_screen(ctx, &mut cam, &mut window, &mut screen, &input, target, &ww, &wh, &mut fps, &open_project, &new_project, &proj_name_owned, &mut current_project, &mut project_dir, &mut ecs_world, &*selected_entity, &*pending_delete, &*dirty, &*show_confirm, &*confirm_target, &*show_settings, &*renaming, &*rename_buffer, &*undo_history, &mut sprite_editor, &pending_mesh_load, &mut audio_engine, &mut audio_instance, &mut waveform_viewer);
                                }
                            }
                        });

                        if let Some(path) = open_project.borrow_mut().take() {
                            let dir = Path::new(&path);
                            let info = load_project_file(dir).or_else(|| create_project_file(dir, ProjectType::Dim3));
                            if let Some(ref proj_info) = info {
                                if !proj_info.scene.entities.is_empty() {
                                    scene_to_world(&mut ecs_world, &proj_info.scene);
                                }
                                current_project = info;
                                project_dir = Some(path.clone());
                                add_recent_project(&mut recent_projects, path, &current_project);
                                screen = AppScreen::Editor;
                                window.request_redraw();
                            }
                        }
                        if let Some(path) = new_project.borrow_mut().take() {
                            let dir = Path::new(&path);
                            let ptype = new_project_type.get();
                            let info = create_project_file(dir, ptype);
                            if info.is_some() {
                                current_project = info;
                                project_dir = Some(path.clone());
                                add_recent_project(&mut recent_projects, path, &current_project);
                                screen = AppScreen::Editor;
                                window.request_redraw();
                            }
                        }

                        // Upload egui textures (font atlas etc.) before tessellation
                        egui_r.update_textures(&renderer, &out.textures_delta);
                        let clipped = egui_ctx.tessellate(out.shapes, out.pixels_per_point);

                        egui_r.draw_primitives(cmd, &renderer, &clipped, out.pixels_per_point, renderer.frame_index());

                        egui_state.handle_platform_output(window.inner(), out.platform_output);

                        if let Err(e) = renderer.end_frame() {
                            tracing::error!("end_frame: {e}");
                        }
                        input.end_tick();

                        fc += 1;
                        if ft.elapsed().as_secs_f32() >= 1.0 {
                            fps = fc;
                            fc = 0;
                            ft = Instant::now();
                            let proj_label = current_project.as_ref().map(|p| p.name.as_str()).unwrap_or("Rustix Editor");
                            let star = if dirty.get() { " *" } else { "" };
                            window.set_title(&format!("{proj_label}{star} \u{2014} FPS: {fps}"));
                        }
                    }
                    _ => {}
                }
            }
            winit::event::Event::AboutToWait => window.request_redraw(),
            _ => {}
        }
    });
}
