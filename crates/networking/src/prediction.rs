//! Client prediction and server reconciliation.
//!
//! The client predicts its own movement locally and sends inputs to the server.
//! When the server authoritative state arrives, the client reconciles by replaying
//! inputs from the last confirmed tick.

use std::collections::VecDeque;

/// A client-side predicted input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictedInput<T> {
    pub tick: u64,
    pub input: T,
}

/// Client-side prediction buffer.
#[derive(Debug, Clone)]
pub struct ClientPrediction<T: Clone> {
    /// Inputs sent but not yet acknowledged by the server.
    pub pending_inputs: VecDeque<PredictedInput<T>>,
    /// The last tick we received an authoritative snapshot for.
    pub last_confirmed_tick: u64,
    /// The next tick to assign to a new input.
    pub next_tick: u64,
}

impl<T: Clone> ClientPrediction<T> {
    pub fn new() -> Self {
        Self {
            pending_inputs: VecDeque::new(),
            last_confirmed_tick: 0,
            next_tick: 1,
        }
    }

    /// Record a new input for the current tick.
    ///
    /// Returns the tick number assigned to this input.
    pub fn push_input(&mut self, input: T) -> u64 {
        let tick = self.next_tick;
        self.next_tick += 1;
        self.pending_inputs.push_back(PredictedInput { tick, input });
        tick
    }

    /// Acknowledge inputs up to `server_tick`.
    ///
    /// Removes all pending inputs with `tick <= server_tick`.
    pub fn acknowledge(&mut self, server_tick: u64) {
        self.last_confirmed_tick = server_tick;
        while let Some(front) = self.pending_inputs.front() {
            if front.tick <= server_tick {
                self.pending_inputs.pop_front();
            } else {
                break;
            }
        }
    }

    /// Returns inputs that need to be replayed after reconciliation.
    ///
    /// These are inputs with `tick > last_confirmed_tick`.
    pub fn inputs_to_replay(&self) -> Vec<&PredictedInput<T>> {
        self.pending_inputs.iter().filter(|i| i.tick > self.last_confirmed_tick).collect()
    }
}

/// Server-side input history for a single client.
#[derive(Debug, Clone)]
pub struct ServerReconciliation<T: Clone> {
    /// Inputs received from the client, keyed by tick.
    pub received_inputs: VecDeque<PredictedInput<T>>,
    /// The last tick the server has fully processed.
    pub last_processed_tick: u64,
}

impl<T: Clone> ServerReconciliation<T> {
    pub fn new() -> Self {
        Self {
            received_inputs: VecDeque::new(),
            last_processed_tick: 0,
        }
    }

    /// Store an input from a client.
    pub fn receive_input(&mut self, tick: u64, input: T) {
        // Insert in sorted order by tick.
        let pos = self.received_inputs.iter().position(|i| i.tick > tick);
        let entry = PredictedInput { tick, input };
        match pos {
            Some(p) => self.received_inputs.insert(p, entry),
            None => self.received_inputs.push_back(entry),
        }
    }

    /// Consume and return all inputs up to `tick`.
    pub fn take_inputs_up_to(&mut self, tick: u64) -> Vec<PredictedInput<T>> {
        let mut result = Vec::new();
        while let Some(front) = self.received_inputs.front() {
            if front.tick <= tick {
                result.push(self.received_inputs.pop_front().unwrap());
            } else {
                break;
            }
        }
        self.last_processed_tick = tick;
        result
    }
}
