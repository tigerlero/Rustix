//! Animation state machine for managing animation states and transitions.

use std::collections::HashMap;

/// A condition that must be satisfied for a transition to trigger.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionCondition {
    /// Trigger immediately (no condition).
    Always,
    /// Wait until the current clip has played for at least `seconds`.
    TimeElapsed(f32),
    /// Wait until the current clip is within `seconds` of ending.
    TimeRemaining(f32),
    /// A named trigger parameter (set externally, then consumed).
    Trigger(String),
    /// A float parameter must be greater than or equal to `threshold`.
    ParameterGte { name: String, threshold: f32 },
    /// A float parameter must be less than `threshold`.
    ParameterLt { name: String, threshold: f32 },
    /// A bool parameter must be `true`.
    ParameterBool { name: String, value: bool },
    /// Combine two conditions with logical AND.
    And(Box<TransitionCondition>, Box<TransitionCondition>),
}

impl TransitionCondition {
    /// Evaluate the condition against runtime state.
    pub fn evaluate(
        &self,
        current_time: f32,
        duration: f32,
        triggers: &mut HashMap<String, bool>,
        parameters: &HashMap<String, f32>,
        bool_parameters: &HashMap<String, bool>,
    ) -> bool {
        match self {
            TransitionCondition::Always => true,
            TransitionCondition::TimeElapsed(t) => current_time >= *t,
            TransitionCondition::TimeRemaining(t) => duration - current_time <= *t,
            TransitionCondition::Trigger(name) => {
                triggers.remove(name).unwrap_or(false)
            }
            TransitionCondition::ParameterGte { name, threshold } => {
                parameters.get(name).copied().unwrap_or(0.0) >= *threshold
            }
            TransitionCondition::ParameterLt { name, threshold } => {
                parameters.get(name).copied().unwrap_or(0.0) < *threshold
            }
            TransitionCondition::ParameterBool { name, value } => {
                bool_parameters.get(name).copied().unwrap_or(false) == *value
            }
            TransitionCondition::And(a, b) => {
                a.evaluate(current_time, duration, triggers, parameters, bool_parameters)
                    && b.evaluate(current_time, duration, triggers, parameters, bool_parameters)
            }
        }
    }
}

/// A transition from one animation state to another.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub target_state: String,
    pub condition: TransitionCondition,
    pub blend_duration: f32,
}

impl Transition {
    pub fn new(target_state: impl Into<String>, condition: TransitionCondition, blend_duration: f32) -> Self {
        Self {
            target_state: target_state.into(),
            condition,
            blend_duration: blend_duration.max(0.0),
        }
    }
}

/// A single state in the animation state machine.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationState {
    pub clip_name: String,
    pub looped: bool,
    pub speed: f32,
    pub transitions: Vec<Transition>,
}

impl AnimationState {
    pub fn new(clip_name: impl Into<String>) -> Self {
        Self {
            clip_name: clip_name.into(),
            looped: true,
            speed: 1.0,
            transitions: Vec::new(),
        }
    }

    pub fn with_transition(mut self, transition: Transition) -> Self {
        self.transitions.push(transition);
        self
    }
}

/// An animation state machine that manages states, transitions, and parameters.
#[derive(Debug, Clone)]
pub struct AnimationStateMachine {
    pub states: HashMap<String, AnimationState>,
    pub current_state: String,
    pub current_time: f32,
    pub parameters: HashMap<String, f32>,
    pub bool_parameters: HashMap<String, bool>,
    pub triggers: HashMap<String, bool>,
    pub blend_time: f32,
    pub blend_duration: f32,
}

impl AnimationStateMachine {
    pub fn new(initial_state: impl Into<String>, states: Vec<AnimationState>) -> Self {
        let initial = initial_state.into();
        let mut map = HashMap::with_capacity(states.len());
        for state in states {
            map.insert(state.clip_name.clone(), state);
        }
        Self {
            states: map,
            current_state: initial.clone(),
            current_time: 0.0,
            parameters: HashMap::new(),
            bool_parameters: HashMap::new(),
            triggers: HashMap::new(),
            blend_time: 0.0,
            blend_duration: 0.0,
        }
    }

    pub fn set_parameter(&mut self, name: impl Into<String>, value: f32) {
        self.parameters.insert(name.into(), value);
    }

    pub fn set_bool_parameter(&mut self, name: impl Into<String>, value: bool) {
        self.bool_parameters.insert(name.into(), value);
    }

    pub fn set_trigger(&mut self, name: impl Into<String>) {
        self.triggers.insert(name.into(), true);
    }

    /// Advance the state machine by `dt` and return the active clip name + blend info.
    ///
    /// Returns `(current_clip, previous_clip, blend_weight)` where `blend_weight`
    /// is 1.0 when fully in the current state, and 0.0 at the start of a transition.
    pub fn update(&mut self, dt: f32) -> (&str, Option<&str>, f32) {
        if let Some(state) = self.states.get(&self.current_state) {
            self.current_time += dt * state.speed;

            // Check transitions
            for transition in &state.transitions {
                let duration = if state.looped { f32::MAX } else { 1.0 }; // rough estimate for non-looped
                if transition.condition.evaluate(
                    self.current_time,
                    duration,
                    &mut self.triggers,
                    &self.parameters,
                    &self.bool_parameters,
                ) {
                    self.current_state = transition.target_state.clone();
                    self.current_time = 0.0;
                    self.blend_time = 0.0;
                    self.blend_duration = transition.blend_duration;
                    break;
                }
            }
        }

        let blend_weight = if self.blend_duration > 0.0 {
            let w = self.blend_time / self.blend_duration;
            self.blend_time += dt;
            w.min(1.0)
        } else {
            1.0
        };

        let current_clip = self.current_state.as_str();
        let previous_clip = None; // For now, simple single-transition blending
        (current_clip, previous_clip, blend_weight)
    }
}
