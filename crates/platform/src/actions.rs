use std::collections::HashMap;

use crate::input::{GamepadButton, GamepadAxis, InputEvent, InputManager, KeyCode, MouseButton};

/// What kind of physical input can trigger an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ActionBinding {
    Key(KeyCode),
    MouseButton(MouseButton),
    GamepadButton(GamepadButton),
    /// Axis input (deadzone is fixed at 0.15).
    GamepadAxis { axis: GamepadAxis, positive: bool },
}

/// Action state queried by game code each frame.
#[derive(Debug, Clone, Default)]
pub struct ActionState {
    pub pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
    pub value: f32, // For analog inputs, 0..1
}

/// Maps abstract action names to physical bindings and tracks their state.
pub struct InputActions {
    bindings: HashMap<String, Vec<ActionBinding>>,
    state: HashMap<String, ActionState>,
    // Per-binding raw state for disambiguation
    binding_pressed: HashMap<ActionBinding, bool>,
    binding_just_pressed: HashMap<ActionBinding, bool>,
    binding_just_released: HashMap<ActionBinding, bool>,
    binding_value: HashMap<ActionBinding, f32>,
}

impl Default for InputActions {
    fn default() -> Self { Self::new() }
}

impl InputActions {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            state: HashMap::new(),
            binding_pressed: HashMap::new(),
            binding_just_pressed: HashMap::new(),
            binding_just_released: HashMap::new(),
            binding_value: HashMap::new(),
        }
    }

    /// Register one or more bindings for an action name.
    pub fn bind(&mut self, action: &str, bindings: &[ActionBinding]) {
        self.bindings.insert(action.to_string(), bindings.to_vec());
        self.state.entry(action.to_string()).or_default();
    }

    /// Remove all bindings for an action.
    pub fn unbind(&mut self, action: &str) {
        self.bindings.remove(action);
        self.state.remove(action);
    }

    /// Bind a set of common actions suitable for most projects.
    pub fn bind_defaults(&mut self) {
        self.bind("jump", &[ActionBinding::Key(KeyCode::Space)]);
        self.bind("fire", &[
            ActionBinding::MouseButton(MouseButton::Left),
            ActionBinding::GamepadButton(GamepadButton::South),
        ]);
        self.bind("move_forward", &[ActionBinding::Key(KeyCode::W)]);
        self.bind("move_back", &[ActionBinding::Key(KeyCode::S)]);
        self.bind("move_left", &[ActionBinding::Key(KeyCode::A)]);
        self.bind("move_right", &[ActionBinding::Key(KeyCode::D)]);
        self.bind("look_horizontal", &[
            ActionBinding::GamepadAxis { axis: GamepadAxis::RightStickX, positive: true },
        ]);
        self.bind("look_vertical", &[
            ActionBinding::GamepadAxis { axis: GamepadAxis::RightStickY, positive: true },
        ]);
        self.bind("pause", &[ActionBinding::Key(KeyCode::Escape)]);
    }

    /// Update action states from the raw `InputManager`.
    /// Call this **after** `input.poll()` each frame.
    pub fn update(&mut self, input: &InputManager) {
        // Clear per-frame flags
        for (_, b) in self.binding_just_pressed.iter_mut() { *b = false; }
        for (_, b) in self.binding_just_released.iter_mut() { *b = false; }

        let kb = input.keyboard();
        let ms = input.mouse();

        // Evaluate each known binding
        for (binding, pressed) in self.binding_pressed.iter_mut() {
            let new_pressed = match binding {
                ActionBinding::Key(k) => kb.down(*k),
                ActionBinding::MouseButton(b) => ms.down(*b),
                ActionBinding::GamepadButton(_) => {
                    // Gamepad state is already in InputManager via push_event
                    // but we can't query per-gamepad here easily.
                    // For now keep previous state; user should wire gamepad events.
                    *pressed
                }
                ActionBinding::GamepadAxis { axis: _, positive: _ } => {
                    // Analog: value is updated below, pressed follows value > 0
                    *pressed
                }
            };
            if new_pressed && !*pressed {
                self.binding_just_pressed.insert(*binding, true);
            }
            if !new_pressed && *pressed {
                self.binding_just_released.insert(*binding, true);
            }
            *pressed = new_pressed;
        }

        // Aggregate per-action state from bindings
        for (action, bindings) in &self.bindings {
            let mut pressed = false;
            let mut just_pressed = false;
            let mut just_released = false;
            let mut value = 0.0f32;

            for b in bindings {
                let bp = *self.binding_pressed.get(b).unwrap_or(&false);
                pressed = pressed || bp;
                just_pressed = just_pressed || *self.binding_just_pressed.get(b).unwrap_or(&false);
                just_released = just_released || *self.binding_just_released.get(b).unwrap_or(&false);
                let bv = self.binding_value.get(b).copied().unwrap_or(0.0);
                value = value.max(bv);
            }

            let state = self.state.entry(action.clone()).or_default();
            state.pressed = pressed;
            state.just_pressed = just_pressed;
            state.just_released = just_released;
            state.value = value.max(if pressed { 1.0 } else { 0.0 });
        }
    }

    /// Feed a raw gamepad event to update analog / button binding state.
    pub fn handle_gamepad_event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::GamepadButton(_, btn, pressed) => {
                let binding = ActionBinding::GamepadButton(*btn);
                if let Some(b) = self.binding_pressed.get_mut(&binding) {
                    if *pressed && !*b {
                        self.binding_just_pressed.insert(binding, true);
                    }
                    if !*pressed && *b {
                        self.binding_just_released.insert(binding, true);
                    }
                    *b = *pressed;
                }
            }
            InputEvent::GamepadAxis(_, axis, val) => {
                let binding_pos = ActionBinding::GamepadAxis { axis: *axis, positive: true };
                let binding_neg = ActionBinding::GamepadAxis { axis: *axis, positive: false };
                for (b, v) in [(&binding_pos, *val), (&binding_neg, -*val)] {
                    if self.bindings.values().any(|list| list.contains(b)) {
                        let clamped = v.max(-1.0).min(1.0);
                        self.binding_value.insert(*b, if clamped.abs() > 0.15 { clamped } else { 0.0 });
                    }
                }
            }
            _ => {}
        }
    }

    /// Query the current state of an action.
    pub fn state(&self, action: &str) -> &ActionState {
        self.state.get(action).unwrap_or(&DEFAULT_STATE)
    }

    pub fn pressed(&self, action: &str) -> bool { self.state(action).pressed }
    pub fn just_pressed(&self, action: &str) -> bool { self.state(action).just_pressed }
    pub fn just_released(&self, action: &str) -> bool { self.state(action).just_released }
    pub fn value(&self, action: &str) -> f32 { self.state(action).value }

    /// Replace all current bindings from a config map.
    pub fn load_bindings(&mut self, map: &HashMap<String, Vec<ActionBinding>>) {
        self.bindings.clear();
        self.state.clear();
        self.binding_pressed.clear();
        self.binding_just_pressed.clear();
        self.binding_just_released.clear();
        self.binding_value.clear();
        for (action, bindings) in map {
            self.bind(action, bindings);
            for b in bindings {
                self.binding_pressed.entry(*b).or_insert(false);
            }
        }
    }

    /// Export current bindings to a config map.
    pub fn save_bindings(&self) -> HashMap<String, Vec<ActionBinding>> {
        self.bindings.clone()
    }
}

/// Serializable representation of an input bindings config file.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BindingConfig {
    pub bindings: HashMap<String, Vec<ActionBinding>>,
}

/// Load binding config from a JSON file path, falling back to defaults if missing or corrupt.
pub fn load_binding_config(path: &std::path::Path) -> Option<BindingConfig> {
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

/// Save binding config to a JSON file path.
pub fn save_binding_config(path: &std::path::Path, config: &BindingConfig) -> Option<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(config).ok()?;
    std::fs::write(path, json).ok()
}

static DEFAULT_STATE: ActionState = ActionState {
    pressed: false,
    just_pressed: false,
    just_released: false,
    value: 0.0,
};
