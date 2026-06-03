use crate::input::{GamepadAxis, GamepadButton, GamepadId, InputEvent};

/// Gamepad input source.
/// When the `gamepad` feature is enabled, this is backed by `gilrs`.
/// Otherwise it is a no-op stub.
pub struct GamepadInput {
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    #[cfg(not(feature = "gamepad"))]
    _dummy: (),
}

impl GamepadInput {
    pub fn new() -> Self {
        #[cfg(feature = "gamepad")]
        {
            match gilrs::Gilrs::new() {
                Ok(g) => {
                    tracing::info!("gamepad: gilrs initialized");
                    Self { gilrs: Some(g) }
                }
                Err(e) => {
                    tracing::warn!("gamepad: gilrs init failed: {e}");
                    Self { gilrs: None }
                }
            }
        }
        #[cfg(not(feature = "gamepad"))]
        {
            Self { _dummy: () }
        }
    }

    /// Poll pending gamepad events and return them.
    pub fn poll(&mut self) -> Vec<InputEvent> {
        #[cfg(feature = "gamepad")]
        {
            let Some(ref mut gilrs) = self.gilrs else { return Vec::new() };
            let mut out = Vec::new();
            while let Some(ev) = gilrs.next_event() {
                let id = GamepadId(ev.event.gamepad_id());
                match ev.event {
                    gilrs::EventType::ButtonPressed(btn, _code) => {
                        if let Some(b) = convert_button(btn) {
                            out.push(InputEvent::GamepadButton(id, b, true));
                        }
                    }
                    gilrs::EventType::ButtonReleased(btn, _code) => {
                        if let Some(b) = convert_button(btn) {
                            out.push(InputEvent::GamepadButton(id, b, false));
                        }
                    }
                    gilrs::EventType::AxisChanged(axis, value, _code) => {
                        if let Some(a) = convert_axis(axis) {
                            out.push(InputEvent::GamepadAxis(id, a, value));
                        }
                    }
                    _ => {}
                }
            }
            out
        }
        #[cfg(not(feature = "gamepad"))]
        {
            Vec::new()
        }
    }

    /// Number of connected gamepads.
    pub fn connected_count(&self) -> usize {
        #[cfg(feature = "gamepad")]
        {
            self.gilrs.as_ref().map_or(0, |g| g.gamepads().count())
        }
        #[cfg(not(feature = "gamepad"))]
        {
            0
        }
    }
}

#[cfg(feature = "gamepad")]
fn convert_button(btn: gilrs::Button) -> Option<GamepadButton> {
    Some(match btn {
        gilrs::Button::South => GamepadButton::South,
        gilrs::Button::East => GamepadButton::East,
        gilrs::Button::North => GamepadButton::North,
        gilrs::Button::West => GamepadButton::West,
        gilrs::Button::LeftTrigger => GamepadButton::LeftTrigger,
        gilrs::Button::RightTrigger => GamepadButton::RightTrigger,
        gilrs::Button::LeftTrigger2 => GamepadButton::LeftShoulder,
        gilrs::Button::RightTrigger2 => GamepadButton::RightShoulder,
        gilrs::Button::Select => GamepadButton::Select,
        gilrs::Button::Start => GamepadButton::Start,
        gilrs::Button::LeftThumb => GamepadButton::LeftStick,
        gilrs::Button::RightThumb => GamepadButton::RightStick,
        gilrs::Button::DPadUp => GamepadButton::DPadUp,
        gilrs::Button::DPadDown => GamepadButton::DPadDown,
        gilrs::Button::DPadLeft => GamepadButton::DPadLeft,
        gilrs::Button::DPadRight => GamepadButton::DPadRight,
        _ => return None,
    })
}

#[cfg(feature = "gamepad")]
fn convert_axis(axis: gilrs::Axis) -> Option<GamepadAxis> {
    Some(match axis {
        gilrs::Axis::LeftStickX => GamepadAxis::LeftStickX,
        gilrs::Axis::LeftStickY => GamepadAxis::LeftStickY,
        gilrs::Axis::RightStickX => GamepadAxis::RightStickX,
        gilrs::Axis::RightStickY => GamepadAxis::RightStickY,
        gilrs::Axis::LeftZ => GamepadAxis::LeftTrigger,
        gilrs::Axis::RightZ => GamepadAxis::RightTrigger,
        _ => return None,
    })
}
