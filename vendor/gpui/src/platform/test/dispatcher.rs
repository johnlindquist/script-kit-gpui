use crate::{PlatformDispatcher, Priority, RunnableVariant};
use scheduler::Instant;
use scheduler::{Clock, Scheduler, SessionId, TestScheduler, TestSchedulerConfig, Yield};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

/// TestDispatcher provides deterministic async execution for tests.
///
/// This implementation delegates task scheduling to the scheduler crate's `TestScheduler`.
/// Access the scheduler directly via `scheduler()` for clock, rng, and parking control.
#[doc(hidden)]
pub struct TestDispatcher {
    session_id: SessionId,
    scheduler: Arc<TestScheduler>,
    num_cpus_override: Arc<AtomicUsize>,
}

impl TestDispatcher {
    pub fn new(seed: u64) -> Self {
        let scheduler = Arc::new(TestScheduler::new(TestSchedulerConfig {
            seed,
            randomize_order: true,
            allow_parking: false,
            capture_pending_traces: std::env::var("PENDING_TRACES")
                .map_or(false, |var| var == "1" || var == "true"),
            timeout_ticks: 0..=1000,
        }));
        Self::from_scheduler(scheduler)
    }

    pub fn from_scheduler(scheduler: Arc<TestScheduler>) -> Self {
        TestDispatcher {
            session_id: scheduler.allocate_session_id(),
            scheduler,
            num_cpus_override: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn scheduler(&self) -> &Arc<TestScheduler> {
        &self.scheduler
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn drain_tasks(&self) {
        self.scheduler.drain_tasks();
    }

    pub fn advance_clock(&self, by: Duration) {
        self.scheduler.advance_clock(by);
    }

    pub fn advance_clock_to_next_timer(&self) -> bool {
        self.scheduler.advance_clock_to_next_timer()
    }

    pub fn simulate_random_delay(&self) -> Yield {
        self.scheduler.yield_random()
    }

    pub fn tick(&self, background_only: bool) -> bool {
        if background_only {
            self.scheduler.tick_background_only()
        } else {
            self.scheduler.tick()
        }
    }

    pub fn run_until_parked(&self) {
        while self.tick(false) {}
    }

    /// Advance time without recursively draining tasks; each subsequent tick is budgetable.
    pub fn advance_clock_without_running(&self, duration: Duration) {
        self.scheduler.clock().advance(duration);
    }

    /// Execute no more than the requested number of scheduler steps (including timer wakes).
    pub fn run_bounded(&self, max_steps: usize) -> usize {
        let mut executed = 0;
        while executed < max_steps && self.tick(false) {
            executed += 1;
        }
        executed
    }

    /// Actual queued foreground/background runnables. Timers are reported separately as pending work.
    pub fn pending_task_counts(&self) -> (usize, usize) {
        self.scheduler.pending_task_counts()
    }

    /// Whether runnable or timer work remains in this scheduler.
    pub fn has_pending_work(&self) -> bool {
        self.scheduler.has_pending_tasks()
    }

    pub fn allow_parking(&self) {
        self.scheduler.allow_parking();
    }

    pub fn forbid_parking(&self) {
        self.scheduler.forbid_parking();
    }

    /// Override the value returned by `BackgroundExecutor::num_cpus()` in tests.
    /// A value of 0 means no override (the default of 4 is used).
    pub fn set_num_cpus(&self, count: usize) {
        self.num_cpus_override.store(count, Ordering::SeqCst);
    }

    /// Returns the overridden CPU count, or `None` if no override is set.
    pub fn num_cpus_override(&self) -> Option<usize> {
        match self.num_cpus_override.load(Ordering::SeqCst) {
            0 => None,
            n => Some(n),
        }
    }
}

impl Clone for TestDispatcher {
    fn clone(&self) -> Self {
        let session_id = self.scheduler.allocate_session_id();
        Self {
            session_id,
            scheduler: self.scheduler.clone(),
            num_cpus_override: self.num_cpus_override.clone(),
        }
    }
}

impl PlatformDispatcher for TestDispatcher {
    fn get_all_timings(&self) -> Vec<crate::ThreadTaskTimings> {
        Vec::new()
    }

    fn get_current_thread_timings(&self) -> crate::ThreadTaskTimings {
        crate::ThreadTaskTimings {
            thread_name: None,
            thread_id: std::thread::current().id(),
            timings: Vec::new(),
            total_pushed: 0,
        }
    }

    fn is_main_thread(&self) -> bool {
        self.scheduler.is_main_thread()
    }

    fn now(&self) -> Instant {
        self.scheduler.clock().now()
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        self.scheduler
            .schedule_background_with_priority(runnable, priority);
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _priority: Priority) {
        self.scheduler
            .schedule_foreground(self.session_id, runnable);
    }

    fn dispatch_after(&self, _duration: Duration, _runnable: RunnableVariant) {
        panic!(
            "dispatch_after should not be called in tests. \
            Use BackgroundExecutor::timer() which uses the scheduler's native timer."
        );
    }

    fn as_test(&self) -> Option<&TestDispatcher> {
        Some(self)
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(move || {
            f();
        });
    }
}

#[cfg(test)]
mod bounded_tests {
    use super::*;
    use crate::BackgroundExecutor;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn self_waking_work_cannot_escape_step_budget() {
        let dispatcher = TestDispatcher::new(0);
        let executor = BackgroundExecutor::new(Arc::new(dispatcher.clone()));
        let polls = Arc::new(AtomicUsize::new(0));
        let observed = polls.clone();
        let task = executor.spawn(std::future::poll_fn(move |cx| {
            observed.fetch_add(1, Ordering::SeqCst);
            cx.waker().wake_by_ref();
            std::task::Poll::<()>::Pending
        }));
        assert_eq!(dispatcher.run_bounded(0), 0);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert_eq!(dispatcher.run_bounded(7), 7);
        assert_eq!(polls.load(Ordering::SeqCst), 7);
        assert!(dispatcher.has_pending_work());
        drop(task);
    }

    #[test]
    fn advancing_clock_does_not_execute_timer_callbacks() {
        let dispatcher = TestDispatcher::new(0);
        let executor = BackgroundExecutor::new(Arc::new(dispatcher.clone()));
        let completed = Arc::new(AtomicBool::new(false));
        let observed = completed.clone();
        let timer = executor.timer(Duration::from_millis(100));
        let task = executor.spawn(async move {
            timer.await;
            observed.store(true, Ordering::SeqCst);
        });
        dispatcher.run_bounded(1);
        dispatcher.advance_clock_without_running(Duration::from_millis(99));
        dispatcher.run_bounded(8);
        assert!(!completed.load(Ordering::SeqCst));
        dispatcher.advance_clock_without_running(Duration::from_millis(1));
        assert!(!completed.load(Ordering::SeqCst));
        dispatcher.run_bounded(8);
        assert!(completed.load(Ordering::SeqCst));
        drop(task);
    }
}
