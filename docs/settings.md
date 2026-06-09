# Settings — Missing Features

This document tracks settings that exist in `AppState` or `ProjectSettings` but are **not yet exposed** in the Settings UI (`post_process_panel` / Settings window).

---

## Currently Exposed

### Post-Process (`post_process.rs`)
| Group | Controls |
|---|---|
| **Bloom** | Threshold, Intensity |
| **SSAO** | Enabled, Radius, Bias, Power, Intensity |
| **TAA** | Enabled, Blend Factor |
| **SSR** | Enabled, Max Steps, Stride, Max Distance |
| **Volumetric Fog** | Enabled, Density, Scattering, Height Falloff, Max Distance, Max Steps, Sun Intensity |
| **Skybox / Atmosphere** | Enabled, Rayleigh, Mie, Zenith Shift, Exposure |
| **Rendering Tech** | Instanced Rendering, GPU Culling, Mesh Shaders (NV), Ordered Independent Transparency |

### Project (`main.rs` Settings window)
| Control | Notes |
|---|---|
| Resolution (Width / Height) | Only visible when project is open |
| Enable V-Sync | |
| Target FPS | |
| Project Type | 3D / 2D toggle |

---

## Missing from Settings UI

### Audio
| Field | Type | Default | Description |
|---|---|---|---|
| `audio_engine` master volume | `f32` | system default | Global audio gain |
| `audio_instance` playback controls | — | None | Play / pause / stop / loop for loaded sounds |
| `waveform_viewer` zoom / offset | `f32` / `f32` | — | Timeline zoom and scroll position |

### Physics
| Field | Type | Default | Description |
|---|---|---|---|
| `physics_world.gravity` | `Vec3` | `(0, -9.81, 0)` | Gravity vector |
| `physics_world.enabled` | `bool` | `true` | Enable/disable simulation |
| `physics_world.substeps` | `u32` | `4` | Simulation substeps per frame |
| `physics_world.timestep` | `f32` | `1/60` | Fixed timestep |

### Shadows & Lighting
| Field | Type | Default | Description |
|---|---|---|---|
| Shadow map resolution | `u32` | 2048 | CSM / point / spot shadow texture size |
| CSM cascade splits | `[f32; 4]` | — | Distance thresholds for cascade layers |
| CSM enabled | `bool` | `true` | Toggle cascade shadow maps |
| Point shadow enabled | `bool` | `true` | Toggle point light shadows |
| Spot shadow enabled | `bool` | `true` | Toggle spot light shadows |

### Rendering Pipeline
| Field | Type | Default | Description |
|---|---|---|---|
| `hdr_framebuffer` enabled | `bool` | `true` | HDR intermediate target |
| Forward+ vs Deferred | `enum` | Forward+ | Lighting pipeline mode |
| GBuffer enabled | `bool` | `true` | Deferred gbuffer pass (for SSR/SSAO) |
| Tonemap curve | `enum` | Reinhard | ACES, Reinhard, Uncharted2, etc. |
| Exposure EV | `f32` | `0.0` | Additional exposure stop |
| Gamma correction | `f32` | `2.2` | Output gamma |

### Editor Preferences
| Field | Type | Default | Description |
|---|---|---|---|
| Show grid | `bool` | `true` | Viewport ground grid visibility |
| Grid size | `f32` | `1.0` | Grid cell size |
| Camera move speed | `f32` | `5.0` | WASD fly speed |
| Camera rotate speed | `f32` | `1.0` | Mouse look sensitivity |
| Gizmo size | `f32` | `80.0` | On-screen gizmo pixel size |
| Default snap size | `f32` | `0.5` | Translate snap increment |
| Default snap rotate | `f32` | `15.0` | Rotate snap increment (degrees) |
| Default snap scale | `f32` | `0.1` | Scale snap increment |
| Frame graph overlay | `bool` | `false` | F10 toggle — add to UI |

### Project Settings (Not Yet in UI)
| Field | Type | Default | Description |
|---|---|---|---|
| Physics enabled | `bool` | `true` | Global physics toggle for project |
| Audio enabled | `bool` | `true` | Global audio toggle for project |
| Startup scene | `String` | `"main"` | Scene loaded on project open |
| Build target | `enum` | Linux | Target platform for builds |
| Scripting enabled | `bool` | `true` | Rhai scripting system toggle |
| Auto-save interval | `u32` | `300` | Seconds between auto-saves |

### Input / Recording
| Field | Type | Default | Description |
|---|---|---|---|
| `input_recorder.enabled` | `bool` | `false` | Record input for replay |
| `recording_dir` | `PathBuf` | `~/.config/rustix/recordings` | Save location |

---

## Suggested UI Layout

Add tabs or collapsible sections to the Settings window:

1. **Post-Process** (current 3-column layout)
2. **Project** (resolution, vsync, fps, type — currently shown)
3. **Audio** (master volume, waveform viewer)
4. **Physics** (gravity, enabled, substeps)
5. **Shadows** (resolution, cascade splits, toggles)
6. **Rendering** (HDR, pipeline mode, tonemap, exposure)
7. **Editor** (grid, camera speeds, gizmo size, snap defaults, overlay toggles)
8. **Input** (recording, keybindings)

---

## Implementation Notes

- Most fields already exist in `AppState` and only need UI wiring.
- `ProjectSettings` needs new fields for physics/audio/build toggles.
- Settings should be saved to disk (currently only resolution/type/vsync/fps are serialized in `project.json`).
- Consider a global `EditorSettings` file at `~/.config/rustix/editor.json` for preferences that apply across all projects.
