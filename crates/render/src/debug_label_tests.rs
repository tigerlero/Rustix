//! Tests for Vulkan debug label stubs.

use std::mem::MaybeUninit;
use crate::debug_label::{label_object, begin_label, end_label};

#[test]
fn label_object_stub_does_not_panic() {
    // label_object is unsafe and takes a Device reference.
    // As a stub it does nothing, so a zeroed Device is fine.
    unsafe {
        let mut device_uninit = MaybeUninit::<ash::Device>::zeroed();
        let device = device_uninit.assume_init_ref();
        label_object(
            device,
            ash::vk::ObjectType::BUFFER,
            0,
            "test_buffer",
        );
    }
}

#[test]
fn begin_label_stub_does_not_panic() {
    unsafe {
        begin_label(ash::vk::CommandBuffer::null(), "test_region", [1.0, 0.0, 0.0, 1.0]);
    }
}

#[test]
fn end_label_stub_does_not_panic() {
    unsafe {
        end_label(ash::vk::CommandBuffer::null());
    }
}
