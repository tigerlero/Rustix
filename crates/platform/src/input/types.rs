use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MouseButton { Left, Right, Middle, Side(u8) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GamepadButton {
    South, East, North, West,
    LeftTrigger, RightTrigger,
    LeftShoulder, RightShoulder,
    Select, Start,
    LeftStick, RightStick,
    DPadUp, DPadDown, DPadLeft, DPadRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GamepadAxis {
    LeftStickX, LeftStickY,
    RightStickX, RightStickY,
    LeftTrigger, RightTrigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GamepadId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TouchPhase { Started, Moved, Ended, Cancelled }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub force: Option<f32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum InputEvent {
    KeyPress(KeyCode),
    KeyRelease(KeyCode),
    MouseMove(f32, f32),
    MouseButton(MouseButton, bool),
    MouseScroll(f32, f32),
    RawMouseMotion(f32, f32),
    GamepadButton(GamepadId, GamepadButton, bool),
    GamepadAxis(GamepadId, GamepadAxis, f32),
    Text(char),
    ImeEnabled,
    ImeDisabled,
    ImePreedit(String, Option<(usize, usize)>),
    ImeCommit(String),
    Touch { id: u64, phase: TouchPhase, x: f32, y: f32, force: Option<f32> },
}

/// Tracks IME composition state and committed text.
#[derive(Debug, Clone, Default)]
pub struct TextInputState {
    pub enabled: bool,
    pub preedit: String,
    pub preedit_cursor: Option<(usize, usize)>,
    pub committed: String,
}

/// Tracks active touch points and per-frame gesture helpers.
#[derive(Debug, Clone, Default)]
pub struct TouchState {
    /// Currently active touch points keyed by finger id.
    pub active: HashMap<u64, TouchPoint>,
    /// Previous frame positions for active touches (used for gesture deltas).
    pub(crate) prev_active: HashMap<u64, (f32, f32)>,
    /// Touch points that started this frame.
    pub started: Vec<TouchPoint>,
    /// Touch points that ended this frame.
    pub ended: Vec<TouchPoint>,
    /// Total two-finger pinch delta this frame (negative = pinch in, positive = pinch out).
    pub pinch_delta: f32,
    /// Total two-finger scroll delta this frame.
    pub two_finger_scroll: (f32, f32),
}
