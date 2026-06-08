//! Scene hierarchy panel: tree view, drag-drop reparenting.

use rustix_core::ecs::Entity;

/// A node in the scene hierarchy tree.
#[derive(Debug, Clone)]
pub struct HierarchyNode {
    pub entity: Entity,
    pub name: String,
    pub children: Vec<HierarchyNode>,
    pub expanded: bool,
}

impl HierarchyNode {
    pub fn new(entity: Entity, name: impl Into<String>) -> Self {
        Self {
            entity,
            name: name.into(),
            children: Vec::new(),
            expanded: true,
        }
    }
}

/// Flattened list for UI rendering with indentation levels.
#[derive(Debug, Clone)]
pub struct FlatNode {
    pub entity: Entity,
    pub name: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}

/// Build a flat list from a hierarchy tree for immediate-mode UI rendering.
pub fn flatten_hierarchy(nodes: &[HierarchyNode]) -> Vec<FlatNode> {
    let mut result = Vec::new();
    fn visit(result: &mut Vec<FlatNode>, node: &HierarchyNode, depth: usize) {
        result.push(FlatNode {
            entity: node.entity,
            name: node.name.clone(),
            depth,
            has_children: !node.children.is_empty(),
            expanded: node.expanded,
        });
        if node.expanded {
            for child in &node.children {
                visit(result, child, depth + 1);
            }
        }
    }
    for node in nodes {
        visit(&mut result, node, 0);
    }
    result
}

/// Reparent an entity under a new parent in the hierarchy.
pub struct ReparentCommand {
    pub entity: Entity,
    pub old_parent: Option<Entity>,
    pub new_parent: Option<Entity>,
}

impl ReparentCommand {
    pub fn new(entity: Entity, new_parent: Option<Entity>) -> Self {
        Self {
            entity,
            old_parent: None,
            new_parent,
        }
    }
}
