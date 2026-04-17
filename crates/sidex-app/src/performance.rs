//! Performance monitoring — frame timing, render latency, and memory usage.
//!
//! The [`PerformanceMonitor`] collects sliding-window statistics that the
//! application can query for diagnostics, developer overlays, or telemetry.

use std::collections::VecDeque;
use std::time::Duration;

/// Maximum number of samples kept for each rolling metric.
const WINDOW_SIZE: usize = 300;

/// Target frame budget (16.67 ms ≈ 60 fps).
const TARGET_FRAME_TIME: Duration = Duration::from_micros(16_667);

/// Collects and summarises frame / render timing data.
pub struct PerformanceMonitor {
    pub frame_times: VecDeque<Duration>,
    pub render_times: VecDeque<Duration>,
    pub event_processing_times: VecDeque<Duration>,
    pub memory_usage: u64,
    pub max_frame_time: Duration,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            frame_times: VecDeque::with_capacity(WINDOW_SIZE),
            render_times: VecDeque::with_capacity(WINDOW_SIZE),
            event_processing_times: VecDeque::with_capacity(WINDOW_SIZE),
            memory_usage: 0,
            max_frame_time: Duration::ZERO,
        }
    }

    // ── Recording ────────────────────────────────────────────────────────

    pub fn record_frame_time(&mut self, duration: Duration) {
        if self.frame_times.len() >= WINDOW_SIZE {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(duration);
        if duration > self.max_frame_time {
            self.max_frame_time = duration;
        }
    }

    pub fn record_render_time(&mut self, duration: Duration) {
        if self.render_times.len() >= WINDOW_SIZE {
            self.render_times.pop_front();
        }
        self.render_times.push_back(duration);
    }

    pub fn record_event_processing_time(&mut self, duration: Duration) {
        if self.event_processing_times.len() >= WINDOW_SIZE {
            self.event_processing_times.pop_front();
        }
        self.event_processing_times.push_back(duration);
    }

    pub fn update_memory_usage(&mut self, bytes: u64) {
        self.memory_usage = bytes;
    }

    // ── Queries ──────────────────────────────────────────────────────────

    pub fn average_fps(&self) -> f32 {
        let avg = self.average_frame_time();
        if avg.is_zero() {
            return 0.0;
        }
        1.0 / avg.as_secs_f32()
    }

    pub fn average_frame_time(&self) -> Duration {
        mean_duration(&self.frame_times)
    }

    pub fn average_render_time(&self) -> Duration {
        mean_duration(&self.render_times)
    }

    pub fn average_event_time(&self) -> Duration {
        mean_duration(&self.event_processing_times)
    }

    /// 99th-percentile frame time (worst 1 % of frames).
    pub fn p99_frame_time(&self) -> Duration {
        percentile_duration(&self.frame_times, 99)
    }

    /// 95th-percentile frame time.
    pub fn p95_frame_time(&self) -> Duration {
        percentile_duration(&self.frame_times, 95)
    }

    pub fn memory_usage_mb(&self) -> f64 {
        self.memory_usage as f64 / (1024.0 * 1024.0)
    }

    /// Heuristic: performance is acceptable when the p99 frame time stays
    /// within 2× the 60 fps budget (~33 ms).
    pub fn is_performance_ok(&self) -> bool {
        self.p99_frame_time() <= TARGET_FRAME_TIME * 2
    }

    /// Reset all collected samples.
    pub fn reset(&mut self) {
        self.frame_times.clear();
        self.render_times.clear();
        self.event_processing_times.clear();
        self.max_frame_time = Duration::ZERO;
    }

    /// Number of frame samples currently stored.
    pub fn sample_count(&self) -> usize {
        self.frame_times.len()
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn mean_duration(samples: &VecDeque<Duration>) -> Duration {
    if samples.is_empty() {
        return Duration::ZERO;
    }
    let total: Duration = samples.iter().copied().sum();
    total / samples.len() as u32
}

fn percentile_duration(samples: &VecDeque<Duration>, pct: usize) -> Duration {
    if samples.is_empty() {
        return Duration::ZERO;
    }
    let mut sorted: Vec<Duration> = samples.iter().copied().collect();
    sorted.sort_unstable();
    let idx = (sorted.len() * pct / 100).min(sorted.len() - 1);
    sorted[idx]
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_monitor_is_empty() {
        let pm = PerformanceMonitor::new();
        assert_eq!(pm.sample_count(), 0);
        assert_eq!(pm.average_fps(), 0.0);
        assert_eq!(pm.memory_usage_mb(), 0.0);
    }

    #[test]
    fn record_and_average_frame_time() {
        let mut pm = PerformanceMonitor::new();
        for _ in 0..10 {
            pm.record_frame_time(Duration::from_millis(16));
        }
        let avg = pm.average_frame_time();
        assert!(avg >= Duration::from_millis(15) && avg <= Duration::from_millis(17));
    }

    #[test]
    fn fps_calculation() {
        let mut pm = PerformanceMonitor::new();
        for _ in 0..100 {
            pm.record_frame_time(Duration::from_micros(16_667));
        }
        let fps = pm.average_fps();
        assert!(fps > 58.0 && fps < 62.0, "fps = {fps}");
    }

    #[test]
    fn window_evicts_old_samples() {
        let mut pm = PerformanceMonitor::new();
        for i in 0..(WINDOW_SIZE + 50) {
            pm.record_frame_time(Duration::from_millis(i as u64));
        }
        assert_eq!(pm.frame_times.len(), WINDOW_SIZE);
    }

    #[test]
    fn p99_frame_time_works() {
        let mut pm = PerformanceMonitor::new();
        for _ in 0..99 {
            pm.record_frame_time(Duration::from_millis(10));
        }
        pm.record_frame_time(Duration::from_millis(100));
        let p99 = pm.p99_frame_time();
        assert!(p99 >= Duration::from_millis(10));
    }

    #[test]
    fn is_performance_ok_under_budget() {
        let mut pm = PerformanceMonitor::new();
        for _ in 0..100 {
            pm.record_frame_time(Duration::from_millis(10));
        }
        assert!(pm.is_performance_ok());
    }

    #[test]
    fn is_performance_bad_over_budget() {
        let mut pm = PerformanceMonitor::new();
        for _ in 0..100 {
            pm.record_frame_time(Duration::from_millis(50));
        }
        assert!(!pm.is_performance_ok());
    }

    #[test]
    fn memory_usage_mb_conversion() {
        let mut pm = PerformanceMonitor::new();
        pm.update_memory_usage(1024 * 1024 * 256);
        assert!((pm.memory_usage_mb() - 256.0).abs() < 0.01);
    }

    #[test]
    fn reset_clears_all() {
        let mut pm = PerformanceMonitor::new();
        pm.record_frame_time(Duration::from_millis(16));
        pm.record_render_time(Duration::from_millis(5));
        pm.record_event_processing_time(Duration::from_millis(2));
        pm.reset();
        assert_eq!(pm.sample_count(), 0);
        assert_eq!(pm.max_frame_time, Duration::ZERO);
    }
}
