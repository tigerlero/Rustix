#[cfg(test)]
use super::*;

#[test]
fn aligned_vec_basic() {
    let mut buf = AlignedVec::new(4, 4);
    let a: u32 = 42;
    buf.push_raw(&a as *const u32 as *const u8);
    let b: u32 = 99;
    buf.push_raw(&b as *const u32 as *const u8);
    assert_eq!(buf.len(), 2);
    unsafe {
        let slice = buf.as_slice::<u32>();
        assert_eq!(slice, &[42, 99]);
    }
}

#[test]
fn aligned_vec_grows() {
    let mut buf = AlignedVec::new(4, 4);
    for i in 0..100u32 {
        buf.push_raw(&i as *const u32 as *const u8);
    }
    assert_eq!(buf.len(), 100);
    unsafe {
        let slice = buf.as_slice::<u32>();
        assert_eq!(slice[99], 99);
    }
}

#[test]
fn aligned_vec_alignment() {
    let buf = AlignedVec::new(64, 64);
    assert!(buf.ptr.is_null());
    // After pushing once, ptr should be non-null and aligned
    // (can't test alignment directly without a push)
}

#[test]
fn soa_storage_insert_and_read() {
    let fields = vec![
        SoAField { name: "x", size: 4, align: 4 },
        SoAField { name: "y", size: 4, align: 4 },
    ];
    let mut storage = SoAStorage::new(fields);
    let mut w = hecs::World::new();

    let e = w.spawn(());
    let bytes: [u8; 8] = [0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00];
    storage.insert(e, &bytes);

    let mut out = [0u8; 4];
    storage.read_field(e, 0, &mut out);
    assert_eq!(u32::from_le_bytes(out), 1);

    storage.read_field(e, 1, &mut out);
    assert_eq!(u32::from_le_bytes(out), 2);
}

#[test]
fn soa_storage_update_existing() {
    let fields = vec![SoAField { name: "v", size: 4, align: 4 }];
    let mut storage = SoAStorage::new(fields);
    let mut w = hecs::World::new();

    let e = w.spawn(());
    let bytes1: [u8; 4] = [0x0A, 0x00, 0x00, 0x00];
    storage.insert(e, &bytes1);

    let bytes2: [u8; 4] = [0x14, 0x00, 0x00, 0x00];
    storage.insert(e, &bytes2);

    let mut out = [0u8; 4];
    storage.read_field(e, 0, &mut out);
    assert_eq!(u32::from_le_bytes(out), 20);
    assert_eq!(storage.len(), 1);
}

#[test]
fn soa_storage_remove_swap_compact() {
    let fields = vec![SoAField { name: "v", size: 4, align: 4 }];
    let mut storage = SoAStorage::new(fields);
    let mut w = hecs::World::new();

    let e0 = w.spawn(());
    let e1 = w.spawn(());
    let e2 = w.spawn(());

    storage.insert(e0, &[0x00, 0x00, 0x00, 0x00]);
    storage.insert(e1, &[0x01, 0x00, 0x00, 0x00]);
    storage.insert(e2, &[0x02, 0x00, 0x00, 0x00]);

    assert!(storage.remove(e1));
    assert_eq!(storage.len(), 2);

    // e2 should have been swapped into slot 1
    let mut out = [0u8; 4];
    storage.read_field(e2, 0, &mut out);
    assert_eq!(u32::from_le_bytes(out), 2);
    assert_eq!(storage.slot(e2), Some(1));
}

#[test]
fn soa_storage_field_slice() {
    let fields = vec![SoAField { name: "v", size: 4, align: 4 }];
    let mut storage = SoAStorage::new(fields);
    let mut w = hecs::World::new();

    for i in 0..5u32 {
        let e = w.spawn(());
        let bytes = i.to_le_bytes();
        storage.insert(e, &bytes);
    }

    unsafe {
        let slice = storage.field_slice::<u32>(0);
        assert_eq!(slice.len(), 5);
        assert_eq!(slice[0], 0);
        assert_eq!(slice[4], 4);
    }
}

#[test]
fn soa_storage_entities_iter() {
    let fields = vec![SoAField { name: "v", size: 4, align: 4 }];
    let mut storage = SoAStorage::new(fields);
    let mut w = hecs::World::new();

    let e0 = w.spawn(());
    let e1 = w.spawn(());
    storage.insert(e0, &[0; 4]);
    storage.insert(e1, &[0; 4]);

    let mut ids: Vec<_> = storage.entities().map(|(e, _)| e).collect();
    ids.sort_by_key(|e| e.to_bits().get());
    assert_eq!(ids, vec![e0, e1]);
}

#[test]
fn soa_registry_basic() {
    let mut reg = SoARegistry::new();
    let storage = SoAStorage::new(vec![SoAField { name: "x", size: 4, align: 4 }]);
    reg.register("transform", storage);
    assert!(reg.get("transform").is_some());
    assert!(reg.get("missing").is_none());
}
