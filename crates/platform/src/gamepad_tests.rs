//! Tests for gamepad input stub.

use crate::gamepad::GamepadInput;

#[test]
fn gamepad_input_new() {
    let input = GamepadInput::new();
    assert_eq!(input.connected_count(), 0);
}

#[test]
fn gamepad_input_poll_empty() {
    let mut input = GamepadInput::new();
    let events = input.poll();
    assert!(events.is_empty());
}

#[test]
fn gamepad_input_connected_count_zero() {
    let input = GamepadInput::new();
    assert_eq!(input.connected_count(), 0);
}
