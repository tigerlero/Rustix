use std::collections::BinaryHeap;
use std::cmp::Ordering;

/// A node in a pathfinding graph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathNode {
    pub id: usize,
    pub x: f32,
    pub y: f32,
}

impl PathNode {
    pub fn new(id: usize, x: f32, y: f32) -> Self { Self { id, x, y } }
}

/// A path as a sequence of node indices.
#[derive(Debug, Clone)]
pub struct PathFinder {
    edges: Vec<Vec<(usize, f32)>>,
    nodes: Vec<PathNode>,
}

impl PathFinder {
    pub fn new(nodes: Vec<PathNode>, edges: Vec<Vec<(usize, f32)>>) -> Self {
        Self { nodes, edges }
    }

    /// Find path from start to goal using A* with Euclidean heuristic.
    pub fn find_path(&self, start: usize, goal: usize) -> Option<Vec<usize>> {
        if start >= self.nodes.len() || goal >= self.nodes.len() {
            return None;
        }

        let h = |a: usize, b: usize| -> f32 {
            let na = &self.nodes[a];
            let nb = &self.nodes[b];
            ((na.x - nb.x).powi(2) + (na.y - nb.y).powi(2)).sqrt()
        };

        let mut open = BinaryHeap::new();
        let mut g_score = vec![f32::INFINITY; self.nodes.len()];
        let mut came_from = vec![None; self.nodes.len()];

        g_score[start] = 0.0;
        open.push(State { cost: h(start, goal), node: start });

        while let Some(State { node: current, .. }) = open.pop() {
            if current == goal {
                let mut path = vec![goal];
                let mut cur = goal;
                while let Some(prev) = came_from[cur] {
                    path.push(prev);
                    cur = prev;
                }
                path.reverse();
                return Some(path);
            }

            for &(neighbor, weight) in &self.edges[current] {
                let tentative = g_score[current] + weight;
                if tentative < g_score[neighbor] {
                    g_score[neighbor] = tentative;
                    came_from[neighbor] = Some(current);
                    open.push(State { cost: tentative + h(neighbor, goal), node: neighbor });
                }
            }
        }
        None
    }

    pub fn node_count(&self) -> usize { self.nodes.len() }
    pub fn nodes(&self) -> &[PathNode] { &self.nodes }
}

#[derive(Clone, PartialEq)]
struct State {
    cost: f32,
    node: usize,
}

impl Eq for State {}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Create a grid-based PathFinder with 4-directional connectivity.
/// Grid dimensions: width x height. Nodes are indexed row-major.
pub fn a_star_grid(width: u32, height: u32, blocked: &[bool]) -> PathFinder {
    let n = (width * height) as usize;
    let mut nodes = Vec::with_capacity(n);
    let mut edges = vec![Vec::new(); n];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            nodes.push(PathNode::new(idx, x as f32, y as f32));

            if blocked[idx] { continue; }

            let dirs = [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)];
            for (dx, dy) in dirs {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    let nidx = (ny as u32 * width + nx as u32) as usize;
                    if !blocked[nidx] {
                        edges[idx].push((nidx, 1.0));
                    }
                }
            }
        }
    }
    PathFinder::new(nodes, edges)
}

/// Create a graph-based PathFinder from a list of nodes and edges.
pub fn a_star_graph(nodes: Vec<PathNode>, edges: Vec<Vec<(usize, f32)>>) -> PathFinder {
    PathFinder::new(nodes, edges)
}
