/// Render and filter-performance diagnostics owned by the main script list surface.
#[derive(Debug)]
struct MainMenuRenderDiagnosticsState {
    /// Last filter value that produced render diagnostics.
    last_render_log_filter: String,
    /// Last selection index that produced render diagnostics.
    last_render_log_selection: usize,
    /// Last item count that produced render diagnostics.
    last_render_log_item_count: usize,
    /// True when the current render changed enough to log preview diagnostics.
    log_this_render: bool,
    /// Start time for the current input-to-grouped-results performance sample.
    filter_perf_start: Option<std::time::Instant>,
    /// Cache fields for highlighting
    last_input_highlight_text: String,
    last_input_highlight_ranges: Vec<(std::ops::Range<usize>, gpui::Hsla, String)>,
}

#[derive(Clone, Debug)]
struct SubmitDiagnosticEvent {
    generation: u64,
    owner: &'static str,
    route: &'static str,
    surface: String,
    prompt_id: Option<String>,
    value: Option<String>,
    selected_index: Option<usize>,
    consumed_enter: bool,
}

#[derive(Debug, Default)]
struct SubmitDiagnosticsState {
    generation: u64,
    last: Option<SubmitDiagnosticEvent>,
    pending_enter_consumed_at: Option<std::time::Instant>,
}

impl Default for MainMenuRenderDiagnosticsState {
    fn default() -> Self {
        Self {
            last_render_log_filter: String::new(),
            last_render_log_selection: usize::MAX,
            last_render_log_item_count: usize::MAX,
            log_this_render: true,
            filter_perf_start: None,
            last_input_highlight_text: String::new(),
            last_input_highlight_ranges: Vec::new(),
        }
    }
}

/// Environment-gated proxy timing from main-list wheel events to GPUI's next
/// frame callback. This is callback timing, not compositor presentation time.
#[derive(Debug)]
struct MainListScrollFrameTrace {
    enabled: bool,
    gesture_generation: u64,
    event_count: u64,
    frame_callback_count: u64,
    callback_scheduled: bool,
    pending_event_times: Vec<std::time::Instant>,
    last_frame_callback_at: Option<std::time::Instant>,
    event_to_frame_ms: std::collections::VecDeque<f64>,
    frame_interval_ms: std::collections::VecDeque<f64>,
}

impl MainListScrollFrameTrace {
    const MAX_SAMPLES: usize = 512;

    fn from_env() -> Self {
        Self {
            enabled: std::env::var_os("SCRIPT_KIT_MAIN_LIST_SCROLL_FRAME_TRACE").is_some(),
            gesture_generation: 0,
            event_count: 0,
            frame_callback_count: 0,
            callback_scheduled: false,
            pending_event_times: Vec::new(),
            last_frame_callback_at: None,
            event_to_frame_ms: std::collections::VecDeque::new(),
            frame_interval_ms: std::collections::VecDeque::new(),
        }
    }

    fn record_event(&mut self, began_gesture: bool, now: std::time::Instant) -> bool {
        if !self.enabled {
            return false;
        }
        if began_gesture {
            self.gesture_generation = self.gesture_generation.wrapping_add(1);
        }
        self.event_count = self.event_count.wrapping_add(1);
        self.pending_event_times.push(now);
        if self.callback_scheduled {
            false
        } else {
            self.callback_scheduled = true;
            true
        }
    }

    fn record_frame_callback(&mut self, now: std::time::Instant) {
        if !self.enabled {
            return;
        }
        self.callback_scheduled = false;
        self.frame_callback_count = self.frame_callback_count.wrapping_add(1);
        let event_samples = self
            .pending_event_times
            .drain(..)
            .map(|event_at| now.duration_since(event_at).as_secs_f64() * 1000.0)
            .collect::<Vec<_>>();
        for sample in event_samples {
            Self::push_sample(&mut self.event_to_frame_ms, sample);
        }
        if let Some(previous) = self.last_frame_callback_at.replace(now) {
            Self::push_sample(
                &mut self.frame_interval_ms,
                now.duration_since(previous).as_secs_f64() * 1000.0,
            );
        }
    }

    fn push_sample(samples: &mut std::collections::VecDeque<f64>, sample: f64) {
        if samples.len() == Self::MAX_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(sample);
    }

    fn percentile(samples: &std::collections::VecDeque<f64>, percentile: f64) -> Option<f64> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.iter().copied().collect::<Vec<_>>();
        sorted.sort_by(f64::total_cmp);
        let index = ((sorted.len() - 1) as f64 * percentile).ceil() as usize;
        sorted.get(index).copied()
    }

    fn receipt(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": self.enabled,
            "timingSource": "gpuiNextFrameCallbackProxy",
            "gestureGeneration": self.gesture_generation,
            "eventCount": self.event_count,
            "frameCallbackCount": self.frame_callback_count,
            "eventToFrameMsP50": Self::percentile(&self.event_to_frame_ms, 0.50),
            "eventToFrameMsP95": Self::percentile(&self.event_to_frame_ms, 0.95),
            "eventToFrameMsMax": Self::percentile(&self.event_to_frame_ms, 1.0),
            "frameIntervalMsP50": Self::percentile(&self.frame_interval_ms, 0.50),
            "frameIntervalMsP95": Self::percentile(&self.frame_interval_ms, 0.95),
            "frameIntervalMsMax": Self::percentile(&self.frame_interval_ms, 1.0),
            "framesOver16_7Ms": self.frame_interval_ms.iter().filter(|sample| **sample > 16.7).count(),
            "framesOver33_3Ms": self.frame_interval_ms.iter().filter(|sample| **sample > 33.3).count(),
        })
    }
}
