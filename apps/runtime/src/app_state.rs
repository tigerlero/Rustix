use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use ash::vk;
use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;
use rustix_render::Renderer;
use rustix_render::mesh::Mesh;
use rustix_audio::{AudioEngine, SoundInstance};
use rustix_animation::AnimationClip;
use rustix_physics::PhysicsWorld;
use rustix_asset::mmap::MappedFile;
use rustix_scripting::ScriptEngine;

use crate::project::{AppScreen, ConfirmTarget, ProjectType, ProjectInfo};
use crate::scene::{Transform, Name, MeshComponent, Material};
use crate::undo::UndoHistory;
use crate::player::{PlayerManager, spawn_player};
use crate::enemy::{spawn_enemy, spawn_enemy_with_skill, EnemyAI};
use crate::combat::{CombatStats, Skill};
use crate::ui::animation_editor::AnimationEditor;

#[allow(dead_code)]
pub struct AppState {
    pub screen: AppScreen,
    pub recent_projects: Vec<crate::project::ProjectEntry>,
    pub current_project: Option<ProjectInfo>,
    pub project_dir: Option<String>,
    pub ecs_world: EcsWorld,
    pub meshes: HashMap<String, Mesh>,
    pub textures: HashMap<String, rustix_render::GpuTexture>,
    pub animation_clips: HashMap<String, AnimationClip>,
    pub physics_world: PhysicsWorld,
    pub player_manager: PlayerManager,
    pub script_engine: ScriptEngine,
    pub voxel_chunks: Vec<crate::voxel::Chunk>,
    pub tetris_game: crate::tetris::TetrisGame,

    pub scene_pipeline: Option<rustix_render::pipeline::GraphicsPipeline>,
    pub scene_descriptor_pool: Option<vk::DescriptorPool>,
    pub scene_descriptor_set: Option<vk::DescriptorSet>,
    pub scene_uniform_buffer: Option<rustix_render::memory::GpuBuffer>,
    pub scene_depth_buffer: Option<rustix_render::DepthBuffer>,
    pub shadow_pipeline: Option<rustix_render::pipeline::ShadowPipeline>,
    pub shadow_descriptor_pool: Option<vk::DescriptorPool>,
    pub shadow_descriptor_set: Option<vk::DescriptorSet>,
    pub csm_resources: Option<crate::render::CsmResources>,
    pub point_shadow_resources: Option<crate::render::PointShadowResources>,
    pub spot_shadow_resources: Option<crate::render::SpotShadowResources>,
    pub shadow_layout: vk::ImageLayout,
    pub frame_graph_snapshot: Option<rustix_render::graph::FrameGraphSnapshot>,
    pub show_frame_graph_overlay: bool,
    pub fwd_plus_resources: Option<crate::render::ForwardPlusResources>,
    pub gbuffer_resources: Option<crate::render::GBufferResources>,

    pub hdr_framebuffer: Option<rustix_render::HdrFramebuffer>,
    pub hdr_fb_size: (u32, u32),
    pub tonemap_pipeline: Option<rustix_render::pipeline::ToneMapPipeline>,
    pub tonemap_desc_set: Option<vk::DescriptorSet>,

    pub bloom_resources: Option<crate::render::BloomResources>,
    pub bloom_fb_size: (u32, u32),
    pub bloom_extract_pipeline: Option<rustix_render::pipeline::BloomPipeline>,
    pub bloom_down_pipeline: Option<rustix_render::pipeline::BloomPipeline>,
    pub bloom_up_pipeline: Option<rustix_render::pipeline::BloomPipeline>,
    pub bloom_desc_set: Option<vk::DescriptorSet>,
    pub bloom_threshold: f32,
    pub bloom_intensity: f32,

    pub oit_resources: Option<crate::render::OitResources>,
    pub oit_fb_size: (u32, u32),
    pub oit_accumulate_pipeline: Option<rustix_render::pipeline::OitAccumulatePipeline>,
    pub oit_composite_pipeline: Option<rustix_render::pipeline::OitCompositePipeline>,
    pub oit_desc_set: Option<vk::DescriptorSet>,
    pub oit_enabled: bool,

    pub ssao_resources: Option<crate::render::SsaoResources>,
    pub ssao_fb_size: (u32, u32),
    pub ssao_pipeline: Option<rustix_render::pipeline::BloomPipeline>,
    pub ssao_blur_pipeline: Option<rustix_render::pipeline::BloomPipeline>,
    pub ssao_desc_set: Option<vk::DescriptorSet>,
    pub ssao_enabled: bool,
    pub ssao_radius: f32,
    pub ssao_bias: f32,
    pub ssao_power: f32,
    pub ssao_intensity: f32,

    pub taa_resources: Option<crate::render::TaaResources>,
    pub taa_fb_size: (u32, u32),
    pub taa_pipeline: Option<rustix_render::pipeline::TaaPipeline>,
    pub taa_desc_set: Option<vk::DescriptorSet>,
    pub taa_enabled: bool,
    pub taa_blend_factor: f32,
    pub prev_view_proj: Option<rustix_core::math::Mat4>,
    pub taa_jitter_idx: u32,

    pub ssr_resources: Option<crate::render::SsrResources>,
    pub ssr_fb_size: (u32, u32),
    pub ssr_pipeline: Option<rustix_render::pipeline::SsrPipeline>,
    pub ssr_desc_set: Option<vk::DescriptorSet>,
    pub ssr_enabled: bool,
    pub ssr_max_steps: f32,
    pub ssr_stride: f32,
    pub ssr_max_dist: f32,

    pub fog_resources: Option<crate::render::VolumetricFogResources>,
    pub fog_fb_size: (u32, u32),
    pub fog_pipeline: Option<rustix_render::pipeline::VolumetricFogPipeline>,
    pub fog_desc_set: Option<vk::DescriptorSet>,
    pub fog_enabled: bool,
    pub fog_density: f32,
    pub fog_scattering: f32,
    pub fog_height_falloff: f32,
    pub fog_max_dist: f32,
    pub fog_max_steps: f32,
    pub fog_sun_intensity: f32,

    pub skybox_resources: Option<crate::render::SkyboxResources>,
    pub skybox_fb_size: (u32, u32),
    pub skybox_pipeline: Option<rustix_render::pipeline::SkyboxPipeline>,
    pub skybox_desc_set: Option<vk::DescriptorSet>,
    pub skybox_enabled: bool,
    pub skybox_rayleigh: f32,
    pub skybox_mie: f32,
    pub skybox_zenith_shift: f32,
    pub skybox_exposure: f32,

    pub instanced_batcher: Option<crate::render::InstancedMeshBatcher>,
    pub instanced_pipeline: Option<rustix_render::pipeline::InstancedGraphicsPipeline>,
    pub instanced_gbuffer_pipeline: Option<rustix_render::pipeline::InstancedGBufferPipeline>,
    pub instanced_enabled: bool,

    pub gpu_culling_resources: Option<crate::render::GpuCullingResources>,
    pub gpu_culling_enabled: bool,

    pub mesh_shader_pipeline: Option<rustix_render::pipeline::MeshShaderPipeline>,
    pub mesh_shader_enabled: bool,

    pub wireframe_scene_pipeline: Option<rustix_render::pipeline::GraphicsPipeline>,
    pub overdraw_pipeline: Option<rustix_render::pipeline::GraphicsPipeline>,
    pub light_complexity_pipeline: Option<rustix_render::pipeline::GraphicsPipeline>,
    pub debug_render_mode: rustix_render::DebugRenderMode,
    pub debug_render_resources: Option<crate::render::DebugRenderResources>,

    pub particle_system: crate::render::ParticleSystem,

    pub viewport_framebuffers: Vec<[Option<rustix_render::Framebuffer>; 3]>,
    pub viewport_fb_sizes: Vec<(u32, u32)>,

    pub pipeline_2d: Option<rustix_render::pipeline::GraphicsPipeline2D>,
    pub ubo_2d: Option<rustix_render::memory::GpuBuffer>,
    pub desc_set_2d: Option<vk::DescriptorSet>,
    pub quad_buffer_2d: Option<rustix_render::memory::GpuBuffer>,
    pub texture_2d: Option<rustix_render::GpuTexture>,

    pub selected_entities: std::rc::Rc<std::cell::RefCell<Vec<hecs::Entity>>>,
    pub pending_delete: std::rc::Rc<std::cell::RefCell<Vec<hecs::Entity>>>,
    pub dirty: std::rc::Rc<std::cell::Cell<bool>>,
    pub show_confirm: std::rc::Rc<std::cell::Cell<bool>>,
    pub confirm_target: std::rc::Rc<std::cell::Cell<ConfirmTarget>>,
    pub show_settings: std::rc::Rc<std::cell::Cell<bool>>,
    pub renaming: std::rc::Rc<std::cell::RefCell<Option<hecs::Entity>>>,
    pub rename_buffer: std::rc::Rc<std::cell::RefCell<String>>,
    pub undo_history: std::rc::Rc<std::cell::RefCell<UndoHistory>>,
    pub show_new_project_type: std::rc::Rc<std::cell::Cell<bool>>,
    pub new_project_type: std::rc::Rc<std::cell::Cell<ProjectType>>,

    pub sprite_editor: crate::sprite_editor::SpriteEditor,
    pub audio_engine: Option<AudioEngine>,
    pub audio_instance: Option<SoundInstance>,
    pub waveform_viewer: crate::waveform::WaveformViewer,

    pub open_project: std::rc::Rc<std::cell::RefCell<Option<String>>>,
    pub new_project: std::rc::Rc<std::cell::RefCell<Option<String>>>,
    pub pending_mesh_load: std::rc::Rc<std::cell::RefCell<Option<String>>>,
    pub pending_texture_load: std::rc::Rc<std::cell::RefCell<Option<String>>>,
    pub pending_audio_load: std::rc::Rc<std::cell::RefCell<Option<String>>>,
    pub pending_terrain_regen: std::rc::Rc<std::cell::Cell<bool>>,

    pub sounds: std::collections::HashMap<String, crate::audio_import::ImportedAudio>,

    pub asset_watcher: Option<crate::asset_watcher::AssetWatcher>,
    pub hot_reload_enabled: bool,
    pub pak_archive: Option<crate::asset_cook::PakArchive>,

    pub input_recorder: rustix_platform::recorder::InputRecorder,
    pub recording_dir: std::path::PathBuf,
    pub start_time: Instant,
    pub animation_editor: AnimationEditor,
    pub terrain_editor: crate::terrain::TerrainEditor,
    pub prefab_editor: crate::prefab::PrefabEditor,

    pub cli_project_path: Option<String>,
    pub cli_playtest: bool,
    pub endless_runner_game: crate::endless_runner::EndlessRunnerGame,
    pub breakout_game: crate::breakout::BreakoutGame,
    pub platformer_game: crate::platformer::PlatformerGame,
    pub scene_manager: crate::scene::SceneManager,
    pub last_screen: crate::project::AppScreen,
    pub play_mode_snapshot: Option<crate::play_mode::PlayModeSnapshot>,
}

#[allow(dead_code)]
impl AppState {
    pub fn new() -> Self {
        let mut ecs_world = EcsWorld::new();
        for i in 0..3 {
            let e = ecs_world.spawn((
                Transform { position: Vec3::new(i as f32 * 2.0, 0.0, 0.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
                Name(format!("Entity {}", i)),
                MeshComponent("Cube".into()),
                Material { base_color: Vec3::new(0.6 + i as f32 * 0.15, 0.4 + i as f32 * 0.1, 0.5), alpha: 1.0, roughness: 0.3 + i as f32 * 0.2, metallic: 0.0, ao: 1.0, emissive: 0.0 },
            ));
            tracing::info!("created entity {}: {:?}", i, e);
        }
        tracing::info!("startup world has {} named entities", ecs_world.query::<&Name>().iter().count());

        let mut player_manager = PlayerManager::new();
        // Spawn 3 default player capsules spaced apart.
        for i in 0..3 {
            let pos = Vec3::new((i as f32 - 1.0) * 3.0, 2.0, 0.0);
            let entity = spawn_player(&mut ecs_world, pos, i, "Capsule");
            player_manager.players.push(entity);
            tracing::info!("spawned player {} at {:?}", i + 1, pos);
        }

        // Spawn default enemies
        spawn_enemy(
            &mut ecs_world,
            Vec3::new(8.0, 2.0, 5.0),
            "Grunt",
            "Capsule",
            Vec3::new(0.9, 0.2, 0.2),
            EnemyAI { can_follow: true, can_attack: true, follow_range: 20.0, move_speed: 2.5, stop_distance: 1.2 },
            40.0,  // HP
            8.0,   // damage
            1.8,   // attack range
            1.2,   // cooldown
        );
        spawn_enemy(
            &mut ecs_world,
            Vec3::new(-8.0, 2.0, 5.0),
            "Ranger",
            "Capsule",
            Vec3::new(0.2, 0.7, 0.2),
            EnemyAI { can_follow: true, can_attack: true, follow_range: 25.0, move_speed: 3.0, stop_distance: 4.0 },
            30.0,  // HP
            5.0,   // damage
            6.0,   // attack range
            1.5,   // cooldown
        );
        spawn_enemy_with_skill(
            &mut ecs_world,
            Vec3::new(0.0, 2.0, -10.0),
            "Boss",
            "Capsule",
            Vec3::new(0.8, 0.1, 0.8),
            EnemyAI { can_follow: true, can_attack: true, follow_range: 30.0, move_speed: 2.0, stop_distance: 2.0 },
            120.0, // HP
            CombatStats { attack_damage: 12.0, attack_range: 2.5, attack_cooldown: 1.0, current_cooldown: 0.0 },
            Skill {
                name: "Fireball".into(),
                damage: 25.0,
                range: 12.0,
                cooldown: 5.0,
                current_cooldown: 0.0,
            },
        );
        tracing::info!("spawned 3 default enemies");

        Self {
            screen: AppScreen::Startup,
            recent_projects: crate::project::load_recent_projects(),
            current_project: None,
            project_dir: None,
            ecs_world,
            meshes: HashMap::new(),
            textures: HashMap::new(),
            animation_clips: HashMap::new(),
            physics_world: PhysicsWorld::default(),
            player_manager,
            script_engine: ScriptEngine::new(),
            voxel_chunks: Vec::new(),
            tetris_game: crate::tetris::TetrisGame::new(),

            scene_pipeline: None,
            scene_descriptor_pool: None,
            scene_descriptor_set: None,
            scene_uniform_buffer: None,
            scene_depth_buffer: None,
            shadow_pipeline: None,
            shadow_descriptor_pool: None,
            shadow_descriptor_set: None,
            csm_resources: None,
            point_shadow_resources: None,
            spot_shadow_resources: None,
            shadow_layout: vk::ImageLayout::UNDEFINED,
            frame_graph_snapshot: None,
            show_frame_graph_overlay: false,
            fwd_plus_resources: None,
            gbuffer_resources: None,

            hdr_framebuffer: None,
            hdr_fb_size: (0, 0),
            tonemap_pipeline: None,
            tonemap_desc_set: None,

            bloom_resources: None,
            bloom_fb_size: (0, 0),
            bloom_extract_pipeline: None,
            bloom_down_pipeline: None,
            bloom_up_pipeline: None,
            bloom_desc_set: None,
            bloom_threshold: 1.0,
            bloom_intensity: 0.5,

            oit_resources: None,
            oit_fb_size: (0, 0),
            oit_accumulate_pipeline: None,
            oit_composite_pipeline: None,
            oit_desc_set: None,
            oit_enabled: false,

            ssao_resources: None,
            ssao_fb_size: (0, 0),
            ssao_pipeline: None,
            ssao_blur_pipeline: None,
            ssao_desc_set: None,
            ssao_enabled: true,
            ssao_radius: 0.5,
            ssao_bias: 0.025,
            ssao_power: 1.5,
            ssao_intensity: 1.0,

            taa_resources: None,
            taa_fb_size: (0, 0),
            taa_pipeline: None,
            taa_desc_set: None,
            taa_enabled: true,
            taa_blend_factor: 0.1,
            prev_view_proj: None,
            taa_jitter_idx: 0,

            ssr_resources: None,
            ssr_fb_size: (0, 0),
            ssr_pipeline: None,
            ssr_desc_set: None,
            ssr_enabled: true,
            ssr_max_steps: 64.0,
            ssr_stride: 4.0,
            ssr_max_dist: 50.0,

            fog_resources: None,
            fog_fb_size: (0, 0),
            fog_pipeline: None,
            fog_desc_set: None,
            fog_enabled: true,
            fog_density: 0.02,
            fog_scattering: 1.0,
            fog_height_falloff: 0.1,
            fog_max_dist: 100.0,
            fog_max_steps: 64.0,
            fog_sun_intensity: 1.0,

            skybox_resources: None,
            skybox_fb_size: (0, 0),
            skybox_pipeline: None,
            skybox_desc_set: None,
            skybox_enabled: true,
            skybox_rayleigh: 1.5,
            skybox_mie: 0.5,
            skybox_zenith_shift: 0.1,
            skybox_exposure: 1.0,

            instanced_batcher: None,
            instanced_pipeline: None,
            instanced_gbuffer_pipeline: None,
            instanced_enabled: false,

            gpu_culling_resources: None,
            gpu_culling_enabled: false,

            mesh_shader_pipeline: None,
            mesh_shader_enabled: false,

            wireframe_scene_pipeline: None,
            overdraw_pipeline: None,
            light_complexity_pipeline: None,
            debug_render_mode: rustix_render::DebugRenderMode::default(),
            debug_render_resources: None,
            particle_system: crate::render::ParticleSystem::new(),

            viewport_framebuffers: (0..crate::ui::viewport::MAX_VIEWPORTS)
                .map(|_| [None, None, None])
                .collect(),
            viewport_fb_sizes: vec![(0, 0); crate::ui::viewport::MAX_VIEWPORTS],

            pipeline_2d: None,
            ubo_2d: None,
            desc_set_2d: None,
            quad_buffer_2d: None,
            texture_2d: None,

            selected_entities: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
            pending_delete: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
            dirty: std::rc::Rc::new(std::cell::Cell::new(false)),
            show_confirm: std::rc::Rc::new(std::cell::Cell::new(false)),
            confirm_target: std::rc::Rc::new(std::cell::Cell::new(ConfirmTarget::None)),
            show_settings: std::rc::Rc::new(std::cell::Cell::new(false)),
            renaming: std::rc::Rc::new(std::cell::RefCell::new(None)),
            rename_buffer: std::rc::Rc::new(std::cell::RefCell::new(String::new())),
            undo_history: std::rc::Rc::new(std::cell::RefCell::new(UndoHistory::new(100))),
            show_new_project_type: std::rc::Rc::new(std::cell::Cell::new(false)),
            new_project_type: std::rc::Rc::new(std::cell::Cell::new(ProjectType::Dim3)),

            sprite_editor: crate::sprite_editor::SpriteEditor::default(),
            audio_engine: AudioEngine::new().ok(),
            audio_instance: None,
            waveform_viewer: crate::waveform::WaveformViewer::new(),

            open_project: std::rc::Rc::new(std::cell::RefCell::new(None)),
            new_project: std::rc::Rc::new(std::cell::RefCell::new(None)),
            pending_mesh_load: std::rc::Rc::new(std::cell::RefCell::new(None)),
            pending_texture_load: std::rc::Rc::new(std::cell::RefCell::new(None)),
            pending_audio_load: std::rc::Rc::new(std::cell::RefCell::new(None)),
            pending_terrain_regen: std::rc::Rc::new(std::cell::Cell::new(false)),

            sounds: std::collections::HashMap::new(),

            asset_watcher: None,
            hot_reload_enabled: true,
            pak_archive: None,

            input_recorder: rustix_platform::recorder::InputRecorder::new(),
            recording_dir: dirs::config_dir()
                .map(|d| d.join("rustix").join("recordings"))
                .unwrap_or_else(|| std::path::PathBuf::from("recordings")),
            start_time: Instant::now(),
            animation_editor: AnimationEditor::default(),
            terrain_editor: crate::terrain::TerrainEditor::default(),
            prefab_editor: crate::prefab::PrefabEditor::default(),
            cli_project_path: None,
            cli_playtest: false,
            endless_runner_game: crate::endless_runner::EndlessRunnerGame::new(),
            breakout_game: crate::breakout::BreakoutGame::new(),
            platformer_game: crate::platformer::PlatformerGame::new(),
            scene_manager: crate::scene::SceneManager::new(),
            last_screen: crate::project::AppScreen::Startup,
            play_mode_snapshot: None,
        }
    }

    pub fn init_scene_resources(&mut self, renderer: &Renderer) {
        crate::init::init_scene_resources(
            renderer, &mut self.meshes,
            &mut self.scene_pipeline, &mut self.wireframe_scene_pipeline, &mut self.scene_descriptor_pool, &mut self.scene_descriptor_set,
            &mut self.scene_uniform_buffer, &mut self.scene_depth_buffer,
            &mut self.shadow_pipeline, &mut self.shadow_descriptor_pool, &mut self.shadow_descriptor_set,
            &mut self.csm_resources,
            &mut self.point_shadow_resources,
            &mut self.spot_shadow_resources,
            &mut self.tonemap_pipeline, &mut self.tonemap_desc_set,
            &mut self.bloom_extract_pipeline, &mut self.bloom_down_pipeline,
            &mut self.bloom_up_pipeline, &mut self.bloom_desc_set,
            &mut self.ssao_pipeline, &mut self.ssao_blur_pipeline,
            &mut self.ssao_desc_set,
            &mut self.taa_pipeline, &mut self.taa_desc_set,
            &mut self.ssr_pipeline, &mut self.ssr_desc_set,
            &mut self.fog_pipeline, &mut self.fog_desc_set,
            &mut self.skybox_pipeline, &mut self.skybox_desc_set,
            &mut self.instanced_pipeline, &mut self.instanced_gbuffer_pipeline,
            &mut self.mesh_shader_pipeline,
            &mut self.oit_accumulate_pipeline, &mut self.oit_composite_pipeline,
            &mut self.oit_desc_set,
        );
    }

    pub fn init_2d_resources(&mut self, renderer: &Renderer) {
        crate::init::init_2d_resources(
            renderer,
            &mut self.pipeline_2d, &mut self.ubo_2d, &mut self.desc_set_2d,
            &mut self.quad_buffer_2d, &mut self.texture_2d,
        );
    }

    pub fn try_create_fwd_plus(&mut self, renderer: &Renderer) {
        if self.fwd_plus_resources.is_none() {
            match crate::render::ForwardPlusResources::new(renderer) {
                Ok(res) => self.fwd_plus_resources = Some(res),
                Err(e) => tracing::error!("failed to create Forward+ resources: {e}"),
            }
        }
    }

    pub fn try_create_gbuffer(&mut self, renderer: &Renderer, extent: vk::Extent2D) {
        if self.gbuffer_resources.is_none() {
            if let Some(ref depth) = self.scene_depth_buffer {
                match crate::render::GBufferResources::new(renderer, extent, depth) {
                    Ok(res) => self.gbuffer_resources = Some(res),
                    Err(e) => tracing::error!("failed to create GBuffer resources: {e}"),
                }
            }
        }
    }

    pub fn try_recreate_hdr_framebuffer(&mut self, renderer: &Renderer, sw_extent: vk::Extent2D) {
        if self.hdr_fb_size != (sw_extent.width, sw_extent.height) {
            self.hdr_framebuffer = None;
            self.hdr_fb_size = (sw_extent.width, sw_extent.height);
            self.gbuffer_resources = None;
        }
        if self.hdr_framebuffer.is_none() {
            match rustix_render::HdrFramebuffer::new(renderer, sw_extent.width, sw_extent.height) {
                Ok(fb) => self.hdr_framebuffer = Some(fb),
                Err(e) => tracing::error!("HDR framebuffer creation failed: {e}"),
            }
        }
    }

    pub fn handle_hot_reload(&mut self, renderer: &Renderer) {
        if renderer.frame_index() > 0 {
            if let Some(reloader) = renderer.hot_reloader() {
                for path in reloader.take_events() {
                    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    match file {
                        "pbr.vert" | "pbr.frag" => {
                            crate::init::reload_scene_pipeline(renderer, &mut self.scene_pipeline);
                        }
                        "shadow.vert" => {
                            let bindless_layout = renderer.bindless_heap().layout();
                            crate::init::reload_shadow_pipeline(renderer, &mut self.shadow_pipeline, bindless_layout);
                        }
                        "tonemap.vert" | "tonemap.frag" => {
                            crate::init::reload_tonemap_pipeline(renderer, &mut self.tonemap_pipeline, &mut self.tonemap_desc_set);
                        }
                        "sprite.vert" | "sprite.frag" => {
                            crate::init::reload_2d_pipeline(renderer, &mut self.pipeline_2d, &mut self.desc_set_2d);
                        }
                        "light_cull.comp" => {
                            renderer.compute_pipeline_cache().clear();
                        }
                        "gbuffer.vert" | "gbuffer.frag" => {
                            if let Some(ref mut gbuf) = self.gbuffer_resources {
                                let bindless_layout = renderer.bindless_heap().layout();
                                match (
                                    rustix_render::shader::builtin::gbuffer_vertex_shader_override(renderer.device().logical()),
                                    rustix_render::shader::builtin::gbuffer_fragment_shader_override(renderer.device().logical()),
                                ) {
                                    (Ok(vs), Ok(fs)) => {
                                        match rustix_render::pipeline::GBufferPipeline::create(renderer.device(), &vs, &fs, bindless_layout) {
                                            Ok(p) => gbuf.gbuffer_pipeline = p,
                                            Err(e) => tracing::error!("gbuffer pipeline reload failed: {e}"),
                                        }
                                    }
                                    (Err(e), _) | (_, Err(e)) => tracing::error!("gbuffer shader reload failed: {e}"),
                                }
                            }
                        }
                        "deferred.vert" | "deferred.frag" => {
                            if let Some(ref mut gbuf) = self.gbuffer_resources {
                                let bindless_layout = renderer.bindless_heap().layout();
                                match (
                                    rustix_render::shader::builtin::deferred_vertex_shader_override(renderer.device().logical()),
                                    rustix_render::shader::builtin::deferred_fragment_shader_override(renderer.device().logical()),
                                ) {
                                    (Ok(vs), Ok(fs)) => {
                                        match rustix_render::pipeline::DeferredLightingPipeline::create(renderer.device(), &vs, &fs, bindless_layout) {
                                            Ok(p) => gbuf.deferred_pipeline = p,
                                            Err(e) => tracing::error!("deferred pipeline reload failed: {e}"),
                                        }
                                    }
                                    (Err(e), _) | (_, Err(e)) => tracing::error!("deferred shader reload failed: {e}"),
                                }
                            }
                        }
                        "oit_accumulate.vert" | "oit_accumulate.frag" => {
                            self.oit_accumulate_pipeline = None;
                            self.init_scene_resources(renderer);
                        }
                        "oit_composite.vert" | "oit_composite.frag" => {
                            self.oit_composite_pipeline = None;
                            self.oit_desc_set = None;
                            self.init_scene_resources(renderer);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub fn handle_pending_mesh_load(&mut self, renderer: &Renderer) {
        if let Some(path) = self.pending_mesh_load.borrow_mut().take() {
            if let Ok(data) = MappedFile::open(Path::new(&path)) {
                let mesh_name = Path::new(&path)
                    .file_stem().and_then(|s| s.to_str()).unwrap_or("Imported")
                    .to_string();
                let ext = Path::new(&path)
                    .extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                match crate::model_import::import_model(renderer, &data, &mesh_name, &ext) {
                    Ok(result) => {
                        let mat = &result.material;
                        tracing::info!("loaded mesh {mesh_name} from {path} (base={:?} rough={:.2} metal={:.2}) [degen={} zero_area={} nan={}]",
                            mat.base_color, mat.roughness, mat.metallic,
                            result.validation.degenerate_triangles,
                            result.validation.zero_area_faces,
                            result.validation.nan_vertices);
                        if !result.validation.warnings.is_empty() {
                            for w in &result.validation.warnings {
                                tracing::warn!("mesh validation: {}", w);
                            }
                        }
                        self.meshes.insert(mesh_name.clone(), result.mesh);
                        let e = self.ecs_world.spawn((
                            Transform { position: Vec3::new(0.0, 1.0, 0.0), rotation: Vec3::ZERO, scale: Vec3::ONE },
                            Name(mesh_name.clone()),
                            MeshComponent(mesh_name.clone()),
                            Material { base_color: Vec3::from(mat.base_color), alpha: 1.0, roughness: mat.roughness, metallic: mat.metallic, ao: mat.ao, emissive: mat.emissive },
                        ));
                        if let Some(skel) = result.skeleton {
                            let _ = self.ecs_world.insert(e, (skel,));
                        }
                        *self.selected_entities.borrow_mut() = vec![e];
                        self.dirty.set(true);
                    }
                    Err(e) => tracing::error!("failed to load mesh from {path}: {e}"),
                }
            } else {
                tracing::error!("failed to read file {path}");
            }
        }
    }

    pub fn handle_pending_texture_load(&mut self, renderer: &Renderer) {
        if let Some(path) = self.pending_texture_load.borrow_mut().take() {
            if let Ok(data) = MappedFile::open(Path::new(&path)) {
                let tex_name = Path::new(&path)
                    .file_stem().and_then(|s| s.to_str()).unwrap_or("Imported")
                    .to_string();
                let ext = Path::new(&path)
                    .extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                let is_normal = tex_name.to_lowercase().contains("normal") || tex_name.to_lowercase().contains("nrm");
                let options = crate::texture_import::TextureImportOptions {
                    compress: Some(rustix_asset::texture_compress::CompressedBlockFormat::Bc7Unorm),
                    generate_mips: true,
                    normal_map: is_normal,
                    srgb: !is_normal,
                };
                match crate::texture_import::import_texture(renderer, &data, &tex_name, &ext, &options) {
                    Ok(result) => {
                        tracing::info!("loaded texture {tex_name} from {path}: {}x{} mips={} compressed={} normal={}",
                            result.width, result.height, result.mip_levels, result.compressed, result.normal_map);
                        self.textures.insert(tex_name, result.texture);
                    }
                    Err(e) => tracing::error!("failed to load texture from {path}: {e}"),
                }
            } else {
                tracing::error!("failed to read file {path}");
            }
        }
    }

    pub fn handle_pending_audio_load(&mut self) {
        if let Some(path) = self.pending_audio_load.borrow_mut().take() {
            let audio_name = Path::new(&path)
                .file_stem().and_then(|s| s.to_str()).unwrap_or("Imported")
                .to_string();
            let ext = Path::new(&path)
                .extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            let is_music = audio_name.to_lowercase().contains("music")
                || audio_name.to_lowercase().contains("bgm")
                || audio_name.to_lowercase().contains("theme");
            let options = crate::audio_import::AudioImportOptions {
                streaming: is_music,
                volume: 1.0,
                looping: is_music,
                spatial_blend: 0.0,
            };
            match crate::audio_import::import_audio(Path::new(&path), &audio_name, &options) {
                Ok(result) => {
                    tracing::info!("loaded audio {audio_name} from {path}: {}Hz {}ch {:.2}s streaming={}",
                        result.sample_rate, result.channels, result.duration_seconds, result.streaming);
                    self.sounds.insert(audio_name, result);
                }
                Err(e) => tracing::error!("failed to load audio from {path}: {e}"),
            }
        }
    }

    pub fn handle_terrain_regen(&mut self, renderer: &Renderer) {
        let mut needs_regen = self.pending_terrain_regen.get() || self.terrain_editor.regen_needed;

        // Auto-detect terrain entities whose meshes are missing
        let terrain_entries: Vec<(hecs::Entity, String)> = {
            let mut q = self.ecs_world.query::<(hecs::Entity, &crate::terrain::Terrain)>()
;
            q.iter()
                .map(|(e, t)| (e, t.mesh_name.clone()))
                .collect()
        };

        for (_, mesh_name) in &terrain_entries {
            if !self.meshes.contains_key(mesh_name) {
                needs_regen = true;
            }
        }

        if !needs_regen {
            return;
        }
        self.pending_terrain_regen.set(false);
        self.terrain_editor.regen_needed = false;

        for (entity, mesh_name) in terrain_entries {
            if let Ok(terrain) = self.ecs_world.get::<&crate::terrain::Terrain>(entity) {
                let terrain = (*terrain).clone();
                match terrain.regenerate_mesh(renderer) {
                    Ok(mesh) => {
                        self.meshes.insert(mesh_name.clone(), mesh);
                        tracing::info!("regenerated terrain mesh '{}' ({}x{})", mesh_name, terrain.resolution, terrain.resolution);
                    }
                    Err(e) => {
                        tracing::error!("failed to regenerate terrain mesh: {e}");
                    }
                }
            }
        }
    }
}
