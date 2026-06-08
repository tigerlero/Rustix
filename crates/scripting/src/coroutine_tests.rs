//! Tests for script coroutine scheduler.

use crate::coroutine::{CoroutineScheduler, CutsceneCoroutine, YieldReason, CoroutineState, ScriptCoroutine};

#[test]
fn scheduler_new_is_empty() {
    let s = CoroutineScheduler::new();
    assert!(s.is_empty());
}

#[test]
fn scheduler_spawns_and_ticks_wait_seconds() {
    let mut s = CoroutineScheduler::new();
    let mut co = CutsceneCoroutine::new("wait_test");
    co.wait_seconds(0.5);
    s.spawn(Box::new(co));
    assert!(!s.is_empty());

    s.tick(0.1);
    assert!(!s.is_empty()); // still waiting
    s.tick(0.5);
    assert!(s.is_empty()); // completed
}

#[test]
fn scheduler_spawns_and_ticks_wait_frames() {
    let mut s = CoroutineScheduler::new();
    let mut co = CutsceneCoroutine::new("frame_test");
    co.wait_frames(3);
    s.spawn(Box::new(co));

    s.tick(0.016);
    assert!(!s.is_empty());
    s.tick(0.016);
    assert!(!s.is_empty());
    s.tick(0.016);
    assert!(!s.is_empty());
    s.tick(0.016);
    assert!(s.is_empty());
}

#[test]
fn cutscene_action_executes() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let executed = Arc::new(AtomicBool::new(false));
    let mut co = CutsceneCoroutine::new("action_test");
    {
        let flag = executed.clone();
        co.action(move || { flag.store(true, Ordering::SeqCst); });
    }
    co.resume(0.016);
    assert!(executed.load(Ordering::SeqCst));
}

#[test]
fn cutscene_completes_after_steps() {
    let mut co = CutsceneCoroutine::new("completion_test");
    co.wait_seconds(0.1);
    co.wait_seconds(0.2);

    assert_eq!(co.state(), CoroutineState::Running);
    co.resume(0.016); // returns WaitSeconds(0.1)
    assert_eq!(co.state(), CoroutineState::Running);
    co.resume(0.016); // returns WaitSeconds(0.2)
    assert_eq!(co.state(), CoroutineState::Running);
    let final_result = co.resume(0.016); // no more steps
    assert_eq!(co.state(), CoroutineState::Completed);
    assert!(final_result.is_none());
}

#[test]
fn scheduler_clear_empties() {
    let mut s = CoroutineScheduler::new();
    let mut co = CutsceneCoroutine::new("clear_test");
    co.wait_seconds(1.0);
    s.spawn(Box::new(co));
    assert!(!s.is_empty());
    s.clear();
    assert!(s.is_empty());
}
