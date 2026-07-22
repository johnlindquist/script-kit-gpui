//! Notes-hosted Agent Chat surface helpers.
//!
//! Manages the embedded Agent Chat chat lifecycle inside the Notes window:
//! open/reuse, switch back to Notes editor, toggle actions dialog,
//! and dispatch Notes-specific Agent Chat actions.

use super::*;

pub(crate) const NOTES_EMBEDDED_AI_AUTOMATION_ID: &str = "notes:ai";

impl NotesApp {
    fn sync_notes_embedded_agent_chat_automation_window(&self, active: bool) -> bool {
        if !active {
            let _ = crate::windows::remove_automation_window(NOTES_EMBEDDED_AI_AUTOMATION_ID);
            return true;
        }

        if crate::windows::automation_window_by_id("notes").is_none() {
            tracing::warn!(
                target: "script_kit::automation",
                event = "notes_embedded_agent_chat_automation_parent_missing",
                parent_window_id = "notes",
                child_window_id = NOTES_EMBEDDED_AI_AUTOMATION_ID,
                "Notes embedded Agent Chat automation identity was not registered because the Notes parent is missing"
            );
            return false;
        }

        crate::windows::upsert_automation_window(crate::protocol::AutomationWindowInfo {
            id: NOTES_EMBEDDED_AI_AUTOMATION_ID.to_string(),
            kind: crate::protocol::AutomationWindowKind::AgentChatDetached,
            title: Some("Script Kit Notes AI".to_string()),
            focused: false,
            visible: true,
            semantic_surface: Some("notesAgentChat".to_string()),
            bounds: None,
            parent_window_id: Some("notes".to_string()),
            parent_kind: Some(crate::protocol::AutomationWindowKind::Notes),
            pid: Some(std::process::id()),
        });
        true
    }

    pub(super) fn ensure_embedded_agent_chat_view(
        &mut self,
        initial_input: Option<String>,
        cx: &mut Context<Self>,
    ) -> Result<gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>, String> {
        if let Some(entity) = self.embedded_agent_chat.as_ref().cloned() {
            tracing::info!(
                event = "notes_embedded_agent_chat_view_ensured",
                created = false,
                has_input = initial_input.as_ref().is_some_and(|s| !s.trim().is_empty()),
            );
            return Ok(entity);
        }

        let requirements = crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default();
        let view = crate::ai::agent_chat::ui::hosted::spawn_hosted_view(
            initial_input.clone(),
            requirements,
            cx,
        )?;
        self.wire_agent_chat_host_callbacks(&view, cx);
        self.embedded_agent_chat = Some(view.clone());
        self.notes_agent_chat_generation = self.notes_agent_chat_generation.wrapping_add(1);

        tracing::info!(
            event = "notes_embedded_agent_chat_view_ensured",
            created = true,
            has_input = initial_input.as_ref().is_some_and(|s| !s.trim().is_empty()),
        );

        Ok(view)
    }

    /// Switch the Notes window to show an embedded Agent Chat chat surface.
    ///
    /// If an Agent Chat view is already cached, reuses it (sets input if provided).
    /// Otherwise spawns a fresh hosted view via the host-neutral bootstrap.
    pub(crate) fn open_or_focus_embedded_agent_chat(
        &mut self,
        initial_input: Option<String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let reused = self.embedded_agent_chat.is_some();

        tracing::info!(
            event = "notes_agent_chat_surface_open_requested",
            reused,
            has_input = initial_input.as_ref().is_some_and(|s| !s.trim().is_empty()),
        );

        let _view = self.ensure_embedded_agent_chat_view(initial_input, cx)?;

        self.surface_mode = NotesSurfaceMode::AgentChat;
        self.pending_focus_surface = Some(focus::NotesFocusSurface::AgentChat);
        let automation_synced = self.sync_notes_embedded_agent_chat_automation_window(true);

        tracing::info!(
            event = "notes_agent_chat_surface_opened",
            reused,
            mode = "agent_chat",
            automation_synced,
            automation_id = NOTES_EMBEDDED_AI_AUTOMATION_ID,
        );

        cx.notify();
        Ok(())
    }

    /// Switch from embedded Agent Chat back to the Notes editor surface.
    ///
    /// Calls `prepare_for_host_hide()` on the cached Agent Chat view so popups
    /// and composer sessions are properly closed, then returns focus to
    /// the editor.
    pub(super) fn switch_to_notes_surface(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_embedded_agent_chat_via_host("switch_to_notes_surface", Some(window), cx);
    }

    fn close_embedded_agent_chat_via_host(
        &mut self,
        reason: &'static str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref entity) = self.embedded_agent_chat {
            entity.update(cx, |chat, cx| {
                chat.prepare_for_host_hide(cx);
            });
        }

        if crate::actions::is_actions_window_open() {
            crate::actions::close_actions_window(cx);
        }

        self.surface_mode = NotesSurfaceMode::Notes;
        let automation_synced = self.sync_notes_embedded_agent_chat_automation_window(false);
        if let Some(window) = window {
            self.request_focus_surface(focus::NotesFocusSurface::Editor, window, cx);
        } else {
            self.pending_focus_surface = Some(focus::NotesFocusSurface::Editor);
            cx.notify();
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_embedded_agent_chat_closed_via_host",
            reason,
            automation_synced,
        );
    }

    pub(super) fn prepare_embedded_agent_chat_for_window_close(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref entity) = self.embedded_agent_chat {
            entity.update(cx, |chat, cx| {
                chat.prepare_for_host_hide(cx);
            });
        }

        if crate::actions::is_actions_window_open() {
            crate::actions::close_actions_window(cx);
        }
        let automation_synced = self.sync_notes_embedded_agent_chat_automation_window(false);

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_embedded_agent_chat_prepared_for_window_close",
            reason,
            automation_synced,
        );
    }

    pub(super) fn clear_notes_hosted_agent_chat_context_for_note(
        &mut self,
        note_id: Option<NoteId>,
        cx: &mut Context<Self>,
    ) {
        let Some(note_id) = note_id else {
            return;
        };
        if let Some(ref entity) = self.embedded_agent_chat {
            entity.update(cx, |chat, cx| {
                chat.clear_hosted_context_parts_from_host("notes_note_switch_detach", cx);
            });
        }
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_hosted_agent_chat_context_cleared_for_note",
            note_id = %note_id,
        );
    }

    fn close_notes_agent_chat_actions_via_host(
        &mut self,
        reason: &'static str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        // When this runs from the dialog's on_close callback, the actions window
        // is already in its close path. Only actively close it from call sites
        // that still own a live parent window handle.
        if window.is_some() && crate::actions::is_actions_window_open() {
            crate::actions::close_actions_window(cx);
        }

        if let Some(window) = window {
            self.request_focus_surface(focus::NotesFocusSurface::AgentChat, window, cx);
        } else if self.surface_mode == NotesSurfaceMode::AgentChat {
            self.pending_focus_surface = Some(focus::NotesFocusSurface::AgentChat);
            cx.notify();
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_agent_chat_actions_closed_via_host",
            reason,
        );
    }

    /// Handle a portal open request from the embedded Agent Chat context selector.
    ///
    /// Static helper so it can be called from the `set_on_open_portal` closure
    /// without holding an immutable borrow on `NotesApp` while also needing
    /// `&mut App`.
    ///
    /// `AgentChatHistory` is supported in Notes because Agent Chat already stages the query.
    /// `FileSearch` and `ClipboardHistory` are filtered from the picker and
    /// guarded at the accept path by `allowed_portal_kinds`, so they should
    /// not reach here in normal operation. The branches remain as
    /// defense-in-depth logging.
    fn handle_agent_chat_portal_static(
        chat: Option<Entity<crate::ai::agent_chat::ui::AgentChatView>>,
        kind: crate::ai::context_selector::types::ContextPortalKind,
        cx: &mut gpui::App,
    ) {
        use crate::ai::context_selector::types::ContextPortalKind;

        let Some(chat) = chat else {
            tracing::info!(
                event = "notes_agent_chat_portal_requested",
                kind = ?kind,
                opened = false,
                reason = "no_embedded_agent_chat_view",
            );
            return;
        };

        match kind {
            ContextPortalKind::AgentChatHistory => {
                let query = chat.update(cx, |view, _cx| {
                    view.take_pending_history_portal_query().unwrap_or_default()
                });
                tracing::info!(
                    target: "script_kit::agent_chat",
                    event = "notes_agent_chat_history_portal_query_seeded_from_contract",
                    query = %query,
                );

                let hits = crate::ai::agent_chat::ui::history::search_history(&query, 12);

                tracing::info!(
                    event = "notes_agent_chat_portal_requested",
                    kind = "AgentChatHistory",
                    opened = true,
                    query = %query,
                    hit_count = hits.len(),
                );

                let opened = chat.update(cx, |view, cx| {
                    view.open_history_portal_with_entries(query, hits, cx)
                });
                if !opened {
                    chat.update(cx, |view, cx| {
                        let _ = view
                            .cancel_pending_portal_session(ContextPortalKind::AgentChatHistory, cx);
                    });
                }
            }
            ContextPortalKind::FileSearch
            | ContextPortalKind::BrowserHistory
            | ContextPortalKind::BrowserTabs
            | ContextPortalKind::DictationHistory
            | ContextPortalKind::ScriptSearch
            | ContextPortalKind::ScriptletSearch
            | ContextPortalKind::SkillSearch
            | ContextPortalKind::NotesBrowse
            | ContextPortalKind::Terminal => {
                tracing::info!(
                    event = "notes_agent_chat_portal_requested",
                    kind = ?kind,
                    opened = false,
                    reason = "unsupported_in_notes_host",
                );
                chat.update(cx, |view, cx| {
                    let _ = view.cancel_pending_portal_session(kind, cx);
                });
            }
            ContextPortalKind::ClipboardHistory => {
                tracing::info!(
                    event = "notes_agent_chat_portal_requested",
                    kind = "ClipboardHistory",
                    opened = false,
                    reason = "unsupported_in_notes_host",
                );
                chat.update(cx, |view, cx| {
                    let _ =
                        view.cancel_pending_portal_session(ContextPortalKind::ClipboardHistory, cx);
                });
            }
        }
    }

    /// Wire Agent Chat host callbacks (toggle-actions, close, history, portals)
    /// to Notes-owned handlers.
    fn wire_agent_chat_host_callbacks(
        &self,
        view: &Entity<crate::ai::agent_chat::ui::AgentChatView>,
        cx: &mut Context<Self>,
    ) {
        let notes_entity = cx.entity().downgrade();

        // Restrict portal kinds to those Notes can actually host.
        // Notes owns @history locally; @file and @clipboard require
        // main-panel view switching that Notes cannot provide.
        view.update(cx, |chat, _cx| {
            chat.set_footer_host(crate::ai::agent_chat::ui::view::AgentChatFooterHost::External);
            chat.set_allowed_portal_kinds(vec![
                crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory,
            ]);
        });

        // Toggle actions: open the Notes-hosted Agent Chat actions dialog.
        let toggle_entity = notes_entity.clone();
        view.update(cx, |chat, _cx| {
            chat.set_on_toggle_actions(move |window, cx| {
                if let Some(entity) = toggle_entity.upgrade() {
                    entity.update(cx, |app, cx| {
                        app.toggle_agent_chat_actions(window, cx);
                    });
                }
            });
        });

        // Close: return to Notes editor rather than closing the window.
        let close_entity = notes_entity.clone();
        view.update(cx, |chat, _cx| {
            chat.set_on_close_requested(move |window, cx| {
                if let Some(entity) = close_entity.upgrade() {
                    entity.update(cx, |app, cx| {
                        app.close_embedded_agent_chat_via_host(
                            "agent_chat_close_requested",
                            Some(window),
                            cx,
                        );
                    });
                }
            });
        });

        // History command (Cmd+P): open the Notes-anchored ActionsDialog history route.
        let history_entity = notes_entity.clone();
        view.update(cx, |chat, _cx| {
            chat.set_on_open_history_command(move |window, cx| {
                if let Some(entity) = history_entity.upgrade() {
                    entity.update(cx, |app, cx| {
                        let _ = app.open_agent_chat_history_actions(window, cx);
                    });
                }
            });
        });

        // Portal requests: only @history reaches here because the view's
        // allowed_portal_kinds filters @file and @clipboard from the picker.
        // The handler still logs rejected kinds as defense-in-depth.
        let portal_view = view.downgrade();
        view.update(cx, |chat, _cx| {
            chat.set_on_open_portal(move |kind, cx| {
                if let Some(chat) = portal_view.upgrade() {
                    Self::handle_agent_chat_portal_static(Some(chat), kind, cx);
                }
            });
        });
    }

    /// Toggle the Agent Chat actions dialog for the Notes-hosted Agent Chat surface.
    ///
    /// Opens a filtered actions popup positioned relative to the Notes window.
    /// On close, re-focuses the Agent Chat chat inside Notes.
    pub(super) fn toggle_agent_chat_actions(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if crate::actions::is_actions_window_open() {
            self.close_notes_agent_chat_actions_via_host(
                "toggle_existing_window",
                Some(window),
                cx,
            );
            return;
        }

        self.defer_open_agent_chat_actions(window, cx);
    }

    fn defer_open_agent_chat_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let expected_generation = self.notes_agent_chat_generation;
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_agent_chat_actions_open_deferred",
            expected_generation,
        );

        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1))
                .await;

            let _ = cx.update(|window, app_cx| {
                let Some(entity) = this.upgrade() else {
                    return;
                };
                entity.update(app_cx, |app: &mut NotesApp, cx| {
                    if app.surface_mode != NotesSurfaceMode::AgentChat {
                        tracing::warn!(
                            target: "script_kit::tab_ai",
                            event = "notes_agent_chat_actions_open_skipped",
                            reason = "not_agent_chat_surface",
                            expected_generation,
                        );
                        return;
                    }
                    if app.notes_agent_chat_generation != expected_generation {
                        tracing::warn!(
                            target: "script_kit::tab_ai",
                            event = "notes_agent_chat_actions_open_skipped",
                            reason = "generation_mismatch",
                            expected_generation,
                            actual_generation = app.notes_agent_chat_generation,
                        );
                        return;
                    }
                    app.open_agent_chat_actions_now(window, cx);
                });
            });
        })
        .detach();
    }

    fn open_agent_chat_actions_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::actions::{self, ActionsDialog, WindowPosition};

        let Some(ref agent_chat_view) = self.embedded_agent_chat else {
            return;
        };
        let actions_target = agent_chat_view.downgrade();
        let actions_generation = self.notes_agent_chat_generation;

        if let Some(thread) = agent_chat_view.read(cx).thread() {
            thread.update(cx, |thread, cx| thread.refresh_models(cx));
        }

        // Read Agent Chat model context from the cached view.
        let (
            selected_model_id,
            available_models,
            standing_approval_count,
            thread_summaries,
            fork_points,
        ) = {
            let view = agent_chat_view.read(cx);
            let thread_summaries = view.retained_thread_summaries(cx);
            match &view.session {
                crate::ai::agent_chat::ui::AgentChatSession::Setup(_) => {
                    (None, Vec::new(), 0, thread_summaries, Vec::new())
                }
                crate::ai::agent_chat::ui::AgentChatSession::Live(thread) => {
                    let thread = thread.read(cx);
                    (
                        thread.selected_model_id().map(str::to_string),
                        thread.available_models().to_vec(),
                        thread.standing_approvals().len(),
                        thread_summaries,
                        thread.fork_points().to_vec(),
                    )
                }
            }
        };

        let theme_arc = std::sync::Arc::new(crate::theme::get_cached_theme());
        let (action_tx, action_rx) = async_channel::bounded::<String>(1);

        let callback: std::sync::Arc<dyn Fn(String) + Send + Sync> =
            std::sync::Arc::new(move |action_id: String| {
                tracing::info!(
                    event = "notes_agent_chat_action_selected_from_popup",
                    action = %action_id,
                );
                let _ = action_tx.try_send(action_id);
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
                },
                theme_arc,
                crate::actions::AgentChatActionsDialogHost::Notes,
            );
            dialog.set_skip_track_focus(true);
            dialog
        });

        let activation_dialog = dialog.clone();
        let notes_entity = cx.entity().downgrade();
        dialog.update(cx, |dialog, _cx| {
            let close_entity = notes_entity.clone();
            dialog.set_on_activation(std::sync::Arc::new(move |activation, _window, cx| {
                match activation {
                    crate::actions::ActionsDialogActivation::DrillDownPushed { .. } => {
                        crate::actions::resize_actions_window(cx, &activation_dialog);
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
                    crate::actions::ActionsDialogActivation::NoSelection => {}
                }
            }));
            dialog.set_on_close(std::sync::Arc::new(move |cx| {
                if let Some(entity) = close_entity.upgrade() {
                    entity.update(cx, |app, cx| {
                        app.close_notes_agent_chat_actions_via_host("dialog_on_close", None, cx);
                    });
                }
            }));
        });

        let parent_window_handle = window.window_handle();
        let bounds = window.bounds();
        let display_id = window.display(cx).map(|d| d.id());
        let parent_automation_id = Some("notes".to_string());

        match actions::open_actions_window(
            cx,
            parent_window_handle,
            bounds,
            display_id,
            dialog,
            WindowPosition::TopRight,
            parent_automation_id.as_deref(),
        ) {
            Ok(handle) => {
                let _ = handle.update(cx, |_root, window, _cx| {
                    window.activate_window();
                });
            }
            Err(e) => {
                tracing::warn!(event = "notes_agent_chat_actions_open_failed", error = %e);
                return;
            }
        }

        tracing::info!(event = "notes_agent_chat_actions_opened");

        // Spawn a one-shot task to dispatch the selected action.
        // `spawn_in(window, ...)` gives `AsyncWindowContext` so `.update()`
        // runs inside the Notes window and yields (&mut Window, &mut App).
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(action_id) = action_rx.recv().await {
                let _ = cx.update(|window, app_cx| {
                    if action_id == "__cancel__" {
                        // ActionsWindow request_close already ran the dialog on_close
                        // callback, so the host focus restore has happened.
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "notes_agent_chat_action_cancel_consumed_after_on_close",
                        );
                        return;
                    }
                    dispatch_notes_agent_chat_action(
                        this.upgrade(),
                        actions_target.clone(),
                        actions_generation,
                        &action_id,
                        window,
                        app_cx,
                    );
                });
            }
        })
        .detach();
    }

    pub(super) fn open_agent_chat_history_actions(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        use crate::actions::{self, ActionsDialog, WindowPosition};

        if actions::is_actions_window_open() {
            self.close_notes_agent_chat_actions_via_host("history_route_reopen", Some(window), cx);
        }

        let Some(ref agent_chat_view) = self.embedded_agent_chat else {
            tracing::info!(
                event = "notes_agent_chat_history_actions_requested",
                opened = false
            );
            return false;
        };

        let actions_target = agent_chat_view.downgrade();
        let actions_generation = self.notes_agent_chat_generation;
        let theme_arc = std::sync::Arc::new(crate::theme::get_cached_theme());
        let route = crate::actions::get_agent_chat_history_route();
        let (action_tx, action_rx) = async_channel::bounded::<String>(1);

        let callback: std::sync::Arc<dyn Fn(String) + Send + Sync> =
            std::sync::Arc::new(move |action_id: String| {
                tracing::info!(
                    event = "notes_agent_chat_history_action_selected_from_popup",
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
        let notes_entity = cx.entity().downgrade();
        dialog.update(cx, |dialog, _cx| {
            let close_entity = notes_entity.clone();
            dialog.set_on_activation(std::sync::Arc::new(move |activation, _window, cx| {
                if let crate::actions::ActionsDialogActivation::Executed { should_close, .. } =
                    activation
                {
                    if should_close {
                        let on_close = activation_dialog.read(cx).on_close.clone();
                        if let Some(on_close) = on_close {
                            on_close(cx);
                        }
                        crate::actions::close_actions_window(cx);
                    }
                }
            }));
            dialog.set_on_close(std::sync::Arc::new(move |cx| {
                if let Some(entity) = close_entity.upgrade() {
                    entity.update(cx, |app, cx| {
                        app.close_notes_agent_chat_actions_via_host(
                            "history_dialog_on_close",
                            None,
                            cx,
                        );
                    });
                }
            }));
        });

        let display_id = window.display(cx).map(|d| d.id());

        match actions::open_actions_window(
            cx,
            window.window_handle(),
            window.bounds(),
            display_id,
            dialog,
            WindowPosition::TopRight,
            Some("notes"),
        ) {
            Ok(handle) => {
                let _ = handle.update(cx, |_root, window, _cx| {
                    window.activate_window();
                });
            }
            Err(e) => {
                tracing::warn!(event = "notes_agent_chat_history_actions_open_failed", error = %e);
                return false;
            }
        }

        cx.spawn_in(window, async move |this, cx| {
            if let Ok(action_id) = action_rx.recv().await {
                let _ = cx.update(|window, app_cx| {
                    dispatch_notes_agent_chat_action(
                        this.upgrade(),
                        actions_target.clone(),
                        actions_generation,
                        &action_id,
                        window,
                        app_cx,
                    );
                });
            }
        })
        .detach();

        tracing::info!(
            event = "notes_agent_chat_history_actions_requested",
            opened = true
        );
        true
    }

    /// Relaunch the cached Notes-hosted Agent Chat surface with fresh session state.
    ///
    /// Use this for explicit note → Agent Chat switches so the user does not land
    /// inside an unrelated prior conversation.
    pub(super) fn relaunch_embedded_agent_chat(
        &mut self,
        initial_input: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let had_cached_view = self.embedded_agent_chat.is_some();

        if let Some(ref entity) = self.embedded_agent_chat {
            entity.update(cx, |chat, cx| {
                chat.prepare_for_host_hide(cx);
            });
        }
        self.embedded_agent_chat = None;

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_agent_chat_surface_relaunch_requested",
            had_cached_view,
            has_input = initial_input.as_ref().is_some_and(|s| !s.trim().is_empty()),
        );

        let result = self.open_or_focus_embedded_agent_chat(initial_input, window, cx);
        match &result {
            Ok(()) => tracing::info!(
                target: "script_kit::tab_ai",
                event = "notes_agent_chat_surface_relaunch_completed",
            ),
            Err(error) => tracing::warn!(
                target: "script_kit::tab_ai",
                event = "notes_agent_chat_surface_relaunch_failed",
                error = %error,
            ),
        }
        result
    }

    /// Accessor for the current surface mode.
    pub(crate) fn surface_mode(&self) -> NotesSurfaceMode {
        self.surface_mode
    }

    pub(crate) fn embedded_agent_chat_entity(
        &self,
    ) -> Option<Entity<crate::ai::agent_chat::ui::AgentChatView>> {
        self.embedded_agent_chat.clone()
    }
}

/// Dispatch an Agent Chat action from the Notes-hosted actions dialog popup.
///
/// Called from the async spawn inside `toggle_agent_chat_actions`.  Receives
/// `&mut Window` and `&mut App` from `AsyncWindowContext::update`, so it
/// can interact with the Agent Chat view entity and the Notes host state.
fn dispatch_notes_agent_chat_action(
    entity: Option<Entity<NotesApp>>,
    agent_chat_target: gpui::WeakEntity<crate::ai::agent_chat::ui::AgentChatView>,
    agent_chat_generation: u64,
    action_id: &str,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let Some(entity) = entity else { return };
    let Some(agent_chat_entity) = agent_chat_target.upgrade() else {
        tracing::warn!(
            target: "script_kit::tab_ai",
            event = "notes_agent_chat_action_stale_view",
            action_id = %action_id,
            reason = "target_dropped",
        );
        return;
    };
    if entity.read(cx).notes_agent_chat_generation != agent_chat_generation {
        tracing::warn!(
            target: "script_kit::tab_ai",
            event = "notes_agent_chat_action_stale_view",
            action_id = %action_id,
            reason = "generation_mismatch",
            expected_generation = agent_chat_generation,
            actual_generation = entity.read(cx).notes_agent_chat_generation,
        );
        return;
    }

    tracing::info!(
        event = "notes_agent_chat_action_dispatched",
        action_id = %action_id,
    );

    if let Some(session_id) =
        action_id.strip_prefix(crate::actions::AGENT_CHAT_HISTORY_SELECT_ACTION_PREFIX)
    {
        let selected = agent_chat_entity.update(cx, |chat, cx| {
            chat.select_history_session_by_id(session_id, cx)
        });
        tracing::info!(
            event = "notes_agent_chat_action_history_selected",
            session_id = %session_id,
            selected,
        );
        return;
    }

    // For `agent_chat_show_history`, open the Notes-anchored history actions route.
    if action_id == "agent_chat_show_history" {
        let opened = entity.update(cx, |app: &mut NotesApp, cx| {
            app.open_agent_chat_history_actions(window, cx)
        });
        tracing::info!(
            event = "notes_agent_chat_action_show_history_actions",
            opened
        );
        return;
    }

    // For `agent_chat_close`, route to the Notes host to switch surfaces.
    if action_id == "agent_chat_close" {
        entity.update(cx, |app: &mut NotesApp, cx| {
            app.close_embedded_agent_chat_via_host("agent_chat_action_close", Some(window), cx);
        });
        return;
    }

    // Handle model switch.
    if let Some(model_id) = crate::actions::agent_chat_switch_model_id_from_action(action_id) {
        agent_chat_entity.update(cx, |chat, cx| {
            if let Some(thread) = chat.thread() {
                thread.update(cx, |thread, cx| {
                    thread.select_model(model_id, cx);
                });
            }
        });
        return;
    }

    // Handle thread switch.
    if let Some(thread_id) = crate::actions::agent_chat_switch_thread_id_from_action(action_id) {
        let thread_id = thread_id.to_string();
        agent_chat_entity.update(cx, |chat, cx| {
            chat.switch_to_thread(&thread_id, cx);
        });
        return;
    }

    // Handle rewind-and-edit.
    if let Some(entry_id) = crate::actions::agent_chat_fork_edit_entry_from_action(action_id) {
        let entry_id = entry_id.to_string();
        agent_chat_entity.update(cx, |chat, cx| {
            if let Some(thread) = chat.thread() {
                thread.update(cx, |thread, cx| {
                    thread.fork_to_message(&entry_id, cx);
                });
            }
        });
        return;
    }

    match action_id {
        "agent_chat_new_thread" => {
            agent_chat_entity.update(cx, |chat, cx| chat.start_new_thread(cx));
        }
        "agent_chat_review_approvals" => {
            agent_chat_entity.update(cx, |chat, cx| {
                if let Some(thread) = chat.thread() {
                    thread.update(cx, |thread, cx| thread.review_standing_approvals(cx));
                }
            });
        }
        "agent_chat_copy_last_response" => {
            let maybe_last = {
                let view = agent_chat_entity.read(cx);
                view.thread().and_then(|thread| {
                    thread
                        .read(cx)
                        .messages
                        .iter()
                        .rev()
                        .find(|m| {
                            matches!(
                                m.role,
                                crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Assistant
                            )
                        })
                        .map(|m| m.body.to_string())
                })
            };
            if let Some(last_assistant) = maybe_last {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(last_assistant));
            }
        }
        "agent_chat_export_markdown" => {
            let maybe_markdown = {
                let view = agent_chat_entity.read(cx);
                view.thread().and_then(|thread| {
                    crate::ai::agent_chat::ui::export::build_agent_chat_conversation_markdown_from_thread(
                        thread.read(cx),
                    )
                })
            };
            if let Some(md) = maybe_markdown {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(md));
            }
        }
        "agent_chat_save_as_note" => {
            let (maybe_markdown, thread_source) = {
                let view = agent_chat_entity.read(cx);
                let maybe_markdown = view.thread().and_then(|thread| {
                    crate::ai::agent_chat::ui::export::build_agent_chat_conversation_markdown_from_thread(
                        thread.read(cx),
                    )
                });
                let thread_source = view.thread().map(|thread| {
                    crate::notes::agent_chat_thread_source(thread.read(cx).ui_thread_id())
                });
                (maybe_markdown, thread_source)
            };
            if let Some(markdown) = maybe_markdown {
                let markdown_len = markdown.len();
                let source_for_log = thread_source.clone();
                match crate::notes::save_note_with_content_and_source(cx, markdown, thread_source) {
                    Ok(()) => {
                        entity.update(cx, |app: &mut NotesApp, cx| {
                            app.close_embedded_agent_chat_via_host(
                                "agent_chat_save_as_note",
                                Some(window),
                                cx,
                            );
                        });
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "notes_agent_chat_save_as_note",
                            saved = true,
                            markdown_len,
                            source = ?source_for_log,
                            closed_host = true,
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "script_kit::tab_ai",
                            event = "notes_agent_chat_save_as_note_failed",
                            error = %error,
                        );
                    }
                }
            } else {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "notes_agent_chat_save_as_note_blocked",
                    reason = "empty_transcript",
                );
            }
        }
        "agent_chat_retry_last" => {
            let last_user_msg = {
                let view = agent_chat_entity.read(cx);
                view.thread().and_then(|thread| {
                    thread
                        .read(cx)
                        .messages
                        .iter()
                        .rev()
                        .find(|m| {
                            matches!(
                                m.role,
                                crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::User
                            )
                        })
                        .map(|m| m.body.to_string())
                })
            };
            if let Some(text) = last_user_msg {
                agent_chat_entity.update(cx, |chat, cx| {
                    chat.live_thread().update(cx, |thread, cx| {
                        thread.set_input(text, cx);
                        let _ = thread.submit_input(cx);
                    });
                });
            }
        }
        "agent_chat_new_conversation" => {
            agent_chat_entity.update(cx, |chat, cx| {
                chat.live_thread().update(cx, |thread, cx| {
                    thread.clear_messages(cx);
                });
                if let Some(transcript) = &chat.transcript {
                    transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
                }
                cx.notify();
            });
        }
        "agent_chat_clear_history" => {
            let kit = crate::setup::get_kit_path();
            let _ = std::fs::remove_file(kit.join("agent_chat-history.jsonl"));
            let _ = std::fs::remove_dir_all(kit.join("agent_chat-conversations"));
        }
        "agent_chat_scroll_to_top" => {
            agent_chat_entity.update(cx, |chat, cx| {
                if let Some(transcript) = &chat.transcript {
                    transcript.read(cx).scroll_to(gpui::ListOffset {
                        item_ix: 0,
                        offset_in_item: px(0.),
                    });
                }
                cx.notify();
            });
        }
        "agent_chat_scroll_to_bottom" => {
            agent_chat_entity.update(cx, |chat, cx| {
                if let Some(transcript) = &chat.transcript {
                    transcript.read(cx).scroll_to_end();
                }
                cx.notify();
            });
        }
        "agent_chat_expand_all" => {
            agent_chat_entity.update(cx, |chat, cx| {
                if let Some(transcript) = &chat.transcript {
                    transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
                }
                cx.notify();
            });
        }
        "agent_chat_collapse_all" => {
            agent_chat_entity.update(cx, |chat, cx| {
                if let Some(transcript) = &chat.transcript {
                    transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
                }
                cx.notify();
            });
        }
        other => {
            tracing::warn!(
                event = "notes_agent_chat_action_unhandled",
                action = %other,
            );
        }
    }

    // Suppress unused-window warning — window is available but most Agent Chat
    // actions dispatch through entity.update() which uses App context.
    let _ = window;
}

/// Close the Notes-hosted embedded Agent Chat (from outside the NotesApp entity).
///
/// Switches the Notes window back to the editor surface.  No-op if
/// the Notes window is not open or is not in Agent Chat mode.
pub fn close_notes_embedded_agent_chat(cx: &mut gpui::App) -> anyhow::Result<()> {
    let (entity, _handle) = match super::get_notes_app_entity_and_handle() {
        Some(pair) => pair,
        None => return Ok(()),
    };

    entity.update(cx, |app: &mut NotesApp, cx| {
        if app.surface_mode == NotesSurfaceMode::AgentChat {
            app.close_embedded_agent_chat_via_host("external_close", None, cx);
        }
    });

    Ok(())
}
