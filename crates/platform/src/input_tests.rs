//! Tests for input types, state, and manager.

use crate::input::types::*;
use crate::input::state::{KeyboardState, MouseState, GamepadState};
use crate::input::manager::InputManager;

#[test]
fn keycode_equality() {
    assert_eq!(KeyCode::A, KeyCode::A);
    assert_ne!(KeyCode::A, KeyCode::B);
}

#[test]
fn mouse_button_equality() {
    assert_eq!(MouseButton::Left, MouseButton::Left);
    assert_ne!(MouseButton::Left, MouseButton::Right);
}

#[test]
fn gamepad_button_variants() {
    let buttons = vec![
        GamepadButton::South, GamepadButton::East, GamepadButton::North, GamepadButton::West,
        GamepadButton::LeftTrigger, GamepadButton::RightTrigger,
    ];
    for i in 0..buttons.len() {
        for j in 0..buttons.len() {
            if i != j {
                assert_ne!(buttons[i], buttons[j]);
            }
        }
    }
}

#[test]
fn gamepad_id_wraps_u32() {
    let id = GamepadId(42);
    assert_eq!(id.0, 42);
}

#[test]
fn touch_phase_variants() {
    assert_ne!(TouchPhase::Started, TouchPhase::Moved);
    assert_ne!(TouchPhase::Moved, TouchPhase::Ended);
}

#[test]
fn input_event_clone() {
    let ev = InputEvent::KeyPress(KeyCode::Space);
    let cloned = ev.clone();
    assert_eq!(ev, cloned);
}

#[test]
fn text_input_state_default() {
    let state = TextInputState::default();
    assert!(!state.enabled);
    assert!(state.preedit.is_empty());
    assert!(state.committed.is_empty());
}

#[test]
fn touch_state_default() {
    let state = TouchState::default();
    assert!(state.active.is_empty());
    assert!(state.started.is_empty());
    assert!(state.ended.is_empty());
    assert_eq!(state.pinch_delta, 0.0);
    assert_eq!(state.two_finger_scroll, (0.0, 0.0));
}

#[test]
fn keyboard_state_new_empty() {
    let kb = KeyboardState::new();
    assert!(!kb.down(KeyCode::A));
    assert!(!kb.just_pressed(KeyCode::A));
    assert!(!kb.just_released(KeyCode::A));
}

#[test]
fn keyboard_state_press() {
    let mut kb = KeyboardState::new();
    kb.press(KeyCode::A);
    assert!(kb.down(KeyCode::A));
    assert!(kb.just_pressed(KeyCode::A));
    assert!(!kb.just_released(KeyCode::A));
}

#[test]
fn keyboard_state_release() {
    let mut kb = KeyboardState::new();
    kb.press(KeyCode::A);
    kb.end_tick();
    kb.release(KeyCode::A);
    assert!(!kb.down(KeyCode::A));
    assert!(!kb.just_pressed(KeyCode::A));
    assert!(kb.just_released(KeyCode::A));
}

#[test]
fn keyboard_state_end_tick_clears_transient() {
    let mut kb = KeyboardState::new();
    kb.press(KeyCode::A);
    kb.end_tick();
    assert!(kb.down(KeyCode::A));
    assert!(!kb.just_pressed(KeyCode::A));
}

#[test]
fn mouse_state_new_empty() {
    let ms = MouseState::new();
    assert_eq!(ms.position(), (0.0, 0.0));
    assert_eq!(ms.delta(), (0.0, 0.0));
    assert!(!ms.down(MouseButton::Left));
}

#[test]
fn mouse_state_press_and_release() {
    let mut ms = MouseState::new();
    ms.press(MouseButton::Left);
    assert!(ms.down(MouseButton::Left));
    assert!(ms.just_pressed(MouseButton::Left));

    ms.end_tick();
    ms.release(MouseButton::Left);
    assert!(!ms.down(MouseButton::Left));
    assert!(ms.just_released(MouseButton::Left));
}

#[test]
fn mouse_state_move_to() {
    let mut ms = MouseState::new();
    ms.move_to(10.0, 20.0);
    assert_eq!(ms.position(), (10.0, 20.0));
    assert_eq!(ms.delta(), (10.0, 20.0));
}

#[test]
fn mouse_state_scroll() {
    let mut ms = MouseState::new();
    ms.scroll_to(0.0, 5.0);
    assert_eq!(ms.scroll(), (0.0, 5.0));
}

#[test]
fn mouse_state_end_tick_resets() {
    let mut ms = MouseState::new();
    ms.move_to(10.0, 20.0);
    ms.scroll_to(0.0, 5.0);
    ms.add_raw_delta(1.0, 1.0);
    ms.end_tick();
    assert_eq!(ms.delta(), (0.0, 0.0));
    assert_eq!(ms.scroll(), (0.0, 0.0));
    assert_eq!(ms.raw_delta(), (0.0, 0.0));
}

#[test]
fn gamepad_state_new_empty() {
    let gp = GamepadState::new();
    assert!(!gp.button_down(GamepadButton::South));
    assert_eq!(gp.axis(GamepadAxis::LeftStickX), 0.0);
}

#[test]
fn gamepad_state_button_and_axis() {
    let mut gp = GamepadState::new();
    gp.buttons.insert(GamepadButton::South, true);
    gp.axes.insert(GamepadAxis::LeftStickX, 0.75);
    assert!(gp.button_down(GamepadButton::South));
    assert_eq!(gp.axis(GamepadAxis::LeftStickX), 0.75);
}

#[test]
fn input_manager_new() {
    let im = InputManager::new();
    assert!(!im.keyboard().down(KeyCode::A));
    assert_eq!(im.mouse().position(), (0.0, 0.0));
    assert!(im.gamepad(GamepadId(0)).is_none());
}

#[test]
fn input_manager_push_and_poll_key() {
    let mut im = InputManager::new();
    im.push_event(InputEvent::KeyPress(KeyCode::A));
    im.poll();
    assert!(im.keyboard().down(KeyCode::A));
    assert!(im.keyboard().just_pressed(KeyCode::A));

    im.end_tick();
    im.push_event(InputEvent::KeyRelease(KeyCode::A));
    im.poll();
    assert!(!im.keyboard().down(KeyCode::A));
    assert!(im.keyboard().just_released(KeyCode::A));
}

#[test]
fn input_manager_push_and_poll_mouse() {
    let mut im = InputManager::new();
    im.push_event(InputEvent::MouseMove(100.0, 200.0));
    im.push_event(InputEvent::MouseButton(MouseButton::Left, true));
    im.poll();
    assert_eq!(im.mouse().position(), (100.0, 200.0));
    assert!(im.mouse().down(MouseButton::Left));
}

#[test]
fn input_manager_push_and_poll_gamepad() {
    let mut im = InputManager::new();
    im.push_event(InputEvent::GamepadButton(GamepadId(0), GamepadButton::South, true));
    im.push_event(InputEvent::GamepadAxis(GamepadId(0), GamepadAxis::LeftStickX, 0.5));
    im.poll();
    let gp = im.gamepad(GamepadId(0)).unwrap();
    assert!(gp.button_down(GamepadButton::South));
    assert_eq!(gp.axis(GamepadAxis::LeftStickX), 0.5);
}

#[test]
fn input_manager_capture() {
    let mut im = InputManager::new();
    im.start_capture();
    im.push_event(InputEvent::KeyPress(KeyCode::A));
    im.push_event(InputEvent::KeyRelease(KeyCode::A));
    let captured = im.drain_captured();
    assert_eq!(captured.len(), 2);
    im.stop_capture();
    im.push_event(InputEvent::KeyPress(KeyCode::B));
    assert!(im.drain_captured().is_empty());
}

#[test]
fn input_manager_touch_events() {
    let mut im = InputManager::new();
    im.push_event(InputEvent::Touch { id: 1, phase: TouchPhase::Started, x: 10.0, y: 20.0, force: Some(0.5) });
    im.poll();
    assert_eq!(im.touch().active.len(), 1);
    assert_eq!(im.touch().started.len(), 1);

    im.push_event(InputEvent::Touch { id: 1, phase: TouchPhase::Moved, x: 15.0, y: 25.0, force: Some(0.6) });
    im.poll();
    assert_eq!(im.touch().active[&1].x, 15.0);

    im.push_event(InputEvent::Touch { id: 1, phase: TouchPhase::Ended, x: 15.0, y: 25.0, force: Some(0.6) });
    im.poll();
    assert!(im.touch().active.is_empty());
    assert_eq!(im.touch().ended.len(), 1);
}
