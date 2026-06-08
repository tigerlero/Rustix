//! Tests for asset handles and type IDs.

use crate::handle::{AssetTypeId, Handle, UntypedHandle, Asset};

struct DummyAsset;
impl Asset for DummyAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("dummy")
    }
}

#[test]
fn asset_type_id_from_crate_name_deterministic() {
    let id1 = AssetTypeId::from_crate_name("mesh");
    let id2 = AssetTypeId::from_crate_name("mesh");
    assert_eq!(id1, id2);
}

#[test]
fn asset_type_id_different_names() {
    let id1 = AssetTypeId::from_crate_name("mesh");
    let id2 = AssetTypeId::from_crate_name("texture");
    assert_ne!(id1, id2);
}

#[test]
fn handle_new() {
    let h: Handle<DummyAsset> = Handle::new(5, 1);
    assert_eq!(h.index(), 5);
    assert_eq!(h.generation(), 1);
}

#[test]
fn handle_copy() {
    let h: Handle<DummyAsset> = Handle::new(5, 1);
    let h2 = h;
    assert_eq!(h.index(), h2.index());
}

#[test]
fn handle_erase_and_typed_roundtrip() {
    let h: Handle<DummyAsset> = Handle::new(5, 1);
    let erased = h.erase();
    assert_eq!(erased.index, 5);
    assert_eq!(erased.generation, 1);
    let typed: Handle<DummyAsset> = erased.typed();
    assert_eq!(typed.index(), 5);
    assert_eq!(typed.generation(), 1);
}

#[test]
fn untyped_handle_new() {
    let h = UntypedHandle::new(10, 2);
    assert_eq!(h.index, 10);
    assert_eq!(h.generation, 2);
}

#[test]
fn handle_debug_includes_type_name() {
    let h: Handle<DummyAsset> = Handle::new(0, 0);
    let s = format!("{:?}", h);
    assert!(s.contains("DummyAsset"));
    assert!(s.contains("i=0"));
}
