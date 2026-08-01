use itertools::Itertools;

use super::*;

/// Which Notes command-bar popup an activation callback routes for.
#[derive(Clone, Copy, Debug)]
enum NotesCommandBarRole {
    Actions,
    NoteSwitcher,
}

impl NotesApp {
    pub(crate) fn open_actions_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Update command bar actions based on current state (dynamic - depends on selection, etc.)
        let actions = get_notes_command_bar_actions(&NotesInfo {
            has_selection: self.selected_note_id.is_some(),
            is_trash_view: self.view_mode == NotesViewMode::Trash,
            auto_sizing_enabled: self.auto_sizing_enabled,
        });

        // Log what actions we're setting
        info!(
            "Notes open_actions_panel: setting {} actions: [{}]",
            actions.len(),
            actions.iter().take(5).map(|a| a.title.as_str()).join(", ")
        );

        self.command_bar.set_actions(actions, cx);

        // Open the command bar (CommandBar handles window creation internally).
        // CommandBar::is_open() is the single source of truth for popup state —
        // there is intentionally no separate NotesApp flag to keep in sync.
        self.command_bar.open_centered(window, cx);
        self.wire_command_bar_activation(NotesCommandBarRole::Actions, window, cx);

        // Route through NotesFocusSurface for structured logging and consistent focus management.
        // The ActionsWindow is a visual-only popup — it does NOT take keyboard focus.
        self.request_focus_surface(focus::NotesFocusSurface::ActionsPanel, window, cx);
    }

    /// Route ActionsDialog activations back into NotesApp.
    ///
    /// The CommandBar wrapper creates its dialog with a no-op `on_select`
    /// (keyboard Enter through the Notes router uses
    /// `execute_selected_action` instead). Row clicks — and Enter/shortcut
    /// activations handled by the detached ActionsWindow when AppKit makes it
    /// the key window — only surface through the dialog's activation
    /// callback, so without this hook they execute nothing.
    fn wire_command_bar_activation(
        &mut self,
        role: NotesCommandBarRole,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let bar = match role {
            NotesCommandBarRole::Actions => &self.command_bar,
            NotesCommandBarRole::NoteSwitcher => &self.note_switcher,
        };
        let Some(dialog) = bar.dialog().cloned() else {
            return;
        };
        let resize_dialog = dialog.clone();
        let notes_app = cx.entity().downgrade();
        let notes_window = window.window_handle();
        let on_close_notes_app = notes_app.clone();
        dialog.update(cx, |dialog, _cx| {
            // Escape / Cmd+K / focus loss while the DETACHED popup is the key
            // window run ActionsWindow::request_close, bypassing
            // close_actions_panel / close_browse_panel entirely. Without this
            // hook the editor never regains focus and the host's `is_open`
            // flag only reconciles on the next keystroke.
            dialog.set_on_close(std::sync::Arc::new(move |cx| {
                let notes_app = on_close_notes_app.clone();
                // Defer out of the popup window's close path before touching
                // the Notes window, mirroring the activation routing below.
                cx.defer(move |cx| {
                    let restored = notes_window.update(cx, |_root, window, cx| {
                        let Some(notes_app) = notes_app.upgrade() else {
                            return;
                        };
                        notes_app.update(cx, |app, cx| {
                            app.handle_detached_popup_closed_externally(role, window, cx);
                        });
                    });
                    if let Err(error) = restored {
                        tracing::warn!(
                            target: "script_kit::actions",
                            ?role,
                            error = %error,
                            "notes_command_bar_on_close_restore_failed"
                        );
                    }
                });
            }));
            dialog.set_on_activation(std::sync::Arc::new(move |activation, _window, cx| {
                match activation {
                    crate::actions::ActionsDialogActivation::Executed { action_id, .. } => {
                        let notes_app = notes_app.clone();
                        // Defer out of the actions window's update stack: the
                        // execute paths close the popup, and removing a window
                        // from inside its own event dispatch fails and leaves
                        // a zombie key window.
                        cx.defer(move |cx| {
                            let routed = notes_window.update(cx, |_root, window, cx| {
                                let Some(notes_app) = notes_app.upgrade() else {
                                    return;
                                };
                                notes_app.update(cx, |app, cx| match role {
                                    NotesCommandBarRole::Actions => {
                                        app.execute_action(&action_id, window, cx);
                                    }
                                    NotesCommandBarRole::NoteSwitcher => {
                                        app.execute_note_switcher_action(&action_id, window, cx);
                                    }
                                });
                            });
                            if let Err(error) = routed {
                                tracing::warn!(
                                    target: "script_kit::actions",
                                    ?role,
                                    error = %error,
                                    "notes_command_bar_activation_route_failed"
                                );
                            }
                        });
                    }
                    crate::actions::ActionsDialogActivation::DrillDownPushed { .. } => {
                        crate::actions::resize_actions_window(cx, &resize_dialog);
                    }
                    crate::actions::ActionsDialogActivation::Blocked { .. }
                    | crate::actions::ActionsDialogActivation::NoSelection => {}
                }
            }));
        });
    }

    pub(super) fn close_actions_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Close the command bar window
        self.command_bar.close(cx);

        // Route through NotesFocusSurface for structured logging and consistent focus management.
        self.request_focus_surface(focus::NotesFocusSurface::Editor, window, cx);
    }

    /// Restore host state and focus after a detached popup closed itself
    /// (Escape/Cmd+K while the popup was the key window, or focus loss).
    ///
    /// Mirrors `close_actions_panel` / `close_browse_panel` without
    /// re-entering the popup's window-close path, which is already running
    /// when the dialog's `on_close` callback fires.
    fn handle_detached_popup_closed_externally(
        &mut self,
        role: NotesCommandBarRole,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_open = match role {
            NotesCommandBarRole::Actions => self.command_bar.mark_closed_externally(),
            NotesCommandBarRole::NoteSwitcher => {
                self.mention_portal_edit = None;
                self.note_switcher.mark_closed_externally()
            }
        };
        tracing::info!(
            target: "script_kit::actions",
            ?role,
            was_open,
            "notes_detached_popup_closed_externally"
        );
        self.restore_primary_focus_after_dialog(window, cx);
    }

    /// Handle action from the actions panel (Cmd+K)
    pub(super) fn handle_action(
        &mut self,
        action: NotesAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        debug!(?action, "Handling notes action");
        match action {
            NotesAction::NewNote => self.create_note(window, cx),
            NotesAction::DuplicateNote => self.duplicate_selected_note(window, cx),
            NotesAction::BrowseNotes => {
                // Close actions panel first, then open browse panel.
                // Don't call close_actions_panel here - it refocuses the editor;
                // open_browse_panel owns focus for this transition.
                self.command_bar.close(cx);
                self.open_browse_panel(window, cx);
                cx.notify();
                return; // Early return - browse panel handles its own focus
            }
            NotesAction::TogglePreview => self.toggle_preview(window, cx),
            NotesAction::CycleSortMode => self.cycle_sort_mode(cx),
            NotesAction::OpenTrash => self.set_view_mode(NotesViewMode::Trash, window, cx),
            NotesAction::EmptyTrash => self.empty_trash(cx),
            NotesAction::BackToNotes => self.set_view_mode(NotesViewMode::AllNotes, window, cx),
            NotesAction::HistoryBack => self.navigate_back(window, cx),
            NotesAction::HistoryForward => self.navigate_forward(window, cx),
            NotesAction::FindInNote => {
                // Close WITHOUT close_actions_panel: its deferred Editor
                // focus-surface apply lands after the Search action opens the
                // find bar, stealing focus away from the find input (find bar
                // visible but typing goes to the note body).
                self.command_bar.close(cx);
                self.editor_state.update(cx, |state, cx| {
                    state.focus(window, cx);
                });
                window.dispatch_action(Box::new(Search), cx);
                cx.notify();
                return; // Early return - already handled focus
            }
            NotesAction::CopyNoteAs => self.copy_note_as_markdown(cx),
            NotesAction::CopyDeeplink => self.copy_note_deeplink(cx),
            NotesAction::CreateQuicklink => self.create_note_quicklink(cx),
            NotesAction::CopyBacklinks => self.copy_note_backlinks(cx),
            NotesAction::Export => self.export_note(ExportFormat::Html, cx),
            NotesAction::DeleteNote => {
                self.close_actions_panel(window, cx);
                self.request_delete_selected_note(window, cx);
                return;
            }
            NotesAction::RestoreNote => self.restore_note(window, cx),
            NotesAction::PermanentlyDeleteNote => self.permanently_delete_note(window, cx),
            NotesAction::MoveListItemUp => {
                self.close_actions_panel(window, cx);
                self.move_line_up(window, cx);
                return;
            }
            NotesAction::MoveListItemDown => {
                self.close_actions_panel(window, cx);
                self.move_line_down(window, cx);
                return;
            }
            NotesAction::Format => {
                self.show_format_toolbar = !self.show_format_toolbar;
            }
            NotesAction::EnableAutoSizing => {
                self.toggle_auto_sizing(window, cx);
            }
            NotesAction::ResetWindowPosition => {
                self.reset_window_position_to_default(window, cx);
            }
            NotesAction::SendToAi => {
                // Close the panel first, then run the one-shot handoff — it
                // owns its own success/blocked feedback.
                self.close_actions_panel(window, cx);
                let _ = self.handoff_selected_note_to_main_agent_chat("NotesAction::SendToAi", cx);
                return;
            }
            NotesAction::Cancel => {
                // Panel was cancelled, nothing to do
            }
        }
        // Default: close actions panel and refocus editor
        self.close_actions_panel(window, cx);
        cx.notify();
    }

    /// Execute an action by ID (from CommandBar)
    /// Maps string action IDs to NotesAction enum values
    pub(super) fn execute_action(
        &mut self,
        action_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!(action_id, "Executing notes action from CommandBar");

        // Map action ID strings to NotesAction enum
        let action = match action_id {
            "new_note" => Some(NotesAction::NewNote),
            "duplicate_note" => Some(NotesAction::DuplicateNote),
            "browse_notes" => Some(NotesAction::BrowseNotes),
            "toggle_preview" => Some(NotesAction::TogglePreview),
            "cycle_sort_mode" => Some(NotesAction::CycleSortMode),
            "open_trash" => Some(NotesAction::OpenTrash),
            "empty_trash" => Some(NotesAction::EmptyTrash),
            "back_to_notes" => Some(NotesAction::BackToNotes),
            "history_back" => Some(NotesAction::HistoryBack),
            "history_forward" => Some(NotesAction::HistoryForward),
            "find_in_note" => Some(NotesAction::FindInNote),
            "format" => Some(NotesAction::Format),
            "copy_note_as" => Some(NotesAction::CopyNoteAs),
            "copy_deeplink" => Some(NotesAction::CopyDeeplink),
            "create_quicklink" => Some(NotesAction::CreateQuicklink),
            "copy_backlinks" => Some(NotesAction::CopyBacklinks),
            "export" => Some(NotesAction::Export),
            "delete_note" => Some(NotesAction::DeleteNote),
            "restore_note" => Some(NotesAction::RestoreNote),
            "permanently_delete_note" => Some(NotesAction::PermanentlyDeleteNote),
            "toggle_auto_sizing" | "enable_auto_sizing" => Some(NotesAction::EnableAutoSizing),
            "reset_window_position" => Some(NotesAction::ResetWindowPosition),
            "move_list_item_up" => Some(NotesAction::MoveListItemUp),
            "move_list_item_down" => Some(NotesAction::MoveListItemDown),
            "send_to_ai" => Some(NotesAction::SendToAi),
            _ => {
                tracing::warn!(action_id, "Unknown action ID from CommandBar");
                None
            }
        };

        if let Some(action) = action {
            self.handle_action(action, window, cx);
        } else {
            // Unknown action - just close the command bar
            self.close_actions_panel(window, cx);
        }
    }

    /// Execute an action from the note switcher (Cmd+P)
    /// Handles note selection when action_id starts with "note_"
    pub(super) fn execute_note_switcher_action(
        &mut self,
        action_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        debug!(action_id, "Executing note switcher action");

        if let Some(date_str) = action_id.strip_prefix("daypage_") {
            self.close_browse_panel(window, cx);
            if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                self.select_day_note(date, window, cx);
            }
            return;
        }

        // Handle note selection (action_id format: "note_{uuid}")
        if let Some(note_id_str) = action_id.strip_prefix("note_") {
            if let Some(date) = crate::notes::day_switcher::parse_day_note_action_id(note_id_str) {
                self.close_browse_panel(window, cx);
                self.select_day_note(date, window, cx);
                return;
            }

            // Find the note by ID string
            if let Some(note) = self.notes.iter().find(|n| n.id.as_str() == note_id_str) {
                let note_id = note.id;
                if self.replace_active_note_mention_with_note(note_id, window, cx) {
                    return;
                }
                self.close_browse_panel(window, cx);
                self.select_note(note_id, window, cx);
                return;
            }

            tracing::warn!(
                action_id,
                note_id_str,
                selected_note_id = ?self.selected_note_id,
                notes_len = self.notes.len(),
                "notes_note_switcher_selected_note_not_found",
            );
            self.show_selected_note_missing_feedback("execute_note_switcher_action", cx);
            self.close_browse_panel(window, cx);
            return;
        }

        // Handle "no_notes" placeholder action
        if action_id == "no_notes" {
            self.close_browse_panel(window, cx);
            self.create_note(window, cx);
            return;
        }

        // Unknown action - just close
        tracing::warn!(action_id, "Unknown note switcher action");
        self.close_browse_panel(window, cx);
    }

    /// Open the browse panel (note switcher) with current notes
    /// Uses CommandBar for consistent theming with the Cmd+K actions dialog
    pub(super) fn open_browse_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Update note switcher actions based on current notes
        let mut rows = self
            .notes
            .iter()
            .map(|n| NoteSwitcherNoteInfo {
                id: n.id.as_str().to_string(),
                title: if n.title.is_empty() {
                    "Untitled Note".to_string()
                } else {
                    n.title.clone()
                },
                char_count: n.char_count(),
                is_current: Some(n.id) == self.selected_note_id,
                is_pinned: n.is_pinned,
                preview: Self::note_switcher_preview(n),
                relative_time: Self::format_relative_time(n.updated_at),
            })
            .collect::<Vec<_>>();
        let day_entries = crate::notes::day_switcher::load_day_note_switcher_entries(
            &crate::notes::notes_brain_days_dir(),
        );
        rows.extend(crate::notes::day_switcher::day_note_switcher_infos(
            &day_entries,
            self.active_day_binding.as_ref().map(|binding| binding.date),
        ));
        let note_switcher_actions = get_note_switcher_actions(&rows);

        // Log what actions we're setting
        info!(
            "Notes open_browse_panel: setting {} note actions",
            note_switcher_actions.len(),
        );

        self.note_switcher.set_actions(note_switcher_actions, cx);

        // Open the note switcher (CommandBar handles window creation internally)
        self.note_switcher.open_centered(window, cx);
        self.wire_command_bar_activation(NotesCommandBarRole::NoteSwitcher, window, cx);

        // The recent-notes switcher should not show a context-title / count
        // chip. Clear any stale title so reopens after a config change render
        // a clean header. The search placeholder is owned by
        // CommandBarConfig::notes_recent_style ("Search Notes").
        if let Some(dialog) = self.note_switcher.dialog() {
            dialog.update(cx, |d, cx| {
                d.set_context_title(None);
                cx.notify();
            });
        }

        // Route through NotesFocusSurface for structured logging and consistent focus management.
        // The ActionsWindow is a visual-only popup — it does NOT take keyboard focus.
        self.request_focus_surface(focus::NotesFocusSurface::BrowsePanel, window, cx);
    }

    /// Close the browse panel (note switcher) and refocus the editor
    pub(super) fn close_browse_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Close the note switcher CommandBar window
        self.note_switcher.close(cx);

        self.mention_portal_edit = None;

        // Route through NotesFocusSurface for structured logging and consistent focus management.
        self.request_focus_surface(focus::NotesFocusSurface::Editor, window, cx);
    }

    /// Toggle the search bar visibility
    pub(super) fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Exit focus mode if active (search requires chrome)
        if self.focus_mode {
            self.focus_mode = false;
        }
        self.show_search = !self.show_search;

        if self.show_search {
            // Focus the search input
            self.search_state.update(cx, |state, cx| {
                state.focus(window, cx);
            });
        } else {
            // Clear search and refocus editor
            self.search_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
            self.search_query.clear();
            self.editor_state.update(cx, |state, cx| {
                state.focus(window, cx);
            });
        }

        cx.notify();
    }

    /// Toggle markdown preview mode (Cmd+Shift+P)
    pub(crate) fn toggle_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_enabled = !self.preview_enabled;

        if self.preview_enabled {
            // Keep focus on the NotesApp so shortcuts still work while previewing.
            self.focus_handle.focus(window, cx);
        } else {
            // Return focus to editor for editing.
            self.editor_state.update(cx, |state, cx| {
                state.focus(window, cx);
            });
        }

        cx.notify();
    }
}
