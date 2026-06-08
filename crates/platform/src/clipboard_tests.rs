//! Tests for clipboard functions.

use crate::clipboard;

#[test]
fn clipboard_get_text_returns_some_or_none() {
    // May return None in headless environments; just ensure it doesn't panic.
    let _ = clipboard::get_text();
}

#[test]
fn clipboard_set_text_ok_or_err() {
    // May fail in headless environments; just ensure it doesn't panic.
    let _ = clipboard::set_text("hello");
}

#[test]
fn clipboard_clear_ok_or_err() {
    let _ = clipboard::clear();
}
