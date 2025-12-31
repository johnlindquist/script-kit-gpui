/// Direction of navigation (up/down arrow keys).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
}

/// Result of recording a navigation event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavRecord {
    /// First event - apply immediately (move by 1).
    ApplyImmediate,
    /// Same direction - coalesce (buffer additional movement).
    Coalesced,
    /// Direction changed - flush old delta, then apply new direction.
    FlushOld { dir: NavDirection, delta: i32 },
}

/// Coalesces rapid arrow key events to prevent UI lag during fast keyboard repeat.
///
/// The coalescing window is 20ms - events within this window are batched together
/// and applied as a single larger movement at the next flush.
#[derive(Debug)]
pub struct NavCoalescer {
    /// Current pending direction (None if no pending movement).
    pub(crate) pending_dir: Option<NavDirection>,
    /// Accumulated delta for pending direction (# of additional moves beyond first).
    pub(crate) pending_delta: i32,
    /// Timestamp of last navigation event (for determining flush eligibility).
    pub(crate) last_event: std::time::Instant,
    /// Whether the background flush task is currently running.
    pub(crate) flush_task_running: bool,
}

impl NavCoalescer {
    /// Coalescing window: 20ms between events triggers batching.
    pub const WINDOW: std::time::Duration = std::time::Duration::from_millis(20);

    pub fn new() -> Self {
        Self {
            pending_dir: None,
            pending_delta: 0,
            last_event: std::time::Instant::now(),
            flush_task_running: false,
        }
    }

    /// Record a navigation event. Returns how to handle it:
    /// - ApplyImmediate: First event, move by 1 now
    /// - Coalesced: Same direction, buffered for later flush
    /// - FlushOld: Direction changed, flush old delta then move by 1
    pub fn record(&mut self, dir: NavDirection) -> NavRecord {
        self.last_event = std::time::Instant::now();
        match self.pending_dir {
            None => {
                // First event - start tracking this direction
                self.pending_dir = Some(dir);
                self.pending_delta = 0;
                NavRecord::ApplyImmediate
            }
            Some(existing) if existing == dir => {
                // Same direction - coalesce
                self.pending_delta += 1;
                NavRecord::Coalesced
            }
            Some(_) => {
                // Direction changed - flush old, start new
                let old_dir = self.pending_dir.unwrap();
                let old_delta = self.pending_delta;
                self.pending_dir = Some(dir);
                self.pending_delta = 0;
                NavRecord::FlushOld {
                    dir: old_dir,
                    delta: old_delta,
                }
            }
        }
    }

    /// Flush any pending navigation delta. Returns (direction, delta) if there's pending movement.
    pub fn flush_pending(&mut self) -> Option<(NavDirection, i32)> {
        let dir = self.pending_dir?;
        if self.pending_delta == 0 {
            return None;
        }
        let delta = self.pending_delta;
        self.pending_delta = 0;
        Some((dir, delta))
    }

    /// Reset the coalescer state (call after navigation completes or on view change).
    pub fn reset(&mut self) {
        self.pending_dir = None;
        self.pending_delta = 0;
    }
}

impl Default for NavCoalescer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_first_event_applies_immediately() {
        let mut coalescer = NavCoalescer::new();
        assert_eq!(coalescer.record(NavDirection::Up), NavRecord::ApplyImmediate);
        assert_eq!(coalescer.pending_dir, Some(NavDirection::Up));
        assert_eq!(coalescer.pending_delta, 0);
    }

    #[test]
    fn record_same_direction_coalesces() {
        let mut coalescer = NavCoalescer::new();
        assert_eq!(coalescer.record(NavDirection::Down), NavRecord::ApplyImmediate);
        assert_eq!(coalescer.record(NavDirection::Down), NavRecord::Coalesced);
        assert_eq!(coalescer.pending_delta, 1);

        let pending = coalescer.flush_pending();
        assert_eq!(pending, Some((NavDirection::Down, 1)));
        assert_eq!(coalescer.pending_delta, 0);
    }

    #[test]
    fn record_direction_change_flushes_old_delta() {
        let mut coalescer = NavCoalescer::new();
        assert_eq!(coalescer.record(NavDirection::Up), NavRecord::ApplyImmediate);
        assert_eq!(coalescer.record(NavDirection::Up), NavRecord::Coalesced);

        match coalescer.record(NavDirection::Down) {
            NavRecord::FlushOld { dir, delta } => {
                assert_eq!(dir, NavDirection::Up);
                assert_eq!(delta, 1);
            }
            other => panic!("expected FlushOld, got {other:?}"),
        }

        assert_eq!(coalescer.pending_dir, Some(NavDirection::Down));
        assert_eq!(coalescer.pending_delta, 0);
    }

    #[test]
    fn reset_clears_state() {
        let mut coalescer = NavCoalescer::new();
        let _ = coalescer.record(NavDirection::Up);
        let _ = coalescer.record(NavDirection::Up);
        assert_eq!(coalescer.pending_delta, 1);

        coalescer.reset();
        assert_eq!(coalescer.pending_dir, None);
        assert_eq!(coalescer.pending_delta, 0);
        assert_eq!(coalescer.flush_pending(), None);
    }
}
