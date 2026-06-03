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

use rustix_asset::mmap::MappedFile;

use camera::EditorCamera;
use project::{AppScreen, ConfirmTarget, ProjectType, ProjectInfo, load_project_file, create_project_file, add_recent_project, load_recent_projects, write_project_file};
use scene::{Transform, Name, MeshComponent, Material, world_transform, scene_to_world, world_to_scene};
use undo::UndoHistory;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC: {}", info);
        if let Some(loc) = info.location() {
            eprintln!("  at {}:{}", loc.file(), loc.line());
        }
    }));
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
    let wc = WindowConfig { title: "Rustix Editor".into(), width: 1600, height: 900, fullscreen: false, fullscreen_mode: rustix_platform::FullscreenMode::Windowed, resizable: true, decorations: true, cursor_mode: rustix_platform::window::CursorMode::Normal };
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
    tracing::info!("creating Vulkan renderer...");
    let mut renderer = match Renderer::new(&rc) {
        Ok(r) => { tracing::info!("Vulkan renderer created"); r }
        Err(e) => { eprintln!("Failed to create Vulkan renderer: {e}"); std::process::exit(1); }
    };
    tracing::info!("initializing Vulkan surface...");
    if let Err(e) = renderer.init_surface(window.raw_window_handle(), window.raw_display_handle(), ww, wh) {
        eprintln!("Failed to create Vulkan surface: {e}"); std::process::exit(1);
    }
    tracing::info!("Vulkan surface initialized");
    let sc_format = renderer.swapchain.lock().format();
    tracing::info!("creating egui Vulkan renderer...");
    let mut egui_r = match ui_renderer::EguiVulkanRenderer::new(&renderer, sc_format) {
        Ok(r) => { tracing::info!("egui Vulkan renderer created"); r }
        Err(e) => { eprintln!("Failed to create egui renderer: {e}"); std::process::exit(1); }
    };

    let mut viewport_manager = ui::viewport::ViewportManager::new();
    let mut last = Instant::now();
    let mut next_frame_time = Instant::now();

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

    // Per-viewport offscreen framebuffers (each triple-buffered).
    let mut viewport_framebuffers: Vec<[Option<rustix_render::Framebuffer>; 3]> = (0..ui::viewport::MAX_VIEWPORTS)
        .map(|_| [None, None, None])
        .collect();
    let mut viewport_fb_sizes: Vec<(u32, u32)> = vec![(0, 0); ui::viewport::MAX_VIEWPORTS];

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
    let mut gamepad_input = rustix_platform::gamepad::GamepadInput::new();
    let mut input_actions = rustix_platform::actions::InputActions::new();
    let binding_config_path = dirs::config_dir()
        .map(|d| d.join("rustix").join("bindings.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("bindings.json"));
    if let Some(cfg) = rustix_platform::actions::load_binding_config(&binding_config_path) {
        input_actions.load_bindings(&cfg.bindings);
        tracing::info!("loaded input bindings from {}", binding_config_path.display());
    } else {
        input_actions.bind_defaults();
        tracing::info!("using default input bindings");
    }

    let mut input_recorder = rustix_platform::recorder::InputRecorder::new();
    let recording_dir = dirs::config_dir()
        .map(|d| d.join("rustix").join("recordings"))
        .unwrap_or_else(|| std::path::PathBuf::from("recordings"));
    let _ = std::fs::create_dir_all(&recording_dir);

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
                    winit::event::WindowEvent::ScaleFactorChanged { .. } => {
                        // egui_winit updates pixels_per_point automatically.
                        // Invalidate all viewport framebuffers so they are recreated at the new DPI.
                        for vp_idx in 0..viewport_fb_sizes.len() {
                            viewport_fb_sizes[vp_idx] = (0, 0);
                            for fb in &mut viewport_framebuffers[vp_idx] { *fb = None; }
                        }
                        tracing::info!("DPI scale factor changed, invalidating viewport framebuffers");
                    }
                    winit::event::WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = now.duration_since(last).as_secs_f32().min(0.1);
                        last = now;

                        // Capture all input events for recording
                        if input_recorder.mode() == rustix_platform::recorder::RecorderMode::Recording {
                            input.start_capture();
                        } else {
                            input.stop_capture();
                        }

                        for ev in gamepad_input.poll() {
                            input_actions.handle_gamepad_event(&ev);
                            input.push_event(ev);
                        }
                        input.poll();
                        input_actions.update(&input);

                        // Drain captured events into the recorder
                        for ev in input.drain_captured() {
                            input_recorder.record(ev);
                        }

                        // Inject playback events
                        for ev in input_recorder.poll_playback() {
                            input.push_event(ev);
                        }
                        input.poll();
                        viewport_manager.primary_camera_mut().update(&input, dt);

                        // Input recorder controls
                        if input.keyboard().just_pressed(rustix_platform::input::KeyCode::F9) {
                            match input_recorder.mode() {
                                rustix_platform::recorder::RecorderMode::Idle => {
                                    input_recorder.start_recording();
                                    input.start_capture();
                                    tracing::info!("input recording started");
                                }
                                rustix_platform::recorder::RecorderMode::Recording => {
                                    input.stop_capture();
                                    let rec = input_recorder.stop_recording();
                                    let path = recording_dir.join("recording.json");
                                    if rustix_platform::recorder::save_recording(&path, &rec).is_some() {
                                        tracing::info!("input recording saved to {}", path.display());
                                    } else {
                                        tracing::warn!("failed to save input recording");
                                    }
                                }
                                _ => {}
                            }
                        }
                        if input.keyboard().just_pressed(rustix_platform::input::KeyCode::F10) {
                            match input_recorder.mode() {
                                rustix_platform::recorder::RecorderMode::Idle => {
                                    let path = recording_dir.join("recording.json");
                                    if let Some(rec) = rustix_platform::recorder::load_recording(&path) {
                                        input_recorder.start_playback(rec);
                                        tracing::info!("input playback started from {}", path.display());
                                    } else {
                                        tracing::warn!("no recording found at {}", path.display());
                                    }
                                }
                                rustix_platform::recorder::RecorderMode::Playing |
                                rustix_platform::recorder::RecorderMode::Paused => {
                                    input_recorder.stop_playback();
                                    tracing::info!("input playback stopped");
                                }
                                _ => {}
                            }
                        }

                        let follow_pos = selected_entity.borrow().and_then(|sel| {
                            let matrix = world_transform(&ecs_world, sel);
                            let (_scale, _rot, pos) = matrix.to_scale_rotation_translation();
                            Some(pos)
                        });
                        viewport_manager.primary_camera_mut().follow(follow_pos);

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
                        renderer.profiler_begin(cmd);

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
                            if let Ok(data) = MappedFile::open(Path::new(&path)) {
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

                        // Multi-viewport: determine size and create framebuffers for each open viewport.
                        let frame_idx = renderer.frame_index() % 3;
                        let mut any_offscreen = false;
                        let num_viewports = viewport_manager.viewports.len();

                        let ppp = egui_ctx.pixels_per_point();
                        for vp_idx in 0..num_viewports {
                            let rect_key = egui::Id::new(format!("viewport_rect_{}", vp_idx));
                            let viewport_rect: Option<egui::Rect> = egui_ctx.data(|d| d.get_temp(rect_key));
                            let has_valid = viewport_rect.is_some();
                            let (vp_w, vp_h) = viewport_rect.map(|r| {
                                let w = (r.width() * ppp).ceil().max(1.0) as u32;
                                let h = (r.height() * ppp).ceil().max(1.0) as u32;
                                (w, h)
                            }).unwrap_or((ww, wh));

                            if (screen == AppScreen::Editor || screen == AppScreen::PlayTest) && has_valid {
                                if viewport_fb_sizes[vp_idx] != (vp_w, vp_h) {
                                    tracing::trace!("main: recreating viewport {} framebuffers {}x{}", vp_idx, vp_w, vp_h);
                                    for fb in &mut viewport_framebuffers[vp_idx] { *fb = None; }
                                    viewport_fb_sizes[vp_idx] = (vp_w, vp_h);
                                }
                                if viewport_framebuffers[vp_idx][frame_idx].is_none() {
                                    match renderer.create_framebuffer(vp_w, vp_h, sc_format) {
                                        Ok(fb) => {
                                            viewport_framebuffers[vp_idx][frame_idx] = Some(fb);
                                            tracing::trace!("main: viewport {} framebuffer {} created", vp_idx, frame_idx);
                                        }
                                        Err(e) => tracing::error!("viewport {} framebuffer create failed: {e}", vp_idx),
                                    }
                                }
                            }
                        }

                        let mut shadow_layout_opt = if shadow_map.is_some() { Some(shadow_layout) } else { None };
                        if scene_pipeline.is_some() && scene_depth_buffer.is_some() && scene_uniform_buffer.is_some() && scene_descriptor_set.is_some() {
                            // Clear swapchain once before any offscreen rendering.
                            for vp_idx in 0..num_viewports {
                                if let Some(ref fb) = viewport_framebuffers[vp_idx][frame_idx] {
                                    any_offscreen = true;
                                    if vp_idx == 0 {
                                        renderer.begin_scene_pass(cmd, scene_depth_buffer.as_ref().unwrap(), [0.05, 0.05, 0.05, 1.0]);
                                        renderer.end_scene_pass(cmd);
                                    }
                                    break;
                                }
                            }

                            // Render each viewport camera view to its own framebuffer.
                            for vp_idx in 0..num_viewports {
                                let rect_key = egui::Id::new(format!("viewport_rect_{}", vp_idx));
                                let viewport_rect: Option<egui::Rect> = egui_ctx.data(|d| d.get_temp(rect_key));
                                let has_valid = viewport_rect.is_some();
                                let offscreen_fb = if (screen == AppScreen::Editor || screen == AppScreen::PlayTest) && has_valid {
                                    viewport_framebuffers[vp_idx][frame_idx].as_ref()
                                } else {
                                    None
                                };

                                if let Some(ref fb) = offscreen_fb {
                                    let cam = &viewport_manager.viewports[vp_idx].camera;
                                    shadow_layout_opt = render::render_3d_scene(
                                        &renderer, cmd,
                                        scene_pipeline.as_ref().unwrap(),
                                        shadow_pipeline.as_ref(),
                                        scene_depth_buffer.as_ref().unwrap(),
                                        shadow_map.as_ref(),
                                        shadow_layout_opt,
                                        scene_uniform_buffer.as_ref().unwrap(),
                                        scene_descriptor_set.unwrap(),
                                        shadow_descriptor_set,
                                        &meshes, &ecs_world, cam,
                                        Some(fb),
                                    );
                                    let tex_id = ui::viewport::viewport_texture_id(vp_idx);
                                    tracing::trace!("main: registering viewport {} framebuffer color_view={:?} as user texture {:?}", vp_idx, fb.color_view, tex_id);
                                    egui_r.register_user_texture(
                                        tex_id,
                                        fb.color_view,
                                        egui_r.sampler(),
                                    );
                                    let valid_key = egui::Id::new(format!("viewport_offscreen_valid_{}", vp_idx));
                                    egui_ctx.data_mut(|d| d.insert_temp(valid_key, true));
                                } else {
                                    let valid_key = egui::Id::new(format!("viewport_offscreen_valid_{}", vp_idx));
                                    egui_ctx.data_mut(|d| d.remove_temp::<bool>(valid_key));
                                }
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
                                    ui::editor_screen(ctx, &mut viewport_manager, &mut window, &mut screen, &input, target, &ww, &wh, &mut fps, &open_project, &new_project, &proj_name_owned, &mut current_project, &mut project_dir, &mut ecs_world, &*selected_entity, &*pending_delete, &*dirty, &*show_confirm, &*confirm_target, &*show_settings, &*renaming, &*rename_buffer, &*undo_history, &mut sprite_editor, &pending_mesh_load, &mut audio_engine, &mut audio_instance, &mut waveform_viewer);
                                }
                                AppScreen::PlayTest => {
                                    // TODO: replace with dedicated play-test UI once mode is fully wired.
                                    let proj_name = current_project.as_ref().map(|p| p.name.as_str()).unwrap_or("Untitled");
                                    let proj_name_owned = proj_name.to_string();
                                    ui::editor_screen(ctx, &mut viewport_manager, &mut window, &mut screen, &input, target, &ww, &wh, &mut fps, &open_project, &new_project, &proj_name_owned, &mut current_project, &mut project_dir, &mut ecs_world, &*selected_entity, &*pending_delete, &*dirty, &*show_confirm, &*confirm_target, &*show_settings, &*renaming, &*rename_buffer, &*undo_history, &mut sprite_editor, &pending_mesh_load, &mut audio_engine, &mut audio_instance, &mut waveform_viewer);
                                }
                            }
                        });

                        if let Some(path) = open_project.borrow_mut().take() {
                            let dir = Path::new(&path);
                            let info = load_project_file(dir).or_else(|| create_project_file(dir, ProjectType::Dim3));
                            if let Some(ref proj_info) = info {
                                scene_to_world(&mut ecs_world, &proj_info.scene);
                                if let Some(ref cam_state) = proj_info.editor_camera {
                                    let cam = viewport_manager.primary_camera_mut();
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
            winit::event::Event::DeviceEvent { event: winit::event::DeviceEvent::MouseMotion { delta }, .. } => {
                input.push_event(rustix_platform::input::InputEvent::RawMouseMotion(delta.0 as f32, delta.1 as f32));
            }
            winit::event::Event::AboutToWait => {
                if screen == AppScreen::PlayTest {
                    window.request_redraw();
                } else {
                    let now = Instant::now();
                    let editor_interval = std::time::Duration::from_secs_f32(1.0 / 15.0);
                    if now >= next_frame_time {
                        next_frame_time = now + editor_interval;
                        window.request_redraw();
                    } else {
                        target.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(next_frame_time));
                    }
                }
            }
            _ => {}
        }
    });
}
