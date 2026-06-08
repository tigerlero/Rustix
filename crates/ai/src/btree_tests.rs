//! Tests for behavior tree nodes.

use crate::btree::{Status, Blackboard, BehaviorNode, Action, Sequence, Selector, Parallel, Invert, Repeat, Condition, BehaviorTree};

#[test]
fn blackboard_set_get() {
    let mut bb = Blackboard::new();
    bb.set("score", 42i32);
    assert_eq!(*bb.get::<i32>("score").unwrap(), 42);
}

#[test]
fn blackboard_get_mut() {
    let mut bb = Blackboard::new();
    bb.set("x", 10i32);
    *bb.get_mut::<i32>("x").unwrap() += 5;
    assert_eq!(*bb.get::<i32>("x").unwrap(), 15);
}

#[test]
fn blackboard_remove() {
    let mut bb = Blackboard::new();
    bb.set("key", 1i32);
    bb.remove("key");
    assert!(bb.get::<i32>("key").is_none());
}

#[test]
fn blackboard_clear() {
    let mut bb = Blackboard::new();
    bb.set("a", 1i32);
    bb.set("b", 2i32);
    bb.clear();
    assert!(bb.get::<i32>("a").is_none());
}

#[test]
fn action_returns_status() {
    let mut action = Action::new(|_, _| Status::Success);
    let mut bb = Blackboard::new();
    assert_eq!(action.tick(&mut bb, 0.0), Status::Success);
}

#[test]
fn sequence_all_success() {
    let mut seq = Sequence::new(vec![
        Box::new(Action::new(|_, _| Status::Success)),
        Box::new(Action::new(|_, _| Status::Success)),
    ]);
    let mut bb = Blackboard::new();
    assert_eq!(seq.tick(&mut bb, 0.0), Status::Success);
}

#[test]
fn sequence_fails_on_first_failure() {
    let mut seq = Sequence::new(vec![
        Box::new(Action::new(|_, _| Status::Success)),
        Box::new(Action::new(|_, _| Status::Failure)),
    ]);
    let mut bb = Blackboard::new();
    assert_eq!(seq.tick(&mut bb, 0.0), Status::Failure);
}

#[test]
fn sequence_running_pauses() {
    let mut seq = Sequence::new(vec![
        Box::new(Action::new(|_, _| Status::Running)),
        Box::new(Action::new(|_, _| Status::Success)),
    ]);
    let mut bb = Blackboard::new();
    assert_eq!(seq.tick(&mut bb, 0.0), Status::Running);
}

#[test]
fn selector_first_success() {
    let mut sel = Selector::new(vec![
        Box::new(Action::new(|_, _| Status::Failure)),
        Box::new(Action::new(|_, _| Status::Success)),
    ]);
    let mut bb = Blackboard::new();
    assert_eq!(sel.tick(&mut bb, 0.0), Status::Success);
}

#[test]
fn selector_all_fail() {
    let mut sel = Selector::new(vec![
        Box::new(Action::new(|_, _| Status::Failure)),
        Box::new(Action::new(|_, _| Status::Failure)),
    ]);
    let mut bb = Blackboard::new();
    assert_eq!(sel.tick(&mut bb, 0.0), Status::Failure);
}

#[test]
fn selector_running_pauses() {
    let mut sel = Selector::new(vec![
        Box::new(Action::new(|_, _| Status::Running)),
        Box::new(Action::new(|_, _| Status::Success)),
    ]);
    let mut bb = Blackboard::new();
    assert_eq!(sel.tick(&mut bb, 0.0), Status::Running);
}

#[test]
fn parallel_success_threshold() {
    let mut par = Parallel::new(vec![
        Box::new(Action::new(|_, _| Status::Success)),
        Box::new(Action::new(|_, _| Status::Success)),
        Box::new(Action::new(|_, _| Status::Running)),
    ], 2, 2);
    let mut bb = Blackboard::new();
    assert_eq!(par.tick(&mut bb, 0.0), Status::Success);
}

#[test]
fn parallel_failure_threshold() {
    let mut par = Parallel::new(vec![
        Box::new(Action::new(|_, _| Status::Failure)),
        Box::new(Action::new(|_, _| Status::Failure)),
        Box::new(Action::new(|_, _| Status::Running)),
    ], 3, 2);
    let mut bb = Blackboard::new();
    assert_eq!(par.tick(&mut bb, 0.0), Status::Failure);
}

#[test]
fn parallel_running_when_no_threshold_met() {
    let mut par = Parallel::new(vec![
        Box::new(Action::new(|_, _| Status::Running)),
        Box::new(Action::new(|_, _| Status::Running)),
    ], 2, 2);
    let mut bb = Blackboard::new();
    assert_eq!(par.tick(&mut bb, 0.0), Status::Running);
}

#[test]
fn invert_success_to_failure() {
    let mut inv = Invert::new(Box::new(Action::new(|_, _| Status::Success)));
    let mut bb = Blackboard::new();
    assert_eq!(inv.tick(&mut bb, 0.0), Status::Failure);
}

#[test]
fn invert_failure_to_success() {
    let mut inv = Invert::new(Box::new(Action::new(|_, _| Status::Failure)));
    let mut bb = Blackboard::new();
    assert_eq!(inv.tick(&mut bb, 0.0), Status::Success);
}

#[test]
fn invert_running_passthrough() {
    let mut inv = Invert::new(Box::new(Action::new(|_, _| Status::Running)));
    let mut bb = Blackboard::new();
    assert_eq!(inv.tick(&mut bb, 0.0), Status::Running);
}

#[test]
fn repeat_counted() {
    let mut counter = 0usize;
    let mut rep = Repeat::new(Box::new(Action::new(move |_, _| {
        counter += 1;
        Status::Success
    })), Some(3));
    let mut bb = Blackboard::new();
    assert_eq!(rep.tick(&mut bb, 0.0), Status::Running);
    assert_eq!(rep.tick(&mut bb, 0.0), Status::Running);
    assert_eq!(rep.tick(&mut bb, 0.0), Status::Success);
}

#[test]
fn condition_true_runs_child() {
    let mut cond = Condition::new(
        |_| true,
        Box::new(Action::new(|_, _| Status::Success)),
    );
    let mut bb = Blackboard::new();
    assert_eq!(cond.tick(&mut bb, 0.0), Status::Success);
}

#[test]
fn condition_false_returns_failure() {
    let mut cond = Condition::new(
        |_| false,
        Box::new(Action::new(|_, _| Status::Success)),
    );
    let mut bb = Blackboard::new();
    assert_eq!(cond.tick(&mut bb, 0.0), Status::Failure);
}

#[test]
fn behavior_tree_tick() {
    let mut tree = BehaviorTree::new(Box::new(Action::new(|_, _| Status::Success)));
    let mut bb = Blackboard::new();
    assert_eq!(tree.tick(&mut bb, 0.0), Status::Success);
}
