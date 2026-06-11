//! Wireframe / debug overlay rendering mode.

/// Global wireframe toggle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireframeMode {
    Off,
    On,
}

impl Default for WireframeMode {
    fn default() -> Self {
        WireframeMode::Off
    }
}

/// Debug overlay rendering options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugOverlay {
    None,
    Wireframe,
    Normals,
    TangentSpace,
    UV,
    Overdraw,
    LightComplexity,
}

impl Default for DebugOverlay {
    fn default() -> Self {
        DebugOverlay::None
    }
}

/// ECS component for controlling debug render modes per-viewport or globally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugRenderMode {
    pub overlay: DebugOverlay,
    pub wireframe: WireframeMode,
}

impl Default for DebugRenderMode {
    fn default() -> Self {
        Self {
            overlay: DebugOverlay::None,
            wireframe: WireframeMode::Off,
        }
    }
}

impl DebugRenderMode {
    pub fn is_wireframe(&self) -> bool {
        self.wireframe == WireframeMode::On || self.overlay == DebugOverlay::Wireframe
    }

    pub fn is_overdraw(&self) -> bool {
        self.overlay == DebugOverlay::Overdraw
    }

    pub fn is_light_complexity(&self) -> bool {
        self.overlay == DebugOverlay::LightComplexity
    }
}
