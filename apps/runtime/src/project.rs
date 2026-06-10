use std::fs;
use std::path::Path;
use super::scene::{SceneData, SceneEntity, Material};
use rustix_render::{Camera, DirectionalLight};

const PROJECT_FILE: &str = "project.rustixproj";
const RECENT_FILE: &str = "recent_projects.json";

#[derive(Clone, Copy, PartialEq)]
pub enum AppScreen {
    Startup,
    Editor,
    PlayTest,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ConfirmTarget {
    None,
    BackToHub,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Dim2,
    Dim3,
    Voxel,
    Tetris,
    EndlessRunner3D,
    Breakout2D,
    Platformer3D,
}

impl Default for ProjectType {
    fn default() -> Self { ProjectType::Dim3 }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub path: String,
    pub last_opened: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RecentProjectList {
    pub projects: Vec<ProjectEntry>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct EditorCameraState {
    pub position: [f32; 3],
    pub center: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub mode: crate::camera::CameraMode,
    pub follow_target: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CameraBookmark {
    pub name: String,
    pub position: [f32; 3],
    pub center: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub mode: crate::camera::CameraMode,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewportLayout {
    pub name: String,
    pub open: bool,
    #[serde(default)]
    pub position: Option<[f32; 2]>,
    #[serde(default)]
    pub size: Option<[f32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockPosition {
    Left,
    Right,
    Bottom,
    Floating,
    Hidden,
}

impl Default for DockPosition {
    fn default() -> Self { DockPosition::Left }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct LayoutState {
    #[serde(default = "default_hierarchy_width")]
    pub hierarchy_width: f32,
    #[serde(default = "default_inspector_width")]
    pub inspector_width: f32,
    #[serde(default = "default_console_height")]
    pub console_height: f32,
    #[serde(default)]
    pub viewports: Vec<ViewportLayout>,
    #[serde(default)]
    pub hierarchy_dock: DockPosition,
    #[serde(default = "default_inspector_dock")]
    pub inspector_dock: DockPosition,
    #[serde(default = "default_console_dock")]
    pub console_dock: DockPosition,
    #[serde(default = "default_asset_browser_dock")]
    pub asset_browser_dock: DockPosition,
}

fn default_hierarchy_width() -> f32 { 220.0 }
fn default_inspector_width() -> f32 { 260.0 }
fn default_console_height() -> f32 { 160.0 }
fn default_inspector_dock() -> DockPosition { DockPosition::Right }
fn default_console_dock() -> DockPosition { DockPosition::Bottom }
fn default_asset_browser_dock() -> DockPosition { DockPosition::Left }

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            hierarchy_width: default_hierarchy_width(),
            inspector_width: default_inspector_width(),
            console_height: default_console_height(),
            viewports: Vec::new(),
            hierarchy_dock: DockPosition::Left,
            inspector_dock: DockPosition::Right,
            console_dock: DockPosition::Bottom,
            asset_browser_dock: DockPosition::Left,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub description: String,
    pub created: String,
    pub last_opened: String,
    pub default_scene: String,
    pub scenes: Vec<String>,
    pub settings: ProjectSettings,
    #[serde(default)]
    pub scene: SceneData,
    #[serde(default)]
    pub editor_camera: Option<EditorCameraState>,
    #[serde(default)]
    pub bookmarks: Vec<CameraBookmark>,
    #[serde(default)]
    pub layout: Option<LayoutState>,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectSettings {
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub enable_vsync: bool,
    pub target_fps: u32,
    #[serde(default)]
    pub project_type: ProjectType,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self { resolution_width: 1600, resolution_height: 900, enable_vsync: true, target_fps: 60, project_type: ProjectType::Dim3 }
    }
}

pub fn project_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.to_string())
}

fn create_starter_scene(project_type: ProjectType) -> SceneData {
    use rustix_core::math::Vec3;
    match project_type {
        ProjectType::Dim3 => {
            let ground_mat = Material {
                base_color: Vec3::new(0.25, 0.35, 0.25),
                alpha: 1.0,
                roughness: 0.8,
                metallic: 0.0,
                ao: 1.0,
                emissive: 0.0,
            };
            SceneData {
                entities: vec![
                    SceneEntity {
                        name: "Main Camera".to_string(),
                        position: [0.0, 0.0, 0.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: Some(rustix_audio::AudioListener {
                            position: Vec3::new(0.0, 0.0, 0.0),
                            forward: Vec3::new(0.0, 0.0, -1.0),
                            up: Vec3::new(0.0, 1.0, 0.0),
                        }),
                        camera: Some(Camera { fov_degrees: 60.0, near: 0.1, far: 1000.0 }),
                        skeleton: None,
                        parent_idx: None,
                    },
                    SceneEntity {
                        name: "Sun".to_string(),
                        position: [5.0, 10.0, 5.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: Some(DirectionalLight {
                            color: Vec3::new(1.0, 0.98, 0.95),
                            intensity: 1.5,
                        }),
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: None,
                        camera: None,
                        skeleton: None,
                        parent_idx: None,
                    },
                    SceneEntity {
                        name: "Ground".to_string(),
                        position: [0.0, -0.5, 0.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [10.0, 1.0, 10.0],
                        mesh: Some("Cube".to_string()),
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: Some(ground_mat),
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: None,
                        camera: None,
                        skeleton: None,
                        parent_idx: None,
                    },
                    SceneEntity {
                        name: "Cube".to_string(),
                        position: [0.0, 0.5, 0.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: Some("Cube".to_string()),
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: Some(Material {
                            base_color: Vec3::new(0.6, 0.4, 0.3),
                            alpha: 1.0,
                            roughness: 0.5,
                            metallic: 0.0,
                            ao: 1.0,
                            emissive: 0.0,
                        }),
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: None,
                        camera: None,
                        skeleton: None,
                        parent_idx: None,
                    },
                ],
            }
        }
        ProjectType::Dim2 => {
            SceneData {
                entities: vec![
                    SceneEntity {
                        name: "Main Camera".to_string(),
                        position: [0.0, 0.0, 5.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: Some(rustix_audio::AudioListener {
                            position: Vec3::new(0.0, 0.0, 0.0),
                            forward: Vec3::new(0.0, 0.0, -1.0),
                            up: Vec3::new(0.0, 1.0, 0.0),
                        }),
                        camera: Some(Camera { fov_degrees: 60.0, near: 0.1, far: 1000.0 }),
                        skeleton: None,
                        parent_idx: None,
                    },
                    SceneEntity {
                        name: "Sprite".to_string(),
                        position: [0.0, 0.0, 0.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: Some("Quad".to_string()),
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: Some(Material {
                            base_color: Vec3::new(0.6, 0.4, 0.3),
                            alpha: 1.0,
                            roughness: 0.5,
                            metallic: 0.0,
                            ao: 1.0,
                            emissive: 0.0,
                        }),
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: None,
                        camera: None,
                        skeleton: None,
                        parent_idx: None,
                    },
                ],
            }
        }
        ProjectType::Voxel => {
            SceneData {
                entities: vec![
                    SceneEntity {
                        name: "Player".to_string(),
                        position: [8.0, terrain_height(8, 8) as f32 + 2.0, 8.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: Some(rustix_audio::AudioListener {
                            position: Vec3::new(0.0, 0.0, 0.0),
                            forward: Vec3::new(0.0, 0.0, -1.0),
                            up: Vec3::new(0.0, 1.0, 0.0),
                        }),
                        camera: Some(Camera { fov_degrees: 75.0, near: 0.1, far: 1000.0 }),
                        skeleton: None,
                        parent_idx: None,
                    },
                    SceneEntity {
                        name: "Sun".to_string(),
                        position: [20.0, 40.0, 10.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: Some(DirectionalLight {
                            color: Vec3::new(1.0, 0.98, 0.95),
                            intensity: 1.5,
                        }),
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: None,
                        camera: None,
                        skeleton: None,
                        parent_idx: None,
                    },
                ],
            }
        }
        ProjectType::Tetris => {
            SceneData {
                entities: vec![
                    SceneEntity {
                        name: "Main Camera".to_string(),
                        position: [0.0, 0.0, 5.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: Some(rustix_audio::AudioListener {
                            position: Vec3::new(0.0, 0.0, 0.0),
                            forward: Vec3::new(0.0, 0.0, -1.0),
                            up: Vec3::new(0.0, 1.0, 0.0),
                        }),
                        camera: Some(Camera { fov_degrees: 60.0, near: 0.1, far: 1000.0 }),
                        skeleton: None,
                        parent_idx: None,
                    },
                ],
            }
        }
        ProjectType::EndlessRunner3D => {
            SceneData {
                entities: vec![
                    SceneEntity {
                        name: "Main Camera".to_string(),
                        position: [0.0, 8.0, 12.0],
                        rotation: [-30.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: Some(rustix_audio::AudioListener {
                            position: Vec3::new(0.0, 0.0, 0.0),
                            forward: Vec3::new(0.0, 0.0, -1.0),
                            up: Vec3::new(0.0, 1.0, 0.0),
                        }),
                        camera: Some(Camera { fov_degrees: 60.0, near: 0.1, far: 1000.0 }),
                        skeleton: None,
                        parent_idx: None,
                    },
                    SceneEntity {
                        name: "Sun".to_string(),
                        position: [5.0, 10.0, 5.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: Some(DirectionalLight {
                            color: Vec3::new(1.0, 0.98, 0.95),
                            intensity: 1.5,
                        }),
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: None,
                        camera: None,
                        skeleton: None,
                        parent_idx: None,
                    },
                    SceneEntity {
                        name: "Ground".to_string(),
                        position: [0.0, -0.5, 0.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [6.0, 1.0, 100.0],
                        mesh: Some("Cube".to_string()),
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: Some(Material {
                            base_color: Vec3::new(0.2, 0.3, 0.2),
                            alpha: 1.0,
                            roughness: 0.9,
                            metallic: 0.0,
                            ao: 1.0,
                            emissive: 0.0,
                        }),
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: None,
                        camera: None,
                        skeleton: None,
                        parent_idx: None,
                    },
                    SceneEntity {
                        name: "Player".to_string(),
                        position: [0.0, 0.5, 0.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [0.8, 0.8, 0.8],
                        mesh: Some("Capsule".to_string()),
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: Some(Material {
                            base_color: Vec3::new(0.3, 0.6, 0.9),
                            alpha: 1.0,
                            roughness: 0.4,
                            metallic: 0.1,
                            ao: 1.0,
                            emissive: 0.0,
                        }),
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: None,
                        camera: None,
                        skeleton: None,
                        parent_idx: None,
                    },
                ],
            }
        }
        ProjectType::Breakout2D => {
            SceneData {
                entities: vec![
                    SceneEntity {
                        name: "Main Camera".to_string(),
                        position: [0.0, 0.0, 5.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: Some(rustix_audio::AudioListener {
                            position: Vec3::new(0.0, 0.0, 0.0),
                            forward: Vec3::new(0.0, 0.0, -1.0),
                            up: Vec3::new(0.0, 1.0, 0.0),
                        }),
                        camera: Some(Camera { fov_degrees: 60.0, near: 0.1, far: 1000.0 }),
                        skeleton: None,
                        parent_idx: None,
                    },
                ],
            }
        }
        ProjectType::Platformer3D => {
            SceneData {
                entities: vec![
                    SceneEntity {
                        name: "Main Camera".to_string(),
                        position: [0.0, 5.0, 10.0],
                        rotation: [-25.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: None,
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: Some(rustix_audio::AudioListener {
                            position: Vec3::new(0.0, 0.0, 0.0),
                            forward: Vec3::new(0.0, 0.0, -1.0),
                            up: Vec3::new(0.0, 1.0, 0.0),
                        }),
                        camera: Some(Camera { fov_degrees: 60.0, near: 0.1, far: 1000.0 }),
                        skeleton: None,
                        parent_idx: None,
                    },
                    SceneEntity {
                        name: "Sun".to_string(),
                        position: [5.0, 10.0, 5.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        mesh: None,
                        dirlight: Some(DirectionalLight {
                            color: Vec3::new(1.0, 0.98, 0.95),
                            intensity: 1.5,
                        }),
                        pointlight: None,
                        spotlight: None,
                        material: None,
                        script: None,
                        rigidbody: None,
                        collider: None,
                        audiolistener: None,
                        camera: None,
                        skeleton: None,
                        parent_idx: None,
                    },
                ],
            }
        }
    }
}

fn terrain_height(x: i32, z: i32) -> i32 {
    let base = 8.0;
    let variation = crate::voxel::smooth_noise(x, z) * 8.0;
    let hills = (crate::voxel::smooth_noise(x / 3, z / 3) * 6.0).sin() * 3.0;
    (base + variation + hills).max(1.0).min(crate::voxel::CHUNK_HEIGHT as f32 - 2.0) as i32
}

pub fn create_project_file(dir: &Path, project_type: ProjectType) -> Option<ProjectInfo> {
    fs::create_dir_all(dir).ok()?;
    let now = chrono_now();
    let mut settings = ProjectSettings::default();
    settings.project_type = project_type;
    let scene = create_starter_scene(project_type);
    let info = ProjectInfo {
        name: dir.file_name()?.to_string_lossy().to_string(),
        description: String::new(),
        created: now.clone(),
        last_opened: now,
        default_scene: "main".to_string(),
        scenes: vec!["main".to_string()],
        settings,
        scene,
        editor_camera: Some(EditorCameraState {
            position: [5.0, 5.0, 8.0],
            center: [0.0, 0.0, 0.0],
            yaw: 0.5,
            pitch: -0.5,
            distance: 10.0,
            mode: crate::camera::CameraMode::Orbit,
            follow_target: false,
        }),
        bookmarks: Vec::new(),
        layout: None,
    };
    write_project_file(dir, &info)?;
    tracing::info!("created project: {} at {} (type: {:?})", info.name, dir.display(), project_type);
    Some(info)
}

pub fn write_project_file(dir: &Path, info: &ProjectInfo) -> Option<()> {
    let path = dir.join(PROJECT_FILE);
    let json = match serde_json::to_string_pretty(info) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("failed to serialize project: {}", e);
            return None;
        }
    };
    if let Err(e) = fs::write(&path, &json) {
        tracing::error!("failed to write project file {}: {}", path.display(), e);
        return None;
    }
    tracing::debug!("saved project {} with {} scene entities", info.name, info.scene.entities.len());
    Some(())
}

pub fn load_project_file(dir: &Path) -> Option<ProjectInfo> {
    let path = dir.join(PROJECT_FILE);
    let json = match fs::read_to_string(&path) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("failed to read project file {}: {}", path.display(), e);
            return None;
        }
    };
    let mut info: ProjectInfo = match serde_json::from_str(&json) {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("failed to parse project file {}: {}", path.display(), e);
            return None;
        }
    };
    tracing::debug!("loaded project {} with {} scene entities", info.name, info.scene.entities.len());
    info.last_opened = chrono_now();
    write_project_file(dir, &info)?;
    tracing::info!("loaded project: {} from {}", info.name, dir.display());
    Some(info)
}

pub fn chrono_now() -> String {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "0".into())
}

pub fn recent_projects_path() -> std::path::PathBuf {
    dirs::config_dir()
        .map(|d| d.join("rustix").join(RECENT_FILE))
        .unwrap_or_else(|| std::path::PathBuf::from(RECENT_FILE))
}

pub fn load_recent_projects() -> Vec<ProjectEntry> {
    let path = recent_projects_path();
    let json = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str::<RecentProjectList>(&json)
        .map(|list| list.projects)
        .unwrap_or_default()
}

pub fn save_recent_projects(recent: &[ProjectEntry]) {
    let path = recent_projects_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let list = RecentProjectList { projects: recent.to_vec() };
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = fs::write(&path, &json);
    }
}

pub fn add_recent_project(recent: &mut Vec<ProjectEntry>, path: String, info: &Option<ProjectInfo>) {
    recent.retain(|p| p.path != path);
    let name = info.as_ref().map(|i| i.name.clone()).unwrap_or_else(|| project_name_from_path(&path));
    recent.insert(0, ProjectEntry { name, path, last_opened: chrono_now() });
    if recent.len() > 10 {
        recent.truncate(10);
    }
    save_recent_projects(recent);
}
