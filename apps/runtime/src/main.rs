use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

mod camera;
mod fonts;
mod gltf_loader;
mod init;
mod project;
mod render;
mod scene;
mod sprite_editor;
mod ui;
mod ui_renderer;
mod undo;
mod waveform;

use ash::vk;
use rustix_core::config;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::Vec3;
use rustix_platform::input::InputManager;
use rustix_platform::window::{WindowConfig, WindowHandle};
use rustix_render::{Renderer, DirectionalLight};
use rustix_render::mesh::Mesh;
use rustix_audio::{AudioEngine, SoundInstance};
use rustix_animation::{Animator, AnimationClip, update_animators};
use rustix_physics::{RigidBody, Collider, PhysicsWorld, step_physics};

use camera::EditorCamera;
use project::{AppScreen, ConfirmTarget, ProjectType, ProjectInfo, load_project_file, create_project_file, add_recent_project, load_recent_projects, write_project_file};
use scene::{Transform, Name, MeshComponent, Material, world_transform, scene_to_world, world_to_scene};
use undo::UndoHistory;

fn main() {
    let config = config::find_and_load_config();
    let log_buffer = rustix_core::init_log_capture(500);
    rustix_core::diagnostics::init_logging_with_capture(
        &rustix_core::diagnostics::LogConfig {
            level: config.log_level(), crate_filters: vec![], json: false, json_file_path: None, thread_ids: true, targets: true, tracy_enabled: false,
        },
        Some(log_buffer.clone()),
    );
    tracing::info!("Rustix Editor");

    let el = match winit::event_loop::EventLoop::new() {
        Ok(el) => el,
        Err(e) => { eprintln!("Failed to create event loop: {e}"); std::process::exit(1); }
    };
    let wc = WindowConfig { title: "Rustix Editor".into(), width: 1600, height: 900, fullscreen: false, fullscreen_mode: rustix_platform::FullscreenMode::Windowed, resizable: true, decorations: true };
    let mut window = match WindowHandle::new(&el, &wc) {
        Ok(w) => w,
        Err(e) => { eprintln!("Failed to create window: {e}"); std::process::exit(1); }
    };
    let (mut ww, mut wh) = window.physical_size();

    let mut input = InputManager::new();

    let egui_ctx = egui::Context::default();
    fonts::setup_fonts(&egui_ctx);
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
    tracing::info!("startup world has {} named entities", ecs_world.query::<&Name>().iter().count());

    let mut meshes: HashMap<String, Mesh> = HashMap::new();
    let mut animation_clips: HashMap<String, AnimationClip> = HashMap::new();
    let mut physics_world = PhysicsWorld::default();
    let pending_mesh_load: std::rc::Rc<std::cell::RefCell<Option<String>>> = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut scene_pipeline: Option<rustix_render::pipeline::GraphicsPipeline> = None;
    let mut scene_descriptor_pool: Option<vk::DescriptorPool> = None;
    let mut scene_descriptor_set: Option<vk::DescriptorSet> = None;
    let mut scene_uniform_buffer: Option<rustix_render::memory::GpuBuffer> = None;
    let mut scene_depth_buffer: Option<rustix_render::DepthBuffer> = None;
    let mut shadow_pipeline: Option<rustix_render::pipeline::ShadowPipeline> = None;
    let mut shadow_descriptor_pool: Option<vk::DescriptorPool> = None;
    let mut shadow_descriptor_set: Option<vk::DescriptorSet> = None;
    let mut shadow_map: Option<rustix_render::GpuTexture> = None;
    let mut shadow_layout = vk::ImageLayout::UNDEFINED;

    let mut viewport_framebuffers: [Option<rustix_render::Framebuffer>; 3] = std::array::from_fn(|_| None);
    let mut viewport_fb_size: (u32, u32) = (0, 0);

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

                        // Update animations
                        {
                            let mut animators: Vec<(hecs::Entity, &mut Animator)> = Vec::new();
                            for (e, mut a) in ecs_world.query_mut::<(&hecs::Entity, &mut Animator)>() {
                                animators.push((*e, a));
                            }
                            let results = update_animators(&mut animators, &animation_clips, dt);
                            for (entity, pos, rot, scale) in results {
                                if let Ok(mut t) = ecs_world.get::<&mut Transform>(entity) {
                                    if let Some(p) = pos { t.position = p; }
                                    if let Some(r) = rot { t.rotation = r; }
                                    if let Some(s) = scale { t.scale = s; }
                                }
                            }
                        }

                        // Update physics
                        {
                            let mut bodies: Vec<(hecs::Entity, RigidBody)> = Vec::new();
                            for (e, b) in ecs_world.query_mut::<(&hecs::Entity, &RigidBody)>() {
                                bodies.push((*e, *b));
                            }
                            let results = step_physics(&mut bodies, &physics_world, dt);
                            for (entity, pos_delta, rot_delta) in results {
                                if let Ok(mut t) = ecs_world.get::<&mut Transform>(entity) {
                                    t.position += pos_delta;
                                    t.rotation += rot_delta;
                                }
                            }
                            for (entity, body) in bodies {
                                if let Ok(mut b) = ecs_world.get::<&mut RigidBody>(entity) {
                                    *b = body;
                                }
                            }
                        }

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

                        init::init_scene_resources(
                            &renderer, &mut meshes,
                            &mut scene_pipeline, &mut scene_descriptor_pool, &mut scene_descriptor_set,
                            &mut scene_uniform_buffer, &mut scene_depth_buffer,
                            &mut shadow_pipeline, &mut shadow_descriptor_pool, &mut shadow_descriptor_set,
                            &mut shadow_map,
                        );

                        init::init_2d_resources(
                            &renderer,
                            &mut pipeline_2d, &mut ubo_2d, &mut desc_set_2d,
                            &mut quad_buffer_2d, &mut texture_2d,
                        );

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

                        // Determine viewport size from previous frame's egui rect
                        let viewport_rect: Option<egui::Rect> = egui_ctx.data(|d| d.get_temp(egui::Id::new("viewport_rect")));
                        let has_valid_vp_rect = viewport_rect.is_some();
                        let (vp_w, vp_h) = viewport_rect.map(|r| (r.width().max(1.0) as u32, r.height().max(1.0) as u32))
                            .unwrap_or((ww, wh));

                        // Create/recreate viewport framebuffer when size changes (only when we have a valid rect)
                        let frame_idx = renderer.frame_index() % 3;
                        if screen == AppScreen::Editor && has_valid_vp_rect {
                            if viewport_fb_size != (vp_w, vp_h) {
                                tracing::trace!("main: recreating viewport framebuffers {}x{}", vp_w, vp_h);
                                for fb in &mut viewport_framebuffers { *fb = None; }
                                viewport_fb_size = (vp_w, vp_h);
                            }
                            if viewport_framebuffers[frame_idx].is_none() {
                                match renderer.create_framebuffer(vp_w, vp_h, sc_format) {
                                    Ok(fb) => {
                                        viewport_framebuffers[frame_idx] = Some(fb);
                                        tracing::trace!("main: viewport framebuffer {} created", frame_idx);
                                    }
                                    Err(e) => tracing::error!("viewport framebuffer create failed: {e}"),
                                }
                            }
                        }

                        let shadow_layout_ref = if shadow_map.is_some() { Some(&mut shadow_layout) } else { None };
                        if scene_pipeline.is_some() && scene_depth_buffer.is_some() && scene_uniform_buffer.is_some() && scene_descriptor_set.is_some() {
                            // Only render offscreen in Editor mode AND when we have a valid viewport rect
                            let offscreen_fb = if screen == AppScreen::Editor && has_valid_vp_rect {
                                viewport_framebuffers[frame_idx].as_ref()
                            } else {
                                None
                            };
                            
                            // If rendering to offscreen, we must still clear the swapchain so egui has a clean background
                            if offscreen_fb.is_some() {
                                renderer.begin_scene_pass(cmd, scene_depth_buffer.as_ref().unwrap(), [0.05, 0.05, 0.05, 1.0]);
                                renderer.end_scene_pass(cmd);
                            }

                            render::render_3d_scene(
                                &renderer, cmd,
                                scene_pipeline.as_ref().unwrap(),
                                shadow_pipeline.as_ref(),
                                scene_depth_buffer.as_ref().unwrap(),
                                shadow_map.as_ref(),
                                shadow_layout_ref,
                                scene_uniform_buffer.as_ref().unwrap(),
                                scene_descriptor_set.unwrap(),
                                shadow_descriptor_set,
                                &meshes, &ecs_world, &cam,
                                offscreen_fb,
                            );
                            // Register offscreen framebuffer as egui user texture for viewport panel
                            if let Some(ref fb) = viewport_framebuffers[frame_idx] {
                                tracing::trace!("main: registering viewport framebuffer color_view={:?} as user texture", fb.color_view);
                                egui_r.register_user_texture(
                                    egui::TextureId::User(0),
                                    fb.color_view,
                                    egui_r.sampler(),
                                );
                                egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new("viewport_offscreen_valid"), true));
                                tracing::trace!("main: set viewport_offscreen_valid=true");
                            } else {
                                tracing::trace!("main: no viewport framebuffer, removing offscreen valid flag");
                                egui_ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("viewport_offscreen_valid")));
                            }
                        } else if renderer.frame_index() % 60 == 0 {
                            tracing::warn!("3D scene skipped: pipeline={} depth={} ubo={} desc={}",
                                scene_pipeline.is_some(), scene_depth_buffer.is_some(),
                                scene_uniform_buffer.is_some(), scene_descriptor_set.is_some());
                        }

                        // 2D debug overlay disabled — viewport is for 3D scene only.
                        // if let (Some(ref ppl), Some(ref buf), Some(ref ubo), Some(ref tex), Some(ds)) =
                        //     (&pipeline_2d, &quad_buffer_2d, &ubo_2d, &texture_2d, desc_set_2d)
                        // {
                        //     render::render_2d_overlay(
                        //         &renderer, cmd, ppl, buf, ubo, tex, ds, start_time,
                        //     );
                        // }

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
                                if let Some(ref cam_state) = proj_info.editor_camera {
                                    cam.position = cam_state.position.into();
                                    cam.center = cam_state.center.into();
                                    cam.yaw = cam_state.yaw;
                                    cam.pitch = cam_state.pitch;
                                    cam.distance = cam_state.distance;
                                    cam.mode = cam_state.mode;
                                    cam.follow_target = cam_state.follow_target;
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
                            let mut info = create_project_file(dir, ptype);
                            if let Some(ref mut proj) = info {
                                ecs_world.clear();
                                ecs_world.spawn((
                                    Transform { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE },
                                    Name("Cube".into()),
                                    MeshComponent("Cube".into()),
                                    Material { base_color: Vec3::new(0.7, 0.7, 0.7), roughness: 0.5, metallic: 0.0 },
                                ));
                                ecs_world.spawn((
                                    Transform { position: Vec3::new(5.0, 10.0, 5.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
                                    Name("Light".into()),
                                    DirectionalLight { color: Vec3::new(1.0, 1.0, 1.0), intensity: 1.0 },
                                ));
                                proj.scene = world_to_scene(&ecs_world);
                                let _ = write_project_file(dir, proj);
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
