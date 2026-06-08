//! Event subscription system for scripts.

use std::collections::HashMap;

/// A script callback stored as a Rhai function name or closure identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventCallback {
    pub script_id: u64,
    pub function_name: String,
}

/// Event bus for script-to-engine and script-to-script communication.
#[derive(Debug, Default)]
pub struct ScriptEventBus {
    subscribers: HashMap<String, Vec<EventCallback>>,
}

impl ScriptEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe a script function to an event name.
    pub fn subscribe(&mut self, event_name: impl Into<String>, callback: EventCallback) {
        self.subscribers.entry(event_name.into()).or_default().push(callback);
    }

    /// Unsubscribe all callbacks for a given script.
    pub fn unsubscribe_script(&mut self, script_id: u64) {
        for subs in self.subscribers.values_mut() {
            subs.retain(|cb| cb.script_id != script_id);
        }
    }

    /// Get all callbacks for an event name.
    pub fn get_subscribers(&self, event_name: &str) -> &[EventCallback] {
        self.subscribers.get(event_name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Emit an event (the engine invokes the subscribed callbacks).
    pub fn emit(&self, event_name: &str) -> Vec<&EventCallback> {
        self.get_subscribers(event_name).iter().collect()
    }
}
