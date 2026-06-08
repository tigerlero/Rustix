# Untested Features & Test Gaps

This document tracks modules, crates, and features that lack automated tests or have tests that need updating.

## Legend
- **No tests** — crate/module has zero test coverage (no `_tests.rs` files and no inline `#[cfg(test)]` modules)
- **Missing tests** — source file exists but no corresponding `_tests.rs` file
- **Stub tests** — `_tests.rs` exists but contains only placeholder/trivial tests
- **Needs update** — existing tests may be broken or out of date with current API

---

## Crates with No Tests At All

| Crate | Note |
|-------|------|
| `crates/animation` | Skeleton, IK, state machine, keyframe blending — all untested |
| `crates/editor` | All 14 editor modules (camera, undo, hierarchy, inspector, etc.) untested |
| `crates/networking` | UDP, protocol, serialization, prediction, replication, NAT punch — all untested |
| `crates/physics` | Rapier backend, colliders, rigid bodies — untested |
| `crates/platform` | Windowing, input, gamepad, actions, clipboard — untested |
| `crates/scripting` | Rhai engine, event bus, coroutines, sandbox — untested |
| `crates/terrain` | Noise, heightmap import, LOD chunks, sculpt, erosion, water — untested |
| `crates/ui` | Layout, text rendering, GPU overlay — untested |
| `crates/world` | Scene graph, spatial hash, time/weather, serialization — untested |

---

## `crates/ai` — Missing Tests

All AI modules were implemented without corresponding test files:

- `ai/src/nav.rs` — `NavMesh`, `NavMeshGenerator`, `NavTriangle` — **NOW TESTED** via inline `#[cfg(test)]` module in `ai/src/nav.rs` (triangle contains, pathfinding, navmesh generator from colliders/sources; fixed slope filter to use `ny.abs()`)
- `ai/src/path.rs` — `PathFinder`, A* pathfinding
- `ai/src/steering.rs` — `Agent`, seek, flee, arrive, wander, obstacle avoidance — **NOW TESTED** via `ai/src/steering_tests.rs` (seek/flee directions, arrive slowing, wander angle mutation, obstacle avoidance, separation/alignment/cohesion, combine weights, integrate movement, max_force/max_speed clamping)
- `ai/src/btree.rs` — `BehaviorTree`, `Sequence`, `Selector`, `Parallel`, `Blackboard` — **NOW TESTED** via `ai/src/btree_tests.rs` (blackboard set/get/mut/remove/clear, action, sequence/selector success/failure/running, parallel thresholds, invert, repeat count, condition, behavior tree tick)
- `ai/src/fsm.rs` — `Fsm`, `State` with boxed closures — **NOW TESTED** via `ai/src/fsm_tests.rs` (initial state, tick/update, transition conditions, on_enter/on_exit callbacks, same-state guard, context mutation)
- `ai/src/sensor.rs` — `VisionCone`, `HearingRadius`, `AgentSensor` — **NOW TESTED** via `ai/src/sensor_tests.rs` (vision front/behind/FOV/distance/origin, candidate filtering, hearing radius/combination, agent sensor builder/set_position/set_forward)
- `ai/src/goap.rs` — `GoapPlanner`, `WorldState`, `Action` — **NOW TESTED** via `ai/src/goap_tests.rs` (WorldState get/satisfies/apply/distance/eq, action builder, planner simple sequence, already satisfied, no solution, cheaper path preference, multiple preconditions)
- `ai/src/utility.rs` — `UtilityAi`, `Consideration`, response curves — **NOW TESTED** via `ai/src/utility_tests.rs` (curve evaluate linear/inverse/exponential/step/sigmoid/clamp, consideration scoring, action with missing input, reasoner select/ranked/empty)
- `ai/src/influence.rs` — `InfluenceMap`, stamping, decay, combination — **NOW TESTED** via `ai/src/influence_tests.rs` (new/get/set/add, world/grid conversion, stamp/highest/lowest, decay, clamp, add_map, out-of-bounds)
- `ai/src/debug_draw.rs` — `AiDebugDraw`, debug primitives — **NOW TESTED** via `ai/src/debug_draw_tests.rs` (DebugLine/Point/Label construction, AiDebugDraw add/clear/path/vision_cone/hearing_radius/influence_map/fsm_state)
- `ai/src/lib.rs` — re-exports and integration

> Note: `ai/src/nav.rs` may have basic inline `#[test]` functions for triangle containment; these should be expanded into a proper `_tests.rs` file.

---

## `crates/animation` — No Tests

- `animation/src/lib.rs` — `Keyframe`, `AnimationTrack`, blending — **NOW TESTED** via `animation/src/lib_tests.rs` (AnimationTrack empty/single/interpolation/clamp, RotationTrack, EventTrack simple/looped, AnimationClip sample/events, Animator default, update_animators advance/stop/loop, BlendAnimator new/transition/update, RootMotion)
- `animation/src/skeleton.rs` — `Bone`, `Skeleton`
- `animation/src/state_machine.rs` — `AnimationStateMachine`, `Transition`
- `animation/src/ik.rs` — `CcdIkSolver`, `IkJoint`

---

## `crates/asset` — Missing Tests

The asset crate has zero `_tests.rs` files despite having many complex modules:

- `asset/src/handle.rs` — `Handle`, `AssetTypeId`, refcounting — **NOW TESTED** via `asset/src/handle_tests.rs` (AssetTypeId deterministic/different names, Handle new/copy/erase/typed roundtrip, UntypedHandle, debug format)
- `asset/src/server.rs` — `AssetServer`, async loading pipeline
- `asset/src/importer.rs` — import pipeline, format detection
- `asset/src/loader.rs` — `AssetLoader`, dependency resolution
- `asset/src/streaming.rs` — streaming load/unload
- `asset/src/cache.rs` — Disk cache write/read/is_cached/invalidate/entry_count/total_size/clear — **NOW TESTED** via `asset/src/cache_tests.rs` (new creates dir, write/read roundtrip, is_cached true/false, invalidate, entry count, total size, clear)
- `asset/src/vfs.rs` — virtual file system — **NOW TESTED** via `asset/src/vfs_tests.rs` (directory mount read/exists/list/resolve, archive mount read/exists/list, mount shadowing, unmount, read_with_path)
- `asset/src/hot_reload.rs` — file watcher integration
- `asset/src/dependency_graph.rs` — asset dependency resolution
- `asset/src/cook.rs` — build cooking pipeline
- `asset/src/mesh.rs` — `MeshAsset`, vertex formats
- `asset/src/texture.rs` — `TextureAsset`, mipmaps
- `asset/src/material.rs` — `MaterialAsset`
- `asset/src/shader.rs` — `ShaderAsset`, SPIR-V
- `asset/src/prefab.rs` — `Prefab`, entity hierarchy serialization
- `asset/src/animation.rs` — `AnimationAsset`
- `asset/src/audio.rs` — `AudioAsset`
- `asset/src/physics.rs` — collider asset definitions
- `asset/src/skeleton.rs` — skeleton asset
- `asset/src/font.rs` — font atlas asset
- `asset/src/region.rs` — world region asset
- `asset/src/mmap.rs` — memory-mapped file I/O
- `asset/src/decoder_pool.rs` — threaded decode worker pool
- `asset/src/texture_compress.rs` — BC7/ASTC compression
- `asset/src/mesh_opt.rs` — mesh optimization
- `asset/src/lib.rs` — re-exports

---

## `crates/audio` — Partial Tests

Existing test files (may need expansion):
- `audio/src/effects_tests.rs` — audio effects
- `audio/src/spatial_tests.rs` — spatial audio
- `audio/tests/spatial_tests.rs` — integration tests

Missing tests:
- `audio/src/decoder.rs` — format decoding (MP3, OGG, WAV)
- `audio/src/engine.rs` — playback engine, voice management
- `audio/src/stream.rs` — streaming buffer management
- `audio/src/types.rs` — audio buffer types
- `audio/src/waveform.rs` — waveform generation
- `audio/src/lib.rs` — re-exports

---

## `crates/core` — Missing Tests

Existing test files:
- `core/src/component_registry_tests.rs`
- `core/src/diagnostics_tests.rs`
- `core/src/soa_storage_tests.rs`

Missing tests:
- `core/src/change_tracker.rs` — dirty-flag tracking
- `core/src/command_buffer.rs` — ECS command buffers
- `core/src/component_groups.rs` — component group prewarming
- `core/src/components.rs` — built-in components — **NOW TESTED** via `core/src/components_tests.rs` (Transform default/from_translation/from_translation_rotation_scale/matrix, Parent default, LocalToWorld default, ScriptComponent default)
- `core/src/config.rs` — `RenderConfig`, `EngineConfig`
- `core/src/dev_toggles.rs` — runtime feature toggles
- `core/src/ecs.rs` — ECS wrappers
- `core/src/gpu_staging.rs` — GPU staging ring
- `core/src/job.rs` — job system, thread pool
- `core/src/math.rs` — math utilities (may be tested via glam) — **NOW TESTED** via `core/src/math_tests.rs` (Aabb, Sphere, Plane, Frustum, Ray, Color, lerp, smoothstep)
- `core/src/memory.rs` — memory tracking
- `core/src/memory_tracker.rs` — allocation tracking
- `core/src/system_monitor.rs` — performance counters
- `core/src/task_graph.rs` — task graph scheduling
- `core/src/task_priority.rs` — task priority queue
- `core/src/thread_local_arena.rs` — arena allocator
- `core/src/thread_priority.rs` — platform thread priority
- `core/src/transform_hierarchy.rs` — parent-child transforms
- `core/src/world_registry.rs` — world registration

---

## `crates/editor` — No Tests

All 14 modules untested:

- `editor/src/lib.rs` — `GizmoMode`, `SelectionState`, gizmo helpers
- `editor/src/plugin.rs` — `PluginRegistry`, `EditorPanel`, `EditorTool`
- `editor/src/camera.rs` — `EditorCamera`, orbit/fly/fps controls
- `editor/src/undo.rs` — `UndoStack`, `Command` trait
- `editor/src/hierarchy.rs` — `HierarchyNode`, `flatten_hierarchy`
- `editor/src/inspector.rs` — `InspectorState`, `ComponentDesc`
- `editor/src/asset_browser.rs` — `AssetBrowserState`
- `editor/src/console.rs` — `ConsoleState`, log filtering
- `editor/src/profiler.rs` — `ProfilerState`, frame stats
- `editor/src/material_editor.rs` — `MaterialEditorState`
- `editor/src/lighting_editor.rs` — `EditableLight`, `IblProbe`
- `editor/src/animation_editor.rs` — `TimelineState`, `Keyframe`
- `editor/src/terrain_editor.rs` — `TerrainEditorState`
- `editor/src/play_mode.rs` — `PlayModeController`
- `editor/src/build_pipeline.rs` — `BuildPipeline`, `BuildConfig`

---

## `crates/networking` — Partial Tests

- `networking/src/serialize.rs` — **NOW TESTED** via `networking/src/protocol_tests.rs` (serialize/deserialize roundtrip, serialize_unchecked/deserialize_unchecked)
- `networking/src/protocol.rs` — **NOW TESTED** via `networking/src/protocol_tests.rs` (PacketType to_u8/from_u8, ProtocolPacket encode/decode, VirtualConnection new/handshake/connect/disconnect/send_reliable/send_unreliable/receive_reliable/receive_unreliable/duplicate_ignore/heartbeat/timeout/ack/pending_retransmits)
- `networking/src/lib.rs` — **NOW TESTED** via `networking/src/protocol_tests.rs` (ClientId default, InMemoryTransport new/connect/disconnect/send/broadcast/receive FIFO)

Remaining untested:
- `networking/src/udp.rs` — `AsyncUdpSocket`
- `networking/src/prediction.rs` — client-side prediction
- `networking/src/interpolation.rs` — entity interpolation
- `networking/src/lag_compensation.rs` — lag-compensated raycast
- `networking/src/replication.rs` — component replication
- `networking/src/authority.rs` — server/client authority
- `networking/src/bandwidth.rs` — delta compression, interest management
- `networking/src/nat.rs` — NAT punch-through, relay, rendezvous
- `networking/src/matchmaking.rs` — lobby API

---

## `crates/physics` — No Tests

- `physics/src/lib.rs` — `Collider`, `ColliderShape`, `RigidBody` — **NOW TESTED** via `physics/src/lib_tests.rs` (defaults for RigidBody/Collider/PhysicsMaterial/CharacterController/Joint/PhysicsWorld, Aabb from_shape/intersects, step_physics gravity/drag/static/kinematic)
- `physics/src/rapier.rs` — Rapier backend integration

---

## `crates/platform` — No Tests

- `platform/src/window.rs` — `WindowConfig`, fullscreen, cursor modes
- `platform/src/input.rs` — keyboard/mouse input abstraction
- `platform/src/gamepad.rs` — gamepad detection and state
- `platform/src/actions.rs` — action mapping system — **NOW TESTED** via `platform/src/actions_tests.rs` (new/empty, bind/unbind, defaults, update key press/release, mouse button, multiple bindings OR, gamepad button, load/save roundtrip, BindingConfig serialize, ActionState default)
- `platform/src/recorder.rs` — input recording/playback
- `platform/src/clipboard.rs` — clipboard access
- `platform/src/lib.rs` — re-exports, `PlatformError`

---

## `crates/render` — Partial Tests

Existing test files (may need expansion):
- `render/src/graph_tests.rs`
- `render/src/components.rs` — **NOW TESTED** via `render/src/components_tests.rs` (Material/Camera/Light defaults, Visible/CastShadows, Sprite new/empty/clear/fill/fill_rect/checkerboard/set_pixel/get_pixel/draw_line/fill_circle/nine_patch/draw_rect/draw_circle/draw_rect_outline, SpriteRenderer default)
- `render/src/mesh_tests.rs`
- `render/src/pipeline_tests.rs`
- `render/src/renderer_tests.rs`
- `render/src/shader_tests.rs`

Missing tests:
- `render/src/instance.rs` — Vulkan instance creation
- `render/src/device.rs` — physical/logical device
- `render/src/surface.rs` — platform surface creation
- `render/src/swapchain.rs` — swapchain management
- `render/src/shader.rs` — shader compilation
- `render/src/hot_reload.rs` — shader hot reload watcher
- `render/src/texture.rs` — `Framebuffer`, `HdrFramebuffer`, `DepthBuffer`
- `render/src/memory.rs` — GPU memory allocation
- `render/src/components.rs` — render components (Sprite, Camera, Lights)
- `render/src/bindless.rs` — bindless descriptor heap
- `render/src/descriptor_cache.rs` — descriptor layout cache
- `render/src/sampler_cache.rs` — sampler reuse
- `render/src/descriptor_batch.rs` — batched descriptor updates
- `render/src/descriptor_allocator.rs` — pool recycling
- `render/src/spec_constants.rs` — specialization constants
- `render/src/spv_reflect.rs` — SPIR-V reflection
- `render/src/shader_include.rs` — `#include` resolution
- `render/src/shader_archive.rs` — embedded shader archive
- `render/src/gizmo.rs` — gizmo line generation
- `render/src/profiler.rs` — `GpuProfiler` timestamp queries
- `render/src/error.rs` — `RenderError`
- `render/src/lib.rs` — re-exports
- `render/src/secondary_cmd.rs` — secondary command buffers
- `render/src/msaa.rs` — MSAA resolve targets
- `render/src/renderdoc.rs` — RenderDoc capture trigger
- `render/src/debug_label.rs` — debug object labeling
- `render/src/tracy_gpu.rs` — Tracy GPU profiling stubs
- `render/src/wireframe.rs` — wireframe/debug overlay modes

---

## `crates/scripting` — No Tests

- `scripting/src/lib.rs` — `ScriptEngine`, `ScriptApi`, `ScriptInstance`
- `scripting/src/events.rs` — `ScriptEventBus`, `EventCallback` — **NOW TESTED** via `scripting/src/lib_tests.rs` (new/empty, subscribe/emit, multiple subscribers, unsubscribe_script, event isolation)
- `scripting/src/time_api.rs` — `ScriptTime` — **NOW TESTED** via `scripting/src/time_api_tests.rs` (tick accumulation, delta time, elapsed time, frame count)
- `scripting/src/math_api.rs` — `vec3`, `lerp`, `dot`, etc. — **NOW TESTED** via `scripting/src/lib_tests.rs` (vec3, lerp, dot, cross, normalize, distance, quat_from_euler)
- `scripting/src/hot_reload.rs` — `HotReloadWatcher`
- `scripting/src/component_def.rs` — `ComponentRegistry` — **NOW TESTED** via `scripting/src/lib_tests.rs` (new/empty, define/get, remove, field type variants)
- `scripting/src/logging.rs` — script logging bridge
- `scripting/src/sandbox.rs` — `SandboxPolicy`, `Sandbox` — **NOW TESTED** via `scripting/src/sandbox_tests.rs` (default/unrestricted policy, read/write/network enforcement)
- `scripting/src/coroutine.rs` — `CoroutineScheduler`, `CutsceneCoroutine` — **NOW TESTED** via `scripting/src/coroutine_tests.rs` (scheduler spawn/tick/clear, wait seconds/frames, action execution, completion)

---

## `crates/terrain` — No Tests

- `terrain/src/lib.rs` — `Heightmap`, mesh generation — **NOW TESTED** via `terrain/src/lib_tests.rs`
- `terrain/src/noise.rs` — value noise, Perlin, FBM, domain warping — **NOW TESTED** via `terrain/src/noise_tests.rs` (determinism, range bounds, Perlin FBM, domain warp)
- `terrain/src/import.rs` — PNG/RAW/R16 heightmap import
- `terrain/src/chunk.rs` — `TerrainChunk`, `ChunkedTerrain`, LOD
- `terrain/src/splat.rs` — `SplatMap`, `SplatStack`, `TerrainLayer`
- `terrain/src/material.rs` — `TerrainMaterial`, `TerrainMaterialPalette`
- `terrain/src/foliage.rs` — `FoliageInstance`, `scatter_foliage`
- `terrain/src/sculpt.rs` — `SculptBrush`, raise/lower/flatten/smooth — **NOW TESTED** via `terrain/src/sculpt_tests.rs` (raise, lower, flatten, smooth, radius bounds, outside bounds)
- `terrain/src/erosion.rs` — `thermal_erosion`, `hydraulic_erosion`
- `terrain/src/water.rs` — shoreline detection, water body flood-fill — **NOW TESTED** via `terrain/src/water_tests.rs` (shoreline tolerance, flood fill connected cells, water stats depth/shoreline count)

---

## `crates/ui` — No Tests

- `ui/src/lib.rs` — immediate-mode UI, GPU vertex buffers
- `ui/src/text.rs` — font atlas, glyph rendering
- `ui/src/layout.rs` — layout engine

---

## `crates/world` — No Tests

- `world/src/lib.rs` — `ChunkCoord`, `ChunkManager` — **NOW TESTED** via `world/src/lib_tests.rs` (ChunkCoord new/distance/neighbors, ChunkManager new/update load-unload/mark states/world_origin)
- `world/src/scene_graph.rs` — `Parent`, `Children`, `LocalTransform`, `GlobalTransform` — **NOW TESTED** via `world/src/scene_graph_tests.rs` (Local/GlobalTransform defaults/matrix/translation, Children add/remove/dedup, hierarchy depth-first order, propagate transforms root/child/grandchild/no-parent; fixed propagate_transforms bug: reads parent global from computed HashMap instead of stale world state)
- `world/src/spatial.rs` — `SpatialHash` — **NOW TESTED** via `world/src/spatial_tests.rs` (insert/query, multiple entities same cell, different cells, remove, update moves/same cell, sphere query includes/excludes, clear)
- `world/src/time_of_day.rs` — `TimeOfDay`, sun/moon cycle — **NOW TESTED** via `world/src/time_of_day_tests.rs` (hour wrapping, sun/moon direction, ambient/sun colors)
- `world/src/weather.rs` — `WeatherState`, `lerp_weather` — **NOW TESTED** via `world/src/weather_tests.rs` (default clear, rain/snow/wind properties, clear reset, lerp midpoint/clamp/wind interpolation)
- `world/src/serialization.rs` — `WorldSnapshot`, `WorldSerializer` — **NOW TESTED** via `world/src/serialization_tests.rs` (SerializedEntity construction, WorldSnapshot new, serializer snapshot stub, deserializer load stub)
- `world/src/save_load.rs` — `SaveHeader`, `SaveMigrator` — **NOW TESTED** via `world/src/save_load_tests.rs` (SaveHeader new/valid/invalid magic, migrator no migration, newer version fail, single/multi-step migration, missing path)
- `world/src/multi_scene.rs` — `Scene`, `SceneManager` — **NOW TESTED** via `world/src/multi_scene_tests.rs` (Scene defaults, SceneManager new/load/unload/set_active, active_scenes active_scenes_mut)
- `world/src/editor_meta.rs` — `EditorMetadata`, `EditorState` — **NOW TESTED** via `world/src/editor_meta_tests.rs` (EditorMetadata default, EditorState select/deselect/clear/toggle_layer, deduplication, layer visibility)

---

## Priority Order for Adding Tests

1. **High Priority** — Core engine correctness
   - `crates/core` — math, ECS, transform hierarchy, job system
   - `crates/ai` — pathfinding, steering, behavior trees, FSM
   - `crates/terrain` — noise correctness, heightmap operations, chunk LOD
   - `crates/physics` — collider shapes, raycast, body types

2. **Medium Priority** — Feature correctness
   - `crates/render` — framebuffer, texture, pipeline state management
   - `crates/scripting` — Rhai compilation, sandbox policy, coroutine scheduling
   - `crates/world` — scene graph transform propagation, spatial queries
   - `crates/networking` — serialization round-trip, protocol state machine
   - `crates/audio` — decoder output verification, spatial math

3. **Lower Priority** — Editor / tooling
   - `crates/editor` — undo/redo stack, camera math, hierarchy flattening
   - `crates/platform` — input mapping, window state
   - `crates/ui` — layout calculations
   - `crates/asset` — import pipeline (requires test asset files)
   - `crates/animation` — skeleton pose, IK convergence

---

## Files in `apps/runtime` Not Tracked

The `apps/runtime` application layer is not included in this audit. It contains editor UI code, rendering graphs, and scene management that would primarily be covered by integration / E2E tests rather than unit tests.

---

*Last updated: after terrain, world, editor, scripting, and render feature implementation pass.*
