#[cfg(test)]
mod lifecycle_tests {
    use super::{
        notes_window_close_transition, run_current_notes_window_close_sequence,
        run_existing_day_note_reuse_handoff, NotesWindowCloseOrigin,
    };
    use crate::notes::window::navigation::active_notes_selection_id;
    use std::cell::RefCell;

    #[test]
    fn existing_day_note_reuse_selects_then_activates_after_main_hide() {
        let events = RefCell::new(Vec::new());
        let mut context = ();

        run_existing_day_note_reuse_handoff(
            &mut context,
            |_| {
                events.borrow_mut().push("select_day_note");
                Ok::<(), ()>(())
            },
            |_| {
                events.borrow_mut().push("hide_main_window_completed");
                events.borrow_mut().push("activate_notes_window");
            },
        )
        .expect("the modeled day-note selection should succeed");

        assert_eq!(
            events.into_inner(),
            [
                "select_day_note",
                "hide_main_window_completed",
                "activate_notes_window",
            ],
            "an existing Notes window must select the day note before main hides, then activate only after native hide completion",
        );
    }

    #[test]
    fn external_day_note_select_updates_active_selection_id() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 7, 20).expect("valid fixture date");

        assert_eq!(
            active_notes_selection_id(None, Some(date)),
            Some("day:2026-07-20".to_string()),
            "an externally selected day note must remain observable as the active Notes selection",
        );
    }

    #[test]
    fn current_window_close_retires_every_registration_before_launcher_restore() {
        let transition = notes_window_close_transition(NotesWindowCloseOrigin::CurrentWindow);

        assert!(transition.take_window_handle);
        assert!(transition.take_app_entity);
        assert!(transition.remove_automation_registration);
        assert!(transition.remove_runtime_handle);
        assert!(transition.restore_launcher_after_removal);
    }

    #[test]
    fn current_window_close_focus_handoff_precedes_single_window_release() {
        #[derive(Debug)]
        struct FakeLifecycle {
            gpui_window_exists: bool,
            release_count: usize,
            error_logs: Vec<&'static str>,
            events: Vec<&'static str>,
        }

        let lifecycle = RefCell::new(FakeLifecycle {
            gpui_window_exists: true,
            release_count: 0,
            error_logs: Vec::new(),
            events: Vec::new(),
        });
        let transition = notes_window_close_transition(NotesWindowCloseOrigin::CurrentWindow);

        run_current_notes_window_close_sequence(
            transition,
            |_| lifecycle.borrow_mut().events.push("retire"),
            || {
                let mut lifecycle = lifecycle.borrow_mut();
                lifecycle.events.push("restore_launcher");
                if !lifecycle.gpui_window_exists {
                    lifecycle.error_logs.push("window not found");
                }
            },
            || {
                let mut lifecycle = lifecycle.borrow_mut();
                lifecycle.events.push("schedule_window_release");
                lifecycle.gpui_window_exists = false;
                lifecycle.release_count += 1;
            },
        );

        let lifecycle = lifecycle.into_inner();
        assert_eq!(
            lifecycle.events,
            ["retire", "restore_launcher", "schedule_window_release"]
        );
        assert_eq!(
            lifecycle.release_count, 1,
            "window release must be exactly once"
        );
        assert!(
            lifecycle.error_logs.is_empty(),
            "focus callbacks must not touch an already-released GPUI handle: {:?}",
            lifecycle.error_logs
        );
    }
}
