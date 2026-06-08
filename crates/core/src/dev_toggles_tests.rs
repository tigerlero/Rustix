//! Tests for runtime feature toggles.

use crate::dev_toggles::{DevToggles, HotkeyBindings, ToggleInput, ToggleKeyboardState, KeyCode, update_toggles};

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
