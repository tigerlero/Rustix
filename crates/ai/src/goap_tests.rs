//! Tests for GOAP planner and world state.

use crate::goap::{WorldState, GoapAction, GoapPlanner};

#[test]
fn world_state_default_is_false() {
    let state = WorldState::new();
    assert!(!state.get("any_key"));
}

#[test]
fn world_state_with_sets_value() {
    let state = WorldState::new().with("has_key", true);
    assert!(state.get("has_key"));
}

#[test]
fn world_state_satisfies_matching() {
    let state = WorldState::new().with("has_axe", true);
    assert!(state.satisfies(&[("has_axe".to_string(), true)]));
}

#[test]
fn world_state_satisfies_missing_is_false() {
    let state = WorldState::new();
    assert!(!state.satisfies(&[("has_axe".to_string(), true)]));
}

#[test]
fn world_state_apply_inserts() {
    let mut state = WorldState::new();
    state.apply(&[("has_wood".to_string(), true)]);
    assert!(state.get("has_wood"));
}

#[test]
fn world_state_apply_overwrites() {
    let mut state = WorldState::new().with("has_wood", true);
    state.apply(&[("has_wood".to_string(), false)]);
    assert!(!state.get("has_wood"));
}

#[test]
fn world_state_distance_zero_when_satisfied() {
    let state = WorldState::new().with("goal", true);
    assert_eq!(state.distance(&[("goal".to_string(), true)]), 0);
}

#[test]
fn world_state_distance_counts_differences() {
    let state = WorldState::new().with("a", true).with("b", false);
    let goal = vec![("a".to_string(), false), ("b".to_string(), false), ("c".to_string(), true)];
    assert_eq!(state.distance(&goal), 2);
}

#[test]
fn goap_action_builder() {
    let action = GoapAction::new("test", 5)
        .pre("has_tool", true)
        .effect("job_done", true);
    assert_eq!(action.name, "test");
    assert_eq!(action.cost, 5);
    assert_eq!(action.preconditions.len(), 1);
    assert_eq!(action.effects.len(), 1);
}

#[test]
fn planner_simple_sequence() {
    let actions = vec![
        GoapAction::new("get_axe", 1).effect("has_axe", true),
        GoapAction::new("chop_wood", 1)
            .pre("has_axe", true)
            .effect("has_wood", true),
    ];
    let planner = GoapPlanner::new(actions);
    let initial = WorldState::new();
    let goal = vec![("has_wood".to_string(), true)];
    let plan = planner.plan(&initial, &goal).unwrap();
    assert_eq!(plan, vec!["get_axe", "chop_wood"]);
}

#[test]
fn planner_already_satisfied() {
    let planner = GoapPlanner::new(vec![]);
    let initial = WorldState::new().with("rich", true);
    let goal = vec![("rich".to_string(), true)];
    let plan = planner.plan(&initial, &goal).unwrap();
    assert!(plan.is_empty());
}

#[test]
fn planner_no_solution() {
    let actions = vec![
        GoapAction::new("step_a", 1).effect("a", true),
    ];
    let planner = GoapPlanner::new(actions);
    let initial = WorldState::new();
    let goal = vec![("b".to_string(), true)];
    assert!(planner.plan(&initial, &goal).is_none());
}

#[test]
fn planner_prefers_cheaper_path() {
    let actions = vec![
        GoapAction::new("expensive", 10).effect("goal", true),
        GoapAction::new("cheap", 1).pre("prep", true).effect("goal", true),
        GoapAction::new("prep", 1).effect("prep", true),
    ];
    let planner = GoapPlanner::new(actions);
    let initial = WorldState::new();
    let goal = vec![("goal".to_string(), true)];
    let plan = planner.plan(&initial, &goal).unwrap();
    // Should take cheap route: prep -> cheap (cost 2) instead of expensive (cost 10)
    assert_eq!(plan, vec!["prep", "cheap"]);
}

#[test]
fn planner_multiple_preconditions() {
    let actions = vec![
        GoapAction::new("gather", 1).effect("has_wood", true),
        GoapAction::new("craft", 1)
            .pre("has_wood", true)
            .pre("has_tool", true)
            .effect("has_sword", true),
    ];
    let planner = GoapPlanner::new(actions);
    let initial = WorldState::new().with("has_tool", true);
    let goal = vec![("has_sword".to_string(), true)];
    let plan = planner.plan(&initial, &goal).unwrap();
    assert_eq!(plan, vec!["gather", "craft"]);
}

#[test]
fn world_state_clone_eq() {
    let a = WorldState::new().with("x", true);
    let b = a.clone();
    assert_eq!(a, b);
}
