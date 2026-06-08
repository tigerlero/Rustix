use std::fs;
use std::path::Path;
use super::scene::SceneData;

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
}

fn default_hierarchy_width() -> f32 { 220.0 }
fn default_inspector_width() -> f32 { 260.0 }
fn default_console_height() -> f32 { 160.0 }
fn default_inspector_dock() -> DockPosition { DockPosition::Right }
fn default_console_dock() -> DockPosition { DockPosition::Bottom }

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

pub fn create_project_file(dir: &Path, project_type: ProjectType) -> Option<ProjectInfo> {
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
        editor_camera: None,
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
