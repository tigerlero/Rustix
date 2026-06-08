//! Tests for utility AI curves and reasoning.

use std::collections::HashMap;

use crate::utility::{Curve, Consideration, UtilityAction, UtilityReasoner};

#[test]
fn curve_linear() {
    assert_eq!(Curve::Linear.evaluate(0.0), 0.0);
    assert_eq!(Curve::Linear.evaluate(0.5), 0.5);
    assert_eq!(Curve::Linear.evaluate(1.0), 1.0);
}

#[test]
fn curve_inverse() {
    assert_eq!(Curve::Inverse.evaluate(0.0), 1.0);
    assert_eq!(Curve::Inverse.evaluate(0.5), 0.5);
    assert_eq!(Curve::Inverse.evaluate(1.0), 0.0);
}

#[test]
fn curve_exponential() {
    let c = Curve::Exponential { exp: 2.0 };
    assert_eq!(c.evaluate(0.0), 0.0);
    assert_eq!(c.evaluate(1.0), 1.0);
    assert!((c.evaluate(0.5) - 0.25).abs() < 1e-4);
}

#[test]
fn curve_step() {
    let c = Curve::Step { threshold: 0.5 };
    assert_eq!(c.evaluate(0.4), 0.0);
    assert_eq!(c.evaluate(0.5), 1.0);
    assert_eq!(c.evaluate(0.6), 1.0);
}

#[test]
fn curve_sigmoid_midpoint() {
    let c = Curve::Sigmoid { steepness: 10.0, offset: 0.5 };
    let mid = c.evaluate(0.5);
    assert!((mid - 0.5).abs() < 0.01, "sigmoid at offset should be ~0.5, got {}", mid);
}

#[test]
fn curve_clamps_input() {
    assert_eq!(Curve::Linear.evaluate(-1.0), 0.0);
    assert_eq!(Curve::Linear.evaluate(2.0), 1.0);
}

#[test]
fn consideration_score() {
    let c = Consideration::new("dist", Curve::Inverse, 2.0);
    assert_eq!(c.score(1.0), 0.0); // inverse of 1 = 0, * weight 2 = 0
    assert_eq!(c.score(0.0), 2.0); // inverse of 0 = 1, * weight 2 = 2
}

#[test]
fn utility_action_no_considerations() {
    let action = UtilityAction::new("idle");
    let inputs = HashMap::new();
    assert_eq!(action.score(&inputs), 0.0);
}

#[test]
fn utility_action_missing_input_defaults_to_zero() {
    let action = UtilityAction::new("attack")
        .with(Consideration::new("health", Curve::Linear, 1.0));
    let inputs = HashMap::new();
    assert_eq!(action.score(&inputs), 0.0);
}

#[test]
fn utility_action_score_with_input() {
    let action = UtilityAction::new("attack")
        .with(Consideration::new("health", Curve::Linear, 1.0));
    let mut inputs = HashMap::new();
    inputs.insert("health".to_string(), 0.5);
    // score = 0.5 * 1.0 / weight_sum 1.0 = 0.5
    assert!((action.score(&inputs) - 0.5).abs() < 1e-4);
}

#[test]
fn utility_reasoner_select_best() {
    let actions = vec![
        UtilityAction::new("attack")
            .with(Consideration::new("dist", Curve::Inverse, 1.0)),
        UtilityAction::new("flee")
            .with(Consideration::new("dist", Curve::Linear, 1.0)),
    ];
    let reasoner = UtilityReasoner::new(actions);
    let mut inputs = HashMap::new();
    inputs.insert("dist".to_string(), 0.2); // close enemy -> inverse high, linear low
    let (name, _score) = reasoner.select(&inputs).unwrap();
    assert_eq!(name, "attack");
}

#[test]
fn utility_reasoner_ranked_order() {
    let actions = vec![
        UtilityAction::new("low")
            .with(Consideration::new("x", Curve::Linear, 1.0)),
        UtilityAction::new("high")
            .with(Consideration::new("x", Curve::Linear, 1.0)),
    ];
    let reasoner = UtilityReasoner::new(actions);
    let mut inputs = HashMap::new();
    inputs.insert("x".to_string(), 0.5);
    let ranked = reasoner.ranked(&inputs);
    // Both score the same with same consideration and input, so order may vary
    assert_eq!(ranked.len(), 2);
}

#[test]
fn utility_reasoner_empty_returns_none() {
    let reasoner = UtilityReasoner::new(vec![]);
    let inputs = HashMap::new();
    assert!(reasoner.select(&inputs).is_none());
}
