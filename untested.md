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

- `ai/src/nav.rs` — `NavMesh`, `NavMeshGenerator`, `NavTriangle` — **NOW TESTED** via `ai/src/nav_tests.rs` (triangle contains, pathfinding, query counts entities, navmesh generator from colliders/sources; fixed slope filter to use `ny.abs()`)
- `ai/src/path.rs` — `PathFinder`, A* pathfinding — **NOW TESTED** via `ai/src/path_tests.rs` (grid path simple, grid path blocked, no path)
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

## `crates/animation` — All Modules Tested

- `animation/src/lib.rs` — `Keyframe`, `AnimationTrack`, blending — **NOW TESTED** via `animation/src/lib_tests.rs` (AnimationTrack empty/single/interpolation/clamp, RotationTrack, EventTrack simple/looped, AnimationClip sample/events, Animator default, update_animators advance/stop/loop, BlendAnimator new/transition/update, RootMotion)
- `animation/src/skeleton.rs` — `Bone`, `Skeleton` — **NOW TESTED** via `animation/src/skeleton_tests.rs` (Bone name_str, Skeleton new/count/find_bone_index, compute_world_matrices root/child/scale/rotation, compute_skinning_matrices, retarget_from, retargeted_world_matrices, retargeted_skinning_matrices, empty, deep hierarchy, clone/partial_eq)
- `animation/src/state_machine.rs` — `AnimationStateMachine`, `Transition` — **NOW TESTED** via `animation/src/state_machine_tests.rs` (TransitionCondition Always/TimeElapsed/TimeRemaining/Trigger/ParameterGte/ParameterLt/ParameterBool/And, Transition new/clamp, AnimationState defaults/with_transition, StateMachine new/update/time_advance/Always transition/parameters/triggers/blend_weight/TimeElapsed/Trigger/Parameter/Bool/And/unknown_state)
- `animation/src/ik.rs` — `CcdIkSolver`, `IkJoint` — **NOW TESTED** via `animation/src/ik_tests.rs` (solver default/new, empty chain, single joint, already at target, reaches target, bends toward target, chain reaches sideways, tolerance, multiple iterations, joint debug/clone)

---

## `crates/asset` — Missing Tests

The asset crate has zero `_tests.rs` files despite having many complex modules:

- `asset/src/handle.rs` — `Handle`, `AssetTypeId`, refcounting — **NOW TESTED** via `asset/src/handle_tests.rs` (AssetTypeId deterministic/different names, Handle new/copy/erase/typed roundtrip, UntypedHandle, debug format)
- `asset/src/server.rs` — `AssetServer`, async loading pipeline — **NOW TESTED** via `asset/src/server_tests.rs` (new/default, insert/get/resolve stale, remove/remove_stale, replace, insert_with_path/path_for/get_by_path, is_referenced, drain_unreferenced/drain_unreferenced_all, declare_dependencies, are_dependencies_loaded true/false, dependents_of)
- `asset/src/importer.rs` — import pipeline, format detection — **NOW TESTED** via `asset/src/importer_tests.rs` (ImporterRegistry new/register/find, ReloadRegistry new/empty, export/import RON roundtrip, export/import JSON roundtrip, invalid utf8, bad syntax)
- `asset/src/loader.rs` — `AssetLoader`, dependency resolution — **NOW TESTED** via `asset/src/loader_tests.rs` (new from_current_runtime, load success roundtrip, load missing file returns error)
- `asset/src/streaming.rs` — streaming load/unload — **NOW TESTED** via `asset/src/streaming_tests.rs` (StreamingPriority ordering, StreamingSystem new/default, request_load/unload, dedup already loaded, tick loads/unloads/budget/eviction, cancel, handle_for/resolve, RequestKind variants, StreamedAsset clone)
- `asset/src/cache.rs` — Disk cache write/read/is_cached/invalidate/entry_count/total_size/clear — **NOW TESTED** via `asset/src/cache_tests.rs` (new creates dir, write/read roundtrip, is_cached true/false, invalidate, entry count, total size, clear)
- `asset/src/vfs.rs` — virtual file system — **NOW TESTED** via `asset/src/vfs_tests.rs` (directory mount read/exists/list/resolve, archive mount read/exists/list, mount shadowing, unmount, read_with_path)
- `asset/src/hot_reload.rs` — file watcher integration — **NOW TESTED** via `asset/src/hot_reload_tests.rs` (FileChangeKind variants, FileEvent debug, HotReloader new/default, HotReloadService new/default)
- `asset/src/dependency_graph.rs` — asset dependency resolution — **NOW TESTED** via `asset/src/dependency_graph_tests.rs` (transitive dependents, set dependencies replaces old)
- `asset/src/cook.rs` — build cooking pipeline — **NOW TESTED** via `asset/src/cook_tests.rs` (CookKind from_extension mesh/texture/material/animation/skeleton/generic, cooked_extension, CookJob clone, CookResult clone)
- `asset/src/mesh.rs` — `MeshAsset`, vertex formats — **NOW TESTED** via `asset/src/mesh_tests.rs` (Vertex new/default, MeshAsset new/aabb/empty/no_indices, rxmesh roundtrip/invalid_magic/too_small/empty_roundtrip)
- `asset/src/texture.rs` — `TextureAsset`, mipmaps — **NOW TESTED** via `asset/src/texture_tests.rs` (TextureFormat from_u32/bytes_per_pixel, TextureAsset new/with_mips/wrong_size_panic, rxtex roundtrip/invalid_magic/too_small/unknown_format)
- `asset/src/material.rs` — `MaterialAsset` — **NOW TESTED** via `asset/src/material_tests.rs` (AlphaMode default/to_from_u32, TextureSlot to_from_u32, MaterialAsset default/texture_dependencies, rxmat roundtrip/no_textures/invalid_magic/too_small)
- `asset/src/shader.rs` — `ShaderAsset`, SPIR-V — **NOW TESTED** via `asset/src/shader_tests.rs` (ShaderStage/ShaderLanguage to_from_u32, ShaderAsset new/with_entry_point/has_compiled_spv, rxshader roundtrip/empty_spv/invalid_magic/too_small/unknown_stage)
- `asset/src/prefab.rs` — `Prefab`, entity hierarchy serialization — **NOW TESTED** via `asset/src/prefab_tests.rs` (PrefabVec3 from/to array, PrefabBodyType default, PrefabEntity default fields, PrefabAsset new/count, rxprefab roundtrip/empty/invalid_magic/too_small)
- `asset/src/animation.rs` — `AnimationAsset` — **NOW TESTED** via `asset/src/animation_tests.rs` (KeyframeAsset new, AnimationClipAsset new, AnimationAsset new/count/empty, rxanim roundtrip/empty/invalid_magic/too_small/unsupported_version)
- `asset/src/audio.rs` — `AudioAsset` — **NOW TESTED** via `asset/src/audio_tests.rs` (AudioAsset new/empty/zero_channels, rxsound roundtrip/empty/invalid_magic/too_small/unsupported_version)
- `asset/src/physics.rs` — collider asset definitions — **NOW TESTED** via `asset/src/physics_tests.rs` (PhysicsMaterialAsset default, clone_copy, rxphys roundtrip/default/invalid_magic/too_small/unsupported_version)
- `asset/src/skeleton.rs` — skeleton asset — **NOW TESTED** via `asset/src/skeleton_tests.rs` (BoneAsset new/long_name, SkeletonAsset new/count/find_bone_index/empty, rxskel roundtrip/empty/invalid_magic/too_small/unsupported_version)
- `asset/src/font.rs` — font atlas asset — **NOW TESTED** via `asset/src/font_tests.rs` (FontAsset new, rxfont roundtrip/empty_data/invalid_magic/too_small/unsupported_version)
- `asset/src/region.rs` — world region asset — **NOW TESTED** via `asset/src/region_tests.rs` (RegionMetadata default, RegionAsset new/count, rxregion roundtrip/empty/invalid_magic/too_small)
- `asset/src/mmap.rs` — memory-mapped file I/O — **NOW TESTED** via `asset/src/mmap_tests.rs` (mapped_file_reads_correct_bytes, small_file_uses_heap)
- `asset/src/decoder_pool.rs` — threaded decode worker pool — **NOW TESTED** via `asset/src/decoder_pool_tests.rs` (new thread_count, poll_empty, submit_and_poll, submit_error)
- `asset/src/texture_compress.rs` — BC7/ASTC compression — **NOW TESTED** via `asset/src/texture_compress_tests.rs` (CompressedBlockFormat block_dims for BC7/ASTC 4x4/6x6/8x8, block_size_bytes, is_srgb, compressed_size calculations, CompressedTexture size_bytes)
- `asset/src/mesh_opt.rs` — mesh optimization — **NOW TESTED** via `asset/src/mesh_opt_tests.rs` (optimize_vertex_cache/overdraw/vertex_fetch/full preserve counts, stripify roundtrip, analyze_vertex_cache/overdraw return stats, build_meshlets returns clusters)
- `asset/src/lib.rs` — re-exports
- `asset/src/load_state.rs` — async load state — **NOW TESTED** via `asset/src/load_state_tests.rs` (LoadState clone/is_loaded/is_failed, LoadHandle new/resolve/fail/clone, AsyncLoad new/default/handle)

---

## `crates/audio` — Partial Tests

Existing test files (may need expansion):
- `audio/src/effects_tests.rs` — audio effects
- `audio/src/spatial_tests.rs` — spatial audio
- `audio/tests/spatial_tests.rs` — integration tests

Missing tests:
- `audio/src/decoder.rs` — format decoding (MP3, OGG, WAV) — **NOW TESTED** via `audio/src/types_tests.rs` (decode_from_asset returns samples/rate/channels)
- `audio/src/engine.rs` — playback engine, voice management — **NOW TESTED** via `audio/src/engine_tests.rs` (new/default ok, master_volume default/set, playback_not_available without feature, listener default/set)
- `audio/src/stream.rs` — streaming buffer management — **NOW TESTED** via `audio/src/stream_tests.rs` (test_stream_short_wav open/sample_rate/channels/read/total/ended/elapsed, test_stream_then_seek read/seek/read)
- `audio/src/types.rs` — audio buffer types — **NOW TESTED** via `audio/src/types_tests.rs` (SoundId, SoundPlayer default, SoundInstance decoded_samples, AudioError display)
- `audio/src/waveform.rs` — waveform generation — **NOW TESTED** via `audio/src/waveform_tests.rs` (sine wave bounds, stereo average, empty)
- `audio/src/lib.rs` — re-exports — **NOW TESTED** via `audio/src/types_tests.rs` (spatial constants REFERENCE_DISTANCE/SPEED_OF_SOUND/HEAD_RADIUS/MAX_ITD)

---

## `crates/core` — All Core Modules Tested

Existing test files:
- `core/src/component_registry_tests.rs`
- `core/src/diagnostics_tests.rs`
- `core/src/soa_storage_tests.rs`

Tested modules:
- `core/src/change_tracker.rs` — dirty-flag tracking — **NOW TESTED** via `core/src/change_tracker_tests.rs` (new/empty, flag/check, flag_erased, changed_entities, manual filter with is_changed, clear, clear_type, clear_type_erased, multiple entities, duplicate flag idempotent, unknown type returns none)
- `core/src/command_buffer.rs` — ECS command buffers — **NOW TESTED** via `core/src/command_buffer_tests.rs` (new/empty, apply clears, spawn empty, spawn with bundle, despawn, insert bundle, insert one, remove by type id, remove by name, add default by name, multiple commands in order, clear discards, unknown component errors)
- `core/src/component_groups.rs` — component group prewarming — **NOW TESTED** via `core/src/component_groups_tests.rs` (new empty, register/get, unknown returns none, names, with_erased, spawn_group, spawn_group unknown component errors, prewarm empty/unknown)
- `core/src/components.rs` — built-in components — **NOW TESTED** via `core/src/components_tests.rs` (Transform default/from_translation/from_translation_rotation_scale/matrix, Parent default, LocalToWorld default, ScriptComponent default)
- `core/src/config.rs` — `RenderConfig`, `EngineConfig` — **NOW TESTED** via `core/src/config_tests.rs` (watcher first update loads, no change returns false, detects file change, missing file returns false, request_refresh forces check)
- `core/src/dev_toggles.rs` — runtime feature toggles — **NOW TESTED** via `core/src/dev_toggles_tests.rs` (new defaults, toggle flips, set explicit, update_toggles with keys, all three, custom bindings, no press noop, ToggleKeyboardState adapter)
- `core/src/ecs.rs` — ECS wrappers — **NOW TESTED** via `core/src/ecs_tests.rs` (StageLabel order/monotonic, SystemIdGenerator, BoxedSystem runs, Schedule new/add_system/add_multiple_stages_sorted/run_stage/run_all/multiple_systems_same_stage)
- `core/src/gpu_staging.rs` — GPU staging ring — **NOW TESTED** via `core/src/gpu_staging_tests.rs` (new empty, allocate linear, allocate aligned, allocate wrap-around, full returns none, release reclaims space, release non-contiguous, wait_idle with reset, multi-thread allocators)
- `core/src/job.rs` — job system, thread pool — **NOW TESTED** via `core/src/job_tests.rs` (rebuild changes thread count)
- `core/src/math.rs` — math utilities — **NOW TESTED** via `core/src/math_tests.rs` (Aabb, Sphere, Plane, Frustum, Ray, Color, lerp, smoothstep)
- `core/src/memory.rs` — memory tracking — **NOW TESTED** via `core/src/memory_tests.rs` (FrameAllocator basic, FrameAllocator oom, PoolAllocator alloc/free/reuse, Aligned alignment)
- `core/src/memory_tracker.rs` — allocation tracking — **NOW TESTED** via `core/src/memory_tracker_tests.rs` (alloc/free, double free returns false, unknown free returns false, peak updates, leak report no leaks, leak report detects leak, reset clears everything, multi-thread)
- `core/src/system_monitor.rs` — performance counters — **NOW TESTED** via `core/src/system_monitor_tests.rs` (first call returns zero, recommended at zero/full/half load, respects bounds, min equals max)
- `core/src/task_graph.rs` — task graph scheduling — **NOW TESTED** via `core/src/task_graph_tests.rs` (new empty, add task ids, topo sort linear chain, topo sort diamond, topo sort cycle returns none, has_cycle detects loop, has_cycle no cycle, execute runs all tasks, execute respects dependencies, execute parallel frontier, execute panics on cycle)
- `core/src/task_priority.rs` — task priority queue — **NOW TESTED** via `core/src/task_priority_tests.rs` (creation, submit and wait, install returns value, high runs before medium/low, multiple tasks per level, wait_for_all empties queues, shutdown, thread names, submit_named, install_named, resize grow/shrink/noop)
- `core/src/thread_local_arena.rs` — arena allocator — **NOW TESTED** via `core/src/thread_local_arena_tests.rs` (basic alloc, reset_all, alloc typed, total_capacity, multi-thread no contention, oom returns none)
- `core/src/thread_priority.rs` — platform thread priority — **NOW TESTED** via `core/src/thread_priority_tests.rs` (SchedulingPolicy variants, ThreadPriority default is Normal, variants, set_current_thread_priority Normal ok, set_current_thread_name ok, serde roundtrip SchedulingPolicy, serde roundtrip ThreadPriority)
- `core/src/transform_hierarchy.rs` — parent-child transforms — **NOW TESTED** via `core/src/transform_hierarchy_tests.rs` (root only, child inherits parent, grandchild, scale composition, rotation composition, set_parent ok, self-parent rejected, cycle detected, topo order, missing parent treated as root, update overwrites existing ltw)
- `core/src/world_registry.rs` — world registration — **NOW TESTED** via `core/src/world_registry_tests.rs` (starts empty, create makes active, create inactive, set active, set active unknown panics, destroy removes world, destroy unknown returns false, get/get_mut, spawn active, spawn active none when no active, names, clear, EntityMapping insert/get, overwrite existing, remove, len and clear)

---

## `crates/editor` — No Tests

All 14 modules untested:

- `editor/src/lib.rs` — `GizmoMode`, `SelectionState`, gizmo helpers — **NOW TESTED** via `editor/src/lib_tests.rs` (GizmoMode default/variants, GizmoAxis variants, SelectionState default, point_line_distance on/off/degenerate line, gizmo_screen_size nonzero)
- `editor/src/plugin.rs` — `PluginRegistry`, `EditorPanel`, `EditorTool` — **NOW TESTED** via `editor/src/plugin_tests.rs` (PanelId/ToolId equality, PluginRegistry new/default, register_panel/tool, find_panel/panel_mut/tool, missing returns none)
- `editor/src/camera.rs` — `EditorCamera`, orbit/fly/fps controls — **NOW TESTED** via `editor/src/camera_tests.rs` (CameraMode variants, EditorCamera default/new/mode_builder, orbit_drag/zoom_clamps/pan, fly_move/look, forward/right/up orthogonality, view/projection matrix validity)
- `editor/src/undo.rs` — `UndoStack`, `Command` trait — **NOW TESTED** via `editor/src/undo_tests.rs` (UndoStack new/default, execute_and_undo, redo, multiple_commands, new_command_clears_redo, max_size_eviction, clear, undo_at_start_noop, redo_at_end_noop)
- `editor/src/hierarchy.rs` — `HierarchyNode`, `flatten_hierarchy` — **NOW TESTED** via `editor/src/hierarchy_tests.rs` (HierarchyNode new, flatten_hierarchy empty/single/nested/deep, collapsed_skips_children, ReparentCommand new/no_parent)
- `editor/src/inspector.rs` — `InspectorState`, `ComponentDesc` — **NOW TESTED** via `editor/src/inspector_tests.rs` (FieldValue variants/equality, ComponentDesc new/with_field, InspectorState new/default/set_components/add_component/remove_component)
- `editor/src/asset_browser.rs` — `AssetBrowserState` — **NOW TESTED** via `editor/src/asset_browser_tests.rs` (AssetEntry folder/file name/path, AssetBrowserState new/default, set_entries, filtered_entries empty filter/by name, select)
- `editor/src/console.rs` — `ConsoleState`, log filtering — **NOW TESTED** via `editor/src/console_tests.rs` (LogLevel ordering/default, ConsoleState new/default, log adds entry, max_entries eviction, filtered_entries by level and text, submit_command/empty, history_up/down)
- `editor/src/profiler.rs` — `ProfilerState`, frame stats — **NOW TESTED** via `editor/src/profiler_tests.rs` (ProfilerState new, begin_frame clears, add_sample, end_frame adds total, max_samples eviction, average_fps, frame_time_min_max)
- `editor/src/material_editor.rs` — `MaterialEditorState` — **NOW TESTED** via `editor/src/material_editor_tests.rs` (MaterialProperty variants/clone, MaterialEditorState new/default, add_property, set_property, set_property out of bounds)
- `editor/src/lighting_editor.rs` — `EditableLight`, `IblProbe` — **NOW TESTED** via `editor/src/lighting_editor_tests.rs` (EditableLightType variants, EditableLight new/point/directional defaults, IblProbe fields, LightingEditorState new/add_light/remove_light/remove_out_of_bounds/add_ibl_probe)
- `editor/src/animation_editor.rs` — `TimelineState`, `Keyframe` — **NOW TESTED** via `editor/src/animation_editor_tests.rs` (KeyframeValue variants/equality, InterpolationType variants, AnimationTrack new/add_keyframe sorted/remove_keyframe_at, TimelineState default/new/update/loop/no_loop_stop/paused/seek_clamps/play/pause/stop)
- `editor/src/terrain_editor.rs` — `TerrainEditorState` — **NOW TESTED** via `editor/src/terrain_editor_tests.rs` (TerrainEditMode variants, TerrainEditorState new/default, set_brush_mode/radius/strength)
- `editor/src/play_mode.rs` — `PlayModeController` — **NOW TESTED** via `editor/src/play_mode_tests.rs` (PlayModeState variants/default, PlayModeController new/default, enter_and_exit, pause_and_resume, pause_only_when_playing, resume_only_when_paused)
- `editor/src/build_pipeline.rs` — `BuildPipeline`, `BuildConfig` — **NOW TESTED** via `editor/src/build_pipeline_tests.rs` (BuildTarget/BuildProfile variants, BuildConfig default/clone, BuildPipeline new/default/start/set_step/clamps/log/error/finish)

---

## `crates/networking` — Partial Tests

- `networking/src/serialize.rs` — **NOW TESTED** via `networking/src/protocol_tests.rs` (serialize/deserialize roundtrip, serialize_unchecked/deserialize_unchecked)
- `networking/src/protocol.rs` — **NOW TESTED** via `networking/src/protocol_tests.rs` (PacketType to_u8/from_u8, ProtocolPacket encode/decode, VirtualConnection new/handshake/connect/disconnect/send_reliable/send_unreliable/receive_reliable/receive_unreliable/duplicate_ignore/heartbeat/timeout/ack/pending_retransmits)
- `networking/src/lib.rs` — **NOW TESTED** via `networking/src/protocol_tests.rs` (ClientId default, InMemoryTransport new/connect/disconnect/send/broadcast/receive FIFO)

Remaining untested:
- `networking/src/udp.rs` — `AsyncUdpSocket` — **NOW TESTED** via `networking/src/udp_tests.rs` (bind, send_recv roundtrip, arc_clone, pipeline creation, receiver/sender pipeline)
- `networking/src/prediction.rs` — client-side prediction — **NOW TESTED** via `networking/src/prediction_tests.rs` (ClientPrediction new/push_input/acknowledge/inputs_to_replay, ServerReconciliation new/receive_input/take_inputs_up_to)
- `networking/src/interpolation.rs` — entity interpolation — **NOW TESTED** via `networking/src/prediction_tests.rs` (SnapshotBuffer new/push/max_size/eviction, ignore out-of-order, interpolate, InterpPosition interpolate, InterpEntityState interpolate)
- `networking/src/lag_compensation.rs` — lag-compensated raycast — **NOW TESTED** via `networking/src/prediction_tests.rs` (LagCompensationBuffer new/push/evict, rewind_to_tick, rewind_to_time, rewind_and_interpolate, clear, buffer_age, latency_from_rtt, lag_compensated_raycast hit/miss/zero_direction)
- `networking/src/replication.rs` — component replication — **NOW TESTED** via `networking/src/replication_tests.rs` (ReplicationTracker new/clear/is_empty/spawn/despawn/update/remove/into_messages, NetworkEntityMap insert/remove_by_network/remove_by_local/get_local/get_network/clear, batch_messages single/multiple)
- `networking/src/authority.rs` — server/client authority — **NOW TESTED** via `networking/src/replication_tests.rs` (AuthorityManager register/unregister/transfer/can_client_update/is_server_authoritative/is_interpolated/predicted_by/server_entities, ClientAuthorityManager new/set_authority/apply_transfer/remove/is_local_predicted/is_interpolated/is_server_authoritative/local_predicted_entities/interpolated_entities)
- `networking/src/bandwidth.rs` — delta compression, interest management — **NOW TESTED** via `networking/src/replication_tests.rs` (DeltaCompressor new/is_changed/record_sent/filter_updates/record_batch/unregister_client/unregister_entity/clear, InterestCriteria default, InterestManager new/register/unregister/update_interest_set/is_interested/filter_messages/interest_set/client_count/clear, BandwidthOptimizer new/register/unregister/optimize_for_client)
- `networking/src/nat.rs` — NAT punch-through, relay, rendezvous — **NOW TESTED** via `networking/src/nat_tests.rs` (RendezvousMessage serialize/deserialize roundtrip all variants, RelayPacket serialize/deserialize roundtrip, ConnectionMode debug, RelayServer new/register/unregister, RelayClient new/local_client_id/relay_addr, NatPunchThrough new/debug, RendezvousClient new/debug)
- `networking/src/matchmaking.rs` — lobby API — **NOW TESTED** via `networking/src/matchmaking_tests.rs` (Lobby new/player_count/is_full/can_start/join/leave/ready/set_team/start, LobbyManager new/create_lobby/join_lobby/leave_lobby/start_match/set_team/search/remove_lobby/host transfer/empty removal/join started fails, MatchmakingRequest/Response variants)

---

## `crates/physics` — No Tests

- `physics/src/lib.rs` — `Collider`, `ColliderShape`, `RigidBody` — **NOW TESTED** via `physics/src/lib_tests.rs` (defaults for RigidBody/Collider/PhysicsMaterial/CharacterController/Joint/PhysicsWorld, Aabb from_shape/intersects, step_physics gravity/drag/static/kinematic)
- `physics/src/rapier.rs` — Rapier backend integration — **NOW TESTED** via `physics/src/rapier_tests.rs` (new/default, add_entity/remove_entity, transform_of, step, apply force/impulse, set velocity/angular_velocity, wake_up/is_sleeping, raycast miss, add/remove character, joint add/remove, snapshot roundtrip/serialize roundtrip, debug_draw, configure gravity)

---

## `crates/platform` — No Tests

- `platform/src/window.rs` — `WindowConfig`, fullscreen, cursor modes — **NOW TESTED** via `platform/src/window_tests.rs` (FullscreenMode default/variants, CursorMode default/variants, WindowConfig default/clone)
- `platform/src/input.rs` — keyboard/mouse input abstraction — **NOW TESTED** via `platform/src/input_tests.rs` (KeyCode/MouseButton/GamepadButton/TouchPhase equality, GamepadId wraps u32, InputEvent clone, TextInputState/TouchState default, KeyboardState new/press/release/end_tick, MouseState new/press/release/move/scroll/end_tick, GamepadState new/button/axis, InputManager new/push_poll key/mouse/gamepad/touch, capture, touch events)
- `platform/src/gamepad.rs` — gamepad detection and state — **NOW TESTED** via `platform/src/gamepad_tests.rs` (GamepadInput new returns stub, poll empty, connected_count zero)
- `platform/src/actions.rs` — action mapping system — **NOW TESTED** via `platform/src/actions_tests.rs` (new/empty, bind/unbind, defaults, update key press/release, mouse button, multiple bindings OR, gamepad button, load/save roundtrip, BindingConfig serialize, ActionState default)
- `platform/src/recorder.rs` — input recording/playback — **NOW TESTED** via `platform/src/recorder_tests.rs` (RecorderMode variants, InputRecorder new/default/start_stop/no_record_idle, InputRecording serde roundtrip, save/load recording, load missing file)
- `platform/src/clipboard.rs` — clipboard access — **NOW TESTED** via `platform/src/clipboard_tests.rs` (get_text doesn't panic, set_text doesn't panic, clear doesn't panic)
- `platform/src/lib.rs` — re-exports, `PlatformError` — **NOW TESTED** via `platform/src/lib_tests.rs` (PlatformError display/debug for WindowCreation/InputInit/SurfaceCreation)

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
- `render/src/hot_reload.rs` — shader hot reload watcher — **NOW TESTED** via `render/src/hot_reload_tests.rs` (new succeeds when no shader dirs exist, take_events returns empty initially)
- `render/src/texture.rs` — `Framebuffer`, `HdrFramebuffer`, `DepthBuffer`
- `render/src/memory.rs` — GPU memory allocation
- `render/src/components.rs` — render components (Sprite, Camera, Lights)
- `render/src/bindless.rs` — bindless descriptor heap
- `render/src/descriptor_cache.rs` — descriptor layout cache
- `render/src/sampler_cache.rs` — sampler reuse — **NOW TESTED** via `render/src/sampler_cache_tests.rs` (SamplerKey from_info maps all vk::SamplerCreateInfo fields correctly, bool conversions, hash/equality consistency)
- `render/src/descriptor_batch.rs` — batched descriptor updates
- `render/src/descriptor_allocator.rs` — pool recycling
- `render/src/spec_constants.rs` — specialization constants — **NOW TESTED** via `render/src/spec_constants_tests.rs` (new/default, set/get, overwrite, multiple entries, get missing, build)
- `render/src/spv_reflect.rs` — SPIR-V reflection — **NOW TESTED** via `render/src/spv_reflect_tests.rs` (ShaderReflection default, merge adds resources/combines stage flags, bindings_by_set grouped by set index, push_constant_range none/some)
- `render/src/shader_include.rs` — `#include` resolution — **NOW TESTED** via `render/src/shader_include_tests.rs` (no_include_unchanged, detects_cycle returns error)
- `render/src/shader_archive.rs` — embedded shader archive
- `render/src/gizmo.rs` — gizmo line generation — **NOW TESTED** via `render/src/gizmo_tests.rs` (wireframe_box produces 12 lines, generate_audio_gizmo produces non-empty lines, flatten_gizmo_lines correct length)
- `render/src/profiler.rs` — `GpuProfiler` timestamp queries
- `render/src/error.rs` — `RenderError` — **NOW TESTED** via `render/src/error_tests.rs` (Display for InstanceCreation, DeviceCreation, SurfaceCreation, SwapchainCreation, ShaderCompile, PipelineCreation, Debug)
- `render/src/lib.rs` — re-exports
- `render/src/secondary_cmd.rs` — secondary command buffers
- `render/src/msaa.rs` — MSAA resolve targets — **NOW TESTED** via `render/src/msaa_tests.rs` (from_quality maps Low/Medium/High/Ultra, MsaaSamples variants, RenderQuality variants)
- `render/src/renderdoc.rs` — RenderDoc capture trigger — **NOW TESTED** via `render/src/renderdoc_tests.rs` (new/default, set_enabled, trigger_and_consume, trigger_ignored_when_disabled)
- `render/src/debug_label.rs` — debug object labeling — **NOW TESTED** via `render/src/debug_label_tests.rs` (label_object, begin_label, end_label stubs do not panic)
- `render/src/tracy_gpu.rs` — Tracy GPU profiling stubs — **NOW TESTED** via `render/src/tracy_gpu_tests.rs` (TracyGpuZone new, begin_zone/end_zone, collect_timestamps, Debug)
- `render/src/wireframe.rs` — wireframe/debug overlay modes — **NOW TESTED** via `render/src/wireframe_tests.rs` (WireframeMode default/variants, DebugOverlay default/variants)

---

## `crates/scripting` — No Tests

- `scripting/src/lib.rs` — `ScriptEngine`, `ScriptApi`, `ScriptInstance` — **NOW TESTED** via `scripting/src/lib_tests.rs` (ScriptId equality, Script default, ScriptConfig default, ScriptComponent default, ScriptApi new/register/unregister, ScriptLoader from_memory, ScriptRegistry new/register/get_by_path)
- `scripting/src/events.rs` — `ScriptEventBus`, `EventCallback` — **NOW TESTED** via `scripting/src/lib_tests.rs` (new/empty, subscribe/emit, multiple subscribers, unsubscribe_script, event isolation)
- `scripting/src/time_api.rs` — `ScriptTime` — **NOW TESTED** via `scripting/src/time_api_tests.rs` (tick accumulation, delta time, elapsed time, frame count)
- `scripting/src/math_api.rs` — `vec3`, `lerp`, `dot`, etc. — **NOW TESTED** via `scripting/src/lib_tests.rs` (vec3, lerp, dot, cross, normalize, distance, quat_from_euler)
- `scripting/src/hot_reload.rs` — `HotReloadWatcher` — **NOW TESTED** via `scripting/src/hot_reload_tests.rs` (new/default, track/untrack, check empty)
- `scripting/src/component_def.rs` — `ComponentRegistry` — **NOW TESTED** via `scripting/src/lib_tests.rs` (new/empty, define/get, remove, field type variants)
- `scripting/src/logging.rs` — script logging bridge — **NOW TESTED** via `scripting/src/logging_tests.rs` (log_info/warn/error/debug do not panic)
- `scripting/src/sandbox.rs` — `SandboxPolicy`, `Sandbox` — **NOW TESTED** via `scripting/src/sandbox_tests.rs` (default/unrestricted policy, read/write/network enforcement)
- `scripting/src/coroutine.rs` — `CoroutineScheduler`, `CutsceneCoroutine` — **NOW TESTED** via `scripting/src/coroutine_tests.rs` (scheduler spawn/tick/clear, wait seconds/frames, action execution, completion)

---

## `crates/terrain` — No Tests

- `terrain/src/lib.rs` — `Heightmap`, mesh generation — **NOW TESTED** via `terrain/src/lib_tests.rs`
- `terrain/src/noise.rs` — value noise, Perlin, FBM, domain warping — **NOW TESTED** via `terrain/src/noise_tests.rs` (determinism, range bounds, Perlin FBM, domain warp)
- `terrain/src/import.rs` — PNG/RAW/R16 heightmap import — **NOW TESTED** via `terrain/src/chunk_tests.rs` (import_raw success/error, import_r16 success/error)
- `terrain/src/chunk.rs` — `TerrainChunk`, `ChunkedTerrain`, LOD — **NOW TESTED** via `terrain/src/chunk_tests.rs` (from_heightmap resolution/vertices/indices/lod/normals/scaling, ChunkedTerrain new/rebuild/stream, build_chunk_skirt)
- `terrain/src/material.rs` — `TerrainMaterial`, `TerrainMaterialPalette` — **NOW TESTED** via `terrain/src/chunk_tests.rs` (default, builder chaining, clamping, palette get/out-of-bounds fallback)
- `terrain/src/foliage.rs` — `FoliageInstance`, `scatter_foliage` — **NOW TESTED** via `terrain/src/chunk_tests.rs` (to_matrix, layer new/builder, scatter_foliage generates instances)
- `terrain/src/sculpt.rs` — `SculptBrush`, raise/lower/flatten/smooth — **NOW TESTED** via `terrain/src/sculpt_tests.rs` (raise, lower, flatten, smooth, radius bounds, outside bounds)
- `terrain/src/erosion.rs` — `thermal_erosion`, `hydraulic_erosion` — **NOW TESTED** via `terrain/src/erosion_tests.rs` (ThermalErosionParams default, flattens peaks, no change on flat; HydraulicErosionParams default, changes heightmap, reduces peaks)
- `terrain/src/splat.rs` — `SplatMap`, `SplatStack`, `TerrainLayer` — **NOW TESTED** via `terrain/src/erosion_tests.rs` (TerrainLayer new/builder/compute_weight in/out of range, SplatMap new/get/set/normalize, SplatStack new/resize/generate, multiple maps)
- `terrain/src/water.rs` — shoreline detection, water body flood-fill — **NOW TESTED** via `terrain/src/water_tests.rs` (shoreline tolerance, flood fill connected cells, water stats depth/shoreline count)

---

## `crates/ui` — No Tests

- `ui/src/lib.rs` — immediate-mode UI, GPU vertex buffers — **NOW TESTED** via `ui/src/lib_tests.rs` (UIContext new/begin_frame/end_frame/center/cursor/advance/next_id, DrawList new/push/clear/commands, Interaction default, button hover/click/color, slider value update, label fallback rect, text_input type/submit/backspace/delete/escape)
- `ui/src/text.rs` — font atlas, glyph rendering — **NOW TESTED** via `ui/src/lib_tests.rs` (Font::from_asset copies name/data)
- `ui/src/layout.rs` — layout engine — **NOW TESTED** via `ui/src/lib_tests.rs` (LayoutItem new/builder, FlexLayout default/builder/row/column/grow/justify/center/empty, GridLayout default/builder/basic/gap/empty/stretch)

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
