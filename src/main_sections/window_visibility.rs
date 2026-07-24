// ============================================================================
// WINDOW SHOW/HIDE HELPERS
// ============================================================================
// These helpers consolidate duplicated window show/hide logic that was
// scattered across hotkey handler, tray menu, stdin commands, and fallback.
// All show/hide paths should use these helpers for consistency.

static MAIN_WINDOW_GEOMETRY_TRACE_CYCLE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn next_main_window_geometry_trace_cycle_id() -> u64 {
    MAIN_WINDOW_GEOMETRY_TRACE_CYCLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

fn automation_window_bounds_from_gpui(
    bounds: gpui::Bounds<gpui::Pixels>,
) -> crate::protocol::AutomationWindowBounds {
    crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    }
}

fn current_main_automation_bounds() -> Option<crate::protocol::AutomationWindowBounds> {
    crate::platform::get_main_window_bounds().map(|(x, y, width, height)| {
        crate::protocol::AutomationWindowBounds {
            x,
            y,
            width,
            height,
        }
    })
}

fn sync_main_automation_window(
    bounds: Option<crate::protocol::AutomationWindowBounds>,
    visible: bool,
    focused: bool,
) {
    let Some(handle) = crate::get_main_window_handle() else {
        tracing::debug!(
            target: "script_kit::automation",
            visible,
            focused,
            "automation.main_window_sync_skipped_missing_handle"
        );
        return;
    };

    // Preserve whatever `semantic_surface` the subview-routing path already
    // stamped onto the `main` registry entry (via
    // `update_automation_semantic_surface` after a `triggerBuiltin` arm).
    // If this upsert silently hardcoded `"scriptList"`, any window-resize /
    // focus-change that follows a subview transition would clobber the
    // re-key — the subview would appear on screen yet the automation
    // surface would report `scriptList` the next time
    // `listAutomationWindows` ran. Falling back to `"scriptList"` on fresh
    // entries keeps the default for the initial open path.
    let preserved_surface = crate::windows::resolve_automation_window(Some(
        &crate::protocol::AutomationWindowTarget::Id {
            id: "main".to_string(),
        },
    ))
    .ok()
    .and_then(|info| info.semantic_surface)
    .or_else(|| Some("scriptList".to_string()));

    crate::windows::upsert_runtime_window_handle("main", handle);
    crate::windows::upsert_automation_window(crate::protocol::AutomationWindowInfo {
        id: "main".to_string(),
        kind: crate::protocol::AutomationWindowKind::Main,
        title: Some("Script Kit".to_string()),
        focused,
        visible,
        semantic_surface: preserved_surface,
        bounds: bounds.or_else(current_main_automation_bounds),
        parent_window_id: None,
        parent_kind: None,
        pid: Some(std::process::id()),
    });
}

fn clamp_restored_main_window_bounds_to_visible_area(
    bounds: gpui::Bounds<gpui::Pixels>,
    display: &crate::windows::DisplayBounds,
    visible_displays: &[platform::VisibleDisplayBounds],
) -> gpui::Bounds<gpui::Pixels> {
    let display_key = window_state::display_key(display);
    visible_displays
        .iter()
        .find(|candidate| window_state::display_key(&candidate.frame) == display_key)
        .map(|candidate| platform::clamp_to_visible(bounds, &candidate.visible_area))
        .unwrap_or(bounds)
}

fn resync_main_window_gpui_bounds_after_native_show_move(window: WindowHandle<Root>, cx: &mut App) {
    resync_main_window_gpui_bounds_after_native_show_move_for_trace(window, cx, None);
}

fn gpui_bounds_summary(bounds: gpui::Bounds<gpui::Pixels>) -> String {
    let x: f64 = bounds.origin.x.into();
    let y: f64 = bounds.origin.y.into();
    let width: f64 = bounds.size.width.into();
    let height: f64 = bounds.size.height.into();
    format!("{x:.1},{y:.1},{width:.1}x{height:.1}")
}

fn gpui_size_summary(size: gpui::Size<gpui::Pixels>) -> String {
    let width: f64 = size.width.into();
    let height: f64 = size.height.into();
    format!("{width:.1}x{height:.1}")
}

fn trace_main_window_gpui_geometry(
    phase: &'static str,
    cycle_id: u64,
    window: &gpui::Window,
    cx: &App,
) {
    if !platform::main_window_geometry_trace_enabled() {
        return;
    }
    let bounds = window.bounds();
    let viewport_size = window.viewport_size();
    let display_id = window
        .display(cx)
        .map(|display| format!("{:?}", display.id()))
        .unwrap_or_else(|| "none".to_string());
    tracing::info!(
        event_type = "main_window_geometry",
        layer = "gpui",
        phase,
        cycle_id,
        bounds = %gpui_bounds_summary(bounds),
        viewport_size = %gpui_size_summary(viewport_size),
        scale_factor = window.scale_factor(),
        display_id = %display_id,
        "main window geometry GPUI snapshot"
    );
    logging::log(
        "WINDOW_GEOM",
        &format!(
            "SK_GEOM cycle={cycle_id} phase={phase} gpui display={display_id} scale={:.2} bounds={}",
            window.scale_factor(),
            gpui_bounds_summary(bounds),
        ),
    );
}

fn resync_main_window_gpui_bounds_after_native_show_move_for_trace(
    window: WindowHandle<Root>,
    cx: &mut App,
    cycle_id: Option<u64>,
) {
    resync_main_window_gpui_bounds_after_native_show_move_with_phases(
        window,
        cx,
        cycle_id,
        "before_bounds_changed",
        "after_bounds_changed",
    );
}

fn resync_main_window_gpui_bounds_after_native_show_move_with_phases(
    window: WindowHandle<Root>,
    cx: &mut App,
    cycle_id: Option<u64>,
    before_phase: &'static str,
    after_phase: &'static str,
) {
    if window
        .update(cx, |_root, win, cx| {
            if let Some(cycle_id) = cycle_id {
                trace_main_window_gpui_geometry(before_phase, cycle_id, win, cx);
            }
            // `move_first_window_to_bounds` positions the native NSWindow
            // directly. GPUI's cached viewport and display scale can otherwise
            // remain on the previous display. Run this while the window is
            // still hidden so the first visible frame cannot re-present the
            // previous display's drawable before GPUI has been marked dirty.
            win.bounds_changed(cx);
            if let Some(cycle_id) = cycle_id {
                trace_main_window_gpui_geometry(after_phase, cycle_id, win, cx);
            }
        })
        .is_err()
    {
        logging::log(
            "VISIBILITY",
            "Could not resync GPUI bounds after native main-window show move",
        );
    }
}

fn trace_main_window_decision(
    phase: &'static str,
    cycle_id: u64,
    route: &'static str,
    bounds: Option<gpui::Bounds<gpui::Pixels>>,
    detail: String,
) {
    if !platform::main_window_geometry_trace_enabled() {
        return;
    }
    let bounds_summary = bounds
        .map(gpui_bounds_summary)
        .unwrap_or_else(|| "none".to_string());
    tracing::info!(
        event_type = "main_window_geometry",
        layer = "decision",
        phase,
        cycle_id,
        route,
        bounds = %bounds_summary,
        visibility_generation = script_kit_gpui::main_window_visibility_generation(),
        detail = %detail,
        "main window geometry decision"
    );
    logging::log(
        "WINDOW_GEOM",
        &format!(
            "SK_GEOM cycle={cycle_id} phase={phase} decision route={route} bounds={bounds_summary} {detail}"
        ),
    );
}

/// Show the main window with proper positioning, panel configuration, and focus.
///
/// This is the canonical way to show the main window. It:
/// 1. Sets MAIN_WINDOW_VISIBLE state
/// 2. Moves the panel to the active space
/// 3. Consumes NEEDS_RESET and resets hidden state before sizing
///    or consumes focus-loss restore intent without normalizing ScriptList state
/// 4. Computes and applies final hidden geometry
/// 5. Configures as floating panel (first time only)
/// 6. Shows the window as a non-activating panel
/// 7. Restores focus state after the panel becomes key
///
/// # Arguments
/// * `window` - The main window handle (WindowHandle<Root>)
/// * `app_entity` - The ScriptListApp entity
/// * `cx` - The application context
fn show_main_window_helper(
    window: WindowHandle<Root>,
    app_entity: Entity<ScriptListApp>,
    cx: &mut App,
) {
    let geometry_cycle_id = next_main_window_geometry_trace_cycle_id();
    platform::trace_main_window_native_geometry(
        "show_helper_start",
        geometry_cycle_id,
        None,
        Some("show_main_window_helper"),
    );
    logging::bench_log("show_main_window_helper_start");
    logging::log("VISIBILITY", "show_main_window_helper called");

    // 1. Re-enable position saving (may have been suppressed by reset)
    window_state::allow_save();

    // 2. Move to active space (macOS)
    platform::ensure_move_to_active_space();

    // 3. Consume NEEDS_RESET BEFORE any geometry is computed so we size the hidden
    // window for the actual post-reset view instead of the stale pre-show surface.
    let needs_reset_before_show = NEEDS_RESET
        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    if needs_reset_before_show {
        logging::log(
            "VISIBILITY",
            "NEEDS_RESET was true - resetting to script list before computing show bounds",
        );
    }
    let restore_after_focus_loss = if needs_reset_before_show {
        clear_main_state_restore_after_focus_loss();
        false
    } else {
        consume_main_state_restore_after_focus_loss()
    };

    let current_bounds = platform::get_main_window_bounds();
    let current_window_width = current_bounds.map(|(_, _, width, _)| width as f32);
    logging::log(
        "POSITION_TRACE",
        &format!("Current window bounds before show: {:?}", current_bounds),
    );
    let window_size = app_entity.update(cx, |view, ctx| {
        view.was_window_focused = false;
        // Refresh the pinned "Brain Inbox" snapshot (throttled to 30s) before
        // the grouped empty-query view is computed for this show. A fresh
        // show is the one moment a full newest-first reorder is allowed.
        view.refresh_root_brain_inbox_if_stale(true, ctx);
        // Passive AX-only sniff of the source app's selected text for the
        // "Rewrite selection" hint chip. Must run at show time, while the
        // frontmost app still owns AX focus; never touches the pasteboard.
        view.refresh_shown_selection_hint(ctx);
        if needs_reset_before_show {
            view.reset_to_script_list(ctx);
        } else if restore_after_focus_loss && matches!(view.current_view, AppView::ScriptList) {
            logging::log(
                "VISIBILITY",
                "Restoring ScriptList exactly after focus-loss hide",
            );
            view.focused_input = FocusedInput::MainFilter;
            view.pending_focus = Some(FocusTarget::MainFilter);
        } else if restore_after_focus_loss
            && matches!(view.current_view, AppView::AgentChatView { .. })
        {
            // An implicit hide (Space change, focus loss) must not destroy a
            // live Agent Chat session — bring it back exactly as it was and
            // put the caret back in the composer.
            logging::log(
                "VISIBILITY",
                "Restoring embedded Agent Chat exactly after focus-loss hide",
            );
            view.request_focus(FocusTarget::AgentChat, ctx);
        } else if !matches!(view.current_view, AppView::ScriptList) {
            logging::log(
                "VISIBILITY",
                "Preparing ScriptList before showing hidden main window",
            );
            view.reset_to_script_list(ctx);
        } else {
            view.ensure_selection_at_first_item(ctx);
        }

        if matches!(view.current_view, AppView::ScriptList)
            && view.main_window_mode == MainWindowMode::Mini
        {
            let (grouped_items, _) = view.get_grouped_results_cached();
            let sizing =
                crate::window_resize::main_window_sizing_from_grouped_items(&grouped_items);
            let target = crate::window_resize::MainMenuSizingTarget(sizing);
            gpui::size(px(target.width()), target.height())
        } else if matches!(view.current_view, AppView::ScriptList) {
            let target = crate::window_resize::MainMenuSizingTarget(
                crate::window_resize::MainWindowSizing::default(),
            );
            gpui::size(px(target.width()), target.height())
        } else if let Some((view_type, item_count)) =
            view.calculate_window_size_params_with_app(Some(&*ctx))
        {
            gpui::size(
                px(crate::window_resize::width_for_view(view_type)
                    .or(current_window_width)
                    .unwrap_or(crate::window_resize::MAIN_WINDOW_WIDTH)),
                crate::window_resize::height_for_view(view_type, item_count),
            )
        } else {
            gpui::size(
                px(current_window_width.unwrap_or(crate::window_resize::MAIN_WINDOW_WIDTH)),
                crate::window_resize::height_for_view(ViewType::ScriptList, 0),
            )
        }
    });

    // 4. Only mark the main window visible after the hidden state is already
    // normalized for the first visible frame.
    set_main_window_visible(true);
    script_kit_gpui::mark_window_shown();
    hotkeys::sync_main_gesture_window_shown();

    logging::log(
        "POSITION_TRACE",
        &format!(
            "Computed window_size for show: width={:.0}, height={:.0}",
            f32::from(window_size.width),
            f32::from(window_size.height)
        ),
    );

    // Keep legacy resize state clear before re-opening. Interactive resizes still
    // use the deferred paths after the window is already visible.
    reset_resize_debounce();

    // 6. Position the hidden window using the exact target size, then show it.
    let visible_displays = platform::get_macos_visible_displays();
    let displays: Vec<_> = visible_displays
        .iter()
        .map(|display| display.frame.clone())
        .collect();
    let mouse = platform::get_global_mouse_position();
    let bounds = if let Some((mouse_x, mouse_y)) = mouse {
        // Try to restore saved position for the mouse display
        if let Some((saved, display)) =
            window_state::get_main_position_for_mouse_display(mouse_x, mouse_y, &displays)
        {
            // Validate the saved position is still visible
            if window_state::is_bounds_visible(&saved, &displays) {
                let target_width = f32::from(window_size.width) as f64;
                let width_delta = saved.width - target_width;
                logging::log(
                    "POSITION_TRACE",
                    &format!(
                        "Restoring saved position for display {}: saved=({:.0}, {:.0}, {:.0}x{:.0}), target_width={:.0}, width_delta={:.1}",
                        window_state::display_key(&display),
                        saved.x,
                        saved.y,
                        saved.width,
                        saved.height,
                        target_width,
                        width_delta
                    ),
                );
                // Re-center horizontally when the target width differs from the saved width.
                // Without this, a width change shifts the left edge, causing the window
                // to appear offset from its original center.
                let adjusted_x = saved.x + (saved.width - target_width) / 2.0;
                logging::log(
                    "POSITION_TRACE",
                    &format!(
                        "Adjusted x: saved.x={:.0} -> adjusted_x={:.0} (shift={:.1})",
                        saved.x,
                        adjusted_x,
                        adjusted_x - saved.x
                    ),
                );
                // Use saved position with current window size, clamped to the
                // selected display's visible area.
                let restored_bounds = gpui::Bounds {
                    origin: gpui::point(px(adjusted_x as f32), px(saved.y as f32)),
                    size: window_size,
                };
                let clamped_bounds = clamp_restored_main_window_bounds_to_visible_area(
                    restored_bounds,
                    &display,
                    &visible_displays,
                );
                trace_main_window_decision(
                    "show_bounds_decided",
                    geometry_cycle_id,
                    "saved_position",
                    Some(clamped_bounds),
                    format!(
                        "mouse={mouse_x:.1},{mouse_y:.1} display_key={} saved={:.1},{:.1},{:.1}x{:.1} restored={} clamped={}",
                        window_state::display_key(&display),
                        saved.x,
                        saved.y,
                        saved.width,
                        saved.height,
                        gpui_bounds_summary(restored_bounds),
                        gpui_bounds_summary(clamped_bounds),
                    ),
                );
                if clamped_bounds != restored_bounds {
                    let before_x: f64 = restored_bounds.origin.x.into();
                    let before_y: f64 = restored_bounds.origin.y.into();
                    let before_width: f64 = restored_bounds.size.width.into();
                    let before_height: f64 = restored_bounds.size.height.into();
                    let after_x: f64 = clamped_bounds.origin.x.into();
                    let after_y: f64 = clamped_bounds.origin.y.into();
                    let after_width: f64 = clamped_bounds.size.width.into();
                    let after_height: f64 = clamped_bounds.size.height.into();
                    logging::log(
                        "POSITION_TRACE",
                        &format!(
                            "Clamped saved main position: ({before_x:.0}, {before_y:.0}, {before_width:.0}x{before_height:.0}) -> ({after_x:.0}, {after_y:.0}, {after_width:.0}x{after_height:.0})",
                        ),
                    );
                }
                clamped_bounds
            } else {
                logging::log(
                    "VISIBILITY",
                    "Saved position no longer visible, using eye-line",
                );
                let bounds = platform::calculate_eye_line_bounds_for_snapshot(
                    window_size,
                    mouse,
                    &visible_displays,
                );
                trace_main_window_decision(
                    "show_bounds_decided",
                    geometry_cycle_id,
                    "saved_not_visible_eye_line",
                    Some(bounds),
                    format!("mouse={mouse_x:.1},{mouse_y:.1}"),
                );
                bounds
            }
        } else {
            logging::log(
                "VISIBILITY",
                "No saved position for this display, using eye-line",
            );
            let bounds = platform::calculate_eye_line_bounds_for_snapshot(
                window_size,
                mouse,
                &visible_displays,
            );
            trace_main_window_decision(
                "show_bounds_decided",
                geometry_cycle_id,
                "no_saved_eye_line",
                Some(bounds),
                format!("mouse={mouse_x:.1},{mouse_y:.1}"),
            );
            bounds
        }
    } else {
        logging::log("VISIBILITY", "Could not get mouse position, using eye-line");
        let bounds =
            platform::calculate_eye_line_bounds_for_snapshot(window_size, mouse, &visible_displays);
        trace_main_window_decision(
            "show_bounds_decided",
            geometry_cycle_id,
            "no_mouse_eye_line",
            Some(bounds),
            "mouse=none".to_string(),
        );
        bounds
    };
    platform::trace_main_window_native_geometry(
        "before_set_frame",
        geometry_cycle_id,
        Some(&bounds),
        Some("show_main_window_helper"),
    );
    platform::move_first_window_to_bounds(&bounds);
    platform::trace_main_window_native_geometry(
        "after_set_frame",
        geometry_cycle_id,
        Some(&bounds),
        Some("show_main_window_helper"),
    );
    resync_main_window_gpui_bounds_after_native_show_move_with_phases(
        window,
        cx,
        Some(geometry_cycle_id),
        "before_pre_reveal_bounds_changed",
        "after_pre_reveal_bounds_changed",
    );
    platform::trace_main_window_native_geometry(
        "after_pre_reveal_bounds_changed",
        geometry_cycle_id,
        None,
        Some("show_main_window_helper"),
    );

    // 7. Configure as floating panel (first time only).
    //    Oracle-Session `window-activation-invariants-guard` PR1: the helper
    //    only flips `PANEL_CONFIGURED` to `true` after a successful
    //    post-configure invariant report, so an early bail inside
    //    `configure_as_floating_panel` no longer poisons the one-shot guard.
    platform::ensure_main_panel_configured("window_visibility::show_main_window_helper");

    // 8. Show without app activation, then focus (DEFERRED via cx.spawn).
    //
    // macOS makeKeyWindow / makeKeyAndOrderFront: synchronously fires
    // windowDidBecomeKey: → GPUI request_frame_callback → AsyncApp::update().
    // If we're already inside an AsyncApp::update() (the caller's cx.update()),
    // that second borrow_mut() on the AppCell panics with "RefCell already borrowed".
    //
    // Spawning defers to the next event-loop tick where no AppCell borrow is held.
    // Platform calls that trigger delegate callbacks run OUTSIDE cx.update();
    // GPUI-only state changes (focus, selection, resize) run INSIDE cx.update().
    cx.spawn({
        let app_entity = app_entity.clone();
        async move |cx: &mut gpui::AsyncApp| {
            logging::bench_log("window_show_native_start");

            // Platform calls — trigger macOS delegate callbacks.
            // Safe here: no AppCell borrow is active.
            if platform::main_window_geometry_trace_enabled() {
                platform::show_main_window_without_activation_with_geometry_trace(
                    geometry_cycle_id,
                );
            } else {
                platform::show_main_window_without_activation();
            }
            platform::send_ai_window_to_back();
            platform::trace_main_window_native_geometry(
                "after_send_ai_back",
                geometry_cycle_id,
                None,
                None,
            );

            logging::bench_log("window_activated");

            // GPUI state changes — no macOS callbacks, safe inside borrow.
            cx.update(move |cx: &mut gpui::App| {
                resync_main_window_gpui_bounds_after_native_show_move_for_trace(
                    window,
                    cx,
                    Some(geometry_cycle_id),
                );
                platform::trace_main_window_native_geometry(
                    "after_bounds_changed",
                    geometry_cycle_id,
                    None,
                    None,
                );
                app_entity.update(cx, |view, ctx| {
                    let agent_chat_focus_handle = match &view.current_view {
                        AppView::AgentChatView { entity } => {
                            Some(entity.read(ctx).focus_handle(ctx))
                        }
                        _ => None,
                    };
                    let focus_handle = agent_chat_focus_handle
                        .clone()
                        .unwrap_or_else(|| view.focus_handle(ctx));
                    let _ = window.update(ctx, |_root, win, _cx| {
                        win.activate_window();
                        win.focus(&focus_handle, _cx);
                    });

                    // Always re-apply focus state after the window becomes visible.
                    // reset_to_script_list sets these too, but that runs BEFORE the
                    // window is shown — the render loop needs pending_focus set AFTER
                    // the window is key to actually move focus to the input element.
                    if agent_chat_focus_handle.is_some() {
                        view.focused_input = FocusedInput::None;
                        view.pending_focus = Some(FocusTarget::AgentChat);
                    } else if matches!(view.current_view, AppView::DayPage { .. }) {
                        // Hold-from-closed shows the Day Page before this deferred
                        // restore runs — keep focus on the day-page editor instead
                        // of stomping it back to the launcher filter.
                        view.focused_input = FocusedInput::None;
                        view.pending_focus = Some(FocusTarget::EditorPrompt);
                    } else {
                        view.focused_input = FocusedInput::MainFilter;
                        view.pending_focus = Some(FocusTarget::MainFilter);
                    }
                    view.maybe_show_tap_dismiss_retired_hint(ctx);
                    view.maybe_show_sediment_disclosure_hint(ctx);
                    ctx.notify();
                });
                platform::trace_main_window_native_geometry(
                    "after_focus_notify",
                    geometry_cycle_id,
                    None,
                    None,
                );

                logging::log("VISIBILITY", "Main window shown and focused");
                sync_main_automation_window(None, true, true);
            });
        }
    })
    .detach();
}

/// Hide the main window with proper state management.
///
/// This is the canonical way to hide the main window. It:
/// 1. Saves window position for the current display (per-display persistence)
/// 2. Sets MAIN_WINDOW_VISIBLE state to false
/// 3. Cancels any active prompt (if in prompt mode)
/// 4. Defers a main-panel-only hide so secondary windows cannot be app-hidden
/// 5. Resets the hidden main window to ScriptList after the hide turn
///
/// # Arguments
/// * `app_entity` - The ScriptListApp entity
/// * `cx` - The application context
fn hide_main_window_helper(app_entity: Entity<ScriptListApp>, cx: &mut App) {
    let geometry_cycle_id = next_main_window_geometry_trace_cycle_id();
    platform::trace_main_window_native_geometry(
        "before_hide_save",
        geometry_cycle_id,
        None,
        Some("hide_main_window_helper"),
    );
    logging::log("VISIBILITY", "hide_main_window_helper called");

    // 1. Save window position for the current display BEFORE hiding
    if let Some((x, y, width, height)) = platform::get_main_window_bounds() {
        let displays = platform::get_macos_displays();
        let bounds = window_state::PersistedWindowBounds::new(x, y, width, height);
        logging::log(
            "POSITION_TRACE",
            &format!(
                "Hide: saving bounds ({:.0}, {:.0}, {:.0}x{:.0})",
                x, y, width, height
            ),
        );

        // Find which display the window center is on
        if let Some(display) = window_state::find_display_for_bounds(&bounds, &displays) {
            logging::log(
                "VISIBILITY",
                &format!(
                    "Saving position for display {}: ({:.0}, {:.0})",
                    window_state::display_key(display),
                    x,
                    y
                ),
            );
            window_state::save_main_position_for_display(display, bounds);
        } else {
            logging::log(
                "VISIBILITY",
                "Could not determine display for window position",
            );
        }
    } else {
        logging::log("VISIBILITY", "Could not get window bounds for saving");
    }

    // 2. Set visibility state
    set_main_window_visible(false);
    hotkeys::reset_main_gesture_classifier();
    app_entity.update(cx, |view, _| {
        view.was_window_focused = false;
    });
    sync_main_automation_window(current_main_automation_bounds(), false, false);
    let visibility_generation = crate::main_window_visibility_generation();

    // 3. Check secondary windows BEFORE the update closure
    let notes_open = notes::is_notes_window_open();
    let agent_chat_open = ai::agent_chat::ui::chat_window::is_chat_window_open();
    logging::log(
        "VISIBILITY",
        &format!(
            "Secondary windows: notes_open={}, agent_chat_open={}",
            notes_open, agent_chat_open
        ),
    );

    // 4. Cancel prompts without resetting the visible route. The ScriptList reset
    // is deferred until after the native hide turn to avoid a visible stale-view
    // or main-menu frame while AppKit is still closing the panel.
    let reset_mini_bounds_after_hidden_reset = app_entity.update(cx, |view, _ctx| {
        let was_mini = view.main_window_mode == MainWindowMode::Mini;
        if view.is_in_prompt() {
            logging::log("VISIBILITY", "Canceling prompt before hiding");
            view.cancel_script_execution_without_view_reset();
        }
        was_mini
    });
    // Full teardown for actions-dialog (`id: "actions-dialog"`).
    // Pass #29 fix (`cmd-k-on-unfocused-clipboard-pops-overlay-not-actions`):
    // upgraded from bare `remove_automation_window("actions-dialog")` to
    // full `close_actions_window`. The bare registry op (Pass #23) left
    // the `ACTIONS_WINDOW` static `Mutex<Option<WindowHandle>>` holding a
    // stale handle after hide; a later `simulateKey cmd+k` on an unfocused
    // window read `is_actions_window_open()=true` and took the CLOSE
    // branch, popping whichever overlay was top (e.g. ClipboardHistoryView)
    // instead of opening an actions dialog that was never there.
    // `close_actions_window` clears BOTH the static AND the registry via
    // its first line (`remove_automation_window("actions-dialog")`), so
    // it is strictly stronger than the pre-#29 call. Idempotent.
    crate::actions::close_actions_window(cx);
    // Sibling teardown for confirm-popup (`id: "confirm-popup"`, PromptPopup
    // kind). Pass #25 fix: close_confirm_window at src/confirm/window.rs:385
    // is the only production path removing this id; no hide dispatcher calls
    // it (`attacker-hide-path-confirm-popup-registry-stale`). Same bypass
    // shape as the actions-dialog sibling above; pure registry op, idempotent.
    crate::windows::remove_automation_window("confirm-popup");

    // 5. Hide only the main panel. `cx.hide()` app-hides all windows, so any
    // false-negative secondary-window check can take Notes down with main.
    let secondary_windows_open = notes_open || agent_chat_open;
    logging::log(
        "VISIBILITY",
        &format!(
            "Using defer_hide_main_window() - main-only hide, secondary_windows_open={}",
            secondary_windows_open
        ),
    );
    // Glass mode: play the Spotlight-style dematerialize (~120ms fade +
    // blur + slight growth), THEN run the normal hide flow — the native
    // orderOut: itself must stay synchronous inside that flow (deferring it
    // at the platform layer livelocked the hotkey gesture listener). A
    // re-show during the animation supersedes the delayed hide.
    if platform::begin_main_window_exit_dematerialize() {
        let trace = platform::main_window_geometry_trace_enabled();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(135))
                .await;
            cx.update(move |cx| {
                if crate::is_main_window_visible() {
                    logging::log(
                        "VISIBILITY",
                        "Exit dematerialize superseded by re-show; skipping deferred hide",
                    );
                    return;
                }
                // Keep the native footer host installed for the entire window
                // fade and through native orderOut:. Removing it before
                // orderOut lets GPUI draw its fallback footer inside the
                // stage for one frame — the visible "rejoin" glitch.
                let cleanup = move |outcome, cx: &mut gpui::AsyncApp| {
                    if matches!(
                        outcome,
                        crate::platform::MainWindowHideCompletion::Hidden(_)
                    ) {
                        crate::footer_popup::close_main_footer_popup_after_hidden_settle(
                            cx,
                            visibility_generation,
                        );
                    }
                };
                if trace {
                    platform::defer_hide_main_window_with_geometry_trace_and_completion(
                        cx,
                        visibility_generation,
                        geometry_cycle_id,
                        cleanup,
                    );
                } else {
                    platform::defer_hide_main_window_with_completion(
                        cx,
                        visibility_generation,
                        cleanup,
                    );
                }
            });
        })
        .detach();
    } else if platform::main_window_geometry_trace_enabled() {
        platform::defer_hide_main_window_with_geometry_trace_and_completion(
            cx,
            visibility_generation,
            geometry_cycle_id,
            move |outcome, cx| {
                if matches!(
                    outcome,
                    crate::platform::MainWindowHideCompletion::Hidden(_)
                ) {
                    crate::footer_popup::close_main_footer_popup_after_hidden_settle(
                        cx,
                        visibility_generation,
                    );
                }
            },
        );
    } else {
        platform::defer_hide_main_window_with_completion(
            cx,
            visibility_generation,
            move |outcome, cx| {
                if matches!(
                    outcome,
                    crate::platform::MainWindowHideCompletion::Hidden(_)
                ) {
                    crate::footer_popup::close_main_footer_popup_after_hidden_settle(
                        cx,
                        visibility_generation,
                    );
                }
            },
        );
    }
    app_entity.update(cx, |view, ctx| {
        view.defer_reset_to_script_list_after_main_window_hidden(
            ctx,
            "hide_main_window_helper",
            reset_mini_bounds_after_hidden_reset,
        );
    });
    platform::trace_main_window_native_geometry(
        "after_hidden_reset_scheduled",
        geometry_cycle_id,
        None,
        Some("hide_main_window_helper"),
    );

    logging::log("VISIBILITY", "Main window hidden");
}
