//! Goal-Oriented Action Planning (GOAP) for complex agents.
//!
//! A simple GOAP implementation where actions have preconditions
//! and effects, and the planner searches for the cheapest action
//! sequence that satisfies a goal state.

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

/// A symbolic fact in the world state.
pub type Fact = String;

/// World state represented as a set of boolean facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldState {
    facts: HashMap<Fact, bool>,
}

impl Hash for WorldState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut pairs: Vec<_> = self.facts.iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        for (k, v) in pairs {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl WorldState {
    pub fn new() -> Self {
        Self { facts: HashMap::new() }
    }

    pub fn with(mut self, fact: impl Into<Fact>, value: bool) -> Self {
        self.facts.insert(fact.into(), value);
        self
    }

    pub fn get(&self, fact: &str) -> bool {
        self.facts.get(fact).copied().unwrap_or(false)
    }

    pub fn satisfies(&self, preconditions: &[(Fact, bool)]) -> bool {
        preconditions.iter().all(|(f, v)| self.get(f) == *v)
    }

    pub fn apply(&mut self, effects: &[(Fact, bool)]) {
        for (f, v) in effects {
            self.facts.insert(f.clone(), *v);
        }
    }

    /// Distance heuristic: count of facts in `goal` that differ.
    pub fn distance(&self, goal: &[(Fact, bool)]) -> u32 {
        goal.iter()
            .filter(|(f, v)| self.get(f) != *v)
            .count() as u32
    }
}

/// A single action in the GOAP domain.
#[derive(Debug, Clone)]
pub struct GoapAction {
    pub name: String,
    pub cost: u32,
    pub preconditions: Vec<(Fact, bool)>,
    pub effects: Vec<(Fact, bool)>,
}

impl GoapAction {
    pub fn new(name: impl Into<String>, cost: u32) -> Self {
        Self {
            name: name.into(),
            cost,
            preconditions: Vec::new(),
            effects: Vec::new(),
        }
    }

    pub fn pre(mut self, fact: impl Into<Fact>, value: bool) -> Self {
        self.preconditions.push((fact.into(), value));
        self
    }

    pub fn effect(mut self, fact: impl Into<Fact>, value: bool) -> Self {
        self.effects.push((fact.into(), value));
        self
    }
}

/// GOAP planner that uses A* to find an action sequence.
pub struct GoapPlanner {
    actions: Vec<GoapAction>,
}

impl GoapPlanner {
    pub fn new(actions: Vec<GoapAction>) -> Self {
        Self { actions }
    }

    /// Plan a sequence of action names that transforms `initial_state`
    /// into a state satisfying all `goal` preconditions.
    pub fn plan(&self, initial_state: &WorldState, goal: &[(Fact, bool)]) -> Option<Vec<String>> {
        if initial_state.satisfies(goal) {
            return Some(Vec::new());
        }

        #[derive(Clone, PartialEq)]
        struct Node {
            cost: u32,
            heuristic: u32,
            state: WorldState,
            path: Vec<String>,
        }

        impl Eq for Node {}

        impl Ord for Node {
            fn cmp(&self, other: &Self) -> Ordering {
                (other.cost + other.heuristic)
                    .cmp(&(self.cost + self.heuristic))
            }
        }

        impl PartialOrd for Node {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut open = BinaryHeap::new();
        let mut visited: HashMap<WorldState, u32> = HashMap::new();

        let h = initial_state.distance(goal);
        open.push(Node {
            cost: 0,
            heuristic: h,
            state: initial_state.clone(),
            path: Vec::new(),
        });

        while let Some(node) = open.pop() {
            if node.state.satisfies(goal) {
                return Some(node.path);
            }

            if let Some(&best_cost) = visited.get(&node.state) {
                if node.cost > best_cost {
                    continue;
                }
            }

            for action in &self.actions {
                if node.state.satisfies(&action.preconditions) {
                    let mut next_state = node.state.clone();
                    next_state.apply(&action.effects);

                    let next_cost = node.cost + action.cost;
                    let should_push = match visited.get(&next_state) {
                        Some(&c) => next_cost < c,
                        None => true,
                    };

                    if should_push {
                        let mut next_path = node.path.clone();
                        next_path.push(action.name.clone());
                        let h = next_state.distance(goal);
                        visited.insert(next_state.clone(), next_cost);
                        open.push(Node {
                            cost: next_cost,
                            heuristic: h,
                            state: next_state,
                            path: next_path,
                        });
                    }
                }
            }
        }

        None
    }
}
