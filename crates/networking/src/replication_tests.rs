//! Tests for replication, authority, and bandwidth optimization.

use crate::replication::*;
use crate::authority::*;
use crate::bandwidth::*;
use crate::ClientId;

fn dummy_entity() -> rustix_core::ecs::Entity {
    let mut world = rustix_core::ecs::EcsWorld::new();
    world.spawn(())
}

// ---------- replication.rs ----------

#[test]
fn replication_tracker_new_and_empty() {
    let rt = ReplicationTracker::new();
    assert!(rt.is_empty());
}

#[test]
fn replication_tracker_spawn_and_despawn() {
    let mut rt = ReplicationTracker::new();
    rt.spawn(NetworkId(1));
    rt.despawn(NetworkId(2));
    assert!(!rt.is_empty());
    let msgs = rt.into_messages().unwrap();
    assert_eq!(msgs.len(), 2);
}

#[test]
fn replication_tracker_update_and_remove() {
    let mut rt = ReplicationTracker::new();
    rt.update(NetworkId(1), "Health", vec![100]);
    rt.remove(NetworkId(1), "Armor");
    let msgs = rt.into_messages().unwrap();
    assert_eq!(msgs.len(), 2);
    assert!(matches!(&msgs[0], ReplicationMessage::Update(_)));
    assert!(matches!(&msgs[1], ReplicationMessage::Remove(_)));
}

#[test]
fn replication_tracker_clear() {
    let mut rt = ReplicationTracker::new();
    rt.spawn(NetworkId(1));
    rt.clear();
    assert!(rt.is_empty());
}

#[test]
fn replication_tracker_into_messages_empty() {
    let rt = ReplicationTracker::new();
    assert!(rt.into_messages().is_none());
}

#[test]
fn network_entity_map_insert_and_get() {
    let mut map = NetworkEntityMap::new();
    let local = dummy_entity();
    map.insert(NetworkId(10), local);
    assert_eq!(map.get_local(NetworkId(10)), Some(local));
    assert_eq!(map.get_network(local), Some(NetworkId(10)));
}

#[test]
fn network_entity_map_remove_by_network() {
    let mut map = NetworkEntityMap::new();
    let local = dummy_entity();
    map.insert(NetworkId(10), local);
    assert_eq!(map.remove_by_network(NetworkId(10)), Some(local));
    assert!(map.get_local(NetworkId(10)).is_none());
    assert!(map.get_network(local).is_none());
}

#[test]
fn network_entity_map_remove_by_local() {
    let mut map = NetworkEntityMap::new();
    let local = dummy_entity();
    map.insert(NetworkId(20), local);
    assert_eq!(map.remove_by_local(local), Some(NetworkId(20)));
    assert!(map.get_network(local).is_none());
}

#[test]
fn network_entity_map_clear() {
    let mut map = NetworkEntityMap::new();
    map.insert(NetworkId(1), dummy_entity());
    map.clear();
    assert!(map.network_to_local.is_empty());
    assert!(map.local_to_network.is_empty());
}

#[test]
fn batch_messages_single() {
    let msgs = vec![ReplicationMessage::Despawn(NetworkId(1))];
    let batched = batch_messages(msgs);
    assert!(matches!(batched, ReplicationMessage::Despawn(_)));
}

#[test]
fn batch_messages_multiple() {
    let msgs = vec![
        ReplicationMessage::Despawn(NetworkId(1)),
        ReplicationMessage::Despawn(NetworkId(2)),
    ];
    let batched = batch_messages(msgs);
    assert!(matches!(batched, ReplicationMessage::Batch(_)));
}

// ---------- authority.rs ----------

#[test]
fn authority_manager_register_and_get() {
    let mut am = AuthorityManager::new();
    am.register(NetworkId(1), Authority::Server);
    assert_eq!(am.get(NetworkId(1)), Some(Authority::Server));
    assert!(am.is_server_authoritative(NetworkId(1)));
}

#[test]
fn authority_manager_register_client() {
    let mut am = AuthorityManager::new();
    let client = ClientId(42);
    am.register(NetworkId(1), Authority::Client(client));
    assert!(am.can_client_update(client, NetworkId(1)));
    assert!(!am.can_client_update(ClientId(99), NetworkId(1)));
    assert_eq!(am.predicted_by(client).unwrap().len(), 1);
}

#[test]
fn authority_manager_interpolated() {
    let mut am = AuthorityManager::new();
    am.register(NetworkId(1), Authority::Interpolated);
    assert!(am.is_interpolated(NetworkId(1)));
    assert!(!am.is_server_authoritative(NetworkId(1)));
}

#[test]
fn authority_manager_unregister() {
    let mut am = AuthorityManager::new();
    am.register(NetworkId(1), Authority::Server);
    am.unregister(NetworkId(1));
    assert!(am.get(NetworkId(1)).is_none());
    assert!(!am.is_server_authoritative(NetworkId(1)));
}

#[test]
fn authority_manager_transfer() {
    let mut am = AuthorityManager::new();
    am.register(NetworkId(1), Authority::Server);
    let transfer = am.transfer(NetworkId(1), Authority::Client(ClientId(7)));
    assert!(transfer.is_some());
    assert_eq!(am.get(NetworkId(1)), Some(Authority::Client(ClientId(7))));
    assert!(am.transfer(NetworkId(1), Authority::Client(ClientId(7))).is_none());
}

#[test]
fn client_authority_manager_local_predicted() {
    let mut cam = ClientAuthorityManager::new(ClientId(5));
    cam.set_authority(NetworkId(1), Authority::Client(ClientId(5)));
    cam.set_authority(NetworkId(2), Authority::Server);
    assert!(cam.is_local_predicted(NetworkId(1)));
    assert!(!cam.is_local_predicted(NetworkId(2)));
    assert_eq!(cam.local_predicted_entities(), vec![NetworkId(1)]);
}

#[test]
fn client_authority_manager_interpolated() {
    let mut cam = ClientAuthorityManager::new(ClientId(5));
    cam.set_authority(NetworkId(1), Authority::Interpolated);
    assert!(cam.is_interpolated(NetworkId(1)));
    assert_eq!(cam.interpolated_entities(), vec![NetworkId(1)]);
}

#[test]
fn client_authority_manager_apply_transfer() {
    let mut cam = ClientAuthorityManager::new(ClientId(5));
    cam.apply_transfer(&AuthorityTransfer {
        network_id: NetworkId(1),
        new_authority: Authority::Client(ClientId(5)),
    });
    assert!(cam.is_local_predicted(NetworkId(1)));
}

#[test]
fn client_authority_manager_remove() {
    let mut cam = ClientAuthorityManager::new(ClientId(5));
    cam.set_authority(NetworkId(1), Authority::Server);
    cam.remove(NetworkId(1));
    assert!(cam.get(NetworkId(1)).is_none());
}

// ---------- bandwidth.rs ----------

#[test]
fn delta_compressor_new_and_empty() {
    let dc = DeltaCompressor::new();
    assert!(dc.is_empty());
    assert_eq!(dc.len(), 0);
}

#[test]
fn delta_compressor_is_changed_first_time() {
    let dc = DeltaCompressor::new();
    let changed = dc.is_changed(ClientId(1), NetworkId(10), "Pos", &[1.0f32.to_ne_bytes()[0]; 4]);
    assert!(changed);
}

#[test]
fn delta_compressor_record_and_compare() {
    let mut dc = DeltaCompressor::new();
    let payload = vec![1, 2, 3, 4];
    dc.record_sent(ClientId(1), NetworkId(10), "Pos", &payload);
    assert!(!dc.is_changed(ClientId(1), NetworkId(10), "Pos", &payload));
    let changed = dc.is_changed(ClientId(1), NetworkId(10), "Pos", &[5, 6, 7, 8]);
    assert!(changed);
}

#[test]
fn delta_compressor_filter_updates() {
    let mut dc = DeltaCompressor::new();
    dc.record_sent(ClientId(1), NetworkId(10), "Pos", &[1, 2, 3, 4]);
    let updates = vec![
        ComponentUpdate { network_id: NetworkId(10), component_name: "Pos".into(), payload: vec![1, 2, 3, 4] },
        ComponentUpdate { network_id: NetworkId(10), component_name: "Pos".into(), payload: vec![5, 6, 7, 8] },
    ];
    let filtered = dc.filter_updates(ClientId(1), updates);
    assert_eq!(filtered.len(), 1);
}

#[test]
fn delta_compressor_unregister_client() {
    let mut dc = DeltaCompressor::new();
    dc.record_sent(ClientId(1), NetworkId(10), "Pos", &[1, 2, 3, 4]);
    dc.unregister_client(ClientId(1));
    assert!(dc.is_empty());
}

#[test]
fn delta_compressor_unregister_entity() {
    let mut dc = DeltaCompressor::new();
    dc.record_sent(ClientId(1), NetworkId(10), "Pos", &[1, 2, 3, 4]);
    dc.unregister_entity(NetworkId(10));
    assert!(dc.is_empty());
}

#[test]
fn interest_criteria_default() {
    let ic = InterestCriteria::default();
    assert!(ic.max_distance_sq.is_infinite());
    assert!(ic.include_server_authoritative);
    assert!(ic.always_include_own);
}

#[test]
fn interest_manager_new() {
    let im = InterestManager::new();
    assert_eq!(im.client_count(), 0);
}

#[test]
fn interest_manager_register_and_unregister_client() {
    let mut im = InterestManager::new();
    im.register_client(ClientId(1));
    assert_eq!(im.client_count(), 1);
    im.unregister_client(ClientId(1));
    assert_eq!(im.client_count(), 0);
}

#[test]
fn interest_manager_update_interest_set() {
    let mut im = InterestManager::new();
    im.set_criteria(ClientId(1), InterestCriteria { max_distance_sq: 100.0, ..Default::default() });
    let entities = vec![
        (NetworkId(1), [0.0f32, 0.0, 0.0]),
        (NetworkId(2), [100.0, 0.0, 0.0]),
        (NetworkId(3), [8.0, 0.0, 0.0]),
    ];
    im.update_interest_set(ClientId(1), [0.0, 0.0, 0.0], &entities, &[]);
    assert!(im.is_interested(ClientId(1), NetworkId(1)));
    assert!(!im.is_interested(ClientId(1), NetworkId(2)));
    assert!(im.is_interested(ClientId(1), NetworkId(3)));
}

#[test]
fn interest_manager_always_include_own() {
    let mut im = InterestManager::new();
    let criteria = InterestCriteria { max_distance_sq: 1.0, always_include_own: true, ..Default::default() };
    im.set_criteria(ClientId(1), criteria);
    im.update_interest_set(ClientId(1), [0.0, 0.0, 0.0], &[], &[NetworkId(99)]);
    assert!(im.is_interested(ClientId(1), NetworkId(99)));
}

#[test]
fn interest_manager_filter_messages() {
    let mut im = InterestManager::new();
    im.register_client(ClientId(1));
    im.update_interest_set(ClientId(1), [0.0, 0.0, 0.0], &[(NetworkId(1), [0.0f32, 0.0, 0.0])], &[]);
    let messages = vec![
        ReplicationMessage::Despawn(NetworkId(1)),
        ReplicationMessage::Despawn(NetworkId(2)),
    ];
    let filtered = im.filter_messages(ClientId(1), messages);
    assert_eq!(filtered.len(), 1);
    assert!(matches!(filtered[0], ReplicationMessage::Despawn(NetworkId(1))));
}

#[test]
fn bandwidth_optimizer_new() {
    let mut bo = BandwidthOptimizer::new();
    bo.register_client(ClientId(1));
    bo.interest.update_interest_set(ClientId(1), [0.0, 0.0, 0.0], &[(NetworkId(1), [0.0f32, 0.0, 0.0])], &[]);
    let msgs = vec![ReplicationMessage::Despawn(NetworkId(1))];
    let optimized = bo.optimize_for_client(ClientId(1), msgs);
    assert_eq!(optimized.len(), 1);
}

#[test]
fn bandwidth_optimizer_unregister_client() {
    let mut bo = BandwidthOptimizer::new();
    bo.register_client(ClientId(1));
    bo.unregister_client(ClientId(1));
    assert_eq!(bo.interest.client_count(), 0);
    assert!(bo.delta.is_empty());
}
