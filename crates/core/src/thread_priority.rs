/// OS-level scheduling policy for real-time threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulingPolicy {
    /// First-in-first-out real-time scheduling.
    Fifo,
    /// Round-robin real-time scheduling.
    RoundRobin,
    /// Default time-sharing scheduler.
    Other,
}

/// Thread priority configuration.
///
/// On Linux:
/// - `Realtime` → `SCHED_FIFO` or `SCHED_RR` with the given priority (1..99).
///   Requires `CAP_SYS_NICE` or root. Falls back to `SCHED_OTHER` on permission denied.
/// - `High` / `Normal` / `Low` → adjusted via `nice()` (or `setpriority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadPriority {
    /// Real-time priority (1..99).  Higher = more urgent.
    /// The default is `Fifo` at priority 50.
    Realtime {
        priority: u8,
        policy: SchedulingPolicy,
    },
    /// High non-realtime (nice -10 on Linux).
    High,
    /// Normal/default.
    Normal,
    /// Low priority background thread (nice +10 on Linux).
    Low,
}

impl Default for ThreadPriority {
    fn default() -> Self { Self::Normal }
}

/// Set the current thread's OS-level scheduling priority.
///
/// # Platform-specific
/// - **Linux:** Uses `pthread_setschedparam` for real-time policies and
///   `setpriority(PRIO_PROCESS, 0, nice)` for `High`/`Normal`/`Low`.
///   Real-time policies require `CAP_SYS_NICE` or root; falls back to
///   `Normal` with a warning if permission is denied.
/// - **macOS / Windows:** No-op (returns `Ok(())`).
/// - **Other:** No-op.
pub fn set_current_thread_priority(p: ThreadPriority) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        match p {
            ThreadPriority::Realtime { priority, policy } => {
                let policy_val = match policy {
                    SchedulingPolicy::Fifo => libc::SCHED_FIFO,
                    SchedulingPolicy::RoundRobin => libc::SCHED_RR,
                    SchedulingPolicy::Other => libc::SCHED_OTHER,
                };
                let prio = priority.clamp(1, 99) as i32;
                let param = libc::sched_param { sched_priority: prio };
                let result = unsafe {
                    libc::pthread_setschedparam(libc::pthread_self(), policy_val, &param)
                };
                if result == 0 {
                    Ok(())
                } else {
                    let err = std::io::Error::last_os_error();
                    // EPERM (1) means no permission for real-time scheduling.
                    if err.raw_os_error() == Some(libc::EPERM) {
                        tracing::warn!(
                            "real-time thread priority requires CAP_SYS_NICE or root; falling back to Normal"
                        );
                        set_current_thread_priority(ThreadPriority::Normal)
                    } else {
                        Err(format!("pthread_setschedparam failed: {err}"))
                    }
                }
            }
            ThreadPriority::High => {
                let nice = -10;
                let result = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) };
                if result == 0 {
                    Ok(())
                } else {
                    Err(format!("setpriority failed: {}", std::io::Error::last_os_error()))
                }
            }
            ThreadPriority::Normal => Ok(()),
            ThreadPriority::Low => {
                let nice = 10;
                let result = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) };
                if result == 0 {
                    Ok(())
                } else {
                    Err(format!("setpriority failed: {}", std::io::Error::last_os_error()))
                }
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = p;
        Ok(())
    }
}
