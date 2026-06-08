//! Tests for animation state machine transitions and conditions.

use std::collections::HashMap;
use crate::state_machine::*;

// ------------------------------------------------------------------
// TransitionCondition tests
// ------------------------------------------------------------------

#[test]
fn condition_always_true() {
    let cond = TransitionCondition::Always;
    assert!(cond.evaluate(0.0, 1.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
}

#[test]
fn condition_time_elapsed_true() {
    let cond = TransitionCondition::TimeElapsed(1.0);
    assert!(cond.evaluate(1.5, 2.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
}

#[test]
fn condition_time_elapsed_false() {
    let cond = TransitionCondition::TimeElapsed(1.0);
    assert!(!cond.evaluate(0.5, 2.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
}

#[test]
fn condition_time_remaining_true() {
    let cond = TransitionCondition::TimeRemaining(0.5);
    assert!(cond.evaluate(1.6, 2.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
}

#[test]
fn condition_time_remaining_false() {
    let cond = TransitionCondition::TimeRemaining(0.5);
    assert!(!cond.evaluate(0.5, 2.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
}

#[test]
fn condition_trigger_consumed() {
    let cond = TransitionCondition::Trigger("jump".to_string());
    let mut triggers = HashMap::new();
    triggers.insert("jump".to_string(), true);
    assert!(cond.evaluate(0.0, 1.0, &mut triggers, &HashMap::new(), &HashMap::new()));
    // Trigger is consumed after evaluation
    assert!(triggers.get("jump").copied().unwrap_or(false) == false || triggers.get("jump").is_none());
}

#[test]
fn condition_trigger_missing() {
    let cond = TransitionCondition::Trigger("jump".to_string());
    let mut triggers = HashMap::new();
    assert!(!cond.evaluate(0.0, 1.0, &mut triggers, &HashMap::new(), &HashMap::new()));
}

#[test]
fn condition_parameter_gte_true() {
    let cond = TransitionCondition::ParameterGte { name: "speed".to_string(), threshold: 5.0 };
    let mut params = HashMap::new();
    params.insert("speed".to_string(), 7.0);
    assert!(cond.evaluate(0.0, 1.0, &mut HashMap::new(), &params, &HashMap::new()));
}

#[test]
fn condition_parameter_gte_false() {
    let cond = TransitionCondition::ParameterGte { name: "speed".to_string(), threshold: 5.0 };
    let mut params = HashMap::new();
    params.insert("speed".to_string(), 3.0);
    assert!(!cond.evaluate(0.0, 1.0, &mut HashMap::new(), &params, &HashMap::new()));
}

#[test]
fn condition_parameter_lt_true() {
    let cond = TransitionCondition::ParameterLt { name: "speed".to_string(), threshold: 5.0 };
    let mut params = HashMap::new();
    params.insert("speed".to_string(), 3.0);
    assert!(cond.evaluate(0.0, 1.0, &mut HashMap::new(), &params, &HashMap::new()));
}

#[test]
fn condition_parameter_lt_false() {
    let cond = TransitionCondition::ParameterLt { name: "speed".to_string(), threshold: 5.0 };
    let mut params = HashMap::new();
    params.insert("speed".to_string(), 7.0);
    assert!(!cond.evaluate(0.0, 1.0, &mut HashMap::new(), &params, &HashMap::new()));
}

#[test]
fn condition_parameter_bool_true() {
    let cond = TransitionCondition::ParameterBool { name: "grounded".to_string(), value: true };
    let mut bools = HashMap::new();
    bools.insert("grounded".to_string(), true);
    assert!(cond.evaluate(0.0, 1.0, &mut HashMap::new(), &HashMap::new(), &bools));
}

#[test]
fn condition_parameter_bool_false() {
    let cond = TransitionCondition::ParameterBool { name: "grounded".to_string(), value: true };
    let mut bools = HashMap::new();
    bools.insert("grounded".to_string(), false);
    assert!(!cond.evaluate(0.0, 1.0, &mut HashMap::new(), &HashMap::new(), &bools));
}

#[test]
fn condition_and_both_true() {
    let a = Box::new(TransitionCondition::Always);
    let b = Box::new(TransitionCondition::Always);
    let cond = TransitionCondition::And(a, b);
    assert!(cond.evaluate(0.0, 1.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
}

#[test]
fn condition_and_one_false() {
    let a = Box::new(TransitionCondition::Always);
    let b = Box::new(TransitionCondition::TimeElapsed(10.0));
    let cond = TransitionCondition::And(a, b);
    assert!(!cond.evaluate(0.0, 1.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
}

#[test]
fn condition_missing_param_defaults() {
    let cond_gte = TransitionCondition::ParameterGte { name: "missing".to_string(), threshold: 0.0 };
    let cond_lt = TransitionCondition::ParameterLt { name: "missing".to_string(), threshold: 0.0 };
    let cond_bool = TransitionCondition::ParameterBool { name: "missing".to_string(), value: true };
    assert!(cond_gte.evaluate(0.0, 1.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
    assert!(!cond_lt.evaluate(0.0, 1.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
    assert!(!cond_bool.evaluate(0.0, 1.0, &mut HashMap::new(), &HashMap::new(), &HashMap::new()));
}

// ------------------------------------------------------------------
// Transition tests
// ------------------------------------------------------------------

#[test]
fn transition_new() {
    let t = Transition::new("run", TransitionCondition::Always, 0.25);
    assert_eq!(t.target_state, "run");
    assert_eq!(t.blend_duration, 0.25);
}

#[test]
fn transition_blend_duration_clamped() {
    let t = Transition::new("run", TransitionCondition::Always, -1.0);
    assert_eq!(t.blend_duration, 0.0);
}

// ------------------------------------------------------------------
// AnimationState tests
// ------------------------------------------------------------------

#[test]
fn animation_state_new_defaults() {
    let state = AnimationState::new("idle");
    assert_eq!(state.clip_name, "idle");
    assert!(state.looped);
    assert_eq!(state.speed, 1.0);
    assert!(state.transitions.is_empty());
}

#[test]
fn animation_state_with_transition() {
    let state = AnimationState::new("idle")
        .with_transition(Transition::new("run", TransitionCondition::Always, 0.2));
    assert_eq!(state.transitions.len(), 1);
    assert_eq!(state.transitions[0].target_state, "run");
}

// ------------------------------------------------------------------
// AnimationStateMachine tests
// ------------------------------------------------------------------

fn make_idle_run_fsm() -> AnimationStateMachine {
    let idle = AnimationState::new("idle")
        .with_transition(Transition::new("run", TransitionCondition::Always, 0.25));
    let run = AnimationState::new("run")
        .with_transition(Transition::new("idle", TransitionCondition::Always, 0.25));
    AnimationStateMachine::new("idle", vec![idle, run])
}

#[test]
fn state_machine_new() {
    let fsm = make_idle_run_fsm();
    assert_eq!(fsm.current_state, "idle");
    assert_eq!(fsm.current_time, 0.0);
    assert!(fsm.parameters.is_empty());
    assert!(fsm.triggers.is_empty());
}

#[test]
fn state_machine_update_advances_time() {
    // Use an FSM with no transitions so time advances without state change
    let mut fsm = AnimationStateMachine::new("idle", vec![AnimationState::new("idle")]);
    let (clip, prev, blend) = fsm.update(0.5);
    assert_eq!(clip, "idle");
    assert_eq!(prev, None);
    assert_eq!(fsm.current_time, 0.5);
    assert_eq!(blend, 1.0);
}

#[test]
fn state_machine_transition_on_always() {
    let mut fsm = make_idle_run_fsm();
    let (clip, _prev, _blend) = fsm.update(0.1);
    // The Always condition triggers immediately
    assert_eq!(clip, "run");
    assert_eq!(fsm.current_time, 0.0);
    assert_eq!(fsm.blend_duration, 0.25);
}

#[test]
fn state_machine_set_parameter() {
    let mut fsm = AnimationStateMachine::new("idle", vec![AnimationState::new("idle")]);
    fsm.set_parameter("speed", 5.0);
    assert_eq!(fsm.parameters.get("speed"), Some(&5.0));
}

#[test]
fn state_machine_set_bool_parameter() {
    let mut fsm = AnimationStateMachine::new("idle", vec![AnimationState::new("idle")]);
    fsm.set_bool_parameter("grounded", true);
    assert_eq!(fsm.bool_parameters.get("grounded"), Some(&true));
}

#[test]
fn state_machine_set_trigger() {
    let mut fsm = AnimationStateMachine::new("idle", vec![AnimationState::new("idle")]);
    fsm.set_trigger("jump");
    assert_eq!(fsm.triggers.get("jump"), Some(&true));
}

#[test]
fn state_machine_blend_weight_increases() {
    // Use an FSM where run has no transition back so we can observe blending
    let idle = AnimationState::new("idle")
        .with_transition(Transition::new("run", TransitionCondition::Always, 0.25));
    let run = AnimationState::new("run");
    let mut fsm = AnimationStateMachine::new("idle", vec![idle, run]);

    let (_clip, _prev, blend0) = fsm.update(0.1); // transitions, blend = 0.0, blend_time = 0.1
    assert_eq!(blend0, 0.0);

    let (_clip, _prev, blend1) = fsm.update(0.15); // blend = 0.1/0.25 = 0.4, blend_time = 0.25
    assert!(blend1 > 0.0 && blend1 < 1.0);

    let (_clip, _prev, blend2) = fsm.update(0.0); // blend = 0.25/0.25 = 1.0
    assert_eq!(blend2, 1.0);
}

#[test]
fn state_machine_time_elapsed_transition() {
    let idle = AnimationState::new("idle")
        .with_transition(Transition::new("run", TransitionCondition::TimeElapsed(0.5), 0.1));
    let mut fsm = AnimationStateMachine::new("idle", vec![idle, AnimationState::new("run")]);

    let (clip1, _, _) = fsm.update(0.3);
    assert_eq!(clip1, "idle"); // not enough time elapsed

    let (clip2, _, _) = fsm.update(0.3);
    assert_eq!(clip2, "run"); // now 0.6 >= 0.5
}

#[test]
fn state_machine_trigger_transition() {
    let idle = AnimationState::new("idle")
        .with_transition(Transition::new("run", TransitionCondition::Trigger("go".to_string()), 0.1));
    let mut fsm = AnimationStateMachine::new("idle", vec![idle, AnimationState::new("run")]);

    let (clip1, _, _) = fsm.update(0.1);
    assert_eq!(clip1, "idle"); // no trigger

    fsm.set_trigger("go");
    let (clip2, _, _) = fsm.update(0.1);
    assert_eq!(clip2, "run"); // trigger consumed
}

#[test]
fn state_machine_parameter_transition() {
    let idle = AnimationState::new("idle")
        .with_transition(Transition::new("run", TransitionCondition::ParameterGte { name: "speed".to_string(), threshold: 5.0 }, 0.1));
    let mut fsm = AnimationStateMachine::new("idle", vec![idle, AnimationState::new("run")]);

    fsm.set_parameter("speed", 3.0);
    let (clip1, _, _) = fsm.update(0.1);
    assert_eq!(clip1, "idle");

    fsm.set_parameter("speed", 7.0);
    let (clip2, _, _) = fsm.update(0.1);
    assert_eq!(clip2, "run");
}

#[test]
fn state_machine_bool_parameter_transition() {
    let idle = AnimationState::new("idle")
        .with_transition(Transition::new("run", TransitionCondition::ParameterBool { name: "grounded".to_string(), value: true }, 0.1));
    let mut fsm = AnimationStateMachine::new("idle", vec![idle, AnimationState::new("run")]);

    fsm.set_bool_parameter("grounded", false);
    let (clip1, _, _) = fsm.update(0.1);
    assert_eq!(clip1, "idle");

    fsm.set_bool_parameter("grounded", true);
    let (clip2, _, _) = fsm.update(0.1);
    assert_eq!(clip2, "run");
}

#[test]
fn state_machine_and_transition() {
    let cond = TransitionCondition::And(
        Box::new(TransitionCondition::ParameterGte { name: "speed".to_string(), threshold: 5.0 }),
        Box::new(TransitionCondition::ParameterBool { name: "grounded".to_string(), value: true }),
    );
    let idle = AnimationState::new("idle")
        .with_transition(Transition::new("run", cond, 0.1));
    let mut fsm = AnimationStateMachine::new("idle", vec![idle, AnimationState::new("run")]);

    fsm.set_parameter("speed", 7.0);
    fsm.set_bool_parameter("grounded", false);
    let (clip1, _, _) = fsm.update(0.1);
    assert_eq!(clip1, "idle"); // only one condition true

    fsm.set_bool_parameter("grounded", true);
    let (clip2, _, _) = fsm.update(0.1);
    assert_eq!(clip2, "run"); // both true
}

#[test]
fn state_machine_unknown_state_no_crash() {
    let mut fsm = AnimationStateMachine::new("idle", vec![AnimationState::new("idle")]);
    fsm.current_state = "ghost".to_string();
    let (clip, _prev, blend) = fsm.update(0.1);
    assert_eq!(clip, "ghost");
    assert_eq!(blend, 1.0);
}

#[test]
fn state_machine_speed_affects_time() {
    let idle = AnimationState::new("idle");
    let mut fsm = AnimationStateMachine::new("idle", vec![idle]);
    let (clip, _, _) = fsm.update(0.5);
    assert_eq!(clip, "idle");
    assert_eq!(fsm.current_time, 0.5);
}
