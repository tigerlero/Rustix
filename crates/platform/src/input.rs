use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight, SuperLeft, SuperRight,
    Up, Down, Left, Right,
    PageUp, PageDown, Home, End,
    Space, Enter, Escape, Backspace, Tab, Delete, Insert,
    Minus, Equals, BracketLeft, BracketRight, Semicolon, Quote, Comma, Period, Slash, Backslash,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4,
    Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadEnter, NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadDecimal,
    CapsLock, NumLock, ScrollLock, Pause, PrintScreen, Menu,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton { Left, Right, Middle, Side(u8) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    South, East, North, West,
    LeftTrigger, RightTrigger,
    LeftShoulder, RightShoulder,
    Select, Start,
    LeftStick, RightStick,
    DPadUp, DPadDown, DPadLeft, DPadRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX, LeftStickY,
    RightStickX, RightStickY,
    LeftTrigger, RightTrigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GamepadId(pub u32);

#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyPress(KeyCode),
    KeyRelease(KeyCode),
    MouseMove(f32, f32),
    MouseButton(MouseButton, bool),
    MouseScroll(f32, f32),
    GamepadButton(GamepadId, GamepadButton, bool),
    GamepadAxis(GamepadId, GamepadAxis, f32),
    Text(char),
}

#[derive(Debug, Clone)]
pub struct KeyboardState {
    held: HashMap<KeyCode, bool>,
    just_pressed: Vec<KeyCode>,
    just_released: Vec<KeyCode>,
}

impl Default for KeyboardState {
    fn default() -> Self { Self::new() }
}

impl KeyboardState {
    pub fn new() -> Self {
        Self { held: HashMap::new(), just_pressed: Vec::new(), just_released: Vec::new() }
    }

    pub fn down(&self, key: KeyCode) -> bool {
        *self.held.get(&key).unwrap_or(&false)
    }

    pub fn just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed.contains(&key)
    }

    pub fn just_released(&self, key: KeyCode) -> bool {
        self.just_released.contains(&key)
    }

    fn press(&mut self, key: KeyCode) {
        self.held.insert(key, true);
        self.just_pressed.push(key);
    }

    fn release(&mut self, key: KeyCode) {
        self.held.insert(key, false);
        self.just_released.push(key);
    }

    fn end_tick(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
    }
}

#[derive(Debug, Clone)]
pub struct MouseState {
    position: (f32, f32),
    delta: (f32, f32),
    scroll: (f32, f32),
    buttons: HashMap<MouseButton, bool>,
    just_pressed: Vec<MouseButton>,
    just_released: Vec<MouseButton>,
}

impl Default for MouseState {
    fn default() -> Self { Self::new() }
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            position: (0.0, 0.0), delta: (0.0, 0.0), scroll: (0.0, 0.0),
            buttons: HashMap::new(), just_pressed: Vec::new(), just_released: Vec::new(),
        }
    }

    pub fn position(&self) -> (f32, f32) { self.position }
    pub fn delta(&self) -> (f32, f32) { self.delta }
    pub fn scroll(&self) -> (f32, f32) { self.scroll }
    pub fn down(&self, button: MouseButton) -> bool {
        *self.buttons.get(&button).unwrap_or(&false)
    }
    pub fn just_pressed(&self, button: MouseButton) -> bool {
        self.just_pressed.contains(&button)
    }
    pub fn just_released(&self, button: MouseButton) -> bool {
        self.just_released.contains(&button)
    }

    fn move_to(&mut self, x: f32, y: f32) {
        self.delta = (x - self.position.0, y - self.position.1);
        self.position = (x, y);
    }

    fn scroll_to(&mut self, x: f32, y: f32) { self.scroll = (x, y); }

    fn press(&mut self, button: MouseButton) {
        self.buttons.insert(button, true);
        self.just_pressed.push(button);
    }

    fn release(&mut self, button: MouseButton) {
        self.buttons.insert(button, false);
        self.just_released.push(button);
    }

    fn end_tick(&mut self) {
        self.delta = (0.0, 0.0);
        self.scroll = (0.0, 0.0);
        self.just_pressed.clear();
        self.just_released.clear();
    }
}

#[derive(Debug, Clone)]
pub struct GamepadState {
    buttons: HashMap<GamepadButton, bool>,
    axes: HashMap<GamepadAxis, f32>,
}

impl Default for GamepadState {
    fn default() -> Self { Self::new() }
}

impl GamepadState {
    pub fn new() -> Self {
        Self { buttons: HashMap::new(), axes: HashMap::new() }
    }

    pub fn button_down(&self, button: GamepadButton) -> bool {
        *self.buttons.get(&button).unwrap_or(&false)
    }

    pub fn axis(&self, axis: GamepadAxis) -> f32 {
        *self.axes.get(&axis).unwrap_or(&0.0)
    }
}

pub struct InputManager {
    keyboard: KeyboardState,
    mouse: MouseState,
    gamepads: HashMap<GamepadId, GamepadState>,
    pending_events: Vec<InputEvent>,
}

impl InputManager {
    pub fn new() -> Self {
        Self {
            keyboard: KeyboardState::new(),
            mouse: MouseState::new(),
            gamepads: HashMap::new(),
            pending_events: Vec::new(),
        }
    }

    pub fn push_event(&mut self, event: InputEvent) {
        self.pending_events.push(event);
    }

    pub fn poll(&mut self) {
        let events = std::mem::take(&mut self.pending_events);
        for event in &events {
            match event {
                InputEvent::KeyPress(key) => self.keyboard.press(*key),
                InputEvent::KeyRelease(key) => self.keyboard.release(*key),
                InputEvent::MouseMove(x, y) => self.mouse.move_to(*x, *y),
                InputEvent::MouseButton(btn, pressed) => {
                    if *pressed { self.mouse.press(*btn) } else { self.mouse.release(*btn) }
                }
                InputEvent::MouseScroll(x, y) => self.mouse.scroll_to(*x, *y),
                InputEvent::GamepadButton(id, btn, pressed) => {
                    let state = self.gamepads.entry(*id).or_default();
                    state.buttons.insert(*btn, *pressed);
                }
                InputEvent::GamepadAxis(id, axis, value) => {
                    let state = self.gamepads.entry(*id).or_default();
                    state.axes.insert(*axis, *value);
                }
                InputEvent::Text(_) => {}
            }
        }
    }

    pub fn end_tick(&mut self) {
        self.keyboard.end_tick();
        self.mouse.end_tick();
    }

    pub fn keyboard(&self) -> &KeyboardState { &self.keyboard }
    pub fn mouse(&self) -> &MouseState { &self.mouse }
    pub fn gamepad(&self, id: GamepadId) -> Option<&GamepadState> {
        self.gamepads.get(&id)
    }

    pub fn handle_winit_event(&mut self, event: &winit::event::WindowEvent) {
        match event {
            winit::event::WindowEvent::KeyboardInput { event: key_event, .. } => {
                if let Some(keycode) = convert_winit_key(&key_event.physical_key) {
                    let e = if key_event.state == winit::event::ElementState::Pressed {
                        InputEvent::KeyPress(keycode)
                    } else {
                        InputEvent::KeyRelease(keycode)
                    };
                    self.push_event(e);
                }
            }
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                self.push_event(InputEvent::MouseMove(position.x as f32, position.y as f32));
            }
            winit::event::WindowEvent::MouseInput { state, button, .. } => {
                let btn = convert_winit_mouse(*button);
                self.push_event(InputEvent::MouseButton(btn, *state == winit::event::ElementState::Pressed));
            }
            winit::event::WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        self.push_event(InputEvent::MouseScroll(*x, *y));
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        self.push_event(InputEvent::MouseScroll(pos.x as f32, pos.y as f32));
                    }
                }
            }
            _ => {}
        }
    }
}

fn convert_winit_key(key: &winit::keyboard::PhysicalKey) -> Option<KeyCode> {
    use winit::keyboard::KeyCode as W;
    use winit::keyboard::PhysicalKey as P;
    Some(match key {
        P::Code(W::KeyA) => KeyCode::A,
        P::Code(W::KeyB) => KeyCode::B,
        P::Code(W::KeyC) => KeyCode::C,
        P::Code(W::KeyD) => KeyCode::D,
        P::Code(W::KeyE) => KeyCode::E,
        P::Code(W::KeyF) => KeyCode::F,
        P::Code(W::KeyG) => KeyCode::G,
        P::Code(W::KeyH) => KeyCode::H,
        P::Code(W::KeyI) => KeyCode::I,
        P::Code(W::KeyJ) => KeyCode::J,
        P::Code(W::KeyK) => KeyCode::K,
        P::Code(W::KeyL) => KeyCode::L,
        P::Code(W::KeyM) => KeyCode::M,
        P::Code(W::KeyN) => KeyCode::N,
        P::Code(W::KeyO) => KeyCode::O,
        P::Code(W::KeyP) => KeyCode::P,
        P::Code(W::KeyQ) => KeyCode::Q,
        P::Code(W::KeyR) => KeyCode::R,
        P::Code(W::KeyS) => KeyCode::S,
        P::Code(W::KeyT) => KeyCode::T,
        P::Code(W::KeyU) => KeyCode::U,
        P::Code(W::KeyV) => KeyCode::V,
        P::Code(W::KeyW) => KeyCode::W,
        P::Code(W::KeyX) => KeyCode::X,
        P::Code(W::KeyY) => KeyCode::Y,
        P::Code(W::KeyZ) => KeyCode::Z,
        P::Code(W::Digit0) => KeyCode::Key0,
        P::Code(W::Digit1) => KeyCode::Key1,
        P::Code(W::Digit2) => KeyCode::Key2,
        P::Code(W::Digit3) => KeyCode::Key3,
        P::Code(W::Digit4) => KeyCode::Key4,
        P::Code(W::Digit5) => KeyCode::Key5,
        P::Code(W::Digit6) => KeyCode::Key6,
        P::Code(W::Digit7) => KeyCode::Key7,
        P::Code(W::Digit8) => KeyCode::Key8,
        P::Code(W::Digit9) => KeyCode::Key9,
        P::Code(W::F1) => KeyCode::F1,
        P::Code(W::F2) => KeyCode::F2,
        P::Code(W::F3) => KeyCode::F3,
        P::Code(W::F4) => KeyCode::F4,
        P::Code(W::F5) => KeyCode::F5,
        P::Code(W::F6) => KeyCode::F6,
        P::Code(W::F7) => KeyCode::F7,
        P::Code(W::F8) => KeyCode::F8,
        P::Code(W::F9) => KeyCode::F9,
        P::Code(W::F10) => KeyCode::F10,
        P::Code(W::F11) => KeyCode::F11,
        P::Code(W::F12) => KeyCode::F12,
        P::Code(W::ShiftLeft) => KeyCode::ShiftLeft,
        P::Code(W::ShiftRight) => KeyCode::ShiftRight,
        P::Code(W::ControlLeft) => KeyCode::ControlLeft,
        P::Code(W::ControlRight) => KeyCode::ControlRight,
        P::Code(W::AltLeft) => KeyCode::AltLeft,
        P::Code(W::AltRight) => KeyCode::AltRight,
        P::Code(W::SuperLeft) => KeyCode::SuperLeft,
        P::Code(W::SuperRight) => KeyCode::SuperRight,
        P::Code(W::ArrowUp) => KeyCode::Up,
        P::Code(W::ArrowDown) => KeyCode::Down,
        P::Code(W::ArrowLeft) => KeyCode::Left,
        P::Code(W::ArrowRight) => KeyCode::Right,
        P::Code(W::PageUp) => KeyCode::PageUp,
        P::Code(W::PageDown) => KeyCode::PageDown,
        P::Code(W::Home) => KeyCode::Home,
        P::Code(W::End) => KeyCode::End,
        P::Code(W::Space) => KeyCode::Space,
        P::Code(W::Enter) => KeyCode::Enter,
        P::Code(W::Escape) => KeyCode::Escape,
        P::Code(W::Backspace) => KeyCode::Backspace,
        P::Code(W::Tab) => KeyCode::Tab,
        P::Code(W::Delete) => KeyCode::Delete,
        P::Code(W::Insert) => KeyCode::Insert,
        P::Code(W::Minus) => KeyCode::Minus,
        P::Code(W::Equal) => KeyCode::Equals,
        P::Code(W::BracketLeft) => KeyCode::BracketLeft,
        P::Code(W::BracketRight) => KeyCode::BracketRight,
        P::Code(W::Semicolon) => KeyCode::Semicolon,
        P::Code(W::Quote) => KeyCode::Quote,
        P::Code(W::Comma) => KeyCode::Comma,
        P::Code(W::Period) => KeyCode::Period,
        P::Code(W::Slash) => KeyCode::Slash,
        P::Code(W::Backslash) => KeyCode::Backslash,
        P::Code(W::Numpad0) => KeyCode::Numpad0,
        P::Code(W::Numpad1) => KeyCode::Numpad1,
        P::Code(W::Numpad2) => KeyCode::Numpad2,
        P::Code(W::Numpad3) => KeyCode::Numpad3,
        P::Code(W::Numpad4) => KeyCode::Numpad4,
        P::Code(W::Numpad5) => KeyCode::Numpad5,
        P::Code(W::Numpad6) => KeyCode::Numpad6,
        P::Code(W::Numpad7) => KeyCode::Numpad7,
        P::Code(W::Numpad8) => KeyCode::Numpad8,
        P::Code(W::Numpad9) => KeyCode::Numpad9,
        P::Code(W::NumpadEnter) => KeyCode::NumpadEnter,
        P::Code(W::NumpadAdd) => KeyCode::NumpadAdd,
        P::Code(W::NumpadSubtract) => KeyCode::NumpadSubtract,
        P::Code(W::NumpadMultiply) => KeyCode::NumpadMultiply,
        P::Code(W::NumpadDivide) => KeyCode::NumpadDivide,
        P::Code(W::NumpadDecimal) => KeyCode::NumpadDecimal,
        P::Code(W::CapsLock) => KeyCode::CapsLock,
        P::Code(W::NumLock) => KeyCode::NumLock,
        P::Code(W::ScrollLock) => KeyCode::ScrollLock,
        P::Code(W::Pause) => KeyCode::Pause,
        P::Code(W::PrintScreen) => KeyCode::PrintScreen,
        P::Code(W::ContextMenu) => KeyCode::Menu,
        _ => KeyCode::Unknown,
    })
}

fn convert_winit_mouse(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Side(1),
        winit::event::MouseButton::Forward => MouseButton::Side(2),
        _ => MouseButton::Side(0),
    }
}
