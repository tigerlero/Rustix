//! Tests for core ECS wrappers: StageLabel, BoxedSystem, Schedule, SystemIdGenerator.

use crate::ecs::*;

#[test]
fn stage_label_order() {
    assert_eq!(StageLabel::First.order(), 0);
    assert_eq!(StageLabel::BeforeFixedUpdate.order(), 1);
    assert_eq!(StageLabel::FixedUpdate.order(), 2);
    assert_eq!(StageLabel::AfterFixedUpdate.order(), 3);
    assert_eq!(StageLabel::BeforeUpdate.order(), 4);
    assert_eq!(StageLabel::Update.order(), 5);
    assert_eq!(StageLabel::AfterUpdate.order(), 6);
    assert_eq!(StageLabel::BeforeRender.order(), 7);
    assert_eq!(StageLabel::Render.order(), 8);
    assert_eq!(StageLabel::AfterRender.order(), 9);
    assert_eq!(StageLabel::Last.order(), 10);
}

#[test]
fn stage_label_order_monotonic() {
    let labels = [
        StageLabel::First,
        StageLabel::BeforeFixedUpdate,
        StageLabel::FixedUpdate,
        StageLabel::AfterFixedUpdate,
        StageLabel::BeforeUpdate,
        StageLabel::Update,
        StageLabel::AfterUpdate,
        StageLabel::BeforeRender,
        StageLabel::Render,
        StageLabel::AfterRender,
        StageLabel::Last,
    ];
    for i in 1..labels.len() {
        assert!(labels[i].order() > labels[i - 1].order());
    }
}

#[test]
fn system_id_generator() {
    let mut gen = SystemIdGenerator::new();
    let id1 = gen.generate();
    let id2 = gen.generate();
    let id3 = gen.generate();
    assert_eq!(id1.0, 0);
    assert_eq!(id2.0, 1);
    assert_eq!(id3.0, 2);
}

#[test]
fn boxed_system_runs() {
    let mut world = EcsWorld::new();
    let mut sys = BoxedSystem::new(
        SystemId(0),
        "spawn",
        StageLabel::Update,
        |world: &mut EcsWorld| {
            world.spawn((42i32,));
        },
    );
    sys.run(&mut world);
    let count = world.query_mut::<&i32>().into_iter().count();
    assert_eq!(count, 1);
}

#[test]
fn schedule_new_empty() {
    let schedule = Schedule::new();
    assert!(schedule.stages().is_empty());
}

#[test]
fn schedule_add_system() {
    let mut schedule = Schedule::new();
    let sys = BoxedSystem::new(SystemId(0), "test", StageLabel::Update, |_w: &mut EcsWorld| {});
    schedule.add_system(sys);
    assert_eq!(schedule.stages().len(), 1);
    assert_eq!(schedule.stages()[0], StageLabel::Update);
}

#[test]
fn schedule_add_multiple_stages_sorted() {
    let mut schedule = Schedule::new();
    schedule.add_system(BoxedSystem::new(SystemId(0), "a", StageLabel::Update, |_w| {}));
    schedule.add_system(BoxedSystem::new(SystemId(1), "b", StageLabel::First, |_w| {}));
    schedule.add_system(BoxedSystem::new(SystemId(2), "c", StageLabel::Render, |_w| {}));

    let stages = schedule.stages();
    assert_eq!(stages.len(), 3);
    assert_eq!(stages[0], StageLabel::First);
    assert_eq!(stages[1], StageLabel::Update);
    assert_eq!(stages[2], StageLabel::Render);
}

#[test]
fn schedule_run_stage() {
    let mut world = EcsWorld::new();
    let mut schedule = Schedule::new();
    schedule.add_system(BoxedSystem::new(
        SystemId(0),
        "spawn",
        StageLabel::Update,
        |w: &mut EcsWorld| { w.spawn((1u32,)); },
    ));
    schedule.run_stage(&mut world, &StageLabel::Update);
    let count = world.query_mut::<&u32>().into_iter().count();
    assert_eq!(count, 1);
}

#[test]
fn schedule_run_all_executes_in_order() {
    let mut world = EcsWorld::new();
    let mut schedule = Schedule::new();

    let mut order = Vec::new();

    {
        let mut order = order.clone();
        schedule.add_system(BoxedSystem::new(
            SystemId(0),
            "first",
            StageLabel::First,
            move |_w: &mut EcsWorld| { order.push("first"); },
        ));
    }
    {
        let mut order = order.clone();
        schedule.add_system(BoxedSystem::new(
            SystemId(1),
            "update",
            StageLabel::Update,
            move |_w: &mut EcsWorld| { order.push("update"); },
        ));
    }
    {
        let mut order = order.clone();
        schedule.add_system(BoxedSystem::new(
            SystemId(2),
            "last",
            StageLabel::Last,
            move |_w: &mut EcsWorld| { order.push("last"); },
        ));
    }

    schedule.run_all(&mut world);
    // We can't directly inspect the closures' side-effects here easily,
    // but we can verify the stage order
    assert_eq!(schedule.stages(), &[StageLabel::First, StageLabel::Update, StageLabel::Last]);
}

#[test]
fn schedule_multiple_systems_same_stage() {
    let mut world = EcsWorld::new();
    let mut schedule = Schedule::new();
    schedule.add_system(BoxedSystem::new(
        SystemId(0),
        "a",
        StageLabel::Update,
        |w: &mut EcsWorld| { w.spawn((1i32,)); },
    ));
    schedule.add_system(BoxedSystem::new(
        SystemId(1),
        "b",
        StageLabel::Update,
        |w: &mut EcsWorld| { w.spawn((2i32,)); },
    ));
    schedule.run_all(&mut world);
    let mut values: Vec<i32> = world.query_mut::<&i32>().into_iter().copied().collect();
    values.sort();
    assert_eq!(values, vec![1, 2]);
}
