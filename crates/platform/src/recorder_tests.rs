//! Tests for input recorder and playback.

use std::io::Write;
use crate::input::types::{InputEvent, KeyCode};
use crate::recorder::*;

#[test]
fn recorder_mode_variants() {
    assert_ne!(RecorderMode::Idle, RecorderMode::Recording);
    assert_ne!(RecorderMode::Recording, RecorderMode::Playing);
    assert_ne!(RecorderMode::Playing, RecorderMode::Paused);
}

#[test]
fn input_recorder_new_idle() {
    let rec = InputRecorder::new();
    assert_eq!(rec.mode(), RecorderMode::Idle);
}

#[test]
fn input_recorder_start_stop_recording() {
    let mut rec = InputRecorder::new();
    rec.start_recording();
    assert_eq!(rec.mode(), RecorderMode::Recording);

    rec.record(InputEvent::KeyPress(KeyCode::A));
    rec.record(InputEvent::KeyRelease(KeyCode::A));

    let recording = rec.stop_recording();
    assert_eq!(rec.mode(), RecorderMode::Idle);
    assert_eq!(recording.events.len(), 2);
}

#[test]
fn input_recorder_no_record_when_idle() {
    let mut rec = InputRecorder::new();
    rec.record(InputEvent::KeyPress(KeyCode::A));
    let recording = rec.stop_recording();
    assert!(recording.events.is_empty());
}

#[test]
fn input_recording_serde_roundtrip() {
    let recording = InputRecording {
        events: vec![
            TimedEvent { time: 0.0, event: InputEvent::KeyPress(KeyCode::A) },
            TimedEvent { time: 0.1, event: InputEvent::KeyRelease(KeyCode::A) },
        ],
    };
    let json = serde_json::to_string(&recording).unwrap();
    let back: InputRecording = serde_json::from_str(&json).unwrap();
    assert_eq!(back.events.len(), 2);
    assert_eq!(back.events[0].time, 0.0);
    assert_eq!(back.events[1].time, 0.1);
}

#[test]
fn save_and_load_recording() {
    let tmp_path = std::env::temp_dir().join("rustix_recorder_test.json");
    let recording = InputRecording {
        events: vec![
            TimedEvent { time: 0.0, event: InputEvent::KeyPress(KeyCode::Space) },
        ],
    };
    save_recording(&tmp_path, &recording);
    let loaded = load_recording(&tmp_path).unwrap();
    assert_eq!(loaded.events.len(), 1);
    let _ = std::fs::remove_file(&tmp_path);
}

#[test]
fn load_recording_missing_file() {
    let result = load_recording(std::path::Path::new("/nonexistent/path/recording.json"));
    assert!(result.is_none());
}

#[test]
fn input_recorder_default() {
    let rec: InputRecorder = Default::default();
    assert_eq!(rec.mode(), RecorderMode::Idle);
}
