use std::fs;

/// Reads CPU usage from `/proc/stat` on Linux.
///
/// Returns a value between `0.0` and `1.0` representing the
/// percentage of CPU time spent non-idle since the last call.
/// The first call always returns `0.0` because there is no
/// baseline sample.
#[derive(Debug, Default)]
pub struct SystemMonitor {
    prev_idle: u64,
    prev_total: u64,
    has_sample: bool,
}

impl SystemMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute CPU usage since the last call.
    ///
    /// On non-Linux platforms this always returns `0.0`.
    pub fn cpu_usage(&mut self) -> f32 {
        #[cfg(target_os = "linux")]
        {
            if let Some((idle, total)) = Self::read_proc_stat() {
                if self.has_sample {
                    let idle_diff = idle.saturating_sub(self.prev_idle);
                    let total_diff = total.saturating_sub(self.prev_total);
                    if total_diff == 0 {
                        return 0.0;
                    }
                    let usage = 1.0 - (idle_diff as f32 / total_diff as f32);
                    self.prev_idle = idle;
                    self.prev_total = total;
                    return usage.clamp(0.0, 1.0);
                }
                self.prev_idle = idle;
                self.prev_total = total;
                self.has_sample = true;
            }
        }
        0.0
    }

    #[cfg(target_os = "linux")]
    fn read_proc_stat() -> Option<(u64, u64)> {
        let data = fs::read_to_string("/proc/stat").ok()?;
        let first_line = data.lines().next()?;
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 5 || parts[0] != "cpu" {
            return None;
        }
        let values: Vec<u64> = parts[1..]
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();
        let total: u64 = values.iter().sum();
        let idle = values.get(3).copied().unwrap_or(0);
        Some((idle, total))
    }
}

/// Compute a recommended thread count given current CPU load.
///
/// When the system is under heavy load (`cpu_usage` near `1.0`)
/// fewer threads are suggested to avoid oversubscription.
/// When idle (`cpu_usage` near `0.0`) the maximum is returned.
///
/// ```
/// # use rustix_core::system_monitor::recommended_threads;
/// assert_eq!(recommended_threads(8, 0.0, 2, 8), 8);
/// assert_eq!(recommended_threads(8, 1.0, 2, 8), 2);
/// assert_eq!(recommended_threads(8, 0.5, 2, 8), 5); // linear interpolation
/// ```
pub fn recommended_threads(current: usize, cpu_usage: f32, min: usize, max: usize) -> usize {
    let clamped = cpu_usage.clamp(0.0, 1.0);
    let available = max.saturating_sub(min);
    let reduction = (clamped * available as f32).round() as usize;
    max.saturating_sub(reduction).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_first_call_returns_zero() {
        let mut m = SystemMonitor::new();
        assert_eq!(m.cpu_usage(), 0.0);
    }

    #[test]
    fn recommended_at_zero_load() {
        assert_eq!(recommended_threads(8, 0.0, 2, 8), 8);
    }

    #[test]
    fn recommended_at_full_load() {
        assert_eq!(recommended_threads(8, 1.0, 2, 8), 2);
    }

    #[test]
    fn recommended_at_half_load() {
        assert_eq!(recommended_threads(8, 0.5, 2, 8), 5);
    }

    #[test]
    fn recommended_respects_bounds() {
        assert_eq!(recommended_threads(8, -0.5, 2, 8), 8);
        assert_eq!(recommended_threads(8, 1.5, 2, 8), 2);
    }

    #[test]
    fn recommended_min_equals_max() {
        assert_eq!(recommended_threads(4, 0.5, 4, 4), 4);
    }
}
