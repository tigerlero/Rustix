use std::collections::HashMap;

use crate::input::types::{GamepadAxis, GamepadButton, KeyCode, MouseButton};

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

    pub(crate) fn press(&mut self, key: KeyCode) {
        self.held.insert(key, true);
        self.just_pressed.push(key);
    }

    pub(crate) fn release(&mut self, key: KeyCode) {
        self.held.insert(key, false);
        self.just_released.push(key);
    }

    pub(crate) fn end_tick(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
    }
}

#[derive(Debug, Clone)]
pub struct MouseState {
    position: (f32, f32),
    delta: (f32, f32),
    raw_delta: (f32, f32),
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
            position: (0.0, 0.0), delta: (0.0, 0.0), raw_delta: (0.0, 0.0), scroll: (0.0, 0.0),
            buttons: HashMap::new(), just_pressed: Vec::new(), just_released: Vec::new(),
        }
    }

    pub fn position(&self) -> (f32, f32) { self.position }
    pub fn delta(&self) -> (f32, f32) { self.delta }
    pub fn raw_delta(&self) -> (f32, f32) { self.raw_delta }
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

    pub(crate) fn move_to(&mut self, x: f32, y: f32) {
        self.delta = (x - self.position.0, y - self.position.1);
        self.position = (x, y);
    }

    pub(crate) fn add_raw_delta(&mut self, x: f32, y: f32) {
        self.raw_delta = (self.raw_delta.0 + x, self.raw_delta.1 + y);
    }

    pub(crate) fn scroll_to(&mut self, x: f32, y: f32) { self.scroll = (x, y); }

    pub(crate) fn press(&mut self, button: MouseButton) {
        self.buttons.insert(button, true);
        self.just_pressed.push(button);
    }

    pub(crate) fn release(&mut self, button: MouseButton) {
        self.buttons.insert(button, false);
        self.just_released.push(button);
    }

    pub(crate) fn end_tick(&mut self) {
        self.delta = (0.0, 0.0);
        self.raw_delta = (0.0, 0.0);
        self.scroll = (0.0, 0.0);
        self.just_pressed.clear();
        self.just_released.clear();
    }
}

#[derive(Debug, Clone)]
pub struct GamepadState {
    pub(crate) buttons: HashMap<GamepadButton, bool>,
    pub(crate) axes: HashMap<GamepadAxis, f32>,
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
