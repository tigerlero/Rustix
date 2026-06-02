use std::sync::atomic::{AtomicBool, Ordering};

// ------------------------------------------------------------------
// We can't depend on rustix_platform from rustix-core because core
// is a dependency of platform.  So we define a minimal Input trait
// here and provide a blanket impl when the platform crate is available.
//
// For the runtime we will call update_toggles with a concrete adapter.
// ------------------------------------------------------------------

/// Trait abstracting the keyboard state needed for toggle detection.
pub trait ToggleInput {
    /// Was this key pressed this frame (edge-triggered)?
    fn just_pressed(&self, key: KeyCode) -> bool;
}

/// Per-frame keyboard state adapter for hot-key detection.
///
/// In the runtime this wraps `rustix_platform::input::KeyboardState`.
#[derive(Debug, Clone)]
pub struct ToggleKeyboardState {
    just_pressed: Vec<KeyCode>,
}

impl ToggleKeyboardState {
    pub fn new(just_pressed: Vec<KeyCode>) -> Self {
        Self { just_pressed }
    }
}

impl ToggleInput for ToggleKeyboardState {
    fn just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed.contains(&key)
    }
}

/// Configurable key bindings for engine toggle actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyBindings {
    pub dev_mode: KeyCode,
    pub debug_render: KeyCode,
    pub profiling: KeyCode,
}

impl Default for HotkeyBindings {
    fn default() -> Self {
        Self {
            dev_mode: KeyCode::F1,
            debug_render: KeyCode::F2,
            profiling: KeyCode::F3,
        }
    }
}

/// Global developer toggle state.
///
/// All fields are atomics so the resource can be read from any thread
/// (e.g. render thread checking `debug_render`) without locking.
#[derive(Debug)]
pub struct DevToggles {
    /// Show dev UI, extra diagnostics, logging overlays.
    dev_mode: AtomicBool,
    /// Render wireframes, bounds, light frustums, etc.
    debug_render: AtomicBool,
    /// Enable Tracy / profiling zones.
    profiling: AtomicBool,
}

impl Default for DevToggles {
    fn default() -> Self {
        Self::new()
    }
}

impl DevToggles {
    pub fn new() -> Self {
        Self {
            dev_mode: AtomicBool::new(false),
            debug_render: AtomicBool::new(false),
            profiling: AtomicBool::new(false),
        }
    }

    pub fn dev_mode(&self) -> bool {
        self.dev_mode.load(Ordering::Relaxed)
    }

    pub fn debug_render(&self) -> bool {
        self.debug_render.load(Ordering::Relaxed)
    }

    pub fn profiling(&self) -> bool {
        self.profiling.load(Ordering::Relaxed)
    }

    pub fn set_dev_mode(&self, v: bool) {
        self.dev_mode.store(v, Ordering::Relaxed);
    }

    pub fn set_debug_render(&self, v: bool) {
        self.debug_render.store(v, Ordering::Relaxed);
    }

    pub fn set_profiling(&self, v: bool) {
        self.profiling.store(v, Ordering::Relaxed);
    }

    /// Toggle a flag and return its new value.
    pub fn toggle_dev_mode(&self) -> bool {
        let v = !self.dev_mode();
        self.set_dev_mode(v);
        v
    }

    pub fn toggle_debug_render(&self) -> bool {
        let v = !self.debug_render();
        self.set_debug_render(v);
        v
    }

    pub fn toggle_profiling(&self) -> bool {
        let v = !self.profiling();
        self.set_profiling(v);
        v
    }
}

/// Check the keyboard for hot-key presses and flip the corresponding toggles.
///
/// Call once per frame after `input.poll()`.
pub fn update_toggles<I: ToggleInput>(toggles: &DevToggles, input: &I, bindings: &HotkeyBindings) {
    if input.just_pressed(bindings.dev_mode) {
        let v = toggles.toggle_dev_mode();
        tracing::info!("dev mode {}", if v { "enabled" } else { "disabled" });
    }
    if input.just_pressed(bindings.debug_render) {
        let v = toggles.toggle_debug_render();
        tracing::info!("debug render {}", if v { "enabled" } else { "disabled" });
    }
    if input.just_pressed(bindings.profiling) {
        let v = toggles.toggle_profiling();
        tracing::info!("profiling {}", if v { "enabled" } else { "disabled" });
    }
}

// ------------------------------------------------------------------
// Minimal KeyCode enum so we don't depend on rustix_platform
// ------------------------------------------------------------------

/// Subset of key codes used for hot-key toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Escape, Tab, Space, Enter,
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight,
    Unknown,
}

// Blanket adapter for the runtime.  When rustix_platform is available
// the app can provide a small wrapper that implements ToggleInput.

#[cfg(test)]
mod tests {
    use super::*;

    struct MockInput {
        pressed: Vec<KeyCode>,
    }

    impl ToggleInput for MockInput {
        fn just_pressed(&self, key: KeyCode) -> bool {
            self.pressed.contains(&key)
        }
    }

    #[test]
    fn toggles_start_false() {
        let t = DevToggles::new();
        assert!(!t.dev_mode());
        assert!(!t.debug_render());
        assert!(!t.profiling());
    }

    #[test]
    fn toggles_toggle_flip() {
        let t = DevToggles::new();
        assert!(t.toggle_dev_mode());
        assert!(!t.toggle_dev_mode());
    }

    #[test]
    fn toggles_set_explicit() {
        let t = DevToggles::new();
        t.set_debug_render(true);
        assert!(t.debug_render());
    }

    #[test]
    fn update_toggles_flips_on_key() {
        let t = DevToggles::new();
        let input = MockInput {
            pressed: vec![KeyCode::F1],
        };
        update_toggles(&t, &input, &HotkeyBindings::default());
        assert!(t.dev_mode());
        assert!(!t.debug_render());
        assert!(!t.profiling());
    }

    #[test]
    fn update_toggles_all_three() {
        let t = DevToggles::new();
        let input = MockInput {
            pressed: vec![KeyCode::F1, KeyCode::F2, KeyCode::F3],
        };
        update_toggles(&t, &input, &HotkeyBindings::default());
        assert!(t.dev_mode());
        assert!(t.debug_render());
        assert!(t.profiling());
    }

    #[test]
    fn update_toggles_custom_bindings() {
        let t = DevToggles::new();
        let bindings = HotkeyBindings {
            dev_mode: KeyCode::Escape,
            debug_render: KeyCode::Space,
            profiling: KeyCode::Enter,
        };
        let input = MockInput {
            pressed: vec![KeyCode::Escape, KeyCode::Space],
        };
        update_toggles(&t, &input, &bindings);
        assert!(t.dev_mode());
        assert!(t.debug_render());
        assert!(!t.profiling());
    }

    #[test]
    fn update_toggles_no_press_noop() {
        let t = DevToggles::new();
        let input = MockInput { pressed: vec![] };
        update_toggles(&t, &input, &HotkeyBindings::default());
        assert!(!t.dev_mode());
        assert!(!t.debug_render());
        assert!(!t.profiling());
    }

    #[test]
    fn toggle_keyboard_state_adapter() {
        let ks = ToggleKeyboardState::new(vec![KeyCode::F2]);
        assert!(ks.just_pressed(KeyCode::F2));
        assert!(!ks.just_pressed(KeyCode::F1));
    }
}
