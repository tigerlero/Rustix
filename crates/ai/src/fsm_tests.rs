//! Tests for finite state machine.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::fsm::{Fsm, State};

#[derive(Clone, Default)]
struct Ctx {
    update_count: Arc<AtomicUsize>,
    enter_count: Arc<AtomicUsize>,
    exit_count: Arc<AtomicUsize>,
    health: f32,
}

#[test]
fn fsm_starts_in_no_state() {
    let fsm: Fsm<Ctx> = Fsm::new();
    assert!(fsm.current_state().is_none());
}

#[test]
fn fsm_set_initial_state() {
    let mut fsm: Fsm<Ctx> = Fsm::new();
    fsm.set_initial("idle");
    assert!(fsm.is_in_state("idle"));
}

#[test]
fn fsm_tick_calls_update() {
    let mut fsm: Fsm<Ctx> = Fsm::new();
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    let state = State::new("idle", move |_ctx: &mut Ctx, _dt: f32| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    fsm.add_state(state);
    fsm.set_initial("idle");
    fsm.tick(&mut Ctx::default(), 0.016);

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn fsm_transition_fires_on_condition() {
    let mut fsm: Fsm<Ctx> = Fsm::new();

    let idle = State::new("idle", |_ctx: &mut Ctx, _dt: f32| {});
    let run = State::new("run", |_ctx: &mut Ctx, _dt: f32| {});

    fsm.add_state(idle);
    fsm.add_state(run);
    fsm.add_transition("idle", "run", |_ctx: &Ctx| true);
    fsm.set_initial("idle");

    fsm.tick(&mut Ctx::default(), 0.016);
    assert!(fsm.is_in_state("run"));
}

#[test]
fn fsm_transition_respects_condition() {
    let mut fsm: Fsm<Ctx> = Fsm::new();

    let idle = State::new("idle", |_ctx: &mut Ctx, _dt: f32| {});
    let run = State::new("run", |_ctx: &mut Ctx, _dt: f32| {});

    fsm.add_state(idle);
    fsm.add_state(run);
    fsm.add_transition("idle", "run", |_ctx: &Ctx| false);
    fsm.set_initial("idle");

    fsm.tick(&mut Ctx::default(), 0.016);
    assert!(fsm.is_in_state("idle"), "transition should not fire when condition is false");
}

#[test]
fn fsm_on_enter_called_on_transition() {
    let mut fsm: Fsm<Ctx> = Fsm::new();
    let enter_counter = Arc::new(AtomicUsize::new(0));

    let ec = enter_counter.clone();
    let run = State::new("run", |_ctx: &mut Ctx, _dt: f32| {})
        .on_enter(move |_ctx: &mut Ctx| {
            ec.fetch_add(1, Ordering::SeqCst);
        });

    let idle = State::new("idle", |_ctx: &mut Ctx, _dt: f32| {});

    fsm.add_state(idle);
    fsm.add_state(run);
    fsm.add_transition("idle", "run", |_ctx: &Ctx| true);
    fsm.set_initial("idle");

    fsm.tick(&mut Ctx::default(), 0.016);
    assert_eq!(enter_counter.load(Ordering::SeqCst), 1, "on_enter should fire when entering run");
}

#[test]
fn fsm_on_exit_called_on_transition() {
    let mut fsm: Fsm<Ctx> = Fsm::new();
    let exit_counter = Arc::new(AtomicUsize::new(0));

    let xc = exit_counter.clone();
    let idle = State::new("idle", |_ctx: &mut Ctx, _dt: f32| {})
        .on_exit(move |_ctx: &mut Ctx| {
            xc.fetch_add(1, Ordering::SeqCst);
        });
    let run = State::new("run", |_ctx: &mut Ctx, _dt: f32| {});

    fsm.add_state(idle);
    fsm.add_state(run);
    fsm.add_transition("idle", "run", |_ctx: &Ctx| true);
    fsm.set_initial("idle");

    fsm.tick(&mut Ctx::default(), 0.016);
    assert_eq!(exit_counter.load(Ordering::SeqCst), 1, "on_exit should fire when leaving idle");
}

#[test]
fn fsm_no_transition_when_same_state() {
    let mut fsm: Fsm<Ctx> = Fsm::new();

    let idle = State::new("idle", |_ctx: &mut Ctx, _dt: f32| {});
    fsm.add_state(idle);
    fsm.add_transition("idle", "idle", |_ctx: &Ctx| true);
    fsm.set_initial("idle");

    let enter_counter = Arc::new(AtomicUsize::new(0));
    let ec = enter_counter.clone();
    // We need a fresh FSM because the state is already added without on_enter
    let mut fsm2: Fsm<Ctx> = Fsm::new();
    let idle2 = State::new("idle", |_ctx: &mut Ctx, _dt: f32| {})
        .on_enter(move |_ctx: &mut Ctx| {
            ec.fetch_add(1, Ordering::SeqCst);
        });
    fsm2.add_state(idle2);
    fsm2.add_transition("idle", "idle", |_ctx: &Ctx| true);
    fsm2.set_initial("idle");

    fsm2.tick(&mut Ctx::default(), 0.016);
    assert_eq!(enter_counter.load(Ordering::SeqCst), 0, "on_enter should not fire when transitioning to same state");
}

#[test]
fn fsm_multiple_ticks_accumulate() {
    let mut fsm: Fsm<Ctx> = Fsm::new();
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    let state = State::new("idle", move |_ctx: &mut Ctx, _dt: f32| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    fsm.add_state(state);
    fsm.set_initial("idle");

    for _ in 0..5 {
        fsm.tick(&mut Ctx::default(), 0.016);
    }

    assert_eq!(counter.load(Ordering::SeqCst), 5);
}

#[test]
fn fsm_context_passed_to_callbacks() {
    let mut fsm: Fsm<Ctx> = Fsm::new();

    let idle = State::new("idle", |ctx: &mut Ctx, _dt: f32| {
        ctx.health += 1.0;
    });

    fsm.add_state(idle);
    fsm.set_initial("idle");

    let mut ctx = Ctx::default();
    fsm.tick(&mut ctx, 0.016);
    assert_eq!(ctx.health, 1.0, "context should be mutated by update callback");
}

#[test]
fn fsm_default_is_new() {
    let fsm: Fsm<Ctx> = Fsm::default();
    assert!(fsm.current_state().is_none());
}
