use rustix_core::ecs::EcsWorld;
use rustix_core::math::Vec3;
use crate::project::EditorCameraState;
use crate::scene::{SceneData, SceneManager, scene_to_world, world_to_scene};
use crate::player::PlayerManager;

/// Captures editor state before entering play-mode so it can be restored
/// when exiting back to the editor.
#[derive(Clone)]
pub struct PlayModeSnapshot {
    pub scene_data: SceneData,
    pub camera_state: EditorCameraState,
    pub camera_controlling_player: bool,
    pub player_manager: PlayerManager,
    pub scene_manager: SceneManager,
}

impl PlayModeSnapshot {
    pub fn capture(
        world: &EcsWorld,
        camera: &crate::camera::EditorCamera,
        player_manager: &PlayerManager,
        scene_manager: &SceneManager,
    ) -> Self {
        Self {
            scene_data: world_to_scene(world),
            camera_state: EditorCameraState {
                position: camera.position.into(),
                center: camera.center.into(),
                yaw: camera.yaw,
                pitch: camera.pitch,
                distance: camera.distance,
                mode: camera.mode,
                follow_target: camera.follow_target,
            },
            camera_controlling_player: camera.controlling_player,
            player_manager: player_manager.clone(),
            scene_manager: scene_manager.clone(),
        }
    }

    pub fn restore(
        &self,
        world: &mut EcsWorld,
        camera: &mut crate::camera::EditorCamera,
        player_manager: &mut PlayerManager,
        scene_manager: &mut SceneManager,
    ) {
        scene_to_world(world, &self.scene_data);
        camera.position = self.camera_state.position.into();
        camera.center = self.camera_state.center.into();
        camera.yaw = self.camera_state.yaw;
        camera.pitch = self.camera_state.pitch;
        camera.distance = self.camera_state.distance;
        camera.mode = self.camera_state.mode;
        camera.follow_target = self.camera_state.follow_target;
        camera.controlling_player = self.camera_controlling_player;
        *player_manager = self.player_manager.clone();
        *scene_manager = self.scene_manager.clone();
    }
}

#[cfg(test)]
#[path = "play_mode_tests.rs"]
mod play_mode_tests;
