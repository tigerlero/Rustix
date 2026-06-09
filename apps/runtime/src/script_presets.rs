//! Script presets for common game behaviors.
//!
//! These are Rhai script templates that users can attach to entities via
//! the Inspector panel. Each preset modifies the entity's transform
//! using the scripting API (translation, rotation, scale).

/// Available script preset names for the UI dropdown.
pub const PRESET_NAMES: &[&str] = &[
    "(Custom)",
    "Character Controller",
    "Camera Controller",
    "Weather Control",
    "Enemy Controller",
];

/// Get the Rhai source for a named preset.
pub fn get_preset(name: &str) -> Option<String> {
    match name {
        "Character Controller" => Some(character_controller()),
        "Camera Controller" => Some(camera_controller()),
        "Weather Control" => Some(weather_control()),
        "Enemy Controller" => Some(enemy_controller()),
        _ => None,
    }
}

/// Moves the entity in a horizontal circle (autonomous character movement).
fn character_controller() -> String {
    r#"// Character Controller
// Moves the entity in a circular path.
// Adjust radius and speed below.

let radius = 5.0;
let speed = 1.0;
let t = time();

let x = radius * sin(t * speed);
let z = radius * cos(t * speed);

translation.x = x;
translation.z = z;
"#.to_string()
}

/// Orbits the entity around the origin (useful for demo cameras or drones).
fn camera_controller() -> String {
    r#"// Camera Controller
// Orbits the entity around the origin with vertical bobbing.
// Attach to a camera entity and set it to look at the origin.

let t = time();
let radius = 10.0;
let speed = 0.5;
let height = 5.0;
let bob = 2.0;

translation.x = radius * sin(t * speed);
translation.z = radius * cos(t * speed);
translation.y = height + bob * sin(t * 0.3);
"#.to_string()
}

/// Pulses scale over time (good for rain, clouds, or magical effects).
fn weather_control() -> String {
    r#"// Weather Control
// Oscillates scale to simulate breathing/pulsing weather elements.
// Adjust base_scale, amplitude, and frequency.

let t = time();
let base_scale = 1.0;
let amplitude = 0.2;
let frequency = 2.0;

let s = base_scale + amplitude * sin(t * frequency);
scale = vec3(s, s, s);
"#.to_string()
}

/// Patrols back and forth along the X axis (simple enemy patrol AI).
fn enemy_controller() -> String {
    r#"// Enemy Controller
// Patrols back and forth along the X axis.
// Adjust patrol_range and speed to fit your scene.

let t = time();
let patrol_range = 8.0;
let speed = 1.5;

let x = patrol_range * sin(t * speed);
translation.x = x;
"#.to_string()
}
