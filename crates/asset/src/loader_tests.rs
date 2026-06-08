//! Tests for async asset loader.

use std::future::Future;
use std::pin::Pin;
use std::io::Write;

use crate::handle::{Asset, AssetTypeId};
use crate::importer::Importer;
use crate::loader::AssetLoader;

#[derive(Debug, Clone, PartialEq)]
struct DummyAsset(pub String);

impl Asset for DummyAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("dummy_asset")
    }
}

struct EchoImporter;

impl Importer for EchoImporter {
    type Asset = DummyAsset;

    fn name(&self) -> &'static str {
        "echo"
    }

    fn extensions(&self) -> &[&'static str] {
        &["txt"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = crate::importer::ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(async move {
            let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            Ok(DummyAsset(s.to_string()))
        })
    }
}

fn temp_file_with(contents: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("rustix_loader_test_{}.txt", std::process::id()));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(contents.as_bytes()).unwrap();
    path
}

#[tokio::test]
async fn asset_loader_load_success() {
    let path = temp_file_with("hello world");
    let loader = AssetLoader::new(tokio::runtime::Handle::current());
    let rx = loader.load(&path, EchoImporter);
    let result = rx.await.unwrap();
    assert_eq!(result.unwrap().0, "hello world");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn asset_loader_load_missing_file() {
    let path = std::path::PathBuf::from("/nonexistent/path/to/file.txt");
    let loader = AssetLoader::new(tokio::runtime::Handle::current());
    let rx = loader.load(&path, EchoImporter);
    let result = rx.await.unwrap();
    assert!(result.is_err());
}
