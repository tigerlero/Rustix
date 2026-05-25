use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

mod gltf_loader;
mod sprite_editor;
mod ui_renderer;

use ash::vk;
use rustix_core::config;
use rustix_core::ecs::{EcsWorld, Entity};
use rustix_core::math::{Vec3, Vec4, Mat4, Quat, EulerRot};
use rustix_platform::input::{InputManager, KeyCode};
use rustix_platform::window::{WindowConfig, WindowHandle};
use rustix_render::{Renderer, DirectionalLight, PointLight, SpotLight};
use rustix_render::mesh::Mesh;

const PROJECT_FILE: &str = "project.rustixproj";
const RECENT_FILE: &str = "recent_projects.json";

#[derive(Clone, Copy, PartialEq)]
enum CameraMode {
    Orbit,
    FirstPerson,
}

struct EditorCamera {
    position: Vec3,
    center: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
    mode: CameraMode,
    follow_target: bool,
}

impl EditorCamera {
    fn new() -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 5.0),
            center: Vec3::ZERO,
            yaw: 0.0,
            pitch: -0.3,
            distance: 8.0,
            mode: CameraMode::Orbit,
            follow_target: false,
        }
    }

    fn view_proj(&self, aspect: f32) -> Mat4 {
        let proj = Mat4::perspective_rh_gl(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);
        match self.mode {
            CameraMode::Orbit => {
                let eye = self.eye_pos();
                proj * Mat4::look_at_rh(eye, self.center, Vec3::Y)
            }
            CameraMode::FirstPerson => {
                let forward = Vec3::new(
                    self.pitch.cos() * self.yaw.sin(),
                    self.pitch.sin(),
                    self.pitch.cos() * self.yaw.cos(),
                );
                let look_at = self.position + forward;
                proj * Mat4::look_at_rh(self.position, look_at, Vec3::Y)
            }
        }
    }

    fn eye_pos(&self) -> Vec3 {
        match self.mode {
            CameraMode::Orbit => Vec3::new(
                self.center.x + self.distance * self.pitch.cos() * self.yaw.sin(),
                self.center.y + self.distance * self.pitch.sin(),
                self.center.z + self.distance * self.pitch.cos() * self.yaw.cos(),
            ),
            CameraMode::FirstPerson => self.position,
        }
    }

    fn follow(&mut self, target: Option<Vec3>) {
        if !self.follow_target { return; }
        if let Some(pos) = target {
            self.center = pos;
            if self.mode == CameraMode::FirstPerson {
                self.position = pos + Vec3::new(0.0, 1.6, 0.0);
            }
        }
    }

    fn update(&mut self, input: &InputManager, dt: f32) {
        let k = input.keyboard();
        let rot_speed = 2.0 * dt;
        let zoom_speed = 3.0 * dt;
        let move_speed = 5.0 * dt;

        let (dx, dy) = input.mouse().delta();

        match self.mode {
            CameraMode::Orbit => {
                if k.down(KeyCode::W) { self.distance -= zoom_speed; }
                if k.down(KeyCode::S) { self.distance += zoom_speed; }
                if k.down(KeyCode::A) { self.yaw -= rot_speed; }
                if k.down(KeyCode::D) { self.yaw += rot_speed; }
                if k.down(KeyCode::Q) { self.pitch = (self.pitch - rot_speed).clamp(-1.4, 1.4); }
                if k.down(KeyCode::E) { self.pitch = (self.pitch + rot_speed).clamp(-1.4, 1.4); }
                self.distance = self.distance.max(0.5);

                if input.mouse().down(rustix_platform::input::MouseButton::Left) {
                    self.yaw += dx * 0.005;
                    self.pitch = (self.pitch - dy * 0.005).clamp(-1.4, 1.4);
                }
                if input.mouse().down(rustix_platform::input::MouseButton::Right) {
                    self.center += Vec3::new(-dx * 0.01 * self.distance * 0.05, dy * 0.01 * self.distance * 0.05, 0.0);
                }
            }
            CameraMode::FirstPerson => {
                let forward = Vec3::new(
                    self.pitch.cos() * self.yaw.sin(),
                    0.0,
                    self.pitch.cos() * self.yaw.cos(),
                ).normalize();
                let right = Vec3::new(forward.z, 0.0, -forward.x).normalize();

                if k.down(KeyCode::W) { self.position += forward * move_speed; }
                if k.down(KeyCode::S) { self.position -= forward * move_speed; }
                if k.down(KeyCode::A) { self.position -= right * move_speed; }
                if k.down(KeyCode::D) { self.position += right * move_speed; }
                if k.down(KeyCode::Q) { self.position.y -= move_speed; }
                if k.down(KeyCode::E) { self.position.y += move_speed; }

                if input.mouse().down(rustix_platform::input::MouseButton::Right) {
                    self.yaw += dx * 0.005;
                    self.pitch = (self.pitch - dy * 0.005).clamp(-1.4, 1.4);
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AppScreen {
    Startup,
    Editor,
}

#[derive(Clone, Copy, PartialEq)]
enum ConfirmTarget {
    None,
    BackToHub,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum ProjectType {
    Dim2,
    Dim3,
}

impl Default for ProjectType {
    fn default() -> Self { ProjectType::Dim3 }
}

#[derive(Clone)]
enum EditorAction {
    AddEntity(hecs::Entity),
    DeleteEntity { name: String, transform: Transform, mesh: String, material: Vec4 },
    RenameEntity { entity: hecs::Entity, old_name: String },
    TransformEntity { entity: hecs::Entity, old_transform: Transform },
}

struct UndoHistory {
    actions: Vec<EditorAction>,
    index: usize,
    max_actions: usize,
}

impl UndoHistory {
    fn new(max: usize) -> Self {
        Self { actions: Vec::with_capacity(max), index: 0, max_actions: max }
    }

    fn push(&mut self, action: EditorAction) {
        self.actions.truncate(self.index);
        if self.actions.len() >= self.max_actions {
            self.actions.remove(0);
        }
        self.actions.push(action);
        self.index = self.actions.len();
    }

    fn undo(&mut self) -> Option<&EditorAction> {
        if self.index > 0 {
            self.index -= 1;
            Some(&self.actions[self.index])
        } else {
            None
        }
    }

    fn redo(&mut self) -> Option<&EditorAction> {
        if self.index < self.actions.len() {
            let action = &self.actions[self.index];
            self.index += 1;
            Some(action)
        } else {
            None
        }
    }

    fn can_undo(&self) -> bool { self.index > 0 }
    fn can_redo(&self) -> bool { self.index < self.actions.len() }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ProjectEntry {
    name: String,
    path: String,
    last_opened: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct RecentProjectList {
    projects: Vec<ProjectEntry>,
}

fn project_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.to_string())
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ProjectInfo {
    name: String,
    description: String,
    created: String,
    last_opened: String,
    default_scene: String,
    scenes: Vec<String>,
    settings: ProjectSettings,
    #[serde(default)]
    scene: SceneData,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct ProjectSettings {
    resolution_width: u32,
    resolution_height: u32,
    enable_vsync: bool,
    target_fps: u32,
    #[serde(default)]
    project_type: ProjectType,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self { resolution_width: 1600, resolution_height: 900, enable_vsync: true, target_fps: 60, project_type: ProjectType::Dim3 }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct SceneEntity {
    name: String,
    position: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
    #[serde(default)]
    mesh: Option<String>,
    #[serde(default)]
    dirlight: Option<DirectionalLight>,
    #[serde(default)]
    pointlight: Option<PointLight>,
    #[serde(default)]
    spotlight: Option<SpotLight>,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct SceneData {
    entities: Vec<SceneEntity>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Transform {
    position: Vec3,
    rotation: Vec3,
    scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self { position: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE }
    }
}

#[derive(Debug, Clone)]
struct Name(pub String);

#[derive(Debug, Clone)]
struct MeshComponent(pub String);

#[derive(Debug, Clone)]
struct Material {
    pub base_color: Vec3,
    pub roughness: f32,
}

fn create_project_file(dir: &Path, project_type: ProjectType) -> Option<ProjectInfo> {
    fs::create_dir_all(dir).ok()?;
    let now = chrono_now();
    let mut settings = ProjectSettings::default();
    settings.project_type = project_type;
    let info = ProjectInfo {
        name: dir.file_name()?.to_string_lossy().to_string(),
        description: String::new(),
        created: now.clone(),
        last_opened: now,
        default_scene: String::new(),
        scenes: Vec::new(),
        settings,
        scene: SceneData::default(),
    };
    write_project_file(dir, &info)?;
    tracing::info!("created project: {} at {} (type: {:?})", info.name, dir.display(), project_type);
    Some(info)
}

fn write_project_file(dir: &Path, info: &ProjectInfo) -> Option<()> {
    let path = dir.join(PROJECT_FILE);
    let json = serde_json::to_string_pretty(info).ok()?;
    fs::write(&path, &json).ok()?;
    Some(())
}

fn load_project_file(dir: &Path) -> Option<ProjectInfo> {
    let path = dir.join(PROJECT_FILE);
    let json = fs::read_to_string(&path).ok()?;
    // Try new format first, fall back to old format
    let mut info: ProjectInfo = serde_json::from_str(&json).ok()?;
    // Update last_opened timestamp
    info.last_opened = chrono_now();
    write_project_file(dir, &info)?;
    tracing::info!("loaded project: {} from {}", info.name, dir.display());
    Some(info)
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "0".into())
}

fn world_to_scene(world: &EcsWorld) -> SceneData {
    let mut entities = Vec::new();
    for (entity, name, t) in world.query::<(&Entity, &Name, &Transform)>().iter() {
        let dirlight = world.get::<&DirectionalLight>(*entity).ok().map(|r| *r);
        let pointlight = world.get::<&PointLight>(*entity).ok().map(|r| *r);
        let spotlight = world.get::<&SpotLight>(*entity).ok().map(|r| *r);
        let mesh = world.get::<&MeshComponent>(*entity).ok().map(|r| r.0.clone());
        entities.push(SceneEntity {
            name: name.0.clone(),
            position: t.position.into(),
            rotation: t.rotation.into(),
            scale: t.scale.into(),
            mesh,
            dirlight,
            pointlight,
            spotlight,
        });
    }
    SceneData { entities }
}

fn scene_to_world(world: &mut EcsWorld, data: &SceneData) {
    world.clear();
    for e in &data.entities {
        let entity = world.spawn((
            Name(e.name.clone()),
            Transform {
                position: e.position.into(),
                rotation: e.rotation.into(),
                scale: e.scale.into(),
            },
            MeshComponent(e.mesh.clone().unwrap_or_else(|| "Cube".into())),
            Material { base_color: Vec3::new(0.7, 0.7, 0.7), roughness: 0.5 },
        ));
        if let Some(ref dl) = e.dirlight {
            let _ = world.insert(entity, (*dl,));
        }
        if let Some(ref pl) = e.pointlight {
            let _ = world.insert(entity, (*pl,));
        }
        if let Some(ref sl) = e.spotlight {
            let _ = world.insert(entity, (*sl,));
        }
    }
}

fn recent_projects_path() -> std::path::PathBuf {
    dirs::config_dir()
        .map(|d| d.join("rustix").join(RECENT_FILE))
        .unwrap_or_else(|| std::path::PathBuf::from(RECENT_FILE))
}

fn load_recent_projects() -> Vec<ProjectEntry> {
    let path = recent_projects_path();
    let json = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str::<RecentProjectList>(&json)
        .map(|list| list.projects)
        .unwrap_or_default()
}

fn save_recent_projects(recent: &[ProjectEntry]) {
    let path = recent_projects_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let list = RecentProjectList { projects: recent.to_vec() };
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = fs::write(&path, &json);
    }
}

fn add_recent_project(recent: &mut Vec<ProjectEntry>, path: String, info: &Option<ProjectInfo>) {
    recent.retain(|p| p.path != path);
    let name = info.as_ref().map(|i| i.name.clone()).unwrap_or_else(|| project_name_from_path(&path));
    recent.insert(0, ProjectEntry { name, path, last_opened: chrono_now() });
    if recent.len() > 10 {
        recent.truncate(10);
    }
    save_recent_projects(recent);
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

    let el = winit::event_loop::EventLoop::new().expect("event loop");
    let wc = WindowConfig { title: "Rustix Editor".into(), width: 1600, height: 900, fullscreen: false, resizable: true, decorations: true };
    let mut window = WindowHandle::new(&el, &wc).expect("window");
    let (mut ww, mut wh) = window.physical_size();

    let mut input = InputManager::new();

    let egui_ctx = egui::Context::default();
    {
        let mut fonts = egui::FontDefinitions::default();
        let candidates = [
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
            "/usr/share/fonts/truetype/ubuntu/Ubuntu-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ];
        let mut loaded = false;
        for path in &candidates {
            if let Ok(bytes) = std::fs::read(path) {
                let name = std::path::Path::new(path)
                    .file_stem().and_then(|s| s.to_str()).unwrap_or("custom");
                fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
                fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, name.to_owned());
                fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().insert(0, name.to_owned());
                loaded = true;
                tracing::info!("loaded system font: {name}");
                break;
            }
        }
        if !loaded {
            tracing::warn!("no system font found, using egui defaults (Greek may not render)");
        }
        egui_ctx.set_fonts(fonts);
    }
    let mut egui_state = egui_winit::State::new(egui_ctx.clone(), egui_ctx.viewport_id(), window.inner(), None, None, None);

    let rc = rustix_core::config::RenderConfig {
        enable_validation: false, preferred_gpu: config.render.preferred_gpu, frame_count: config.render.frame_count,
        shader_cache_path: config.render.shader_cache_path, pipeline_cache_path: config.render.pipeline_cache_path,
    };
    let mut renderer = Renderer::new(&rc).expect("renderer");
    renderer.init_surface(window.raw_window_handle(), window.raw_display_handle(), ww, wh).expect("surf");
    let sc_format = renderer.swapchain.lock().format();
    let mut egui_r = ui_renderer::EguiVulkanRenderer::new(&renderer, sc_format).expect("egui");

    let mut cam = EditorCamera::new();
    let mut last = Instant::now();

    let mut fc = 0u64;
    let mut ft = Instant::now();
    let mut fps = 0u64;

    let mut screen = AppScreen::Startup;
    let mut recent_projects: Vec<ProjectEntry> = load_recent_projects();
    let mut current_project: Option<ProjectInfo> = None;
    let mut project_dir: Option<String> = None;
    let mut ecs_world = EcsWorld::new();

    // Create initial entities
    for i in 0..3 {
        let e = ecs_world.spawn((
            Transform { position: Vec3::new(i as f32 * 2.0, 0.0, 0.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
            Name(format!("Entity {}", i)),
            MeshComponent("Cube".into()),
            Material { base_color: Vec3::new(0.6 + i as f32 * 0.15, 0.4 + i as f32 * 0.1, 0.5), roughness: 0.3 + i as f32 * 0.2 },
        ));
        tracing::info!("created entity {}: {:?}", i, e);
    }

    // Load the cube mesh and create a graphics pipeline for scene view rendering
    let mut meshes: HashMap<String, Mesh> = HashMap::new();
    let pending_mesh_load: std::rc::Rc<std::cell::RefCell<Option<String>>> = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut scene_pipeline: Option<rustix_render::pipeline::GraphicsPipeline> = None;
    let mut scene_descriptor_pool: Option<vk::DescriptorPool> = None;
    let mut scene_descriptor_set: Option<vk::DescriptorSet> = None;
    let mut scene_uniform_buffer: Option<rustix_render::memory::GpuBuffer> = None;
    let mut scene_depth_buffer: Option<rustix_render::DepthBuffer> = None;

    // 2D rendering resources
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
                            ecs_world.query::<(&Entity, &Transform)>().iter()
                                .find(|(e, _)| **e == sel)
                                .map(|(_, t)| t.position)
                        });
                        cam.follow(follow_pos);

                        if needs_resize {
                            renderer.swapchain.lock().recreate(&renderer.instance, &renderer.device).ok();
                            let ext = renderer.swapchain.lock().extent();
                            scene_depth_buffer = renderer.create_depth_buffer(ext).ok();
                            needs_resize = false;
                        }

                        if !renderer.begin_frame().unwrap_or(false) { return; }
                        let cmd = renderer.current_cmd();
                        unsafe { renderer.device().logical().begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).unwrap(); }

                        // Lazily initialize scene rendering resources
                        if scene_pipeline.is_none() {
                            if let Ok(mesh) = gltf_loader::load_glb(&renderer, &gltf_loader::generate_cube_glb(), "Cube") {
                                meshes.insert("Cube".into(), mesh);
                            }
                            // Also create sphere and torus procedural meshes
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
                                scene_pipeline = rustix_render::pipeline::GraphicsPipeline::create(renderer.device(), &sw, &vs, &fs).ok();
                                let dp = renderer.create_descriptor_pool().ok();
                                scene_descriptor_pool = dp;
                                drop(sw);
                            }
                            if let Some(ref pipeline) = scene_pipeline {
                                if let Some(dp) = scene_descriptor_pool {
                                    scene_descriptor_set = renderer.alloc_descriptor_set(dp, pipeline.descriptor_set_layout).ok();
                                }
                            }
                            scene_uniform_buffer = renderer.create_buffer("scene_ubo", 80, vk::BufferUsageFlags::UNIFORM_BUFFER, gpu_allocator::MemoryLocation::CpuToGpu).ok();
                            
                            // Create depth buffer
                            let sw = renderer.swapchain.lock();
                            scene_depth_buffer = renderer.create_depth_buffer(sw.extent()).ok();
                            drop(sw);
                        }

                        // Lazily initialize 2D rendering resources
                        if pipeline_2d.is_none() {
                            let vs_2d = rustix_render::shader::builtin::vertex_2d_shader(renderer.device().logical());
                            let fs_2d = rustix_render::shader::builtin::fragment_2d_shader(renderer.device().logical());
                            if let (Ok(vs), Ok(fs)) = (vs_2d, fs_2d) {
                                let sw = renderer.swapchain.lock();
                                let ppl = rustix_render::pipeline::GraphicsPipeline2D::create(renderer.device(), &sw, &vs, &fs);
                                if let Ok(p) = ppl {
                                    desc_set_2d = renderer.alloc_descriptor_set(p.desc_pool, p.desc_layout).ok();
                                    pipeline_2d = Some(p);
                                }
                                drop(sw);
                            }
                            ubo_2d = renderer.create_buffer("ubo_2d", 64, vk::BufferUsageFlags::UNIFORM_BUFFER, gpu_allocator::MemoryLocation::CpuToGpu).ok();

                            // Unit quad: position(vec2) + uv(vec2) + color(vec4) = 32 bytes
                            let quad: [f32; 32] = [
                                -0.5, -0.5,  0.0, 0.0,  1.0, 1.0, 1.0, 1.0,
                                 0.5, -0.5,  1.0, 0.0,  1.0, 1.0, 1.0, 1.0,
                                 0.5,  0.5,  1.0, 1.0,  1.0, 1.0, 1.0, 1.0,
                                -0.5,  0.5,  0.0, 1.0,  1.0, 1.0, 1.0, 1.0,
                            ];
                            quad_buffer_2d = renderer.create_buffer("quad_2d", 128, vk::BufferUsageFlags::VERTEX_BUFFER, gpu_allocator::MemoryLocation::CpuToGpu).ok();
                            if let Some(ref mut buf) = quad_buffer_2d {
                                buf.write(bytemuck::bytes_of(&quad));
                            }

                            // Checkerboard texture
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
                            texture_2d = renderer.create_texture(tex_size, tex_size, &pixels).ok();
                        }

                        // Handle pending mesh load from file
                        if let Some(path) = pending_mesh_load.borrow_mut().take() {
                            if let Ok(data) = std::fs::read(&path) {
                                let mesh_name = Path::new(&path)
                                    .file_stem().and_then(|s| s.to_str()).unwrap_or("Imported")
                                    .to_string();
                                if let Ok(mesh) = gltf_loader::load_glb(&renderer, &data, &mesh_name) {
                                    tracing::info!("loaded mesh {mesh_name} from {path}");
                                    meshes.insert(mesh_name.clone(), mesh);
                                    let e = ecs_world.spawn((
                                        Transform { position: Vec3::new(0.0, 1.0, 0.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
                                        Name(mesh_name.clone()),
                                        MeshComponent(mesh_name),
                                        Material { base_color: Vec3::new(0.8, 0.8, 0.8), roughness: 0.5 },
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

                        // Render 3D scene
                        if let (Some(ref pipeline), Some(depth_buf)) = 
                            (&scene_pipeline, &scene_depth_buffer) 
                        {
                            if let Some(ubo) = &scene_uniform_buffer {
                                // Upload view-projection matrix to uniform buffer
                                let aspect = {
                                    let sw = renderer.swapchain.lock();
                                    sw.extent().width as f32 / sw.extent().height as f32
                                };
                                let view_proj = cam.view_proj(aspect);
                                let eye = cam.eye_pos();
                                let mut ubo_data = [0u8; 80];
                                ubo_data[0..64].copy_from_slice(bytemuck::bytes_of(&view_proj));
                                ubo_data[64..80].copy_from_slice(bytemuck::bytes_of(&Vec4::new(eye.x, eye.y, eye.z, 0.0)));
                                ubo.write(&ubo_data);
                                
                                // Update descriptor set
                                if let Some(set) = scene_descriptor_set {
                                    renderer.update_descriptor_set(set, ubo);
                                }
                                
                                // Gather directional light from scene (first found)
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
                                
                                // Begin scene render pass
                                let clear_color = [0.04, 0.04, 0.08, 1.0f32];
                                renderer.begin_scene_pass(cmd, depth_buf, clear_color);
                                
                                for (entity, transform, mesh_comp) in ecs_world.query::<(&Entity, &Transform, &MeshComponent)>().iter() {
                                    if let Some(mesh) = meshes.get(&mesh_comp.0) {
                                        let rot = Quat::from_euler(EulerRot::XYZ, transform.rotation.x, transform.rotation.y, transform.rotation.z);
                                        let model = Mat4::from_scale_rotation_translation(
                                            transform.scale,
                                            rot,
                                            transform.position,
                                        );

                                        let mat: Option<Vec4> = ecs_world.get::<&Material>(*entity).ok()
                                            .map(|m| Vec4::new(m.base_color.x, m.base_color.y, m.base_color.z, m.roughness));
                                        let mat_v = mat.unwrap_or(Vec4::new(0.7, 0.7, 0.7, 0.5));
                                        
                                        let mut pc_data = [0u8; 112];
                                        pc_data[0..64].copy_from_slice(bytemuck::bytes_of(&model));
                                        pc_data[64..80].copy_from_slice(bytemuck::bytes_of(&light_dir));
                                        pc_data[80..96].copy_from_slice(bytemuck::bytes_of(&light_color));
                                        pc_data[96..112].copy_from_slice(bytemuck::bytes_of(&mat_v));
                                        
                                        renderer.draw_indexed_in_pass(
                                            cmd, pipeline,
                                            &mesh.vertex_buffer,
                                            mesh.index_buffer.as_ref(), mesh.index_count,
                                            &pc_data,
                                            scene_descriptor_set.unwrap_or_default(),
                                        );
                                    }
                                    let _ = entity;
                                }
                                
                                renderer.end_scene_pass(cmd);
                            }
                        }

                        // 2D sprite rendering pass
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

                        let mut raw_input = egui_state.take_egui_input(window.inner());
                        // Fix stuck primary_down: inject a synthetic release event if
                        // InputManager says left button is not pressed.
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
                                    startup_screen(ctx, &recent_projects, &mut screen, &open_project, &new_project, &*show_new_project_type, &*new_project_type);
                                }
                                AppScreen::Editor => {
                                    let proj_name = current_project.as_ref().map(|p| p.name.as_str()).unwrap_or("Untitled");
                                    let proj_name_owned = proj_name.to_string();
                                    editor_screen(ctx, &mut cam, &mut window, &mut screen, &input, target, &ww, &wh, &mut fps, &open_project, &new_project, &proj_name_owned, &mut current_project, &mut project_dir, &mut ecs_world, &*selected_entity, &*pending_delete, &*dirty, &*show_confirm, &*confirm_target, &*show_settings, &*renaming, &*rename_buffer, &*undo_history, &mut sprite_editor, &pending_mesh_load);
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
                            }
                        }

                        egui_r.update_textures(&renderer, &out.textures_delta);
                        let clipped = egui_ctx.tessellate(out.shapes, out.pixels_per_point);
                        egui_r.draw_primitives(cmd, &renderer, &clipped, out.pixels_per_point);
                        egui_state.handle_platform_output(window.inner(), out.platform_output);

                        renderer.end_frame().unwrap_or(());
                        input.end_tick();

                        fc += 1;
                        if ft.elapsed().as_secs_f32() >= 1.0 {
                            fps = fc;
                            fc = 0;
                            ft = Instant::now();
                            let proj_label = current_project.as_ref().map(|p| p.name.as_str()).unwrap_or("Rustix Editor");
                            let star = if dirty.get() { " *" } else { "" };
                            window.set_title(&format!("{proj_label}{star} — FPS: {fps}"));
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

fn startup_screen(ctx: &egui::Context, recent: &[ProjectEntry], _screen: &mut AppScreen, open_project: &std::cell::RefCell<Option<String>>, new_project: &std::cell::RefCell<Option<String>>, show_new_project_type: &std::cell::Cell<bool>, new_project_type: &std::cell::Cell<ProjectType>) {
    let bg = egui::Color32::from_rgb(22, 22, 28);
    let panel_bg = egui::Color32::from_rgb(30, 30, 38);
    let surface = egui::Color32::from_rgb(38, 38, 46);
    let border = egui::Color32::from_rgb(50, 50, 60);
    let accent = egui::Color32::from_rgb(72, 120, 240);
    let text_primary = egui::Color32::from_rgb(220, 220, 228);
    let text_secondary = egui::Color32::from_rgb(140, 140, 155);

    egui::CentralPanel::default().show(ctx, |ui| {
        let avail = ui.available_size();
        let pw = (avail.x * 0.50).min(640.0).max(500.0);
        let ph = (avail.y * 0.55).min(440.0).max(340.0);

        let (_, rect) = ui.allocate_space(avail);
        ui.painter().rect_filled(rect, 0.0, bg);

        let panel_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(pw, ph));
        let shadow_rect = panel_rect.translate(egui::vec2(0.0, 2.0));
        ui.painter().rect_filled(shadow_rect, 10.0, egui::Color32::from_rgba_premultiplied(0, 0, 0, 60));
        ui.painter().rect_filled(panel_rect, 10.0, panel_bg);
        ui.painter().rect_stroke(panel_rect, 10.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);

        let inner = panel_rect.shrink(28.0);
        ui.allocate_ui_at_rect(inner, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);

                ui.label(egui::RichText::new("Rustix").size(28.0).color(text_primary).strong());
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Engine").size(14.0).color(accent));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Project Hub").size(12.0).color(text_secondary));
                ui.add_space(24.0);

                let sep_rect = ui.allocate_space(egui::vec2(60.0, 2.0)).1;
                ui.painter().rect_filled(sep_rect, 1.0, accent);
                ui.add_space(20.0);

                ui.horizontal(|ui| {
                    let lw = inner.width() * 0.48;
                    let rw = inner.width() - lw - 20.0;

                    ui.vertical(|ui| {
                        ui.set_min_width(lw);
                        ui.label(egui::RichText::new("RECENT").size(10.0).color(text_secondary));
                        ui.add_space(6.0);

                        if recent.is_empty() {
                            egui::Frame::none()
                                .fill(surface)
                                .stroke(egui::Stroke::new(1.0, border))
                                .rounding(egui::Rounding::same(6))
                                .show(ui, |ui| {
                                    ui.set_min_width(lw);
                                    ui.add_space(16.0);
                                    ui.label(egui::RichText::new("No recent projects")
                                        .size(12.0).color(text_secondary));
                                    ui.label(egui::RichText::new("Open a project to get started")
                                        .size(11.0).color(text_secondary));
                                    ui.add_space(16.0);
                                });
                        } else {
                            egui::Frame::none()
                                .fill(surface)
                                .stroke(egui::Stroke::new(1.0, border))
                                .rounding(egui::Rounding::same(6))
                                .show(ui, |ui| {
                                    ui.set_min_width(lw);
                                    let name_font = egui::FontId::proportional(13.0);
                                    let path_font = egui::FontId::proportional(10.0);
                                    for proj in recent.iter() {
                                        let item_h = 44.0;
                                        let id = ui.next_auto_id();
                                        let (_, rect) = ui.allocate_space(egui::vec2(lw, item_h));
                                        let resp = ui.interact(rect, id, egui::Sense::click());
                                        if resp.hovered() {
                                            ui.painter().rect_filled(rect.shrink(2.0), 4.0, egui::Color32::from_rgb(44, 44, 54));
                                        }
                                        ui.painter().text(
                                            egui::pos2(rect.min.x + 10.0, rect.min.y + 8.0),
                                            egui::Align2::LEFT_TOP,
                                            &proj.name,
                                            name_font.clone(),
                                            text_primary,
                                        );
                                        ui.painter().text(
                                            egui::pos2(rect.min.x + 10.0, rect.min.y + 26.0),
                                            egui::Align2::LEFT_TOP,
                                            format!("{}  ·  {}", proj.path, proj.last_opened),
                                            path_font.clone(),
                                            text_secondary,
                                        );
                                        if resp.clicked() {
                                            *open_project.borrow_mut() = Some(proj.path.clone());
                                        }
                                    }
                                });
                        }
                    });

                    ui.add_space(20.0);

                    ui.vertical(|ui| {
                        ui.set_min_width(rw);
                        ui.label(egui::RichText::new("GET STARTED").size(10.0).color(text_secondary));
                        ui.add_space(12.0);

                        let btn_size = egui::vec2(rw, 44.0);

                        let new_btn = egui::Button::new(
                            egui::RichText::new("New Project").size(14.0).color(egui::Color32::WHITE)
                        )
                        .min_size(btn_size)
                        .fill(accent)
                        .rounding(egui::Rounding::same(6));
                        if ui.add(new_btn).clicked() {
                            show_new_project_type.set(true);
                        }

                        ui.add_space(10.0);

                        let open_btn = egui::Button::new(
                            egui::RichText::new("Open Project…").size(14.0).color(text_primary)
                        )
                        .min_size(btn_size)
                        .fill(surface)
                        .stroke(egui::Stroke::new(1.0, border))
                        .rounding(egui::Rounding::same(6));
                        if ui.add(open_btn).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Open Project")
                                .pick_folder()
                            {
                                *open_project.borrow_mut() = Some(path.to_string_lossy().to_string());
                            }
                        }

                        ui.add_space(24.0);
                        ui.label(egui::RichText::new("Create a new project or open an")
                            .size(11.0).color(text_secondary));
                        ui.label(egui::RichText::new("existing one to begin editing.")
                            .size(11.0).color(text_secondary));
                    });
                });
            });
        });
    });

    // Project type selection dialog
    if show_new_project_type.get() {
        egui::Window::new("New Project")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Choose project type:");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add_sized(egui::vec2(120.0, 60.0), egui::Button::new(
                        egui::RichText::new("3D Project").size(16.0).color(egui::Color32::WHITE)
                    )).clicked() {
                        new_project_type.set(ProjectType::Dim3);
                        show_new_project_type.set(false);
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Create New Project")
                            .pick_folder()
                        {
                            new_project.borrow_mut().replace(path.to_string_lossy().to_string());
                        }
                    }
                    if ui.add_sized(egui::vec2(120.0, 60.0), egui::Button::new(
                        egui::RichText::new("2D Project").size(16.0).color(egui::Color32::WHITE)
                    )).clicked() {
                        new_project_type.set(ProjectType::Dim2);
                        show_new_project_type.set(false);
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Create New Project")
                            .pick_folder()
                        {
                            new_project.borrow_mut().replace(path.to_string_lossy().to_string());
                        }
                    }
                });
                ui.add_space(8.0);
                if ui.button("Cancel").clicked() {
                    show_new_project_type.set(false);
                }
            });
    }
}

fn editor_screen(ctx: &egui::Context, cam: &mut EditorCamera, _window: &mut WindowHandle, screen: &mut AppScreen, _input: &InputManager, target: &winit::event_loop::ActiveEventLoop, ww: &u32, wh: &u32, fps: &u64, open_project: &std::cell::RefCell<Option<String>>, new_project: &std::cell::RefCell<Option<String>>, project_name: &str, current_project: &mut Option<ProjectInfo>, project_dir: &mut Option<String>, world: &mut EcsWorld, selected_entity: &std::cell::RefCell<Option<hecs::Entity>>, pending_delete: &std::cell::RefCell<Option<hecs::Entity>>, dirty: &std::cell::Cell<bool>, show_confirm: &std::cell::Cell<bool>, confirm_target: &std::cell::Cell<ConfirmTarget>, show_settings: &std::cell::Cell<bool>, renaming: &std::cell::RefCell<Option<hecs::Entity>>, rename_buffer: &std::cell::RefCell<String>, undo_history: &std::cell::RefCell<UndoHistory>, sprite_editor: &mut sprite_editor::SpriteEditor, pending_mesh_load: &std::cell::RefCell<Option<String>>) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            let star = if dirty.get() { " *" } else { "" };
            ui.label(egui::RichText::new(format!("{project_name}{star}")).strong());
            ui.label(egui::RichText::new("— Rustix Editor").weak());
            ui.separator();
            ui.separator();
            ui.menu_button("File", |ui| {
                if ui.button("New Project").clicked() {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Create New Project")
                        .pick_folder()
                    {
                        *new_project.borrow_mut() = Some(path.to_string_lossy().to_string());
                    }
                }
                if ui.button("Open Project…").clicked() {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Open Project")
                        .pick_folder()
                    {
                        *open_project.borrow_mut() = Some(path.to_string_lossy().to_string());
                    }
                }
                ui.separator();
                if ui.button("Save").clicked() {
                    ui.close_menu();
                    if let Some(ref mut proj) = current_project {
                        proj.settings.resolution_width = *ww;
                        proj.settings.resolution_height = *wh;
                        proj.scene = world_to_scene(world);
                        if let Some(ref dir) = project_dir {
                            let _ = write_project_file(Path::new(dir), proj);
                        }
                    }
                    dirty.set(false);
                }
                if ui.button("Save As…").clicked() {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Save Project As")
                        .pick_folder()
                    {
                        let dir = Path::new(&path);
                        if let Some(ref mut proj) = current_project {
                            proj.settings.resolution_width = *ww;
                            proj.settings.resolution_height = *wh;
                            proj.scene = world_to_scene(world);
                            let _ = write_project_file(dir, proj);
                            *project_dir = Some(path.to_string_lossy().to_string());
                            *open_project.borrow_mut() = Some(path.to_string_lossy().to_string());
                            dirty.set(false);
                        }
                    }
                }
                ui.separator();
                if ui.button("Load GLB…").clicked() {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Import GLB Mesh")
                        .add_filter("GLB", &["glb"])
                        .pick_file()
                    {
                        pending_mesh_load.replace(Some(path.to_string_lossy().to_string()));
                    }
                }
                if ui.button("Project Settings…").clicked() {
                    show_settings.set(true);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Back to Project Hub").clicked() {
                    if dirty.get() {
                        show_confirm.set(true);
                        confirm_target.set(ConfirmTarget::BackToHub);
                    } else {
                        *screen = AppScreen::Startup;
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    if dirty.get() {
                        show_confirm.set(true);
                        confirm_target.set(ConfirmTarget::Exit);
                    } else {
                        target.exit();
                    }
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Preferences").clicked() { ui.close_menu(); }
            });
            ui.menu_button("Assets", |ui| {
                if ui.button("Import New Asset…").clicked() { ui.close_menu(); }
                ui.separator();
                if ui.button("Sprite Editor").clicked() {
                    sprite_editor.set_visible(true);
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About Rustix").clicked() { ui.close_menu(); }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ptype = current_project.as_ref().map(|p| match p.settings.project_type {
                    ProjectType::Dim3 => "3D",
                    ProjectType::Dim2 => "2D",
                }).unwrap_or("");
                if !ptype.is_empty() {
                    ui.label(egui::RichText::new(ptype).color(egui::Color32::from_rgb(120, 240, 200)).weak());
                }
                ui.label(format!("FPS: {fps}"));
                ui.separator();
                if ui.selectable_label(cam.follow_target, "Follow").clicked() {
                    cam.follow_target = !cam.follow_target;
                }
                let orbit_selected = cam.mode == CameraMode::Orbit;
                if ui.selectable_label(orbit_selected, "Orbit").clicked() && !orbit_selected {
                    cam.mode = CameraMode::Orbit;
                }
                if ui.selectable_label(!orbit_selected, "1stP").clicked() && orbit_selected {
                    cam.mode = CameraMode::FirstPerson;
                }
                ui.label(egui::RichText::new("Cam:").weak());
            });
        });
    });

    egui::SidePanel::left("hierarchy").resizable(true).default_width(220.0).show(ctx, |ui| {
        ui.heading("Hierarchy");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let is_renaming = *renaming.borrow();
            let mut finish_rename = None;

            for (entity, name) in world.query_mut::<(&Entity, &Name)>() {
                let is_selected = *selected_entity.borrow() == Some(*entity);

                if Some(*entity) == is_renaming {
                    let mut buf = rename_buffer.borrow_mut();
                    let resp = ui.add_sized(
                        egui::vec2(ui.available_width(), 0.0),
                        egui::TextEdit::singleline(&mut *buf)
                            .text_color(egui::Color32::WHITE)
                            .desired_width(f32::INFINITY),
                    );
                    if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        finish_rename = Some(*entity);
                    }
                    if !resp.has_focus() {
                        ctx.memory_mut(|mem| mem.request_focus(resp.id));
                    }
                } else {
                    let resp = ui.add_sized(
                        egui::vec2(ui.available_width(), 0.0),
                        egui::Button::new(egui::RichText::new(&name.0).color(egui::Color32::WHITE))
                            .fill(if is_selected { egui::Color32::from_rgb(50, 90, 150) } else { egui::Color32::TRANSPARENT }),
                    );
                    if resp.clicked() {
                        *selected_entity.borrow_mut() = Some(*entity);
                    }
                    if resp.double_clicked() {
                        *renaming.borrow_mut() = Some(*entity);
                        *rename_buffer.borrow_mut() = name.0.clone();
                    }
                    if resp.secondary_clicked() {
                        *selected_entity.borrow_mut() = Some(*entity);
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Rename").clicked() {
                            *renaming.borrow_mut() = Some(*entity);
                            *rename_buffer.borrow_mut() = name.0.clone();
                            ui.close_menu();
                        }
                        if ui.button("Delete").clicked() {
                            *pending_delete.borrow_mut() = Some(*entity);
                            ui.close_menu();
                        }
                    });
                }
            }

            if let Some(entity) = finish_rename {
                let new_name = rename_buffer.borrow().clone();
                for (e, n) in world.query_mut::<(&Entity, &mut Name)>() {
                    if *e == entity {
                        if n.0 != new_name {
                            undo_history.borrow_mut().push(EditorAction::RenameEntity { entity, old_name: n.0.clone() });
                            n.0 = new_name;
                            dirty.set(true);
                        }
                        break;
                    }
                }
                *renaming.borrow_mut() = None;
            }
        });
        if let Some(entity) = pending_delete.borrow_mut().take() {
            let mut name = String::new();
            let mut transform = Transform::default();
            let mut mesh = String::new();
            let mut mat = Vec4::new(0.7, 0.7, 0.7, 0.5);
            for (e, n, t, m) in world.query_mut::<(&Entity, &Name, &Transform, &MeshComponent)>() {
                if *e == entity {
                    name = n.0.clone();
                    transform = t.clone();
                    mesh = m.0.clone();
                    if let Ok(mat_comp) = world.get::<&Material>(entity) {
                        mat = Vec4::new(mat_comp.base_color.x, mat_comp.base_color.y, mat_comp.base_color.z, mat_comp.roughness);
                    }
                    break;
                }
            }
            undo_history.borrow_mut().push(EditorAction::DeleteEntity { name, transform, mesh, material: mat });
            let _ = world.despawn(entity);
            dirty.set(true);
            if *selected_entity.borrow() == Some(entity) {
                *selected_entity.borrow_mut() = None;
            }
        }
        ui.add_space(4.0);
        if ui.button("Add Entity").clicked() {
            let e = world.spawn((Name("New Entity".to_string()), Transform::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(0.7, 0.7, 0.7), roughness: 0.5 }));
            undo_history.borrow_mut().push(EditorAction::AddEntity(e));
            *selected_entity.borrow_mut() = Some(e);
            dirty.set(true);
        }
        ui.menu_button("Create Light", |ui| {
            if ui.button("Directional").clicked() {
                let e = world.spawn((Name("Directional Light".to_string()), Transform::default(), DirectionalLight::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(1.0, 0.95, 0.8), roughness: 0.3 }));
                undo_history.borrow_mut().push(EditorAction::AddEntity(e));
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
                ui.close_menu();
            }
            if ui.button("Point").clicked() {
                let e = world.spawn((Name("Point Light".to_string()), Transform { position: Vec3::new(0.0, 3.0, 0.0), ..Default::default() }, PointLight::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(1.0, 0.9, 0.6), roughness: 0.3 }));
                undo_history.borrow_mut().push(EditorAction::AddEntity(e));
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
                ui.close_menu();
            }
            if ui.button("Spot").clicked() {
                let e = world.spawn((Name("Spot Light".to_string()), Transform { position: Vec3::new(0.0, 3.0, 0.0), ..Default::default() }, SpotLight::default(), MeshComponent("Cube".into()), Material { base_color: Vec3::new(1.0, 0.95, 0.7), roughness: 0.3 }));
                undo_history.borrow_mut().push(EditorAction::AddEntity(e));
                *selected_entity.borrow_mut() = Some(e);
                dirty.set(true);
                ui.close_menu();
            }
        });
    });

    let selected_entity_val = *selected_entity.borrow();
    let selected_name: Option<String> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &Name)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, n)| n.0.clone())
    });
    let mut selected_transform: Option<Transform> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &Transform)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, t)| t.clone())
    });
    let mut selected_dirlight: Option<DirectionalLight> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &DirectionalLight)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, l)| l.clone())
    });
    let mut selected_pointlight: Option<PointLight> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &PointLight)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, l)| l.clone())
    });
    let mut selected_spotlight: Option<SpotLight> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &SpotLight)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, l)| l.clone())
    });
    let mut selected_material: Option<Material> = selected_entity_val.and_then(|sel_ent| {
        world.query::<(&Entity, &Material)>().iter().find(|(e, _)| **e == sel_ent).map(|(_, m)| m.clone())
    });

    egui::SidePanel::right("inspector").resizable(true).default_width(260.0).show(ctx, |ui| {
        ui.heading("Inspector");
        ui.separator();
        if let (Some(name), Some(transform)) = (selected_name.as_ref(), selected_transform.as_mut()) {
            ui.label(egui::RichText::new(name).strong());
            ui.separator();
            ui.label("Transform");
            ui.add(egui::DragValue::new(&mut transform.position.x).prefix("x: "));
            ui.add(egui::DragValue::new(&mut transform.position.y).prefix("y: "));
            ui.add(egui::DragValue::new(&mut transform.position.z).prefix("z: "));
            ui.horizontal(|ui| {
                ui.label("Rotation");
                ui.add(egui::DragValue::new(&mut transform.rotation.x).prefix("x: "));
                ui.add(egui::DragValue::new(&mut transform.rotation.y).prefix("y: "));
                ui.add(egui::DragValue::new(&mut transform.rotation.z).prefix("z: "));
            });
            ui.add(egui::DragValue::new(&mut transform.scale.x).prefix("Scale x: "));
            ui.add(egui::DragValue::new(&mut transform.scale.y).prefix("Scale y: "));
            ui.add(egui::DragValue::new(&mut transform.scale.z).prefix("Scale z: "));
            if let Some(target_entity) = selected_entity_val {
                for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                    if *e == target_entity {
                        let old = t.clone();
                        *t = transform.clone();
                        if old != *t {
                            undo_history.borrow_mut().push(EditorAction::TransformEntity { entity: target_entity, old_transform: old });
                            dirty.set(true);
                        }
                        break;
                    }
                }
            }
            // Light component editing
            // Write-back light component changes
            if let Some(target_entity) = selected_entity_val {
                if let Some(ref dl) = selected_dirlight {
                    for (e, l) in world.query_mut::<(&Entity, &mut DirectionalLight)>() {
                        if *e == target_entity { *l = *dl; dirty.set(true); break; }
                    }
                }
                if let Some(ref pl) = selected_pointlight {
                    for (e, l) in world.query_mut::<(&Entity, &mut PointLight)>() {
                        if *e == target_entity { *l = *pl; dirty.set(true); break; }
                    }
                }
                if let Some(ref sl) = selected_spotlight {
                    for (e, l) in world.query_mut::<(&Entity, &mut SpotLight)>() {
                        if *e == target_entity { *l = *sl; dirty.set(true); break; }
                    }
                }
                if let Some(ref mat) = selected_material {
                    for (e, m) in world.query_mut::<(&Entity, &mut Material)>() {
                        if *e == target_entity { *m = mat.clone(); dirty.set(true); break; }
                    }
                }
            }
            
            if let Some(ref mut dl) = selected_dirlight {
                ui.separator();
                ui.label("Directional Light");
                ui.horizontal(|ui| { ui.label("Color"); ui.add(egui::DragValue::new(&mut dl.color.x).prefix("R:")); ui.add(egui::DragValue::new(&mut dl.color.y).prefix("G:")); ui.add(egui::DragValue::new(&mut dl.color.z).prefix("B:")); });
                ui.add(egui::DragValue::new(&mut dl.intensity).prefix("Intensity: ").speed(0.1));
            }
            if let Some(ref mut pl) = selected_pointlight {
                ui.separator();
                ui.label("Point Light");
                ui.horizontal(|ui| { ui.label("Color"); ui.add(egui::DragValue::new(&mut pl.color.x).prefix("R:")); ui.add(egui::DragValue::new(&mut pl.color.y).prefix("G:")); ui.add(egui::DragValue::new(&mut pl.color.z).prefix("B:")); });
                ui.add(egui::DragValue::new(&mut pl.intensity).prefix("Intensity: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut pl.radius).prefix("Radius: ").speed(0.1));
            }
            if let Some(ref mut sl) = selected_spotlight {
                ui.separator();
                ui.label("Spot Light");
                ui.horizontal(|ui| { ui.label("Color"); ui.add(egui::DragValue::new(&mut sl.color.x).prefix("R:")); ui.add(egui::DragValue::new(&mut sl.color.y).prefix("G:")); ui.add(egui::DragValue::new(&mut sl.color.z).prefix("B:")); });
                ui.add(egui::DragValue::new(&mut sl.intensity).prefix("Intensity: ").speed(0.1));
                ui.add(egui::DragValue::new(&mut sl.inner_angle).prefix("Inner angle: ").speed(0.01));
                ui.add(egui::DragValue::new(&mut sl.outer_angle).prefix("Outer angle: ").speed(0.01));
                ui.add(egui::DragValue::new(&mut sl.radius).prefix("Radius: ").speed(0.1));
            }
            if let Some(ref mut mat) = selected_material {
                ui.separator();
                ui.label("Material");
                ui.horizontal(|ui| {
                    ui.label("Base");
                    ui.add(egui::DragValue::new(&mut mat.base_color.x).prefix("R:").speed(0.01));
                    ui.add(egui::DragValue::new(&mut mat.base_color.y).prefix("G:").speed(0.01));
                    ui.add(egui::DragValue::new(&mut mat.base_color.z).prefix("B:").speed(0.01));
                });
                ui.add(egui::DragValue::new(&mut mat.roughness).prefix("Roughness: ").speed(0.01).range(0.01..=1.0));
            }
        } else {
            ui.label("Select an object in the Hierarchy to inspect.");
            ui.add_space(10.0);
            ui.label(egui::RichText::new("No object selected").italics());
        }
        ui.separator();
        ui.add_space(5.0);
        ui.label("Camera:");
        match cam.mode {
            CameraMode::Orbit => {
                ui.label(format!("  Mode: Orbit{}", if cam.follow_target { " (following)" } else { "" }));
                ui.label(format!("  Center: ({:.2}, {:.2}, {:.2})",
                    cam.center.x, cam.center.y, cam.center.z));
                ui.label(format!("  Distance: {:.2}", cam.distance));
            }
            CameraMode::FirstPerson => {
                ui.label(format!("  Mode: 1st Person{}", if cam.follow_target { " (following)" } else { "" }));
                ui.label(format!("  Eye: ({:.2}, {:.2}, {:.2})",
                    cam.position.x, cam.position.y, cam.position.z));
                ui.label(format!("  Yaw: {:.2}  Pitch: {:.2}", cam.yaw, cam.pitch));
            }
        }
    });

    egui::TopBottomPanel::bottom("console_tabs").resizable(true).default_height(160.0).show(ctx, |ui| {
        let tab_id = egui::Id::new("bottom_tab");
        let mut active_tab = ctx.data(|d| d.get_temp::<usize>(tab_id).unwrap_or(0));

        ui.horizontal(|ui| {
            if ui.selectable_label(active_tab == 0, "Console").clicked() { active_tab = 0; }
            if ui.selectable_label(active_tab == 1, "Asset Browser").clicked() { active_tab = 1; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if active_tab == 0 && ui.button("Clear").clicked() {
                    rustix_core::log_capture::clear_logs();
                }
                if active_tab == 1 && ui.button("Refresh").clicked() {
                    // Refresh the asset list on next frame
                }
            });
        });
        ctx.data_mut(|d| d.insert_temp(tab_id, active_tab));
        ui.separator();

        if active_tab == 0 {
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                let logs = rustix_core::log_capture::get_logs();
                for entry in logs {
                    let color = match entry.level {
                        tracing::Level::ERROR => egui::Color32::from_rgb(240, 80, 80),
                        tracing::Level::WARN => egui::Color32::from_rgb(240, 200, 50),
                        tracing::Level::INFO => egui::Color32::from_rgb(180, 200, 220),
                        tracing::Level::DEBUG => egui::Color32::from_rgb(140, 140, 160),
                        tracing::Level::TRACE => egui::Color32::from_rgb(100, 100, 120),
                    };
                    ui.label(egui::RichText::new(format!("{}", entry)).color(color));
                }
            });
        } else {
            if let Some(ref dir) = project_dir {
                let path = Path::new(dir);
                let mut entries: Vec<_> = std::fs::read_dir(path)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .collect();
                entries.sort_by_key(|e| !e.file_type().map(|t| t.is_dir()).unwrap_or(false));

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in &entries {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let ft = entry.file_type().ok();
                        let is_dir = ft.map(|t| t.is_dir()).unwrap_or(false);
                        let ext = Path::new(&name).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

                        let (icon, color) = if is_dir {
                            ("[DIR]", egui::Color32::from_rgb(240, 200, 80))
                        } else {
                            match ext.as_str() {
                                "glb" | "gltf" | "obj" | "fbx" => ("[MODEL]", egui::Color32::from_rgb(130, 200, 250)),
                                "png" | "jpg" | "jpeg" | "hdr" | "exr" => ("[TEX]", egui::Color32::from_rgb(100, 220, 140)),
                                "wav" | "mp3" | "ogg" | "flac" => ("[AUDIO]", egui::Color32::from_rgb(250, 150, 200)),
                                "wgsl" | "glsl" | "vert" | "frag" | "spv" => ("[SHADER]", egui::Color32::from_rgb(200, 180, 100)),
                                "rs" | "lua" | "py" => ("[CODE]", egui::Color32::from_rgb(180, 200, 220)),
                                "rustixproj" => ("[PROJ]", egui::Color32::from_rgb(120, 240, 200)),
                                _ => ("[FILE]", egui::Color32::from_rgb(160, 160, 170)),
                            }
                        };
                        ui.label(egui::RichText::new(format!("{icon}  {name}")).color(color).size(12.0));
                    }
                });
            } else {
                ui.label("Open a project to browse its assets.");
            }
        }
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let rect = ui.max_rect();
        
        let entity_count = world.query::<&Name>().iter().count();
        let mut clicked_entity = None;
        
        // Camera view-projection for 3D→2D projection
        let aspect = rect.width() / rect.height().max(1.0);
        let vp = cam.view_proj(aspect);
        
        let world_to_screen = |wpos: Vec3| -> Option<egui::Pos2> {
            let clip = vp * Vec4::new(wpos.x, wpos.y, wpos.z, 1.0);
            if clip.w <= 0.0 { return None; }
            let ndc = clip.truncate() / clip.w;
            let x = rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width();
            let y = rect.min.y + (1.0 - (ndc.y * 0.5 + 0.5)) * rect.height();
            Some(egui::pos2(x, y))
        };
        
        // Gizmo state persisted in egui memory
        let gizmo_active_id = egui::Id::new("gizmo_active");
        let gizmo_drag_start_id = egui::Id::new("gizmo_drag_start");
        let gizmo_entity_pos_id = egui::Id::new("gizmo_entity_pos");
        let mut gizmo_active = ctx.data(|d| d.get_temp::<usize>(gizmo_active_id).unwrap_or(usize::MAX));
        let mut gizmo_drag_start = ctx.data(|d| d.get_temp::<egui::Vec2>(gizmo_drag_start_id).unwrap_or(egui::Vec2::ZERO));
        let mut gizmo_entity_pos = ctx.data(|d| d.get_temp::<Vec3>(gizmo_entity_pos_id).unwrap_or(Vec3::ZERO));
        
        let mut deferred_new_pos: Option<Vec3> = None;
        
        // Ground plane grid (XZ plane, y=0)
        let grid_half = 20.0;
        let grid_step = 1.0;
        let major_step = 5.0;
        let grid_color_minor = egui::Color32::from_rgba_premultiplied(100, 110, 130, 30);
        let grid_color_major = egui::Color32::from_rgba_premultiplied(100, 110, 130, 70);
        
        let mut z = -grid_half;
        while z <= grid_half {
            let near = Vec3::new(-grid_half, 0.0, z);
            let far = Vec3::new(grid_half, 0.0, z);
            if let (Some(a), Some(b)) = (world_to_screen(near), world_to_screen(far)) {
                let is_major = (z % major_step).abs() < 0.01;
                let col = if is_major { grid_color_major } else { grid_color_minor };
                ui.painter().line_segment([a, b], egui::Stroke::new(if is_major { 1.5 } else { 0.5 }, col));
            }
            z += grid_step;
        }
        let mut x = -grid_half;
        while x <= grid_half {
            let near = Vec3::new(x, 0.0, -grid_half);
            let far = Vec3::new(x, 0.0, grid_half);
            if let (Some(a), Some(b)) = (world_to_screen(near), world_to_screen(far)) {
                let is_major = (x % major_step).abs() < 0.01;
                let col = if is_major { grid_color_major } else { grid_color_minor };
                ui.painter().line_segment([a, b], egui::Stroke::new(if is_major { 1.5 } else { 0.5 }, col));
            }
            x += grid_step;
        }
        
        for (entity, name, transform) in world.query_mut::<(&Entity, &Name, &Transform)>() {
            let is_selected = *selected_entity.borrow() == Some(*entity);
            
            if let Some(screen_pos) = world_to_screen(transform.position) {
                let entity_color = if is_selected {
                    egui::Color32::from_rgb(70, 150, 250)
                } else {
                    egui::Color32::from_rgb(200, 200, 220)
                };
                
                let ent_resp = ui.interact(
                    egui::Rect::from_center_size(screen_pos, egui::vec2(12.0, 12.0)),
                    ui.next_auto_id(),
                    egui::Sense::click()
                );
                if ent_resp.clicked() { clicked_entity = Some(*entity); }
                ui.painter().circle_filled(screen_pos, 4.0, entity_color);
                ui.painter().text(
                    egui::pos2(screen_pos.x + 8.0, screen_pos.y - 4.0),
                    egui::Align2::LEFT_CENTER,
                    &name.0,
                    egui::FontId::proportional(11.0),
                    egui::Color32::LIGHT_GRAY
                );
                
                // Gizmo handles for selected entity
                if is_selected {
                    let axis_colors = [
                        (Vec3::X, egui::Color32::from_rgb(220, 60, 60)),
                        (Vec3::Y, egui::Color32::from_rgb(60, 200, 60)),
                        (Vec3::Z, egui::Color32::from_rgb(60, 100, 220)),
                    ];
                    let gizmo_len = 60.0;
                    
                    for (axis_idx, (axis_dir, color)) in axis_colors.iter().enumerate() {
                        let tip_world = transform.position + *axis_dir * 2.0;
                        if let Some(tip_screen) = world_to_screen(tip_world) {
                            let dir_2d = (tip_screen - screen_pos).normalized();
                            let handle_screen = screen_pos + dir_2d * gizmo_len;
                            
                            ui.painter().line_segment(
                                [screen_pos, handle_screen],
                                egui::Stroke::new(2.0, *color),
                            );
                            
                            let handle_rect = egui::Rect::from_center_size(handle_screen, egui::vec2(14.0, 14.0));
                            let handle_resp = ui.interact(handle_rect, ui.next_auto_id(), egui::Sense::click_and_drag());
                            
                            let is_active = gizmo_active == axis_idx;
                            let handle_color = if is_active { egui::Color32::WHITE } else { *color };
                            ui.painter().circle_filled(handle_screen, 6.0, handle_color);
                            ui.painter().circle_stroke(handle_screen, 6.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                            
                            if handle_resp.drag_started() {
                                gizmo_active = axis_idx;
                                gizmo_drag_start = handle_resp.interact_pointer_pos().unwrap_or(handle_screen).to_vec2();
                                gizmo_entity_pos = transform.position;
                            }
                        }
                    }
                    
                    // Gizmo drag — project mouse delta along screen-space axis direction
                    if gizmo_active != usize::MAX {
                        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                            if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                                let axis_dir = match gizmo_active {
                                    0 => Vec3::X,
                                    1 => Vec3::Y,
                                    _ => Vec3::Z,
                                };
                                // Project mouse delta onto screen-space axis direction
                                let drag_delta = pointer_pos.to_vec2() - gizmo_drag_start;
                                if let (Some(tip), Some(base)) = (
                                    world_to_screen(gizmo_entity_pos + axis_dir),
                                    world_to_screen(gizmo_entity_pos),
                                ) {
                                    let axis_2d = (tip - base).normalized();
                                    let along = drag_delta.dot(axis_2d) * cam.distance * 0.01;
                                    deferred_new_pos = Some(gizmo_entity_pos + axis_dir * along);
                                }
                            } else {
                                gizmo_active = usize::MAX;
                            }
                        } else {
                            gizmo_active = usize::MAX;
                        }
                    }
                }
            }
        }
        
        // Apply deferred gizmo position update
        if let Some(new_pos) = deferred_new_pos {
            if let Some(sel) = *selected_entity.borrow() {
                for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                    if *e == sel {
                        t.position = new_pos;
                        dirty.set(true);
                        break;
                    }
                }
            }
        }
        
        if let Some(e) = clicked_entity {
            *selected_entity.borrow_mut() = Some(e);
        } else if ui.interact(rect, ui.next_auto_id(), egui::Sense::click()).clicked() {
            *selected_entity.borrow_mut() = None;
            gizmo_active = usize::MAX;
        }
        
        // Persist gizmo state
        ctx.data_mut(|d| d.insert_temp(gizmo_active_id, gizmo_active));
        ctx.data_mut(|d| d.insert_temp(gizmo_drag_start_id, gizmo_drag_start));
        ctx.data_mut(|d| d.insert_temp(gizmo_entity_pos_id, gizmo_entity_pos));
        
        // Subtle HUD overlay at bottom-right
        let hud = format!("{} entities", entity_count);
        ui.painter().text(
            egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0),
            egui::Align2::RIGHT_BOTTOM,
            hud,
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgba_premultiplied(180, 200, 220, 160),
        );
    });

    // Project Settings window
    if show_settings.get() {
        if let Some(ref mut proj) = current_project {
            let mut settings = proj.settings.clone();
            egui::Window::new("Project Settings")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Resolution");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut settings.resolution_width).prefix("Width: ").range(320..=7680));
                        ui.add(egui::DragValue::new(&mut settings.resolution_height).prefix("Height: ").range(240..=4320));
                    });
                    ui.add_space(8.0);
                    ui.add(egui::Checkbox::new(&mut settings.enable_vsync, "Enable V-Sync"));
                    ui.add_space(8.0);
                    ui.add(egui::DragValue::new(&mut settings.target_fps).prefix("Target FPS: ").range(30..=480));
                    ui.add_space(8.0);
                    let mut is_3d = settings.project_type == ProjectType::Dim3;
                    ui.horizontal(|ui| {
                        ui.label("Project type:");
                        if ui.selectable_label(is_3d, "3D").clicked() { is_3d = true; }
                        if ui.selectable_label(!is_3d, "2D").clicked() { is_3d = false; }
                    });
                    settings.project_type = if is_3d { ProjectType::Dim3 } else { ProjectType::Dim2 };
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            if proj.settings != settings {
                                proj.settings = settings;
                                dirty.set(true);
                            }
                            show_settings.set(false);
                        }
                        if ui.button("Cancel").clicked() {
                            show_settings.set(false);
                        }
                    });
                });
        } else {
            show_settings.set(false);
        }
    }

    // Confirmation dialog for unsaved changes
    if show_confirm.get() {
        egui::Window::new("Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("You have unsaved changes. Discard them?");
                ui.horizontal(|ui| {
                    if ui.button("Discard Changes").clicked() {
                        let target_action = confirm_target.get();
                        show_confirm.set(false);
                        confirm_target.set(ConfirmTarget::None);
                        dirty.set(false);
                        match target_action {
                            ConfirmTarget::BackToHub => *screen = AppScreen::Startup,
                            ConfirmTarget::Exit => target.exit(),
                            ConfirmTarget::None => {}
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        show_confirm.set(false);
                        confirm_target.set(ConfirmTarget::None);
                    }
                });
            });
    }

    // Sprite editor window
    if sprite_editor.is_visible() {
        sprite_editor.show(ctx);
    }

    // Delete key: remove selected entity
    if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
        if let Some(entity) = *selected_entity.borrow() {
            let mut name = String::new();
            let mut transform = Transform::default();
            let mut mesh = String::new();
            let mut mat = Vec4::new(0.7, 0.7, 0.7, 0.5);
            for (e, n, t, m) in world.query_mut::<(&Entity, &Name, &Transform, &MeshComponent)>() {
                if *e == entity {
                    name = n.0.clone();
                    transform = t.clone();
                    mesh = m.0.clone();
                    if let Ok(mat_comp) = world.get::<&Material>(entity) {
                        mat = Vec4::new(mat_comp.base_color.x, mat_comp.base_color.y, mat_comp.base_color.z, mat_comp.roughness);
                    }
                    break;
                }
            }
            undo_history.borrow_mut().push(EditorAction::DeleteEntity { name, transform, mesh, material: mat });
            let _ = world.despawn(entity);
            *selected_entity.borrow_mut() = None;
            dirty.set(true);
        }
    }

    // Undo (Ctrl+Z)
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z) && !i.modifiers.shift) {
        let action = undo_history.borrow_mut().undo().cloned();
        if let Some(action) = action {
            match action {
                EditorAction::AddEntity(entity) => {
                    let _ = world.despawn(entity);
                    if *selected_entity.borrow() == Some(entity) {
                        *selected_entity.borrow_mut() = None;
                    }
                }
                EditorAction::DeleteEntity { name, transform, mesh, material } => {
                    let e = world.spawn((Name(name), transform, MeshComponent(mesh), Material { base_color: Vec3::new(material.x, material.y, material.z), roughness: material.w }));
                    *selected_entity.borrow_mut() = Some(e);
                }
                EditorAction::RenameEntity { entity, old_name } => {
                    for (e, n) in world.query_mut::<(&Entity, &mut Name)>() {
                        if *e == entity {
                            n.0 = old_name;
                            break;
                        }
                    }
                }
                EditorAction::TransformEntity { entity, old_transform } => {
                    for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                        if *e == entity {
                            *t = old_transform;
                            break;
                        }
                    }
                }
            }
            dirty.set(true);
        }
    }

    // Redo (Ctrl+Shift+Z)
    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z)) {
        let action = undo_history.borrow_mut().redo().cloned();
        if let Some(action) = action {
            match action {
                EditorAction::AddEntity(entity) => {
                    // Re-adding is too fragile since the old entity ID is gone
                }
                EditorAction::DeleteEntity { name, transform, mesh, material } => {
                    let e = world.spawn((Name(name), transform, MeshComponent(mesh), Material { base_color: Vec3::new(material.x, material.y, material.z), roughness: material.w }));
                    *selected_entity.borrow_mut() = Some(e);
                }
                EditorAction::RenameEntity { entity, old_name } => {
                    for (e, n) in world.query_mut::<(&Entity, &mut Name)>() {
                        if *e == entity {
                            n.0 = old_name;
                            break;
                        }
                    }
                }
                EditorAction::TransformEntity { entity, old_transform } => {
                    for (e, t) in world.query_mut::<(&Entity, &mut Transform)>() {
                        if *e == entity {
                            *t = old_transform;
                            break;
                        }
                    }
                }
            }
            dirty.set(true);
        }
    }
}
