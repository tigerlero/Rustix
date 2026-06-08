//! Tests for action mapping system.

use std::collections::HashMap;
use crate::actions::*;
use crate::input::{InputEvent, InputManager, KeyCode, MouseButton};

#[test]
fn input_actions_new_is_empty() {
    let actions = InputActions::new();
    assert!(!actions.pressed("jump"));
    assert_eq!(actions.value("fire"), 0.0);
}

#[test]
fn input_actions_bind() {
    let mut actions = InputActions::new();
    actions.bind("jump", &[ActionBinding::Key(KeyCode::Space)]);
    // binding registers the mapping but doesn't press the key
    assert!(!actions.pressed("jump"));
    assert_eq!(actions.value("jump"), 0.0);
}

#[test]
fn input_actions_unbind() {
    let mut actions = InputActions::new();
    actions.bind("jump", &[ActionBinding::Key(KeyCode::Space)]);
    actions.unbind("jump");
    assert!(!actions.pressed("jump"));
    assert_eq!(actions.value("jump"), 0.0);
}

#[test]
fn input_actions_bind_defaults() {
    let mut actions = InputActions::new();
    actions.bind_defaults();
    assert!(actions.pressed("jump") == false);
    assert!(actions.value("jump") == 0.0);
}

#[test]
fn input_actions_update_key_pressed() {
    let mut actions = InputActions::new();
    actions.bind("jump", &[ActionBinding::Key(KeyCode::Space)]);

    let mut input = InputManager::new();
    input.push_event(InputEvent::KeyPress(KeyCode::Space));
    input.poll();

    actions.update(&input);
    assert!(actions.pressed("jump"));
    assert!(actions.just_pressed("jump"));
    assert!(!actions.just_released("jump"));
    assert_eq!(actions.value("jump"), 1.0);
}

#[test]
fn input_actions_update_key_released() {
    let mut actions = InputActions::new();
    actions.bind("jump", &[ActionBinding::Key(KeyCode::Space)]);

    let mut input = InputManager::new();
    input.push_event(InputEvent::KeyPress(KeyCode::Space));
    input.poll();
    actions.update(&input);

    input.push_event(InputEvent::KeyRelease(KeyCode::Space));
    input.poll();
    actions.update(&input);

    assert!(!actions.pressed("jump"));
    assert!(!actions.just_pressed("jump"));
    assert!(actions.just_released("jump"));
}

#[test]
fn input_actions_update_mouse() {
    let mut actions = InputActions::new();
    actions.bind("fire", &[ActionBinding::MouseButton(MouseButton::Left)]);

    let mut input = InputManager::new();
    input.push_event(InputEvent::MouseButton(MouseButton::Left, true));
    input.poll();

    actions.update(&input);
    assert!(actions.pressed("fire"));
    assert!(actions.just_pressed("fire"));
}

#[test]
fn input_actions_multiple_bindings_or() {
    let mut actions = InputActions::new();
    actions.bind("fire", &[
        ActionBinding::MouseButton(MouseButton::Left),
        ActionBinding::Key(KeyCode::Enter),
    ]);

    let mut input = InputManager::new();
    input.push_event(InputEvent::KeyPress(KeyCode::Enter));
    input.poll();

    actions.update(&input);
    assert!(actions.pressed("fire"));
}

#[test]
fn input_actions_handle_gamepad_button() {
    let event = InputEvent::GamepadButton(
        crate::input::GamepadId(0),
        crate::input::GamepadButton::South,
        true,
    );

    let mut actions = InputActions::new();
    let mut map = HashMap::new();
    map.insert("jump".to_string(), vec![ActionBinding::GamepadButton(crate::input::GamepadButton::South)]);
    actions.load_bindings(&map);

    actions.handle_gamepad_event(&event);
    // propagate to action state via update
    let input = InputManager::new();
    actions.update(&input);
    assert!(actions.pressed("jump"));
}

#[test]
fn input_actions_load_save_roundtrip() {
    let mut actions = InputActions::new();
    let mut map = HashMap::new();
    map.insert("jump".to_string(), vec![ActionBinding::Key(KeyCode::Space)]);
    map.insert("fire".to_string(), vec![ActionBinding::MouseButton(MouseButton::Left)]);
    actions.load_bindings(&map);

    let saved = actions.save_bindings();
    assert_eq!(saved.len(), 2);
    assert_eq!(saved["jump"], vec![ActionBinding::Key(KeyCode::Space)]);
}

#[test]
fn binding_config_serialize_roundtrip() {
    let mut config = BindingConfig::default();
    config.bindings.insert("test".to_string(), vec![ActionBinding::Key(KeyCode::A)]);

    let json = serde_json::to_string(&config).unwrap();
    let restored: BindingConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.bindings["test"], vec![ActionBinding::Key(KeyCode::A)]);
}

#[test]
fn action_state_default() {
    let s = ActionState::default();
    assert!(!s.pressed);
    assert!(!s.just_pressed);
    assert!(!s.just_released);
    assert_eq!(s.value, 0.0);
}
