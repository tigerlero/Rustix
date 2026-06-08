//! Tests for client prediction, server reconciliation, interpolation, and lag compensation.

use crate::prediction::*;
use crate::interpolation::*;
use crate::lag_compensation::*;

// ---------- prediction.rs ----------

#[test]
fn client_prediction_new() {
    let cp: ClientPrediction<u32> = ClientPrediction::new();
    assert!(cp.pending_inputs.is_empty());
    assert_eq!(cp.last_confirmed_tick, 0);
    assert_eq!(cp.next_tick, 1);
}

#[test]
fn client_prediction_push_input() {
    let mut cp = ClientPrediction::new();
    let tick = cp.push_input(42u32);
    assert_eq!(tick, 1);
    assert_eq!(cp.next_tick, 2);
    assert_eq!(cp.pending_inputs.len(), 1);
}

#[test]
fn client_prediction_acknowledge() {
    let mut cp = ClientPrediction::new();
    cp.push_input(1u32);
    cp.push_input(2u32);
    cp.push_input(3u32);
    cp.acknowledge(2);
    assert_eq!(cp.last_confirmed_tick, 2);
    assert_eq!(cp.pending_inputs.len(), 1);
    assert_eq!(cp.pending_inputs.front().unwrap().tick, 3);
}

#[test]
fn client_prediction_inputs_to_replay() {
    let mut cp = ClientPrediction::new();
    cp.push_input(1u32);
    cp.push_input(2u32);
    cp.acknowledge(1);
    let replay = cp.inputs_to_replay();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].tick, 2);
}

#[test]
fn server_reconciliation_new() {
    let sr: ServerReconciliation<u32> = ServerReconciliation::new();
    assert!(sr.received_inputs.is_empty());
    assert_eq!(sr.last_processed_tick, 0);
}

#[test]
fn server_reconciliation_receive_and_take() {
    let mut sr = ServerReconciliation::new();
    sr.receive_input(3, 30u32);
    sr.receive_input(1, 10u32);
    sr.receive_input(2, 20u32);
    assert_eq!(sr.received_inputs.len(), 3);
    assert_eq!(sr.received_inputs.front().unwrap().tick, 1);

    let taken = sr.take_inputs_up_to(2);
    assert_eq!(taken.len(), 2);
    assert_eq!(taken[0].tick, 1);
    assert_eq!(taken[1].tick, 2);
    assert_eq!(sr.last_processed_tick, 2);
    assert_eq!(sr.received_inputs.len(), 1);
}

// ---------- interpolation.rs ----------

#[test]
fn snapshot_buffer_push_and_max_size() {
    let mut buf = SnapshotBuffer::new(0.1, 3);
    buf.push(Snapshot { tick: 1, timestamp: 0.0, state: InterpPosition { x: 0.0, y: 0.0, z: 0.0 } });
    buf.push(Snapshot { tick: 2, timestamp: 0.05, state: InterpPosition { x: 1.0, y: 0.0, z: 0.0 } });
    buf.push(Snapshot { tick: 3, timestamp: 0.1, state: InterpPosition { x: 2.0, y: 0.0, z: 0.0 } });
    buf.push(Snapshot { tick: 4, timestamp: 0.15, state: InterpPosition { x: 3.0, y: 0.0, z: 0.0 } });
    assert_eq!(buf.snapshots.len(), 3);
    assert_eq!(buf.snapshots.front().unwrap().tick, 2);
}

#[test]
fn snapshot_buffer_ignores_out_of_order() {
    let mut buf = SnapshotBuffer::new(0.1, 10);
    buf.push(Snapshot { tick: 2, timestamp: 0.05, state: InterpPosition { x: 1.0, y: 0.0, z: 0.0 } });
    buf.push(Snapshot { tick: 1, timestamp: 0.0, state: InterpPosition { x: 0.0, y: 0.0, z: 0.0 } });
    assert_eq!(buf.snapshots.len(), 1);
}

#[test]
fn snapshot_buffer_interpolate() {
    let mut buf = SnapshotBuffer::new(0.0, 10);
    buf.push(Snapshot { tick: 1, timestamp: 0.0, state: InterpPosition { x: 0.0, y: 0.0, z: 0.0 } });
    buf.push(Snapshot { tick: 2, timestamp: 1.0, state: InterpPosition { x: 10.0, y: 0.0, z: 0.0 } });
    let result = buf.interpolate(0.5).unwrap();
    assert!(result.x >= 4.0 && result.x <= 6.0);
}

#[test]
fn snapshot_buffer_interpolate_not_enough() {
    let mut buf = SnapshotBuffer::new(0.0, 10);
    buf.push(Snapshot { tick: 1, timestamp: 0.0, state: InterpPosition { x: 5.0, y: 0.0, z: 0.0 } });
    let result = buf.interpolate(0.0).unwrap();
    assert_eq!(result.x, 5.0);
}

#[test]
fn interp_position_interpolate() {
    let a = InterpPosition { x: 0.0, y: 0.0, z: 0.0 };
    let b = InterpPosition { x: 10.0, y: 20.0, z: 30.0 };
    let c = a.interpolate(&b, 0.5);
    assert_eq!(c.x, 5.0);
    assert_eq!(c.y, 10.0);
    assert_eq!(c.z, 15.0);
}

#[test]
fn interp_entity_state_interpolate() {
    let a = InterpEntityState {
        position: InterpPosition { x: 0.0, y: 0.0, z: 0.0 },
        rotation: [0.0, 0.0, 0.0, 1.0],
    };
    let b = InterpEntityState {
        position: InterpPosition { x: 10.0, y: 0.0, z: 0.0 },
        rotation: [0.0, 0.0, 0.0, 1.0],
    };
    let c = a.interpolate(&b, 0.5);
    assert_eq!(c.position.x, 5.0);
}

// ---------- lag_compensation.rs ----------

fn make_frame(tick: u64, timestamp: f64, entity_id: u64, pos: [f32; 3]) -> LagCompFrame {
    LagCompFrame {
        tick,
        timestamp,
        entities: vec![LagCompSnapshot {
            entity_id,
            position: pos,
            rotation: [0.0, 0.0, 0.0, 1.0],
            bounds_radius: 1.0,
        }],
    }
}

#[test]
fn lag_comp_buffer_new() {
    let buf = LagCompensationBuffer::new(60, 20.0);
    assert!(buf.frames.is_empty());
    assert_eq!(buf.max_frames, 60);
}

#[test]
fn lag_comp_buffer_push_and_evict() {
    let mut buf = LagCompensationBuffer::new(2, 20.0);
    buf.push(make_frame(1, 0.0, 1, [0.0, 0.0, 0.0]));
    buf.push(make_frame(2, 0.05, 1, [1.0, 0.0, 0.0]));
    buf.push(make_frame(3, 0.1, 1, [2.0, 0.0, 0.0]));
    assert_eq!(buf.frames.len(), 2);
    assert_eq!(buf.frames.front().unwrap().tick, 2);
}

#[test]
fn lag_comp_buffer_rewind_to_tick() {
    let mut buf = LagCompensationBuffer::new(10, 20.0);
    buf.push(make_frame(1, 0.0, 1, [0.0, 0.0, 0.0]));
    buf.push(make_frame(10, 0.5, 1, [10.0, 0.0, 0.0]));
    let frame = buf.rewind_to_tick(7).unwrap();
    assert_eq!(frame.tick, 10); // nearest to 7
}

#[test]
fn lag_comp_buffer_rewind_to_time() {
    let mut buf = LagCompensationBuffer::new(10, 20.0);
    buf.push(make_frame(1, 0.0, 1, [0.0, 0.0, 0.0]));
    buf.push(make_frame(10, 0.5, 1, [10.0, 0.0, 0.0]));
    let frame = buf.rewind_to_time(0.45).unwrap();
    assert_eq!(frame.tick, 10); // nearest to 0.45
}

#[test]
fn lag_comp_buffer_rewind_and_interpolate() {
    let mut buf = LagCompensationBuffer::new(10, 20.0);
    buf.push(make_frame(1, 0.0, 1, [0.0, 0.0, 0.0]));
    buf.push(make_frame(3, 0.1, 1, [10.0, 0.0, 0.0]));
    let ents = buf.rewind_and_interpolate(2);
    assert_eq!(ents.len(), 1);
    assert!(ents[0].position[0] >= 4.0 && ents[0].position[0] <= 6.0);
}

#[test]
fn lag_comp_buffer_clear() {
    let mut buf = LagCompensationBuffer::new(10, 20.0);
    buf.push(make_frame(1, 0.0, 1, [0.0, 0.0, 0.0]));
    buf.clear();
    assert!(buf.frames.is_empty());
}

#[test]
fn lag_comp_buffer_buffer_age() {
    let mut buf = LagCompensationBuffer::new(10, 20.0);
    assert_eq!(buf.buffer_age(), 0.0);
    buf.push(make_frame(1, 0.0, 1, [0.0, 0.0, 0.0]));
    buf.push(make_frame(2, 0.5, 1, [0.0, 0.0, 0.0]));
    assert_eq!(buf.buffer_age(), 0.5);
}

#[test]
fn lag_comp_buffer_latency_from_rtt() {
    assert_eq!(LagCompensationBuffer::latency_from_rtt(100.0), 0.05);
}

#[test]
fn lag_compensated_raycast_hit() {
    let entities = vec![LagCompSnapshot {
        entity_id: 1,
        position: [5.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        bounds_radius: 2.0,
    }];
    let hit = lag_compensated_raycast([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 100.0, &entities);
    assert!(hit.is_some());
    assert_eq!(hit.unwrap().entity_id, 1);
}

#[test]
fn lag_compensated_raycast_miss() {
    let entities = vec![LagCompSnapshot {
        entity_id: 1,
        position: [5.0, 5.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        bounds_radius: 0.5,
    }];
    let hit = lag_compensated_raycast([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 100.0, &entities);
    assert!(hit.is_none());
}

#[test]
fn lag_compensated_raycast_zero_direction() {
    let entities = vec![LagCompSnapshot {
        entity_id: 1,
        position: [5.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        bounds_radius: 2.0,
    }];
    let hit = lag_compensated_raycast([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 100.0, &entities);
    assert!(hit.is_none());
}
