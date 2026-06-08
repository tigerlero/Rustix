//! Tests for job system.

use crate::job::{JobSystem, JobSystemConfig};

#[test]
fn job_system_rebuild_changes_thread_count() {
    let mut sys = JobSystem::new(&JobSystemConfig {
        thread_count: Some(2),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(sys.thread_count(), 2);

    sys.rebuild(&JobSystemConfig {
        thread_count: Some(4),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(sys.thread_count(), 4);
}
