//! Console panel: log levels, filtering, command input.

/// Severity level for console messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Debug
    }
}

/// A single console log entry.
#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: f64,
}

/// Console state with filtering and command history.
#[derive(Debug, Clone, Default)]
pub struct ConsoleState {
    pub entries: Vec<ConsoleEntry>,
    pub filter_level: LogLevel,
    pub filter_text: String,
    pub input_text: String,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub max_entries: usize,
}

impl ConsoleState {
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Default::default()
        }
    }

    pub fn log(&mut self, level: LogLevel, message: impl Into<String>, time: f64) {
        self.entries.push(ConsoleEntry {
            level,
            message: message.into(),
            timestamp: time,
        });
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    pub fn filtered_entries(&self) -> Vec<&ConsoleEntry> {
        self.entries
            .iter()
            .filter(|e| e.level >= self.filter_level)
            .filter(|e| {
                self.filter_text.is_empty()
                    || e.message.to_lowercase().contains(&self.filter_text.to_lowercase())
            })
            .collect()
    }

    pub fn submit_command(&mut self) {
        let cmd = self.input_text.trim();
        if !cmd.is_empty() {
            self.history.push(cmd.to_string());
            self.history_index = None;
            self.input_text.clear();
        }
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            Some(i) => i.saturating_sub(1),
            None => self.history.len() - 1,
        };
        self.history_index = Some(idx);
        self.input_text = self.history[idx].clone();
    }

    pub fn history_down(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            Some(i) => (i + 1).min(self.history.len() - 1),
            None => return,
        };
        self.history_index = Some(idx);
        self.input_text = self.history[idx].clone();
    }
}
