# Rustix Engine Release Roadmap

This document tracks the remaining work needed before the engine can ship a commercial-quality game.

---

## 1. Asset Pipeline & Import Workflow

- [x] **Asset Browser UI**: File-system backed browser with drag-and-drop import, thumbnails, and search.
- [x] **Model Import Pipeline**: FBX/OBJ/GLTF import with automatic material generation, mesh validation, and bone mapping.
- [x] **Texture Import**: Auto-compress to BCn/ASTC, generate mipmaps, support HDRi and normal map swizzle.
- [x] **Audio Import**: WAV/OGG/MP3 ingestion with streaming support for music.
- [x] **Asset Hot-Reload**: Detect changed source assets, re-cook, and update in-game references without restart.
- [x] **Asset Cooking**: Build-time pipeline that strips editor metadata and packs into platform-optimized archives.

---

## 2. Level Editor & Tools

- [x] **Terrain Editor**: Heightmap brush sculpting, texture splat painting, foliage scatter.
- [x] **Prefab System**: Save entity hierarchies as reusable prefabs with override tracking.
- [x] **Gizmos & Snapping**: Translate/Rotate/Scale gizmos with grid snapping, local/world space toggle, and multi-selection support.
- [x] **Undo/Redo Polish**: Full action history with compound actions and selective undo per subsystem.
- [x] **Scene Serialization**: Robust save/load of ECS world state including custom components and script variables.
- [x] **Multi-Scene Workflow**: Load/unload additive scenes for streaming open worlds.
- [x] **Editor Play-Mode**: Enter/exit play test without leaking state or spawning duplicate entities.

---

## 3. Rendering Polish

- [x] **PBR Validation**: Validate BRDF against reference images; fix energy conservation on metallic materials.
- [x] **Shadow Improvements**: PCF/PCSS soft shadows, cascaded shadow map stability for moving objects, point-light shadows.
- [x] **Global Illumination**: Light probes (irradiance volumes) or SDF-based GI fallback for static scenes.
- [ ] **Particle System**: GPU-based particle rendering with emitters, collision, and sorting.
- [ ] **Post-Process Stack**: Film grain, chromatic aberration, vignette, color grading LUTs.
- [ ] **Render Scaling**: Dynamic resolution scaling based on frame time budget.
- [ ] **Debug Render Modes**: Wireframe, overdraw heatmap, light complexity visualization.

---

## 4. Physics & Gameplay Systems

- [ ] **Character Controller**: Robust kinematic or physics-driven controller with stair stepping, slope limits, and push mechanics.
- [ ] **Trigger Volumes**: Enter/exit callbacks for gameplay zones (checkpoint, damage, etc.).
- [ ] **Raycast API**: World/query raycasts with mask filters and hit info (normal, distance, entity).
- [ ] **Joints & Constraints**: Hinge, spring, and fixed joints for physics puzzles.
- [ ] **Destructible/Procedural Colliders**: Runtime mesh collider generation for dynamic objects.
- [ ] **Save/Load Physics State**: Serialize rigid body velocities and sleeping state.

---

## 5. Audio Systems

- [ ] **Spatial Audio**: HRTF or simple pan/attenuation with occlusion/occlusion raycasts.
- [ ] **Audio Mixer**: Bus hierarchy (Master -> SFX -> Music -> Ambience) with per-bus volume and ducking.
- [ ] **Music Streaming**: Stream OGG from disk to keep memory usage low for long soundtracks.
- [ ] **Sound Banks**: Group and preload related sounds; async load with reference counting.
- [ ] **Audio Triggers**: Footstep surface detection (material-based), ambient zone blending.

---

## 6. Scripting & Gameplay Programming

- [ ] **Script Hot-Reload**: Reload scripts at runtime without recompiling the engine.
- [ ] **Script Editor**: Inline editor with syntax highlighting, error diagnostics, and breakpoint integration.
- [ ] **Component API**: Expose ECS queries, entity spawning, and component access to scripts safely.
- [ ] **Event System**: Decoupled gameplay events (OnDeath, OnCollect, OnTrigger) that scripts can subscribe to.
- [ ] **Visual Scripting** (optional): Node-graph alternative to code for designers.

---

## 7. Animation & Characters

- [ ] **Animation Retargeting**: Reuse animation clips across skeletons with different proportions.
- [ ] **Blend Trees**: 1D/2D blend spaces for locomotion (walk -> run -> sprint).
- [ ] **Inverse Kinematics**: Foot locking, hand-to-target IK for interactive objects.
- [ ] **Animation Events**: Frame-accurate events for footstep sounds, hit boxes, and VFX spawning.
- [ ] **Facial Animation**: Blend shape/morph target support.

---

## 8. Networking

- [ ] **Client-Server Model**: Authoritative server with client prediction and reconciliation.
- [ ] **Entity Replication**: Selective component sync with delta compression and interest management.
- [ ] **Latency Compensation**: Rewind-based hit detection for competitive multiplayer.
- [ ] **NAT Punchthrough**: Complete hole-punching for peer-hosted sessions.
- [ ] **Dedicated Server Build**: Headless runtime binary without rendering/UI.
- [ ] **Matchmaking & Lobbies**: Room-based matchmaking with skill-based pairing.

---

## 9. UI & HUD Systems

- [ ] **In-Game UI Framework**: Canvas-based system with anchors, layout groups, and nine-slice sprites.
- [ ] **Localization**: String table system with pluralization and font fallback for non-Latin scripts.
- [ ] **Input Navigation**: Controller/keyboard navigation for menus without mouse.
- [ ] **Screen Transitions**: Cross-fade, wipe, and loading screen management.
- [ ] **Settings Persistence**: Save graphics, audio, and control settings to disk.

---

## 10. Platform & Build Systems

- [ ] **Windows Build**: Statically linked release binary with icon and manifest.
- [ ] **Linux Build**: AppImage or tarball distribution with Steam runtime compatibility.
- [ ] **macOS Build**: Metal backend or MoltenVK wrapper; notarize and sign for distribution.
- [ ] **Console Ports** (future): Define abstraction layers for platform-specific I/O and memory.
- [ ] **CI/CD**: GitHub Actions with build matrix, automated tests, and artifact publishing.

---

## 11. Quality Assurance & Profiling

- [ ] **Automated Tests**: Unit tests for math, physics, and ECS; integration tests for rendering pipeline.
- [ ] **Frame Profiler**: GPU/CPU timing markers displayed in-editor (draw call cost, shader time).
- [ ] **Memory Profiler**: Track transient allocations, detect leaks in asset loading.
- [ ] **Crash Reporter**: Mini-dump generation with stack trace and log attachment.
- [ ] **Stress Testing**: Large entity counts, complex scenes, and long-running play sessions.

---

## 12. Documentation & Examples

- [ ] **API Reference**: Auto-generated docs for all public engine APIs.
- [ ] **Tutorials**: Written + video series: "Your First 3D Game", "Multiplayer Setup", "Custom Shaders".
- [ ] **Sample Projects**: FPS, RTS, and RPG starter kits beyond the existing platformer/runner templates.
- [ ] **Engine Architecture Docs**: Frame graph, ECS layout, and threading model for contributors.

---

## 13. Polish & Bug Fixes

- [ ] **Memory Safety Audit**: Fix all `unsafe` blocks in Vulkan/graphics code; validate buffer lifetimes.
- [ ] **Input Latency**: Reduce input-to-photon latency by merging input polling closer to render submission.
- [ ] **Garbage Collection**: Cleanup dead entities and dangling component references in the editor.
- [ ] **Error Handling**: Replace `.unwrap()` in asset loading and file I/O with graceful degradation.
- [ ] **Accessibility**: Subtitle system, color-blind modes, adjustable UI scale.

---

## Priority Tiers

| Tier | Items | Goal |
|------|-------|------|
| **P0 (Blocker)** | Asset cooking, scene serialization, physics save/load, crash reporter, error handling | Ship a stable single-player game |
| **P1 (Major)** | Character controller, spatial audio, script hot-reload, build packaging, particle system | Match competing indie engines |
| **P2 (Enhancement)** | Visual scripting, GI, facial animation, console ports | AAA-adjacent feature set |
| **P3 (Future)** | Dedicated server scaling, matchmaking, cloud saves | Live-service multiplayer |

---

*Last updated: 2026-06-10*
