//! App-independent latest-value coalescing for launcher filter updates.
//!
//! Keep this state machine outside the GUI binary so its scheduling and reset
//! contracts remain executable without compiling GPUI or enabling binary tests.

#[derive(Debug, Default)]
pub struct FilterCoalescer {
    pending: bool,
    latest: Option<String>,
}

impl FilterCoalescer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue(&mut self, value: impl Into<String>) -> bool {
        self.latest = Some(value.into());
        if self.pending {
            false
        } else {
            self.pending = true;
            true
        }
    }

    pub fn take_latest(&mut self) -> Option<String> {
        if !self.pending {
            return None;
        }
        self.pending = false;
        self.latest.take()
    }

    pub fn reset(&mut self) {
        self.pending = false;
        self.latest = None;
    }
}

#[cfg(test)]
mod tests {
    use super::FilterCoalescer;

    #[test]
    fn coalescer_returns_latest_value_on_tick() {
        let mut coalescer = FilterCoalescer::new();

        assert!(coalescer.queue("a"));
        assert!(!coalescer.queue("ab"));

        assert_eq!(coalescer.take_latest().as_deref(), Some("ab"));
    }

    #[test]
    fn coalescer_only_starts_one_task_per_batch() {
        let mut coalescer = FilterCoalescer::new();

        assert!(coalescer.queue("first"));
        assert!(!coalescer.queue("second"));
        assert!(!coalescer.queue("third"));

        assert!(coalescer.take_latest().is_some());
        assert!(coalescer.queue("next"));
    }

    #[test]
    fn coalescer_returns_none_when_idle() {
        let mut coalescer = FilterCoalescer::new();

        assert!(coalescer.take_latest().is_none());
        assert!(coalescer.queue("value"));
        assert!(coalescer.take_latest().is_some());
        assert!(coalescer.take_latest().is_none());
    }

    #[test]
    fn coalescer_accepts_clear_updates() {
        let mut coalescer = FilterCoalescer::new();

        assert!(coalescer.queue("query"));
        assert!(!coalescer.queue(""));

        assert_eq!(coalescer.take_latest().as_deref(), Some(""));
    }

    #[test]
    fn reset_discards_queued_work_without_emitting_stale_filter() {
        let mut coalescer = FilterCoalescer::new();

        assert!(coalescer.queue("stale previous surface"));
        coalescer.reset();

        assert!(coalescer.take_latest().is_none());
    }

    #[test]
    fn reset_allows_a_fresh_surface_to_schedule_exactly_one_worker() {
        let mut coalescer = FilterCoalescer::new();

        assert!(coalescer.queue("stale previous surface"));
        coalescer.reset();

        assert!(coalescer.queue("fresh"));
        assert!(!coalescer.queue("fresh query"));
        assert_eq!(coalescer.take_latest().as_deref(), Some("fresh query"));
        assert!(coalescer.take_latest().is_none());
    }
}
