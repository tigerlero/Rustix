//! Tests for platform thread priority.

use crate::thread_priority::{SchedulingPolicy, ThreadPriority, set_current_thread_priority, set_current_thread_name};

#[test]
fn scheduling_policy_variants() {
    assert_ne!(SchedulingPolicy::Fifo, SchedulingPolicy::RoundRobin);
    assert_ne!(SchedulingPolicy::Fifo, SchedulingPolicy::Other);
    assert_ne!(SchedulingPolicy::RoundRobin, SchedulingPolicy::Other);
}

#[test]
fn thread_priority_default_is_normal() {
    assert_eq!(ThreadPriority::default(), ThreadPriority::Normal);
}

#[test]
fn thread_priority_variants() {
    let rt = ThreadPriority::Realtime { priority: 50, policy: SchedulingPolicy::Fifo };
    assert_ne!(rt, ThreadPriority::Normal);
    assert_ne!(ThreadPriority::High, ThreadPriority::Low);
}

#[test]
fn set_current_thread_priority_normal_ok() {
    // Normal is a no-op on all platforms and should always succeed
    assert!(set_current_thread_priority(ThreadPriority::Normal).is_ok());
}

#[test]
fn set_current_thread_name_ok() {
    // Short name should work on all platforms
    assert!(set_current_thread_name("test").is_ok());
}

#[test]
fn serde_roundtrip_scheduling_policy() {
    let policies = vec![
        SchedulingPolicy::Fifo,
        SchedulingPolicy::RoundRobin,
        SchedulingPolicy::Other,
    ];
    for policy in policies {
        let json = serde_json::to_string(&policy).unwrap();
        let back: SchedulingPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, back);
    }
}

#[test]
fn serde_roundtrip_thread_priority() {
    let priorities = vec![
        ThreadPriority::Realtime { priority: 50, policy: SchedulingPolicy::Fifo },
        ThreadPriority::Realtime { priority: 10, policy: SchedulingPolicy::RoundRobin },
        ThreadPriority::High,
        ThreadPriority::Normal,
        ThreadPriority::Low,
    ];
    for priority in priorities {
        let json = serde_json::to_string(&priority).unwrap();
        let back: ThreadPriority = serde_json::from_str(&json).unwrap();
        assert_eq!(priority, back);
    }
}
