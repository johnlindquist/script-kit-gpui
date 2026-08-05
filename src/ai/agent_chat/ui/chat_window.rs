//! Detachable Agent Chat Chat window.
//!
//! Creates a separate PopUp window for the Agent Chat chat that persists
//! independently from the main Script Kit panel.

use std::sync::{Mutex, OnceLock};

use gpui::{
    px, AnyWindowHandle, App, AppContext as _, Entity, Focusable, WeakEntity, WindowBounds,
    WindowKind, WindowOptions,
};

use super::thread::AgentChatThread;
use super::view::AgentChatView;
use crate::ai::context_selector::types::ContextPortalKind;
use crate::theme;

/// State for the detached Agent Chat Chat window.
struct ChatWindowState {
    handle: AnyWindowHandle,
    /// The live AgentChatView entity inside the detached window, if opened with a thread.
    view_entity: Option<WeakEntity<AgentChatView>>,
    /// Automation window ID registered in the runtime handle registry.
    /// Stored so we can remove the exact handle on close.
    automation_id: Option<String>,
}

/// Global handle to the detached Agent Chat Chat window.
static CHAT_WINDOW: OnceLock<Mutex<Option<ChatWindowState>>> = OnceLock::new();

/// Check if the detached Agent Chat Chat window is open.
pub fn is_chat_window_open() -> bool {
    let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
    let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().is_some()
}

/// Check if the given window is the detached Agent Chat chat window.
pub fn is_chat_window(window: &gpui::Window) -> bool {
    let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
    let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .map(|state| window.window_handle() == state.handle)
        .unwrap_or(false)
}

/// Clear the global chat window handle (called when the view closes itself).
pub fn clear_chat_window_handle() {
    let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = slot.lock() {
        if let Some(state) = g.take() {
            if let Some(ref id) = state.automation_id {
                crate::windows::remove_runtime_window_handle(id);
                crate::windows::remove_automation_window(id);
            }
        }
    }
}

/// Build standard window options for the detached chat window.
///
/// If `inherit_bounds` is Some, the detached window uses those bounds
/// (offset slightly right so it doesn't overlap the main panel).
fn chat_window_bounds(inherit_bounds: Option<gpui::Bounds<gpui::Pixels>>) -> WindowBounds {
    if let Some(bounds) = inherit_bounds {
        // Offset 20px right from the main window so both are visible
        return WindowBounds::Windowed(gpui::Bounds {
            origin: gpui::Point {
                x: bounds.origin.x + px(20.0),
                y: bounds.origin.y + px(20.0),
            },
            size: bounds.size,
        });
    }

    crate::window_state::load_window_bounds(crate::window_state::WindowRole::AgentChat)
        .map(|persisted| persisted.to_gpui())
        .unwrap_or_else(|| {
            WindowBounds::Windowed(gpui::Bounds {
                origin: gpui::Point {
                    x: px(100.0),
                    y: px(100.0),
                },
                size: gpui::Size {
                    width: px(480.0),
                    height: px(440.0),
                },
            })
        })
}

fn automation_bounds_from_window_bounds(
    bounds: WindowBounds,
) -> Option<crate::protocol::AutomationWindowBounds> {
    let bounds = match bounds {
        WindowBounds::Windowed(bounds)
        | WindowBounds::Maximized(bounds)
        | WindowBounds::Fullscreen(bounds) => bounds,
    };
    Some(crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    })
}

fn upsert_agent_chat_detached_automation_window(
    automation_id: &str,
    bounds: Option<crate::protocol::AutomationWindowBounds>,
) {
    crate::windows::upsert_automation_window(crate::protocol::AutomationWindowInfo {
        id: automation_id.to_string(),
        kind: crate::protocol::AutomationWindowKind::AgentChatDetached,
        title: Some("Script Kit Agent Chat".to_string()),
        focused: true,
        visible: true,
        semantic_surface: Some("agentChatChat".to_string()),
        bounds,
        parent_window_id: None,
        parent_kind: None,
        pid: Some(std::process::id()),
        generation: None,
    });
}

/// Move the detached Agent Chat window to deterministic bounds for visual proof.
pub fn set_chat_window_fixture_bounds(bounds: gpui::Bounds<gpui::Pixels>, cx: &mut App) -> bool {
    let (handle, automation_id) = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = guard.as_ref() else {
            return false;
        };
        let Some(id) = state.automation_id.clone() else {
            return false;
        };
        (state.handle, id)
    };

    let _ = handle.update(cx, |_root, window, cx| {
        crate::components::inline_popup_window::set_inline_popup_window_bounds(window, bounds, cx);
    });
    crate::windows::set_automation_bounds(
        &automation_id,
        Some(crate::protocol::AutomationWindowBounds {
            x: f32::from(bounds.origin.x) as f64,
            y: f32::from(bounds.origin.y) as f64,
            width: f32::from(bounds.size.width) as f64,
            height: f32::from(bounds.size.height) as f64,
        }),
    );
    true
}

fn chat_window_options(inherit_bounds: Option<gpui::Bounds<gpui::Pixels>>) -> WindowOptions {
    let window_bounds = chat_window_bounds(inherit_bounds);
    let window_background = if theme::get_cached_theme().is_vibrancy_enabled() {
        crate::platform::vibrancy_window_background()
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };

    WindowOptions {
        window_bounds: Some(window_bounds),
        titlebar: None,
        is_movable: true,
        window_background,
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        ..Default::default()
    }
}

/// Open (or focus) the detached Agent Chat Chat window with a placeholder.
pub fn open_chat_window(cx: &mut App) -> anyhow::Result<()> {
    // If already open, just focus it
    let existing = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        slot.lock().ok().and_then(|g| g.as_ref().map(|s| s.handle))
    };

    if existing.is_some() {
        activate_chat_window(cx);
        return Ok(());
    }

    let window_bounds = chat_window_bounds(None);
    let automation_bounds = automation_bounds_from_window_bounds(window_bounds);
    let handle = cx.open_window(chat_window_options(None), |window, cx| {
        window.on_window_should_close(cx, |window, cx| {
            let wb = window.window_bounds();
            crate::window_state::save_window_from_gpui(
                crate::window_state::WindowRole::AgentChat,
                wb,
            );
            let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
            if let Ok(mut g) = slot.lock() {
                if let Some(state) = g.take() {
                    if let Some(ref id) = state.automation_id {
                        crate::windows::remove_runtime_window_handle(id);
                        crate::windows::remove_automation_window(id);
                    }
                }
            }
            if crate::platform::begin_gpui_window_exit_dematerialize(
                window,
                "AGENT_CHAT",
                "Agent Chat",
            ) {
                crate::platform::remove_gpui_window_after_glass_exit_from_app(window, cx);
                false
            } else {
                true
            }
        });
        let view = cx.new(ChatWindowPlaceholder::new);
        let focus_handle = view.read(cx).focus_handle.clone();
        window.focus(&focus_handle, cx);
        view
    })?;

    let any_handle: AnyWindowHandle = handle.into();
    let automation_id = "agentChatDetached:placeholder".to_string();

    // Store the handle (placeholder has no AgentChatView entity)
    {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        if let Ok(mut g) = slot.lock() {
            *g = Some(ChatWindowState {
                handle: any_handle,
                view_entity: None,
                automation_id: Some(automation_id.clone()),
            });
        }
    }

    // Register the exact runtime handle so simulateGpuiEvent can target
    // this window by its automation ID without collapsing to WindowRole.
    crate::windows::upsert_runtime_window_handle(&automation_id, any_handle);
    upsert_agent_chat_detached_automation_window(&automation_id, automation_bounds);
    configure_agent_chat_vibrancy(cx);

    tracing::info!("agent_chat_window_opened");
    Ok(())
}

/// Open the detached Agent Chat Chat window with an existing AgentChatThread entity.
/// This is used when "Detach to Window" transfers a live conversation.
///
/// If `inherit_bounds` is provided, the detached window opens at those bounds
/// (offset +20px x/y) instead of using persisted Agent Chat chat bounds.
pub fn open_chat_window_with_thread(
    thread: Entity<AgentChatThread>,
    inherit_bounds: Option<gpui::Bounds<gpui::Pixels>>,
    cx: &mut App,
) -> anyhow::Result<()> {
    // Close existing if any
    if is_chat_window_open() {
        close_chat_window(cx);
    }

    let has_inherited_bounds = inherit_bounds.is_some();

    // Read the thread's UI ID before the closure moves ownership of `thread`.
    let ui_thread_id = thread.read(cx).ui_thread_id().to_string();

    let view_entity_slot: std::sync::Arc<Mutex<Option<WeakEntity<AgentChatView>>>> =
        std::sync::Arc::new(Mutex::new(None));
    let view_entity_slot_inner = view_entity_slot.clone();
    let view_entity_slot_on_close = view_entity_slot.clone();

    let window_bounds = chat_window_bounds(inherit_bounds);
    let automation_bounds = automation_bounds_from_window_bounds(window_bounds);
    let handle = cx.open_window(chat_window_options(inherit_bounds), |window, cx| {
        // Save bounds and clear handle when window closes
        window.on_window_should_close(cx, move |window, cx| {
            let close_allowed = if let Ok(slot) = view_entity_slot_on_close.lock() {
                slot.as_ref()
                    .and_then(|weak| weak.upgrade())
                    .is_none_or(|entity| {
                        entity.update(cx, |view, cx| view.allow_native_close_request(cx))
                    })
            } else {
                false
            };
            if !close_allowed {
                return false;
            }

            let wb = window.window_bounds();
            crate::window_state::save_window_from_gpui(
                crate::window_state::WindowRole::AgentChat,
                wb,
            );
            if let Ok(slot) = view_entity_slot_on_close.lock() {
                if let Some(entity) = slot.as_ref().and_then(|weak| weak.upgrade()) {
                    entity.update(cx, |view, cx| {
                        view.prepare_for_host_hide(cx);
                    });
                }
            }
            // Clear the global handle and remove the runtime automation handle
            let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
            if let Ok(mut g) = slot.lock() {
                if let Some(state) = g.take() {
                    if let Some(ref id) = state.automation_id {
                        crate::windows::remove_runtime_window_handle(id);
                        crate::windows::remove_automation_window(id);
                    }
                }
            }
            if crate::platform::begin_gpui_window_exit_dematerialize(
                window,
                "AGENT_CHAT",
                "Agent Chat",
            ) {
                crate::platform::remove_gpui_window_after_glass_exit_from_app(window, cx);
                false
            } else {
                true
            }
        });

        let view = cx.new(|cx| AgentChatView::new(thread, cx));
        let entity_weak = view.downgrade();
        view.update(cx, |view, _cx| {
            view.set_allowed_portal_kinds(vec![ContextPortalKind::AgentChatHistory]);
            view.set_on_toggle_actions(move |_window, cx| {
                // The callback runs while the detached host window is updating.
                // Opening Actions immediately would re-enter that same window
                // handle to measure its bounds and the nested update is rejected.
                // Defer to the next App turn so the real popup owner can read the
                // host safely; the user still gets one Cmd+K transition.
                cx.defer(toggle_detached_actions);
            });
            view.set_on_close_requested(move |_window, cx| {
                close_chat_window(cx);
            });
            view.set_on_open_history_command(move |_window, cx| {
                let _ = open_detached_history_actions(cx);
            });
            let paste_entity_weak = entity_weak.clone();
            view.set_on_paste_response_requested(move |_window, cx| {
                dispatch_detached_action(&paste_entity_weak, "agent_chat_paste_to_frontmost", cx);
            });
            view.set_on_open_portal(move |kind, cx| match kind {
                ContextPortalKind::AgentChatHistory => {
                    let opened = open_history_portal_in_detached_chat_window(cx);
                    if !opened {
                        let _ = cancel_portal_session_in_detached_chat_window(kind, cx);
                    }
                    tracing::info!(
                        target: "script_kit::agent_chat",
                        event = "detached_agent_chat_portal_requested",
                        kind = "AgentChatHistory",
                        opened,
                    );
                }
                other => {
                    tracing::info!(
                        target: "script_kit::agent_chat",
                        event = "detached_agent_chat_portal_requested",
                        kind = ?other,
                        opened = false,
                        reason = "unsupported_in_detached_host",
                    );
                }
            });
        });
        // Capture weak reference to the view entity for action dispatch.
        if let Ok(mut slot) = view_entity_slot_inner.lock() {
            *slot = Some(view.downgrade());
        }
        cx.new(|cx| gpui_component::Root::new(view, window, cx))
    })?;

    // Extract the captured weak entity.
    let view_weak = view_entity_slot.lock().ok().and_then(|mut g| g.take());

    let any_handle: AnyWindowHandle = handle.into();
    let automation_id = format!("agentChatDetached:{ui_thread_id}");

    {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        if let Ok(mut g) = slot.lock() {
            *g = Some(ChatWindowState {
                handle: any_handle,
                view_entity: view_weak,
                automation_id: Some(automation_id.clone()),
            });
        }
    }

    // Register the exact runtime handle so simulateGpuiEvent can target
    // this window by its automation ID without collapsing to WindowRole.
    crate::windows::upsert_runtime_window_handle(&automation_id, any_handle);

    // Also register the window in the automation metadata registry so that
    // `listAutomationWindows` and target-resolution by kind=agentChatDetached can
    // find it. Runtime handle registry and metadata registry are separate —
    // a missing metadata entry makes the window invisible to discovery even
    // when the runtime handle exists.
    upsert_agent_chat_detached_automation_window(&automation_id, automation_bounds);

    // Activate the detached window so it gets keyboard focus immediately.
    activate_chat_window(cx);

    // Configure vibrancy to match main window appearance
    configure_agent_chat_vibrancy(cx);

    tracing::info!(
        event = "agent_chat_window_opened_with_thread",
        has_inherited_bounds,
        activated = true,
    );
    Ok(())
}

/// Return a strong reference to the detached Agent Chat chat view entity, if the
/// detached window is open and was opened with a live thread.
///
/// This is used by the automation substrate to read Agent Chat state and test-probe
/// data from the detached window without routing through the main window.
pub fn get_detached_agent_chat_view_entity() -> Option<Entity<AgentChatView>> {
    let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
    let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .and_then(|state| state.view_entity.as_ref())
        .and_then(|weak| weak.upgrade())
}

/// Apply the existing deterministic Agent Chat fixture to the detached host.
/// This is the narrow automation seam used to prove identical Stop/Retry/copy
/// behavior across embedded and detached windows.
pub(crate) fn apply_detached_agent_chat_test_fixture(
    phase: &str,
    user_text: Option<String>,
    assistant_text: Option<String>,
    message_count: Option<usize>,
    cx: &mut App,
) -> Result<(), String> {
    let entity = get_detached_agent_chat_view_entity()
        .ok_or_else(|| "Detached Agent Chat view is not active".to_string())?;
    entity.update(cx, |view, cx| {
        view.apply_test_fixture(phase, user_text, assistant_text, message_count, cx)
    })
}

fn open_picker_in_detached_chat_window(
    cx: &mut App,
    open_picker: impl FnOnce(&mut AgentChatView, &mut gpui::Window, &mut gpui::Context<AgentChatView>),
) -> bool {
    let (handle, entity) = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = match slot.lock() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        };
        let Some(state) = guard.as_ref() else {
            return false;
        };
        let Some(entity) = state.view_entity.as_ref().and_then(|weak| weak.upgrade()) else {
            return false;
        };
        (state.handle, entity)
    };

    handle
        .update(cx, |_root, window, cx| {
            entity.update(cx, |view, cx| {
                open_picker(view, window, cx);
            });
        })
        .is_ok()
}

pub(crate) fn open_agent_chat_history_popup_fixture(cx: &mut App) -> bool {
    open_history_portal_in_detached_chat_window(cx)
}

fn open_history_portal_in_detached_chat_window(cx: &mut App) -> bool {
    let (handle, entity) = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = match slot.lock() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        };
        let Some(state) = guard.as_ref() else {
            return false;
        };
        let Some(entity) = state.view_entity.as_ref().and_then(|weak| weak.upgrade()) else {
            return false;
        };
        (state.handle, entity)
    };

    handle
        .update(cx, |_root, window, cx| {
            window.activate_window();
            let parent_handle = window.window_handle();
            let parent_bounds = window.bounds();
            let display_id = window.display(cx).map(|display| display.id());
            entity.update(cx, |view, cx| {
                let query = view.take_pending_history_portal_query().unwrap_or_default();
                tracing::info!(
                    target: "script_kit::agent_chat",
                    event = "detached_agent_chat_history_portal_query_seeded_from_contract",
                    query = %query,
                );
                let hits = crate::ai::agent_chat::ui::history::search_history(&query, 12);
                view.open_history_portal_with_entries(query, hits, cx);
                view.open_history_popup_from_host(parent_handle, parent_bounds, display_id, cx);
            });
            let focus_handle = entity.read(cx).focus_handle(cx);
            window.focus(&focus_handle, cx);
        })
        .is_ok()
}

fn cancel_portal_session_in_detached_chat_window(kind: ContextPortalKind, cx: &mut App) -> bool {
    let entity = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = match slot.lock() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        };
        let Some(state) = guard.as_ref() else {
            return false;
        };
        let Some(entity) = state.view_entity.as_ref().and_then(|weak| weak.upgrade()) else {
            return false;
        };
        entity
    };

    entity.update(cx, |view, cx| view.cancel_pending_portal_session(kind, cx))
}

pub fn open_detached_slash_picker(cx: &mut App) -> bool {
    open_picker_in_detached_chat_window(cx, |view, window, cx| {
        view.open_slash_picker_in_window(window, cx);
    })
}

pub fn submit_reused_entry_intent_in_detached_chat(
    intent: String,
    cx: &mut App,
) -> anyhow::Result<bool> {
    let state = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().map(|state| {
            (
                state.handle,
                state.view_entity.as_ref().and_then(|weak| weak.upgrade()),
            )
        })
    };

    let Some((handle, Some(view_entity))) = state else {
        return Ok(false);
    };

    let input_len = intent.len();
    let reused = handle
        .update(cx, |_root, window, cx| {
            window.activate_window();
            let reused = view_entity.update(cx, |chat, cx| {
                if chat.is_setup_mode() {
                    return false;
                }

                chat.submit_reused_entry_intent(intent.clone(), cx);
                true
            });
            let focus_handle = view_entity.read(cx).focus_handle(cx);
            window.focus(&focus_handle, cx);
            Ok::<bool, anyhow::Error>(reused)
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))??;

    tracing::info!(
        target: "script_kit::tab_ai",
        event = "tab_ai_reused_detached_window_with_entry_intent",
        reused,
        input_len,
    );
    Ok(reused)
}

pub fn submit_reused_entry_intent_with_host_context_in_detached_chat(
    intent: String,
    parts: Vec<crate::ai::message_parts::AiContextPart>,
    source: &'static str,
    cx: &mut App,
) -> anyhow::Result<bool> {
    let state = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().map(|state| {
            (
                state.handle,
                state.view_entity.as_ref().and_then(|weak| weak.upgrade()),
            )
        })
    };

    let Some((handle, Some(view_entity))) = state else {
        return Ok(false);
    };

    let input_len = intent.len();
    let part_count = parts.len();
    let reused = handle
        .update(cx, |_root, window, cx| {
            window.activate_window();
            let reused = view_entity.update(cx, |chat, cx| {
                if chat.is_setup_mode() {
                    return Ok(false);
                }

                chat.submit_reused_entry_intent_with_host_context(
                    intent.clone(),
                    parts.clone(),
                    source,
                    cx,
                )?;
                Ok::<bool, String>(true)
            });
            let focus_handle = view_entity.read(cx).focus_handle(cx);
            window.focus(&focus_handle, cx);
            reused.map_err(anyhow::Error::msg)
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))??;

    tracing::info!(
        target: "script_kit::tab_ai",
        event = "tab_ai_reused_detached_window_with_host_context_entry_intent",
        reused,
        source,
        input_len,
        part_count,
    );
    Ok(reused)
}

pub(crate) fn request_detached_conversation_dismiss(
    trigger: crate::components::conversation_actions::ConversationDismissTrigger,
    cx: &mut App,
) -> bool {
    let state = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = match slot.lock() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        };
        guard
            .as_ref()
            .map(|state| (state.handle, state.view_entity.clone()))
    };
    let Some((handle, Some(view_weak))) = state else {
        return false;
    };
    handle
        .update(cx, move |_root, window, cx| {
            let Some(view) = view_weak.upgrade() else {
                return false;
            };
            view.update(cx, |chat, cx| {
                matches!(
                    chat.request_conversation_dismiss(trigger, window, cx),
                    crate::components::conversation_actions::ConversationCommandExecution::Executed
                )
            })
        })
        .unwrap_or(false)
}

/// Close the detached Agent Chat Chat window.
#[allow(dead_code)]
pub fn close_chat_window(cx: &mut App) {
    let existing = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        slot.lock().ok().and_then(|mut g| g.take())
    };

    if let Some(state) = existing {
        if let Some(entity) = state.view_entity.as_ref().and_then(|weak| weak.upgrade()) {
            entity.update(cx, |view, cx| {
                view.prepare_for_host_hide(cx);
            });
        }

        // Remove both the runtime handle AND the automation registry
        // entry before `window.remove_window()`. The on_close callback
        // registered in `chat_window_options` would normally do both,
        // but this helper has already taken CHAT_WINDOW.slot above so
        // the callback's `g.take()` returns `None` and the
        // registry-cleanup branch never runs. Doing it here keeps
        // `listAutomationWindows` in sync with the window's actual
        // lifetime regardless of whether close is initiated from the
        // window's titlebar (on_close fires first) or from an outside
        // TriggerAction / automation path (this helper fires first).
        if let Some(ref id) = state.automation_id {
            crate::windows::remove_runtime_window_handle(id);
            crate::windows::remove_automation_window(id);
        }

        let _ = state.handle.update(cx, |_root, window, _cx| {
            // Save window bounds before closing
            let wb = window.window_bounds();
            crate::window_state::save_window_from_gpui(
                crate::window_state::WindowRole::AgentChat,
                wb,
            );
            crate::platform::dematerialize_then_remove_gpui_window_from_app(
                window,
                _cx,
                "AGENT_CHAT",
                "Agent Chat",
            );
        });
    }
}

// Detached Agent Chat action allowlist now lives in the Agent Chat route builder layer:
// `AgentChatActionsDialogHost::Detached` in src/actions/builders/script_context.rs.

/// Dispatch an action to the detached Agent Chat chat window from outside it.
///
/// Used by the automation substrate's `ExternalCommand::TriggerAction`
/// dispatcher when `host="agentChatDetached"`: routes the supplied action id
/// through the same `dispatch_detached_action` that the detached
/// window's own actions popup uses, so closing / reattaching / model
/// switching from automation is end-to-end identical to clicking the
/// action in the detached popup.
///
/// Returns `false` if no detached window exists or its view entity has
/// been deallocated, so callers can distinguish a dropped dispatch
/// from a successful one.
pub fn dispatch_action_to_detached(action_id: &str, cx: &mut App) -> bool {
    let view_weak = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = match slot.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        guard.as_ref().and_then(|state| state.view_entity.clone())
    };
    let Some(view_weak) = view_weak else {
        tracing::warn!(
            event = "detached_action_dispatch_no_window",
            action = %action_id,
        );
        return false;
    };
    dispatch_detached_action_checked(&view_weak, action_id, cx)
}

/// Reattach the detached conversation into the main panel from the detached
/// window's own actions menu.
///
/// Routes through `ScriptListApp::reattach_embedded_agent_chat_from_detached`
/// (which pulls the live thread out of the detached view, closes this window,
/// and rebuilds the embedded chat around the same thread), then reveals the
/// main window. Falls back to a plain close when the main app entity is
/// unreachable so the action never silently no-ops.
fn reattach_detached_chat_into_main_panel(cx: &mut App) -> bool {
    if crate::get_main_window_handle().is_none() {
        tracing::warn!(event = "detached_reattach_no_main_window_handle");
        close_chat_window(cx);
        return false;
    }
    // Deferred: this action can arrive while the main app is already on the
    // GPUI update stack (automation TriggerAction dispatch), so updating the
    // app entity inline would re-enter it. Defer until the stack unwinds.
    cx.defer(move |cx| {
        let reattached = REATTACH_INTO_MAIN_HOOK
            .get()
            .map(|hook| hook(cx))
            .unwrap_or(false);
        if !reattached {
            tracing::warn!(event = "detached_reattach_main_app_unreachable");
            close_chat_window(cx);
        }
        tracing::info!(event = "detached_reattach_into_main_panel", reattached);
    });
    true
}

/// Binary-registered hook that reattaches the detached chat into the main
/// panel. The main app type lives in the binary crate, so this dual-compiled
/// file cannot downcast the main window's root view itself; the binary
/// registers the downcast-and-reattach closure at app startup.
static REATTACH_INTO_MAIN_HOOK: std::sync::OnceLock<fn(&mut App) -> bool> =
    std::sync::OnceLock::new();

pub fn register_reattach_into_main_hook(hook: fn(&mut App) -> bool) {
    let _ = REATTACH_INTO_MAIN_HOOK.set(hook);
}

/// Activate (bring to front) the detached chat window.
pub(crate) fn activate_chat_window(cx: &mut App) {
    let state = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        slot.lock().ok().and_then(|g| {
            g.as_ref().map(|s| {
                (
                    s.handle,
                    s.view_entity.as_ref().and_then(|weak| weak.upgrade()),
                )
            })
        })
    };
    if let Some((handle, view_entity)) = state {
        let _ = handle.update(cx, |_root, window, cx| {
            window.activate_window();
            if let Some(ref entity) = view_entity {
                let focus_handle = entity.read(cx).focus_handle(cx);
                window.focus(&focus_handle, cx);
            }
        });
        tracing::info!(event = "agent_chat_window_activated");
    }
}

/// Toggle the actions popup from the detached Agent Chat chat window.
///
/// Creates a dialog with the subset of Agent Chat chat actions that work in the
/// detached context, positioned relative to the detached chat window.
/// After selection, the detached chat re-gains focus.
pub fn toggle_detached_actions(cx: &mut App) {
    use crate::actions::{self, ActionsDialog, WindowPosition};

    let actions_window_open_before = actions::is_actions_window_open();

    // If actions are already open, close them and re-focus the chat (toggle behavior)
    if actions_window_open_before {
        actions::close_actions_window(cx);
        activate_chat_window(cx);
        tracing::info!(
            target: "script_kit::keyboard",
            event = "detached_actions_toggle_result",
            route_owner = "detached_chat",
            restored_chat_focus = true,
            actions_window_open_before,
            actions_window_open_after = actions::is_actions_window_open(),
        );
        return;
    }

    let (handle, view_weak) = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = match slot.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        match guard.as_ref() {
            Some(state) => (state.handle, state.view_entity.clone()),
            None => return,
        }
    };

    if view_weak.is_none() {
        tracing::warn!(target: "script_kit::keyboard", event = "detached_actions_no_view_entity");
        return;
    }

    // Get window bounds and display from the detached chat window
    let window_info = handle.update(cx, |_root, window, cx| {
        (
            window.window_handle(),
            window.bounds(),
            window.display(cx).map(|d| d.id()),
        )
    });

    let Ok((parent_window_handle, bounds, display_id)) = window_info else {
        return;
    };

    let theme_arc = std::sync::Arc::new(crate::theme::get_cached_theme());

    // Channel for the on_select callback to send the selected action_id
    // to the async dispatch task that has App context.
    let (action_tx, action_rx) = async_channel::bounded::<String>(1);

    let callback: std::sync::Arc<dyn Fn(String) + Send + Sync> =
        std::sync::Arc::new(move |action_id: String| {
            tracing::info!(
                event = "detached_action_selected_from_popup",
                action = %action_id,
            );
            let _ = action_tx.try_send(action_id);
        });

    // Build Agent Chat model context from the view entity (mirrors actions_toggle.rs pattern)
    #[allow(clippy::type_complexity)]
    let agent_chat_context: Option<(
        Option<String>,
        Vec<crate::ai::agent_chat::ui::config::AgentChatModelEntry>,
        usize,
        Vec<crate::ai::agent_chat::ui::AgentChatThreadSummary>,
        Vec<crate::ai::agent_chat::ui::AgentChatForkPoint>,
        Vec<crate::actions::Action>,
        crate::components::conversation_actions::AgentChatConversationCommandFacts,
    )> = view_weak.as_ref().and_then(|weak| {
        weak.upgrade().map(|entity| {
            let view = entity.read(cx);
            let thread_summaries = view.retained_thread_summaries(cx);
            let recovery_actions = view.ai_recovery_actions(cx);
            match &view.session {
                crate::ai::agent_chat::ui::AgentChatSession::Setup(_) => (
                    None,
                    Vec::new(),
                    0,
                    thread_summaries,
                    Vec::new(),
                    Vec::new(),
                    crate::components::conversation_actions::AgentChatConversationCommandFacts {
                        response_in_progress: false,
                        waiting_for_permission: false,
                        context_preparing: false,
                        composer_has_text: false,
                        retry_available: false,
                        has_response: false,
                        ..Default::default()
                    },
                ),
                crate::ai::agent_chat::ui::AgentChatSession::Live(thread) => {
                    let thread = thread.read(cx);
                    (
                        thread.selected_model_id().map(str::to_string),
                        thread.available_models().to_vec(),
                        thread.standing_approvals().len(),
                        thread_summaries,
                        thread.fork_points().to_vec(),
                        recovery_actions,
                        view.conversation_command_facts(cx),
                    )
                }
            }
        })
    });

    let (
        selected_model_id,
        available_models,
        standing_approval_count,
        thread_summaries,
        fork_points,
        recovery_actions,
        command_facts,
    ) = agent_chat_context.unwrap_or_else(|| {
        (
            None,
            Vec::new(),
            0,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            crate::components::conversation_actions::AgentChatConversationCommandFacts {
                response_in_progress: false,
                waiting_for_permission: false,
                context_preparing: false,
                composer_has_text: false,
                retry_available: false,
                has_response: false,
                ..Default::default()
            },
        )
    });

    let dialog = cx.new(|cx| {
        let focus_handle = cx.focus_handle();
        let mut dialog = ActionsDialog::with_agent_chat_for_host(
            focus_handle,
            callback,
            crate::actions::AgentChatActionsDialogContext {
                available_models: &available_models,
                selected_model_id: selected_model_id.as_deref(),
                focused_text: false,
                focused_text_expanded: false,
                standing_approval_count,
                thread_summaries: &thread_summaries,
                fork_points: &fork_points,
                command_facts,
            },
            theme_arc,
            crate::actions::AgentChatActionsDialogHost::Detached,
        );
        dialog.prepend_root_route_actions(recovery_actions.clone());
        dialog.set_skip_track_focus(true);
        dialog
    });

    let activation_dialog = dialog.clone();
    dialog.update(cx, |dialog, _cx| {
        dialog.set_on_activation(std::sync::Arc::new(move |activation, _window, cx| match activation {
            crate::actions::ActionsDialogActivation::DrillDownPushed { .. } => {
                let (route_id, search_placeholder, route_depth, escape_hint) = {
                    let dialog_ref = activation_dialog.read(cx);
                    (
                        dialog_ref.current_route_id().map(str::to_string),
                        dialog_ref.current_search_placeholder().map(str::to_string),
                        dialog_ref.route_depth(),
                        dialog_ref.route_hint_label(),
                    )
                };
                tracing::info!(
                    target: "script_kit::actions",
                    host = "detached_agent_chat_actions_window",
                    route_id = ?route_id,
                    route_depth,
                    escape_hint,
                    search_placeholder = ?search_placeholder,
                    "actions_dialog_route_visible"
                );
            }
            crate::actions::ActionsDialogActivation::Executed { should_close, .. } => {
                if should_close {
                    let on_close = activation_dialog.read(cx).on_close.clone();
                    if let Some(on_close) = on_close {
                        on_close(cx);
                    }
                    crate::actions::close_actions_window(cx);
                }
            }
            crate::actions::ActionsDialogActivation::Blocked { .. }
            | crate::actions::ActionsDialogActivation::NoSelection => {}
        }));
        dialog.set_on_close(std::sync::Arc::new(|cx| {
            activate_chat_window(cx);
            tracing::info!(target: "script_kit::keyboard", event = "detached_actions_closed_restore_chat_focus");
        }));
    });

    let parent_automation_id = crate::windows::focused_automation_window_id();
    let actions_handle = match actions::open_actions_window(
        cx,
        parent_window_handle,
        bounds,
        display_id,
        dialog,
        WindowPosition::TopRight,
        parent_automation_id.as_deref(),
    ) {
        Ok(handle) => handle,
        Err(e) => {
            tracing::warn!(target: "script_kit::keyboard", %e, "detached_actions_open_failed");
            return;
        }
    };

    let _ = actions_handle.update(cx, |_root, window, _cx| {
        window.activate_window();
    });

    tracing::info!(
        target: "script_kit::keyboard",
        event = "detached_actions_toggle_result",
        route_owner = "detached_chat",
        restored_chat_focus = false,
        actions_window_open_before,
        actions_window_open_after = crate::actions::is_actions_window_open(),
        has_view_entity = view_weak.is_some(),
    );

    // Spawn a one-shot task that receives the selected action_id from the
    // channel, dispatches it to the AgentChatView entity, and re-focuses the chat.
    if let Some(entity_weak) = view_weak {
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            if let Ok(action_id) = action_rx.recv().await {
                if action_id == "__cancel__" {
                    return;
                }
                tracing::info!(
                    event = "detached_action_dispatch",
                    action = %action_id,
                );
                cx.update(|cx| {
                    let handled = dispatch_detached_action_checked(&entity_weak, &action_id, cx);
                    // Re-focus the detached chat after action dispatch
                    // (unless the action closed the window)
                    if handled && action_id != "agent_chat_close" {
                        activate_chat_window(cx);
                    }
                    tracing::info!(
                        event = "detached_action_dispatch_completed",
                        action = %action_id,
                        handled,
                    );
                });
            }
        })
        .detach();
    }
}

fn open_detached_history_actions(cx: &mut App) -> bool {
    use crate::actions::{self, ActionsDialog, WindowPosition};

    if actions::is_actions_window_open() {
        actions::close_actions_window(cx);
    }

    let (handle, view_weak) = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = match slot.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        match guard.as_ref() {
            Some(state) => (state.handle, state.view_entity.clone()),
            None => return false,
        }
    };

    let Some(entity_weak) = view_weak else {
        tracing::warn!(target: "script_kit::keyboard", event = "detached_history_actions_no_view_entity");
        return false;
    };

    let Ok((parent_window_handle, bounds, display_id)) = handle.update(cx, |_root, window, cx| {
        (
            window.window_handle(),
            window.bounds(),
            window.display(cx).map(|d| d.id()),
        )
    }) else {
        return false;
    };

    let route = crate::actions::get_agent_chat_history_route();
    let theme_arc = std::sync::Arc::new(crate::theme::get_cached_theme());
    let (action_tx, action_rx) = async_channel::bounded::<String>(1);
    let callback: std::sync::Arc<dyn Fn(String) + Send + Sync> =
        std::sync::Arc::new(move |action_id: String| {
            tracing::info!(
                event = "detached_history_action_selected",
                action = %action_id,
            );
            let _ = action_tx.try_send(action_id);
        });

    let dialog = cx.new(|cx| {
        let focus_handle = cx.focus_handle();
        let mut dialog = ActionsDialog::from_actions_with_context(
            focus_handle,
            callback,
            route.actions.clone(),
            None,
            None,
            theme_arc,
            crate::designs::DesignVariant::Default,
            route.context_title.clone(),
            ActionsDialog::agent_chat_dialog_config(),
        );
        dialog.set_root_route(route);
        dialog.set_skip_track_focus(true);
        dialog
    });

    let activation_dialog = dialog.clone();
    dialog.update(cx, |dialog, _cx| {
        dialog.set_on_activation(std::sync::Arc::new(move |activation, _window, cx| match activation {
            crate::actions::ActionsDialogActivation::Executed { should_close, .. } => {
                if should_close {
                    let on_close = activation_dialog.read(cx).on_close.clone();
                    if let Some(on_close) = on_close {
                        on_close(cx);
                    }
                    crate::actions::close_actions_window(cx);
                }
            }
            crate::actions::ActionsDialogActivation::DrillDownPushed { .. }
            | crate::actions::ActionsDialogActivation::Blocked { .. }
            | crate::actions::ActionsDialogActivation::NoSelection => {}
        }));
        dialog.set_on_close(std::sync::Arc::new(|cx| {
            activate_chat_window(cx);
            tracing::info!(target: "script_kit::keyboard", event = "detached_history_actions_closed_restore_chat_focus");
        }));
    });

    let parent_automation_id = crate::windows::focused_automation_window_id();
    let actions_handle = match actions::open_actions_window(
        cx,
        parent_window_handle,
        bounds,
        display_id,
        dialog,
        WindowPosition::TopRight,
        parent_automation_id.as_deref(),
    ) {
        Ok(handle) => handle,
        Err(e) => {
            tracing::warn!(target: "script_kit::keyboard", %e, "detached_history_actions_open_failed");
            return false;
        }
    };

    let _ = actions_handle.update(cx, |_root, window, _cx| {
        window.activate_window();
    });

    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        if let Ok(action_id) = action_rx.recv().await {
            cx.update(|cx| {
                let handled = dispatch_detached_action_checked(&entity_weak, &action_id, cx);
                if handled {
                    activate_chat_window(cx);
                }
                tracing::info!(
                    event = "detached_history_action_dispatch_completed",
                    action = %action_id,
                    handled,
                );
            });
        }
    })
    .detach();

    tracing::info!(
        target: "script_kit::keyboard",
        event = "detached_history_actions_opened",
        actions_window_open_after = crate::actions::is_actions_window_open(),
    );
    true
}

/// Checked wrapper around `dispatch_detached_action` that logs when the
/// view entity has already been deallocated and avoids a silent no-op.
fn dispatch_detached_action_checked(
    entity_weak: &WeakEntity<AgentChatView>,
    action_id: &str,
    cx: &mut App,
) -> bool {
    if entity_weak.upgrade().is_none() {
        tracing::warn!(
            event = "detached_action_dispatch_dropped_no_view",
            action = %action_id,
        );
        return false;
    }
    dispatch_detached_action(entity_weak, action_id, cx);
    true
}

/// Dispatch an action to the detached AgentChatView entity.
///
/// Handles the subset of Agent Chat chat actions that make sense in the detached
/// window context (copy, scroll, expand/collapse, close, reattach, etc.).
fn dispatch_detached_action(
    entity_weak: &WeakEntity<AgentChatView>,
    action_id: &str,
    cx: &mut App,
) {
    tracing::info!(
        event = "agent_chat_actions_menu_selected",
        host = "detached",
        action_id,
        "Selected Agent Chat Actions Menu item"
    );

    let model_id = crate::actions::agent_chat_switch_model_id_from_action(action_id)
        .or_else(|| action_id.strip_prefix("chat:select_model_"));
    if let Some(model_id) = model_id {
        if let Some(entity) = entity_weak.upgrade() {
            entity.update(cx, |chat, cx| {
                if let Some(thread) = chat.thread() {
                    thread.update(cx, |thread, cx| {
                        thread.select_model(model_id, cx);
                    });
                }
            });
            tracing::info!(
                event = "detached_action_switch_model",
                model_id = %model_id,
            );
        }
        return;
    }

    if let Some(session_id) =
        action_id.strip_prefix(crate::actions::AGENT_CHAT_HISTORY_SELECT_ACTION_PREFIX)
    {
        if let Some(entity) = entity_weak.upgrade() {
            entity.update(cx, |chat, cx| {
                let selected = chat.select_history_session_by_id(session_id, cx);
                tracing::info!(
                    event = "detached_action_history_selected",
                    session_id = %session_id,
                    selected,
                );
            });
        }
        return;
    }

    if let Some(request_id) =
        crate::actions::agent_chat_receipt_history_request_id_from_action(action_id)
    {
        if let Some(entry) =
            crate::agentic_protocol_bus::find_protocol_response_by_request_id(request_id)
        {
            let json = serde_json::to_string_pretty(&entry).unwrap_or_else(|_| "{}".to_string());
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(json));
            tracing::info!(
                event = "detached_action_receipt_history_copied",
                request_id,
                response_type = %entry.response_type,
            );
        } else {
            tracing::warn!(
                event = "detached_action_receipt_history_missing",
                request_id,
            );
        }
        return;
    }

    if let Some(adapter_id) = crate::ai::agent_prompt_handoff::adapter_from_action_id(action_id) {
        if let Some(entity) = entity_weak.upgrade() {
            let result = entity.update(cx, |chat, cx| {
                chat.current_prompt_handoff_payload(adapter_id, cx)
                    .and_then(|payload| {
                        crate::ai::agent_prompt_handoff::launch_prompt_handoff(&payload)
                    })
            });
            match result {
                Ok(receipt) => {
                    tracing::info!(
                        target: "script_kit::agent_handoff",
                        event = "detached_agent_prompt_handoff_succeeded",
                        adapter_id = %receipt.adapter_id,
                        action_id = %receipt.action_id,
                        dry_run = receipt.dry_run,
                        prompt_chars = receipt.prompt_chars,
                        prompt_sha256 = %receipt.prompt_sha256,
                        spawned = receipt.spawned,
                        pid = ?receipt.pid,
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        target: "script_kit::agent_handoff",
                        event = "detached_agent_prompt_handoff_failed",
                        action_id,
                        error = %error.user_message(),
                    );
                }
            }
        }
        return;
    }

    if let Some(prompt_action) =
        crate::ai::agent_prompt_handoff::prompt_action_from_action_id(action_id)
    {
        if let Some(entity) = entity_weak.upgrade() {
            let result = entity.update(cx, |chat, cx| {
                chat.current_prompt_handoff_payload(
                    crate::ai::agent_prompt_handoff::AgentPromptHandoffAdapterId::CmuxCodex,
                    cx,
                )
                .and_then(|payload| {
                    crate::ai::agent_prompt_handoff::export_prompt(&payload, prompt_action)
                })
            });
            match result {
                Ok(receipt) => {
                    tracing::info!(
                        target: "script_kit::agent_handoff",
                        event = "detached_agent_prompt_export_succeeded",
                        action_id = %receipt.action_id,
                        dry_run = receipt.dry_run,
                        export_kind = %receipt.export_kind,
                        context_part_count = receipt.context_part_count,
                        prompt_builder_segment_count = receipt.prompt_builder_segment_count,
                        clipboard_written = receipt.clipboard_written,
                        prompt_chars = receipt.prompt_chars,
                        prompt_sha256 = %receipt.prompt_sha256,
                        path = ?receipt.path,
                        url = ?receipt.url,
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        target: "script_kit::agent_handoff",
                        event = "detached_agent_prompt_export_failed",
                        action_id,
                        error = %error.user_message(),
                    );
                }
            }
        }
        return;
    }

    if let Some(thread_id) = crate::actions::agent_chat_switch_thread_id_from_action(action_id) {
        if let Some(entity) = entity_weak.upgrade() {
            let thread_id = thread_id.to_string();
            entity.update(cx, |chat, cx| {
                chat.switch_to_thread(&thread_id, cx);
            });
            tracing::info!(event = "detached_action_switch_thread");
        }
        return;
    }

    if let Some(entry_id) = crate::actions::agent_chat_fork_edit_entry_from_action(action_id) {
        if let Some(entity) = entity_weak.upgrade() {
            let entry_id = entry_id.to_string();
            entity.update(cx, |chat, cx| {
                if let Some(thread) = chat.thread() {
                    thread.update(cx, |thread, cx| {
                        thread.fork_to_message(&entry_id, cx);
                    });
                }
            });
            tracing::info!(event = "detached_action_fork_edit");
        }
        return;
    }

    match action_id {
        super::view::AGENT_CHAT_RECOVERY_SIGN_IN_ACTION_ID
        | super::view::AGENT_CHAT_RECOVERY_SWITCH_ACCOUNT_ACTION_ID
        | super::view::AGENT_CHAT_RECOVERY_COPY_DETAILS_ACTION_ID => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| {
                    let _ = chat.dispatch_ai_recovery_action(action_id, cx);
                });
            }
        }
        "agent_chat_send" | "agent_chat_stop" => {
            if let Some(entity) = entity_weak.upgrade() {
                let command = if action_id == "agent_chat_send" {
                    crate::components::conversation_actions::AgentChatConversationCommand::Send
                } else {
                    crate::components::conversation_actions::AgentChatConversationCommand::Stop
                };
                let executed = entity.update(cx, |chat, cx| {
                    chat.execute_conversation_command(command, cx)
                });
                tracing::info!(event = "detached_action_conversation_command", executed);
            }
        }
        "agent_chat_new_thread" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| chat.start_new_thread(cx));
                tracing::info!(event = "detached_action_new_thread");
            }
        }
        "agent_chat_review_approvals" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| {
                    if let Some(thread) = chat.thread() {
                        thread.update(cx, |thread, cx| thread.review_standing_approvals(cx));
                    }
                });
                tracing::info!(event = "detached_action_review_approvals");
            }
        }
        "agent_chat_copy_last_response" => {
            if let Some(entity) = entity_weak.upgrade() {
                let maybe_last = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        thread
                            .read(cx)
                            .messages
                            .iter()
                            .rev()
                            .find(|m| {
                                matches!(
                                    m.role,
                                    super::thread::AgentChatThreadMessageRole::Assistant
                                )
                            })
                            .map(|m| m.body.to_string())
                    })
                };
                if let Some(last_assistant) = maybe_last {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(last_assistant));
                    tracing::info!(event = "detached_action_copy_last_response");
                }
            }
        }
        "agent_chat_paste_to_frontmost" => {
            if let Some(entity) = entity_weak.upgrade() {
                let maybe_last = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        thread
                            .read(cx)
                            .messages
                            .iter()
                            .rev()
                            .find(|m| {
                                matches!(
                                    m.role,
                                    super::thread::AgentChatThreadMessageRole::Assistant
                                ) && !m.body.trim().is_empty()
                            })
                            .map(|m| m.body.to_string())
                    })
                };
                if let Some(last_assistant) = maybe_last {
                    close_chat_window(cx);
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        let injector = crate::text_injector::TextInjector::new();
                        if let Err(error) = injector.paste_text(&last_assistant) {
                            tracing::warn!(
                                event = "detached_action_paste_last_response_failed",
                                %error
                            );
                        }
                    });
                    tracing::info!(event = "detached_action_paste_last_response");
                }
            }
        }
        "agent_chat_export_markdown" => {
            if let Some(entity) = entity_weak.upgrade() {
                let maybe_markdown = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        super::export::build_agent_chat_conversation_markdown_from_thread(
                            thread.read(cx),
                        )
                    })
                };
                if let Some(md) = maybe_markdown {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(md));
                    tracing::info!(event = "detached_action_export_markdown");
                }
            }
        }
        "agent_chat_save_as_note" => {
            if let Some(entity) = entity_weak.upgrade() {
                let (maybe_markdown, thread_source) = {
                    let view = entity.read(cx);
                    let maybe_markdown = view.thread().and_then(|thread| {
                        super::export::build_agent_chat_conversation_markdown_from_thread(
                            thread.read(cx),
                        )
                    });
                    let thread_source = view.thread().map(|thread| {
                        crate::notes::agent_chat_thread_source(thread.read(cx).ui_thread_id())
                    });
                    (maybe_markdown, thread_source)
                };
                if let Some(markdown) = maybe_markdown {
                    match crate::notes::save_note_with_content_and_source(
                        cx,
                        markdown,
                        thread_source,
                    ) {
                        Ok(()) => {
                            close_chat_window(cx);
                            tracing::info!(
                                event = "detached_action_save_as_note",
                                handoff = "notes_window"
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                event = "detached_action_save_as_note_failed",
                                error = %error
                            );
                        }
                    }
                } else {
                    tracing::warn!(event = "detached_action_save_as_note_blocked");
                }
            }
        }
        "agent_chat_retry_last" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| chat.retry_last_user_turn(cx));
                tracing::info!(
                    event = "detached_action_retry_last",
                    replay = "immutable_prepared_turn"
                );
            }
        }
        "agent_chat_new_conversation" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| {
                    let _ = chat.start_new_conversation(cx);
                });
                tracing::info!(event = "detached_action_new_conversation");
            }
        }
        "agent_chat_show_history" => {
            let opened = open_detached_history_actions(cx);
            tracing::info!(event = "detached_action_show_history_actions", opened);
        }
        "chat:toggle_favorite_model" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| {
                    if let Some(thread) = chat.thread() {
                        thread.update(cx, |thread, cx| {
                            if let Some(model_id) = thread.selected_model_id().map(str::to_string) {
                                thread.toggle_favorite_model(&model_id, cx);
                            }
                        });
                    }
                });
            }
        }
        "chat:cycle_favorite_model" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| {
                    if let Some(thread) = chat.thread() {
                        thread.update(cx, |thread, cx| thread.cycle_favorite_model(cx));
                    }
                });
            }
        }
        "chat:expand_composer" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| chat.toggle_expanded_composer(cx));
            }
        }
        "agent_chat_clear_history" => {
            let kit = crate::setup::get_kit_path();
            let _ = std::fs::remove_file(kit.join("agent_chat-history.jsonl"));
            let _ = std::fs::remove_dir_all(kit.join("agent_chat-conversations"));
            tracing::info!(event = "detached_action_clear_history");
        }
        "agent_chat_scroll_to_top" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| {
                    if let Some(transcript) = &chat.transcript {
                        transcript.read(cx).scroll_to(gpui::ListOffset {
                            item_ix: 0,
                            offset_in_item: px(0.),
                        });
                    }
                    cx.notify();
                });
            }
        }
        "agent_chat_scroll_to_bottom" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| {
                    if let Some(transcript) = &chat.transcript {
                        transcript.read(cx).scroll_to_end();
                    }
                    cx.notify();
                });
            }
        }
        "agent_chat_expand_all" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| {
                    let _ids: Vec<u64> = chat
                        .live_thread()
                        .read(cx)
                        .messages
                        .iter()
                        .filter(|m| {
                            matches!(
                                m.role,
                                super::thread::AgentChatThreadMessageRole::Thought
                                    | super::thread::AgentChatThreadMessageRole::Tool
                            )
                        })
                        .map(|m| m.id)
                        .collect();
                    // KNOWN: Collapse-all needs a set-all-collapsed API on AgentChatTranscript; expand_ids adds to collapsed set but there is no counterpart for bulk collapse.
                    cx.notify();
                });
            }
        }
        "agent_chat_collapse_all" => {
            if let Some(entity) = entity_weak.upgrade() {
                entity.update(cx, |chat, cx| {
                    if let Some(transcript) = &chat.transcript {
                        transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
                    }
                    cx.notify();
                });
            }
        }
        "agent_chat_reattach_panel" => {
            let reattached = reattach_detached_chat_into_main_panel(cx);
            tracing::info!(event = "detached_action_reattach_panel", reattached);
        }
        "agent_chat_close" => {
            let dismissed = request_detached_conversation_dismiss(
                crate::components::conversation_actions::ConversationDismissTrigger::CloseButton,
                cx,
            );
            tracing::info!(event = "detached_action_close", dismissed);
        }
        other => {
            tracing::warn!(
                event = "detached_action_unhandled",
                action = %other,
            );
        }
    }
}

/// Window title used internally for NSWindow matching (not displayed — titlebar is None).
const AGENT_CHAT_WINDOW_TITLE: &str = "Script Kit Agent Chat";

/// Configure vibrancy on the Agent Chat chat window to match the main window appearance.
///
/// Sets the NSWindow title (invisible since titlebar is None), then finds the
/// window by title and applies the same vibrancy material as Notes/AI windows.
#[cfg(target_os = "macos")]
fn configure_agent_chat_vibrancy(cx: &mut App) {
    use objc::{msg_send, sel, sel_impl};

    let handle = {
        let slot = CHAT_WINDOW.get_or_init(|| Mutex::new(None));
        slot.lock().ok().and_then(|g| g.as_ref().map(|s| s.handle))
    };

    let Some(handle) = handle else { return };

    // First, set the window title via the GPUI handle so we can find it by title.
    let _ = handle.update(cx, |_root, window, _cx| {
        // SAFETY: GPUI Window exposes the title setter.
        window.set_window_title(&gpui::SharedString::from(AGENT_CHAT_WINDOW_TITLE));
    });

    // Now find it by title, same pattern as Notes/AI windows.
    // SAFETY: We're on the main thread (GPUI guarantees this for App callbacks).
    // All NSWindow pointers are nil-checked. Title string pointer is nil-checked.
    unsafe {
        use cocoa::appkit::NSApp;
        use cocoa::base::nil;
        use std::ffi::CStr;

        let app = NSApp();
        let windows: cocoa::base::id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];

        for i in 0..count {
            let window: cocoa::base::id = msg_send![windows, objectAtIndex: i];
            let title: cocoa::base::id = msg_send![window, title];

            if title != nil {
                let title_cstr: *const i8 = msg_send![title, UTF8String];
                if !title_cstr.is_null() {
                    let title_str = CStr::from_ptr(title_cstr).to_string_lossy();

                    if title_str == AGENT_CHAT_WINDOW_TITLE {
                        let theme = crate::theme::get_cached_theme();
                        let is_dark = theme.should_use_dark_vibrancy();
                        crate::platform::configure_secondary_window_vibrancy(
                            window,
                            "Agent Chat",
                            is_dark,
                        );
                        tracing::info!("agent_chat_vibrancy_configured");
                        return;
                    }
                }
            }
        }

        tracing::warn!("agent_chat_window_not_found_for_vibrancy");
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_agent_chat_vibrancy(_cx: &mut App) {}

/// Minimal placeholder view for the detached chat window.
struct ChatWindowPlaceholder {
    focus_handle: gpui::FocusHandle,
}

impl ChatWindowPlaceholder {
    fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl gpui::Render for ChatWindowPlaceholder {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::{div, prelude::*, rgb};
        let theme = crate::theme::get_cached_theme();

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(
                cx.listener(|_this, event: &gpui::KeyDownEvent, _window, cx| {
                    let key = event.keystroke.key.as_str();
                    let close_requested = (event.keystroke.modifiers.platform
                        && key.eq_ignore_ascii_case("w"))
                        || crate::ui_foundation::is_key_escape(key);
                    if close_requested {
                        cx.stop_propagation();
                        cx.defer(|cx| {
                            close_chat_window(cx);
                        });
                    }
                }),
            )
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(div().text_base().opacity(0.7).child("Agent Chat Window"))
            .child(
                div()
                    .pt(px(8.0))
                    .text_sm()
                    .opacity(0.45)
                    .child("Detached chat \u{2014} full implementation coming soon"),
            )
            .child(
                div()
                    .pt(px(4.0))
                    .text_xs()
                    .opacity(0.35)
                    .text_color(rgb(theme.colors.accent.selected))
                    .child("\u{2318}W to close"),
            )
    }
}
