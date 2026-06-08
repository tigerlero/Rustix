use std::collections::HashMap;
use std::any::Any;

/// Status returned by behavior tree node ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Success,
    Failure,
    Running,
}

/// Shared key-value store for behavior tree data.
pub struct Blackboard {
    data: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl Blackboard {
    pub fn new() -> Self { Self { data: HashMap::new() } }

    pub fn set<T: Send + Sync + 'static>(&mut self, key: &str, value: T) {
        self.data.insert(key.to_string(), Box::new(value));
    }

    pub fn get<T: Send + Sync + 'static>(&self, key: &str) -> Option<&T> {
        self.data.get(key).and_then(|v| v.downcast_ref::<T>())
    }

    pub fn get_mut<T: Send + Sync + 'static>(&mut self, key: &str) -> Option<&mut T> {
        self.data.get_mut(key).and_then(|v| v.downcast_mut::<T>())
    }

    pub fn remove(&mut self, key: &str) -> Option<Box<dyn Any + Send + Sync>> {
        self.data.remove(key)
    }

    pub fn clear(&mut self) { self.data.clear(); }
}

/// Trait implemented by all behavior tree nodes.
pub trait BehaviorNode: Send + Sync {
    fn tick(&mut self, blackboard: &mut Blackboard, dt: f32) -> Status;
    fn reset(&mut self) {}
    fn children(&self) -> &[Box<dyn BehaviorNode>] { &[] }
    fn children_mut(&mut self) -> &mut [Box<dyn BehaviorNode>] { &mut [] }
}

// --- Leaf nodes ---

/// A node that runs a closure and returns a status.
pub struct Action<F: FnMut(&mut Blackboard, f32) -> Status + Send + Sync> {
    func: F,
}

impl<F: FnMut(&mut Blackboard, f32) -> Status + Send + Sync> Action<F> {
    pub fn new(func: F) -> Self { Self { func } }
}

impl<F: FnMut(&mut Blackboard, f32) -> Status + Send + Sync> BehaviorNode for Action<F> {
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> Status { (self.func)(bb, dt) }
}

/// A condition node: ticks child only if predicate returns true.
pub struct Condition<F: FnMut(&mut Blackboard) -> bool + Send + Sync> {
    predicate: F,
    child: Box<dyn BehaviorNode>,
}

impl<F: FnMut(&mut Blackboard) -> bool + Send + Sync> Condition<F> {
    pub fn new(predicate: F, child: Box<dyn BehaviorNode>) -> Self { Self { predicate, child } }
}

impl<F: FnMut(&mut Blackboard) -> bool + Send + Sync> BehaviorNode for Condition<F> {
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> Status {
        if (self.predicate)(bb) { self.child.tick(bb, dt) } else { Status::Failure }
    }
    fn children(&self) -> &[Box<dyn BehaviorNode>] { std::slice::from_ref(&self.child) }
    fn children_mut(&mut self) -> &mut [Box<dyn BehaviorNode>] { std::slice::from_mut(&mut self.child) }
}

// --- Composite nodes ---

/// Sequence: ticks children in order. Fails on first Failure, succeeds when all succeed.
pub struct Sequence {
    children: Vec<Box<dyn BehaviorNode>>,
    current: usize,
}

impl Sequence {
    pub fn new(children: Vec<Box<dyn BehaviorNode>>) -> Self { Self { children, current: 0 } }
}

impl BehaviorNode for Sequence {
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> Status {
        while self.current < self.children.len() {
            match self.children[self.current].tick(bb, dt) {
                Status::Running => return Status::Running,
                Status::Failure => { self.current = 0; return Status::Failure; }
                Status::Success => self.current += 1,
            }
        }
        self.current = 0;
        Status::Success
    }
    fn reset(&mut self) { self.current = 0; for c in &mut self.children { c.reset(); } }
    fn children(&self) -> &[Box<dyn BehaviorNode>] { &self.children }
    fn children_mut(&mut self) -> &mut [Box<dyn BehaviorNode>] { &mut self.children }
}

/// Selector: ticks children in order. Succeeds on first Success, fails when all fail.
pub struct Selector {
    children: Vec<Box<dyn BehaviorNode>>,
    current: usize,
}

impl Selector {
    pub fn new(children: Vec<Box<dyn BehaviorNode>>) -> Self { Self { children, current: 0 } }
}

impl BehaviorNode for Selector {
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> Status {
        while self.current < self.children.len() {
            match self.children[self.current].tick(bb, dt) {
                Status::Running => return Status::Running,
                Status::Success => { self.current = 0; return Status::Success; }
                Status::Failure => self.current += 1,
            }
        }
        self.current = 0;
        Status::Failure
    }
    fn reset(&mut self) { self.current = 0; for c in &mut self.children { c.reset(); } }
    fn children(&self) -> &[Box<dyn BehaviorNode>] { &self.children }
    fn children_mut(&mut self) -> &mut [Box<dyn BehaviorNode>] { &mut self.children }
}

/// Parallel: ticks all children each frame.
///
/// Succeeds when at least `success_threshold` children have returned
/// Success. Fails when at least `failure_threshold` children have
/// returned Failure. Otherwise returns Running.
pub struct Parallel {
    children: Vec<Box<dyn BehaviorNode>>,
    success_threshold: usize,
    failure_threshold: usize,
    child_status: Vec<Status>,
}

impl Parallel {
    pub fn new(children: Vec<Box<dyn BehaviorNode>>, success_threshold: usize, failure_threshold: usize) -> Self {
        let n = children.len();
        Self {
            children,
            success_threshold,
            failure_threshold,
            child_status: vec![Status::Running; n],
        }
    }
}

impl BehaviorNode for Parallel {
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> Status {
        let mut success_count = 0usize;
        let mut failure_count = 0usize;

        for (i, child) in self.children.iter_mut().enumerate() {
            if self.child_status[i] == Status::Running {
                self.child_status[i] = child.tick(bb, dt);
            }
            match self.child_status[i] {
                Status::Success => success_count += 1,
                Status::Failure => failure_count += 1,
                Status::Running => {}
            }
        }

        if success_count >= self.success_threshold {
            self.reset();
            Status::Success
        } else if failure_count >= self.failure_threshold {
            self.reset();
            Status::Failure
        } else {
            Status::Running
        }
    }

    fn reset(&mut self) {
        self.child_status.fill(Status::Running);
        for c in &mut self.children { c.reset(); }
    }

    fn children(&self) -> &[Box<dyn BehaviorNode>] { &self.children }
    fn children_mut(&mut self) -> &mut [Box<dyn BehaviorNode>] { &mut self.children }
}

// --- Decorator nodes ---

/// Invert: returns Success for Failure and vice versa. Running passes through.
pub struct Invert {
    child: Box<dyn BehaviorNode>,
}

impl Invert {
    pub fn new(child: Box<dyn BehaviorNode>) -> Self { Self { child } }
}

impl BehaviorNode for Invert {
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> Status {
        match self.child.tick(bb, dt) {
            Status::Success => Status::Failure,
            Status::Failure => Status::Success,
            Status::Running => Status::Running,
        }
    }
    fn children(&self) -> &[Box<dyn BehaviorNode>] { std::slice::from_ref(&self.child) }
    fn children_mut(&mut self) -> &mut [Box<dyn BehaviorNode>] { std::slice::from_mut(&mut self.child) }
}

/// Repeat: repeats the child n times, or forever if n is None.
pub struct Repeat {
    child: Box<dyn BehaviorNode>,
    count: Option<usize>,
    current: usize,
}

impl Repeat {
    pub fn new(child: Box<dyn BehaviorNode>, count: Option<usize>) -> Self { Self { child, count, current: 0 } }
}

impl BehaviorNode for Repeat {
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> Status {
        if let Some(max) = self.count {
            if self.current >= max { return Status::Success; }
        }
        match self.child.tick(bb, dt) {
            Status::Running => Status::Running,
            _status => {
                self.current += 1;
                self.child.reset();
                if let Some(max) = self.count {
                    if self.current >= max { self.current = 0; Status::Success }
                    else { Status::Running }
                } else {
                    Status::Running
                }
            }
        }
    }
    fn reset(&mut self) { self.current = 0; self.child.reset(); }
    fn children(&self) -> &[Box<dyn BehaviorNode>] { std::slice::from_ref(&self.child) }
    fn children_mut(&mut self) -> &mut [Box<dyn BehaviorNode>] { std::slice::from_mut(&mut self.child) }
}

// --- Tree runner ---

/// The top-level behavior tree wrapper.
pub struct BehaviorTree {
    root: Box<dyn BehaviorNode>,
}

impl BehaviorTree {
    pub fn new(root: Box<dyn BehaviorNode>) -> Self { Self { root } }

    pub fn tick(&mut self, blackboard: &mut Blackboard, dt: f32) -> Status {
        self.root.tick(blackboard, dt)
    }

    pub fn reset(&mut self) { self.root.reset(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_success() {
        let mut seq = Sequence::new(vec![
            Box::new(Action::new(|_, _| Status::Success)),
            Box::new(Action::new(|_, _| Status::Success)),
        ]);
        let mut bb = Blackboard::new();
        assert_eq!(seq.tick(&mut bb, 0.0), Status::Success);
    }

    #[test]
    fn test_sequence_failure() {
        let mut seq = Sequence::new(vec![
            Box::new(Action::new(|_, _| Status::Success)),
            Box::new(Action::new(|_, _| Status::Failure)),
            Box::new(Action::new(|_, _| Status::Success)),
        ]);
        let mut bb = Blackboard::new();
        assert_eq!(seq.tick(&mut bb, 0.0), Status::Failure);
    }

    #[test]
    fn test_selector_first_succeeds() {
        let mut sel = Selector::new(vec![
            Box::new(Action::new(|_, _| Status::Failure)),
            Box::new(Action::new(|_, _| Status::Success)),
            Box::new(Action::new(|_, _| Status::Failure)),
        ]);
        let mut bb = Blackboard::new();
        assert_eq!(sel.tick(&mut bb, 0.0), Status::Success);
    }

    #[test]
    fn test_invert() {
        let mut inv = Invert::new(Box::new(Action::new(|_, _| Status::Failure)));
        let mut bb = Blackboard::new();
        assert_eq!(inv.tick(&mut bb, 0.0), Status::Success);
    }

    #[test]
    fn test_blackboard() {
        let mut bb = Blackboard::new();
        bb.set("health", 100i32);
        assert_eq!(*bb.get::<i32>("health").unwrap(), 100);
    }

    #[test]
    fn test_condition() {
        let mut cond = Condition::new(
            |bb| *bb.get::<i32>("alive").unwrap_or(&0) > 0,
            Box::new(Action::new(|_, _| Status::Success)),
        );
        let mut bb = Blackboard::new();
        bb.set("alive", 1i32);
        assert_eq!(cond.tick(&mut bb, 0.0), Status::Success);
        bb.set("alive", 0i32);
        assert_eq!(cond.tick(&mut bb, 0.0), Status::Failure);
    }

    #[test]
    fn test_behavior_tree() {
        let mut tree = BehaviorTree::new(Box::new(Sequence::new(vec![
            Box::new(Action::new(|_, _| Status::Success)),
            Box::new(Action::new(|_, _| Status::Success)),
        ])));
        let mut bb = Blackboard::new();
        assert_eq!(tree.tick(&mut bb, 0.0), Status::Success);
    }
}
