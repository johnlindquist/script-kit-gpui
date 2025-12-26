#![allow(dead_code)]
//! Performance instrumentation and benchmarking utilities
//!
//! This module provides timing utilities for measuring:
//! - Key event processing rates and latency
//! - Scroll operation timing
//! - Frame timing and render performance
//!
//! Used to establish baseline metrics and identify performance bottlenecks.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::logging::log;

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Maximum number of samples to keep for rolling averages
const MAX_SAMPLES: usize = 100;

/// Threshold for "slow" key event processing (microseconds)
const SLOW_KEY_THRESHOLD_US: u128 = 16_666; // ~16ms (60fps frame budget)

/// Threshold for "slow" scroll operation (microseconds)
const SLOW_SCROLL_THRESHOLD_US: u128 = 8_000; // 8ms

// =============================================================================
// KEY EVENT TRACKING
// =============================================================================

/// Tracks key event timing and rate
pub struct KeyEventTracker {
    /// Timestamps of recent key events for rate calculation
    event_times: VecDeque<Instant>,
    /// Processing durations for recent key events
    processing_durations: VecDeque<Duration>,
    /// Last event timestamp for inter-event timing
    last_event: Option<Instant>,
    /// Count of events that exceeded threshold
    slow_event_count: usize,
    /// Total events processed
    total_events: usize,
}

impl KeyEventTracker {
    pub fn new() -> Self {
        Self {
            event_times: VecDeque::with_capacity(MAX_SAMPLES),
            processing_durations: VecDeque::with_capacity(MAX_SAMPLES),
            last_event: None,
            slow_event_count: 0,
            total_events: 0,
        }
    }

    /// Record the start of a key event, returns the start instant for timing
    pub fn start_event(&mut self) -> Instant {
        let now = Instant::now();

        // Track event time for rate calculation
        if self.event_times.len() >= MAX_SAMPLES {
            self.event_times.pop_front();
        }
        self.event_times.push_back(now);

        self.total_events += 1;
        now
    }

    /// Record the end of key event processing
    pub fn end_event(&mut self, start: Instant) {
        let duration = start.elapsed();

        // Track processing duration
        if self.processing_durations.len() >= MAX_SAMPLES {
            self.processing_durations.pop_front();
        }
        self.processing_durations.push_back(duration);

        // Check if this was a slow event
        if duration.as_micros() > SLOW_KEY_THRESHOLD_US {
            self.slow_event_count += 1;
        }

        self.last_event = Some(start);
    }

    /// Calculate events per second based on recent events
    pub fn events_per_second(&self) -> f64 {
        if self.event_times.len() < 2 {
            return 0.0;
        }

        let first = self.event_times.front().unwrap();
        let last = self.event_times.back().unwrap();
        let elapsed = last.duration_since(*first);

        if elapsed.as_secs_f64() < 0.001 {
            return 0.0;
        }

        (self.event_times.len() - 1) as f64 / elapsed.as_secs_f64()
    }

    /// Get average processing time in microseconds
    pub fn avg_processing_time_us(&self) -> u128 {
        if self.processing_durations.is_empty() {
            return 0;
        }

        let total: Duration = self.processing_durations.iter().sum();
        total.as_micros() / self.processing_durations.len() as u128
    }

    /// Get the percentage of events that were slow
    pub fn slow_event_percentage(&self) -> f64 {
        if self.total_events == 0 {
            return 0.0;
        }
        (self.slow_event_count as f64 / self.total_events as f64) * 100.0
    }

    /// Time since last event in milliseconds (for detecting bursts)
    pub fn time_since_last_event_ms(&self) -> Option<f64> {
        self.last_event.map(|last| last.elapsed().as_secs_f64() * 1000.0)
    }

    /// Log current statistics
    pub fn log_stats(&self) {
        log(
            "KEY_PERF",
            &format!(
                "rate={:.1}/s avg={:.2}ms slow={:.1}% total={}",
                self.events_per_second(),
                self.avg_processing_time_us() as f64 / 1000.0,
                self.slow_event_percentage(),
                self.total_events
            ),
        );
    }
}

impl Default for KeyEventTracker {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// SCROLL TIMING
// =============================================================================

/// Tracks scroll operation timing
pub struct ScrollTimer {
    /// Start instant for current scroll operation
    start: Option<Instant>,
    /// Recent scroll operation durations
    durations: VecDeque<Duration>,
    /// Count of slow scroll operations
    slow_count: usize,
    /// Total scroll operations
    total_ops: usize,
}

impl ScrollTimer {
    pub fn new() -> Self {
        Self {
            start: None,
            durations: VecDeque::with_capacity(MAX_SAMPLES),
            slow_count: 0,
            total_ops: 0,
        }
    }

    /// Start timing a scroll operation
    pub fn start(&mut self) -> Instant {
        let now = Instant::now();
        self.start = Some(now);
        now
    }

    /// End timing and record the duration
    pub fn end(&mut self) -> Duration {
        let duration = self.start.map(|s| s.elapsed()).unwrap_or(Duration::ZERO);

        if self.durations.len() >= MAX_SAMPLES {
            self.durations.pop_front();
        }
        self.durations.push_back(duration);

        if duration.as_micros() > SLOW_SCROLL_THRESHOLD_US {
            self.slow_count += 1;
        }

        self.total_ops += 1;
        self.start = None;
        duration
    }

    /// Get average scroll time in microseconds
    pub fn avg_time_us(&self) -> u128 {
        if self.durations.is_empty() {
            return 0;
        }
        let total: Duration = self.durations.iter().sum();
        total.as_micros() / self.durations.len() as u128
    }

    /// Get max scroll time in microseconds from recent samples
    pub fn max_time_us(&self) -> u128 {
        self.durations
            .iter()
            .map(|d| d.as_micros())
            .max()
            .unwrap_or(0)
    }

    /// Log scroll timing stats
    pub fn log_stats(&self) {
        log(
            "SCROLL_TIMING",
            &format!(
                "avg={:.2}ms max={:.2}ms slow={} total={}",
                self.avg_time_us() as f64 / 1000.0,
                self.max_time_us() as f64 / 1000.0,
                self.slow_count,
                self.total_ops
            ),
        );
    }
}

impl Default for ScrollTimer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// FRAME TIMING
// =============================================================================

/// Tracks frame timing for render performance
pub struct FrameTimer {
    /// Last frame timestamp
    last_frame: Option<Instant>,
    /// Recent frame durations
    frame_times: VecDeque<Duration>,
    /// Count of dropped/slow frames
    dropped_frames: usize,
    /// Total frames tracked
    total_frames: usize,
}

impl FrameTimer {
    pub fn new() -> Self {
        Self {
            last_frame: None,
            frame_times: VecDeque::with_capacity(MAX_SAMPLES),
            dropped_frames: 0,
            total_frames: 0,
        }
    }

    /// Mark a frame and calculate time since last frame
    pub fn mark_frame(&mut self) -> Option<Duration> {
        let now = Instant::now();
        let duration = self.last_frame.map(|last| now.duration_since(last));

        if let Some(d) = duration {
            if self.frame_times.len() >= MAX_SAMPLES {
                self.frame_times.pop_front();
            }
            self.frame_times.push_back(d);

            // Consider frame dropped if > 32ms (less than 30fps)
            if d.as_millis() > 32 {
                self.dropped_frames += 1;
            }
        }

        self.last_frame = Some(now);
        self.total_frames += 1;
        duration
    }

    /// Get average FPS from recent samples
    pub fn avg_fps(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let avg_frame_time: Duration =
            self.frame_times.iter().sum::<Duration>() / self.frame_times.len() as u32;

        if avg_frame_time.as_secs_f64() < 0.001 {
            return 0.0;
        }

        1.0 / avg_frame_time.as_secs_f64()
    }

    /// Get dropped frame percentage
    pub fn dropped_percentage(&self) -> f64 {
        if self.total_frames == 0 {
            return 0.0;
        }
        (self.dropped_frames as f64 / self.total_frames as f64) * 100.0
    }

    /// Log frame timing stats
    pub fn log_stats(&self) {
        log(
            "FRAME_PERF",
            &format!(
                "fps={:.1} dropped={:.1}% total={}",
                self.avg_fps(),
                self.dropped_percentage(),
                self.total_frames
            ),
        );
    }
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// GLOBAL PERF TRACKER
// =============================================================================

/// Global performance tracker instance
static PERF_TRACKER: OnceLock<Mutex<PerfTracker>> = OnceLock::new();

/// Combined performance tracker for all metrics
pub struct PerfTracker {
    pub key_events: KeyEventTracker,
    pub scroll: ScrollTimer,
    pub frames: FrameTimer,
}

impl PerfTracker {
    pub fn new() -> Self {
        Self {
            key_events: KeyEventTracker::new(),
            scroll: ScrollTimer::new(),
            frames: FrameTimer::new(),
        }
    }

    /// Log all performance stats
    pub fn log_all_stats(&self) {
        self.key_events.log_stats();
        self.scroll.log_stats();
        self.frames.log_stats();
    }
}

impl Default for PerfTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the global performance tracker
pub fn get_perf_tracker() -> &'static Mutex<PerfTracker> {
    PERF_TRACKER.get_or_init(|| Mutex::new(PerfTracker::new()))
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Start timing a key event, returns start instant
pub fn start_key_event() -> Instant {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.key_events.start_event()
    } else {
        Instant::now()
    }
}

/// End timing a key event
pub fn end_key_event(start: Instant) {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.key_events.end_event(start);
    }
}

/// Start timing a scroll operation
pub fn start_scroll() -> Instant {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.scroll.start()
    } else {
        Instant::now()
    }
}

/// End timing a scroll operation
pub fn end_scroll() -> Duration {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.scroll.end()
    } else {
        Duration::ZERO
    }
}

/// Mark a frame for FPS tracking
pub fn mark_frame() -> Option<Duration> {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.frames.mark_frame()
    } else {
        None
    }
}

/// Log current key event rate (useful for detecting fast repeat)
pub fn log_key_rate() {
    if let Ok(tracker) = get_perf_tracker().lock() {
        let rate = tracker.key_events.events_per_second();
        if rate > 20.0 {
            log(
                "KEY_PERF",
                &format!("HIGH_RATE: {:.1} events/sec", rate),
            );
        }
    }
}

/// Log all performance stats
pub fn log_perf_summary() {
    if let Ok(tracker) = get_perf_tracker().lock() {
        tracker.log_all_stats();
    }
}

// =============================================================================
// TIMING GUARD (RAII pattern for timing)
// =============================================================================

/// RAII guard for timing operations - logs when dropped
pub struct TimingGuard {
    operation: &'static str,
    start: Instant,
    threshold_us: u128,
}

impl TimingGuard {
    /// Create a new timing guard
    pub fn new(operation: &'static str, threshold_us: u128) -> Self {
        Self {
            operation,
            start: Instant::now(),
            threshold_us,
        }
    }

    /// Create a timing guard for key events
    pub fn key_event() -> Self {
        Self::new("key_event", SLOW_KEY_THRESHOLD_US)
    }

    /// Create a timing guard for scroll operations
    pub fn scroll() -> Self {
        Self::new("scroll", SLOW_SCROLL_THRESHOLD_US)
    }
}

impl Drop for TimingGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        let duration_us = duration.as_micros();

        if duration_us > self.threshold_us {
            log(
                "PERF_SLOW",
                &format!(
                    "{} took {:.2}ms (threshold: {:.2}ms)",
                    self.operation,
                    duration_us as f64 / 1000.0,
                    self.threshold_us as f64 / 1000.0
                ),
            );
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_key_event_tracker() {
        let mut tracker = KeyEventTracker::new();

        // Simulate some key events
        for _ in 0..5 {
            let start = tracker.start_event();
            thread::sleep(Duration::from_micros(100));
            tracker.end_event(start);
        }

        assert_eq!(tracker.total_events, 5);
        assert!(tracker.avg_processing_time_us() >= 100);
    }

    #[test]
    fn test_scroll_timer() {
        let mut timer = ScrollTimer::new();

        let _start = timer.start();
        thread::sleep(Duration::from_micros(100));
        let duration = timer.end();

        assert!(duration.as_micros() >= 100);
        assert_eq!(timer.total_ops, 1);
    }

    #[test]
    fn test_frame_timer() {
        let mut timer = FrameTimer::new();

        // First frame has no previous
        assert!(timer.mark_frame().is_none());

        thread::sleep(Duration::from_millis(16));
        let duration = timer.mark_frame();

        assert!(duration.is_some());
        assert!(duration.unwrap().as_millis() >= 16);
    }

    #[test]
    fn test_timing_guard() {
        // Just ensure it doesn't panic
        {
            let _guard = TimingGuard::key_event();
            thread::sleep(Duration::from_micros(100));
        }
    }
}
