//! App-independent latest-value coalescing for launcher filter updates.
//!
//! Keep this state machine outside the GUI binary so its scheduling and reset
//! contracts remain executable without compiling GPUI or enabling binary tests.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterBatchTicket {
    generation: u64,
}

#[derive(Debug, Default)]
pub struct FilterCoalescer {
    pending: Option<FilterBatchTicket>,
    next_generation: u64,
    latest: Option<String>,
}

impl FilterCoalescer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a ticket only when a new worker must be scheduled. Later values
    /// join that exact batch; exhausted ticket space refuses new work.
    pub fn queue(&mut self, value: impl Into<String>) -> Option<FilterBatchTicket> {
        if self.pending.is_some() {
            self.latest = Some(value.into());
            return None;
        }
        let generation = self.next_generation.checked_add(1)?;
        let ticket = FilterBatchTicket { generation };
        self.next_generation = generation;
        self.pending = Some(ticket);
        self.latest = Some(value.into());
        Some(ticket)
    }

    pub fn take_latest(&mut self, ticket: FilterBatchTicket) -> Option<String> {
        if self.pending != Some(ticket) {
            return None;
        }
        self.pending = None;
        self.latest.take()
    }

    pub fn reset(&mut self) {
        self.pending = None;
        self.latest = None;
    }
}

#[cfg(test)]
mod tests {
    use super::FilterCoalescer;

    #[test]
    fn coalescer_returns_latest_value_for_exact_batch() {
        let mut coalescer = FilterCoalescer::new();
        let ticket = coalescer.queue("a").unwrap();
        assert!(coalescer.queue("ab").is_none());
        assert!(coalescer.queue("abc").is_none());
        assert_eq!(coalescer.take_latest(ticket).as_deref(), Some("abc"));
        assert!(coalescer.take_latest(ticket).is_none());
        assert_ne!(coalescer.queue("next").unwrap(), ticket);
    }

    #[test]
    fn coalescer_accepts_clear_and_exact_no_op_values_in_current_batch() {
        let mut coalescer = FilterCoalescer::new();
        let ticket = coalescer.queue("query").unwrap();
        assert!(coalescer.queue("query").is_none());
        assert!(coalescer.queue("").is_none());
        assert_eq!(coalescer.take_latest(ticket).as_deref(), Some(""));
    }

    #[test]
    fn reset_retires_old_timer_before_fresh_batch_even_for_aba_text() {
        let mut coalescer = FilterCoalescer::new();
        let stale = coalescer.queue("a").unwrap();
        assert!(coalescer.queue("b").is_none());
        coalescer.reset();
        let current = coalescer.queue("a").unwrap();
        assert_ne!(stale, current);
        assert!(coalescer.take_latest(stale).is_none());
        assert!(coalescer.queue("latest a").is_none());
        assert_eq!(coalescer.take_latest(current).as_deref(), Some("latest a"));
    }

    #[test]
    fn completed_batch_cannot_drain_a_later_worker() {
        let mut coalescer = FilterCoalescer::new();
        let old = coalescer.queue("first").unwrap();
        assert_eq!(coalescer.take_latest(old).as_deref(), Some("first"));
        let current = coalescer.queue("second").unwrap();
        assert!(coalescer.take_latest(old).is_none());
        assert_eq!(coalescer.take_latest(current).as_deref(), Some("second"));
    }

    #[test]
    fn reset_discards_queued_work_and_is_idempotent() {
        let mut coalescer = FilterCoalescer::new();
        let stale = coalescer.queue("previous surface").unwrap();
        coalescer.reset();
        coalescer.reset();
        assert!(coalescer.take_latest(stale).is_none());
        assert!(coalescer.latest.is_none());
        assert!(coalescer.queue("fresh surface").is_some());
    }

    #[test]
    fn ticket_exhaustion_refuses_reuse_without_retaining_unscheduled_work() {
        let mut coalescer = FilterCoalescer {
            next_generation: u64::MAX - 1,
            ..Default::default()
        };
        let last = coalescer.queue("last").unwrap();
        assert!(coalescer.queue("last update").is_none());
        assert_eq!(coalescer.take_latest(last).as_deref(), Some("last update"));
        coalescer.reset();
        assert!(coalescer.queue("must not wrap").is_none());
        assert!(coalescer.take_latest(last).is_none());
        assert!(coalescer.latest.is_none());
    }
}
