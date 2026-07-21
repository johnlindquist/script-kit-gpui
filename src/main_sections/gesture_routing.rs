// Main-hotkey gesture routing: classifier listener + surface morph handlers.

use std::time::Instant;

use gpui::AsyncApp;

use crate::hotkeys::gesture::GestureEvent;
use crate::hotkeys::process_main_hotkey_physical_event;

/// True while the in-flight gesture began from the closed (window hidden) state,
/// i.e. its key-down emitted `ShowImmediate`. The classifier does not carry this
/// distinction, but routing needs it: the opening tap's deferred `Tap` must NOT
/// immediately hide the window it just opened, and `HoldStart` only acts for
/// hold-from-closed (hold-while-open is intentionally dead until a later task).
static MAIN_GESTURE_BEGAN_CLOSED: AtomicBool = AtomicBool::new(false);
static MAIN_GESTURE_TAP_PREVIEW_APPLIED: AtomicBool = AtomicBool::new(false);

/// Start the async listener that classifies main-hotkey key-down/key-up pairs.
///
/// Single-task design: ONE loop owns every classifier mutation. It waits for
/// either the next physical key event or the classifier's next deadline, then
/// drains all queued physical events (their timestamps come from injection
/// time) BEFORE applying time-based transitions. A separate poll task would
/// race the listener — e.g. firing `HoldStart` from wall-clock time while a
/// sub-250ms key-up was still queued behind a busy main thread.
pub(crate) fn spawn_main_hotkey_gesture_listener(
    cx: &mut App,
    app_entity: Entity<ScriptListApp>,
    window: WindowHandle<Root>,
) {
    let executor = cx.background_executor().clone();
    cx.spawn(async move |cx: &mut AsyncApp| {
        logging::log("HOTKEY", "Main hotkey gesture listener started");
        let receiver = hotkeys::main_hotkey_physical_channel().1.clone();
        loop {
            // Wait for the next physical event, or wake at the classifier
            // deadline (hold threshold / double-tap window expiry).
            let waited: Option<Result<hotkeys::MainHotkeyPhysicalEvent, async_channel::RecvError>> =
                match hotkeys::main_gesture_next_deadline() {
                    Some(deadline) => {
                        // Floor the wait: if a classifier state ever reports
                        // an already-elapsed deadline that poll can't clear,
                        // an unfloored timer spins this loop at 100% CPU.
                        let wait = deadline
                            .saturating_duration_since(Instant::now())
                            .max(std::time::Duration::from_millis(4));
                        let recv = async { Some(receiver.recv().await) };
                        let timer = async {
                            executor.timer(wait).await;
                            None
                        };
                        smol::future::or(recv, timer).await
                    }
                    None => Some(receiver.recv().await),
                };

            let mut events = Vec::new();
            let mut correlation_id = None;
            match waited {
                Some(Ok(physical)) => {
                    correlation_id = Some(physical.correlation_id.clone());
                    events.extend(process_main_hotkey_physical_event(physical));
                    // Drain everything already queued so ordering follows the
                    // injected timestamps, not poll wall-clock time.
                    while let Ok(more) = receiver.try_recv() {
                        events.extend(process_main_hotkey_physical_event(more));
                    }
                }
                Some(Err(_)) => break,
                None => {}
            }
            while let Some(gesture) = hotkeys::poll_main_gesture_classifier(Instant::now()) {
                events.push(gesture);
            }
            if events.is_empty() {
                continue;
            }

            let _guard = correlation_id.map(logging::set_correlation_id);
            let app_entity_inner = app_entity.clone();
            let window_inner = window;
            let _ = cx.update(move |cx: &mut App| {
                for gesture in events {
                    dispatch_main_gesture_event(
                        gesture,
                        window_inner,
                        app_entity_inner.clone(),
                        cx,
                    );
                }
            });
        }
        logging::log(
            "HOTKEY",
            "Main hotkey gesture listener exiting (channel closed)",
        );
    })
    .detach();
}

fn dispatch_main_gesture_event(
    event: GestureEvent,
    window: WindowHandle<Root>,
    app_entity: Entity<ScriptListApp>,
    cx: &mut App,
) {
    match event {
        GestureEvent::ShowImmediate => {
            logging::log("GESTURE", "ShowImmediate — key-down show");
            MAIN_GESTURE_BEGAN_CLOSED.store(true, Ordering::SeqCst);
            MAIN_GESTURE_TAP_PREVIEW_APPLIED.store(false, Ordering::SeqCst);
            if !script_kit_gpui::is_main_window_visible() {
                show_main_window_helper(window, app_entity, cx);
            }
        }
        GestureEvent::TapPreview => {
            if MAIN_GESTURE_BEGAN_CLOSED.load(Ordering::SeqCst) {
                return;
            }
            if !script_kit_gpui::is_main_window_visible() {
                return;
            }

            let applied = window
                .update(cx, |_root, window, cx| {
                    app_entity.update(cx, |view, cx| {
                        view.try_handle_main_hotkey_tap_preview(window, cx)
                    })
                })
                .unwrap_or(false);
            MAIN_GESTURE_TAP_PREVIEW_APPLIED.store(applied, Ordering::SeqCst);
            if applied {
                logging::log("GESTURE", "TapPreview — immediate safe toggle applied");
            }
        }
        GestureEvent::Tap => {
            if MAIN_GESTURE_TAP_PREVIEW_APPLIED.swap(false, Ordering::SeqCst) {
                logging::log("GESTURE", "Tap — final tap ignored after preview");
                return;
            }
            let began_closed = MAIN_GESTURE_BEGAN_CLOSED.swap(false, Ordering::SeqCst);
            if began_closed {
                // The tap that opened the window resolves to the launcher
                // steady state — it must not also toggle to Day Page.
                logging::log(
                    "GESTURE",
                    "Tap — opening tap resolved (launcher steady state)",
                );
                return;
            }
            logging::log("GESTURE", "Tap — hide launcher (hotkey toggle)");
            if !script_kit_gpui::is_main_window_visible() {
                return;
            }
            let _ = window.update(cx, |_root, window, cx| {
                app_entity.update(cx, |view, cx| {
                    view.handle_main_hotkey_tap(window, cx);
                });
            });
        }
        GestureEvent::DoubleTap => {
            logging::log("GESTURE", "DoubleTap — Agent Chat entry intent");
            MAIN_GESTURE_BEGAN_CLOSED.store(false, Ordering::SeqCst);
            MAIN_GESTURE_TAP_PREVIEW_APPLIED.store(false, Ordering::SeqCst);
            if !script_kit_gpui::is_main_window_visible() {
                show_main_window_helper(window, app_entity.clone(), cx);
            }
            app_entity.update(cx, |view, cx| {
                view.mark_opened_directly("gesture_double_tap");
                // Double-tap is the quick-question entry: a CLEAN composer,
                // never the launcher's auto-selected row as a context chip.
                // Routing through the chip-staging entry here caused the
                // 2026-07-10 "double-tap attaches the first selected item"
                // regression.
                view.open_agent_chat_for_quick_question(cx);
            });
        }
        GestureEvent::HoldStart => {
            let began_closed = MAIN_GESTURE_BEGAN_CLOSED.load(Ordering::SeqCst);
            if !began_closed {
                // Hold-while-open semantics are intentionally dead (T8 scope).
                logging::log("GESTURE", "HoldStart — ignored (hold-while-open is dead)");
                return;
            }
            logging::log("GESTURE", "HoldStart — show Day Page");
            if !script_kit_gpui::is_main_window_visible() {
                show_main_window_helper(window, app_entity.clone(), cx);
            }
            let _ = window.update(cx, |_root, window, cx| {
                app_entity.update(cx, |view, cx| {
                    view.show_day_page_view(window, cx);
                });
            });
        }
        GestureEvent::HoldEnd => {
            // Hold release is inert: the Day Page is already showing.
            // Dictation is owned by the dedicated dictation shortcut/window.
            MAIN_GESTURE_BEGAN_CLOSED.store(false, Ordering::SeqCst);
        }
    }
}

impl ScriptListApp {
    /// Fast path for taps that are safe to apply before the double-tap window
    /// expires. Hiding resets state (`close_and_reset_window` clears the
    /// filter), so it only previews where there is nothing to lose: an empty
    /// launcher, or the Day Page (whose dirty content is flushed by
    /// `reset_to_script_list`). Taps over typed filter text or prompts stay
    /// on the delayed `Tap` path so a double-tap cannot destroy input.
    pub(crate) fn try_handle_main_hotkey_tap_preview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.current_view {
            AppView::ScriptList if self.filter_text.trim().is_empty() => {
                self.handle_main_hotkey_tap(window, cx);
                true
            }
            AppView::DayPage { .. } => {
                self.handle_main_hotkey_tap(window, cx);
                true
            }
            _ => false,
        }
    }

    /// Tap-while-open: hide the launcher (the universal hotkey-toggle
    /// convention). Day Page entry lives on the `,` first-character trigger
    /// and hold-from-closed instead. `close_and_reset_window` funnels through
    /// `reset_to_script_list`, which flushes a dirty Day Page before the
    /// entity drops.
    ///
    /// Exception: an embedded Agent Chat is a sticky surface that survives
    /// click-outside, so the panel can be on screen without key status. A tap
    /// in that state means "get me back to my chat" — reclaim key + composer
    /// focus instead of destroying the live session.
    pub(crate) fn handle_main_hotkey_tap(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.was_window_focused && matches!(self.current_view, AppView::AgentChatView { .. }) {
            logging::log(
                "GESTURE",
                "Tap with unfocused Agent Chat — reclaiming focus instead of closing",
            );
            platform::show_main_window_without_activation();
            self.request_focus(FocusTarget::AgentChat, cx);
            cx.notify();
            return;
        }
        self.close_and_reset_window(cx);
    }
}
