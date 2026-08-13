use super::*;

static KEYBOARD_FEEDBACK_FIXTURE_ENQUEUED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

struct HiddenMainWindowResetRequest {
    _native_hidden: crate::platform::NativeMainWindowHidden,
    visibility_generation: u64,
    reason: &'static str,
    reset_mini_bounds_after_hidden_reset: bool,
}

/// What happens to the (already hidden) main window AFTER the calibrated exit
/// completes and AppKit confirms `orderOut:`. Every ordinary main-window
/// dismissal funnels through [`ScriptListApp::defer_calibrated_main_window_hide`]
/// with one of these policies instead of duplicating fade/timer/hide/reset
/// closures per call site.
#[derive(Clone, Copy, Debug)]
pub(crate) enum MainWindowPostHide {
    /// Reset the launcher route to ScriptList (the ordinary close).
    ResetScriptList {
        reason: &'static str,
        reset_mini_bounds_after_hidden_reset: bool,
    },
    /// Reset a prepared Agent Chat surface back to ScriptList.
    ResetPreparedAgentChat { reason: &'static str },
    /// Keep the view state exactly as-is (focus-loss / escape preserve hides).
    PreserveState { reason: &'static str },
}

impl MainWindowPostHide {
    fn reason(self) -> &'static str {
        match self {
            Self::ResetScriptList { reason, .. }
            | Self::ResetPreparedAgentChat { reason }
            | Self::PreserveState { reason } => reason,
        }
    }
}

/// Whether a scheduled calibrated hide is still the CURRENT request when its
/// exit-fade delay elapses. A re-show (logical visibility true) or a newer
/// visibility generation supersedes it.
fn calibrated_hide_request_is_current(
    expected_visibility_generation: u64,
    current_visibility_generation: u64,
    is_logically_visible: bool,
) -> bool {
    !is_logically_visible && current_visibility_generation == expected_visibility_generation
}

fn hidden_main_window_reset_is_current(
    expected_visibility_generation: u64,
    current_visibility_generation: u64,
    is_logically_visible: bool,
) -> bool {
    !is_logically_visible && current_visibility_generation == expected_visibility_generation
}

#[cfg(unix)]
fn process_group_id_from_pid(pid: u32) -> Result<i32, String> {
    i32::try_from(pid).map_err(|_| format!("PID {} is out of range for killpg", pid))
}

#[cfg(unix)]
fn force_kill_script_process_group(pid: u32) -> Result<(), String> {
    let process_group_id = process_group_id_from_pid(pid)?;

    // SAFETY: killpg is a syscall wrapper. We pass a validated process group id
    // captured at spawn time and a constant signal value.
    let kill_result = unsafe { libc::killpg(process_group_id, libc::SIGKILL) };
    if kill_result == 0 {
        return Ok(());
    }

    Err(format!(
        "killpg failed for process group {}: {}",
        pid,
        std::io::Error::last_os_error()
    ))
}

impl ScriptListApp {
    pub(crate) fn reset_window_positions_to_default_main_menu(&mut self, cx: &mut Context<Self>) {
        logging::log(
            "WINDOW_STATE",
            "Resetting window positions and returning main window to default menu",
        );
        self.record_return_to_script_list_submit(
            "settings",
            "reset_window_positions_to_default_main_menu",
            Some("Reset Window Positions"),
        );

        crate::window_state::suppress_save();
        crate::window_state::reset_all_positions();

        self.reset_to_script_list(cx);

        let (grouped_items, _) = self.get_grouped_results_cached();
        let sizing = crate::window_resize::main_window_sizing_from_grouped_items(&grouped_items);
        let target = crate::window_resize::MainMenuSizingTarget(sizing);
        let window_size = size(px(target.width()), target.height());
        let bounds = calculate_eye_line_bounds_on_mouse_display(window_size);
        platform::move_first_window_to_bounds(&bounds);

        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            crate::window_state::allow_save();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn cancel_script_execution_without_view_reset(&mut self) {
        logging::log("EXEC", "=== Canceling script execution ===");

        // Send cancel message to script (Exit with cancel code)
        // Use try_send to avoid blocking UI thread during cancellation
        if let Some(ref sender) = self.response_sender {
            // Try to send Exit message to terminate the script cleanly
            let exit_msg = Message::Exit {
                code: Some(1), // Non-zero code indicates cancellation
                message: Some("Cancelled by user".to_string()),
            };
            match sender.try_send(exit_msg) {
                Ok(()) => logging::log("EXEC", "Sent Exit message to script"),
                Err(std::sync::mpsc::TrySendError::Full(_)) => logging::log(
                    "EXEC",
                    "Exit message dropped - channel full (script may be stuck)",
                ),
                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                    logging::log("EXEC", "Exit message dropped - script already exited")
                }
            }
        } else {
            logging::log("EXEC", "No response_sender - script may not be running");
        }

        // Belt-and-suspenders: Force-kill the process group using stored PID
        // This ensures cleanup even if Drop doesn't fire properly
        if let Some(pid) = self.current_script_pid.take() {
            logging::log(
                "CLEANUP",
                &format!("Force-killing script process group {}", pid),
            );
            #[cfg(unix)]
            {
                if let Err(error) = force_kill_script_process_group(pid) {
                    logging::log(
                        "CLEANUP",
                        &format!(
                            "Failed to force-kill script process group {}: {}",
                            pid, error
                        ),
                    );
                }
            }
        }

        // Abort script session if it exists
        {
            let mut session_guard = self.script_session.lock();
            if let Some(_session) = session_guard.take() {
                logging::log("EXEC", "Cleared script session");
            }
        }
    }

    pub(crate) fn cancel_script_execution(&mut self, cx: &mut Context<Self>) {
        self.cancel_script_execution_without_view_reset();
        // Reset to script list view
        self.reset_to_script_list(cx);
        logging::log("EXEC", "=== Script cancellation complete ===");
    }

    fn enqueue_keyboard_feedback_fixture_if_requested(
        &mut self,
        window: &gpui::Window,
        cx: &gpui::App,
    ) {
        use std::sync::atomic::Ordering;

        if std::env::var("SCRIPT_KIT_TEST_KEYBOARD_FEEDBACK")
            .ok()
            .as_deref()
            != Some("1")
            || window.focused(cx).is_none()
            || KEYBOARD_FEEDBACK_FIXTURE_ENQUEUED.swap(true, Ordering::AcqRel)
        {
            return;
        }

        let first = crate::components::Toast::new(
            "Duplicate status",
            crate::components::ToastColors::from_theme(
                self.theme.as_ref(),
                crate::components::ToastVariant::Info,
            ),
        )
        .with_id("ux16-runtime-a")
        .variant(crate::components::ToastVariant::Info)
        .persistent()
        .action(crate::components::ToastAction::new(
            "open-local",
            "Open",
            Box::new(|_, _, _| logging::log("UX16_FIXTURE", "action=open-local")),
        ))
        .action(crate::components::ToastAction::new(
            "open-details",
            "Open",
            Box::new(|_, _, _| logging::log("UX16_FIXTURE", "action=open-details")),
        ))
        .on_dismiss(Box::new(|_, _| {
            logging::log("UX16_FIXTURE", "dismiss=ux16-runtime-a")
        }));
        let second = crate::components::Toast::new(
            "Duplicate status",
            crate::components::ToastColors::from_theme(
                self.theme.as_ref(),
                crate::components::ToastVariant::Warning,
            ),
        )
        .with_id("ux16-runtime-b")
        .variant(crate::components::ToastVariant::Warning)
        .persistent()
        .action(crate::components::ToastAction::new(
            "open-remote",
            "Open",
            Box::new(|_, _, _| logging::log("UX16_FIXTURE", "action=open-remote")),
        ));

        self.toast_manager.push(first);
        self.toast_manager.push(second);
    }

    /// Flush pending toasts from ToastManager to gpui-component's NotificationList
    ///
    /// This should be called at the start of render() where we have window access.
    /// The ToastManager acts as a staging queue for toasts pushed from callbacks
    /// that don't have window access.
    pub(crate) fn flush_pending_toasts(&mut self, window: &mut gpui::Window, cx: &mut gpui::App) {
        use gpui_component::WindowExt;

        self.enqueue_keyboard_feedback_fixture_if_requested(window, cx);
        let pending = self.toast_manager.drain_pending();
        let count = pending.len();
        if count > 0 {
            logging::log(
                "UI",
                &format!("Flushing {} pending toast(s) to NotificationList", count),
            );
        }
        for toast in pending {
            logging::log("UI", &format!("Pushing notification: {}", toast.message));
            let notification =
                crate::toast_manager::notification::pending_toast_to_notification(toast);
            window.push_notification(notification, cx);
        }
    }

    /// Close window and reset to default state (Cmd+W global handler)
    ///
    /// This method handles the global Cmd+W shortcut which should work
    /// regardless of what prompt or view is currently active. It:
    /// 1. Cancels any running script
    /// 2. Resets state to the default script list
    /// 3. Hides the window
    /// Clear owner-bound popup state and force-close detached popup windows owned
    /// by the main launcher (Agent Chat `@` composer picker and Agent Chat history).
    /// Detached windows cache a `WeakEntity` of the owner view but
    /// rely on the owner to explicitly close them on lifecycle / surface
    /// transitions. Whenever we hide the main window, return to ScriptList, or
    /// otherwise abandon the owner surface, call this to guarantee they cannot
    /// survive past their owner.
    pub(crate) fn close_floating_popups_for_owner_loss(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.menu_syntax_trigger_picker_state = Default::default();
        crate::ai::agent_chat::ui::history_popup::close_history_popup_window_for_owner_loss(cx);
        tracing::info!(
            target: "script_kit::popup_owner",
            event = "floating_popups_closed_for_owner_loss",
            reason,
        );
    }

    fn prepare_main_window_close(
        &mut self,
        cx: &mut Context<Self>,
        reason: &'static str,
        honor_day_page_context_return: bool,
        normalize_mode_before_hide: bool,
    ) -> Option<u64> {
        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        // Today → main-menu `@context` round trip: Escape/close while the
        // search is pending cancels back to Today instead of closing the
        // launcher (the second Escape then closes from Today as usual).
        if honor_day_page_context_return
            && self.day_page_context_return.is_some()
            && matches!(self.current_view, AppView::ScriptList)
        {
            self.cancel_day_page_context_round_trip_deferred(cx);
            return None;
        }
        logging::log("VISIBILITY", "=== Close and reset window ===");
        self.close_floating_popups_for_owner_loss(reason, cx);
        clear_main_state_restore_after_focus_loss();

        // Reset pin state when window is closed. Agent Chat defers mode
        // normalization until after AppKit confirms the panel is hidden so a
        // layout change cannot flash during dismissal.
        self.is_pinned = false;
        if normalize_mode_before_hide {
            self.set_main_window_mode_state_only(MainWindowMode::Full, cx, reason);
        }

        // Close child windows FIRST if open (they are children of main window)
        // Actions window
        if self.show_actions_popup || is_actions_window_open() {
            self.clear_actions_popup_state();
            cx.spawn(async move |_this, cx| {
                cx.update(move |cx| {
                    close_actions_window(cx);
                });
            })
            .detach();
            logging::log("VISIBILITY", "Closed actions window before hiding main");
        }

        // Save window position BEFORE hiding (main window is hidden, not closed)
        if let Some((x, y, w, h)) = crate::platform::get_main_window_bounds() {
            let bounds = crate::window_state::PersistedWindowBounds::new(x, y, w, h);
            let displays = crate::platform::get_macos_displays();
            let _ =
                crate::window_state::save_main_position_with_display_detection(bounds, &displays);
        }

        // Update visibility state FIRST to prevent race conditions
        script_kit_gpui::set_main_window_visible(false);
        crate::hotkeys::reset_main_gesture_classifier();
        self.was_window_focused = false;
        crate::windows::set_automation_visibility("main", false);
        logging::log("VISIBILITY", "WINDOW_VISIBLE set to: false");

        // If in a prompt, cancel execution without resetting the visible route.
        // The reset is deferred until after the native hide turn below, avoiding
        // a visible ScriptList frame while the panel is closing.
        if self.is_in_prompt() {
            logging::log(
                "VISIBILITY",
                "In prompt mode - canceling script before hidden reset",
            );
            self.cancel_script_execution_without_view_reset();
        }

        // Check if Notes or Agent Chat windows are open BEFORE hiding
        let notes_open = notes::is_notes_window_open();
        let agent_chat_open = ai::agent_chat::ui::chat_window::is_chat_window_open();
        logging::log(
            "VISIBILITY",
            &format!(
                "Secondary windows: notes_open={}, agent_chat_open={}",
                notes_open, agent_chat_open
            ),
        );

        // CRITICAL: Always hide only the main window. App-level hide conceals the
        // entire app (all windows), so any false-negative secondary-window
        // check can hide Notes together with main.
        // Must be deferred: orderOut: triggers window_did_change_key_status
        // synchronously, which re-enters GPUI's App RefCell and panics.
        let secondary_windows_open = notes_open || agent_chat_open;
        logging::log(
            "VISIBILITY",
            &format!(
                "Prepared main-only hide, secondary_windows_open={}",
                secondary_windows_open
            ),
        );
        Some(script_kit_gpui::main_window_visibility_generation())
    }

    /// The ONE application-level owner of an ordinary main-window dismissal.
    ///
    /// Sequence (locked exit contract, values immutable):
    /// 1. Play the calibrated fixed-frame exit fade
    ///    (`begin_main_window_exit_dematerialize`, `DetachedRegionsFadeOnly`).
    /// 2. Wait the locked removal delay (`glass_exit_remove_delay`) — zero when
    ///    glass is unavailable so non-glass hides stay instant.
    /// 3. Reject the request when a re-show or newer visibility generation
    ///    superseded it during the fade.
    /// 4. Run the native completion hide (`orderOut:` stays synchronous inside
    ///    that flow — deferring it at the platform layer livelocked hotkey
    ///    toggling), then apply `post_hide` ONLY on a confirmed
    ///    `MainWindowHideCompletion::Hidden`.
    ///
    /// Every ordinary dismissal (protocol hide, escape, focus-loss preserve,
    /// close-and-reset, Agent Chat close) must call this instead of pairing raw
    /// `platform::defer_hide_main_window` with its own reset scheduling — the
    /// raw pairing is exactly what skipped the calibrated exit on the protocol
    /// and escape routes (exit-fade regression receipts, 2026-08-13).
    pub(crate) fn defer_calibrated_main_window_hide(
        &mut self,
        cx: &mut Context<Self>,
        expected_visibility_generation: u64,
        geometry_cycle_id: Option<u64>,
        post_hide: MainWindowPostHide,
    ) {
        let app_entity = cx.entity().downgrade();
        let fade_started = platform::begin_main_window_exit_dematerialize();
        let delay = if fade_started {
            platform::glass_exit_remove_delay()
        } else {
            std::time::Duration::ZERO
        };
        cx.spawn(async move |_this, cx: &mut gpui::AsyncApp| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            let _ = cx.update(move |cx| {
                if !calibrated_hide_request_is_current(
                    expected_visibility_generation,
                    script_kit_gpui::main_window_visibility_generation(),
                    script_kit_gpui::is_main_window_visible(),
                ) {
                    logging::log(
                        "VISIBILITY",
                        &format!(
                            "Calibrated hide superseded before native hide ({})",
                            post_hide.reason()
                        ),
                    );
                    return;
                }
                let completion = move |outcome: crate::platform::MainWindowHideCompletion,
                                       cx: &mut gpui::AsyncApp| {
                    match outcome {
                        crate::platform::MainWindowHideCompletion::Hidden(native_hidden) => {
                            crate::footer_popup::close_main_footer_popup_after_hidden_settle(
                                cx,
                                expected_visibility_generation,
                            );
                            match post_hide {
                                MainWindowPostHide::PreserveState { .. } => {}
                                MainWindowPostHide::ResetScriptList {
                                    reason,
                                    reset_mini_bounds_after_hidden_reset,
                                } => {
                                    let request = HiddenMainWindowResetRequest {
                                        _native_hidden: native_hidden,
                                        visibility_generation: expected_visibility_generation,
                                        reason,
                                        reset_mini_bounds_after_hidden_reset,
                                    };
                                    let _ = cx.update(|cx| {
                                        if let Some(app_entity) = app_entity.upgrade() {
                                            app_entity.update(cx, |app, cx| {
                                                app.complete_hidden_main_window_script_list_reset(
                                                    request, cx,
                                                );
                                            });
                                        }
                                    });
                                }
                                MainWindowPostHide::ResetPreparedAgentChat { reason } => {
                                    let request = HiddenMainWindowResetRequest {
                                        _native_hidden: native_hidden,
                                        visibility_generation: expected_visibility_generation,
                                        reason,
                                        reset_mini_bounds_after_hidden_reset: false,
                                    };
                                    let _ = cx.update(|cx| {
                                        if let Some(app_entity) = app_entity.upgrade() {
                                            app_entity.update(cx, |app, cx| {
                                                app.complete_hidden_main_window_reset(request, cx);
                                            });
                                        }
                                    });
                                }
                            }
                        }
                        failure => {
                            logging::log(
                                "VISIBILITY",
                                &format!(
                                    "Calibrated hide barrier failed closed ({}): {failure:?}",
                                    post_hide.reason()
                                ),
                            );
                        }
                    }
                };
                if let Some(cycle_id) = geometry_cycle_id {
                    platform::defer_hide_main_window_with_geometry_trace_and_completion(
                        cx,
                        expected_visibility_generation,
                        cycle_id,
                        completion,
                    );
                } else {
                    platform::defer_hide_main_window_with_completion(
                        cx,
                        expected_visibility_generation,
                        completion,
                    );
                }
            });
        })
        .detach();
    }

    pub(crate) fn close_and_reset_window(&mut self, cx: &mut Context<Self>) {
        let Some(visibility_generation) =
            self.prepare_main_window_close(cx, "close_and_reset_window", true, false)
        else {
            return;
        };
        logging::log(
            "VISIBILITY",
            "Using calibrated main-window hide - main-only hide",
        );
        self.defer_calibrated_main_window_hide(
            cx,
            visibility_generation,
            None,
            MainWindowPostHide::ResetScriptList {
                reason: "close_and_reset_window",
                reset_mini_bounds_after_hidden_reset: false,
            },
        );
        logging::log("VISIBILITY", "=== Window closed ===");
    }

    /// Agent Chat's strict close barrier: native `orderOut:` must complete and
    /// AppKit must report the window hidden before ScriptList can be rendered.
    pub(crate) fn close_agent_chat_and_reset_window_after_native_hide(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        logging::log(
            "VISIBILITY",
            "=== Close Agent Chat with native hide/reset barrier ===",
        );
        let Some(visibility_generation) = self.prepare_main_window_close(
            cx,
            "close_agent_chat_native_hide_barrier",
            false,
            false,
        ) else {
            return;
        };
        self.defer_calibrated_main_window_hide(
            cx,
            visibility_generation,
            None,
            MainWindowPostHide::ResetPreparedAgentChat {
                reason: "close_agent_chat_native_hide_barrier",
            },
        );
    }

    fn complete_hidden_main_window_reset(
        &mut self,
        request: HiddenMainWindowResetRequest,
        cx: &mut Context<Self>,
    ) {
        if !hidden_main_window_reset_is_current(
            request.visibility_generation,
            script_kit_gpui::main_window_visibility_generation(),
            script_kit_gpui::is_main_window_visible(),
        ) {
            logging::log(
                "VISIBILITY",
                &format!(
                    "Skipping stale Agent Chat hidden reset after {}",
                    request.reason
                ),
            );
            return;
        }

        logging::log(
            "VISIBILITY",
            &format!(
                "Resetting prepared Agent Chat in hidden main window to ScriptList after {}",
                request.reason
            ),
        );
        let was_mini = self.main_window_mode == MainWindowMode::Mini;
        self.reset_to_script_list_after_agent_chat_prepared(cx);
        let post_reset_is_mini = self.main_window_mode == MainWindowMode::Mini;
        self.rekey_main_automation_surface_from_current_view();
        crate::windows::set_automation_visibility("main", false);
        let hidden_reset_is_mini = was_mini || post_reset_is_mini;
        if request.reset_mini_bounds_after_hidden_reset || hidden_reset_is_mini {
            crate::window_resize::resize_to_mini_main_window_sync();
        }
    }

    /// Reset the ordinary launcher route only after AppKit has confirmed the
    /// native main window hidden. Changing mode or rendering ScriptList during
    /// the fade produces a duplicate in-window footer beside the still-visible
    /// detached footer.
    fn complete_hidden_main_window_script_list_reset(
        &mut self,
        request: HiddenMainWindowResetRequest,
        cx: &mut Context<Self>,
    ) {
        if !hidden_main_window_reset_is_current(
            request.visibility_generation,
            script_kit_gpui::main_window_visibility_generation(),
            script_kit_gpui::is_main_window_visible(),
        ) {
            logging::log(
                "VISIBILITY",
                &format!(
                    "Skipping stale hidden main window reset after {}",
                    request.reason
                ),
            );
            return;
        }

        let hidden_reset_is_mini = self.reset_hidden_main_window_to_script_list(cx, request.reason);
        if request.reset_mini_bounds_after_hidden_reset || hidden_reset_is_mini {
            crate::window_resize::resize_to_mini_main_window_sync();
        }
    }

    pub(crate) fn reset_hidden_main_window_to_script_list(
        &mut self,
        cx: &mut Context<Self>,
        reason: &'static str,
    ) -> bool {
        logging::log(
            "VISIBILITY",
            &format!("Resetting hidden main window to ScriptList after {reason}"),
        );
        let was_mini = self.main_window_mode == MainWindowMode::Mini;
        self.reset_to_script_list(cx);
        let post_reset_is_mini = self.main_window_mode == MainWindowMode::Mini;
        self.rekey_main_automation_surface_from_current_view();
        crate::windows::set_automation_visibility("main", false);
        was_mini || post_reset_is_mini
    }

    pub(crate) fn can_preserve_hide_script_list_on_passive_focus_loss(&self) -> bool {
        matches!(self.current_view, AppView::ScriptList)
            && self.is_dismissable_view()
            && script_kit_gpui::is_main_window_visible()
            && !self.is_pinned
            && !confirm::is_confirm_window_open()
            && !ai::agent_chat::ui::chat_window::is_chat_window_open()
            && !crate::dictation::is_dictation_overlay_open()
            && !crate::dictation::is_dictation_recording()
            && self.tab_ai_save_offer_state.is_none()
            && self.shortcut_recorder_state.is_none()
    }

    /// Hide a clicked-away ScriptList without clearing the user's filter/list state.
    pub(crate) fn hide_main_window_preserving_state_for_focus_loss(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        logging::log(
            "VISIBILITY",
            "=== Hide main window after focus loss, preserving ScriptList state ===",
        );
        self.close_floating_popups_for_owner_loss("focus_loss_hide", cx);

        if !matches!(self.current_view, AppView::ScriptList) {
            logging::log(
                "VISIBILITY",
                "Preserve-state focus-loss hide requested outside ScriptList; falling back to reset close",
            );
            self.close_and_reset_window(cx);
            return;
        }

        self.is_pinned = false;

        if let Some((x, y, w, h)) = crate::platform::get_main_window_bounds() {
            let bounds = crate::window_state::PersistedWindowBounds::new(x, y, w, h);
            let displays = crate::platform::get_macos_displays();
            let _ =
                crate::window_state::save_main_position_with_display_detection(bounds, &displays);
        }

        script_kit_gpui::set_main_window_visible(false);
        self.was_window_focused = false;
        mark_main_state_restore_after_focus_loss();

        let notes_open = notes::is_notes_window_open();
        let agent_chat_open = ai::agent_chat::ui::chat_window::is_chat_window_open();
        logging::log(
            "VISIBILITY",
            &format!(
                "Secondary windows: notes_open={}, agent_chat_open={}",
                notes_open, agent_chat_open
            ),
        );

        let secondary_windows_open = notes_open || agent_chat_open;
        logging::log(
            "VISIBILITY",
            &format!(
                "Using calibrated main-window hide - preserving state with main-only hide, secondary_windows_open={}",
                secondary_windows_open
            ),
        );
        let visibility_generation = script_kit_gpui::main_window_visibility_generation();
        self.defer_calibrated_main_window_hide(
            cx,
            visibility_generation,
            None,
            MainWindowPostHide::PreserveState {
                reason: "focus_loss_preserve_state",
            },
        );

        logging::log(
            "VISIBILITY",
            "=== Main window hidden after focus loss without resetting ScriptList ===",
        );
    }

    /// Clear the current built-in view's filter/query text if non-empty.
    ///
    /// Returns `true` if the filter was cleared (caller should stop processing ESC).
    /// Returns `false` if the filter was already empty (caller should proceed with go_back_or_close).
    ///
    /// This implements the "ESC clears filter first" UX pattern that matches the main menu behavior.
    pub(crate) fn clear_builtin_view_filter(&mut self, cx: &mut Context<Self>) -> bool {
        let cleared = match &self.current_view {
            AppView::ClipboardHistoryView { filter, .. } if !filter.is_empty() => {
                Some("ClipboardHistory filter")
            }
            AppView::EmojiPickerView { filter, .. } if !filter.is_empty() => {
                Some("EmojiPicker filter")
            }
            AppView::AppLauncherView { filter, .. } if !filter.is_empty() => {
                Some("AppLauncher filter")
            }
            AppView::WindowSwitcherView { filter, .. } if !filter.is_empty() => {
                Some("WindowSwitcher filter")
            }
            AppView::BrowserTabsView { filter, .. } if !filter.is_empty() => {
                Some("BrowserTabs filter")
            }
            AppView::ProcessManagerView { filter, .. } if !filter.is_empty() => {
                Some("ProcessManager filter")
            }
            AppView::FlowUxView { filter, .. } if !filter.is_empty() => Some("FlowUx filter"),
            AppView::SettingsView { filter, .. } if !filter.is_empty() => Some("Settings filter"),
            AppView::CurrentAppCommandsView { filter, .. } if !filter.is_empty() => {
                Some("CurrentAppCommands filter")
            }
            AppView::SearchAiPresetsView { filter, .. } if !filter.is_empty() => {
                Some("SearchAiPresets filter")
            }
            AppView::FavoritesBrowseView { filter, .. } if !filter.is_empty() => {
                Some("FavoritesBrowse filter")
            }
            AppView::AgentChatHistoryView { filter, .. } if !filter.is_empty() => {
                Some("AgentChatHistory filter")
            }
            AppView::BrowserHistoryView { filter, .. } if !filter.is_empty() => {
                Some("BrowserHistory filter")
            }
            AppView::DictationHistoryView { filter, .. } if !filter.is_empty() => {
                Some("DictationHistory filter")
            }
            AppView::NotesBrowseView { search } if !search.query.is_empty() => {
                Some("NotesBrowse filter")
            }
            AppView::ThemeChooserView { filter, .. } if !filter.is_empty() => {
                Some("ThemeChooser filter")
            }
            AppView::ScriptTemplateCatalogView { filter, .. } if !filter.is_empty() => {
                Some("ScriptTemplateCatalog filter")
            }
            AppView::TipsView { filter, .. } if !filter.is_empty() => Some("Tips filter"),
            AppView::FileSearchView { query, .. } if !query.is_empty() => Some("FileSearch query"),
            _ => None,
        };
        let Some(cleared) = cleared else {
            return false;
        };
        logging::log("KEY", &format!("ESC - clearing {}", cleared));

        // Clear shared filter state (for views using the shared input component)
        self.filter_text.clear();
        self.pending_filter_sync = true;

        // Clear view-specific filter and reset selection
        match &mut self.current_view {
            AppView::ClipboardHistoryView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.clipboard_list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
                // Update focused entry to first entry (filter cleared = show all)
                self.focused_clipboard_entry_id =
                    self.cached_clipboard_entries.first().map(|e| e.id.clone());
            }
            AppView::AppLauncherView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
            }
            AppView::EmojiPickerView {
                filter,
                selected_index,
                ..
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.emoji_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
            }
            AppView::WindowSwitcherView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.window_list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
            }
            AppView::BrowserTabsView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.browser_tabs_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
            }
            AppView::ProcessManagerView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.process_list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
            }
            AppView::FlowUxView {
                filter,
                selected_index,
                ..
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.flow_ux_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
            }
            AppView::CurrentAppCommandsView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.current_app_commands_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
            }
            AppView::SettingsView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.builtin_row_stack_scroll_handle.scroll_to_item(0);
            }
            AppView::SearchAiPresetsView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.builtin_row_stack_scroll_handle.scroll_to_item(0);
            }
            AppView::FavoritesBrowseView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.builtin_row_stack_scroll_handle.scroll_to_item(0);
            }
            AppView::AgentChatHistoryView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.agent_chat_history_scroll_handle
                    .scroll_to_top_of_item(0);
            }
            AppView::BrowserHistoryView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.browser_history_scroll_handle.scroll_to_top_of_item(0);
            }
            AppView::DictationHistoryView {
                filter,
                selected_index,
                visible_limit,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                *visible_limit = crate::dictation::DICTATION_HISTORY_PAGE_SIZE;
                self.dictation_history_previous_page = None;
                self.dictation_history_scroll_handle
                    .scroll_to_top_of_item(0);
            }
            AppView::NotesBrowseView { search } => {
                search.refresh(String::new(), &crate::notes::notes_brain_days_dir());
                self.notes_browse_scroll_handle.scroll_to_top_of_item(0);
            }
            AppView::ThemeChooserView {
                filter,
                selected_index,
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.theme_chooser_list_state.scroll_to(ListOffset {
                    item_ix: 0,
                    offset_in_item: px(0.),
                });
            }
            AppView::ScriptTemplateCatalogView {
                filter,
                selected_index,
                ..
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
            }
            AppView::TipsView {
                filter,
                selected_index,
                ..
            } => {
                Self::clear_builtin_query_state(filter, selected_index);
                self.tips_list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
            }
            AppView::FileSearchView {
                query,
                selected_index,
                ..
            } => {
                Self::clear_builtin_query_state(query, selected_index);
                // Cancel any pending search
                self.file_search_debounce_task = None;
                self.file_search_loading = false;
                self.file_search_current_dir = None;
                self.file_search_current_dir_show_hidden = false;
                self.cached_file_results.clear();
                self.file_search_display_indices.clear();
                self.file_search_preview_thumbnail = FileSearchThumbnailPreviewState::Idle;
                self.file_search_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
            }
            _ => {}
        }

        // Clear hover state to prevent stale highlights after filter change
        self.hovered_index = None;

        cx.notify();
        true
    }

    pub(crate) fn reset_script_list_filter_state(&mut self) {
        self.filter_text.clear();
        self.computed_filter_text.clear();
        self.filter_coalescer.reset();
        self.pending_filter_sync = true;
    }

    pub(crate) fn reset_script_list_selection_state(&mut self, cx: &mut Context<Self>) {
        self.invalidate_grouped_cache();
        self.sync_list_state();
        self.selected_index = 0;
        self.hovered_index = None;
        self.validate_selection_bounds(cx);
        self.main_list_state.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.),
        });
        self.last_scrolled_index = Some(0);
    }

    pub(crate) fn reset_script_list_filter_and_selection_state(&mut self, cx: &mut Context<Self>) {
        self.reset_script_list_filter_state();
        self.reset_script_list_selection_state(cx);
    }

    pub(crate) fn request_script_list_main_filter_focus(&mut self, cx: &mut Context<Self>) {
        self.focused_input = FocusedInput::MainFilter;
        self.request_focus(FocusTarget::MainFilter, cx);
    }

    /// Go back to main menu or close window depending on how the view was opened.
    ///
    /// If the current built-in view was opened from the main menu, this returns to the
    /// mini main menu (ScriptList in mini mode). If it was opened directly via hotkey or
    /// protocol command,
    /// this closes the window entirely.
    ///
    /// This provides consistent UX: pressing ESC always "goes back" one step.
    pub(crate) fn go_back_or_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.opened_from_main_menu {
            logging::log(
                "KEY",
                "ESC - returning to main menu (opened from main menu)",
            );
            // Stop process manager refresh if it was running
            self.stop_process_manager_refresh();
            // Return to the mini main menu
            self.reset_to_script_list(cx);
            // Reset the flag since we're now in main menu
            self.opened_from_main_menu = false;

            // Sync input and reset the root launcher discovery placeholder.
            self.gpui_input_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
                state.set_selection(0, 0, window, cx);
                state.set_placeholder(crate::ROOT_LAUNCHER_PLACEHOLDER, window, cx);
            });

            // Clear actions popup state (prevents stale overlay on return to menu)
            self.clear_actions_popup_state();

            self.update_window_size_deferred(window, cx);
            self.request_script_list_main_filter_focus(cx);
        } else {
            logging::log(
                "KEY",
                "ESC - closing window (opened directly via hotkey/protocol)",
            );
            self.close_and_reset_window(cx);
        }
    }

    pub(crate) fn mark_opened_from_main_menu(&mut self, reason: &'static str) {
        logging::log("NAV", &format!("launch_origin=main_menu reason={reason}"));
        self.opened_from_main_menu = true;
    }

    pub(crate) fn mark_opened_directly(&mut self, reason: &'static str) {
        logging::log("NAV", &format!("launch_origin=direct reason={reason}"));
        self.opened_from_main_menu = false;
    }

    pub(crate) fn clear_menu_origin_after_script_list_confirm_cancel(
        &mut self,
        reason: &'static str,
    ) {
        if !self.opened_from_main_menu || !matches!(self.current_view, AppView::ScriptList) {
            return;
        }

        if self.is_in_attachment_portal() || self.computed_filter_text.starts_with("vault: ") {
            logging::log(
                "NAV",
                &format!(
                    "confirm_cancel_preserved_menu_origin reason={reason} portal={} filter={}",
                    self.is_in_attachment_portal(),
                    self.computed_filter_text
                ),
            );
            return;
        }

        logging::log(
            "NAV",
            &format!("confirm_cancel_cleared_menu_origin reason={reason}"),
        );
        self.opened_from_main_menu = false;
    }
}

#[cfg(all(test, unix))]
mod lifecycle_reset_unix_tests {
    use super::{hidden_main_window_reset_is_current, process_group_id_from_pid};

    #[test]
    fn hidden_reset_requires_same_hidden_visibility_generation() {
        assert!(hidden_main_window_reset_is_current(11, 11, false));
        assert!(!hidden_main_window_reset_is_current(11, 12, false));
        assert!(!hidden_main_window_reset_is_current(11, 11, true));
    }

    #[test]
    fn test_process_group_id_from_pid_rejects_out_of_range_u32() {
        let err = process_group_id_from_pid(u32::MAX).expect_err("u32::MAX should be rejected");
        assert!(err.contains("out of range"));
    }
}

#[cfg(test)]
mod calibrated_hide_tests {
    /// The shared calibrated-hide owner may perform its native hide only while
    /// it is still the CURRENT request: same visibility generation, still
    /// logically hidden. A re-show or newer generation supersedes it.
    #[test]
    fn calibrated_hide_request_currency_matches_the_supersede_contract() {
        // same generation + logically hidden => current
        assert!(super::calibrated_hide_request_is_current(7, 7, false));
        // changed generation + logically hidden => superseded
        assert!(!super::calibrated_hide_request_is_current(7, 8, false));
        // same generation + logically visible (re-shown) => superseded
        assert!(!super::calibrated_hide_request_is_current(7, 7, true));
    }
}
