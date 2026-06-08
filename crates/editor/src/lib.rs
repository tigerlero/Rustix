use rustix_core::ecs::Entity;
use rustix_core::math::{Vec3, Mat4};

pub mod plugin;
pub mod camera;
pub mod undo;
pub mod hierarchy;
pub mod inspector;
pub mod asset_browser;
pub mod console;
pub mod profiler;
pub mod material_editor;
pub mod lighting_editor;
pub mod animation_editor;
pub mod terrain_editor;
pub mod play_mode;
pub mod build_pipeline;

pub use plugin::{PluginRegistry, PanelId, ToolId, EditorPanel, EditorTool};
pub use camera::{EditorCamera, CameraMode};
pub use undo::{UndoStack, Command};
pub use hierarchy::{HierarchyNode, FlatNode, flatten_hierarchy, ReparentCommand};
pub use inspector::{InspectorState, ComponentDesc, FieldDesc, FieldValue};
pub use asset_browser::{AssetBrowserState, AssetEntry};
pub use console::{ConsoleState, ConsoleEntry, LogLevel};
pub use profiler::{ProfilerState, ProfileSample};
pub use material_editor::{MaterialEditorState, MaterialProperty};
pub use lighting_editor::{LightingEditorState, EditableLight, EditableLightType, IblProbe};
pub use animation_editor::{TimelineState, AnimationTrack, Keyframe, KeyframeValue, InterpolationType};
pub use terrain_editor::{TerrainEditorState, TerrainEditMode};
pub use play_mode::{PlayModeController, PlayModeState};
pub use build_pipeline::{BuildPipeline, BuildConfig, BuildTarget, BuildProfile};

/// Reusable editor gizmo state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

impl Default for GizmoMode {
    fn default() -> Self { GizmoMode::Translate }
}

/// Current gizmo operation if the user is dragging a gizmo handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
    XY,
    XZ,
    YZ,
    XYZ,
}

/// Editor selection state shared between panels.
#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    pub selected: Option<Entity>,
    pub gizmo_mode: GizmoMode,
    pub gizmo_active: Option<GizmoAxis>,
    pub gizmo_start_pos: Vec3,
    pub gizmo_start_mouse: Vec3,
}

/// Compute the screen-space distance from a point to a line segment.
pub fn point_line_distance(point: Vec3, line_start: Vec3, line_end: Vec3) -> f32 {
    let line = line_end - line_start;
    let len_sq = line.length_squared();
    if len_sq < 0.0001 {
        return (point - line_start).length();
    }
    let t = ((point - line_start).dot(line) / len_sq).clamp(0.0, 1.0);
    let closest = line_start + line * t;
    (point - closest).length()
}

/// Axis colors for gizmo rendering.
pub const AXIS_COLORS: [(Vec3, [u8; 3]); 3] = [
    (Vec3::X, [200, 60, 60]),   // X = red
    (Vec3::Y, [60, 200, 60]),   // Y = green
    (Vec3::Z, [60, 60, 200]),   // Z = blue
];

/// Convert a world transform to a gizmo screen-space size.
pub fn gizmo_screen_size(world_pos: Vec3, view_proj: Mat4, screen_height: f32) -> f32 {
    let clip = view_proj * world_pos.extend(1.0);
    if clip.w.abs() < 0.0001 {
        return 0.0;
    }
    let ndc = clip.truncate() / clip.w;
    let pixel_per_unit = screen_height * 0.5; // approximate
    80.0 / pixel_per_unit // 80 pixels at screen center
}
