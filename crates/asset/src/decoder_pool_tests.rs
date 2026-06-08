//! Tests for asset decoder pool.

use crate::decoder_pool::AssetDecoderPool;
use crate::importer::{Importer, ImportResult};
use crate::handle::{Asset, AssetTypeId};

#[derive(Clone, Debug, PartialEq)]
struct DummyAsset(Vec<u8>);

impl Asset for DummyAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("dummy")
    }
}

#[derive(Clone)]
struct DummyImporter;

impl Importer for DummyImporter {
    type Asset = DummyAsset;

    fn name(&self) -> &'static str { "dummy" }
    fn extensions(&self) -> &[&'static str] { &["bin"] }

    fn import<'a>(
        &self,
        bytes: &'a [u8],
        _hint: Option<&str>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(async move { Ok(DummyAsset(bytes.to_vec())) })
    }
}

#[test]
fn decoder_pool_new() {
    let pool = AssetDecoderPool::new(1);
    assert_eq!(pool.thread_count(), 1);
}

#[test]
fn decoder_pool_poll_empty() {
    let pool = AssetDecoderPool::new(1);
    let completed = pool.poll_completed();
    assert!(completed.is_empty());
}

#[test]
fn decoder_pool_submit_and_poll() {
    let pool = AssetDecoderPool::new(1);
    pool.submit_import(DummyImporter, vec![1, 2, 3], None, std::path::PathBuf::from("test.bin"));
    pool.wait_for_all();
    let completed = pool.poll_completed();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].path, std::path::PathBuf::from("test.bin"));
    assert!(completed[0].error.is_none());
}

#[test]
fn decoder_pool_submit_error() {
    struct FailingImporter;
    impl Importer for FailingImporter {
        type Asset = DummyAsset;
        fn name(&self) -> &'static str { "failing" }
        fn extensions(&self) -> &[&'static str] { &[] }
        fn import<'a>(
            &self,
            _bytes: &'a [u8],
            _hint: Option<&str>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
            Box::pin(async move { Err("fail".to_string()) })
        }
    }

    let pool = AssetDecoderPool::new(1);
    pool.submit_import(FailingImporter, vec![], None, std::path::PathBuf::from("fail.bin"));
    pool.wait_for_all();
    let completed = pool.poll_completed();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].error, Some("fail".to_string()));
}
