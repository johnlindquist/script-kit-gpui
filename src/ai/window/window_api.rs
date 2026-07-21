use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

fn ensure_theme_initialized(cx: &mut App) {
    // Use the shared theme sync function from src/theme/gpui_integration.rs
    crate::theme::sync_gpui_component_theme(cx);
    info!("AI window theme synchronized with Script Kit");
}

static OPENING: AtomicBool = AtomicBool::new(false);

pub(super) fn window_role_for_mode(mode: AiWindowMode) -> crate::window_state::WindowRole {
    match mode {
        AiWindowMode::Full => crate::window_state::WindowRole::Ai,
        AiWindowMode::Mini => crate::window_state::WindowRole::AiMini,
    }
}

fn centered_ai_window_bounds_on_cursor_display(mode: AiWindowMode) -> gpui::Bounds<gpui::Pixels> {
    crate::platform::calculate_centered_bounds_on_mouse_display(size(
        px(mode.default_width()),
        px(mode.default_height()),
    ))
}

fn resolve_new_ai_window_bounds(
    saved_bounds: Option<crate::window_state::PersistedWindowBounds>,
    displays: &[crate::windows::DisplayBounds],
    fallback_bounds: gpui::Bounds<gpui::Pixels>,
) -> gpui::Bounds<gpui::Pixels> {
    if let Some(saved) = saved_bounds {
        if crate::window_state::is_bounds_visible(&saved, displays) {
            return saved.to_gpui().get_bounds();
        }
    }

    fallback_bounds
}

/// Toggle the AI window (open if closed, bring to front if open)
///
/// The AI window behaves as a NORMAL window (not a floating panel):
/// - Can go behind other windows when it loses focus
/// - Hotkey brings it to front and focuses it
/// - Does NOT affect other windows (main window, notes window)
/// - Does NOT hide the app when closed
pub fn open_ai_window(cx: &mut App) -> Result<()> {
    open_ai_window_with_mode(AiWindowMode::Full, cx)
}

pub fn open_mini_ai_window(cx: &mut App) -> Result<()> {
    open_ai_window_with_mode(AiWindowMode::Mini, cx)
}

/// Open the mini AI window with a caller-source tag for tracing the handoff.
///
/// Use this from builtin execution and other programmatic entry points so the
/// open→mode→focus arc is machine-verifiable end-to-end.
pub fn open_mini_ai_window_from(source: &'static str, cx: &mut App) -> Result<()> {
    open_ai_window_with_mode_from(AiWindowMode::Mini, source, cx)
}

fn open_ai_window_with_mode(mode: AiWindowMode, cx: &mut App) -> Result<()> {
    open_ai_window_with_mode_from(mode, "direct", cx)
}

fn open_ai_window_with_mode_from(
    mode: AiWindowMode,
    source: &'static str,
    cx: &mut App,
) -> Result<()> {
    if OPENING.swap(true, Ordering::SeqCst) {
        super::telemetry::log_ai_lifecycle(
            "ai_window_open_requested",
            mode,
            source,
            "already_opening",
        );
        return Ok(());
    }

    super::telemetry::log_ai_lifecycle("ai_window_open_requested", mode, source, "start");

    // Ensure gpui-component theme is initialized before opening window
    ensure_theme_initialized(cx);

    // SAFETY: Release lock BEFORE calling handle.update() to prevent deadlock.
    // WindowHandle is Copy, so we just dereference to get it out.
    let existing_handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    // Check if window already exists and is valid
    if let Some(handle) = existing_handle {
        // Window exists - check if it's valid (lock is released)
        let window_valid = handle
            .update(cx, |_root, window, cx| {
                push_ai_command(AiCommand::SetWindowMode(mode));
                window.set_window_title(mode.title());
                // Window is valid - bring it to front and focus it
                window.activate_window();
                cx.notify();
            })
            .is_ok();

        if window_valid {
            super::telemetry::log_ai_lifecycle(
                "ai_window_open_existing",
                mode,
                source,
                "activated",
            );

            // Ensure regular app mode (in case it was switched back to accessory)
            crate::platform::set_regular_app_mode();

            // Activate the app to ensure the window can receive focus
            cx.activate(true);

            // Request focus on the input field via the global flag.
            // AiApp checks this flag in render() and focuses if set.
            // This avoids the need for a global Entity<AiApp> reference which caused memory leaks.
            AI_FOCUS_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);

            // Notify to trigger re-render which will process the focus request
            let _ = handle.update(cx, |_root, _window, cx| {
                cx.notify();
            });

            OPENING.store(false, Ordering::SeqCst);
            return Ok(());
        }

        // Window handle was invalid, clear it
        super::telemetry::log_ai_lifecycle("ai_window_handle_invalid", mode, source, "clearing");
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        if let Ok(mut g) = slot.lock() {
            *g = None;
        }
    }

    // Create new window
    let create_start = std::time::Instant::now();
    super::telemetry::log_ai_lifecycle("ai_window_create", mode, source, "begin");

    // Load theme to determine window background appearance (vibrancy)
    let theme = crate::theme::get_cached_theme();
    let window_background = if theme.is_vibrancy_enabled() {
        crate::platform::vibrancy_window_background()
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };

    // Restore one global AI window position. If that position is now off-screen
    // (for example, a display was disconnected), center on the cursor display.
    let displays = crate::platform::get_macos_displays();
    let saved_bounds = crate::window_state::load_window_bounds(window_role_for_mode(mode));
    {
        let bounds_status = match saved_bounds {
            Some(ref saved) if crate::window_state::is_bounds_visible(saved, &displays) => {
                "restored"
            }
            Some(_) => "offscreen_fallback",
            None => "no_saved_bounds",
        };
        tracing::debug!(
            target: "ai",
            bounds_status,
            window_mode = ?mode,
            "ai_window_bounds_resolve"
        );
    }
    let bounds = resolve_new_ai_window_bounds(
        saved_bounds,
        &displays,
        centered_ai_window_bounds_on_cursor_display(mode),
    );

    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(mode.title().into()),
            appears_transparent: true,
            ..Default::default()
        }),
        window_background,
        focus: true,
        show: true,
        // IMPORTANT: Use Normal window kind (not PopUp) so it behaves like a regular window
        // This allows it to go behind other windows and participate in normal window ordering
        kind: gpui::WindowKind::Normal,
        ..Default::default()
    };

    // Switch to regular app mode BEFORE creating the window.
    // The AI window uses WindowKind::Normal, which requires the app to be in
    // regular activation policy. Creating a Normal window while in accessory
    // mode causes AppKit to block (beachball) because the window server cannot
    // properly register the window until the activation policy allows it.
    crate::platform::set_regular_app_mode();
    cx.activate(true);

    // Create a holder for the AiApp entity so we can focus it after window creation.
    // NOTE: This is a LOCAL holder, not stored globally, to avoid memory leaks.
    let ai_app_holder: std::sync::Arc<std::sync::Mutex<Option<Entity<AiApp>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let ai_app_holder_clone = ai_app_holder.clone();

    tracing::info!(
        elapsed_ms = create_start.elapsed().as_millis(),
        "BEACHBALL TRACE: window options prepared, calling cx.open_window (app mode=regular)"
    );

    let handle = match cx.open_window(window_options, move |window, cx| {
        window.on_window_should_close(cx, move |window, cx| {
            let wb = window.window_bounds();
            crate::window_state::save_window_from_gpui(window_role_for_mode(mode), wb);
            cleanup_ai_window_globals();
            if crate::platform::begin_gpui_window_exit_dematerialize(window, "AI", "AI") {
                crate::platform::remove_gpui_window_after_glass_exit_from_app(window, cx);
                false
            } else {
                true
            }
        });
        let view = cx.new(|cx| AiApp::new_with_mode(mode, window, cx));
        // Store the AiApp entity temporarily for immediate focus after window creation
        *ai_app_holder_clone
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(view.clone());
        cx.new(|cx| Root::new(view, window, cx))
    }) {
        Ok(handle) => handle,
        Err(err) => {
            OPENING.store(false, Ordering::SeqCst);
            return Err(err);
        }
    };

    tracing::info!(
        elapsed_ms = create_start.elapsed().as_millis(),
        "BEACHBALL TRACE: cx.open_window returned, window created"
    );

    // Activate the window (app already activated before open_window)
    let _ = handle.update(cx, |_root, window, _cx| {
        window.activate_window();
        crate::platform::configure_overlay_window_glass(window, "AI");
    });

    // Focus the input field immediately after window creation
    // Use the local entity reference (not stored globally to avoid leaks)
    if let Some(ai_app) = ai_app_holder.lock().ok().and_then(|mut h| h.take()) {
        // Store a weak reference for re-render notification from enqueue_ai_window_command.
        // WeakEntity does NOT prevent the entity from being dropped (no memory leak).
        {
            let slot = AI_APP_WEAK.get_or_init(|| std::sync::Mutex::new(None));
            if let Ok(mut g) = slot.lock() {
                *g = Some(ai_app.downgrade());
            }
        }
        let _ = handle.update(cx, |_root, window, cx| {
            ai_app.update(cx, |app, cx| {
                app.focus_input(window, cx);
            });
        });
    }

    // Store the window handle (release lock immediately after)
    {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        if let Ok(mut g) = slot.lock() {
            *g = Some(handle);
        }
    }
    OPENING.store(false, Ordering::SeqCst);

    // NOTE: set_regular_app_mode() was already called BEFORE cx.open_window()
    // to prevent AppKit from blocking when creating a Normal window in accessory mode.
    // The mode is restored to accessory when the AI window closes (see AiApp::drop).

    // NOTE: We do NOT configure as floating panel - this is a normal window
    // that can go behind other windows
    // However, we DO want vibrancy configuration for proper blur effect
    configure_ai_window_vibrancy();

    // NOTE: Theme hot-reload is now handled by the centralized ThemeService
    // (crate::theme::service::ensure_theme_service) which is started once at app init.
    // This eliminates per-window theme watcher tasks and their potential for leaks.

    tracing::info!(
        elapsed_ms = create_start.elapsed().as_millis(),
        "BEACHBALL TRACE: AI window fully configured and ready"
    );
    super::telemetry::log_ai_lifecycle("ai_window_create", mode, source, "success");

    Ok(())
}

/// A single message in a pending chat transfer, including optional image data.
pub struct PendingChatMessage {
    pub role: MessageRole,
    pub content: String,
    /// Optional base64-encoded PNG image data attached to this message.
    pub image_base64: Option<String>,
}

/// Pending chat to initialize after window opens.
/// This is used by open_ai_window_with_chat to pass messages to the newly created window.
static AI_PENDING_CHAT: std::sync::OnceLock<std::sync::Mutex<Option<Vec<PendingChatMessage>>>> =
    std::sync::OnceLock::new();

pub(super) fn get_pending_chat() -> &'static std::sync::Mutex<Option<Vec<PendingChatMessage>>> {
    AI_PENDING_CHAT.get_or_init(|| std::sync::Mutex::new(None))
}

/// Stash pending chat messages and enqueue `InitializeWithPendingChat`.
///
/// Call this *after* the AI window is already open. The caller is responsible
/// for opening the window first (e.g. via `open_ai_window`).
///
/// Returns `Err` if the pending chat lock cannot be acquired or the command
/// cannot be enqueued (window not open).
pub fn set_ai_pending_chat(cx: &mut App, messages: Vec<PendingChatMessage>) -> Result<(), String> {
    let message_count = messages.len();
    let image_count = messages.iter().filter(|m| m.image_base64.is_some()).count();

    tracing::info!(
        category = "AI",
        event = "set_ai_pending_chat",
        message_count,
        image_count,
        "Stashing pending chat messages for AI window"
    );

    if let Ok(mut pending) = get_pending_chat().lock() {
        *pending = Some(messages);
    } else {
        tracing::error!(
            category = "AI",
            event = "ai_pending_chat_failed",
            reason = "lock_poisoned",
            "Pending chat lock poisoned — cannot stash messages"
        );
        return Err("pending chat lock poisoned".to_string());
    }

    enqueue_ai_window_command(
        cx,
        "initialize_with_pending_chat",
        AiCommand::InitializeWithPendingChat,
    )
}

/// Open the AI window with an existing conversation.
///
/// This function:
/// 1. Opens the AI window (or brings it to front if already open)
/// 2. Creates a new chat with the provided messages
/// 3. Displays the chat immediately
///
/// Use this for "Continue in Chat" functionality to transfer a conversation
/// from the chat prompt to the AI window.
pub fn open_ai_window_with_chat(cx: &mut App, messages: Vec<PendingChatMessage>) -> Result<()> {
    tracing::info!(
        target: "ai",
        category = "AI",
        event = "ai_window_open_with_chat",
        message_count = messages.len(),
        "Opening AI window with pending chat"
    );

    // Open or bring the window to front
    open_ai_window(cx)?;

    // Stash messages and enqueue the initialize command
    set_ai_pending_chat(cx, messages)
        .map_err(|error| anyhow::anyhow!("failed to enqueue pending chat after open: {error}"))?;

    Ok(())
}

/// Clear all AI window global state (handle, focus flag, mode, SDK state).
///
/// Call this from any close path — both `close_ai_window()` (external) and
/// the Cmd+W / Esc handlers inside `handle_root_key_down()` (internal).
/// Without this cleanup, `is_ai_window_open()` returns true for a dead handle
/// and subsequent `open_mini_ai_window()` calls try to bring a removed window
/// to front instead of creating a new one.
pub(super) fn cleanup_ai_window_globals() {
    // Snapshot mode before clearing so the log reflects the actual mode at close time.
    let closing_mode = AiWindowMode::from_u8(
        super::types::AI_CURRENT_WINDOW_MODE.load(std::sync::atomic::Ordering::SeqCst),
    );
    // Take the handle out of the global slot (clears it for future open calls)
    let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = None;
    }
    // Clear the weak AiApp reference
    {
        let slot = AI_APP_WEAK.get_or_init(|| std::sync::Mutex::new(None));
        if let Ok(mut guard) = slot.lock() {
            *guard = None;
        }
    }
    // Clear the focus request flag (no longer needed after window closes)
    AI_FOCUS_REQUESTED.store(false, Ordering::SeqCst);
    // Reset window mode to Full (default for next open)
    super::types::AI_CURRENT_WINDOW_MODE.store(0, std::sync::atomic::Ordering::SeqCst);
    // Clear SDK-visible state so handlers report correct state
    clear_sdk_state();
    super::telemetry::log_ai_lifecycle(
        "ai_window_globals_cleaned",
        closing_mode,
        "cleanup",
        "done",
    );
}

/// Close the AI window
pub fn close_ai_window(cx: &mut App) {
    // SAFETY: Release lock BEFORE calling handle.update() to prevent deadlock
    // If handle.update() causes Drop to fire synchronously and tries to acquire
    // the same lock, we would deadlock. Taking the handle out first avoids this.
    let handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|mut g| g.take())
    };

    if let Some(handle) = handle {
        let _ = handle.update(cx, |_, window, cx| {
            let wb = window.window_bounds();
            // Derive role from the global atomic mirror, not from window title string.
            let mode = AiWindowMode::from_u8(
                super::types::AI_CURRENT_WINDOW_MODE.load(std::sync::atomic::Ordering::SeqCst),
            );
            let role = window_role_for_mode(mode);
            crate::window_state::save_window_from_gpui(role, wb);
            crate::platform::dematerialize_then_remove_gpui_window(window, cx, "AI", "AI");
        });
    }

    // Cleanup globals (handle already taken above, but this clears the rest)
    cleanup_ai_window_globals();
}

/// Check if the AI window is currently open
///
/// Returns true if the AI window exists and is valid.
/// This is used by other parts of the app to check if AI is open
/// without affecting it.
pub fn is_ai_window_open() -> bool {
    let window_handle = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let guard = window_handle.lock().unwrap_or_else(|e| e.into_inner());
    guard.is_some()
}

/// Check if the AI window handle is present **and** still valid.
///
/// Unlike `is_ai_window_open()` which only checks handle presence, this
/// validates the stored handle via `handle.update(...)`. Use this before
/// queueing commands to avoid false-success when the window has been
/// closed but the handle has not yet been cleared.
pub fn is_ai_window_ready(cx: &mut App) -> bool {
    let handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    let Some(handle) = handle else {
        return false;
    };

    handle.update(cx, |_root, _window, _cx| {}).is_ok()
}

/// Centralized AI command enqueue helper.
///
/// Queues an `AiCommand` and notifies the AI window. Returns `Ok(())` on
/// success or `Err(reason)` with an actionable message on failure.
/// Emits structured `ai_command_enqueue` logs for every outcome.
fn enqueue_ai_window_command(
    cx: &mut App,
    command: &'static str,
    ai_command: AiCommand,
) -> Result<(), String> {
    let handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let window_is_open = handle.is_some();

    let queued_index = get_pending_commands().lock().ok().and_then(|mut commands| {
        let queued_index = commands.len();
        ai_window_queue_command_if_open(&mut commands, window_is_open, ai_command)
            .then_some(queued_index)
    });

    if queued_index.is_none() {
        tracing::warn!(
            category = "AI",
            event = "ai_command_enqueue",
            command,
            status = "rejected",
            "AI window not open"
        );
        return Err("AI window not open".to_string());
    }

    if let Some(handle) = handle {
        // Notify the AiApp entity directly so its render() runs and processes
        // the queued command. The weak entity avoids the memory leak that the
        // old global `AI_APP_ENTITY` caused.
        let weak = AI_APP_WEAK
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .ok()
            .and_then(|g| g.clone());
        let update_result = handle.update(cx, |_root, window, cx| {
            if let Some(weak) = weak {
                if let Some(entity) = weak.upgrade() {
                    entity.update(cx, |_app, cx| cx.notify());
                }
            }
            // Also refresh the window to ensure a paint cycle runs
            window.refresh();
        });
        if update_result.is_err() {
            if let Some(queued_index) = queued_index {
                if let Ok(mut commands) = get_pending_commands().lock() {
                    if queued_index < commands.len() {
                        commands.remove(queued_index);
                    }
                }
            }
            tracing::warn!(
                category = "AI",
                event = "ai_command_enqueue",
                command,
                status = "notify_failed",
                "AI window closed before command notify"
            );
            return Err("notify-failed".to_string());
        }
    }

    tracing::info!(
        category = "AI",
        event = "ai_command_enqueue",
        command,
        status = "queued",
        "AI command queued"
    );

    Ok(())
}

/// Check if the given window handle matches the AI window
///
/// Returns true if the window is the AI window.
/// Used by keystroke interceptors to avoid handling keys meant for AI.
pub fn is_ai_window(window: &gpui::Window) -> bool {
    let window_handle = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = window_handle.lock() {
        if let Some(ai_handle) = guard.as_ref() {
            // Convert WindowHandle<Root> to AnyWindowHandle via Into trait
            let ai_any: gpui::AnyWindowHandle = (*ai_handle).into();
            return window.window_handle() == ai_any;
        }
    }
    false
}

/// Read the current AI window state as a machine-readable snapshot.
///
/// Returns `None` if the AI window is not open or the entity has been dropped.
/// The snapshot contains structural metadata (mode, overlay visibility, counts)
/// but never conversation content or PII.
pub fn get_ai_window_state(cx: &mut App) -> Option<super::state::AiMiniDebugSnapshot> {
    let handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let weak = AI_APP_WEAK
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .ok()
        .and_then(|g| g.clone());

    let handle = handle?;
    let weak = weak?;

    handle
        .update(cx, |_root, _window, cx| {
            weak.upgrade()
                .map(|entity| entity.read(cx).debug_snapshot().redact_for_external_use())
        })
        .ok()
        .flatten()
}

/// Set the search filter text in the AI window.
/// Returns an actionable error if the AI window is not ready.
pub fn set_ai_search(cx: &mut App, query: &str) -> Result<(), String> {
    enqueue_ai_window_command(cx, "set_search", AiCommand::SetSearch(query.to_string()))
}

/// Set the main input text in the AI window and optionally submit.
/// Used for testing the streaming functionality via stdin commands.
pub fn set_ai_input(cx: &mut App, text: &str, submit: bool) -> Result<(), String> {
    enqueue_ai_window_command(
        cx,
        "set_input",
        AiCommand::SetInput {
            text: text.to_string(),
            submit,
        },
    )
}

/// Set the main input text with an attached image in the AI window and optionally submit.
/// The image should be base64 encoded PNG data.
/// Used by AI commands like "Send Screen to Agent Chat".
///
/// Guards against window-close races: if the window handle becomes invalid between
/// the open check and the notify, a warning is logged and the command is not silently lost.
pub fn set_ai_input_with_image(
    cx: &mut App,
    text: &str,
    image_base64: &str,
    submit: bool,
) -> Result<(), String> {
    enqueue_ai_window_command(
        cx,
        "set_input_with_image",
        AiCommand::SetInputWithImage {
            text: text.to_string(),
            image_base64: image_base64.to_string(),
            submit,
        },
    )
}

/// Start a new AI chat with a user message.
///
/// Creates a chat with a pre-generated ChatId so the caller can return it immediately.
/// If `submit` is true, the AI will stream a response. If false (noResponse), only the
/// user message is created.
#[allow(clippy::too_many_arguments)]
pub fn start_ai_chat(
    cx: &mut App,
    chat_id: ChatId,
    message: &str,
    parts: Vec<crate::ai::message_parts::AiContextPart>,
    image: Option<&str>,
    system_prompt: Option<&str>,
    model_id: Option<&str>,
    provider: Option<&str>,
    on_created: Option<std::sync::Arc<dyn Fn(String, String) + Send + Sync + 'static>>,
    submit: bool,
) -> bool {
    let handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let window_is_open = handle.is_some();
    let command_queued = get_pending_commands()
        .lock()
        .ok()
        .map(|mut commands| {
            ai_window_queue_command_if_open(
                &mut commands,
                window_is_open,
                AiCommand::StartChat {
                    chat_id,
                    message: message.to_string(),
                    parts,
                    image: image.map(|s| s.to_string()),
                    system_prompt: system_prompt.map(|s| s.to_string()),
                    model_id: model_id.map(|s| s.to_string()),
                    provider: provider.map(|s| s.to_string()),
                    on_created,
                    submit,
                },
            )
        })
        .unwrap_or(false);

    if !command_queued {
        tracing::warn!(
            chat_id = %chat_id,
            "Cannot start AI chat - AI window not open"
        );
        return false;
    }

    if let Some(handle) = handle {
        let _ = handle.update(cx, |_root, _window, cx| {
            cx.notify();
        });
    }

    info!(
        chat_id = %chat_id,
        submit = submit,
        has_image = image.is_some(),
        "ai_sdk.start_chat queued"
    );

    true
}

/// Add a file attachment to the AI window.
/// Used by file-reference actions like file search context menus.
pub fn add_ai_attachment(cx: &mut App, path: &str) -> Result<(), String> {
    enqueue_ai_window_command(
        cx,
        "add_attachment",
        AiCommand::AddAttachment {
            path: path.to_string(),
        },
    )
}

/// Show the AI command bar (Cmd+K menu) in the AI window.
///
/// This is triggered by the stdin command `{"type":"showAiCommandBar"}`.
/// Opens the AI window if not already open, then shows the command bar overlay.
pub fn show_ai_command_bar(cx: &mut App) {
    // First ensure the AI window is open
    if !is_ai_window_open() {
        if let Err(e) = open_ai_window(cx) {
            tracing::warn!(
                target: "ai",
                error = %e,
                "Failed to open AI window for command bar"
            );
            return;
        }
    }

    // Queue the command and notify the window to process it in render()
    push_ai_command(AiCommand::ShowCommandBar);

    // Notify the window to process the command
    let handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    if let Some(handle) = handle {
        let _ = handle.update(cx, |_root, _window, cx| {
            cx.notify();
        });
        tracing::info!(target: "ai", event = "show_ai_command_bar", "Command bar shown");
    } else {
        tracing::warn!(target: "ai", event = "show_ai_command_bar", status = "no_handle", "AI window handle not found");
    }
}

/// Simulate a key press in the AI window.
///
/// This is triggered by the stdin command `{"type":"simulateAiKey","key":"up","modifiers":["cmd"]}`.
/// Used for testing keyboard navigation in the AI window, especially the command bar.
///
/// Unlike other commands that queue for render-loop processing, simulate key
/// is dispatched directly via the `WeakEntity<AiApp>` to guarantee immediate
/// execution regardless of window render scheduling.
pub fn simulate_ai_key(cx: &mut App, key: &str, modifiers: Vec<KeyModifier>) {
    let handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let weak = AI_APP_WEAK
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .ok()
        .and_then(|g| g.clone());

    let Some(handle) = handle else {
        tracing::warn!(target: "ai", key, "Cannot simulate key - AI window not open");
        return;
    };

    let key_owned = key.to_string();
    let modifiers_owned = modifiers.clone();
    let fallback_to_queue = |cx: &mut App, reason: &'static str| {
        tracing::warn!(
            target: "ai",
            key = %key_owned,
            reason,
            "Falling back to queued simulate key dispatch"
        );
        let _ = enqueue_ai_window_command(
            cx,
            "simulate_key",
            AiCommand::SimulateKey {
                key: key_owned.clone(),
                modifiers: modifiers_owned.clone(),
            },
        );
    };

    let Some(weak) = weak else {
        fallback_to_queue(cx, "weak_ref_missing");
        return;
    };

    let mut dispatched = false;
    let result = handle.update(cx, |_root, window, cx| {
        if let Some(entity) = weak.upgrade() {
            entity.update(cx, |app, cx| {
                app.handle_simulated_key(&key_owned, &modifiers, window, cx);
                tracing::info!(
                    target: "ai",
                    key = %key_owned,
                    "simulate_ai_key_dispatched"
                );
            });
            dispatched = true;
        } else {
            tracing::warn!(target: "ai", key = %key_owned, "AiApp entity already dropped");
        }
    });
    if result.is_err() {
        tracing::warn!(target: "ai", key, "AI window handle invalid for simulate key");
        fallback_to_queue(cx, "window_handle_invalid");
    } else if !dispatched {
        fallback_to_queue(cx, "entity_dropped");
    }
}

/// Apply a preset by ID in the AI window.
///
/// Opens the AI window if needed, then creates a new chat with the preset's
/// system prompt and preferred model.
pub fn apply_ai_preset(cx: &mut App, preset_id: &str) {
    if !is_ai_window_open() {
        if let Err(e) = open_ai_window(cx) {
            tracing::warn!(
                target: "ai",
                error = %e,
                "Failed to open AI window for preset"
            );
            return;
        }
    }

    push_ai_command(AiCommand::ApplyPreset {
        preset_id: preset_id.to_string(),
    });

    let handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    if let Some(handle) = handle {
        let _ = handle.update(cx, |_root, _window, cx| {
            cx.notify();
        });
        tracing::info!(
            preset_id = %preset_id,
            action = "apply_ai_preset",
            "Applying preset in AI window"
        );
    }
}

/// Reload presets from disk in the AI window.
///
/// Call this after creating or importing presets to refresh the AI window's preset list.
pub fn reload_ai_presets(cx: &mut App) {
    if !is_ai_window_open() {
        return;
    }

    push_ai_command(AiCommand::ReloadPresets);

    let handle = {
        let slot = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };

    if let Some(handle) = handle {
        let _ = handle.update(cx, |_root, _window, cx| {
            cx.notify();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_display() -> crate::windows::DisplayBounds {
        crate::windows::DisplayBounds {
            origin_x: 0.0,
            origin_y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    fn assert_bounds_eq(actual: gpui::Bounds<gpui::Pixels>, expected: gpui::Bounds<gpui::Pixels>) {
        assert_eq!(f64::from(actual.origin.x), f64::from(expected.origin.x));
        assert_eq!(f64::from(actual.origin.y), f64::from(expected.origin.y));
        assert_eq!(f64::from(actual.size.width), f64::from(expected.size.width));
        assert_eq!(
            f64::from(actual.size.height),
            f64::from(expected.size.height)
        );
    }

    #[test]
    fn test_resolve_new_ai_window_bounds_returns_saved_bounds_when_visible() {
        let displays = vec![test_display()];
        let fallback = gpui::Bounds {
            origin: point(px(10.0), px(20.0)),
            size: size(px(900.0), px(700.0)),
        };
        let saved = crate::window_state::PersistedWindowBounds::new(100.0, 200.0, 910.0, 710.0);

        let actual = resolve_new_ai_window_bounds(Some(saved), &displays, fallback);

        assert_eq!(f64::from(actual.origin.x), 100.0);
        assert_eq!(f64::from(actual.origin.y), 200.0);
        assert_eq!(f64::from(actual.size.width), 910.0);
        assert_eq!(f64::from(actual.size.height), 710.0);
    }

    #[test]
    fn test_resolve_new_ai_window_bounds_returns_fallback_when_saved_is_offscreen() {
        let displays = vec![test_display()];
        let fallback = gpui::Bounds {
            origin: point(px(300.0), px(400.0)),
            size: size(px(900.0), px(700.0)),
        };
        let saved = crate::window_state::PersistedWindowBounds::new(5000.0, 5000.0, 900.0, 700.0);

        let actual = resolve_new_ai_window_bounds(Some(saved), &displays, fallback);

        assert_bounds_eq(actual, fallback);
    }

    #[test]
    fn test_resolve_new_ai_window_bounds_returns_fallback_when_no_saved_bounds() {
        let displays = vec![test_display()];
        let fallback = gpui::Bounds {
            origin: point(px(50.0), px(60.0)),
            size: size(px(900.0), px(700.0)),
        };

        let actual = resolve_new_ai_window_bounds(None, &displays, fallback);

        assert_bounds_eq(actual, fallback);
    }

    #[test]
    fn test_centered_ai_window_bounds_on_cursor_display_uses_mode_dimensions() {
        let full = centered_ai_window_bounds_on_cursor_display(AiWindowMode::Full);
        let mini = centered_ai_window_bounds_on_cursor_display(AiWindowMode::Mini);

        assert_eq!(f64::from(full.size.width), 900.0);
        assert_eq!(f64::from(full.size.height), 700.0);
        assert_eq!(f64::from(mini.size.width), 720.0);
        assert_eq!(f64::from(mini.size.height), 440.0);
    }

    #[test]
    fn test_opening_guard_blocks_concurrent_open_until_reset() {
        OPENING.store(false, Ordering::SeqCst);

        let first_attempt = OPENING.swap(true, Ordering::SeqCst);
        let second_attempt = OPENING.swap(true, Ordering::SeqCst);

        assert!(!first_attempt);
        assert!(second_attempt);

        OPENING.store(false, Ordering::SeqCst);
        let attempt_after_reset = OPENING.swap(true, Ordering::SeqCst);
        assert!(!attempt_after_reset);

        OPENING.store(false, Ordering::SeqCst);
    }
}
