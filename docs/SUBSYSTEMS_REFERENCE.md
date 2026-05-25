# Rustix Engine — Subsystem Interface Reference

This document defines the public API boundaries for each engine crate. Every subsystem communicates through well-defined trait and type interfaces.

---

## 1. Core Subsystem Interface

### 1.1 Engine Facade (engine/)

```rust
// engine/src/lib.rs

pub struct App { /* private */ }

impl App {
    pub fn new() -> Self;
    pub fn add_plugin<P: Plugin>(self, plugin: P) -> Self;
    pub fn insert_resource<R: Resource>(self, resource: R) -> Self;
    pub fn add_system<S: System>(self, system: S, stage: StageLabel) -> Self;
    pub fn run(self) -> !;
}

pub trait Plugin: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn build(&self, app: &mut AppBuilder);

    fn on_load(&self, _world: &mut World) {}
    fn on_unload(&self, _world: &mut World) {}
}

pub trait Resource: Send + Sync + 'static {}

pub trait System: Send + 'static {
    type Stage: StageLabel;
    fn run(&mut self, world: &mut World);
}
```

### 1.2 Schedule System (engine/src/schedule.rs)

```rust
pub enum StageLabel {
    First,
    FixedUpdate,  // 120 Hz, deterministic
    PreUpdate,
    Update,
    PostUpdate,
    PreRender,
    Render,
    Last,
}

pub struct Schedule { /* private */ }

impl Schedule {
    pub fn new() -> Self;
    pub fn add_system(&mut self, system: impl System, stage: StageLabel);
    pub fn run_stage(&mut self, world: &mut World, stage: StageLabel);
    pub fn run_fixed_update(&mut self, world: &mut World);
    pub fn has_stage(&self, stage: StageLabel) -> bool;
}
```

### 1.3 World Extension (crates/core/src/world_ext.rs)

Extensions to `hecs::World` for engine integration:

```rust
pub trait WorldExt {
    /// Run a closure with read access to all components of type T
    fn query<T: Component>(&self) -> QueryResult<'_, T>;

    /// Spawn entity with components from a bundle
    fn spawn_bundle(&mut self, bundle: impl DynamicBundle) -> Entity;

    /// Send an event to all registered event readers
    fn send_event<E: Event>(&mut self, event: E);

    /// Register a singleton resource
    fn insert_resource<R: Resource>(&mut self, resource: R);

    /// Get a singleton resource (panics if not found)
    fn resource<R: Resource>(&self) -> &R;
    fn resource_mut<R: Resource>(&mut self) -> &mut R;
}
```

---

## 2. Platform Subsystem Interface

### 2.1 Window (crates/platform/src/window.rs)

```rust
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: FullscreenMode,
    pub vsync: bool,
    pub priority: WindowBackend,  // Wayland | X11 | Auto
}

pub enum FullscreenMode {
    Windowed,
    BorderlessFullscreen,
    ExclusiveFullscreen,
}

pub enum WindowBackend {
    Auto,
    Wayland,
    X11,
}

impl Window {
    pub fn new(config: &WindowConfig) -> Result<Self, PlatformError>;
    pub fn raw_handle(&self) -> RawWindowHandle;
    pub fn size(&self) -> (u32, u32);
    pub fn set_title(&self, title: &str);
    pub fn request_redraw(&self);

    /// Process all pending window events, returning input state
    pub fn poll_events(&mut self) -> Vec<WindowEvent>;

    /// Returns true if window should close
    pub fn should_close(&self) -> bool;
}
```

### 2.2 Input (crates/platform/src/input.rs)

```rust
pub struct InputManager { /* private */ }

impl InputManager {
    pub fn new() -> Result<Self, PlatformError>;

    /// Poll all input sources. Call once per fixed update tick.
    pub fn poll(&mut self);

    // Keyboard
    pub fn key_down(&self, key: KeyCode) -> bool;
    pub fn key_just_pressed(&self, key: KeyCode) -> bool;
    pub fn key_just_released(&self, key: KeyCode) -> bool;

    // Mouse
    pub fn mouse_position(&self) -> (f32, f32);
    pub fn mouse_delta(&self) -> (f32, f32);
    pub fn mouse_button_down(&self, btn: MouseButton) -> bool;
    pub fn mouse_scroll(&self) -> (f32, f32);

    // Gamepad
    pub fn gamepad_connected(&self, id: GamepadId) -> bool;
    pub fn gamepad_axis(&self, id: GamepadId, axis: GamepadAxis) -> f32;
    pub fn gamepad_button(&self, id: GamepadId, btn: GamepadButton) -> bool;

    // Action mapping
    pub fn action_value(&self, action: &str) -> f32;
    pub fn action_just_pressed(&self, action: &str) -> bool;

    // Raw input events for advanced use
    pub fn raw_events(&self) -> &[InputEvent];
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode { /* all physical keys */ }

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton { Left, Right, Middle, Side(u8) }

#[derive(Clone, Copy)]
pub enum InputEvent {
    KeyPress(KeyCode),
    KeyRelease(KeyCode),
    MouseMove(f32, f32),
    MouseButton(MouseButton, bool),
    MouseScroll(f32, f32),
    GamepadButton(GamepadId, GamepadButton, bool),
    GamepadAxis(GamepadId, GamepadAxis, f32),
}
```

---

## 3. Renderer Subsystem Interface

### 3.1 Renderer Plugin (crates/render/src/lib.rs)

```rust
pub struct RenderPlugin {
    pub config: RenderConfig,
}

impl Plugin for RenderPlugin { /* ... */ }

pub struct RenderConfig {
    pub preferred_gpu: GpuPreference,  // HighPerformance | Integrated
    pub present_mode: PresentMode,      // Mailbox | Fifo | Immediate
    pub enable_validation: bool,        // Vulkan validation layers
    pub shader_cache_path: PathBuf,
    pub pipeline_cache_path: PathBuf,
    pub frame_count: u32,               // 2 or 3 (triple buffering)
}

pub enum PresentMode {
    Mailbox,     // VK_PRESENT_MODE_MAILBOX_KHR (preferred)
    Fifo,        // VK_PRESENT_MODE_FIFO_KHR (vsync)
    Immediate,   // VK_PRESENT_MODE_IMMEDIATE_KHR (no sync)
}
```

### 3.2 GPU Resource Handles (crates/render/src/resources.rs)

```rust
/// Opaque handle to a GPU resource. 8 bytes, copyable.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuBuffer(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuTexture(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSampler(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuPipeline(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindlessIndex(u32);

// Resource creation (called from asset system)
impl Renderer {
    pub fn create_buffer(&self, desc: &BufferDesc) -> GpuBuffer;
    pub fn create_texture(&self, desc: &TextureDesc, data: &[u8]) -> GpuTexture;
    pub fn create_sampler(&self, desc: &SamplerDesc) -> GpuSampler;
    pub fn create_mesh(&self, desc: &MeshDesc) -> GpuMesh;

    pub fn destroy_buffer(&self, buffer: GpuBuffer);
    pub fn destroy_texture(&self, texture: GpuTexture);

    /// Upload data to existing buffer (via staging)
    pub fn upload_buffer(&self, buffer: GpuBuffer, offset: u64, data: &[u8]);
}
```

### 3.3 Frame Graph Interface (crates/render/src/frame_graph/mod.rs)

```rust
pub struct FrameGraphBuilder { /* private */ }

impl FrameGraphBuilder {
    pub fn new() -> Self;

    pub fn add_resource(&mut self, desc: ResourceDesc) -> ResourceId;

    pub fn add_pass(
        &mut self,
        name: &str,
        color_attachments: &[AttachmentDesc],
        depth_attachment: Option<AttachmentDesc>,
        execute: Box<dyn Fn(&mut CommandBuffer, &FrameContext)>,
    ) -> PassId;

    pub fn add_compute_pass(
        &mut self,
        name: &str,
        execute: Box<dyn Fn(&mut CommandBuffer, &FrameContext)>,
    ) -> PassId;

    pub fn add_raytrace_pass(
        &mut self,
        name: &str,
        execute: Box<dyn Fn(&mut CommandBuffer, &FrameContext)>,
    ) -> PassId;

    pub fn build(&mut self) -> FrameGraph;
}

pub struct FrameGraph { /* private */ }

impl FrameGraph {
    /// Compile graph into command buffers for this frame
    pub fn execute(&mut self, device: &GpuDevice, ctx: &FrameContext);
}
```

### 3.4 Shader Interface (crates/render/src/shader.rs)

```rust
pub struct ShaderModule { /* private */ }

impl ShaderModule {
    pub fn from_spirv(device: &GpuDevice, code: &[u32], stage: ShaderStage) -> Result<Self, ShaderError>;
    pub fn from_glsl(device: &GpuDevice, source: &str, stage: ShaderStage) -> Result<Self, ShaderError>;
}

pub struct ShaderLibrary { /* private */ }

impl ShaderLibrary {
    pub fn register(&mut self, name: &str, module: ShaderModule);
    pub fn get(&self, name: &str) -> Option<&ShaderModule>;
    pub fn hot_reload(&mut self, name: &str) -> Result<(), ShaderError>;
}
```

### 3.5 Material Interface (crates/render/src/material.rs)

```rust
pub struct Material {
    pub shader: String,
    pub textures: Vec<(String, GpuTexture)>,
    pub params: MaterialParams,
}

pub struct MaterialParams {
    pub base_color: Vec4,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: Vec3,
    pub ambient_occlusion: f32,
    pub alpha_cutoff: Option<f32>,
    // Additional PBR parameters
}

impl Material {
    pub fn pbr(base_color: Vec4, metallic: f32, roughness: f32) -> Self;
}
```

---

## 4. Asset Subsystem Interface

### 4.1 Asset System (crates/asset/src/lib.rs)

```rust
pub struct AssetPlugin;

impl Plugin for AssetPlugin { /* ... */ }

pub enum AssetType {
    Mesh,
    Texture,
    Material,
    Shader,
    AudioClip,
    AnimationClip,
    Skeleton,
    Prefab,
    WorldRegion,
    Font,
}

pub struct AssetConfig {
    pub asset_root: PathBuf,
    pub cache_path: PathBuf,
    pub hot_reload: bool,
    pub async_loading: bool,
    pub streaming_enabled: bool,
}
```

### 4.2 Handle System (crates/asset/src/handle.rs)

```rust
pub struct Handle<T: Asset> {
    id: u64,
    marker: PhantomData<T>,
}

impl<T: Asset> Handle<T> {
    pub fn id(&self) -> u64;
    pub fn is_valid(&self) -> bool;
}

impl<T: Asset> Clone, Copy, PartialEq, Eq, Hash for Handle<T> { /* 8 bytes total */ }

pub trait Asset: Send + Sync + 'static {
    type Loader: AssetLoader<Self>;
    fn asset_type() -> AssetType;
}

pub trait AssetLoader<T: Asset>: Send + Sync + 'static {
    fn load(&self, path: &Path, ctx: &LoadContext) -> Result<T, AssetError>;
    fn extensions() -> &'static [&'static str];
}

pub struct LoadContext<'a> {
    pub registry: &'a AssetRegistry,
    pub loaders: &'a LoaderRegistry,
    pub importer: &'a ImporterRegistry,
}
```

### 4.3 Asset Registry (crates/asset/src/registry.rs)

```rust
pub struct AssetRegistry { /* private */ }

impl AssetRegistry {
    pub fn register_loader<T: Asset>(&mut self, loader: T::Loader);
    pub fn load_async<T: Asset>(&self, path: &Path) -> LoadFuture<T>;
    pub fn load_blocking<T: Asset>(&self, path: &Path) -> Result<Handle<T>, AssetError>;
    pub fn get<T: Asset>(&self, handle: Handle<T>) -> Option<Access<T>>;
    pub fn get_by_path<T: Asset>(&self, path: &Path) -> Option<Handle<T>>;
    pub fn hot_reload(&self, path: &Path) -> Result<(), AssetError>;
    pub fn force_gc(&self);  // Garbage collect unreferenced assets
}

pub enum Access<'a, T: Asset> {
    Read( RwLockReadGuard<'a, T>),
    Write(RwLockWriteGuard<'a, T>),
}
```

### 4.4 Streaming (crates/asset/src/stream.rs)

```rust
pub struct StreamEngine { /* private */ }

impl StreamEngine {
    pub fn request_load<T: Asset>(&self, path: &Path, priority: Priority);
    pub fn request_unload<T: Asset>(&self, handle: Handle<T>);
    pub fn update(&self, camera_position: Vec3, view_distance: f32);

    pub fn set_priority(&self, handle: &Handle<impl Asset>, priority: Priority);
    pub fn is_loaded(&self, handle: &Handle<impl Asset>) -> bool;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Priority {
    Critical,   // Must load NOW (player interaction)
    High,       // Player visible
    Medium,     // Near view distance
    Low,        // Background / far
    Stream,     // Prefetch (not yet needed)
}

pub enum StreamingStrategy {
    Synchronous,    // Block until loaded (small assets)
    AsyncDecode,    // Async IO, decode on workers
    DirectGPU,      // Stream directly to GPU (textures)
}
```

### 4.5 Hot-Reload (crates/asset/src/hot_reload.rs)

```rust
pub struct HotReloadConfig {
    pub watch_dirs: Vec<PathBuf>,
    pub debounce_ms: u64,
}

pub struct HotReloadWatcher { /* private */ }

impl HotReloadWatcher {
    pub fn new(config: &HotReloadConfig) -> Result<Self, AssetError>;

    /// Returns list of changed files since last poll
    pub fn poll_changes(&mut self) -> Vec<PathBuf>;
}
```

### 4.6 Importer Pipeline (crates/asset/src/importer/mod.rs)

```rust
pub trait Importer: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn supported_extensions(&self) -> &[&str];
    fn import(&self, source: &Path, dest: &Path, ctx: &ImporterContext) -> Result<(), AssetError>;
}

pub struct ImporterRegistry(Vec<Box<dyn Importer>>);

impl ImporterRegistry {
    pub fn register(&mut self, importer: Box<dyn Importer>);
    pub fn get(&self, extension: &str) -> Option<&dyn Importer>;
}
```

---

## 5. Physics Subsystem Interface

### 5.1 Physics Plugin (crates/physics/src/lib.rs)

```rust
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin { /* ... */ }

pub struct PhysicsConfig {
    pub gravity: Vec3,
    pub substeps: u32,
    pub max_velocity: f32,
    pub ccd_enabled: bool,
    pub sleep_linear_threshold: f32,
    pub sleep_angular_threshold: f32,
}
```

### 5.2 Physics Components (crates/physics/src/components.rs)

```rust
// ECS Components added by PhysicsPlugin:

#[derive(Component)]
pub struct RigidBody {
    pub kind: RigidBodyKind,        // Dynamic, Kinematic, Static
    pub mass: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub enabled: bool,
}

#[derive(Component)]
pub struct Collider {
    pub shape: ColliderShape,
    pub material: PhysicsMaterialHandle,
    pub is_trigger: bool,
    pub density: Option<f32>,
}

pub enum ColliderShape {
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
    Capsule { radius: f32, half_height: f32 },
    Cylinder { radius: f32, half_height: f32 },
    Cone { radius: f32, half_height: f32 },
    ConvexMesh(Handle<Mesh>),
    TriMesh(Handle<Mesh>),
    Heightfield { heights: Vec<f32>, size: Vec2 },
}

#[derive(Component)]
pub struct Velocity {
    pub linvel: Vec3,
    pub angvel: Vec3,
}

#[derive(Component)]
pub struct ExternalForce {
    pub force: Vec3,
    pub torque: Vec3,
}
```

### 5.3 Physics Queries (crates/physics/src/query.rs)

```rust
impl PhysicsWorld {
    pub fn ray_cast(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RayHit>;
    pub fn ray_cast_all(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Vec<RayHit>;

    pub fn shape_cast(
        &self,
        shape: &ColliderShape,
        from: Vec3,
        to: Vec3,
    ) -> Option<ShapeCastHit>;

    pub fn overlap_test(
        &self,
        shape: &ColliderShape,
        position: Vec3,
    ) -> Vec<Entity>;

    pub fn point_query(&self, point: Vec3) -> Vec<Entity>;
}

pub struct RayHit {
    pub entity: Entity,
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}
```

---

## 6. Audio Subsystem Interface

### 6.1 Audio Engine (crates/audio/src/lib.rs)

```rust
pub struct AudioEngine {
    // rodio::OutputStream + handle (when audio-playback feature enabled)
    master_volume: f32,
    playback_available: bool,
}

impl AudioEngine {
    pub fn new() -> Result<Self, AudioError>;  // Always succeeds, falls back gracefully
    pub fn play_sound(&self, path: &Path, volume: f32, looping: bool) -> Result<SoundInstance, AudioError>;
    pub fn play_sound_file(&self, path: &Path) -> Result<SoundInstance, AudioError>;
    pub fn update(&mut self);
    pub fn set_master_volume(&mut self, volume: f32);
    pub fn master_volume(&self) -> f32;
    pub fn is_playback_available(&self) -> bool;
}
```

### 6.2 Sound Instance

```rust
pub struct SoundInstance {
    decoded: Vec<f32>,     // Raw interleaved PCM samples (always available)
    sample_rate: u32,
    channels: u16,
}

impl SoundInstance {
    pub fn set_volume(&self, volume: f32);
    pub fn stop(&self);
    pub fn pause(&self);
    pub fn play(&self);
    pub fn is_playing(&self) -> bool;
    pub fn decoded_samples(&self) -> &[f32];  // For waveform visualization
    pub fn sample_rate(&self) -> u32;
    pub fn channels(&self) -> u16;
}
```

### 6.3 Audio Decoding

```rust
fn decode_audio(path: &Path) -> Result<(Vec<f32>, u32, u16), AudioError>;
// Uses symphonia (pure Rust) — supports WAV, MP3, OGG/Vorbis, FLAC, AAC
// Returns (interleaved f32 samples, sample_rate, channel_count)
```

### 6.4 ECS Components (Planned)

```rust
pub struct AudioListener { pub position: Vec3, pub forward: Vec3, pub up: Vec3 }
pub struct AudioSource { pub position: Vec3, pub min_distance: f32, pub max_distance: f32, pub rolloff: f32 }
pub struct SoundPlayer { pub sound_path: PathBuf, pub volume: f32, pub looping: bool, pub spatial_blend: f32 }
```

### 6.5 Error Handling

```rust
pub enum AudioError {
    PlaybackNotEnabled,       // audio-playback feature not active
    Io(std::io::Error),        // file read errors
    Decode(String),             // symphonia decode errors
}
```

### 6.6 Feature Flags

| Feature | Default | Requires |
|---------|---------|----------|
| `audio-playback` | off | rodio (+ libasound2-dev on Linux) |

Without `audio-playback`: full decode + sample access, no hardware output.
With `audio-playback`: hardware playback via rodio/cpal, graceful fallback if no device.

---

## 7. Animation Subsystem Interface

### 7.1 Animation Components (crates/animation/src/components.rs)

```rust
#[derive(Component)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
    pub bone_matrices: Vec<Mat4>,  // Computed per frame
}

pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub inverse_bind: Mat4,
    pub local_transform: Transform,
}

#[derive(Component)]
pub struct Animator {
    pub state_machine: AnimationStateMachine,
    pub current_state: StateId,
    pub blend_weights: Vec<f32>,
    pub root_motion: Vec3,
}

#[derive(Component)]
pub struct SkinnedMesh {
    pub mesh: Handle<Mesh>,
    pub skeleton: Entity,
}
```

### 7.2 Animation Clips (crates/animation/src/clip.rs)

```rust
pub struct AnimationClip {
    pub duration: f32,
    pub tracks: Vec<AnimationTrack>,
    pub events: Vec<AnimationEvent>,
}

pub struct AnimationTrack {
    pub bone_index: usize,
    pub position_keys: Vec<Keyframe<Vec3>>,
    pub rotation_keys: Vec<Keyframe<Quat>>,
    pub scale_keys: Vec<Keyframe<Vec3>>,
}

pub struct Keyframe<T> {
    pub time: f32,
    pub value: T,
    pub interpolation: Interpolation,
}

pub enum Interpolation {
    Step,
    Linear,
    CubicSpline,
}
```

### 7.3 Blend Trees (crates/animation/src/blend_tree.rs)

```rust
pub struct BlendTree {
    pub nodes: Vec<BlendNode>,
    pub parameters: HashMap<String, f32>,
}

pub enum BlendNode {
    Clip(Handle<AnimationClip>),
    Blend1D { children: Vec<BlendChild>, parameter: String },
    Blend2D { grid: Vec<Vec<BlendChild>>, parameters: [String; 2] },
    Additive { base: NodeId, additive: NodeId },
    Mask { child: NodeId, bone_mask: Vec<usize> },
}

pub struct BlendChild {
    pub position: f32,       // For 1D blending
    pub node: NodeId,
}
```

---

## 8. Networking Subsystem Interface

### 8.1 Transport (crates/networking/src/transport.rs)

```rust
pub enum TransportType {
    Client { server_addr: SocketAddr },
    Server { bind_addr: SocketAddr, max_clients: u32 },
}

pub struct NetworkTransport { /* private (quinn) */ }

impl NetworkTransport {
    pub fn new(config: TransportConfig) -> impl Future<Output = Result<Self, NetworkError>>;

    // Send
    pub fn send_reliable(&self, connection: &ConnectionId, data: Bytes);
    pub fn send_unreliable(&self, connection: &ConnectionId, data: Bytes);

    // Receive
    pub fn poll_reliable(&mut self) -> Vec<(ConnectionId, Bytes)>;
    pub fn poll_unreliable(&mut self) -> Vec<(ConnectionId, Bytes)>;

    // Events
    pub fn poll_events(&mut self) -> Vec<TransportEvent>;

    // Stats
    pub fn rtt(&self, connection: &ConnectionId) -> Duration;
    pub fn stats(&self) -> TransportStats;
}
```

### 8.2 Replication (crates/networking/src/replication.rs)

```rust
pub struct ReplicationConfig {
    pub tick_rate: f32,           // Server ticks per second (30-60 Hz)
    pub snapshot_rate: f32,       // Snapshots per second (usually = tick_rate)
    pub max_replication_distance: f32,
    pub compression: CompressionType,  // None, LZ4, Zstd
}

pub enum CompressionType {
    None,
    Lz4,
    Zstd { level: i32 },
}

pub struct ReplicationManager { /* private */ }

impl ReplicationManager {
    pub fn register_replicated<T: Component + Serialize>(&mut self);
    pub fn set_replication_mask(&mut self, entity: Entity, mask: ReplicationMask);

    /// Called on server: build snapshot for client
    pub fn build_snapshot(&self, world: &World, client_view: &ClientView) -> Snapshot;

    /// Called on client: apply snapshot from server
    pub fn apply_snapshot(&mut self, world: &mut World, snapshot: Snapshot);

    /// Client-side prediction
    pub fn predict(&self, world: &mut World, inputs: &[InputSnapshot]);

    /// Client-side reconciliation
    pub fn reconcile(&self, world: &mut World, server_snapshot: &Snapshot);
}
```

### 8.3 RPC System (crates/networking/src/rpc.rs)

```rust
pub trait RpcHandler: Send + Sync + 'static {
    type Request: Serialize + DeserializeOwned;
    type Response: Serialize + DeserializeOwned;
    fn handle(&self, request: Self::Request, context: &RpcContext) -> Self::Response;
}

pub struct RpcRegistry { /* private */ }

impl RpcRegistry {
    pub fn register<H: RpcHandler>(&mut self, name: &str, handler: H);
    pub fn call<T: Serialize + DeserializeOwned>(
        &self,
        conn: &ConnectionId,
        method: &str,
        args: &[u8],
    ) -> impl Future<Output = Result<T, RpcError>>;
}
```

---

## 9. UI Subsystem Interface

### 9.1 Immediate Mode UI (crates/ui/src/lib.rs)

```rust
pub struct UiPlugin;

impl Plugin for UiPlugin { /* ... */ }

pub struct UiContext { /* private */ }

impl UiContext {
    pub fn begin_frame(&mut self, dt: f32, input: &InputManager);
    pub fn end_frame(&mut self) -> DrawData;

    // Widgets (return true if interacted)
    pub fn button(&mut self, id: &str, label: &str, size: Vec2) -> bool;
    pub fn label(&mut self, text: &str, style: &LabelStyle);
    pub fn image(&mut self, id: &str, texture: GpuTexture, size: Vec2) -> bool;
    pub fn panel(&mut self, id: &str, size: Vec2, movable: bool) -> PanelContext;
    pub fn text_input(&mut self, id: &str, buffer: &mut String) -> bool;
    pub fn slider(&mut self, id: &str, value: &mut f32, min: f32, max: f32) -> bool;
    pub fn progress_bar(&mut self, value: f32, max: f32, size: Vec2);
    pub fn scroll_area(&mut self, id: &str, size: Vec2, content: impl FnOnce(&mut UiContext));
}
```

### 9.2 Layout (crates/ui/src/layout.rs)

```rust
pub struct Layout {
    pub anchor: Anchor,
    pub offset: Vec2,
    pub size: SizePolicy,
    pub margin: Margin,
}

pub enum Anchor {
    TopLeft, TopCenter, TopRight,
    CenterLeft, Center, CenterRight,
    BottomLeft, BottomCenter, BottomRight,
    Stretch,
}

pub enum SizePolicy {
    Fixed(f32),
    Fill(f32),             // Fill available space (weighted)
    Content,               // Fit to content
    Percent(f32),          // Percent of parent
    Aspect(f32),           // Maintain aspect ratio based on other axis
}
```

---

## 10. World Streaming Interface

### 10.1 World Subsystem (crates/world/src/lib.rs)

```rust
pub struct WorldPlugin;

impl Plugin for WorldPlugin { /* ... */ }

pub struct WorldConfig {
    pub chunk_size: f32,            // Size of a chunk in world units
    pub region_size: u32,           // Chunks per region side
    pub load_radius: u32,           // Chunks to load around player
    pub keep_radius: u32,           // Chunks to keep loaded (but sleeping)
    pub streaming_enabled: bool,
    pub save_directory: PathBuf,
}
```

### 10.2 Chunk Components (crates/world/src/chunk.rs)

```rust
#[derive(Component)]
pub struct Chunk {
    pub position: IVec2,            // Chunk coordinate
    pub region: IVec2,              // Region coordinate
    pub state: ChunkState,
}

pub enum ChunkState {
    Unloaded,
    Loading,
    Loaded,
    Active,        // Within load radius, fully simulated
    Unloading,
}

#[derive(Component)]
pub struct WorldPosition {
    pub chunk: IVec2,
    pub local: Vec3,                // Position within chunk
}
```

### 10.3 Streaming System (crates/world/src/stream.rs)

```rust
pub struct WorldStreamer { /* private */ }

impl WorldStreamer {
    pub fn update(
        &mut self,
        world: &mut World,
        player_pos: Vec3,
    );

    pub fn save_region(
        &self,
        world: &World,
        region_pos: IVec2,
    ) -> impl Future<Output = Result<(), WorldError>>;

    pub fn load_region(
        &self,
        world: &mut World,
        region_pos: IVec2,
    ) -> impl Future<Output = Result<(), WorldError>>;

    pub fn is_region_loaded(&self, region_pos: IVec2) -> bool;
}
```

---

## 11. Profiling & Diagnostics Interface

### 11.1 Profiling Macros (crates/core/src/profiling.rs)

```rust
// Instrument a scope for Tracy profiling
#[macro_export]
macro_rules! profile_scope {
    ($name:literal $(, $color:expr)?) => { ... };
}

// Instrument a system (adds system name to trace)
#[macro_export]
macro_rules! profile_system {
    () => { ... };
}

// GPU zone marker
#[macro_export]
macro_rules! profile_gpu {
    ($name:literal) => { ... };
}

// Frame marker (called once per frame)
#[macro_export]
macro_rules! profile_frame {
    () => { ... };
}
```

### 11.2 Metrics (crates/core/src/metrics.rs)

```rust
pub struct MetricsCollector { /* private */ }

impl MetricsCollector {
    pub fn new() -> Self;
    pub fn record_counter(&mut self, name: &str, value: u64);
    pub fn record_histogram(&mut self, name: &str, value: f64);
    pub fn record_gauge(&mut self, name: &str, value: f64);

    pub fn snapshot(&self) -> MetricsSnapshot;
}

pub struct MetricsSnapshot {
    pub frame_time: f64,
    pub cpu_time: f64,
    pub gpu_time: f64,
    pub draw_calls: u32,
    pub triangles: u64,
    pub entities: u32,
    pub chunks_loaded: u32,
    pub memory_used: u64,
    pub vram_used: u64,
    pub network_rtt: f64,
    pub network_bandwidth: f64,
}

// Built-in diagnostics overlay (uses UI system)
pub fn diagnostics_overlay(ui: &mut UiContext, metrics: &MetricsSnapshot);
```

---

## Error Handling Convention

Every crate defines its own error type implementing `std::error::Error`:

```rust
#[derive(Debug)]
pub enum RenderError {
    DeviceLost,
    OutOfMemory,
    SurfaceLost,
    InvalidShader(String),
    PipelineCompile(String),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for RenderError { ... }
impl std::error::Error for RenderError { ... }
impl From<ash::vk::Result> for RenderError { ... }
```

Crate errors are re-exported through the public API and converted at crate boundaries using `?` and `Into`.

---

## FFI Boundaries

The following crates interact with system FFI:

| Crate | FFI Target | Crate | Mechanism |
|-------|------------|-------|-----------|
| `platform` | evdev | libc | `evdev-rs` crate |
| `platform` | xkbcommon | libc | `xkbcommon` crate |
| `platform` | pthread | libc | `libc` crate (thread affinity) |
| `render` | Vulkan | libvulkan.so | `ash` crate |
| `audio` | ALSA/pulse | libc | `cpal` crate |
| `asset` | inotify | libc | `notify` crate |

All FFI is contained within the responsible crate. No other crate touches libc directly.
