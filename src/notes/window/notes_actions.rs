use super::*;
use std::path::PathBuf;

use crate::notes::deeplink_activation::{
    resolve_activation, run_deeplink_confirm_options, Activation, ActivationErrorReason,
    ActivationSurface,
};

impl NotesApp {
    const SELECTED_NOTE_NOT_FOUND_FEEDBACK: &'static str = "Selected note could not be found";

    pub(super) fn resolve_selected_note(
        selected_note_id: Option<NoteId>,
        notes: &[Note],
    ) -> Option<(NoteId, &Note)> {
        let selected_note_id = selected_note_id?;
        notes
            .iter()
            .find(|note| note.id == selected_note_id)
            .map(|note| (selected_note_id, note))
    }

    pub(super) fn show_selected_note_missing_feedback(
        &mut self,
        action: &'static str,
        cx: &mut Context<Self>,
    ) {
        tracing::warn!(
            action,
            selected_note_id = ?self.selected_note_id,
            notes_len = self.notes.len(),
            "notes_action_selected_note_not_found",
        );
        self.show_action_feedback(Self::SELECTED_NOTE_NOT_FOUND_FEEDBACK, true);
        cx.notify();
    }

    pub(super) fn selected_note_for_action(
        &mut self,
        action: &'static str,
        cx: &mut Context<Self>,
    ) -> Option<(NoteId, &Note)> {
        let Some(selected_note_id) = self.selected_note_id else {
            self.show_selected_note_missing_feedback(action, cx);
            return None;
        };

        if !self.notes.iter().any(|note| note.id == selected_note_id) {
            self.show_selected_note_missing_feedback(action, cx);
            return None;
        }

        Self::resolve_selected_note(Some(selected_note_id), &self.notes)
    }

    pub(super) fn activate_deeplink_under_cursor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let href = self.notes_editor.read(cx).activation_href_at_cursor(cx);
        let Some(href) = href else {
            return false;
        };

        let activation = resolve_activation(&href, ActivationSurface::NotesWindow);
        self.handle_deeplink_activation(activation, window, cx);
        true
    }

    pub(super) fn activate_deeplink_from_mouse_up(
        &mut self,
        event: gpui::MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let notes = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let Some(notes) = notes.upgrade() else {
                return;
            };
            notes.update(cx, |this, cx| {
                let selection = this.notes_editor.read(cx).selection(cx);
                if !crate::components::notes_editor::should_activate_deeplink_from_mouse_up(
                    &event, selection,
                ) {
                    return;
                }

                this.activate_deeplink_under_cursor(window, cx);
            });
        });
    }

    fn handle_deeplink_activation(
        &mut self,
        activation: Activation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match activation {
            Activation::ConfirmBeforeRun {
                command_id,
                raw_href,
            } => self.open_run_deeplink_confirm(command_id, raw_href, window, cx),
            Activation::Error(error) => {
                let body = format!(
                    "{}\n\n{}",
                    error.raw_href,
                    activation_error_message(&error.reason)
                );
                self.open_deeplink_info_dialog(
                    "Can't open this link",
                    body,
                    error.raw_href,
                    window,
                    cx,
                );
            }
            Activation::OpenExternalUrl { href } => {
                self.open_external_deeplink_url(href, window, cx);
            }
            Activation::OpenFile { path, raw_href } => {
                self.open_file_deeplink(path, raw_href, window, cx);
            }
            Activation::OpenNote { note_id } => {
                self.open_note_deeplink(note_id, window, cx);
            }
            Activation::ScopedSearch { source, query } => {
                self.open_scoped_search_deeplink(source, query, window, cx);
            }
            Activation::KitResourcePreview { uri, .. } => {
                self.open_kit_resource_preview(uri, window, cx);
            }
        }
    }

    fn open_kit_resource_preview(
        &mut self,
        uri: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match crate::notes::deeplink_activation::read_cheap_kit_resource_preview(&uri) {
            Ok(preview) => {
                tracing::info!(
                    event = "notes_deeplink_kit_resource_preview_opened",
                    uri = %preview.uri,
                    mime_type = %preview.mime_type,
                    truncated = preview.truncated,
                );
                self.kit_resource_preview = Some(preview.into());
                self.preview_enabled = false;
                self.show_search = false;
                self.command_bar.close_app(cx);
                self.note_switcher.close_app(cx);
                self.focus_handle.focus(window, cx);
                self.show_action_feedback("Opened read-only resource preview", false);
                cx.notify();
            }
            Err(error) => self.open_deeplink_error_dialog(
                "Can't open this link",
                format!("{uri}\n\n{error}"),
                uri,
                window,
                cx,
            ),
        }
    }

    pub(super) fn close_kit_resource_preview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.kit_resource_preview.take().is_some() {
            self.editor_state.update(cx, |state, cx| {
                state.focus(window, cx);
            });
            self.show_action_feedback("Closed resource preview", false);
            cx.notify();
        }
    }

    /// The editable source note behind the open kit resource preview, when
    /// the previewed URI is `kit://notes/{id}` and that note still exists.
    pub(super) fn kit_resource_preview_note_source(&self) -> Option<NoteId> {
        let preview = self.kit_resource_preview.as_ref()?;
        let note_id = crate::notes::deeplink_activation::kit_note_source_id(&preview.uri)?;
        crate::notes::get_note(note_id)
            .ok()
            .flatten()
            .is_some()
            .then_some(note_id)
    }

    pub(super) fn copy_kit_resource_preview_uri(&mut self, cx: &mut Context<Self>) {
        let Some(preview) = self.kit_resource_preview.as_ref() else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(preview.uri.clone()));
        self.show_action_feedback("Copied resource URI", false);
        cx.notify();
    }

    /// Preview → edit: close the preview and open its source note in the
    /// editor. Returns false when the previewed resource has no source note.
    pub(super) fn open_kit_resource_preview_source(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(note_id) = self.kit_resource_preview_note_source() else {
            return false;
        };
        self.close_kit_resource_preview(window, cx);
        self.open_note_deeplink(note_id, window, cx);
        true
    }

    fn open_external_deeplink_url(
        &mut self,
        href: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match open::that(&href) {
            Ok(()) => {
                tracing::info!(event = "notes_deeplink_url_opened", href = %href);
                self.show_action_feedback("Opened link", false);
                cx.notify();
            }
            Err(error) => self.open_deeplink_error_dialog(
                "Can't open this link",
                format!("{href}\n\nFailed to open URL: {error}"),
                href,
                window,
                cx,
            ),
        }
    }

    fn open_file_deeplink(
        &mut self,
        path: PathBuf,
        raw_href: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path_display = path.to_string_lossy().to_string();
        if !path.exists() {
            self.open_missing_file_deeplink_dialog(path, raw_href, window, cx);
            return;
        }

        match crate::file_search::open_file(&path_display) {
            Ok(()) => {
                tracing::info!(
                    event = "notes_deeplink_file_opened",
                    path = %path_display,
                    raw_href = %raw_href,
                );
                self.show_action_feedback("Opened file", false);
                cx.notify();
            }
            Err(error) => self.open_deeplink_error_dialog(
                "Can't open this link",
                format!("{raw_href}\n\nFailed to open file:\n{path_display}\n\n{error}"),
                raw_href,
                window,
                cx,
            ),
        }
    }

    fn open_missing_file_deeplink_dialog(
        &mut self,
        path: PathBuf,
        raw_href: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path_display = path.to_string_lossy().to_string();
        let parent = path
            .parent()
            .filter(|parent| parent.exists())
            .map(|parent| parent.to_path_buf());
        let Some(parent) = parent else {
            self.open_deeplink_error_dialog(
                "Can't open this link",
                format!("{raw_href}\n\nFile does not exist:\n{path_display}"),
                raw_href,
                window,
                cx,
            );
            return;
        };

        let parent_display = parent.to_string_lossy().to_string();
        self.request_focus_surface(NotesFocusSurface::Dialog, window, cx);
        crate::confirm::open_parent_confirm_dialog_for_automation_parent(
            window,
            cx,
            "notes",
            crate::confirm::ParentConfirmOptions {
                title: "Can't open this link".into(),
                body: format!(
                    "{raw_href}\n\nFile does not exist:\n{path_display}\n\nParent folder exists:\n{parent_display}"
                )
                .into(),
                confirm_text: "Reveal parent".into(),
                cancel_text: "Dismiss".into(),
                confirm_variant: gpui_component::button::ButtonVariant::Primary,
                width: gpui::px(crate::confirm::PARENT_MODAL_WIDTH_PX),
            },
            move |_window, cx| {
                if let Err(error) = crate::file_search::reveal_in_finder(&parent_display) {
                    tracing::warn!(
                        event = "notes_deeplink_reveal_parent_failed",
                        parent = %parent_display,
                        %error
                    );
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(raw_href.clone()));
                }
            },
            |_window, _cx| {},
        );
    }

    fn open_note_deeplink(&mut self, note_id: NoteId, window: &mut Window, cx: &mut Context<Self>) {
        match crate::notes::open_note_in_notes_window(cx, note_id) {
            Ok(()) => {
                tracing::info!(event = "notes_deeplink_note_opened", note_id = %note_id);
                self.show_action_feedback("Opened note", false);
                cx.notify();
            }
            Err(error) => self.open_deeplink_error_dialog(
                "Can't open this link",
                format!(
                    "scriptkit://notes/{}\n\nCould not open note: {}",
                    note_id.as_str(),
                    error
                ),
                format!("scriptkit://notes/{}", note_id.as_str()),
                window,
                cx,
            ),
        }
    }

    fn open_scoped_search_deeplink(
        &mut self,
        source: crate::spine::catalog_subsearch::ContextSubsearchSource,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let context_link = format!("@{}:{query}", source.prefix());
        if matches!(
            source,
            crate::spine::catalog_subsearch::ContextSubsearchSource::File
                | crate::spine::catalog_subsearch::ContextSubsearchSource::Project
        ) {
            let path = PathBuf::from(query.trim());
            if path.exists() {
                self.open_file_deeplink(path, context_link, window, cx);
                return;
            }
        }

        if source == crate::spine::catalog_subsearch::ContextSubsearchSource::BrowserHistory
            && (query.starts_with("http://") || query.starts_with("https://"))
        {
            self.open_external_deeplink_url(query, window, cx);
            return;
        }

        self.open_deeplink_info_dialog(
            "Open context search",
            format!(
                "{context_link}\n\nUse this scoped context token to search {} for matching context. Exact file/project paths and exact browser URLs open directly.",
                source.search_hint_noun()
            ),
            context_link,
            window,
            cx,
        );
    }

    fn open_run_deeplink_confirm(
        &mut self,
        command_id: String,
        raw_href: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_focus_surface(NotesFocusSurface::Dialog, window, cx);
        let weak_notes = cx.entity().downgrade();
        let weak_notes_for_confirm = weak_notes.clone();
        let weak_notes_for_cancel = weak_notes.clone();
        let command_id_for_confirm = command_id.clone();
        let command_id_for_cancel = command_id.clone();
        crate::confirm::open_parent_confirm_dialog_for_automation_parent(
            window,
            cx,
            "notes",
            run_deeplink_confirm_options(&command_id, &raw_href),
            move |_window, cx| {
                let execution_result =
                    super::execute_notes_run_command_deeplink(&command_id_for_confirm, cx);
                match &execution_result {
                    Ok(needs_main_window) => {
                        tracing::info!(
                            event = "notes_deeplink_run_confirmed",
                            command_id = %command_id_for_confirm,
                            needs_main_window = *needs_main_window,
                            "notes_deeplink_run_confirmed",
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            event = "notes_deeplink_run_execute_failed",
                            command_id = %command_id_for_confirm,
                            error = %error,
                            "notes_deeplink_run_execute_failed",
                        );
                    }
                }
                let feedback = if execution_result.is_ok() {
                    ("Run link submitted", false)
                } else {
                    ("Could not run link", true)
                };
                if let Some(entity) = weak_notes_for_confirm.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.show_action_feedback(feedback.0, feedback.1);
                        cx.notify();
                    });
                }
            },
            {
                move |_window, cx| {
                    tracing::info!(
                        event = "notes_deeplink_run_cancelled",
                        command_id = %command_id_for_cancel,
                        "notes_deeplink_run_cancelled",
                    );
                    if let Some(entity) = weak_notes_for_cancel.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.show_action_feedback("Run link cancelled", false);
                            cx.notify();
                        });
                    }
                }
            },
        );
    }

    fn open_deeplink_info_dialog(
        &mut self,
        title: &'static str,
        body: String,
        copy_link: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_focus_surface(NotesFocusSurface::Dialog, window, cx);
        crate::confirm::open_parent_confirm_dialog_for_automation_parent(
            window,
            cx,
            "notes",
            crate::confirm::ParentConfirmOptions {
                title: title.into(),
                body: body.into(),
                confirm_text: "Copy link".into(),
                cancel_text: "Dismiss".into(),
                confirm_variant: gpui_component::button::ButtonVariant::Primary,
                width: gpui::px(crate::confirm::PARENT_MODAL_WIDTH_PX),
            },
            move |_window, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_link.clone()));
            },
            |_window, _cx| {},
        );
    }

    fn open_deeplink_error_dialog(
        &mut self,
        title: &'static str,
        body: String,
        copy_link: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_deeplink_info_dialog(title, body, copy_link, window, cx);
    }

    /// Cycle sort mode: Updated → Created → Alphabetical → Updated
    pub(super) fn cycle_sort_mode(&mut self, cx: &mut Context<Self>) {
        self.sort_mode = match self.sort_mode {
            NotesSortMode::Updated => NotesSortMode::Created,
            NotesSortMode::Created => NotesSortMode::Alphabetical,
            NotesSortMode::Alphabetical => NotesSortMode::Updated,
        };
        self.apply_sort(cx);
        info!(sort_mode = ?self.sort_mode, "Cycled sort mode");
    }

    /// Apply current sort mode to the notes list
    pub(super) fn apply_sort(&mut self, cx: &mut Context<Self>) {
        match self.sort_mode {
            NotesSortMode::Updated => {
                self.notes.sort_by(|a, b| match (a.is_pinned, b.is_pinned) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => b.updated_at.cmp(&a.updated_at),
                });
            }
            NotesSortMode::Created => {
                self.notes.sort_by(|a, b| match (a.is_pinned, b.is_pinned) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => b.created_at.cmp(&a.created_at),
                });
            }
            NotesSortMode::Alphabetical => {
                self.notes
                    .sort_by_cached_key(|n| (!n.is_pinned, n.title.to_lowercase()));
            }
        }
        cx.notify();
    }

    pub(super) fn request_empty_trash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.deleted_notes.len();
        if count == 0 {
            self.show_action_feedback("Trash is already empty", false);
            cx.notify();
            return;
        }

        self.request_focus_surface(NotesFocusSurface::Dialog, window, cx);
        let weak_notes = cx.entity().downgrade();
        let weak_notes_for_cancel = weak_notes.clone();
        crate::confirm::open_parent_confirm_dialog_for_automation_parent(
            window,
            cx,
            "notes",
            crate::confirm::ParentConfirmOptions {
                title: "Empty Trash".into(),
                body: format!(
                    "Permanently delete {count} {}? This cannot be undone.",
                    if count == 1 { "note" } else { "notes" }
                )
                .into(),
                confirm_text: "Empty Trash".into(),
                cancel_text: "Cancel".into(),
                confirm_variant: gpui_component::button::ButtonVariant::Danger,
                width: gpui::px(crate::confirm::PARENT_MODAL_WIDTH_PX),
            },
            move |_window, cx| {
                if let Some(entity) = weak_notes.upgrade() {
                    entity.update(cx, |this, cx| this.empty_trash(cx));
                }
            },
            move |window, cx| {
                if let Some(entity) = weak_notes_for_cancel.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.restore_primary_focus_after_dialog(window, cx);
                    });
                }
            },
        );
        cx.notify();
    }

    /// Empty the entire trash — permanently deletes all trashed notes after confirmation.
    pub(super) fn empty_trash(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<NoteId> = self.deleted_notes.iter().map(|n| n.id).collect();
        for id in &ids {
            if let Err(e) = storage::delete_note_permanently(*id) {
                tracing::error!(error = %e, note_id = %id, "Failed to permanently delete note");
            }
        }
        self.deleted_notes.clear();
        self.selected_note_id = None;
        info!(count = ids.len(), "Emptied trash");
        cx.notify();
    }

    /// Copy the current note content to clipboard
    pub(super) fn copy_note_to_clipboard(&self, cx: &Context<Self>) {
        let content = self.editor_state.read(cx).value().to_string();
        self.copy_text_to_clipboard(&content);
    }

    pub(super) fn copy_text_to_clipboard(&self, content: &str) {
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
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = content; // Avoid unused warning
        }
    }

    pub(super) fn note_deeplink(&self, id: NoteId) -> String {
        format!("scriptkit://notes/{}", id.as_str())
    }

    /// Follow the `[[wiki link]]` under the cursor (Cmd+Shift+Enter).
    ///
    /// Resolves the link target against note titles/aliases. A unique match
    /// opens that note; no match creates a new note titled after the target
    /// (Obsidian-style); an ambiguous match shows feedback instead of guessing.
    ///
    /// Returns true when the cursor was inside a wiki link.
    pub(super) fn follow_wiki_link_at_cursor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let (value, cursor) = {
            let state = self.editor_state.read(cx);
            (state.value().to_string(), state.selection().start)
        };

        let Some(target) = Self::wiki_link_target_at(&value, cursor) else {
            return false;
        };

        match storage::resolve_note_ref(&target) {
            Ok(storage::NoteRefResolution::Unique(note_id)) => {
                if self.has_unsaved_changes {
                    self.save_current_note();
                }
                self.select_note(note_id, window, cx);
            }
            Ok(storage::NoteRefResolution::NotFound) => {
                if self.has_unsaved_changes {
                    self.save_current_note();
                }
                if let Err(error) =
                    self.create_note_with_content(format!("{target}\n\n"), window, cx)
                {
                    tracing::warn!(error = %error, target, "Failed to create note from wiki link");
                    self.show_action_feedback("Could not create linked note", true);
                }
            }
            Ok(storage::NoteRefResolution::Ambiguous) => {
                self.show_action_feedback(format!("Multiple notes match \"{target}\""), true);
            }
            Err(error) => {
                tracing::warn!(error = %error, target, "Failed to resolve wiki link");
                self.show_action_feedback("Could not resolve link", true);
            }
        }
        cx.notify();
        true
    }

    /// Extract the `[[target]]` whose span contains `cursor`, if any.
    fn wiki_link_target_at(value: &str, cursor: usize) -> Option<String> {
        let mut scan = 0;
        while let Some(relative_start) = value[scan..].find("[[") {
            let start = scan + relative_start;
            let content_start = start + 2;
            let relative_end = value[content_start..].find("]]")?;
            let end = content_start + relative_end + 2;
            if cursor >= start && cursor <= end {
                let inner = value[content_start..content_start + relative_end].trim();
                if inner.is_empty() {
                    return None;
                }
                let target = inner
                    .split_once('|')
                    .map(|(t, _)| t.trim())
                    .unwrap_or(inner);
                return (!target.is_empty()).then(|| target.to_string());
            }
            if end <= cursor {
                scan = end;
            } else {
                return None;
            }
        }
        None
    }

    pub(super) fn copy_note_as_markdown(&mut self, cx: &mut Context<Self>) {
        self.export_note(ExportFormat::Markdown, cx);
    }

    pub(super) fn copy_note_deeplink(&mut self, cx: &mut Context<Self>) {
        let Some((id, _)) = self.selected_note_for_action("copy_note_deeplink", cx) else {
            return;
        };
        let deeplink = self.note_deeplink(id);
        self.copy_text_to_clipboard(&deeplink);
    }

    pub(super) fn create_note_quicklink(&mut self, cx: &mut Context<Self>) {
        let Some((id, note)) = self.selected_note_for_action("create_note_quicklink", cx) else {
            return;
        };
        let title = if note.title.is_empty() {
            "Untitled Note".to_string()
        } else {
            note.title.clone()
        };
        let deeplink = self.note_deeplink(id);
        let quicklink = format!("[{}]({})", title, deeplink);
        self.copy_text_to_clipboard(&quicklink);
    }

    pub(super) fn copy_note_backlinks(&mut self, cx: &mut Context<Self>) {
        let Some((id, _)) = self.selected_note_for_action("copy_note_backlinks", cx) else {
            return;
        };

        match storage::get_note_backlinks(id) {
            Ok(backlinks) if backlinks.is_empty() => {
                self.copy_text_to_clipboard("No backlinks");
                self.show_action_feedback("No backlinks", false);
            }
            Ok(backlinks) => {
                let markdown = backlinks
                    .iter()
                    .map(|note| {
                        let title = if note.title.trim().is_empty() {
                            "Untitled Note"
                        } else {
                            note.title.trim()
                        };
                        format!("- [{}]({})", title, self.note_deeplink(note.id))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.copy_text_to_clipboard(&markdown);
                self.show_action_feedback("Copied backlinks", false);
            }
            Err(error) => {
                tracing::warn!(error = %error, note_id = %id, "Failed to copy note backlinks");
                self.show_action_feedback("Backlinks unavailable", true);
            }
        }

        cx.notify();
    }

    pub(super) fn duplicate_selected_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_id, note)) = self.selected_note_for_action("duplicate_selected_note", cx) else {
            return;
        };

        let content = note.content.clone();
        let duplicate = Note::with_content(content);
        if let Err(e) = storage::save_note(&duplicate) {
            tracing::error!(error = %e, "Failed to duplicate note");
            return;
        }

        self.notes.insert(0, duplicate.clone());
        self.show_action_feedback("Duplicated", false);
        self.select_note(duplicate.id, window, cx);
    }
}

fn activation_error_message(reason: &ActivationErrorReason) -> String {
    match reason {
        ActivationErrorReason::EmptyHref => "The link is empty.".to_string(),
        ActivationErrorReason::UnknownScheme { scheme } => {
            format!("`{scheme}` is not a supported link scheme.")
        }
        ActivationErrorReason::UnknownSpinePrefix { prefix, supported } => format!(
            "`{prefix}` is not a supported context type. Supported types: {}.",
            supported.join(", ")
        ),
        ActivationErrorReason::EmptySpineValue { prefix } => {
            format!("`{prefix}` context links need a value to search or open.")
        }
        ActivationErrorReason::MalformedUri { message } => message.clone(),
    }
}
