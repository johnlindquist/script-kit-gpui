use super::*;
use crate::mcp_notes_tools::{
    NotesCreateArgs, NotesDeleteArgs, NotesMutationError, NotesMutationErrorCode,
    NotesMutationRequest, NotesMutationResult, NotesUpdateArgs, NOTE_BODY_MAX_BYTES,
};
use crate::theme::get_cached_theme;

#[cfg(target_os = "macos")]
const NS_WINDOW_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
#[cfg(target_os = "macos")]
const NS_WINDOW_COLLECTION_BEHAVIOR_MOVE_TO_ACTIVE_SPACE: u64 = 1 << 1;
#[cfg(target_os = "macos")]
const NS_WINDOW_COLLECTION_BEHAVIOR_IGNORES_CYCLE: u64 = 1 << 6;
#[cfg(target_os = "macos")]
const NS_WINDOW_COLLECTION_BEHAVIOR_FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

#[cfg(target_os = "macos")]
const fn notes_window_collection_behavior(current: u64) -> u64 {
    (current
        & !NS_WINDOW_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES
        & !NS_WINDOW_COLLECTION_BEHAVIOR_MOVE_TO_ACTIVE_SPACE)
        | NS_WINDOW_COLLECTION_BEHAVIOR_FULL_SCREEN_AUXILIARY
        | NS_WINDOW_COLLECTION_BEHAVIOR_IGNORES_CYCLE
}

/// Sync Script Kit theme with gpui-component theme
/// NOTE: Do NOT call gpui_component::init here - it's already called in main.rs
/// and calling it again resets the theme to system defaults (opaque backgrounds),
/// which breaks vibrancy.
fn ensure_theme_initialized(cx: &mut App) {
    // Just sync our theme colors - gpui_component is already initialized in main.rs
    crate::theme::sync_gpui_component_theme(cx);

    info!("Notes window theme synchronized with Script Kit");
}

fn hide_main_window_for_notes(cx: &mut App) {
    if !crate::is_main_window_visible() {
        return;
    }

    crate::set_main_window_visible(false);
    crate::hotkeys::reset_main_gesture_classifier();
    crate::platform::defer_hide_main_window(cx);
}

fn hide_main_window_then_activate_notes(cx: &mut App, notes_handle: gpui::WindowHandle<Root>) {
    crate::set_main_window_visible(false);
    crate::hotkeys::reset_main_gesture_classifier();
    let visibility_generation = crate::main_window_visibility_generation();

    crate::platform::defer_hide_main_window_with_completion(
        cx,
        visibility_generation,
        move |completion, cx| match completion {
            crate::platform::MainWindowHideCompletion::Hidden(_) => {
                if let Err(error) = update_notes_window_detached(notes_handle, cx, |window, _cx| {
                    window.activate_window();
                }) {
                    tracing::warn!(
                        target: "script_kit::notes",
                        %error,
                        "notes_reuse_activation_after_main_hide_failed"
                    );
                }
            }
            failure => {
                tracing::warn!(
                    target: "script_kit::notes",
                    ?failure,
                    "notes_reuse_main_hide_failed_closed"
                );
            }
        },
    );
}

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
    notes_app: &Entity<NotesApp>,
) -> bool {
    let current_handle = NOTES_WINDOW
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| *guard);
    let current_entity = NOTES_APP_ENTITY
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    current_handle == Some(handle)
        && current_entity
            .as_ref()
            .is_some_and(|current| current.entity_id() == notes_app.entity_id())
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
    if !notes_entry_owner_is_current(handle, &notes_app) || !notes_entry_lifecycle_is_open() {
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
    let unlock_delay = std::time::Duration::from_nanos(
        unlock_target_ns.saturating_sub(crate::platform::host_clock::host_time_ns()),
    );
    let owner = notes_app.clone();
    let weak = notes_app.downgrade();
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        cx.background_executor().timer(unlock_delay).await;
        cx.update(|cx| {
            if !notes_entry_owner_is_current(handle, &owner) || !notes_entry_lifecycle_is_open() {
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
    if !notes_entry_owner_is_current(handle, &notes_app) || !notes_entry_lifecycle_is_open() {
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
                crate::platform::host_clock::host_time_ns(),
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
    let weak = notes_app.downgrade();
    let owner = notes_app.clone();
    let _ = update_notes_window_detached(handle, cx, |window, _cx| {
        window.on_next_frame(move |window, cx| {
            if !notes_entry_owner_is_current(handle, &owner)
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
            let owner = owner.clone();
            let any_handle = window.window_handle();
            // The timer is anchored to native configuration, not this GPUI
            // callback. That aligns body reveal with phase one's first crossing
            // of the final window size even if the callback itself arrives late.
            let reveal_target_ns = configured_at_monotonic_ns
                .saturating_add(reveal_delay_ms.saturating_mul(1_000_000));
            let reveal_delay = std::time::Duration::from_nanos(
                reveal_target_ns.saturating_sub(crate::platform::host_clock::host_time_ns()),
            );
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                cx.background_executor().timer(reveal_delay).await;
                cx.update(|cx| {
                    if !notes_entry_owner_is_current(handle, &owner)
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
                            if !notes_entry_owner_is_current(handle, &owner)
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

/// Default Notes window bounds: top-right corner of the display the mouse
/// cursor is on (falling back to the primary display). Geometry is owned by
/// the app-authored contract (`notes::window::contract`), shared by
/// first-open placement, "Reset Window Position", and the design-contract
/// exporter.
pub(crate) fn default_notes_window_bounds() -> gpui::Bounds<gpui::Pixels> {
    calculate_top_right_bounds(
        super::contract::NOTES_DEFAULT_WIDTH,
        super::contract::NOTES_DEFAULT_HEIGHT,
        super::contract::NOTES_DEFAULT_EDGE_PADDING,
    )
}

fn calculate_top_right_bounds(width: f32, height: f32, padding: f32) -> gpui::Bounds<gpui::Pixels> {
    use crate::platform::{
        clamp_to_visible, display_for_point, get_global_mouse_position, get_macos_visible_displays,
    };

    let displays = get_macos_visible_displays();

    // Find display containing mouse
    let target_display =
        get_global_mouse_position().and_then(|mouse_pt| display_for_point(mouse_pt, &displays));

    // Use found display or fall back to primary
    let display = target_display.or_else(|| displays.first().cloned());

    if let Some(display) = display {
        let visible = &display.visible_area;

        // Position in top-right corner with padding
        let x = visible.origin_x + visible.width - width as f64 - padding as f64;
        let y = visible.origin_y + padding as f64;

        let desired_bounds = gpui::Bounds::new(
            gpui::Point::new(px(x as f32), px(y as f32)),
            gpui::Size::new(px(width), px(height)),
        );

        clamp_to_visible(desired_bounds, visible)
    } else {
        // Fallback to centered on primary
        gpui::Bounds::new(
            gpui::Point::new(px(100.0), px(100.0)),
            gpui::Size::new(px(width), px(height)),
        )
    }
}

fn notes_automation_bounds(
    bounds: gpui::Bounds<gpui::Pixels>,
) -> crate::protocol::AutomationWindowBounds {
    crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    }
}

/// Update the Notes window WITHOUT leasing the `Root` entity.
///
/// `WindowHandle<Root>::update` leases the `Root` view for the duration of the
/// closure, so any inner code that touches `Root` again — `window.has_active_dialog`,
/// `window.close_all_dialogs`, the focus-transition log behind
/// `request_focus_surface`, or dialog open/close helpers — panics with
/// "cannot read/update Root while it is already being updated"
/// (gpui entity_map double-lease). Routing through `AnyWindowHandle::update`
/// provides the same `&mut Window` + `&mut App` access with no `Root` lease,
/// which matches the live keyboard/mouse listener environment.
///
/// Every automation/helper entry point that drives `NotesApp` from outside the
/// window MUST use this instead of `handle.update(cx, |_root, ...|)`.
pub(crate) fn update_notes_window_detached<C, R>(
    handle: gpui::WindowHandle<Root>,
    cx: &mut C,
    f: impl FnOnce(&mut Window, &mut App) -> R,
) -> Result<R>
where
    C: gpui::AppContext,
{
    gpui::AnyWindowHandle::from(handle).update(cx, |_root, window, cx| f(window, cx))
}

/// Toggle the notes window (open if closed, close if open)
pub fn open_notes_window(cx: &mut App) -> Result<()> {
    open_notes_window_with_close_behavior(cx, NotesCloseBehavior::RestoreLauncher)
}

pub fn open_notes_window_without_launcher_restore(cx: &mut App) -> Result<()> {
    open_notes_window_with_close_behavior(cx, NotesCloseBehavior::LeaveLauncherHidden)
}

pub fn open_note_in_notes_window(cx: &mut App, note_id: NoteId) -> Result<()> {
    storage::init_notes_db()?;
    let note = storage::get_note(note_id)?.ok_or_else(|| anyhow::anyhow!("Note not found"))?;
    if note.deleted_at.is_some() {
        anyhow::bail!("Note is deleted");
    }

    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    let existing_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (existing_handle, existing_app.clone()) {
        hide_main_window_for_notes(cx);

        let result = update_notes_window_detached(handle, cx, |window, cx| {
            window.activate_window();
            notes_app.update(cx, |app, cx| {
                app.select_note_by_id_from_root(note_id, window, cx)
            })
        });

        match result {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
                if let Ok(mut g) = slot.lock() {
                    *g = None;
                }
                let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
                if let Ok(mut g) = slot.lock() {
                    *g = None;
                }
            }
        }
    }

    open_notes_window_with_close_behavior(cx, NotesCloseBehavior::LeaveLauncherHidden)?;

    let handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let notes_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (handle, notes_app) {
        let result = update_notes_window_detached(handle, cx, |window, cx| {
            window.activate_window();
            notes_app.update(cx, |app, cx| {
                app.select_note_by_id_from_root(note_id, window, cx)
            })
        });

        return match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error),
            Err(error) => Err(anyhow::anyhow!("Failed to update Notes window: {error}")),
        };
    }

    Err(anyhow::anyhow!("Notes window is unavailable"))
}

fn run_existing_day_note_reuse_handoff<C, E>(
    cx: &mut C,
    select_day_note: impl FnOnce(&mut C) -> std::result::Result<(), E>,
    hide_main_then_activate_notes: impl FnOnce(&mut C),
) -> std::result::Result<(), E> {
    select_day_note(cx)?;
    hide_main_then_activate_notes(cx);
    Ok(())
}

pub fn open_day_note_in_notes_window(cx: &mut App, date: chrono::NaiveDate) -> Result<()> {
    storage::init_notes_db()?;
    let path = storage::notes_brain_days_dir().join(format!("{date}.md"));
    if !path.exists() {
        anyhow::bail!("Day note not found");
    }

    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    let existing_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (existing_handle, existing_app.clone()) {
        let main_was_visible = crate::is_main_window_visible();
        let result = run_existing_day_note_reuse_handoff(
            cx,
            |cx| {
                update_notes_window_detached(handle, cx, |window, cx| {
                    notes_app.update(cx, |app, cx| {
                        app.select_day_note(date, window, cx);
                    });
                })
            },
            |cx| {
                if main_was_visible {
                    hide_main_window_then_activate_notes(cx, handle);
                } else if let Err(error) =
                    update_notes_window_detached(handle, cx, |window, _cx| {
                        window.activate_window();
                    })
                {
                    tracing::warn!(
                        target: "script_kit::notes",
                        %error,
                        "notes_reuse_activation_without_main_hide_failed"
                    );
                }
            },
        );

        match result {
            Ok(()) => return Ok(()),
            Err(_) => {
                let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
                if let Ok(mut g) = slot.lock() {
                    *g = None;
                }
                let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
                if let Ok(mut g) = slot.lock() {
                    *g = None;
                }
            }
        }
    }

    open_notes_window_with_close_behavior(cx, NotesCloseBehavior::LeaveLauncherHidden)?;

    let handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let notes_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (handle, notes_app) {
        let result = update_notes_window_detached(handle, cx, |window, cx| {
            window.activate_window();
            notes_app.update(cx, |app, cx| {
                app.select_day_note(date, window, cx);
            });
        });

        return result.map_err(|error| anyhow::anyhow!("Failed to update Notes window: {error}"));
    }

    Err(anyhow::anyhow!("Notes window is unavailable"))
}

pub fn apply_mcp_notes_mutation_on_main_thread(
    request: NotesMutationRequest,
    cx: &mut App,
) -> Result<NotesMutationResult, NotesMutationError> {
    storage::init_notes_db().map_err(internal_notes_error)?;
    save_open_notes_window_if_dirty(cx)?;

    let result = match request {
        NotesMutationRequest::Create(args) => create_note_from_mcp(args)?,
        NotesMutationRequest::Update(args) => update_note_from_mcp(args)?,
        NotesMutationRequest::Delete(args) => delete_note_from_mcp(args)?,
    };

    refresh_or_open_notes_window_after_mcp_mutation(
        result.id,
        result.open_after && !result.deleted,
        cx,
    )?;
    Ok(NotesMutationResult {
        id: result.id.as_str(),
        uri: format!("kit://notes/{}", result.id),
        title: result.title,
        deleted: result.deleted,
        permanent: result.permanent,
    })
}

struct AppliedMcpNoteMutation {
    id: NoteId,
    title: Option<String>,
    deleted: bool,
    permanent: bool,
    open_after: bool,
}

fn create_note_from_mcp(
    args: NotesCreateArgs,
) -> Result<AppliedMcpNoteMutation, NotesMutationError> {
    let id = match args.id {
        Some(id) => NoteId::parse(&id).ok_or_else(|| {
            NotesMutationError::new(
                NotesMutationErrorCode::InvalidParams,
                format!("Invalid note id: {id}"),
            )
        })?,
        None => NoteId::new(),
    };

    let body = crate::notes::metadata::merge_frontmatter(
        &args.body,
        crate::notes::metadata::MetadataFrontmatterPatch {
            tags: args.tags,
            aliases: args.aliases,
            source: args.source,
        },
    );
    validate_mcp_note_content_len(&body)?;
    let mut note = Note::with_content(body);
    note.id = id;
    if storage::get_note(id)
        .map_err(internal_notes_error)?
        .is_some()
    {
        return Err(NotesMutationError::new(
            NotesMutationErrorCode::Conflict,
            format!("Note already exists: {id}"),
        ));
    }
    if let Some(title) = args.title.filter(|title| !title.trim().is_empty()) {
        note.title = title;
    } else if note.title.trim().is_empty() {
        note.title = title_from_body(&note.content);
    }
    note.is_pinned = args.is_pinned;
    if let Some(sort_order) = args.sort_order {
        note.sort_order = sort_order;
    }
    storage::save_note(&note).map_err(internal_notes_error)?;

    Ok(AppliedMcpNoteMutation {
        id,
        title: Some(note.title),
        deleted: false,
        permanent: false,
        open_after: args.open || args.select,
    })
}

fn update_note_from_mcp(
    args: NotesUpdateArgs,
) -> Result<AppliedMcpNoteMutation, NotesMutationError> {
    let id = NoteId::parse(&args.id).ok_or_else(|| {
        NotesMutationError::new(
            NotesMutationErrorCode::InvalidParams,
            format!("Invalid note id: {}", args.id),
        )
    })?;
    let mut note = storage::get_note(id)
        .map_err(internal_notes_error)?
        .ok_or_else(|| {
            NotesMutationError::new(
                NotesMutationErrorCode::NotFound,
                format!("Note not found: {id}"),
            )
        })?;

    if let Some(body) = args.body {
        note.content = crate::notes::metadata::merge_frontmatter(
            &body,
            crate::notes::metadata::MetadataFrontmatterPatch {
                tags: args.tags.clone(),
                aliases: args.aliases.clone(),
                source: None,
            },
        );
        validate_mcp_note_content_len(&note.content)?;
        if args.title.is_none() {
            note.title = title_from_body(&note.content);
        }
    } else if !args.tags.is_empty() || !args.aliases.is_empty() {
        note.content = crate::notes::metadata::merge_frontmatter(
            &note.content,
            crate::notes::metadata::MetadataFrontmatterPatch {
                tags: args.tags.clone(),
                aliases: args.aliases.clone(),
                source: None,
            },
        );
        validate_mcp_note_content_len(&note.content)?;
    }
    if let Some(title) = args.title {
        note.title = if title.trim().is_empty() {
            title_from_body(&note.content)
        } else {
            title
        };
    }
    if let Some(is_pinned) = args.is_pinned {
        note.is_pinned = is_pinned;
    }
    if let Some(sort_order) = args.sort_order {
        note.sort_order = sort_order;
    }
    note.updated_at = chrono::Utc::now();
    note.deleted_at = None;

    storage::save_note(&note).map_err(internal_notes_error)?;

    Ok(AppliedMcpNoteMutation {
        id,
        title: Some(note.title),
        deleted: false,
        permanent: false,
        open_after: args.open || args.select,
    })
}

fn delete_note_from_mcp(
    args: NotesDeleteArgs,
) -> Result<AppliedMcpNoteMutation, NotesMutationError> {
    let id = NoteId::parse(&args.id).ok_or_else(|| {
        NotesMutationError::new(
            NotesMutationErrorCode::InvalidParams,
            format!("Invalid note id: {}", args.id),
        )
    })?;

    if args.permanent {
        if !args.confirm {
            return Err(NotesMutationError::new(
                NotesMutationErrorCode::ConfirmRequired,
                "Permanent note delete requires confirm:true",
            ));
        }
        storage::get_note(id)
            .map_err(internal_notes_error)?
            .ok_or_else(|| {
                NotesMutationError::new(
                    NotesMutationErrorCode::NotFound,
                    format!("Note not found: {id}"),
                )
            })?;
        storage::delete_note_permanently(id).map_err(internal_notes_error)?;
        return Ok(AppliedMcpNoteMutation {
            id,
            title: None,
            deleted: true,
            permanent: true,
            open_after: false,
        });
    }

    let mut note = storage::get_note(id)
        .map_err(internal_notes_error)?
        .ok_or_else(|| {
            NotesMutationError::new(
                NotesMutationErrorCode::NotFound,
                format!("Note not found: {id}"),
            )
        })?;
    let title = note.title.clone();
    note.soft_delete();
    note.updated_at = chrono::Utc::now();
    storage::save_note(&note).map_err(internal_notes_error)?;

    Ok(AppliedMcpNoteMutation {
        id,
        title: Some(title),
        deleted: true,
        permanent: false,
        open_after: false,
    })
}

fn save_open_notes_window_if_dirty(cx: &mut App) -> Result<(), NotesMutationError> {
    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let existing_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (existing_handle, existing_app) {
        update_notes_window_detached(handle, cx, |_window, cx| {
            notes_app.update(cx, |app, _cx| app.save_current_note())
        })
        .map_err(|error| {
            NotesMutationError::new(
                NotesMutationErrorCode::Internal,
                format!("Failed to save open Notes window before MCP mutation: {error}"),
            )
        })?
        .then_some(())
        .ok_or_else(|| {
            NotesMutationError::new(
                NotesMutationErrorCode::Conflict,
                "Failed to save dirty Notes editor before MCP mutation",
            )
        })?;
    }
    Ok(())
}

fn refresh_or_open_notes_window_after_mcp_mutation(
    note_id: NoteId,
    open_or_select: bool,
    cx: &mut App,
) -> Result<(), NotesMutationError> {
    if open_or_select {
        open_note_in_notes_window(cx, note_id).map_err(internal_notes_error)?;
        return Ok(());
    }

    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let existing_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (existing_handle, existing_app) {
        update_notes_window_detached(handle, cx, |window, cx| {
            notes_app.update(cx, |app, cx| {
                app.reload_after_external_note_mutation(note_id, window, cx)
            })
        })
        .map_err(|error| {
            NotesMutationError::new(
                NotesMutationErrorCode::Internal,
                format!("Failed to refresh Notes window after MCP mutation: {error}"),
            )
        })?
        .map_err(internal_notes_error)?;
    }

    Ok(())
}

fn title_from_body(body: &str) -> String {
    crate::notes::metadata::strip_frontmatter(body)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().trim_start_matches('#').trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Untitled Note".to_string())
}

fn validate_mcp_note_content_len(content: &str) -> Result<(), NotesMutationError> {
    if content.len() > NOTE_BODY_MAX_BYTES {
        return Err(NotesMutationError::new(
            NotesMutationErrorCode::InvalidParams,
            format!(
                "notes content exceeds max byte length of {NOTE_BODY_MAX_BYTES} after metadata merge"
            ),
        ));
    }
    Ok(())
}

fn internal_notes_error(error: impl std::fmt::Display) -> NotesMutationError {
    NotesMutationError::new(NotesMutationErrorCode::Internal, error.to_string())
}

fn open_notes_window_with_close_behavior(
    cx: &mut App,
    close_behavior: NotesCloseBehavior,
) -> Result<()> {
    use crate::logging;

    logging::log("PANEL", "open_notes_window called - checking toggle state");

    // Ensure gpui-component theme is initialized before opening window
    ensure_theme_initialized(cx);

    // SAFETY: Release lock BEFORE calling handle.update() to prevent deadlock.
    // We clone the handle (it's just an ID) and release the lock immediately.
    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    // Check if window already exists and is valid
    if let Some(handle) = existing_handle {
        // Read the pending exit ticket WITHOUT taking it: the ticket may only
        // leave the slot after the native exit cancellation actually
        // succeeded, otherwise the delayed removal must keep running.
        let pending_exit = NOTES_EXIT_TICKET
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .as_ref()
            .copied();
        if let Some(ticket) = pending_exit {
            let reopened = update_notes_window_detached(handle, cx, |window, _cx| {
                let native_exit_cancelled =
                    crate::platform::cancel_gpui_window_exit_dematerialize(window);
                if !native_exit_cancelled {
                    return (false, None);
                }
                let native = configure_notes_as_floating_panel(window);
                window.activate_window();
                (true, native)
            });
            match reopened {
                Ok((true, native)) => {
                    let removed_ticket = {
                        let mut slot = NOTES_EXIT_TICKET
                            .lock()
                            .unwrap_or_else(|poison| poison.into_inner());
                        if *slot != Some(ticket) {
                            anyhow::bail!("Notes exit ticket changed while superseding reopen");
                        }
                        slot.take()
                    };
                    debug_assert_eq!(removed_ticket, Some(ticket));
                    crate::windows::upsert_automation_window(
                        crate::protocol::AutomationWindowInfo {
                            id: "notes".to_string(),
                            kind: crate::protocol::AutomationWindowKind::Notes,
                            title: Some("Notes".to_string()),
                            focused: true,
                            visible: true,
                            semantic_surface: Some("notes".to_string()),
                            bounds: None,
                            parent_window_id: None,
                            parent_kind: None,
                            pid: Some(std::process::id()),
                        },
                    );
                    logging::log(
                        "PANEL",
                        "Notes exit superseded - reused the live native window",
                    );
                    if let Some(notes_app) = NOTES_APP_ENTITY
                        .get_or_init(|| std::sync::Mutex::new(None))
                        .lock()
                        .ok()
                        .and_then(|guard| guard.clone())
                    {
                        // User-visible contract (bug report 2026-07-26, "text
                        // fades in twice"): text that was still stably visible
                        // through the superseded exit must NOT vanish and
                        // replay the 90ms body reveal. Restart only when the
                        // reveal never completed.
                        let disposition = notes_app.update(cx, |app, _cx| {
                            app.entry_reveal
                                .exit_supersede_disposition(true)
                                .expect("native exit was cancelled")
                        });
                        match disposition {
                            NotesExitRevealDisposition::PreserveVisible => {
                                // The window stayed visible at its user-chosen
                                // frame: restore the user-resizable policy
                                // immediately (ExitLocked -> Enabled).
                                let _ = update_notes_window_detached(handle, cx, |window, cx| {
                                    notes_app.update(cx, |app, _cx| {
                                        app.restore_native_resize_after_exit_supersede(
                                            true, window,
                                        );
                                    });
                                });
                                tracing::info!(
                                    target: "script_kit::notes",
                                    event = "notes_exit_supersede_preserved_visible_reveal",
                                    "Notes exit supersede kept the already-visible body"
                                );
                            }
                            NotesExitRevealDisposition::RestartHidden => {
                                // Remain locked (ExitLocked -> EntryLocked) and
                                // run the normal entry unlock after settle.
                                let _ = update_notes_window_detached(handle, cx, |window, cx| {
                                    notes_app.update(cx, |app, _cx| {
                                        app.restore_native_resize_after_exit_supersede(
                                            false, window,
                                        );
                                    });
                                });
                                notes_app.update(cx, |app, cx| {
                                    app.entry_reveal.restart();
                                    cx.notify();
                                });
                                schedule_notes_resize_unlock(
                                    handle,
                                    notes_app.clone(),
                                    native.as_ref(),
                                    cx,
                                );
                                schedule_notes_entry_reveal(handle, notes_app, native, cx);
                            }
                        }
                    }
                    return Ok(());
                }
                Ok((false, _)) => {
                    anyhow::bail!("Notes exit supersede could not cancel the native exit");
                }
                Err(error) => {
                    return Err(anyhow::anyhow!(
                        "Notes exit supersede failed to update the live window: {error}"
                    ));
                }
            }
        }
        // Window exists - check if it's valid and close it (toggle OFF)
        // Lock is released, safe to call handle.update()
        if let Some(notes_app) = NOTES_APP_ENTITY
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
        {
            notes_app.update(cx, |app, cx| {
                let _ = app.entry_reveal.prepare_for_window_exit();
                cx.notify();
            });
        }
        let close_result = handle.update(cx, |root, window, cx| {
            // Save bounds before closing (fixes bounds persistence on toggle
            // close) — but only a STABLE frame; a mid-morph close must not
            // persist the transient entry geometry.
            if let Some(notes_app) = NOTES_APP_ENTITY
                .get_or_init(|| std::sync::Mutex::new(None))
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
            {
                notes_app.update(cx, |app, _cx| {
                    app.maybe_save_stable_bounds_for_exit(window);
                });
            }
            // Avoid re-entrant Root lease: `window.close_all_dialogs(cx)` wraps its body
            // in `Root::update(self, cx, ...)`, but we already hold the Root lease via
            // `handle.update`. Calling the inner method on the leased `root` directly
            // bypasses the second lease and prevents the entity_map.rs:142 double-lease
            // panic that fires on rapid `openNotes` -> `hide` -> `openNotes` toggles.
            root.close_all_dialogs(window, cx);
            // Lock native resizing so the user-selected frame becomes the
            // fixed exit frame; an edge drag must not fight the fade.
            if let Some(notes_app) = NOTES_APP_ENTITY
                .get_or_init(|| std::sync::Mutex::new(None))
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
            {
                notes_app.update(cx, |app, _cx| {
                    app.lock_native_resize_for_exit(window);
                });
            }
            // Glass mode: play the Spotlight dematerialize (same as the
            // main window's exit), then remove after the fade.
            if let Some(ticket) =
                crate::platform::begin_gpui_window_exit_with_ticket(window, "PANEL", "Notes")
            {
                Some((ticket, window.window_handle()))
            } else {
                window.remove_window();
                None
            }
        });
        if let Ok(exit) = close_result {
            // Close any open CommandBar windows (command_bar and note_switcher)
            // They use a global singleton, so we close it via the actions module
            crate::actions::close_actions_window(cx);
            logging::log("PANEL", "Notes window was open - closing (toggle OFF)");
            tracing::info!(
                target: "script_kit::keyboard",
                event = "notes_toggle_off_restore_launcher_requested"
            );
            if let Some((ticket, any_handle)) = exit {
                *NOTES_EXIT_TICKET
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner()) = Some(ticket);
                cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                    cx.background_executor()
                        .timer(crate::platform::glass_exit_remove_delay())
                        .await;
                    cx.update(|cx| {
                        let pending_matches = NOTES_EXIT_TICKET
                            .lock()
                            .unwrap_or_else(|poison| poison.into_inner())
                            .as_ref()
                            .copied()
                            == Some(ticket);
                        if !pending_matches
                            || !crate::platform::glass_exit_ticket_is_current(ticket)
                        {
                            return;
                        }
                        NOTES_EXIT_TICKET
                            .lock()
                            .unwrap_or_else(|poison| poison.into_inner())
                            .take();
                        crate::platform::record_glass_exit_commit(ticket);
                        retire_notes_window_registrations(notes_window_close_transition(
                            NotesWindowCloseOrigin::CurrentWindow,
                        ));
                        let _ = any_handle.update(cx, |_root, window, _cx| {
                            window.remove_window();
                        });
                        restore_launcher_after_notes_close_if_needed(cx);
                    });
                })
                .detach();
            } else {
                retire_notes_window_registrations(notes_window_close_transition(
                    NotesWindowCloseOrigin::CurrentWindow,
                ));
                restore_launcher_after_notes_close_if_needed(cx);
            }
            return Ok(());
        }
        // Window handle was invalid, fall through to create new window
        logging::log("PANEL", "Notes window handle was invalid - creating new");
    }

    // If main window is visible, hide it (Notes takes focus)
    // Use defer_hide_main_window to only hide the main window, not the whole app.
    // Must be deferred to avoid RefCell reentrancy from macOS callbacks.
    // IMPORTANT: Set visibility to false so the main hotkey knows to SHOW (not hide) next time
    let main_was_visible_at_open = crate::is_main_window_visible();
    if main_was_visible_at_open {
        logging::log(
            "PANEL",
            "Main window was visible - hiding it since Notes is opening",
        );
        hide_main_window_for_notes(cx);
    }

    // Create new window (toggle ON)
    logging::log("PANEL", "Notes window not open - creating new (toggle ON)");
    info!("Opening new notes window");

    // Calculate position: try saved position first, then top-right default
    let default_bounds = default_notes_window_bounds();
    let displays = crate::platform::get_macos_displays();
    let bounds = if std::env::var_os("SCRIPT_KIT_TEST_NOTES_DB_PATH").is_some() {
        default_bounds
    } else {
        let restored = crate::window_state::get_initial_bounds(
            crate::window_state::WindowRole::Notes,
            default_bounds,
            &displays,
        );
        // Sanitize the restored size against the Notes shell policy: clamp up
        // to the policy minimum, no product maximum (a user size beyond the
        // 600pt auto-size ceiling restores unchanged).
        let policy = crate::window_resize::policy::resize_policy(
            crate::window_resize::policy::WindowShellKind::Notes,
        );
        let (width, height) = crate::window_resize::policy::clamp_restored_content_size(
            f32::from(restored.size.width).into(),
            f32::from(restored.size.height).into(),
            &policy,
        );
        gpui::Bounds {
            origin: restored.origin,
            size: gpui::size(px(width as f32), px(height as f32)),
        }
    };

    // Load theme to determine window background appearance (vibrancy)
    let theme = get_cached_theme();
    let window_background = if theme.is_vibrancy_enabled() {
        if crate::platform::tahoe_liquid_glass_available() {
            // Tahoe: the native glass backdrop supplies the material; Blurred
            // would cover it with the fork's NSVisualEffectView.
            gpui::WindowBackgroundAppearance::Transparent
        } else {
            gpui::WindowBackgroundAppearance::Blurred
        }
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };

    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("Notes".into()),
            appears_transparent: true,
            // App-authored group origin, shared with the design-contract
            // exporter. (NOTE: the y comment used to claim "centered in a
            // 26px header" while the painted titlebar is 36px — the origin
            // value itself is the contract, not that stale rationale.)
            traffic_light_position: Some(gpui::Point {
                x: px(super::contract::NOTES_TRAFFIC_LIGHT_ORIGIN_X),
                y: px(super::contract::NOTES_TRAFFIC_LIGHT_ORIGIN_Y),
            }),
        }),
        window_background,
        focus: true,
        show: true,
        // Use PopUp for floating panel behavior - allows keyboard input without
        // activating the app (Raycast-like). Creates NSPanel with NonactivatingPanel mask.
        kind: gpui::WindowKind::PopUp,
        // Notes owns a precise bottom-edge resize handoff so its floating
        // footer buttons can remain protected. AppKit's ordinary resizable
        // frame intercepts those overlapping button pixels before GPUI can
        // classify them, so native frame resizing must stay disabled here.
        is_resizable: false,
        ..Default::default()
    };

    // Store the NotesApp entity so we can focus it after window creation
    let notes_app_holder: std::sync::Arc<std::sync::Mutex<Option<Entity<NotesApp>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let notes_app_for_closure = notes_app_holder.clone();

    let handle = cx.open_window(window_options, |window, cx| {
        let view = cx.new(|cx| NotesApp::new(window, cx));
        *notes_app_for_closure
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(view.clone());
        cx.new(|cx| Root::new(view, window, cx))
    })?;

    // NOTE: We do NOT call cx.activate(true) here!
    // Notes is a PopUp window (NSPanel with NonactivatingPanel style), which means
    // it can receive keyboard input without activating the application.
    // Calling activate(true) would bring ALL windows forward (including main window),
    // causing a flash before we could hide it.
    //
    // Instead, we just ensure the main window is hidden (in case it was visible)
    // and let the PopUp window handle focus naturally.
    crate::platform::defer_hide_main_window(cx);

    // Store the window handle (release lock immediately)
    {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        if let Ok(mut g) = slot.lock() {
            *g = Some(handle);
        }
    }
    {
        // "Restore launcher" means: bring back what Notes hid. When Notes was
        // opened with the launcher already hidden (direct cmd+ctrl+N), there
        // is nothing to restore — closing Notes must NOT summon the launcher.
        let effective_behavior =
            if close_behavior == NotesCloseBehavior::RestoreLauncher && !main_was_visible_at_open {
                NotesCloseBehavior::LeaveLauncherHidden
            } else {
                close_behavior
            };
        let slot = NOTES_CLOSE_BEHAVIOR
            .get_or_init(|| std::sync::Mutex::new(NotesCloseBehavior::RestoreLauncher));
        if let Ok(mut g) = slot.lock() {
            *g = effective_behavior;
        }
    }

    let notes_any: gpui::AnyWindowHandle = handle.into();
    crate::windows::upsert_runtime_window_handle("notes", notes_any);
    crate::windows::upsert_automation_window(crate::protocol::AutomationWindowInfo {
        id: "notes".to_string(),
        kind: crate::protocol::AutomationWindowKind::Notes,
        title: Some("Notes".to_string()),
        focused: true,
        visible: true,
        semantic_surface: Some("notes".to_string()),
        bounds: Some(notes_automation_bounds(bounds)),
        parent_window_id: None,
        parent_kind: None,
        pid: Some(std::process::id()),
    });

    // Resolve and configure the exact GPUI-owned NSWindow before the body
    // reveal sequence begins. A title scan can select a stale tail window
    // during rapid close/reopen and is therefore not an acceptable owner.
    let native_config = update_notes_window_detached(handle, cx, |window, _cx| {
        let native = configure_notes_as_floating_panel(window);
        // Apply the Notes shell's minimum-size constraints immediately, but
        // keep native user resizing locked until the calibrated entry morph
        // fully settles (`schedule_notes_resize_unlock`).
        crate::platform::apply_window_resize_policy(
            window,
            crate::window_resize::policy::resize_policy(
                crate::window_resize::policy::WindowShellKind::Notes,
            ),
            false,
        );
        native
    })
    .ok()
    .flatten();
    if native_config.is_none() {
        tracing::warn!(
            target: "script_kit::notes",
            event = "notes_native_entry_configuration_failed",
            "Notes exact-window native configuration failed; bounded reveal fallback remains active"
        );
    }

    // Focus the editor input in the Notes window
    // Release lock before calling update
    let notes_app_entity = notes_app_holder.lock().ok().and_then(|mut g| g.take());
    if notes_app_entity.is_none() {
        // Chaos-15 diagnosability: the open_window closure should have filled
        // the holder synchronously; an empty holder here leaves the global
        // entity slot stale/empty and breaks every protocol notes target.
        tracing::warn!(
            target: "script_kit::automation",
            "notes_open_holder_empty_after_open_window"
        );
    }
    if let Some(notes_app) = notes_app_entity {
        // Store the entity globally for quick_capture access
        {
            let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
            if let Ok(mut g) = slot.lock() {
                *g = Some(notes_app.clone());
            }
            tracing::info!(
                target: "script_kit::automation",
                entity_slot_addr = format!("{:p}", &NOTES_APP_ENTITY),
                window_slot_addr = format!("{:p}", &NOTES_WINDOW),
                "notes_entity_stored_after_open"
            );
        }

        let _ = update_notes_window_detached(handle, cx, |window, cx| {
            window.activate_window();

            // Focus the NotesApp's editor input and move cursor to end
            notes_app.update(cx, |app, cx| {
                // Get content length for cursor positioning
                let content_len = app.editor_state.read(cx).value().len();

                // Call the InputState's focus method and move cursor to end
                app.editor_state.update(cx, |state, inner_cx| {
                    state.focus(window, inner_cx);
                    // Move cursor to end of text (same as select_note behavior)
                    state.set_selection(content_len, content_len, window, inner_cx);
                });

                if std::env::var("SCRIPT_KIT_TEST_NOTES_HOVERED")
                    .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
                {
                    app.force_hovered = true;
                    app.window_hovered = true;
                    app.titlebar_hovered = true;
                }

                if std::env::var("SCRIPT_KIT_TEST_NOTES_ACTIONS_PANEL")
                    .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
                {
                    app.open_actions_panel(window, cx);
                }

                cx.notify();
            });
        });
        schedule_notes_resize_unlock(handle, notes_app.clone(), native_config.as_ref(), cx);
        schedule_notes_entry_reveal(handle, notes_app, native_config, cx);
    }

    // NOTE: Theme hot-reload is now handled by the centralized ThemeService
    // (crate::theme::service::ensure_theme_service) which is started once at app init.
    // This eliminates per-window theme watcher tasks and their potential for leaks.

    Ok(())
}

/// Quick capture - open notes with a new note ready for input
///
/// Creates a new empty note and focuses the editor immediately,
/// providing a frictionless capture experience like Apple Quick Note (Fn+Q)
/// or Raycast's Option-click menu bar.
pub fn quick_capture(cx: &mut App) -> Result<()> {
    use crate::logging;

    // Get existing window and app entity
    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    let existing_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    // If window exists with valid app entity, create new note in existing window
    if let (Some(handle), Some(notes_app)) = (existing_handle, existing_app) {
        let result = update_notes_window_detached(handle, cx, |window, cx| {
            notes_app.update(cx, |app, cx| {
                app.create_note(window, cx);
            });
        });

        if result.is_ok() {
            logging::log(
                "PANEL",
                "Quick capture: created new note in existing window",
            );
            return Ok(());
        }
        // Handle was invalid, fall through to create new window
    }

    // Window doesn't exist - create new window with a new note
    open_notes_window(cx)?;

    // After window is created, create a new note using the stored entity
    let handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    let notes_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (handle, notes_app) {
        let _ = update_notes_window_detached(handle, cx, |window, cx| {
            notes_app.update(cx, |app, cx| {
                app.create_note(window, cx);
            });
        });
        logging::log("PANEL", "Quick capture: created new window with new note");
    }

    Ok(())
}

/// Open the Notes window with the note switcher (search) already showing.
///
/// Backs the root "Search Notes" command: lands the user directly in the
/// Cmd+P switcher instead of the last-viewed note.
pub fn open_notes_search(cx: &mut App) -> Result<()> {
    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let existing_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    // Window already open: just raise it and show the switcher.
    if let (Some(handle), Some(notes_app)) = (existing_handle, existing_app) {
        let result = update_notes_window_detached(handle, cx, |window, cx| {
            window.activate_window();
            notes_app.update(cx, |app, cx| {
                app.open_browse_panel(window, cx);
            });
        });
        if result.is_ok() {
            return Ok(());
        }
        // Stale handle: fall through and recreate the window.
    }

    open_notes_window_without_launcher_restore(cx)?;

    let handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let notes_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (handle, notes_app) {
        let _ = update_notes_window_detached(handle, cx, |window, cx| {
            notes_app.update(cx, |app, cx| {
                app.open_browse_panel(window, cx);
            });
        });
    }

    Ok(())
}

/// Save content as a new note, opening the Notes window if needed.
///
/// Creates a note pre-filled with the given content and selects it in the
/// Notes window. If the window is already open, adds the note there;
/// otherwise opens the window first.
///
/// Used by "Save as Note" from the AI chat.
pub fn save_note_with_content(cx: &mut App, content: String) -> Result<()> {
    save_note_with_content_and_source(cx, content, None)
}

/// Like [`save_note_with_content`], but records provenance frontmatter
/// (`source: <link>`) so the note points back at the conversation or surface
/// that produced it.
pub fn save_note_with_content_and_source(
    cx: &mut App,
    content: String,
    source: Option<String>,
) -> Result<()> {
    use crate::logging;

    let content = match source {
        Some(source) => crate::notes::metadata::merge_frontmatter(
            &content,
            crate::notes::metadata::MetadataFrontmatterPatch {
                tags: vec![],
                aliases: vec![],
                source: Some(source),
            },
        ),
        None => content,
    };

    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    let existing_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    // If window exists, create note in the existing window
    if let (Some(handle), Some(notes_app)) = (existing_handle, existing_app.clone()) {
        hide_main_window_for_notes(cx);

        let result = update_notes_window_detached(handle, cx, |window, cx| {
            window.activate_window();
            notes_app.update(cx, |app, cx| {
                app.create_note_with_content(content.clone(), window, cx)
            })
        });

        if let Ok(Ok(())) = result {
            logging::log(
                "PANEL",
                "save_note_with_content: created in existing window",
            );
            return Ok(());
        }

        if let Ok(Err(error)) = result {
            return Err(error);
        }
    }

    // Window doesn't exist — open it, then create the note
    open_notes_window(cx)?;

    let handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    let notes_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (handle, notes_app) {
        let result = update_notes_window_detached(handle, cx, |window, cx| {
            notes_app.update(cx, |app, cx| {
                app.create_note_with_content(content, window, cx)
            })
        });

        if let Ok(Ok(())) = result {
            logging::log("PANEL", "save_note_with_content: created in new window");
            return Ok(());
        }

        if let Ok(Err(error)) = result {
            return Err(error);
        }

        return Err(anyhow::anyhow!(
            "Notes window opened but note creation could not be completed"
        ));
    }

    Err(anyhow::anyhow!(
        "Notes window is unavailable for creating a note"
    ))
}

/// Inject dictated text into the notes editor at the current cursor position.
///
/// If the notes window is open, inserts the text at the cursor. Otherwise
/// returns an error. Used by the dictation delivery pipeline when the user
/// started dictation from the notes surface.
pub fn inject_text_into_notes(cx: &mut App, text: &str) -> Result<serde_json::Value, String> {
    let handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let notes_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    let (Some(handle), Some(notes_app)) = (handle, notes_app) else {
        return Err("Notes window is not open".to_string());
    };

    update_notes_window_detached(handle, cx, |window, cx| {
        notes_app.update(cx, |app, cx| app.inject_dictation_text(text, window, cx))
    })
    .map_err(|e| format!("Failed to update notes window: {e}"))
}

/// Close the notes window
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotesWindowCloseOrigin {
    /// The close began inside the live Notes window, so its global handle is
    /// still registered and must be taken without trying to lease it again.
    CurrentWindow,
    /// The outside close helper already took the handle to avoid a re-entrant
    /// Root lease while it updates the window.
    StoredHandleAlreadyTaken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NotesWindowCloseTransition {
    take_window_handle: bool,
    take_app_entity: bool,
    remove_automation_registration: bool,
    remove_runtime_handle: bool,
    restore_launcher_after_removal: bool,
}

const fn notes_window_close_transition(
    origin: NotesWindowCloseOrigin,
) -> NotesWindowCloseTransition {
    NotesWindowCloseTransition {
        take_window_handle: matches!(origin, NotesWindowCloseOrigin::CurrentWindow),
        take_app_entity: true,
        remove_automation_registration: true,
        remove_runtime_handle: true,
        restore_launcher_after_removal: true,
    }
}

fn retire_notes_window_registrations(transition: NotesWindowCloseTransition) {
    if transition.take_window_handle {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        if let Ok(mut guard) = slot.lock() {
            guard.take();
        }
    }
    if transition.take_app_entity {
        let entity = {
            let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
            slot.lock().ok().and_then(|mut guard| guard.take())
        };
        // Release the slot lock before dropping the entity: NotesApp::drop
        // clears these same lifecycle globals when this is the current instance.
        drop(entity);
    }
    if transition.remove_automation_registration {
        crate::windows::remove_automation_window("notes");
    }
    if transition.remove_runtime_handle {
        crate::windows::remove_runtime_window_handle("notes");
    }
}

fn run_current_notes_window_close_sequence(
    transition: NotesWindowCloseTransition,
    retire: impl FnOnce(NotesWindowCloseTransition),
    restore_launcher: impl FnOnce(),
    schedule_window_release: impl FnOnce(),
) {
    retire(transition);
    if transition.restore_launcher_after_removal {
        restore_launcher();
    }
    schedule_window_release();
}

/// Finish a close initiated from inside the live Notes window.
///
/// The caller already owns the live `NotesApp` lease and must cancel its entry
/// reveal before entering here. Never update `NOTES_APP_ENTITY` from this
/// function: doing so recursively leases the same entity and aborts the app.
/// Retire the same-crate globals and registries directly, hand focus back while
/// GPUI can still service the native active-status callback, then release the
/// window once on the next frame. Removing it before the focus handoff makes
/// GPUI's callback log `window not found`.
pub(crate) fn close_current_notes_window(window: &mut Window, cx: &mut App) {
    let transition = notes_window_close_transition(NotesWindowCloseOrigin::CurrentWindow);
    if let Some(ticket) =
        crate::platform::begin_gpui_window_exit_with_ticket(window, "PANEL", "Notes")
    {
        let any_handle = window.window_handle();
        *NOTES_EXIT_TICKET
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(ticket);
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            cx.background_executor()
                .timer(crate::platform::glass_exit_remove_delay())
                .await;
            cx.update(|cx| {
                let pending_matches = NOTES_EXIT_TICKET
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .as_ref()
                    .copied()
                    == Some(ticket);
                if !pending_matches || !crate::platform::glass_exit_ticket_is_current(ticket) {
                    return;
                }
                NOTES_EXIT_TICKET
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .take();
                crate::platform::record_glass_exit_commit(ticket);
                retire_notes_window_registrations(transition);
                let _ = any_handle.update(cx, |_root, window, _cx| {
                    crate::components::footer_chrome::remove_glass_capsule_window(window);
                    window.remove_window();
                });
                if transition.restore_launcher_after_removal {
                    restore_launcher_after_notes_close_if_needed(cx);
                }
            });
        })
        .detach();
    } else {
        run_current_notes_window_close_sequence(
            transition,
            retire_notes_window_registrations,
            || restore_launcher_after_notes_close_if_needed(cx),
            || {
                window.on_next_frame(|window, _cx| {
                    crate::components::footer_chrome::remove_glass_capsule_window(window);
                    window.remove_window();
                });
                window.request_animation_frame();
            },
        );
    }
}

pub fn close_notes_window(cx: &mut App) {
    if let Some(notes_app) = NOTES_APP_ENTITY
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
    {
        notes_app.update(cx, |app, cx| {
            let _ = app.entry_reveal.prepare_for_window_exit();
            cx.notify();
        });
    }

    // Keep the handle and entity registered through the visual tail so a
    // toggle arriving before removal can supersede this exit and reuse the
    // same physical Notes window.
    let handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let transition = notes_window_close_transition(NotesWindowCloseOrigin::CurrentWindow);

    if let Some(handle) = handle {
        match update_notes_window_detached(handle, cx, |window, cx| {
            // Save window bounds before closing — stable frames only (a
            // mid-morph close must not persist transient entry geometry).
            if let Some(notes_app) = NOTES_APP_ENTITY
                .get_or_init(|| std::sync::Mutex::new(None))
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
            {
                notes_app.update(cx, |app, _cx| {
                    app.maybe_save_stable_bounds_for_exit(window);
                });
            }
            // Safe here: no Root lease is held, so the Root::update inside
            // close_all_dialogs does not double-lease.
            window.close_all_dialogs(cx);
            // Lock native resizing before the fixed-frame exit fade begins.
            if let Some(notes_app) = NOTES_APP_ENTITY
                .get_or_init(|| std::sync::Mutex::new(None))
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
            {
                notes_app.update(cx, |app, _cx| {
                    app.lock_native_resize_for_exit(window);
                });
            }
            if let Some(ticket) =
                crate::platform::begin_gpui_window_exit_with_ticket(window, "PANEL", "Notes")
            {
                Some((ticket, window.window_handle()))
            } else {
                crate::components::footer_chrome::remove_glass_capsule_window(window);
                window.remove_window();
                None
            }
        }) {
            Ok(exit) => {
                tracing::info!(
                    target: "script_kit::keyboard",
                    event = "notes_helper_close_restore_launcher_requested"
                );
                if let Some((ticket, any_handle)) = exit {
                    *NOTES_EXIT_TICKET
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()) = Some(ticket);
                    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                        cx.background_executor()
                            .timer(crate::platform::glass_exit_remove_delay())
                            .await;
                        cx.update(|cx| {
                            let pending_matches = NOTES_EXIT_TICKET
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner())
                                .as_ref()
                                .copied()
                                == Some(ticket);
                            if !pending_matches
                                || !crate::platform::glass_exit_ticket_is_current(ticket)
                            {
                                return;
                            }
                            NOTES_EXIT_TICKET
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner())
                                .take();
                            crate::platform::record_glass_exit_commit(ticket);
                            retire_notes_window_registrations(transition);
                            let _ = any_handle.update(cx, |_root, window, _cx| {
                                crate::components::footer_chrome::remove_glass_capsule_window(
                                    window,
                                );
                                window.remove_window();
                            });
                            if transition.restore_launcher_after_removal {
                                restore_launcher_after_notes_close_if_needed(cx);
                            }
                        });
                    })
                    .detach();
                } else {
                    retire_notes_window_registrations(transition);
                    if transition.restore_launcher_after_removal {
                        restore_launcher_after_notes_close_if_needed(cx);
                    }
                }
            }
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::keyboard",
                    event = "notes_helper_close_failed",
                    error = ?error
                );
            }
        }
    }
}

pub(crate) fn restore_launcher_after_notes_close_if_needed(cx: &mut App) {
    let should_restore = {
        let slot = NOTES_CLOSE_BEHAVIOR
            .get_or_init(|| std::sync::Mutex::new(NotesCloseBehavior::RestoreLauncher));
        slot.lock()
            .map(|g| *g == NotesCloseBehavior::RestoreLauncher)
            .unwrap_or(true)
    };

    {
        let slot = NOTES_CLOSE_BEHAVIOR
            .get_or_init(|| std::sync::Mutex::new(NotesCloseBehavior::RestoreLauncher));
        if let Ok(mut g) = slot.lock() {
            *g = NotesCloseBehavior::RestoreLauncher;
        }
    }

    if should_restore {
        restore_launcher_after_notes_close(cx);
    }
}

/// Restore the main launcher window after Notes closes.
///
/// Notes hides the main window on open (`set_main_window_visible(false)` +
/// `defer_hide_main_window`). This function reverses that: it marks the main
/// window visible, syncs the main-hotkey gesture classifier, brings it to
/// front, and makes it key so the user lands back on whatever launcher surface
/// was active before Notes opened.
///
/// The launcher surface is NOT reset — `current_view` and focus target are
/// preserved across the Notes session, so the user returns to the exact
/// view they left (ScriptList, embedded Agent Chat, FileSearch, etc.).
pub(crate) fn restore_launcher_after_notes_close(_cx: &mut App) {
    // Only restore if the main window is currently hidden.
    // If it's already visible (e.g. Notes was opened without hiding it),
    // there's nothing to restore.
    if crate::is_main_window_visible() {
        tracing::debug!(
            target: "script_kit::keyboard",
            event = "notes_restore_skipped_already_visible"
        );
        return;
    }

    crate::set_main_window_visible(true);
    crate::hotkeys::sync_main_gesture_window_shown();
    crate::platform::show_main_window_without_activation();

    tracing::info!(
        target: "script_kit::keyboard",
        event = "notes_restore_launcher_completed"
    );
}

/// Check if the notes window is currently open
///
/// Returns true if the Notes window exists and is valid.
/// This is used by other parts of the app to check if Notes is open
/// without affecting it.
pub fn is_notes_window_open() -> bool {
    let window_handle = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let guard = window_handle.lock().unwrap_or_else(|e| e.into_inner());
    guard.is_some()
}

/// Check if the given window handle matches the Notes window
///
/// Returns true if the window is the Notes window.
/// Used by keystroke interceptors to avoid handling keys meant for Notes.
pub fn is_notes_window(window: &gpui::Window) -> bool {
    let window_handle = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = window_handle.lock() {
        if let Some(notes_handle) = guard.as_ref() {
            // Convert WindowHandle<Root> to AnyWindowHandle via Into trait
            let notes_any: gpui::AnyWindowHandle = (*notes_handle).into();
            return window.window_handle() == notes_any;
        }
    }
    false
}

/// Configure the Notes window as a floating panel (always on top).
///
/// This sets:
/// - Preserve the GPUI-assigned PopUp window level (101)
/// - NSWindowCollectionBehaviorMoveToActiveSpace - moves to current space when shown
/// - Disabled window restoration - prevents macOS position caching
#[cfg(target_os = "macos")]
fn configure_notes_as_floating_panel(gpui_window: &gpui::Window) -> Option<NotesNativeEntryConfig> {
    use crate::logging;

    unsafe {
        let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(gpui_window) else {
            logging::log("PANEL", "Warning: Notes raw window handle unavailable");
            return None;
        };
        let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            return None;
        };
        let ns_view = appkit.ns_view.as_ptr() as id;
        let window: id = msg_send![ns_view, window];
        if window == nil {
            return None;
        }

        let _: () = msg_send![window, setCanHide: false];
        let current: u64 = msg_send![window, collectionBehavior];
        let desired: u64 = notes_window_collection_behavior(current);
        let _: () = msg_send![window, setCollectionBehavior:desired];
        let sharing_type: i64 = 1;
        let _: () = msg_send![window, setSharingType:sharing_type];
        let _: () = msg_send![window, setRestorable:false];
        let _: () = msg_send![window, setAnimationBehavior: 2i64];

        let theme = get_cached_theme();
        let is_dark = theme.should_use_dark_vibrancy();
        let receipt = crate::platform::configure_secondary_window_vibrancy_with_receipt(
            window, "Notes", is_dark,
        )?;

        let window_number: i64 = msg_send![window, windowNumber];
        let has_can_join = (desired & NS_WINDOW_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES) != 0;
        let has_ignores = (desired & NS_WINDOW_COLLECTION_BEHAVIOR_IGNORES_CYCLE) != 0;
        let has_move_to_active =
            (desired & NS_WINDOW_COLLECTION_BEHAVIOR_MOVE_TO_ACTIVE_SPACE) != 0;
        logging::log(
            "PANEL",
            &format!(
                "event=notes_native_entry_configured window_number={} behavior={}->{} CanJoinAllSpaces={} IgnoresCycle={} MoveToActiveSpace={}",
                window_number,
                current,
                desired,
                has_can_join,
                has_ignores,
                has_move_to_active
            ),
        );
        Some(NotesNativeEntryConfig {
            window_number: receipt.window_number,
            configured: receipt.configured,
            backdrop_found_or_created: receipt.backdrop_found_or_created,
            native_selectors_supported: receipt.native_selectors_supported,
            style_applied: receipt.style_applied,
            style_signature: format!("{:?}", receipt.style_signature),
            configured_at_monotonic_ns: receipt.configured_at_monotonic_ns,
            configured_at_unix_ms: receipt.configured_at_unix_ms,
            settle_duration_ms: receipt.settle_duration_ms,
            settled_crossing_delay_ms: receipt.settled_crossing_delay_ms,
            content_reveal_delay_ms: receipt.content_reveal_delay_ms,
            morph_started: receipt.morph_started,
            morph_start_alpha_bits: receipt.morph_start_alpha_bits,
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_notes_as_floating_panel(_window: &gpui::Window) -> Option<NotesNativeEntryConfig> {
    None
}

/// Return the current editor text from the Notes window, if open.
///
/// Used by the automation surface collector to expose Notes state to
/// `getElements` and `inspectAutomationWindow` without routing through
/// the main window.
/// Current Notes surface mode (`Notes` editor vs embedded `AgentChat`) for
/// the live Notes window. Read from the SAME entity state the renderer and
/// titlebar switcher consume, so automation projections cannot drift from
/// the visible selection.
pub fn get_notes_surface_mode(cx: &gpui::App) -> Option<super::NotesSurfaceMode> {
    let entity = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok()?.clone()?
    };
    Some(entity.read(cx).surface_mode)
}

pub fn get_notes_editor_text(cx: &gpui::App) -> Option<String> {
    let entity = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok()?.clone()?
    };
    Some(entity.read(cx).editor_state.read(cx).value().to_string())
}

/// Return shared Markdown editor runtime metadata for the live Notes window.
pub fn get_notes_editor_runtime_info(
    cx: &gpui::App,
) -> Option<crate::protocol::ElementEditorRuntimeInfo> {
    let entity = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok()?.clone()?
    };
    Some(
        entity
            .read(cx)
            .notes_editor
            .read(cx)
            .markdown_runtime_info_with_scroll(cx),
    )
}

/// Return the live `NotesApp` entity and its window handle, if the Notes
/// window is open.
///
/// Used by the automation transaction provider to read and mutate Notes
/// editor state without routing through the main window.
pub fn get_notes_app_entity_and_handle() -> Option<(Entity<NotesApp>, gpui::WindowHandle<Root>)> {
    let entity = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };
    let handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    match (entity, handle) {
        (Some(entity), Some(handle)) => Some((entity, handle)),
        (entity, handle) => {
            // Diagnosability for the chaos-15 reopen bug: report WHICH slot is
            // empty, plus the statics' addresses — if a store site logs a
            // different address, the lib/bin dual-crate static trap is in play.
            tracing::warn!(
                target: "script_kit::automation",
                entity_present = entity.is_some(),
                handle_present = handle.is_some(),
                entity_slot_addr = format!("{:p}", &NOTES_APP_ENTITY),
                window_slot_addr = format!("{:p}", &NOTES_WINDOW),
                "notes_entity_handle_lookup_failed"
            );
            None
        }
    }
}

/// Handle the current Notes ghost autocomplete prediction through the live
/// Notes window, for target-scoped DevTools `simulateKey` proof.
pub fn handle_notes_ghost_key_for_automation(
    cx: &mut App,
    key: &str,
) -> Result<serde_json::Value, String> {
    let (entity, handle) =
        get_notes_app_entity_and_handle().ok_or_else(|| "Notes window is not open".to_string())?;
    // Must not lease Root: the escape ladder reaches `close_actions_panel` →
    // `request_focus_surface`, which reads Root via `window.has_active_dialog`.
    update_notes_window_detached(handle, cx, |window, cx| {
        entity.update(cx, |app, cx| {
            let key = key.to_ascii_lowercase();
            let (action, handled) = match key.as_str() {
                "escape" | "esc" => app.escape_dismiss_ladder(window, cx),
                "tab" => (
                    "acceptNotesGhostWord",
                    app.try_accept_notes_ghost(
                        super::keyboard::NotesGhostAcceptMode::Word,
                        window,
                        cx,
                    ),
                ),
                "`" | "backtick" => (
                    "acceptNotesGhostFull",
                    app.try_accept_notes_ghost(
                        super::keyboard::NotesGhostAcceptMode::Full,
                        window,
                        cx,
                    ),
                ),
                _ => ("unsupportedNotesGhostKey", false),
            };
            serde_json::json!({
                "handled": handled,
                "target": "notes",
                "action": action,
            })
        })
    })
    .map_err(|error| format!("Failed to handle Notes ghost autocomplete key: {error}"))
}

/// Handle Notes editor keys that automation cannot route through the root
/// `capture_key_down` path, while still calling the same surface methods as the
/// live keyboard handler.
pub fn handle_notes_editor_key_for_automation(
    cx: &mut App,
    key: &str,
    platform: bool,
    shift: bool,
    control: bool,
    alt: bool,
) -> Result<serde_json::Value, String> {
    let (entity, handle) =
        get_notes_app_entity_and_handle().ok_or_else(|| "Notes window is not open".to_string())?;
    update_notes_window_detached(handle, cx, |window, cx| {
        entity.update(cx, |app, cx| {
            let key = key.to_ascii_lowercase();
            let mut action = "unsupportedNotesEditorKey";
            let mut handled = false;

            if !platform
                && !shift
                && !control
                && !alt
                && crate::ui_foundation::is_key_escape(&key)
                && crate::confirm::is_confirm_window_open()
            {
                action = "cancelNotesParentConfirm";
                handled = crate::confirm::route_key_to_confirm_popup("escape", cx);
            } else if !platform && !control && !alt {
                if crate::ui_foundation::is_key_up(&key) {
                    action = "moveNotesSpineSelectionUp";
                    handled = app.move_notes_spine_selection(-1, cx);
                } else if crate::ui_foundation::is_key_down(&key) {
                    action = "moveNotesSpineSelectionDown";
                    handled = app.move_notes_spine_selection(1, cx);
                } else if crate::ui_foundation::is_key_enter(&key) {
                    action = "acceptNotesSpineSelection";
                    handled = app.accept_notes_spine_selection(window, cx);
                } else if crate::ui_foundation::is_key_tab(&key) && !shift {
                    action = "acceptNotesSpineSelectionFromTab";
                    handled = app.accept_notes_spine_selection(window, cx);
                }
            } else if platform
                && !shift
                && !control
                && !alt
                && crate::ui_foundation::is_key_enter(&key)
            {
                action = "openMainAgentChatFromNotesCmdEnter";
                handled = app.handoff_selected_note_to_main_agent_chat("NotesWindowCmdEnter", cx);
            } else if platform && !shift && !control && !alt && key == "." {
                action = "activateNotesDeeplinkOrFocusMode";
                if !app.activate_deeplink_under_cursor(window, cx) {
                    app.toggle_focus_mode(cx);
                }
                handled = true;
            }

            serde_json::json!({
                "handled": handled,
                "target": "notes",
                "action": action,
            })
        })
    })
    .map_err(|error| format!("Failed to handle Notes editor key: {error}"))
}

/// Toggle a Notes popup from the automation/simulateKey path, mirroring the
/// live Cmd+K ("actions" command bar) and Cmd+P ("noteSwitcher") keyboard arms
/// so target-scoped `simulateKey` can drive the same popups the user sees.
pub fn toggle_notes_popup_for_automation(
    cx: &mut App,
    popup: &str,
) -> Result<serde_json::Value, String> {
    let (entity, handle) =
        get_notes_app_entity_and_handle().ok_or_else(|| "Notes window is not open".to_string())?;
    // Must not lease Root: popup open/close reaches `request_focus_surface`,
    // which reads Root via `window.has_active_dialog`.
    update_notes_window_detached(handle, cx, |window, cx| {
        entity.update(cx, |app, cx| {
            let action = match popup {
                "actions" => {
                    if app.command_bar.is_open() {
                        app.close_actions_panel(window, cx);
                        "closeActionsPanel"
                    } else {
                        app.open_actions_panel(window, cx);
                        "openActionsPanel"
                    }
                }
                "noteSwitcher" => {
                    app.close_actions_panel(window, cx);
                    if app.note_switcher.is_open() {
                        app.close_browse_panel(window, cx);
                        "closeNoteSwitcher"
                    } else {
                        app.open_browse_panel(window, cx);
                        "openNoteSwitcher"
                    }
                }
                other => {
                    return serde_json::json!({
                        "handled": false,
                        "target": "notes",
                        "popup": other,
                    });
                }
            };
            serde_json::json!({
                "handled": true,
                "target": "notes",
                "popup": popup,
                "action": action,
            })
        })
    })
    .map_err(|error| format!("Failed to toggle Notes popup: {error}"))
}

/// Backward-compatible helper for older target-scoped DevTools `simulateKey Tab`
/// proof. New callers should use `handle_notes_ghost_key_for_automation`.
pub fn accept_notes_ghost_for_automation(cx: &mut App) -> Result<serde_json::Value, String> {
    handle_notes_ghost_key_for_automation(cx, "tab")
}

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
