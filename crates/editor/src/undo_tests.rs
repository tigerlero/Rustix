//! Tests for undo/redo stack.

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use crate::undo::{Command, UndoStack};

struct SharedCounter {
    value: Arc<AtomicI32>,
    delta: i32,
    name: &'static str,
}

impl Command for SharedCounter {
    fn execute(&mut self) {
        self.value.fetch_add(self.delta, Ordering::SeqCst);
    }
    fn undo(&mut self) {
        self.value.fetch_sub(self.delta, Ordering::SeqCst);
    }
    fn name(&self) -> &str { self.name }
}

#[test]
fn undo_stack_new() {
    let stack = UndoStack::new(50);
    assert_eq!(stack.index, 0);
    assert!(stack.stack.is_empty());
    assert!(!stack.can_undo());
    assert!(!stack.can_redo());
}

#[test]
fn undo_stack_default() {
    let stack: UndoStack = Default::default();
    assert_eq!(stack.max_size, 100);
}

#[test]
fn undo_stack_execute_and_undo() {
    let mut stack = UndoStack::new(100);
    let value = Arc::new(AtomicI32::new(0));

    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 5, name: "add5" }));
    assert_eq!(value.load(Ordering::SeqCst), 5);
    assert!(stack.can_undo());

    stack.undo();
    assert_eq!(value.load(Ordering::SeqCst), 0);
    assert!(!stack.can_undo());
}

#[test]
fn undo_stack_redo() {
    let mut stack = UndoStack::new(100);
    let value = Arc::new(AtomicI32::new(0));

    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 3, name: "add3" }));
    stack.undo();
    assert_eq!(value.load(Ordering::SeqCst), 0);
    assert!(stack.can_redo());

    stack.redo();
    assert_eq!(value.load(Ordering::SeqCst), 3);
    assert!(stack.can_undo());
    assert!(!stack.can_redo());
}

#[test]
fn undo_stack_multiple_commands() {
    let mut stack = UndoStack::new(100);
    let value = Arc::new(AtomicI32::new(0));

    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 2, name: "add2" }));
    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 3, name: "add3" }));
    assert_eq!(value.load(Ordering::SeqCst), 5);

    stack.undo();
    assert_eq!(value.load(Ordering::SeqCst), 2);

    stack.undo();
    assert_eq!(value.load(Ordering::SeqCst), 0);
}

#[test]
fn undo_stack_new_command_clears_redo() {
    let mut stack = UndoStack::new(100);
    let value = Arc::new(AtomicI32::new(0));

    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 2, name: "add2" }));
    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 3, name: "add3" }));
    stack.undo();

    // Now execute a new command — should clear the redo branch
    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 10, name: "add10" }));
    assert_eq!(value.load(Ordering::SeqCst), 12);
    assert!(!stack.can_redo());
}

#[test]
fn undo_stack_max_size_eviction() {
    let mut stack = UndoStack::new(2);
    let value = Arc::new(AtomicI32::new(0));

    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 1, name: "add1" }));
    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 1, name: "add1" }));
    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 1, name: "add1" }));
    assert_eq!(stack.stack.len(), 2);
    assert_eq!(stack.index, 2);
}

#[test]
fn undo_stack_clear() {
    let mut stack = UndoStack::new(100);
    let value = Arc::new(AtomicI32::new(0));

    stack.execute(Box::new(SharedCounter { value: value.clone(), delta: 5, name: "add5" }));
    stack.clear();
    assert!(stack.stack.is_empty());
    assert_eq!(stack.index, 0);
    assert!(!stack.can_undo());
}

#[test]
fn undo_stack_undo_at_start_is_noop() {
    let mut stack = UndoStack::new(100);
    stack.undo();
    assert_eq!(stack.index, 0);
}

#[test]
fn undo_stack_redo_at_end_is_noop() {
    let mut stack = UndoStack::new(100);
    stack.redo();
    assert_eq!(stack.index, 0);
}

