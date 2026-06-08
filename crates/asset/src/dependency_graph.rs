//! Dependency graph for incremental asset builds.
//!
//! Tracks which source assets depend on which other source assets so that
//! when a dependency changes, all transitive dependents are re-cooked.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// A directed dependency graph between source asset paths.
///
/// `A -> B` means "A depends on B" (B must be cooked before A).
/// When B changes, A (and all other dependents of B) must be re-cooked.
#[derive(Debug, Default, Clone)]
pub struct DependencyGraph {
    /// source path -> paths it directly depends on
    edges: HashMap<PathBuf, Vec<PathBuf>>,
    /// dependency path -> source paths that depend on it
    reverse: HashMap<PathBuf, Vec<PathBuf>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single dependency edge: `source` depends on `dependency`.
    pub fn add_edge(&mut self, source: impl AsRef<Path>, dependency: impl AsRef<Path>) {
        let source = source.as_ref().to_path_buf();
        let dependency = dependency.as_ref().to_path_buf();

        let source_deps = self.edges.entry(source.clone()).or_default();
        if !source_deps.contains(&dependency) {
            source_deps.push(dependency.clone());
            self.reverse.entry(dependency).or_default().push(source);
        }
    }

    /// Replace all outgoing dependency edges for `source`.
    pub fn set_dependencies(&mut self, source: impl AsRef<Path>, deps: &[PathBuf]) {
        let source = source.as_ref().to_path_buf();

        // Remove old edges for this source
        if let Some(old_deps) = self.edges.remove(&source) {
            for dep in old_deps {
                if let Some(list) = self.reverse.get_mut(&dep) {
                    list.retain(|p| p != &source);
                }
            }
        }

        // Add new edges
        for dep in deps {
            self.reverse.entry(dep.clone()).or_default().push(source.clone());
        }
        if !deps.is_empty() {
            self.edges.insert(source, deps.to_vec());
        }
    }

    /// Return the direct dependencies of `source`.
    pub fn dependencies_of(&self, source: impl AsRef<Path>) -> &[PathBuf] {
        self.edges.get(source.as_ref()).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Return the direct dependents of `dependency`.
    pub fn dependents_of(&self, dependency: impl AsRef<Path>) -> &[PathBuf] {
        self.reverse.get(dependency.as_ref()).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Return all transitive dependents of `path` (includes indirect dependents).
    pub fn transitive_dependents(&self, path: impl AsRef<Path>) -> Vec<PathBuf> {
        let path = path.as_ref();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        for direct in self.dependents_of(path) {
            if visited.insert(direct.clone()) {
                queue.push_back(direct.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            result.push(current.clone());
            for dep in self.dependents_of(&current) {
                if visited.insert(dep.clone()) {
                    queue.push_back(dep.clone());
                }
            }
        }

        result
    }

    /// Remove `source` and all of its outgoing edges from the graph.
    pub fn remove(&mut self, source: impl AsRef<Path>) {
        let source = source.as_ref();
        if let Some(deps) = self.edges.remove(source) {
            for dep in deps {
                if let Some(list) = self.reverse.get_mut(&dep) {
                    list.retain(|p| p != source);
                }
            }
        }
        for list in self.reverse.values_mut() {
            list.retain(|p| p != source);
        }
    }

    pub fn clear(&mut self) {
        self.edges.clear();
        self.reverse.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }

    // ── persistence ──

    /// Save the graph to a JSON file.
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let data = serde_json::to_string_pretty(&self.edges)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, data)
    }

    /// Load the graph from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let edges: HashMap<PathBuf, Vec<PathBuf>> = serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut reverse = HashMap::new();
        for (source, deps) in &edges {
            for dep in deps {
                reverse.entry(dep.clone()).or_insert_with(Vec::new).push(source.clone());
            }
        }

        Ok(Self { edges, reverse })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_transitive_dependents() {
        let mut g = DependencyGraph::new();
        g.add_edge("C", "B");
        g.add_edge("B", "A");

        let deps = g.transitive_dependents("A");
        assert!(deps.contains(&PathBuf::from("B")));
        assert!(deps.contains(&PathBuf::from("C")));
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_set_dependencies_replaces_old() {
        let mut g = DependencyGraph::new();
        g.add_edge("X", "Y");
        g.add_edge("X", "Z");
        g.set_dependencies("X", &[PathBuf::from("W")]);

        assert_eq!(g.dependencies_of("X"), &[PathBuf::from("W")]);
        assert!(g.dependents_of("Y").is_empty());
        assert!(g.dependents_of("Z").is_empty());
        assert_eq!(g.dependents_of("W"), &[PathBuf::from("X")]);
    }
}
