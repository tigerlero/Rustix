//! Undo/redo system using the command pattern.

use std::any::Any;

/// A reversible editor command.
pub trait Command: Send + Sync {
    fn execute(&mut self);
    fn undo(&mut self);
    fn name(&self) -> &str;
}

/// Stack-based undo/redo history.
pub struct UndoStack {
    pub stack: Vec<Box<dyn Command>>,
    pub index: usize,
    pub max_size: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            index: 0,
            max_size: 100,
        }
    }
}

impl std::fmt::Debug for UndoStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UndoStack")
            .field("index", &self.index)
            .field("max_size", &self.max_size)
            .field("stack_len", &self.stack.len())
            .finish()
    }
}

impl UndoStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            stack: Vec::new(),
            index: 0,
            max_size,
        }
    }

    pub fn execute(&mut self, mut cmd: Box<dyn Command>) {
        cmd.execute();
        // Truncate redo branch
        if self.index < self.stack.len() {
            self.stack.truncate(self.index);
        }
        self.stack.push(cmd);
        self.index += 1;
        if self.stack.len() > self.max_size {
            self.stack.remove(0);
            self.index = self.index.saturating_sub(1);
        }
    }

    pub fn undo(&mut self) {
        if self.index == 0 {
            return;
        }
        self.index -= 1;
        if let Some(cmd) = self.stack.get_mut(self.index) {
            cmd.undo();
        }
    }

    pub fn redo(&mut self) {
        if self.index >= self.stack.len() {
            return;
        }
        if let Some(cmd) = self.stack.get_mut(self.index) {
            cmd.execute();
        }
        self.index += 1;
    }

    pub fn can_undo(&self) -> bool {
        self.index > 0
    }

    pub fn can_redo(&self) -> bool {
        self.index < self.stack.len()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
        self.index = 0;
    }
}
