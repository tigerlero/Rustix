//! Tests for play-in-editor mode controller.

use crate::play_mode::{PlayModeController, PlayModeState};

#[test]
fn play_mode_state_variants() {
    assert_ne!(PlayModeState::Editing, PlayModeState::Playing);
    assert_ne!(PlayModeState::Playing, PlayModeState::Paused);
}

#[test]
fn play_mode_state_default() {
    assert_eq!(PlayModeState::default(), PlayModeState::Editing);
}

#[test]
fn play_mode_controller_new() {
    let ctrl = PlayModeController::new();
    assert!(ctrl.is_editing());
    assert!(!ctrl.is_playing());
    assert!(!ctrl.is_paused());
    assert!(ctrl.saved_scene.is_none());
}

#[test]
fn play_mode_controller_default() {
    let ctrl: PlayModeController = Default::default();
    assert!(ctrl.is_editing());
}

#[test]
fn play_mode_controller_enter_and_exit() {
    let mut ctrl = PlayModeController::new();
    ctrl.enter_play_mode(vec![1, 2, 3]);
    assert!(ctrl.is_playing());
    assert_eq!(ctrl.saved_scene, Some(vec![1, 2, 3]));

    let scene = ctrl.exit_play_mode();
    assert!(ctrl.is_editing());
    assert_eq!(scene, Some(vec![1, 2, 3]));
    assert!(ctrl.saved_scene.is_none());
}

#[test]
fn play_mode_controller_pause_and_resume() {
    let mut ctrl = PlayModeController::new();
    ctrl.enter_play_mode(vec![]);
    assert!(ctrl.is_playing());

    ctrl.pause();
    assert!(ctrl.is_paused());
    assert!(!ctrl.is_playing());

    ctrl.resume();
    assert!(ctrl.is_playing());
    assert!(!ctrl.is_paused());
}

#[test]
fn play_mode_controller_pause_only_when_playing() {
    let mut ctrl = PlayModeController::new();
    ctrl.pause();
    assert!(ctrl.is_editing()); // no change
}

#[test]
fn play_mode_controller_resume_only_when_paused() {
    let mut ctrl = PlayModeController::new();
    ctrl.resume();
    assert!(ctrl.is_editing()); // no change
}
