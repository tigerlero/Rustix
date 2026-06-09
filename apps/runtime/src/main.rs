use std::path::Path;
use std::time::Instant;

mod app_state;
mod camera;
mod combat;
mod enemy;
mod fonts;
mod gltf_loader;
mod init;
mod player;
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
use rustix_core::math::{Vec3, Mat4};
use rustix_platform::input::InputManager;
use rustix_platform::window::{WindowConfig, WindowHandle};
use rustix_render::{Renderer, DirectionalLight};
use rustix_animation::{Animator, update_animators};
use rustix_physics::{RigidBody, step_physics};

use rustix_asset::mmap::MappedFile;

use project::{AppScreen, ProjectType, load_project_file, create_project_file, add_recent_project, write_project_file};
use scene::{Transform, Name, MeshComponent, Material, world_transform, scene_to_world, world_to_scene};

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
            level: config.log_level(), crate_filters: vec![], json: false, json_file_path: None, json_max_size_mb: 10, json_max_backups: 3, thread_ids: true, targets: true, tracy_enabled: false,
        },
        Some(log_buffer.clone()),
    );
    tracing::info!("Rustix Editor");

    let el = match winit::event_loop::EventLoop::new() {
        Ok(el) => el,
        Err(e) => { eprintln!("Failed to create event loop: {e}"); std::process::exit(1); }
    };
    let wc = WindowConfig { title: "Rustix Editor".into(), width: 1280, height: 720, fullscreen: false, fullscreen_mode: rustix_platform::FullscreenMode::Windowed, resizable: true, decorations: true, cursor_mode: rustix_platform::window::CursorMode::Normal };
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
    let mut egui_r = match ui_renderer::EguiVulkanRenderer::new(&renderer, sc_format) {
        Ok(r) => r,
        Err(e) => { eprintln!("Failed to create egui renderer: {e}"); std::process::exit(1); }
    };

    let mut viewport_manager = ui::viewport::ViewportManager::new();
    let mut last = Instant::now();
    let mut next_frame_time = Instant::now();

    let mut fc = 0u64;
    let mut ft = Instant::now();
    let mut fps = 0u64;

    let mut app = app_state::AppState::new();
    let _ = std::fs::create_dir_all(&app.recording_dir);
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
                        for vp_idx in 0..app.viewport_fb_sizes.len() {
                            app.viewport_fb_sizes[vp_idx] = (0, 0);
                            for fb in &mut app.viewport_framebuffers[vp_idx] { *fb = None; }
                        }
                        tracing::info!("DPI scale factor changed, invalidating viewport framebuffers");
                    }
                    winit::event::WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = now.duration_since(last).as_secs_f32().min(0.1);
                        last = now;

                        // Capture all input events for recording
                        if app.input_recorder.mode() == rustix_platform::recorder::RecorderMode::Recording {
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
                            app.input_recorder.record(ev);
                        }

                        // Inject playback events
                        for ev in app.input_recorder.poll_playback() {
                            input.push_event(ev);
                        }
                        input.poll();

                        // Update player controllers (Tab cycles, WASD moves active player)
                        {
                            let cam = viewport_manager.primary_camera_mut();
                            let mut damage_events: Vec<combat::DamageEvent> = Vec::new();
                            player::update_players(
                                &mut app.ecs_world,
                                &mut app.player_manager,
                                cam,
                                &input,
                                dt,
                                &mut damage_events,
                            );

                            // Enemy AI
                            enemy::update_enemies(&mut app.ecs_world, dt, &mut damage_events);

                            // Tick cooldowns, resolve queued damage, cleanup dead
                            combat::tick_cooldowns(&mut app.ecs_world, dt);
                            combat::resolve_damage(&mut app.ecs_world, &damage_events);
                            let dead = combat::cleanup_dead(&mut app.ecs_world);
                            if !dead.is_empty() {
                                // Also remove from player manager if a player died
                                app.player_manager.players.retain(|p| !dead.contains(p));
                            }
                            // When a player is active, camera follows it and relinquishes WASD
                            let has_active = app.player_manager.active_entity().is_some();
                            cam.controlling_player = has_active;
                            cam.follow_target = has_active || !app.selected_entities.borrow().is_empty();
                        }

                        viewport_manager.primary_camera_mut().update(&input, dt);

                        // Input recorder controls
                        if input.keyboard().just_pressed(rustix_platform::input::KeyCode::F9) {
                            match app.input_recorder.mode() {
                                rustix_platform::recorder::RecorderMode::Idle => {
                                    app.input_recorder.start_recording();
                                    input.start_capture();
                                    tracing::info!("input recording started");
                                }
                                rustix_platform::recorder::RecorderMode::Recording => {
                                    input.stop_capture();
                                    let rec = app.input_recorder.stop_recording();
                                    let path = app.recording_dir.join("recording.json");
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
                            match app.input_recorder.mode() {
                                rustix_platform::recorder::RecorderMode::Idle => {
                                    let path = app.recording_dir.join("recording.json");
                                    if let Some(rec) = rustix_platform::recorder::load_recording(&path) {
                                        app.input_recorder.start_playback(rec);
                                        tracing::info!("input playback started from {}", path.display());
                                    } else {
                                        tracing::warn!("no recording found at {}", path.display());
                                    }
                                }
                                rustix_platform::recorder::RecorderMode::Playing |
                                rustix_platform::recorder::RecorderMode::Paused => {
                                    app.input_recorder.stop_playback();
                                    tracing::info!("input playback stopped");
                                }
                                _ => {}
                            }
                        }

                        let follow_pos = if let Some(pos) = player::active_player_position(&app.ecs_world, &app.player_manager) {
                            Some(pos)
                        } else {
                            app.selected_entities.borrow().first().and_then(|sel| {
                                let matrix = world_transform(&app.ecs_world, *sel);
                                let (_scale, _rot, pos) = matrix.to_scale_rotation_translation();
                                Some(pos)
                            })
                        };
                        viewport_manager.primary_camera_mut().follow(follow_pos);

                        // Update animations
                        {
                            let mut animators: Vec<(hecs::Entity, &mut Animator)> = Vec::new();
                            for (e, a) in app.ecs_world.query_mut::<(&hecs::Entity, &mut Animator)>() {
                                animators.push((*e, a));
                            }
                            let results = update_animators(&mut animators, &app.animation_clips, dt);
                            for (entity, pos, rot, scale) in results {
                                if let Ok(mut t) = app.ecs_world.get::<&mut Transform>(entity) {
                                    if let Some(p) = pos { t.position = p; }
                                    if let Some(r) = rot {
                                        let (x, y, z) = r.to_euler(rustix_core::math::EulerRot::XYZ);
                                        t.rotation = Vec3::new(x, y, z);
                                    }
                                    if let Some(s) = scale { t.scale = s; }
                                }
                            }
                        }

                        // Update physics
                        {
                            let mut bodies: Vec<(hecs::Entity, RigidBody)> = Vec::new();
                            for (e, b) in app.ecs_world.query_mut::<(&hecs::Entity, &RigidBody)>() {
                                bodies.push((*e, *b));
                            }
                            let results = step_physics(&mut bodies, &app.physics_world, dt);
                            for (entity, pos_delta, rot_delta) in results {
                                if let Ok(mut t) = app.ecs_world.get::<&mut Transform>(entity) {
                                    t.position += pos_delta;
                                    t.rotation += rot_delta;
                                }
                            }
                            for (entity, body) in bodies {
                                if let Ok(mut b) = app.ecs_world.get::<&mut RigidBody>(entity) {
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
                                Ok(db) => app.scene_depth_buffer = Some(db),
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

                        if app.scene_pipeline.is_none() {
                            init::init_scene_resources(
                                &renderer, &mut app.meshes,
                                &mut app.scene_pipeline, &mut app.scene_descriptor_pool, &mut app.scene_descriptor_set,
                                &mut app.scene_uniform_buffer, &mut app.scene_depth_buffer,
                                &mut app.shadow_pipeline, &mut app.shadow_descriptor_pool, &mut app.shadow_descriptor_set,
                                &mut app.csm_resources,
                                &mut app.point_shadow_resources,
                                &mut app.spot_shadow_resources,
                                &mut app.tonemap_pipeline, &mut app.tonemap_desc_set,
                                &mut app.bloom_extract_pipeline, &mut app.bloom_down_pipeline,
                                &mut app.bloom_up_pipeline, &mut app.bloom_desc_set,
                                &mut app.ssao_pipeline, &mut app.ssao_blur_pipeline,
                                &mut app.ssao_desc_set,
                                &mut app.taa_pipeline, &mut app.taa_desc_set,
                                &mut app.ssr_pipeline, &mut app.ssr_desc_set,
                                &mut app.fog_pipeline, &mut app.fog_desc_set,
                                &mut app.skybox_pipeline, &mut app.skybox_desc_set,
                                &mut app.instanced_pipeline, &mut app.instanced_gbuffer_pipeline,
                                &mut app.mesh_shader_pipeline,
                                &mut app.oit_accumulate_pipeline, &mut app.oit_composite_pipeline,
                                &mut app.oit_desc_set,
                            );
                        }

                        if app.fwd_plus_resources.is_none() {
                            match crate::render::ForwardPlusResources::new(&renderer) {
                                Ok(res) => app.fwd_plus_resources = Some(res),
                                Err(e) => tracing::error!("failed to create Forward+ resources: {e}"),
                            }
                        }

                        if app.gbuffer_resources.is_none() {
                            if let Some(ref depth) = app.scene_depth_buffer {
                                match crate::render::GBufferResources::new(&renderer, renderer.swapchain.lock().extent(), depth) {
                                    Ok(res) => app.gbuffer_resources = Some(res),
                                    Err(e) => tracing::error!("failed to create GBuffer resources: {e}"),
                                }
                            }
                        }

                        init::init_2d_resources(
                            &renderer,
                            &mut app.pipeline_2d, &mut app.ubo_2d, &mut app.desc_set_2d,
                            &mut app.quad_buffer_2d, &mut app.texture_2d,
                        );

                        // Shader hot-reload: poll file watcher and recreate affected pipelines.
                        // Skip on frame 0 to avoid processing initial file-watcher create events.
                        if renderer.frame_index() > 0 {
                            if let Some(reloader) = renderer.hot_reloader() {
                                for path in reloader.take_events() {
                                    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                    match file {
                                        "pbr.vert" | "pbr.frag" => {
                                            init::reload_scene_pipeline(&renderer, &mut app.scene_pipeline);
                                        }
                                        "shadow.vert" => {
                                            let bindless_layout = renderer.bindless_heap().layout();
                                            init::reload_shadow_pipeline(&renderer, &mut app.shadow_pipeline, bindless_layout);
                                        }
                                        "tonemap.vert" | "tonemap.frag" => {
                                            init::reload_tonemap_pipeline(&renderer, &mut app.tonemap_pipeline, &mut app.tonemap_desc_set);
                                        }
                                        "bloom.vert" | "bloom_extract.frag" => {
                                            init::reload_bloom_extract_pipeline(&renderer, &mut app.bloom_extract_pipeline, &mut app.bloom_desc_set);
                                        }
                                        "bloom_down.frag" => {
                                            init::reload_bloom_down_pipeline(&renderer, &mut app.bloom_down_pipeline);
                                        }
                                        "bloom_up.frag" => {
                                            init::reload_bloom_up_pipeline(&renderer, &mut app.bloom_up_pipeline);
                                        }
                                        "ssao.vert" | "ssao.frag" => {
                                            init::reload_ssao_pipeline(&renderer, &mut app.ssao_pipeline, &mut app.ssao_desc_set);
                                        }
                                        "ssao_blur.frag" => {
                                            init::reload_ssao_blur_pipeline(&renderer, &mut app.ssao_blur_pipeline);
                                        }
                                        "taa.vert" | "taa.frag" => {
                                            init::reload_taa_pipeline(&renderer, &mut app.taa_pipeline, &mut app.taa_desc_set);
                                        }
                                        "sprite.vert" | "sprite.frag" => {
                                            init::reload_2d_pipeline(&renderer, &mut app.pipeline_2d, &mut app.desc_set_2d);
                                        }
                                        "ssr.vert" | "ssr.frag" => {
                                            init::reload_ssr_pipeline(&renderer, &mut app.ssr_pipeline, &mut app.ssr_desc_set);
                                        }
                                        "volumetric_fog.vert" | "volumetric_fog.frag" => {
                                            init::reload_fog_pipeline(&renderer, &mut app.fog_pipeline, &mut app.fog_desc_set);
                                        }
                                        "skybox.vert" | "skybox.frag" => {
                                            init::reload_skybox_pipeline(&renderer, &mut app.skybox_pipeline, &mut app.skybox_desc_set);
                                        }
                                        "pbr_instanced.vert" | "pbr_instanced.frag" => {
                                            init::reload_instanced_pipeline(&renderer, &mut app.instanced_pipeline);
                                        }
                                        "gbuffer_instanced.vert" | "gbuffer_instanced.frag" => {
                                            init::reload_instanced_gbuffer_pipeline(&renderer, &mut app.instanced_gbuffer_pipeline);
                                        }
                                        "cull_instances.comp" | "gen_draw_cmds.comp" => {
                                            renderer.compute_pipeline_cache().clear();
                                        }
                                        "pbr_mesh.mesh" => {
                                            init::reload_mesh_shader_pipeline(&renderer, &mut app.mesh_shader_pipeline);
                                        }
                                        "light_cull.comp" => {
                                            // Clear compute pipeline cache so the Forward+ compute pipeline is recreated next frame.
                                            renderer.compute_pipeline_cache().clear();
                                        }
                                        "gbuffer.vert" | "gbuffer.frag" => {
                                        if let Some(ref mut gbuf) = app.gbuffer_resources {
                                            let bindless_layout = renderer.bindless_heap().layout();
                                            match (
                                                rustix_render::shader::builtin::gbuffer_vertex_shader_override(renderer.device().logical()),
                                                rustix_render::shader::builtin::gbuffer_fragment_shader_override(renderer.device().logical()),
                                            ) {
                                                (Ok(vs), Ok(fs)) => {
                                                    match rustix_render::pipeline::GBufferPipeline::create(renderer.device(), &vs, &fs, bindless_layout) {
                                                        Ok(p) => gbuf.gbuffer_pipeline = p,
                                                        Err(e) => tracing::error!("gbuffer pipeline reload failed: {e}"),
                                                    }
                                                }
                                                (Err(e), _) | (_, Err(e)) => tracing::error!("gbuffer shader reload failed: {e}"),
                                            }
                                        }
                                    }
                                    "deferred.vert" | "deferred.frag" => {
                                        if let Some(ref mut gbuf) = app.gbuffer_resources {
                                            let bindless_layout = renderer.bindless_heap().layout();
                                            match (
                                                rustix_render::shader::builtin::deferred_vertex_shader_override(renderer.device().logical()),
                                                rustix_render::shader::builtin::deferred_fragment_shader_override(renderer.device().logical()),
                                            ) {
                                                (Ok(vs), Ok(fs)) => {
                                                    match rustix_render::pipeline::DeferredLightingPipeline::create(renderer.device(), &vs, &fs, bindless_layout) {
                                                        Ok(p) => gbuf.deferred_pipeline = p,
                                                        Err(e) => tracing::error!("deferred pipeline reload failed: {e}"),
                                                    }
                                                }
                                                (Err(e), _) | (_, Err(e)) => tracing::error!("deferred shader reload failed: {e}"),
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                        if let Some(path) = app.pending_mesh_load.borrow_mut().take() {
                            if let Ok(data) = MappedFile::open(Path::new(&path)) {
                                let mesh_name = Path::new(&path)
                                    .file_stem().and_then(|s| s.to_str()).unwrap_or("Imported")
                                    .to_string();
                                if let Ok(result) = gltf_loader::load_glb(&renderer, &data, &mesh_name) {
                                    tracing::info!("loaded mesh {mesh_name} from {path} (base={:?} rough={:.2} metal={:.2})",
                                        result.base_color, result.roughness, result.metallic);
                                    app.meshes.insert(mesh_name.clone(), result.mesh);
                                    let e = app.ecs_world.spawn((
                                        Transform { position: Vec3::new(0.0, 1.0, 0.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
                                        Name(mesh_name.clone()),
                                        MeshComponent(mesh_name),
                                        Material { base_color: Vec3::from(result.base_color), alpha: 1.0, roughness: result.roughness, metallic: result.metallic, ao: 1.0, emissive: 0.0 },
                                    ));
                                    *app.selected_entities.borrow_mut() = vec![e];
                                    app.dirty.set(true);
                                } else {
                                    tracing::error!("failed to load mesh from {path}");
                                }
                            } else {
                                tracing::error!("failed to read file {path}");
                            }
                        }

                        // Multi-viewport: determine size and create framebuffers for each open viewport.
                        let frame_idx = renderer.frame_index() % 3;
                        let mut _any_offscreen = false;
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

                            if (app.screen == AppScreen::Editor || app.screen == AppScreen::PlayTest) && has_valid {
                                if app.viewport_fb_sizes[vp_idx] != (vp_w, vp_h) {
                                    tracing::trace!("main: recreating viewport {} framebuffers {}x{}", vp_idx, vp_w, vp_h);
                                    for fb in &mut app.viewport_framebuffers[vp_idx] { *fb = None; }
                                    app.viewport_fb_sizes[vp_idx] = (vp_w, vp_h);
                                }
                                if app.viewport_framebuffers[vp_idx][frame_idx].is_none() {
                                    match renderer.create_framebuffer(vp_w, vp_h, sc_format) {
                                        Ok(fb) => {
                                            app.viewport_framebuffers[vp_idx][frame_idx] = Some(fb);
                                            tracing::trace!("main: viewport {} framebuffer {} created", vp_idx, frame_idx);
                                        }
                                        Err(e) => tracing::error!("viewport {} framebuffer create failed: {e}", vp_idx),
                                    }
                                }
                            }
                        }

                        // Ensure HDR framebuffer and GBuffer match swapchain size.
                        let sw_extent = renderer.swapchain.lock().extent();
                        if app.hdr_fb_size != (sw_extent.width, sw_extent.height) {
                            if let Some(mut br) = app.bloom_resources.take() {
                                br.destroy(renderer.device().logical());
                            }
                            if let Some(mut sr) = app.ssao_resources.take() {
                                sr.destroy(renderer.device().logical());
                            }
                            if let Some(mut tr) = app.taa_resources.take() {
                                tr.destroy(renderer.device().logical());
                            }
                            if let Some(mut sr) = app.ssr_resources.take() {
                                sr.destroy(renderer.device().logical());
                            }
                            if let Some(mut fr) = app.fog_resources.take() {
                                fr.destroy(renderer.device().logical());
                            }
                            if let Some(mut sb) = app.skybox_resources.take() {
                                sb.destroy(renderer.device().logical());
                            }
                            app.hdr_framebuffer = None;
                            app.hdr_fb_size = (sw_extent.width, sw_extent.height);
                            app.gbuffer_resources = None; // Will be recreated below
                        }
                        if app.hdr_framebuffer.is_none() {
                            match rustix_render::HdrFramebuffer::new(&renderer, sw_extent.width, sw_extent.height) {
                                Ok(hfb) => { app.hdr_framebuffer = Some(hfb); }
                                Err(e) => tracing::error!("HDR framebuffer creation failed: {e}"),
                            }
                        }
                        if app.bloom_resources.is_none() {
                            match crate::render::BloomResources::new(&renderer, sw_extent.width, sw_extent.height) {
                                Ok(br) => {
                                    app.bloom_resources = Some(br);
                                }
                                Err(e) => tracing::error!("Bloom resources creation failed: {e}"),
                            }
                        }
                        if app.oit_resources.is_none() {
                            match crate::render::OitResources::new(&renderer, sw_extent.width, sw_extent.height) {
                                Ok(or) => {
                                    app.oit_resources = Some(or);
                                }
                                Err(e) => tracing::error!("OIT resources creation failed: {e}"),
                            }
                        }
                        if app.ssao_resources.is_none() {
                            match crate::render::SsaoResources::new(&renderer, sw_extent.width, sw_extent.height) {
                                Ok(sr) => {
                                    app.ssao_resources = Some(sr);
                                }
                                Err(e) => tracing::error!("SSAO resources creation failed: {e}"),
                            }
                        }
                        if app.taa_resources.is_none() {
                            match crate::render::TaaResources::new(&renderer, sw_extent.width, sw_extent.height) {
                                Ok(tr) => {
                                    app.taa_resources = Some(tr);
                                }
                                Err(e) => tracing::error!("TAA resources creation failed: {e}"),
                            }
                        }
                        if app.ssr_resources.is_none() {
                            match crate::render::SsrResources::new(&renderer, sw_extent.width, sw_extent.height) {
                                Ok(sr) => {
                                    app.ssr_resources = Some(sr);
                                }
                                Err(e) => tracing::error!("SSR resources creation failed: {e}"),
                            }
                        }
                        if app.fog_resources.is_none() {
                            match crate::render::VolumetricFogResources::new(&renderer, sw_extent.width, sw_extent.height) {
                                Ok(fr) => {
                                    app.fog_resources = Some(fr);
                                }
                                Err(e) => tracing::error!("Fog resources creation failed: {e}"),
                            }
                        }
                        if app.skybox_resources.is_none() {
                            match crate::render::SkyboxResources::new(&renderer, sw_extent.width, sw_extent.height) {
                                Ok(sb) => {
                                    app.skybox_resources = Some(sb);
                                }
                                Err(e) => tracing::error!("Skybox resources creation failed: {e}"),
                            }
                        }
                        if app.gbuffer_resources.is_none() {
                            if let Some(ref depth) = app.scene_depth_buffer {
                                match crate::render::GBufferResources::new(&renderer, sw_extent, depth) {
                                    Ok(res) => app.gbuffer_resources = Some(res),
                                    Err(e) => tracing::error!("failed to create GBuffer resources: {e}"),
                                }
                            }
                        }
                        if app.instanced_batcher.is_none() {
                            match crate::render::InstancedMeshBatcher::new(
                                renderer.device(), &mut renderer.allocator.lock(), 4096, 256,
                            ) {
                                Ok(b) => app.instanced_batcher = Some(b),
                                Err(e) => tracing::error!("Instanced batcher creation failed: {e}"),
                            }
                        }
                        if app.gpu_culling_resources.is_none() {
                            match crate::render::GpuCullingResources::new(
                                &renderer, renderer.device(), 4096, 256,
                            ) {
                                Ok(r) => app.gpu_culling_resources = Some(r),
                                Err(e) => tracing::error!("GPU culling resources creation failed: {e}"),
                            }
                        }

                        let mut shadow_layout_opt = if app.csm_resources.is_some() { Some(app.shadow_layout) } else { None };
                        if app.scene_pipeline.is_some() && app.scene_depth_buffer.is_some() && app.scene_uniform_buffer.is_some() {
                            // Clear swapchain once before any offscreen rendering.
                            for vp_idx in 0..num_viewports {
                                if let Some(ref _fb) = app.viewport_framebuffers[vp_idx][frame_idx] {
                                    _any_offscreen = true;
                                    if vp_idx == 0 {
                                        renderer.begin_scene_pass(cmd, app.scene_depth_buffer.as_ref().unwrap(), [0.05, 0.05, 0.05, 1.0]);
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
                                let offscreen_fb = if (app.screen == AppScreen::Editor || app.screen == AppScreen::PlayTest) && has_valid {
                                    app.viewport_framebuffers[vp_idx][frame_idx].as_ref()
                                } else {
                                    None
                                };

                                if let Some(ref fb) = offscreen_fb {
                                    let cam = &viewport_manager.viewports[vp_idx].camera;
                                    // Compute CSM cascades for viewport
                                    if let Some(ref mut c) = app.csm_resources {
                                        let aspect = fb.extent.width as f32 / fb.extent.height as f32;
                                        let cam_view = match cam.mode {
                                            crate::camera::CameraMode::Orbit => Mat4::look_at_rh(cam.eye_pos(), cam.center, Vec3::Y),
                                            crate::camera::CameraMode::FirstPerson => {
                                                let forward = Vec3::new(cam.pitch.cos() * cam.yaw.sin(), cam.pitch.sin(), cam.pitch.cos() * cam.yaw.cos());
                                                Mat4::look_at_rh(cam.position, cam.position + forward, Vec3::Y)
                                            }
                                        };
                                        let cam_proj = Mat4::perspective_rh_gl(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);
                                        let light_dir = {
                                            let mut d = Vec3::new(0.5, 0.8, 0.3);
                                            for (_dirlight, xform) in app.ecs_world.query::<(&DirectionalLight, &Transform)>().iter() {
                                                d = render::directional_light_dir_from_euler(xform.rotation);
                                                break;
                                            }
                                            d
                                        };
                                        c.compute_cascades(&cam_view, &cam_proj, light_dir);
                                        c.upload_ubo();
                                    }
                                    shadow_layout_opt = render::render_3d_scene(
                                        &renderer, cmd,
                                        app.scene_pipeline.as_ref().unwrap(),
                                        app.shadow_pipeline.as_ref(),
                                        app.scene_depth_buffer.as_ref().unwrap(),
                                        app.csm_resources.as_ref(),
                                        app.point_shadow_resources.as_ref(),
                                        app.spot_shadow_resources.as_mut(),
                                        shadow_layout_opt,
                                        app.scene_uniform_buffer.as_ref().unwrap(),
                                        &app.meshes, &app.ecs_world, cam,
                                        Some(fb),
                                        None,
                                        app.fwd_plus_resources.as_ref(),
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
                                } else if vp_idx == 0 && app.hdr_framebuffer.is_some() {
                                    // Primary viewport with no offscreen: render to HDR, then tone-map to swapchain.
                                    let cam = &viewport_manager.viewports[vp_idx].camera;
                                    let hfb = app.hdr_framebuffer.as_ref().unwrap();
                                    let use_deferred = false; // Toggle: set true for deferred shading
                                    if app.instanced_enabled {
                                        if let Some(ref mut batcher) = app.instanced_batcher {
                                            let aspect = hfb.extent.width as f32 / hfb.extent.height as f32;
                                            let frustum = rustix_core::math::Frustum::from_view_proj(&cam.view_proj(aspect));
                                            batcher.build(&app.ecs_world, &app.meshes, &frustum);
                                            if let Some(ref mut cull_res) = app.gpu_culling_resources {
                                                let mut cull_instances: Vec<crate::render::CullInstance> = Vec::with_capacity(batcher.instance_buffer.capacity);
                                                let mut batch_infos: Vec<crate::render::BatchInfo> = Vec::with_capacity(batcher.batches.len());
                                                for (batch_idx, batch) in batcher.batches.iter().enumerate() {
                                                    if let Some(mesh) = app.meshes.get(&batch.mesh_name) {
                                                        let start = batch.instance_offset as usize;
                                                        let end = (batch.instance_offset + batch.instance_count) as usize;
                                                        for inst in &batcher.cpu_instances[start..end] {
                                                            let cull = crate::render::CullInstance::from_instance_data(
                                                                inst, mesh.aabb.min, mesh.aabb.max, batch_idx as u32,
                                                            );
                                                            cull_instances.push(cull);
                                                        }
                                                        batch_infos.push(crate::render::BatchInfo {
                                                            mesh_index: batch_idx as u32,
                                                            instance_offset: batch.instance_offset,
                                                            instance_count: batch.instance_count,
                                                            index_count: mesh.index_count,
                                                        });
                                                    }
                                                }
                                                cull_res.write_input(&cull_instances);
                                                cull_res.write_batch_info(&batch_infos);
                                            }
                                        }
                                    }
                                    let (new_layout, snapshot, _view_proj) = if use_deferred && app.gbuffer_resources.is_some() {
                                        render::render_deferred_with_graph(
                                            &renderer, cmd,
                                            app.scene_pipeline.as_ref().unwrap(),
                                            app.shadow_pipeline.as_ref(),
                                            app.scene_depth_buffer.as_ref().unwrap(),
                                            app.csm_resources.as_mut(),
                                            app.point_shadow_resources.as_ref(),
                                            app.spot_shadow_resources.as_mut(),
                                            shadow_layout_opt,
                                            app.scene_uniform_buffer.as_ref().unwrap(),
                                            &app.meshes, &app.ecs_world, cam,
                                            hfb,
                                            app.tonemap_pipeline.as_ref().unwrap(),
                                            app.tonemap_desc_set.unwrap(),
                                            egui_r.sampler(),
                                            app.gbuffer_resources.as_ref().unwrap(),
                                            app.fwd_plus_resources.as_ref(),
                                        )
                                    } else {
                                        render::render_hdr_with_graph(
                                            &renderer, cmd,
                                            app.scene_pipeline.as_ref().unwrap(),
                                            app.shadow_pipeline.as_ref(),
                                            app.scene_depth_buffer.as_ref().unwrap(),
                                            app.csm_resources.as_mut(),
                                            app.point_shadow_resources.as_ref(),
                                            app.spot_shadow_resources.as_mut(),
                                            shadow_layout_opt,
                                            app.scene_uniform_buffer.as_ref().unwrap(),
                                            &app.meshes, &app.ecs_world, cam,
                                            hfb,
                                            app.tonemap_pipeline.as_ref().unwrap(),
                                            app.tonemap_desc_set.unwrap(),
                                            egui_r.sampler(),
                                            app.fwd_plus_resources.as_ref(),
                                            app.bloom_resources.as_ref(),
                                            app.bloom_extract_pipeline.as_ref(),
                                            app.bloom_down_pipeline.as_ref(),
                                            app.bloom_up_pipeline.as_ref(),
                                            app.bloom_desc_set,
                                            app.bloom_threshold,
                                            app.bloom_intensity,
                                            app.ssao_resources.as_ref(),
                                            app.ssao_pipeline.as_ref(),
                                            app.ssao_blur_pipeline.as_ref(),
                                            app.ssao_desc_set,
                                            app.ssao_enabled,
                                            app.ssao_radius,
                                            app.ssao_bias,
                                            app.ssao_power,
                                            app.ssao_intensity,
                                            app.taa_resources.as_ref(),
                                            app.taa_pipeline.as_ref(),
                                            app.taa_desc_set,
                                            app.taa_enabled,
                                            app.taa_blend_factor,
                                            &mut app.prev_view_proj,
                                            app.ssr_resources.as_ref(),
                                            app.ssr_pipeline.as_ref(),
                                            app.ssr_desc_set,
                                            app.ssr_enabled,
                                            app.ssr_max_steps,
                                            app.ssr_stride,
                                            app.ssr_max_dist,
                                            app.gbuffer_resources.as_ref(),
                                            app.fog_resources.as_ref(),
                                            app.fog_pipeline.as_ref(),
                                            app.fog_desc_set,
                                            app.fog_enabled,
                                            app.fog_density,
                                            app.fog_scattering,
                                            app.fog_height_falloff,
                                            app.fog_max_dist,
                                            app.fog_max_steps,
                                            app.fog_sun_intensity,
                                            app.skybox_resources.as_ref(),
                                            app.skybox_pipeline.as_ref(),
                                            app.skybox_desc_set,
                                            app.skybox_enabled,
                                            app.skybox_rayleigh,
                                            app.skybox_mie,
                                            app.skybox_zenith_shift,
                                            app.skybox_exposure,
                                            app.instanced_pipeline.as_ref(),
                                            app.instanced_batcher.as_ref(),
                                            app.instanced_enabled,
                                            app.gpu_culling_resources.as_ref(),
                                            app.gpu_culling_enabled,
                                            app.mesh_shader_pipeline.as_ref(),
                                            app.mesh_shader_enabled,
                                            app.oit_resources.as_ref(),
                                            app.oit_enabled,
                                            app.oit_accumulate_pipeline.as_ref(),
                                            app.oit_composite_pipeline.as_ref(),
                                            app.oit_desc_set,
                                        )
                                    };
                                    shadow_layout_opt = new_layout;
                                    app.frame_graph_snapshot = snapshot;

                                    // After TAA, copy resolved output to history for next frame
                                    if app.taa_enabled && app.taa_resources.is_some() {
                                        let taa = app.taa_resources.as_ref().unwrap();
                                        let resolved = taa.resolved_image;
                                        let history = taa.history_image;
                                        let extent = taa.extent;
                                        unsafe {
                                            let device = renderer.device().logical();
                                            let resolved_barrier = vk::ImageMemoryBarrier2::default()
                                                .image(resolved)
                                                .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                                                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                                                .src_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
                                                .dst_stage_mask(vk::PipelineStageFlags2::COPY)
                                                .src_access_mask(vk::AccessFlags2::SHADER_READ)
                                                .dst_access_mask(vk::AccessFlags2::TRANSFER_READ)
                                                .subresource_range(vk::ImageSubresourceRange {
                                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                                    base_mip_level: 0, level_count: 1,
                                                    base_array_layer: 0, layer_count: 1,
                                                });
                                            let history_barrier = vk::ImageMemoryBarrier2::default()
                                                .image(history)
                                                .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                                                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                                                .src_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
                                                .dst_stage_mask(vk::PipelineStageFlags2::COPY)
                                                .src_access_mask(vk::AccessFlags2::SHADER_READ)
                                                .dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                                                .subresource_range(vk::ImageSubresourceRange {
                                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                                    base_mip_level: 0, level_count: 1,
                                                    base_array_layer: 0, layer_count: 1,
                                                });
                                            let barriers = [resolved_barrier, history_barrier];
                                            let dep = vk::DependencyInfo::default().image_memory_barriers(&barriers);
                                            device.cmd_pipeline_barrier2(cmd, &dep);

                                            let copy = vk::ImageCopy::default()
                                                .src_subresource(vk::ImageSubresourceLayers {
                                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                                    mip_level: 0,
                                                    base_array_layer: 0,
                                                    layer_count: 1,
                                                })
                                                .dst_subresource(vk::ImageSubresourceLayers {
                                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                                    mip_level: 0,
                                                    base_array_layer: 0,
                                                    layer_count: 1,
                                                })
                                                .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 });
                                            device.cmd_copy_image(cmd, resolved, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, history, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy]);

                                            let resolved_restore = vk::ImageMemoryBarrier2::default()
                                                .image(resolved)
                                                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                                                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                                                .src_stage_mask(vk::PipelineStageFlags2::COPY)
                                                .dst_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
                                                .src_access_mask(vk::AccessFlags2::TRANSFER_READ)
                                                .dst_access_mask(vk::AccessFlags2::SHADER_READ)
                                                .subresource_range(vk::ImageSubresourceRange {
                                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                                    base_mip_level: 0, level_count: 1,
                                                    base_array_layer: 0, layer_count: 1,
                                                });
                                            let history_restore = vk::ImageMemoryBarrier2::default()
                                                .image(history)
                                                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                                                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                                                .src_stage_mask(vk::PipelineStageFlags2::COPY)
                                                .dst_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
                                                .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                                                .dst_access_mask(vk::AccessFlags2::SHADER_READ)
                                                .subresource_range(vk::ImageSubresourceRange {
                                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                                    base_mip_level: 0, level_count: 1,
                                                    base_array_layer: 0, layer_count: 1,
                                                });
                                            let restore_barriers = [resolved_restore, history_restore];
                                            let dep2 = vk::DependencyInfo::default().image_memory_barriers(&restore_barriers);
                                            device.cmd_pipeline_barrier2(cmd, &dep2);
                                        }
                                    }

                                    let valid_key = egui::Id::new(format!("viewport_offscreen_valid_{}", vp_idx));
                                    egui_ctx.data_mut(|d| d.remove_temp::<bool>(valid_key));
                                } else {
                                    let valid_key = egui::Id::new(format!("viewport_offscreen_valid_{}", vp_idx));
                                    egui_ctx.data_mut(|d| d.remove_temp::<bool>(valid_key));
                                }
                            }

                            // Tonemap is now handled inside render_hdr_with_graph via the declarative frame graph.
                        } else if renderer.frame_index() % 60 == 0 {
                            tracing::warn!("3D scene skipped: pipeline={} depth={} ubo={}",
                                app.scene_pipeline.is_some(), app.scene_depth_buffer.is_some(),
                                app.scene_uniform_buffer.is_some());
                        }

                        // 2D debug overlay disabled — viewport is for 3D scene only.
                        // if let (Some(ref ppl), Some(ref buf), Some(ref ubo), Some(ref tex), Some(ds)) =
                        //     (&app.pipeline_2d, &app.quad_buffer_2d, &app.ubo_2d, &app.texture_2d, app.desc_set_2d)
                        // {
                        //     render::render_2d_overlay(
                        //         &renderer, cmd, ppl, buf, ubo, tex, ds, start_time,
                        //     );
                        // }

                        // Process egui and upload textures DURING command buffer recording.
                        // update_textures now waits for GPU idle before partial updates to avoid
                        // layout-transition races with in-flight frames.
                        let raw_input = egui_state.take_egui_input(window.inner());
                        let out = egui_ctx.run(raw_input, |ctx| {
                            if ctx.input(|i| i.key_pressed(egui::Key::F10)) {
                                app.show_frame_graph_overlay = !app.show_frame_graph_overlay;
                            }
                            match app.screen {
                                AppScreen::Startup => {
                                    ui::startup_screen(ctx, &app.recent_projects, &mut app.screen, &app.open_project, &app.new_project, &*app.show_new_project_type, &*app.new_project_type, &*app.show_settings);
                                }
                                AppScreen::Editor => {
                                    let proj_name = app.current_project.as_ref().map(|p| p.name.as_str()).unwrap_or("Untitled");
                                    let proj_name_owned = proj_name.to_string();
                                    ui::editor_screen(ctx, &mut viewport_manager, &mut window, &mut app.screen, &input, target, &ww, &wh, &mut fps, &app.open_project, &app.new_project, &proj_name_owned, &mut app.current_project, &mut app.project_dir, &mut app.ecs_world, &*app.selected_entities, &*app.pending_delete, &*app.dirty, &*app.show_confirm, &*app.confirm_target, &*app.show_settings, &*app.renaming, &*app.rename_buffer, &*app.undo_history, &mut app.sprite_editor, &app.pending_mesh_load, &mut app.audio_engine, &mut app.audio_instance, &mut app.waveform_viewer);
                                }
                                AppScreen::PlayTest => {
                                    let proj_name = app.current_project.as_ref().map(|p| p.name.as_str()).unwrap_or("Untitled");
                                    let proj_name_owned = proj_name.to_string();
                                    ui::editor_screen(ctx, &mut viewport_manager, &mut window, &mut app.screen, &input, target, &ww, &wh, &mut fps, &app.open_project, &app.new_project, &proj_name_owned, &mut app.current_project, &mut app.project_dir, &mut app.ecs_world, &*app.selected_entities, &*app.pending_delete, &*app.dirty, &*app.show_confirm, &*app.confirm_target, &*app.show_settings, &*app.renaming, &*app.rename_buffer, &*app.undo_history, &mut app.sprite_editor, &app.pending_mesh_load, &mut app.audio_engine, &mut app.audio_instance, &mut app.waveform_viewer);
                                }
                            }
                            if app.show_frame_graph_overlay {
                                if let Some(ref snap) = app.frame_graph_snapshot {
                                    ui::show_frame_graph_overlay(ctx, &mut app.show_frame_graph_overlay, snap);
                                }
                            }
                            if app.show_settings.get() {
                                egui::Window::new("Settings")
                                    .default_pos([100.0, 80.0])
                                    .default_size([860.0, 520.0])
                                    .show(ctx, |ui| {
                                        ui::post_process_panel(ui, &mut app);
                                        if let Some(ref mut proj) = app.current_project {
                                            ui.separator();
                                            ui.label(egui::RichText::new("Project").size(14.0).strong());
                                            ui.add_space(8.0);
                                            ui.horizontal(|ui| {
                                                ui.add(egui::DragValue::new(&mut proj.settings.resolution_width).prefix("Width: ").range(320..=7680));
                                                ui.add(egui::DragValue::new(&mut proj.settings.resolution_height).prefix("Height: ").range(240..=4320));
                                            });
                                            ui.add_space(8.0);
                                            ui.add(egui::Checkbox::new(&mut proj.settings.enable_vsync, "Enable V-Sync"));
                                            ui.add_space(8.0);
                                            ui.add(egui::DragValue::new(&mut proj.settings.target_fps).prefix("Target FPS: ").range(30..=480));
                                            ui.add_space(8.0);
                                            let mut is_3d = proj.settings.project_type == crate::project::ProjectType::Dim3;
                                            ui.horizontal(|ui| {
                                                ui.label("Project type:");
                                                if ui.selectable_label(is_3d, "3D").clicked() { is_3d = true; }
                                                if ui.selectable_label(!is_3d, "2D").clicked() { is_3d = false; }
                                            });
                                            proj.settings.project_type = if is_3d { crate::project::ProjectType::Dim3 } else { crate::project::ProjectType::Dim2 };
                                        }
                                    });
                            }
                        });

                        if let Some(path) = app.open_project.borrow_mut().take() {
                            let dir = Path::new(&path);
                            let info = load_project_file(dir).or_else(|| create_project_file(dir, ProjectType::Dim3));
                            if let Some(ref proj_info) = info {
                                scene_to_world(&mut app.ecs_world, &proj_info.scene);
                                app.selected_entities.borrow_mut().clear();
                                tracing::info!("loaded project with {} entities", app.ecs_world.query::<(&Name,)>().iter().count());
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

                                // Restore layout
                                let gen = egui_ctx.data(|d| d.get_temp::<u64>(egui::Id::new("layout_generation")).unwrap_or(0)).wrapping_add(1);
                                egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new("layout_generation"), gen));
                                if let Some(ref layout) = proj_info.layout {
                                    egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new("hierarchy_width"), layout.hierarchy_width));
                                    egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new("inspector_width"), layout.inspector_width));
                                    egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new("console_height"), layout.console_height));
                                    if !layout.viewports.is_empty() {
                                        let primary = viewport_manager.viewports.remove(0);
                                        viewport_manager.viewports.clear();
                                        viewport_manager.viewports.push(primary);
                                        for (_i, vp_layout) in layout.viewports.iter().enumerate().skip(1) {
                                            if let Some(idx) = viewport_manager.add_viewport() {
                                                if let Some(vp) = viewport_manager.viewports.get_mut(idx) {
                                                    vp.name = vp_layout.name.clone();
                                                    vp.open = vp_layout.open;
                                                }
                                                if let Some(pos) = vp_layout.position {
                                                    egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new(format!("viewport_pos_{}", idx)), egui::pos2(pos[0], pos[1])));
                                                }
                                                if let Some(size) = vp_layout.size {
                                                    egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new(format!("viewport_size_{}", idx)), egui::vec2(size[0], size[1])));
                                                }
                                            }
                                        }
                                    }
                                }

                                app.current_project = info;
                                app.project_dir = Some(path.clone());
                                add_recent_project(&mut app.recent_projects, path, &app.current_project);
                                app.screen = AppScreen::Editor;
                                next_frame_time = Instant::now();
                                window.request_redraw();
                            }
                        }
                        if let Some(path) = app.new_project.borrow_mut().take() {
                            let dir = Path::new(&path);
                            let ptype = app.new_project_type.get();
                            let mut info = create_project_file(dir, ptype);
                            if let Some(ref mut proj) = info {
                                app.ecs_world.clear();
                                app.selected_entities.borrow_mut().clear();
                                app.ecs_world.spawn((
                                    Transform { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE },
                                    Name("Cube".into()),
                                    MeshComponent("Cube".into()),
                                    Material { base_color: Vec3::new(0.7, 0.7, 0.7), alpha: 1.0, roughness: 0.5, metallic: 0.0, ao: 1.0, emissive: 0.0 },
                                ));
                                app.ecs_world.spawn((
                                    Transform { position: Vec3::new(5.0, 10.0, 5.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
                                    Name("Light".into()),
                                    DirectionalLight { color: Vec3::new(1.0, 1.0, 1.0), intensity: 1.0 },
                                ));
                                proj.scene = world_to_scene(&app.ecs_world);
                                let _ = write_project_file(dir, proj);
                                app.current_project = info;
                                app.project_dir = Some(path.clone());
                                add_recent_project(&mut app.recent_projects, path, &app.current_project);
                                let gen = egui_ctx.data(|d| d.get_temp::<u64>(egui::Id::new("layout_generation")).unwrap_or(0)).wrapping_add(1);
                                egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new("layout_generation"), gen));
                                app.screen = AppScreen::Editor;
                                next_frame_time = Instant::now();
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
                        if renderer.frame_index() % 60 == 0 {
                            tracing::info!("frame {} rendered", renderer.frame_index());
                        }
                        input.end_tick();

                        fc += 1;
                        if ft.elapsed().as_secs_f32() >= 1.0 {
                            fps = fc;
                            fc = 0;
                            ft = Instant::now();
                            let proj_label = app.current_project.as_ref().map(|p| p.name.as_str()).unwrap_or("Rustix Editor");
                            let star = if app.dirty.get() { " *" } else { "" };
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
                if app.screen == AppScreen::PlayTest {
                    window.request_redraw();
                } else {
                    let now = Instant::now();
                    let editor_interval = std::time::Duration::from_secs_f32(1.0 / 60.0);
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
