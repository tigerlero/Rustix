//! Finite state machine for simple NPCs.
//!
//! A lightweight FSM where states are closures and transitions are
//! evaluated each tick.

use std::collections::HashMap;
use std::any::Any;

/// Identifier for a state in the FSM.
pub type StateId = String;

/// A state in the finite state machine.
pub struct State {
    pub id: StateId,
    pub on_enter: Option<Box<dyn FnMut(&mut dyn Any) + Send + Sync>>,
    pub on_update: Box<dyn FnMut(&mut dyn Any, f32) + Send + Sync>,
    pub on_exit: Option<Box<dyn FnMut(&mut dyn Any) + Send + Sync>>,
}

impl State {
    pub fn new<Ctx: Send + Sync + 'static>(
        id: impl Into<StateId>,
        mut on_update: impl FnMut(&mut Ctx, f32) + Send + Sync + 'static,
    ) -> Self {
        let wrapped: Box<dyn FnMut(&mut dyn Any, f32) + Send + Sync> =
            Box::new(move |ctx: &mut dyn Any, dt: f32| {
                let typed = ctx.downcast_mut::<Ctx>().expect("FSM context type mismatch");
                on_update(typed, dt);
            });
        Self {
            id: id.into(),
            on_enter: None,
            on_update: wrapped,
            on_exit: None,
        }
    }

    pub fn on_enter<Ctx: Send + Sync + 'static>(
        mut self,
        mut f: impl FnMut(&mut Ctx) + Send + Sync + 'static,
    ) -> Self {
        self.on_enter = Some(Box::new(move |ctx: &mut dyn Any| {
            let typed = ctx.downcast_mut::<Ctx>().expect("FSM context type mismatch");
            f(typed);
        }));
        self
    }

    pub fn on_exit<Ctx: Send + Sync + 'static>(
        mut self,
        mut f: impl FnMut(&mut Ctx) + Send + Sync + 'static,
    ) -> Self {
        self.on_exit = Some(Box::new(move |ctx: &mut dyn Any| {
            let typed = ctx.downcast_mut::<Ctx>().expect("FSM context type mismatch");
            f(typed);
        }));
        self
    }
}

/// A transition condition evaluated each tick.
pub type TransitionCondition = Box<dyn Fn(&dyn Any) -> bool + Send + Sync>;

/// A transition from one state to another.
pub struct Transition {
    pub from: StateId,
    pub to: StateId,
    pub condition: TransitionCondition,
}

/// Finite state machine for NPC logic.
///
/// `Ctx` is the user-defined context type passed to state callbacks.
pub struct Fsm<Ctx: Send + Sync + 'static> {
    states: HashMap<StateId, State>,
    transitions: Vec<Transition>,
    current: Option<StateId>,
    _ctx: std::marker::PhantomData<Ctx>,
}

impl<Ctx: Send + Sync + 'static> Fsm<Ctx> {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            transitions: Vec::new(),
            current: None,
            _ctx: std::marker::PhantomData,
        }
    }

    /// Add a state to the machine.
    pub fn add_state(&mut self, state: State) {
        self.states.insert(state.id.clone(), state);
    }

    /// Add a transition between two states.
    pub fn add_transition(
        &mut self,
        from: impl Into<StateId>,
        to: impl Into<StateId>,
        condition: impl Fn(&Ctx) -> bool + Send + Sync + 'static,
    ) {
        self.transitions.push(Transition {
            from: from.into(),
            to: to.into(),
            condition: Box::new(move |ctx: &dyn Any| {
                let typed = ctx.downcast_ref::<Ctx>().expect("FSM context type mismatch");
                condition(typed)
            }),
        });
    }

    /// Set the initial state. Does **not** trigger `on_enter`.
    pub fn set_initial(&mut self, id: impl Into<StateId>) {
        self.current = Some(id.into());
    }

    /// Tick the FSM: update current state, evaluate transitions.
    pub fn tick(&mut self, ctx: &mut Ctx, dt: f32) {
        let current_id = match &self.current {
            Some(id) => id.clone(),
            None => return,
        };

        // Evaluate transitions
        let mut next = None;
        for trans in &self.transitions {
            if trans.from == current_id && (trans.condition)(ctx) {
                next = Some(trans.to.clone());
                break;
            }
        }

        // Switch state if a transition fired
        if let Some(next_id) = next {
            if next_id != current_id {
                if let Some(state) = self.states.get_mut(&current_id) {
                    if let Some(ref mut exit) = state.on_exit {
                        exit(ctx);
                    }
                }
                self.current = Some(next_id.clone());
                if let Some(state) = self.states.get_mut(&next_id) {
                    if let Some(ref mut enter) = state.on_enter {
                        enter(ctx);
                    }
                }
            }
        }

        // Update current state
        if let Some(id) = &self.current {
            if let Some(state) = self.states.get_mut(id) {
                (state.on_update)(ctx, dt);
            }
        }
    }

    /// Current state id, if any.
    pub fn current_state(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Check whether the machine is in a given state.
    pub fn is_in_state(&self, id: &str) -> bool {
        self.current.as_deref() == Some(id)
    }
}

impl<Ctx: Send + Sync + 'static> Default for Fsm<Ctx> {
    fn default() -> Self {
        Self::new()
    }
}

