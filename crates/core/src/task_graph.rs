use std::collections::VecDeque;

use crate::job::JobSystem;

/// Opaque identifier for a task in a [`TaskGraph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub usize);

/// A node in the task graph.
pub struct TaskNode {
    pub id: TaskId,
    pub name: String,
    pub(crate) func: Option<Box<dyn FnOnce() + Send>>,
    /// Tasks that must finish before this one starts.
    pub(crate) deps: Vec<TaskId>,
    /// Tasks that depend on this one.
    pub(crate) dependents: Vec<TaskId>,
}

/// A directed acyclic graph of tasks with dependency edges.
///
/// Tasks are added with [`TaskGraph::add_task`] and linked via
/// [`TaskGraph::add_dependency`].  The graph is executed with
/// [`TaskGraph::execute`], which topologically sorts the DAG and
/// runs each frontier in parallel on the [`JobSystem`] thread pool.
pub struct TaskGraph {
    tasks: Vec<TaskNode>,
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskGraph {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    /// Add a task and return its [`TaskId`].
    pub fn add_task<F>(&mut self, name: impl Into<String>, func: F) -> TaskId
    where
        F: FnOnce() + Send + 'static,
    {
        let id = TaskId(self.tasks.len());
        self.tasks.push(TaskNode {
            id,
            name: name.into(),
            func: Some(Box::new(func)),
            deps: Vec::new(),
            dependents: Vec::new(),
        });
        id
    }

    /// Add a dependency edge: `before` must complete before `after` starts.
    pub fn add_dependency(&mut self, before: TaskId, after: TaskId) {
        assert!(before.0 < self.tasks.len(), "invalid before task id");
        assert!(after.0 < self.tasks.len(), "invalid after task id");
        assert_ne!(before.0, after.0, "self-dependency is not allowed");

        self.tasks[before.0].dependents.push(after);
        self.tasks[after.0].deps.push(before);
    }

    /// Number of tasks in the graph.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Detect cycles via DFS. Returns `true` if the graph contains a cycle.
    pub fn has_cycle(&self) -> bool {
        let n = self.tasks.len();
        if n == 0 {
            return false;
        }

        #[derive(Clone, Copy)]
        enum State {
            Unvisited,
            Visiting,
            Visited,
        }

        let mut state = vec![State::Unvisited; n];

        fn dfs(graph: &TaskGraph, u: usize, state: &mut [State]) -> bool {
            state[u] = State::Visiting;
            for &TaskId(v) in &graph.tasks[u].dependents {
                match state[v] {
                    State::Visiting => return true,
                    State::Unvisited => {
                        if dfs(graph, v, state) {
                            return true;
                        }
                    }
                    State::Visited => {}
                }
            }
            state[u] = State::Visited;
            false
        }

        for u in 0..n {
            if matches!(state[u], State::Unvisited) && dfs(self, u, &mut state) {
                return true;
            }
        }
        false
    }

    /// Execute the task graph on the given [`JobSystem`].
    ///
    /// # Panics
    /// Panics if the graph contains a cycle.
    pub fn execute(&mut self, system: &JobSystem) {
        assert!(!self.has_cycle(), "task graph contains a cycle");

        // Kahn's algorithm: compute in-degree for each node.
        let n = self.tasks.len();
        let mut in_degree = vec![0usize; n];
        for node in &self.tasks {
            for &_dep in &node.deps {
                in_degree[node.id.0] += 1;
            }
        }

        let mut queue: VecDeque<usize> = (0..n)
            .filter(|&i| in_degree[i] == 0)
            .collect();

        let mut execution_order: Vec<Vec<usize>> = Vec::new();

        while !queue.is_empty() {
            // All tasks in `queue` are independent — run them in parallel.
            let frontier: Vec<usize> = queue.drain(..).collect();

            // Collect the closures and names for this frontier.
            let mut batch = Vec::with_capacity(frontier.len());
            for &idx in &frontier {
                let node = &mut self.tasks[idx];
                if let Some(func) = node.func.take() {
                    batch.push((node.name.clone(), func));
                }
            }

            system.install(|| {
                rayon::scope(|s| {
                    for (_name, f) in batch {
                        s.spawn(move |_| {
                            #[cfg(feature = "profiling")]
                            let _zone = tracy_client::span!(name);
                            f();
                        });
                    }
                });
            });

            execution_order.push(frontier.clone());

            // Update in-degrees for dependents.
            for &idx in &frontier {
                for &TaskId(dep_idx) in &self.tasks[idx].dependents {
                    in_degree[dep_idx] -= 1;
                    if in_degree[dep_idx] == 0 {
                        queue.push_back(dep_idx);
                    }
                }
            }
        }

        assert_eq!(
            execution_order.iter().map(|v| v.len()).sum::<usize>(),
            n,
            "not all tasks were executed (graph may be disconnected)"
        );
    }

    /// Return a topological ordering of all task ids.
    ///
    /// Returns `None` if the graph contains a cycle.
    pub fn topo_sort(&self) -> Option<Vec<TaskId>> {
        if self.has_cycle() {
            return None;
        }

        let n = self.tasks.len();
        let mut in_degree = vec![0usize; n];
        for node in &self.tasks {
            for &_dep in &node.deps {
                in_degree[node.id.0] += 1;
            }
        }

        let mut queue: VecDeque<usize> = (0..n)
            .filter(|&i| in_degree[i] == 0)
            .collect();

        let mut order = Vec::with_capacity(n);

        while let Some(idx) = queue.pop_front() {
            order.push(TaskId(idx));
            for &TaskId(dep_idx) in &self.tasks[idx].dependents {
                in_degree[dep_idx] -= 1;
                if in_degree[dep_idx] == 0 {
                    queue.push_back(dep_idx);
                }
            }
        }

        if order.len() == n {
            Some(order)
        } else {
            None // cycle detected (should not happen if has_cycle is correct)
        }
    }

    /// Names of tasks in the order they would be executed.
    pub fn execution_names(&self) -> Option<Vec<String>> {
        self.topo_sort()
            .map(|ids| ids.into_iter().map(|id| self.tasks[id.0].name.clone()).collect())
    }
}
