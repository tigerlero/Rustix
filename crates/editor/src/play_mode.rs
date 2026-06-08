//! Play-in-editor mode: launch runtime without separate build.

/// State of the play-in-editor session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayModeState {
    Editing,
    Playing,
    Paused,
}

impl Default for PlayModeState {
    fn default() -> Self {
        PlayModeState::Editing
    }
}

/// Play-in-editor controller.
#[derive(Debug, Clone, Default)]
pub struct PlayModeController {
    pub state: PlayModeState,
    pub saved_scene: Option<Vec<u8>>,
}

impl PlayModeController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter play mode from editing.
    pub fn enter_play_mode(&mut self, serialized_scene: Vec<u8>) {
        self.saved_scene = Some(serialized_scene);
        self.state = PlayModeState::Playing;
    }

    /// Exit play mode and return to editing.
    pub fn exit_play_mode(&mut self) -> Option<Vec<u8>> {
        self.state = PlayModeState::Editing;
        self.saved_scene.take()
    }

    /// Pause the running simulation.
    pub fn pause(&mut self) {
        if self.state == PlayModeState::Playing {
            self.state = PlayModeState::Paused;
        }
    }

    /// Resume the paused simulation.
    pub fn resume(&mut self) {
        if self.state == PlayModeState::Paused {
            self.state = PlayModeState::Playing;
        }
    }

    pub fn is_playing(&self) -> bool {
        self.state == PlayModeState::Playing
    }

    pub fn is_paused(&self) -> bool {
        self.state == PlayModeState::Paused
    }

    pub fn is_editing(&self) -> bool {
        self.state == PlayModeState::Editing
    }
}
