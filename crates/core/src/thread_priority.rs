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
/// - **Windows:** Uses `SetThreadPriority`. `Realtime` → `THREAD_PRIORITY_TIME_CRITICAL`,
///   `High` → `THREAD_PRIORITY_HIGHEST`, `Normal` → `THREAD_PRIORITY_NORMAL`,
///   `Low` → `THREAD_PRIORITY_LOWEST`.
/// - **macOS:** Uses `pthread_set_qos_class_self_np`. `Realtime` → `QOS_CLASS_USER_INTERACTIVE`,
///   `High` → `QOS_CLASS_USER_INITIATED`, `Normal` → `QOS_CLASS_DEFAULT`, `Low` → `QOS_CLASS_UTILITY`.
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

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::io::AsRawHandle;
        let handle = std::thread::current().as_raw_handle();
        let priority_val = match p {
            ThreadPriority::Realtime { .. } => 15, // THREAD_PRIORITY_TIME_CRITICAL
            ThreadPriority::High => 2,               // THREAD_PRIORITY_HIGHEST
            ThreadPriority::Normal => 0,             // THREAD_PRIORITY_NORMAL
            ThreadPriority::Low => -2,               // THREAD_PRIORITY_LOWEST
        };
        // SAFETY: handle is valid (current thread). SetThreadPriority is a standard Windows API.
        let result = unsafe { SetThreadPriority(handle, priority_val) };
        if result != 0 {
            Ok(())
        } else {
            Err(format!("SetThreadPriority failed: {}", std::io::Error::last_os_error()))
        }
    }

    #[cfg(target_os = "macos")]
    {
        let qos_class = match p {
            ThreadPriority::Realtime { .. } => libc::QOS_CLASS_USER_INTERACTIVE,
            ThreadPriority::High => libc::QOS_CLASS_USER_INITIATED,
            ThreadPriority::Normal => libc::QOS_CLASS_DEFAULT,
            ThreadPriority::Low => libc::QOS_CLASS_UTILITY,
        };
        let result = unsafe { libc::pthread_set_qos_class_self_np(qos_class, 0) };
        if result == 0 {
            Ok(())
        } else {
            Err(format!("pthread_set_qos_class_self_np failed: {}", std::io::Error::last_os_error()))
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = p;
        Ok(())
    }
}

/// Set the current thread's OS-level name.
///
/// # Platform-specific
/// - **Linux:** Uses `pthread_setname_np(pthread_self(), name)`.
/// - **Windows:** Uses `SetThreadDescription`.
/// - **macOS:** Uses `pthread_setname_np(name)` (single-argument variant).
/// - **Other:** No-op.
pub fn set_current_thread_name(name: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let c_name = std::ffi::CString::new(name).map_err(|e| format!("invalid thread name: {e}"))?;
        let result = unsafe { libc::pthread_setname_np(libc::pthread_self(), c_name.as_ptr()) };
        if result == 0 {
            Ok(())
        } else {
            Err(format!("pthread_setname_np failed: {}", std::io::Error::last_os_error()))
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = name;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let c_name = std::ffi::CString::new(name).map_err(|e| format!("invalid thread name: {e}"))?;
        let result = unsafe { libc::pthread_setname_np(c_name.as_ptr()) };
        if result == 0 {
            Ok(())
        } else {
            Err(format!("pthread_setname_np failed: {}", std::io::Error::last_os_error()))
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = name;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
extern "system" {
    fn SetThreadPriority(hThread: *mut std::ffi::c_void, nPriority: i32) -> i32;
}
