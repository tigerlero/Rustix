//! Tests for asset cooking types.

use std::path::PathBuf;
use crate::cook::*;

#[test]
fn cook_kind_from_extension_mesh() {
    assert_eq!(CookKind::from_extension("gltf"), CookKind::Mesh);
    assert_eq!(CookKind::from_extension("glb"), CookKind::Mesh);
    assert_eq!(CookKind::from_extension("obj"), CookKind::Mesh);
    assert_eq!(CookKind::from_extension("fbx"), CookKind::Mesh);
}

#[test]
fn cook_kind_from_extension_texture() {
    assert_eq!(CookKind::from_extension("png"), CookKind::Texture);
    assert_eq!(CookKind::from_extension("jpg"), CookKind::Texture);
    assert_eq!(CookKind::from_extension("jpeg"), CookKind::Texture);
    assert_eq!(CookKind::from_extension("hdr"), CookKind::Texture);
    assert_eq!(CookKind::from_extension("ktx2"), CookKind::Texture);
    assert_eq!(CookKind::from_extension("dds"), CookKind::Texture);
}

#[test]
fn cook_kind_from_extension_material() {
    assert_eq!(CookKind::from_extension("mat.ron"), CookKind::Material);
    assert_eq!(CookKind::from_extension("mat.json"), CookKind::Material);
    assert_eq!(CookKind::from_extension("rxmat"), CookKind::Material);
}

#[test]
fn cook_kind_from_extension_animation() {
    assert_eq!(CookKind::from_extension("anim.ron"), CookKind::Animation);
    assert_eq!(CookKind::from_extension("rxanim"), CookKind::Animation);
}

#[test]
fn cook_kind_from_extension_skeleton() {
    assert_eq!(CookKind::from_extension("skel.ron"), CookKind::Skeleton);
    assert_eq!(CookKind::from_extension("rxskel"), CookKind::Skeleton);
}

#[test]
fn cook_kind_from_extension_generic() {
    assert_eq!(CookKind::from_extension("txt"), CookKind::Generic);
    assert_eq!(CookKind::from_extension("csv"), CookKind::Generic);
}

#[test]
fn cook_kind_cooked_extension() {
    assert_eq!(CookKind::Mesh.cooked_extension(), "rxmesh");
    assert_eq!(CookKind::Material.cooked_extension(), "rxmat");
    assert_eq!(CookKind::Texture.cooked_extension(), "rxtex");
    assert_eq!(CookKind::Animation.cooked_extension(), "rxanim");
    assert_eq!(CookKind::Skeleton.cooked_extension(), "rxskel");
    assert_eq!(CookKind::Generic.cooked_extension(), "rxcooked");
}

#[test]
fn cook_job_clone() {
    let job = CookJob {
        source: PathBuf::from("a.png"),
        output: PathBuf::from("a.rxtex"),
        kind: CookKind::Texture,
    };
    let cloned = job.clone();
    assert_eq!(job.source, cloned.source);
    assert_eq!(job.kind, cloned.kind);
}

#[test]
fn cook_result_clone() {
    let result = CookResult {
        source: PathBuf::from("a.png"),
        output: PathBuf::from("a.rxtex"),
        success: true,
        error: None,
        bytes_written: 128,
    };
    let cloned = result.clone();
    assert_eq!(result.success, cloned.success);
    assert_eq!(result.bytes_written, cloned.bytes_written);
}
