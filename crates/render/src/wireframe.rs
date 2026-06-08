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
}

impl Default for DebugOverlay {
    fn default() -> Self {
        DebugOverlay::None
    }
}
