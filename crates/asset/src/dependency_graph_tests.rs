//! Tests for asset dependency graph.

use std::path::PathBuf;
use crate::dependency_graph::DependencyGraph;

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
