//! Tests for memory-mapped file wrapper.

use std::io::Write;
use crate::mmap::MappedFile;

#[test]
fn mapped_file_reads_correct_bytes() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let content = b"hello mmap world";
    tmp.write_all(content).unwrap();
    tmp.flush().unwrap();

    let mf = MappedFile::open(tmp.path()).unwrap();
    assert_eq!(&*mf, content);
}

#[test]
fn small_file_uses_heap() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(b"tiny").unwrap();
    tmp.flush().unwrap();

    let mf = MappedFile::open(tmp.path()).unwrap();
    assert!(mf.heap.is_some(), "small file should use heap");
    assert!(mf.mmap.is_none());
}
