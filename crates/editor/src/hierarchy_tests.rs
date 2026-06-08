//! Tests for scene hierarchy and flattening.

use rustix_core::ecs::{Entity, EcsWorld};
use crate::hierarchy::{HierarchyNode, FlatNode, flatten_hierarchy, ReparentCommand};

fn spawn() -> Entity {
    let mut world = EcsWorld::new();
    world.spawn(())
}

#[test]
fn hierarchy_node_new() {
    let node = HierarchyNode::new(spawn(), "root");
    assert_eq!(node.name, "root");
    assert!(node.children.is_empty());
    assert!(node.expanded);
}

#[test]
fn flatten_hierarchy_empty() {
    let flat = flatten_hierarchy(&[]);
    assert!(flat.is_empty());
}

#[test]
fn flatten_hierarchy_single() {
    let nodes = vec![
        HierarchyNode::new(spawn(), "root"),
    ];
    let flat = flatten_hierarchy(&nodes);
    assert_eq!(flat.len(), 1);
    assert_eq!(flat[0].name, "root");
    assert_eq!(flat[0].depth, 0);
    assert!(!flat[0].has_children);
}

#[test]
fn flatten_hierarchy_nested() {
    let mut root = HierarchyNode::new(spawn(), "root");
    let child = HierarchyNode::new(spawn(), "child");
    root.children.push(child);
    let nodes = vec![root];
    let flat = flatten_hierarchy(&nodes);
    assert_eq!(flat.len(), 2);
    assert_eq!(flat[0].name, "root");
    assert_eq!(flat[0].depth, 0);
    assert_eq!(flat[1].name, "child");
    assert_eq!(flat[1].depth, 1);
}

#[test]
fn flatten_hierarchy_collapsed_skips_children() {
    let mut root = HierarchyNode::new(spawn(), "root");
    let child = HierarchyNode::new(spawn(), "child");
    root.children.push(child);
    root.expanded = false;
    let nodes = vec![root];
    let flat = flatten_hierarchy(&nodes);
    assert_eq!(flat.len(), 1);
}

#[test]
fn flatten_hierarchy_deep() {
    let mut root = HierarchyNode::new(spawn(), "root");
    let mut child = HierarchyNode::new(spawn(), "child");
    let grandchild = HierarchyNode::new(spawn(), "grandchild");
    child.children.push(grandchild);
    root.children.push(child);
    let nodes = vec![root];
    let flat = flatten_hierarchy(&nodes);
    assert_eq!(flat.len(), 3);
    assert_eq!(flat[2].depth, 2);
    assert_eq!(flat[2].name, "grandchild");
}

#[test]
fn reparent_command_new() {
    let e1 = spawn();
    let e2 = spawn();
    let cmd = ReparentCommand::new(e1, Some(e2));
    assert_eq!(cmd.entity, e1);
    assert_eq!(cmd.new_parent, Some(e2));
    assert!(cmd.old_parent.is_none());
}

#[test]
fn reparent_command_no_parent() {
    let e1 = spawn();
    let cmd = ReparentCommand::new(e1, None);
    assert!(cmd.new_parent.is_none());
}
