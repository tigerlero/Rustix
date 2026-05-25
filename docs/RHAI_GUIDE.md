# Rhai Scripting Guide for Rustix Engine

## Overview

Rhai is a lightweight, embeddable scripting language for Rust. It's designed for game development with fast performance and easy integration with Rust code.

## Getting Started

### Basic Script Syntax

```rhai
// Variables and types
let x = 42;
let name = "player";
let health = 100.0;
let position = vec3(10.0, 0.0, 5.0);

// Control flow
if health <= 0 {
    print("Player is dead!");
} else {
    health -= 10;
}

// Functions
fn calculate_damage(base, multiplier) {
    return base * multiplier;
}

// Loops
for i in 0..10 {
    print(i);
}
```

## Engine Integration

### Script Component

Attach scripts to entities using the `ScriptComponent`:

```rust
// In Rust code
let script = ScriptComponent {
    source: r#"
        fn on_start(entity) {
            print("Entity started!");
        }
        
        fn on_update(entity, dt) {
            // Game logic here
        }
    "#.to_string(),
    enabled: true,
};
```

### Writing Scripts for Entities

```rhai
// on_start - called when entity is created
fn on_start(entity_id) {
    print("Initializing entity " + entity_id);
}

// on_update - called every frame with delta time
fn on_update(entity_id, delta_time) {
    // Movement example
    let speed = 5.0;
    let distance = speed * delta_time;
    move_entity(entity_id, 0, 0, distance);
}

// on_collision - called when entity collides
fn on_collision(entity_id, other_id) {
    print("Collided with " + other_id);
    damage_entity(other_id, 10);
}
```

## Available Engine Functions

### Math Functions

```rhai
let pos = vec3(1.0, 2.0, 3.0);     // Create a 3D vector
let normalized = normalize(pos);     // Normalize vector
let length = length(pos);          // Get length
let dot = dot(pos, vec3(1,0,0));    // Dot product
```

### Entity Functions

```rhai
// Get entity position (to be implemented)
let pos = get_position(entity_id);

// Set entity position (to be implemented)
set_position(entity_id, vec3(10, 0, 0));

// Get component value (to be implemented)
let health = get_component(entity_id, "health");
set_component(entity_id, "health", 50);
```

## Script Examples

### Basic Movement Script

```rhai
// Move entity left/right with A/D keys
fn on_update(entity_id, dt) {
    let speed = 10.0;
    let velocity = vec3(0, 0, 0);
    
    if is_key_down("A") {
        velocity.x = -speed;
    }
    if is_key_down("D") {
        velocity.x = speed;
    }
    
    move_entity(entity_id, velocity.x * dt, 0, 0);
}
```

### Timer Script

```rhai
fn on_start(entity_id) {
    set_timer(entity_id, "explosion", 3.0);
}

fn on_timer(entity_id, timer_name) {
    if timer_name == "explosion" {
        create_effect("explosion", get_position(entity_id));
        destroy_entity(entity_id);
    }
}
```

### State Machine Script

```rhai
let state = "idle";
let timer = 0.0;

fn on_update(entity_id, dt) {
    timer += dt;
    
    if state == "idle" {
        if timer > 2.0 {
            state = "moving";
            timer = 0.0;
        }
    } else if state == "moving" {
        move_entity(entity_id, 5.0 * dt, 0, 0);
        if timer > 3.0 {
            state = "idle";
            timer = 0.0;
        }
    }
}
```

## Data Types

### Supported Types

| Type | Rhai Syntax | Rust Equivalent |
|------|-------------|----------------|
| Integer | `42` | `i64` |
| Float | `3.14` | `f64` |
| String | `"hello"` | `String` |
| Boolean | `true` / `false` | `bool` |
| Vec3 | `vec3(1, 2, 3)` | `glam::Vec3` |
| Quat | `quat(0, 0, 0, 1)` | `glam::Quat` |
| Array | `[1, 2, 3]` | `rhai::Array` |
| Map | `#{"a": 1}` | `rhai::Map` |

## Performance Tips

1. **Cache function results** - Store expensive calculations in variables
2. **Use `const` for constants** - Compile-time evaluation
3. **Avoid dynamic dispatch** - Use concrete types when possible
4. **Batch operations** - Process multiple entities in one script call

## Debugging

```rhai
// Print to console
print("Debug value: " + value);

// Inspect variable types
print(type_of(variable));
```

## Error Handling

```rhai
try {
    let result = risky_operation();
} catch (error) {
    print("Error: " + error);
}
```

## Integration with Asset System

### Loading Scripts

```rust
use rustix_scripting::ScriptLoader;

// From file
let script = ScriptLoader::load(Path::new("scripts/player.rhai"))?;

// From memory
let script = ScriptLoader::load_from_memory(source_code);
```

### Hot Reloading

Scripts support hot-reloading in debug builds. Save your `.rhai` file and it will automatically reload in the running game.

## Best Practices

1. **Keep scripts focused** - One script per behavior
2. **Use functions** - Organize code into reusable functions
3. **Avoid deep nesting** - Flatten logic with early returns
4. **Document complex scripts** - Add comments explaining behavior
5. **Test in isolation** - Verify script behavior independently

## Advanced Features

### Custom Types

```rust
// Register a custom type in Rust
engine.register_type_with_name::<MyStruct>("MyStruct");
```

### Module System

```rhai
// Import modules (future feature)
import "std";
import "math";
```

## Roadmap

- [ ] Full ECS integration (get/set components)
- [ ] Entity queries and iteration
- [ ] Coroutine support for async operations
- [ ] Visual scripting node integration
- [ ] Debug inspector for script variables