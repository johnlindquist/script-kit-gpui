// Calibrated Notes entry reveal and native-resize unlock lifecycle.
#[derive(Clone, Debug)]
struct NotesNativeEntryConfig {
    window_number: i64,
    configured: bool,
    backdrop_found_or_created: bool,
    native_selectors_supported: bool,
    style_applied: bool,
    style_signature: String,
    configured_at_monotonic_ns: u64,
    configured_at_unix_ms: u64,
    settle_duration_ms: u64,
    settled_crossing_delay_ms: u64,
    /// Material-safe reveal anchor: max(geometric crossing, alpha ramp).
    content_reveal_delay_ms: u64,
    morph_started: bool,
    morph_start_alpha_bits: Option<u64>,
}

fn notes_entry_owner_is_current(
    handle: gpui::WindowHandle<Root>,
    notes_app: &gpui::WeakEntity<NotesApp>,
) -> bool {
    let current_handle = NOTES_WINDOW
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| *guard);
    let current_entity_id = NOTES_APP_ENTITY
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|entity| entity.entity_id()));
    current_handle == Some(handle) && current_entity_id == Some(notes_app.entity_id())
}

fn notes_entry_lifecycle_is_open() -> bool {
    NOTES_EXIT_TICKET
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .is_none()
}

#[cfg(target_os = "macos")]
fn notes_native_window_number(window: &gpui::Window) -> Option<i64> {
    unsafe {
        let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
        let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            return None;
        };
        let ns_view = appkit.ns_view.as_ptr() as id;
        let ns_window: id = msg_send![ns_view, window];
        (ns_window != nil).then(|| msg_send![ns_window, windowNumber])
    }
}

#[cfg(not(target_os = "macos"))]
fn notes_native_window_number(_window: &gpui::Window) -> Option<i64> {
    None
}

// Executor wakeups are not proof that the native host-clock anchor has passed.
// Keep both entry deadlines in the same clock domain as their reveal receipts.
async fn wait_for_notes_entry_deadline(executor: &gpui::BackgroundExecutor, target_ns: u64) {
    loop {
        let remaining_ns = target_ns.saturating_sub(crate::platform::host_clock::host_time_ns());
        if remaining_ns == 0 {
            return;
        }
        executor
            .timer(std::time::Duration::from_nanos(remaining_ns))
            .await;
    }
}

/// Schedule the native-resize unlock for the calibrated entry morph.
///
/// The unlock anchor is `configured_at + settle_duration_ms` — the FULL glass
/// settle, deliberately later than the body-reveal crossing
/// (`settled_crossing_delay_ms`) that `schedule_notes_entry_reveal` uses. The
/// text reveal begins while the window is still compressing/rebounding;
/// unlocking there would let a user drag fight the rebound. Stale generations,
/// replaced windows, and active exit tickets are all rejected — both here and
/// again by the phase transition table in `resize.rs`.
fn schedule_notes_resize_unlock(
    handle: gpui::WindowHandle<Root>,
    notes_app: Entity<NotesApp>,
    native: Option<&NotesNativeEntryConfig>,
    cx: &mut App,
) {
    let weak = notes_app.downgrade();
    if !notes_entry_owner_is_current(handle, &weak) || !notes_entry_lifecycle_is_open() {
        return;
    }
    let fallback_used = native.is_none_or(|config| !config.configured);
    let settle_duration_ms = if fallback_used {
        // Same bounded fallback delay the reveal path uses when native
        // configuration failed.
        250
    } else {
        native.map(|config| config.settle_duration_ms).unwrap_or(0)
    };
    let configured_at_monotonic_ns = native
        .map(|config| config.configured_at_monotonic_ns)
        .unwrap_or_else(crate::platform::host_clock::host_time_ns);
    let unlock_target_ns =
        resize::native_resize_unlock_target_ns(configured_at_monotonic_ns, settle_duration_ms);
    // Entry deadlines do not own the window. Closing Notes must release the
    // entity even when its calibrated unlock timer has not elapsed yet.
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        wait_for_notes_entry_deadline(cx.background_executor(), unlock_target_ns).await;
        cx.update(|cx| {
            if !notes_entry_owner_is_current(handle, &weak) || !notes_entry_lifecycle_is_open() {
                return;
            }
            let _ = handle.update(cx, |_root, window, cx| {
                let _ = weak.update(cx, |app, _cx| {
                    app.unlock_native_resize_after_entry(window);
                });
            });
        });
    })
    .detach();
}

fn schedule_notes_entry_reveal(
    handle: gpui::WindowHandle<Root>,
    notes_app: Entity<NotesApp>,
    native: Option<NotesNativeEntryConfig>,
    cx: &mut App,
) {
    let weak = notes_app.downgrade();
    if !notes_entry_owner_is_current(handle, &weak) || !notes_entry_lifecycle_is_open() {
        return;
    }
    let fallback_used = native.as_ref().is_none_or(|config| !config.configured);
    let expected_window_number = native.as_ref().map(|config| config.window_number);
    let settle_duration_ms = if fallback_used {
        250
    } else {
        native
            .as_ref()
            .map(|config| config.settle_duration_ms)
            .unwrap_or(0)
    };
    let reveal_delay_ms = if fallback_used {
        settle_duration_ms
    } else {
        native
            .as_ref()
            .map(|config| config.content_reveal_delay_ms)
            .unwrap_or(0)
    };
    let configured_at_monotonic_ns = native
        .as_ref()
        .map(|config| config.configured_at_monotonic_ns)
        .unwrap_or_else(crate::platform::host_clock::host_time_ns);
    let generation = notes_app.update(cx, |app, cx| {
        let generation = app.entry_reveal.generation;
        if let Some(native) = native.as_ref() {
            app.entry_reveal.record_native_configuration(
                native.window_number,
                native.configured,
                native.backdrop_found_or_created,
                native.native_selectors_supported,
                native.style_applied,
                fallback_used,
                native.style_signature.clone(),
                native.configured_at_monotonic_ns,
                native.configured_at_unix_ms,
                settle_duration_ms,
                reveal_delay_ms,
                native.morph_started,
                native.morph_start_alpha_bits,
            );
        } else {
            app.entry_reveal.record_native_configuration(
                -1,
                false,
                false,
                false,
                false,
                true,
                "unavailable".to_string(),
                configured_at_monotonic_ns,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                settle_duration_ms,
                reveal_delay_ms,
                false,
                None,
            );
        }
        let configured = app
            .entry_reveal
            .advance(generation, NotesEntryRevealPhase::AwaitingPostConfigFrame);
        debug_assert!(configured, "Notes entry must begin hidden");
        cx.notify();
        generation
    });
    let _ = update_notes_window_detached(handle, cx, |window, _cx| {
        window.on_next_frame(move |window, cx| {
            if !notes_entry_owner_is_current(handle, &weak)
                || !notes_entry_lifecycle_is_open()
                || expected_window_number
                    .filter(|number| *number > 0)
                    .is_some_and(|number| notes_native_window_number(window) != Some(number))
            {
                return;
            }
            let should_settle = weak
                .update(cx, |app, cx| {
                    let first = app
                        .entry_reveal
                        .advance(generation, NotesEntryRevealPhase::Settling);
                    if first {
                        app.entry_reveal.completed_frame_count = 1;
                        app.entry_reveal.first_frame_at_monotonic_ns =
                            Some(crate::platform::host_clock::host_time_ns());
                        cx.notify();
                    }
                    first
                })
                .unwrap_or(false);
            if !should_settle {
                return;
            }
            let weak = weak.clone();
            let any_handle = window.window_handle();
            // The timer is anchored to native configuration, not this GPUI
            // callback. That aligns body reveal with phase one's first crossing
            // of the final window size even if the callback itself arrives late.
            let reveal_target_ns = configured_at_monotonic_ns
                .saturating_add(reveal_delay_ms.saturating_mul(1_000_000));
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                wait_for_notes_entry_deadline(cx.background_executor(), reveal_target_ns).await;
                cx.update(|cx| {
                    if !notes_entry_owner_is_current(handle, &weak)
                        || !notes_entry_lifecycle_is_open()
                    {
                        return;
                    }
                    let awaiting_reveal = weak
                        .update(cx, |app, cx| {
                            if app
                                .entry_reveal
                                .advance(generation, NotesEntryRevealPhase::AwaitingRevealFrame)
                            {
                                let now = crate::platform::host_clock::host_time_ns();
                                app.entry_reveal.reveal_anchor_at_monotonic_ns = Some(now);
                                app.entry_reveal.reveal_requested_at_monotonic_ns = Some(now);
                                cx.notify();
                                true
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false);
                    if !awaiting_reveal {
                        return;
                    }
                    let _ = any_handle.update(cx, |_root, window, _cx| {
                        window.on_next_frame(move |_window, cx| {
                            if !notes_entry_owner_is_current(handle, &weak)
                                || !notes_entry_lifecycle_is_open()
                                || expected_window_number
                                    .filter(|number| *number > 0)
                                    .is_some_and(|number| {
                                        notes_native_window_number(_window) != Some(number)
                                    })
                            {
                                return;
                            }
                            let _ = weak.update(cx, |app, cx| {
                                if app
                                    .entry_reveal
                                    .advance(generation, NotesEntryRevealPhase::Visible)
                                {
                                    app.entry_reveal.completed_frame_count = 2;
                                    app.entry_reveal.visible_at_monotonic_ns =
                                        Some(crate::platform::host_clock::host_time_ns());
                                    cx.notify();
                                }
                            });
                        });
                        window.request_animation_frame();
                    });
                });
            })
            .detach();
        });
        window.request_animation_frame();
    });
}
