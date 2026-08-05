use super::*;

impl NotesApp {
    /// Fetch notes matching a search query, or all notes if the query is blank.
    ///
    /// Returns `(notes, used_full_list)` where `used_full_list` is true when
    /// the query was empty and we reloaded the entire note set.
    pub(super) fn refresh_notes_for_search_query(
        &self,
        query: &str,
    ) -> anyhow::Result<(Vec<Note>, bool)> {
        if query.trim().is_empty() {
            return storage::get_all_notes()
                .map(|notes| (notes, true))
                .map_err(|error| {
                    anyhow::anyhow!(
                        "Failed to reload all notes while clearing the notes search: {error}"
                    )
                });
        }

        storage::search_notes(query)
            .map(|notes| (notes, false))
            .map_err(|error| {
                anyhow::anyhow!("Failed to search notes for query {:?}: {error}", query)
            })
    }

    pub(super) fn on_search_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.search_state.read(cx).value().to_string();
        let search_was_focused = self
            .search_state
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);
        let selection_before = self.selected_note_id.map(|id| id.as_str().to_string());

        self.search_query = query.clone();

        tracing::info!(
            event = "notes_search_refresh_started",
            query = %query,
            notes_before = self.notes.len(),
            has_unsaved_changes = self.has_unsaved_changes,
            search_was_focused,
            selection_before = %selection_before.as_deref().unwrap_or("none"),
            "notes_search_refresh_started"
        );

        // Save before replacing self.notes so dirty edits are not lost
        if self.has_unsaved_changes && !self.save_current_note() {
            tracing::warn!(
                event = "notes_search_refresh_blocked",
                query = %query,
                reason = "save_current_note_failed",
                "notes_search_refresh_blocked"
            );
            return;
        }

        let (refreshed_notes, used_full_list) = match self.refresh_notes_for_search_query(&query) {
            Ok(result) => result,
            Err(error) => {
                tracing::error!(
                    event = "notes_search_refresh_failed",
                    query = %query,
                    error = %error,
                    "notes_search_refresh_failed"
                );
                return;
            }
        };

        self.notes = refreshed_notes;

        let selection_is_visible = self
            .selected_note_id
            .is_some_and(|id| self.notes.iter().any(|note| note.id == id));

        let mut restored_search_focus = false;

        if !selection_is_visible {
            self.sync_search_selection(window, cx);

            // Restore search focus after sync_search_selection (which calls select_note → editor focus)
            if search_was_focused {
                self.search_state.update(cx, |state, cx| {
                    state.focus(window, cx);
                });
                restored_search_focus = true;
            }

            let selection_after = self.selected_note_id.map(|id| id.as_str().to_string());

            tracing::info!(
                event = "notes_search_refresh_completed",
                query = %query,
                used_full_list,
                result_count = self.notes.len(),
                selection_before = %selection_before.as_deref().unwrap_or("none"),
                selection_after = %selection_after.as_deref().unwrap_or("none"),
                selection_changed = selection_before != selection_after,
                restored_search_focus,
                "notes_search_refresh_completed"
            );
            return;
        }

        let selection_after = self.selected_note_id.map(|id| id.as_str().to_string());

        tracing::info!(
            event = "notes_search_refresh_completed",
            query = %query,
            used_full_list,
            result_count = self.notes.len(),
            selection_before = %selection_before.as_deref().unwrap_or("none"),
            selection_after = %selection_after.as_deref().unwrap_or("none"),
            selection_changed = selection_before != selection_after,
            restored_search_focus,
            "notes_search_refresh_completed"
        );

        cx.notify();
    }

    pub(super) fn reload_after_external_note_mutation(
        &mut self,
        changed_note_id: NoteId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        tracing::info!(
            event = "notes_external_mcp_refresh_started",
            note_id = %changed_note_id,
            search_query = %self.search_query,
            has_unsaved_changes = self.has_unsaved_changes,
            "notes_external_mcp_refresh_started"
        );

        if self.has_unsaved_changes && !self.save_current_note() {
            anyhow::bail!("Failed to save dirty Notes editor before MCP refresh");
        }

        let query = self.search_query.clone();
        let (refreshed_notes, _used_full_list) = self.refresh_notes_for_search_query(&query)?;
        self.notes = refreshed_notes;
        self.deleted_notes = storage::get_deleted_notes()
            .map_err(|error| anyhow::anyhow!("Failed to reload deleted notes: {error}"))?;

        if self
            .notes
            .iter()
            .any(|note| note.id == changed_note_id && note.deleted_at.is_none())
        {
            self.view_mode = NotesViewMode::AllNotes;
            self.select_note(changed_note_id, window, cx);
        } else {
            self.sync_search_selection(window, cx);
        }

        tracing::info!(
            event = "notes_external_mcp_refresh_completed",
            note_id = %changed_note_id,
            notes_len = self.notes.len(),
            deleted_notes_len = self.deleted_notes.len(),
            selected_note_id = %self.selected_note_id.map(|id| id.as_str()).unwrap_or_else(|| "none".to_string()),
            "notes_external_mcp_refresh_completed"
        );
        cx.notify();
        Ok(())
    }

    /// Sync selection to first search result after filtering changes the visible note list.
    pub(super) fn sync_search_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(first) = self.notes.first() {
            let id = first.id;
            self.select_note(id, window, cx);
        } else {
            self.selected_note_id = None;
            self.editor_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
            cx.notify();
        }
    }

    /// Create a new note
    pub(super) fn create_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let note = Note::new();
        let id = note.id;

        // Save to storage
        if let Err(e) = storage::save_note(&note) {
            tracing::error!(error = %e, "Failed to create note");
            return;
        }

        // Add to cache and select it
        self.notes.insert(0, note);
        self.select_note(id, window, cx);

        info!(note_id = %id, "New note created");
    }

    /// Create a new note pre-filled with the given content.
    ///
    /// Used by cross-window features like "Save as Note" from the AI chat.
    pub(crate) fn create_note_with_content(
        &mut self,
        content: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if content.is_empty() {
            self.create_note(window, cx);
            return Ok(());
        }

        let note = Note::with_content(content);
        let id = note.id;

        storage::save_note(&note).map_err(|e| {
            tracing::error!(error = %e, "Failed to create note with content");
            anyhow::anyhow!("Failed to create note with content: {e}")
        })?;

        self.notes.insert(0, note);
        self.select_note(id, window, cx);

        info!(note_id = %id, "New note created with content");
        Ok(())
    }

    /// Create a new note pre-filled with system clipboard content (Cmd+Shift+N)
    pub(super) fn create_note_from_clipboard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let clipboard_content = Self::read_clipboard();
        if clipboard_content.is_empty() {
            // Nothing on clipboard, just create an empty note
            self.create_note(window, cx);
            return;
        }

        let note = Note::with_content(clipboard_content);
        let id = note.id;

        if let Err(e) = storage::save_note(&note) {
            tracing::error!(error = %e, "Failed to create note from clipboard");
            return;
        }

        self.notes.insert(0, note);
        self.select_note(id, window, cx);

        info!(note_id = %id, "New note created from clipboard");
    }

    /// Read text from system clipboard
    pub(super) fn read_clipboard() -> String {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            Command::new("pbpaste")
                .output()
                .ok()
                .and_then(|output| {
                    if output.status.success() {
                        String::from_utf8(output.stdout).ok()
                    } else {
                        None
                    }
                })
                .unwrap_or_default()
        }
        #[cfg(not(target_os = "macos"))]
        {
            String::new()
        }
    }

    /// Internal note selection with optional editor focus.
    fn select_note_internal(
        &mut self,
        id: NoteId,
        focus_editor_after_select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Save any unsaved changes to the current note before switching
        self.save_current_note();
        self.active_day_binding = None;

        // Push current note onto history stack (unless navigating back/forward)
        if !self.navigating_history {
            if let Some(prev_id) = self.selected_note_id {
                if prev_id != id {
                    self.history_back.push(prev_id);
                    // Clear forward history on new navigation
                    self.history_forward.clear();
                }
            }
        }

        self.selected_note_id = Some(id);

        // Load content into editor
        let note_list = if self.view_mode == NotesViewMode::Trash {
            &self.deleted_notes
        } else {
            &self.notes
        };

        if let Some(note) = note_list.iter().find(|n| n.id == id) {
            self.editor_state.update(cx, |state, cx| {
                state.set_value(&note.content, window, cx);
                // Open at the TOP with the caret at the start so the note's
                // title (the H1 first line) is the first visible line. Putting
                // the caret at the end scrolls the editor to the bottom
                // (set_selection scrolls to the caret), which clipped the
                // accent-colored H1 off the top — the reported "stray yellow dot"
                // was the sliver of that title line peeking above the fold.
                // (select_day_note deliberately lands at the end for append.)
                state.set_selection(0, 0, window, cx);
            });
        }

        if focus_editor_after_select {
            // Focus the editor after selecting a note
            self.editor_state.update(cx, |state, cx| {
                state.focus(window, cx);
            });
        }

        self.recompute_notes_ghost(cx);
        cx.notify();
    }

    pub(super) fn select_day_note(
        &mut self,
        date: chrono::NaiveDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.has_unsaved_changes && !self.save_current_note() {
            tracing::warn!(
                target: "script_kit::notes",
                event = "notes_day_note_select_blocked",
                date = %date,
                reason = "save_current_note_failed",
            );
            return;
        }

        let path = storage::notes_brain_days_dir().join(format!("{date}.md"));
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                tracing::error!(
                    target: "script_kit::notes",
                    event = "notes_day_note_read_failed",
                    date = %date,
                    path = %path.display(),
                    error = %error,
                );
                return;
            }
        };

        self.view_mode = NotesViewMode::AllNotes;
        self.selected_note_id = None;
        self.active_day_binding = Some(NotesDayBinding {
            date,
            path,
            content: content.clone(),
            base_disk_content: content.clone(),
        });
        self.has_unsaved_changes = false;
        self.history_forward.clear();
        self.editor_state.update(cx, |state, cx| {
            state.set_value(&content, window, cx);
            let content_len = content.len();
            state.set_selection(content_len, content_len, window, cx);
            state.focus(window, cx);
        });
        self.recompute_notes_ghost(cx);
        tracing::info!(
            target: "script_kit::notes",
            event = "notes_day_note_selected",
            date = %date,
        );
        cx.notify();
    }

    /// Select a note for editing (with editor focus)
    pub(super) fn select_note(&mut self, id: NoteId, window: &mut Window, cx: &mut Context<Self>) {
        self.select_note_internal(id, true, window, cx);
    }

    /// Select an existing active note from root launcher search.
    pub(super) fn select_note_by_id_from_root(
        &mut self,
        id: NoteId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if self.has_unsaved_changes && !self.save_current_note() {
            anyhow::bail!("Failed to save current note before opening root note");
        }

        self.view_mode = NotesViewMode::AllNotes;
        self.search_query.clear();
        self.search_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.notes = storage::get_all_notes()
            .map_err(|error| anyhow::anyhow!("Failed to reload notes for root open: {error}"))?;

        if !self.notes.iter().any(|note| note.id == id) {
            anyhow::bail!("Root note is missing or deleted");
        }

        self.select_note(id, window, cx);
        Ok(())
    }

    /// Select a note without immediately focusing the editor.
    ///
    /// Used by the delete flow so that focus restoration happens via the
    /// pending focus-surface pattern after dialog dismissal.
    pub(super) fn select_note_without_focus(
        &mut self,
        id: NoteId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_note_internal(id, false, window, cx);
    }

    /// Clamp dialog width so it never exceeds the available space in a
    /// narrow Notes popup window.  The `24.0` accounts for horizontal
    /// padding (12px each side).
    fn clamp_notes_delete_dialog_width(window_width: f32) -> f32 {
        let available_width = (window_width - 24.0).max(0.0);
        available_width.min(448.0)
    }

    /// Minimum width (px) to trust a viewport or bounds measurement as a real
    /// window size.  Tiny positive values during startup produce an invisible
    /// 0 px dialog, so we fall through to the default instead.
    const MIN_NOTES_DELETE_DIALOG_SOURCE_WIDTH: f32 = 240.0;

    /// Default source width when neither viewport nor bounds are viable.
    /// Produces a 448 px dialog after the 24 px padding clamp.
    const DEFAULT_NOTES_DELETE_DIALOG_SOURCE_WIDTH: f32 = 472.0;

    /// Resolve the source width for the Notes delete dialog.
    ///
    /// Prefer the viewport width when available, but fall back to bounds
    /// width for the "open window -> immediate delete shortcut" path where
    /// viewport size can still be zero before the first stable layout.
    /// Tiny positive values (< 240) are rejected so startup measurements
    /// like 12 px don't produce an invisible dialog.
    pub(crate) fn resolve_notes_delete_dialog_source_width(
        viewport_width: f32,
        bounds_width: f32,
    ) -> f32 {
        if viewport_width.is_finite()
            && viewport_width >= Self::MIN_NOTES_DELETE_DIALOG_SOURCE_WIDTH
        {
            return viewport_width;
        }

        if bounds_width.is_finite() && bounds_width >= Self::MIN_NOTES_DELETE_DIALOG_SOURCE_WIDTH {
            return bounds_width;
        }

        Self::DEFAULT_NOTES_DELETE_DIALOG_SOURCE_WIDTH
    }

    /// Compute dialog width clamped to the Notes window so the dialog
    /// never overflows a narrow popup window.
    fn notes_delete_dialog_width(window: &Window) -> gpui::Pixels {
        let viewport_width: f32 = window.viewport_size().width.into();
        let bounds_width: f32 = window.bounds().size.width.into();
        let source_width =
            Self::resolve_notes_delete_dialog_source_width(viewport_width, bounds_width);
        gpui::px(Self::clamp_notes_delete_dialog_width(source_width))
    }

    /// Restore keyboard focus to the editor after modal dismissal.
    pub(super) fn focus_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor_state.update(cx, |state, cx| {
            state.focus(window, cx);
        });
    }

    /// Request deletion of the currently selected note with a confirmation dialog.
    ///
    /// Opens the shared parent confirm dialog; the actual soft-delete happens
    /// only after the user confirms via `WeakEntity::update_in`.
    pub(super) fn request_delete_selected_note(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(note_id) = self.selected_note_id else {
            tracing::debug!(
                event = "notes_delete_confirmation_skipped",
                reason = "no_selected_note",
                "notes_delete_confirmation_skipped"
            );
            return;
        };

        let note_list = if self.view_mode == NotesViewMode::Trash {
            &self.deleted_notes
        } else {
            &self.notes
        };

        let note_title = note_list
            .iter()
            .find(|n| n.id == note_id)
            .map(|n| n.title.clone())
            .unwrap_or_default();

        let viewport_width: f32 = window.viewport_size().width.into();
        let bounds_width: f32 = window.bounds().size.width.into();
        let source_width =
            Self::resolve_notes_delete_dialog_source_width(viewport_width, bounds_width);
        let dialog_width = Self::notes_delete_dialog_width(window);
        let dialog_width_value: f32 = dialog_width.into();

        let viewport_viable = viewport_width.is_finite()
            && viewport_width >= Self::MIN_NOTES_DELETE_DIALOG_SOURCE_WIDTH;
        let bounds_viable =
            bounds_width.is_finite() && bounds_width >= Self::MIN_NOTES_DELETE_DIALOG_SOURCE_WIDTH;

        tracing::info!(
            event = "notes_delete_confirmation_requested",
            note_id = %note_id.as_str(),
            note_title_length = note_title.chars().count(),
            note_title_fingerprint = %Self::devtools_text_fingerprint(&note_title),
            is_trash_view = (self.view_mode == NotesViewMode::Trash),
            viewport_width,
            bounds_width,
            viewport_viable,
            bounds_viable,
            source_width,
            dialog_width = dialog_width_value,
            "notes_delete_confirmation_requested"
        );

        let is_trash_view = self.view_mode == NotesViewMode::Trash;

        let (title, body, confirm_text): (
            gpui::SharedString,
            gpui::SharedString,
            gpui::SharedString,
        ) = if is_trash_view {
            let body = if note_title.is_empty() {
                "Delete this note permanently? This cannot be undone.".into()
            } else {
                format!(
                    "Delete \"{}\" permanently? This cannot be undone.",
                    note_title
                )
                .into()
            };
            (
                "Delete note permanently".into(),
                body,
                "Delete permanently".into(),
            )
        } else {
            let body = if note_title.is_empty() {
                "Move this note to Trash? You can restore it later with \u{2318}\u{21e7}T.".into()
            } else {
                format!(
                    "Move \"{}\" to Trash? You can restore it later with \u{2318}\u{21e7}T.",
                    note_title
                )
                .into()
            };
            ("Move note to Trash".into(), body, "Delete".into())
        };

        self.request_focus_surface(NotesFocusSurface::Dialog, window, cx);

        let weak_notes = cx.entity().downgrade();
        let confirm_note_id = note_id;
        let cancel_note_id = note_id;
        let weak_notes_for_cancel = weak_notes.clone();

        // Non-entity-bound: avoids keep_open_while closing dialog on re-render.
        // Pin the popup explicitly to the Notes window so AppKit attaches it as
        // a child of the Notes NSPanel and bottom-aligns to *its* frame instead
        // of whichever window happens to be the current key window.
        crate::confirm::open_parent_confirm_dialog_for_automation_parent(
            window,
            cx,
            "notes",
            crate::confirm::ParentConfirmOptions {
                title,
                body,
                confirm_text,
                cancel_text: "Cancel".into(),
                confirm_variant: gpui_component::button::ButtonVariant::Danger,
                width: dialog_width,
            },
            {
                let weak_notes = weak_notes.clone();
                move |window, cx| {
                    tracing::info!(
                        event = "notes_delete_confirmed",
                        note_id = %confirm_note_id.as_str(),
                        delete_mode = if is_trash_view { "permanent" } else { "soft" },
                        "notes_delete_confirmed"
                    );

                    if let Some(entity) = weak_notes.upgrade() {
                        entity.update(cx, |this, cx| {
                            if is_trash_view {
                                this.permanently_delete_note(window, cx);
                            } else {
                                this.delete_note_by_id(confirm_note_id, window, cx);
                            }
                        });
                    }

                    let msg = if is_trash_view {
                        "Note deleted permanently"
                    } else {
                        "Note moved to Trash"
                    };
                    let notif_bg = crate::ui_foundation::get_vibrancy_surface_background(0.55);
                    window.push_notification(
                        gpui_component::notification::Notification::success(msg).bg(notif_bg),
                        cx,
                    );
                }
            },
            move |window, cx| {
                tracing::info!(
                    event = "notes_delete_cancelled",
                    note_id = %cancel_note_id.as_str(),
                    delete_mode = if is_trash_view { "permanent" } else { "soft" },
                    "notes_delete_cancelled"
                );

                if let Some(entity) = weak_notes_for_cancel.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.restore_primary_focus_after_dialog(window, cx);
                    });
                }
            },
        );

        cx.notify();

        tracing::info!(
            event = "notes_delete_confirmation_opened",
            note_id = %note_id.as_str(),
            has_active_dialog = window.has_active_dialog(cx),
            pending_focus_surface = ?self.pending_focus_surface,
            "notes_delete_confirmation_opened"
        );
    }

    /// Delete a specific note by ID (soft delete).
    ///
    /// This is the actual deletion logic, called after confirmation.
    pub(super) fn delete_note_by_id(
        &mut self,
        note_id: NoteId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!(note_id = %note_id, notes_count = self.notes.len(), "delete_note_by_id called");
        if let Some(idx) = self.notes.iter().position(|n| n.id == note_id) {
            let mut note = self.notes.remove(idx);
            note.soft_delete();

            if let Err(e) = storage::save_note(&note) {
                tracing::error!(error = %e, "Failed to delete note");
            }

            // Move to deleted notes
            self.deleted_notes.insert(0, note);
        }

        // Select next note without immediate editor focus — focus restoration
        // happens via the pending focus-surface pattern after dialog dismissal.
        if let Some(next_note) = self.notes.first() {
            let next_id = next_note.id;
            self.select_note_without_focus(next_id, window, cx);
        } else {
            self.selected_note_id = None;
            self.editor_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
        }

        self.request_focus_surface(self.primary_focus_surface(), window, cx);
        self.show_action_feedback("Deleted · ⌘⇧T trash", false);
        cx.notify();
    }

    /// Delete the currently selected note (soft delete) — direct path without confirmation.
    ///
    /// Kept for backwards compatibility with browse-panel inline delete.
    pub(super) fn delete_selected_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        info!(selected_note_id = ?self.selected_note_id, notes_count = self.notes.len(), "delete_selected_note called");
        if let Some(id) = self.selected_note_id {
            self.delete_note_by_id(id, window, cx);
        }
    }

    /// Permanently delete the selected note from trash
    pub(super) fn permanently_delete_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_note_id else {
            return;
        };

        if let Err(e) = storage::delete_note_permanently(id) {
            tracing::error!(error = %e, "Failed to permanently delete note");
            return;
        }

        self.deleted_notes.retain(|n| n.id != id);

        // Select next note without immediate editor focus — focus restoration
        // happens via the pending focus-surface pattern after dialog dismissal.
        if let Some(next_note) = self.deleted_notes.first() {
            self.select_note_without_focus(next_note.id, window, cx);
        } else {
            self.selected_note_id = None;
            self.editor_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
        }

        self.request_focus_surface(self.primary_focus_surface(), window, cx);
        info!(note_id = %id, "Note permanently deleted");
        cx.notify();
    }

    /// Restore the selected note from trash
    pub(super) fn restore_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_note_id {
            if let Some(idx) = self.deleted_notes.iter().position(|n| n.id == id) {
                let mut note = self.deleted_notes.remove(idx);
                note.restore();

                if let Err(e) = storage::save_note(&note) {
                    tracing::error!(error = %e, "Failed to restore note");
                    self.deleted_notes.insert(idx, note);
                    return;
                }

                // Move back to active notes
                self.notes.insert(0, note);
            }

            self.view_mode = NotesViewMode::AllNotes;
            self.selected_note_id = Some(id);
            self.select_note(id, window, cx);

            info!(note_id = %id, "Note restored");
            cx.notify();
        }
    }

    /// Switch view mode
    pub(super) fn set_view_mode(
        &mut self,
        mode: NotesViewMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.view_mode = mode;

        // Select first note in new view
        let notes = match mode {
            NotesViewMode::AllNotes => &self.notes,
            NotesViewMode::Trash => &self.deleted_notes,
        };

        if let Some(note) = notes.first() {
            self.select_note(note.id, window, cx);
        } else {
            self.selected_note_id = None;
            self.editor_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
        }

        cx.notify();
    }

    /// Export the current note
    pub(super) fn export_note(&mut self, format: ExportFormat, cx: &mut Context<Self>) {
        let Some((_id, note)) = self.selected_note_for_action("export_note", cx) else {
            return;
        };
        let title = note.title.clone();
        let note_content = note.content.clone();

        let content = match format {
            ExportFormat::PlainText => note_content.clone(),
            // For Markdown, just export the content as-is.
            // The title is derived from the first line of content,
            // so prepending it would cause duplication.
            ExportFormat::Markdown => note_content.clone(),
            ExportFormat::Html => {
                // For HTML, we include proper structure with the title
                // and render the content as preformatted text
                format!(
                    "<!DOCTYPE html>\n<html>\n<head><title>{}</title></head>\n<body>\n<h1>{}</h1>\n<pre>{}</pre>\n</body>\n</html>",
                    title, title, note_content
                )
            }
        };

        // Copy to clipboard
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(stdin) = child.stdin.as_mut() {
                        stdin.write_all(content.as_bytes())?;
                    }
                    child.wait()
                });
            info!(format = ?format, "Note exported to clipboard");
        }
    }
}

#[cfg(test)]
mod notes_search_and_delete_regression_tests {
    use std::fs;

    fn extract_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        source
            .split(start)
            .nth(1)
            .and_then(|section| section.split(end).next())
            .expect("expected section to exist")
    }

    fn normalize_ws(source: &str) -> String {
        source.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn test_on_search_change_saves_before_filtering_and_restores_search_focus() {
        let source = fs::read_to_string("src/notes/window/notes.rs")
            .expect("Failed to read src/notes/window/notes.rs");
        let normalized = normalize_ws(&source);

        let save_idx = normalized
            .find("self.save_current_note()")
            .expect("on_search_change should save the current note before filtering");
        let replace_idx = normalized
            .find("self.notes = refreshed_notes;")
            .expect("on_search_change should replace notes with refreshed results");
        let focus_capture_idx = normalized
            .find("let search_was_focused = self")
            .expect("on_search_change should capture whether the search input was focused");
        let focus_restore_idx = normalized
            .find("self.search_state.update(cx, |state, cx| { state.focus(window, cx); });")
            .expect(
                "on_search_change should restore search focus after search-driven selection sync",
            );

        assert!(
            save_idx < replace_idx,
            "on_search_change must save the edited note before replacing self.notes"
        );
        assert!(
            focus_capture_idx < focus_restore_idx,
            "on_search_change should capture focus state before refresh and restore it afterward"
        );
    }

    #[test]
    fn test_request_delete_selected_note_uses_shared_parent_confirm_helper() {
        let source = fs::read_to_string("src/notes/window/notes.rs")
            .expect("Failed to read src/notes/window/notes.rs");

        let delete_request = extract_section(
            &source,
            "pub(super) fn request_delete_selected_note",
            "/// Delete a specific note by ID (soft delete).",
        );
        let normalized = normalize_ws(delete_request);

        assert!(
            normalized
                .contains("crate::confirm::open_parent_confirm_dialog_for_automation_parent("),
            "Notes delete should use the parent-id-aware confirm helper so the popup pins to the Notes window"
        );
        assert!(
            normalized.contains("\"notes\""),
            "Notes delete should explicitly parent the confirm popup to automation id \"notes\""
        );
        assert!(
            !normalized.contains("window.open_dialog(cx, move |dialog"),
            "Notes delete should not inline dialog construction"
        );
        assert!(
            !normalized.contains("This note will move to Trash."),
            "Notes delete should use the simplified single-sentence dialog body"
        );
    }

    #[test]
    fn test_request_delete_selected_note_routes_through_weak_entity() {
        let source = fs::read_to_string("src/notes/window/notes.rs")
            .expect("Failed to read src/notes/window/notes.rs");

        let delete_request = extract_section(
            &source,
            "pub(super) fn request_delete_selected_note",
            "/// Delete a specific note by ID (soft delete).",
        );
        let normalized = normalize_ws(delete_request);

        assert!(
            normalized.contains("let weak_notes = cx.entity().downgrade();")
                && normalized.contains("entity.update(cx, |this, cx|")
                && normalized.contains("this.delete_note_by_id(confirm_note_id, window, cx);"),
            "confirmed deletes should still route through delete_note_by_id via WeakEntity"
        );
        assert!(
            !normalized.contains("crate::confirm::open_confirm_window")
                && !normalized.contains("async_channel::bounded::<bool>(1)"),
            "notes delete confirmation should not use the separate confirm popup window"
        );
    }

    #[test]
    fn test_request_delete_selected_note_clamps_width_and_notifies() {
        let source = fs::read_to_string("src/notes/window/notes.rs")
            .expect("Failed to read src/notes/window/notes.rs");
        let normalized = normalize_ws(&source);

        assert!(
            normalized.contains("fn notes_delete_dialog_width(window: &Window) -> gpui::Pixels"),
            "Notes delete should define a Notes-specific dialog width helper"
        );
        assert!(
            normalized.contains("fn clamp_notes_delete_dialog_width(window_width: f32) -> f32"),
            "Notes delete should define a testable width clamp helper"
        );

        let delete_request = extract_section(
            &source,
            "pub(super) fn request_delete_selected_note",
            "/// Delete a specific note by ID (soft delete).",
        );
        let delete_request = normalize_ws(delete_request);

        assert!(
            delete_request.contains("width: dialog_width,"),
            "Notes delete should use the computed Notes dialog width"
        );
        assert!(
            delete_request.contains("cx.notify();"),
            "Notes delete should request a repaint after opening the dialog"
        );
    }

    #[test]
    fn test_notes_delete_dialog_width_shrinks_for_narrow_windows() {
        // 240px window → available = 216, no min clamp → 216
        assert_eq!(
            super::NotesApp::clamp_notes_delete_dialog_width(240.0),
            216.0
        );
        // 320px window → available = 296, under cap → 296
        assert_eq!(
            super::NotesApp::clamp_notes_delete_dialog_width(320.0),
            296.0
        );
        // 600px window → available = 576, capped at 448
        assert_eq!(
            super::NotesApp::clamp_notes_delete_dialog_width(600.0),
            448.0
        );
        // Very narrow: 10px window → available = 0 (clamped to 0)
        assert_eq!(super::NotesApp::clamp_notes_delete_dialog_width(10.0), 0.0);
        // Exactly at cap boundary: 472 → 448
        assert_eq!(
            super::NotesApp::clamp_notes_delete_dialog_width(472.0),
            448.0
        );
    }

    #[test]
    fn test_request_delete_selected_note_uses_entity_owned_confirm_helper() {
        let source = fs::read_to_string("src/notes/window/notes.rs")
            .expect("Failed to read src/notes/window/notes.rs");

        let delete_request = extract_section(
            &source,
            "pub(super) fn request_delete_selected_note",
            "/// Delete a specific note by ID (soft delete).",
        );
        let normalized = normalize_ws(delete_request);

        assert!(
            normalized
                .contains("crate::confirm::open_parent_confirm_dialog_for_automation_parent("),
            "Notes delete should use the parent-id-aware confirm helper"
        );
        assert!(
            normalized.contains("weak_notes.clone()"),
            "Notes delete dialog should pass the WeakEntity for lifecycle binding"
        );
    }

    #[test]
    fn test_notes_delete_confirmation_opened_log_reports_dialog_state() {
        const NOTES_SOURCE: &str = include_str!("notes.rs");
        assert!(
            NOTES_SOURCE.contains("has_active_dialog = window.has_active_dialog(cx),")
                && NOTES_SOURCE.contains("pending_focus_surface = ?self.pending_focus_surface,"),
            "Notes delete open log should report dialog state so invisible-dialog failures are diagnosable"
        );
    }
}
