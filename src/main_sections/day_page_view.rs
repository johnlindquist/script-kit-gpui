// Day Page surface entry, render host, and footer helpers.

use chrono::Utc;

use crate::components::notes_editor::{NotesEditorLayout, NotesEditorMarkdownConfig};
use crate::footer_popup::{FooterAction, FooterButtonConfig};
use crate::notes::deeplink_activation::{
    Activation, ActivationErrorReason, ActivationSurface, resolve_activation,
    run_deeplink_confirm_options,
};
use script_kit_gpui::brain::{substrate::BrainSubstrate, wake_indexer};
use script_kit_gpui::day_page::normalize_day_page_markdown_references;
use script_kit_gpui::day_page::{
    DayPageBinding, DayPageSegment, parse_day_page_segments, resolve_fragment_path,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DayPageKitResourceSourceTarget {
    Note(crate::notes::NoteId),
}

#[derive(Debug, Clone)]
pub(crate) struct DayPageKitPreviewActionAvailability {
    pub(crate) can_add_to_agent_chat: bool,
    pub(crate) can_copy_uri: bool,
    pub(crate) open_source_target: Option<DayPageKitResourceSourceTarget>,
    pub(crate) open_source_unavailable_reason: Option<String>,
    pub(crate) can_close: bool,
}

fn day_page_kit_resource_source_target_for_uri(
    uri: &str,
) -> (Option<DayPageKitResourceSourceTarget>, Option<String>) {
    if uri == "kit://notes" || uri.starts_with("kit://notes?") {
        return (
            None,
            Some("Notes collection previews do not have a single source note.".to_string()),
        );
    }

    if uri.starts_with("kit://notes/") {
        let Some(note_id) = crate::notes::deeplink_activation::kit_note_source_id(uri) else {
            return (
                None,
                Some("This notes resource URI does not include a valid note id.".to_string()),
            );
        };
        if crate::notes::get_note(note_id).ok().flatten().is_some() {
            return (Some(DayPageKitResourceSourceTarget::Note(note_id)), None);
        }
        return (None, Some("The source note no longer exists.".to_string()));
    }

    if uri == "kit://scripts" {
        return (
            None,
            Some("Scripts collection previews do not have a single source file.".to_string()),
        );
    }

    if uri.starts_with("kit://clipboard-history") {
        return (
            None,
            Some(
                "Clipboard history previews do not have an editable source in this slice."
                    .to_string(),
            ),
        );
    }

    if uri.starts_with("kit://dictation-history") {
        return (
            None,
            Some(
                "Dictation history previews do not have an editable source in this slice."
                    .to_string(),
            ),
        );
    }

    (
        None,
        Some("This resource has no source opener in this slice.".to_string()),
    )
}

impl DayPageView {
    pub fn new(
        app: Entity<ScriptListApp>,
        substrate: BrainSubstrate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let metrics = crate::notes::window::style::adopted_metrics();
        let (editor_state, notes_editor) = NotesEditor::new_markdown_pair(
            window,
            cx,
            NotesEditorMarkdownConfig::new("")
                .placeholder("Today...")
                .layout(NotesEditorLayout::new(
                    metrics.editor_padding_x,
                    metrics.editor_padding_y,
                ))
                .rows(20),
        );

        // `subscribe_in` already runs the handler with this DayPageView leased
        // (`this` is `&mut Self`); re-leasing via `entity.update` here would
        // double-lease and panic the moment the editor emits a Change.
        let editor_subscription = cx.subscribe_in(
            &editor_state,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => this.on_editor_change(window, cx),
                InputEvent::SelectionChange => {
                    this.notes_editor
                        .update(cx, |editor, cx| editor.sync_markdown_link_highlights(cx));
                }
                _ => {}
            },
        );

        // Deeplink hover state changes via plain cx.notify on the InputState
        // (no InputEvent), so observe it or the hover hint chip won't track
        // the mouse live.
        let editor_hover_observation = cx.observe(&editor_state, |_, _, cx| cx.notify());

        Self {
            app: app.downgrade(),
            session: DayPageDocumentSession::new(substrate),
            notes_editor,
            editor_state,
            editor_subscription,
            editor_hover_observation,
            last_deeplink_hover_hint: None,
            focus_handle: cx.focus_handle(),
            fragment_open_targets: Vec::new(),
            spine_handoff: Default::default(),
            last_autosave: None,
            last_external_poll: None,
            autosave_flush_scheduled: false,
            note_switcher: crate::actions::CommandBar::new(
                Vec::new(),
                crate::actions::CommandBarConfig::notes_recent_style(),
                std::sync::Arc::new(crate::theme::get_cached_theme()),
            ),
            last_editor_content_len: 0,
            kit_resource_preview: None,
            clipboard_shelf: Vec::new(),
            clipboard_shelf_expanded: false,
            last_agent_chat_handoff_receipt: None,
            last_context_round_trip_receipt: None,
            read_mode: false,
        }
    }

    pub fn rebind_substrate(
        &mut self,
        substrate: BrainSubstrate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.session = DayPageDocumentSession::new(substrate);
        self.bind_today(window, cx);
        Ok(())
    }

    pub fn bind_today(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Utc::now();
        if let Err(error) = self.session.bind_today(now) {
            tracing::error!(error = %error, "Failed to bind today's day page");
            return;
        }
        self.apply_loaded_content_to_editor(window, cx);
    }

    /// Lift clipboard sediment refs out of `full` for editor display and
    /// refresh the shelf. Only the plain Day binding carries a shelf; notes
    /// and fragments show their content verbatim.
    fn adopt_clipboard_shelf_from(&mut self, full: &str) -> String {
        use script_kit_gpui::day_page::{DayPageBinding, split_day_page_clipboard_shelf};
        if !matches!(self.session.binding(), DayPageBinding::Day) {
            self.clipboard_shelf.clear();
            return full.to_string();
        }
        let (visible, items) = split_day_page_clipboard_shelf(full);
        self.clipboard_shelf = items
            .into_iter()
            .map(|item| {
                let preview = clipboard_shelf_preview_text(&item.entry_id);
                DayPageClipboardShelfEntry { item, preview }
            })
            .collect();
        visible
    }

    /// Editor content is the visible note body; the canonical day-file
    /// content rejoins the clipboard shelf (grouped at the end) so the day
    /// file remains the raw-free record of every kept clipboard entry.
    fn canonical_content_with_shelf(&self, visible: &str) -> String {
        use script_kit_gpui::day_page::join_day_page_clipboard_shelf;
        if self.clipboard_shelf.is_empty() {
            return visible.to_string();
        }
        let items: Vec<script_kit_gpui::day_page::ClipboardShelfItem> = self
            .clipboard_shelf
            .iter()
            .map(|entry| entry.item.clone())
            .collect();
        join_day_page_clipboard_shelf(visible, &items)
    }

    /// Editor-facing projection of canonical content (clipboard refs lifted
    /// out on the Day binding, verbatim otherwise). Read-only counterpart of
    /// `adopt_clipboard_shelf_from` for diffing against editor text.
    fn visible_content_of(&self, full: &str) -> String {
        use script_kit_gpui::day_page::{DayPageBinding, split_day_page_clipboard_shelf};
        if !matches!(self.session.binding(), DayPageBinding::Day) {
            return full.to_string();
        }
        split_day_page_clipboard_shelf(full).0
    }

    fn apply_loaded_content_to_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let full = self.session.disk_content().to_string();
        let content = self.adopt_clipboard_shelf_from(&full);
        self.reset_day_page_spine_handoff_state(true, true);
        self.kit_resource_preview = None;
        self.read_mode = false;
        self.refresh_fragment_open_targets(&content);
        self.spine_handoff.sync_with_markdown_references(&content);
        // Loads are not typing: pre-set the length so the Change event this
        // emits cannot read as growth and auto-swap to the main menu.
        self.last_editor_content_len = content.len();
        self.notes_editor.update(cx, |editor, cx| {
            editor.load_value_with_cursor_at_end(content, window, cx);
        });
        self.sync_footer(window, cx);
        self.defer_editor_bottom_scroll(window, cx);
        self.schedule_editor_bottom_scroll_retries(cx);
    }

    fn refresh_fragment_open_targets(&mut self, content: &str) {
        let Some(day_path) = self.session.path().cloned() else {
            self.fragment_open_targets.clear();
            return;
        };
        if self.session.is_viewing_fragment() {
            self.fragment_open_targets.clear();
            return;
        }

        self.fragment_open_targets = parse_day_page_segments(content)
            .into_iter()
            .filter_map(|segment| match segment {
                DayPageSegment::FragmentRef { relative_link, .. } => {
                    resolve_fragment_path(&day_path, &relative_link)
                }
                _ => None,
            })
            .collect();
    }

    pub fn open_fragment_at(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(fragment_path) = self.fragment_open_targets.get(index).cloned() else {
            return;
        };
        let now = Utc::now();
        if let Err(error) = self.session.bind_fragment(fragment_path, now) {
            tracing::error!(error = %error, "Failed to open fragment from day page");
            return;
        }
        self.apply_loaded_content_to_editor(window, cx);
        self.focus_editor(window, cx);
        cx.notify();
    }

    pub fn return_to_day_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Utc::now();
        if let Err(error) = self.session.return_to_day(now) {
            tracing::error!(error = %error, "Failed to return to day page from fragment");
            return;
        }
        self.apply_loaded_content_to_editor(window, cx);
        self.focus_editor(window, cx);
        cx.notify();
    }

    fn on_editor_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor
            .update(cx, |editor, cx| editor.sync_markdown_link_highlights(cx));
        // The session holds canonical content (with the clipboard shelf); the
        // editor holds the visible projection. Diff like against like.
        let previous = self.visible_content_of(self.session.disk_content());
        let mut content = self.notes_editor.read(cx).content(cx);
        let previous_len = self.last_editor_content_len;
        let selection = self.notes_editor.read(cx).selection(cx);
        self.last_editor_content_len = content.len();
        if should_normalize_day_page_references_after_edit(&content, previous_len, selection.end) {
            let normalized = normalize_day_page_markdown_references(&content);
            if normalized != content {
                let mut cursor = selection.end.min(content.len());
                while cursor > 0 && !content.is_char_boundary(cursor) {
                    cursor -= 1;
                }
                let normalized_cursor =
                    normalize_day_page_markdown_references(&content[..cursor]).len();
                self.last_editor_content_len = normalized.len();
                self.notes_editor.update(cx, |editor, cx| {
                    editor.set_value_preserving_scroll(
                        normalized.clone(),
                        normalized_cursor,
                        window,
                        cx,
                    );
                });
                content = normalized;
            }
        }
        if let Some((fixed, cursor)) =
            mention_atomic_delete_fixup(&previous, &content, &self.spine_handoff.mention_aliases)
                .or_else(|| {
                    day_page_context_reference_atomic_delete_fixup(
                        &previous,
                        &content,
                        &self.spine_handoff.mention_aliases,
                    )
                })
        {
            self.notes_editor.update(cx, |editor, cx| {
                editor.set_value_preserving_scroll(fixed.clone(), cursor, window, cx);
            });
            self.last_editor_content_len = fixed.len();
            let canonical = self.canonical_content_with_shelf(&fixed);
            self.session.apply_editor_content(&canonical);
            self.refresh_fragment_open_targets(&fixed);
            self.spine_handoff.sync_with_markdown_references(&fixed);
            self.poll_external_disk_changes(window, cx);
            self.schedule_autosave_flush(cx);
            self.sync_footer(window, cx);
            cx.notify();
            return;
        }
        let canonical = self.canonical_content_with_shelf(&content);
        self.session.apply_editor_content(&canonical);
        self.refresh_fragment_open_targets(&content);
        self.spine_handoff.sync_with_markdown_references(&content);
        self.poll_external_disk_changes(window, cx);
        self.schedule_autosave_flush(cx);
        self.sync_footer(window, cx);
        self.maybe_begin_day_page_context_round_trip_from_edit(previous_len, &content, window, cx);
        cx.notify();
    }

    /// Notes-parity autosave: same debounce interval as the Notes window
    /// (`NotesApp::SAVE_DEBOUNCE_MS`), driven from render side effects the
    /// same way `NotesApp::process_render_side_effects` drives
    /// `save_current_note`. A trailing flush timer (scheduled in
    /// `on_editor_change`) guarantees the final keystroke also lands on disk
    /// so the footer dirty state always converges to the real disk state.
    const SAVE_DEBOUNCE_MS: u64 = 300;

    /// When the Day Page is left open across local midnight, the bound file
    /// still points at yesterday, so new typing would autosave into the wrong
    /// day. On the next render side effect, flush the pre-midnight buffer to
    /// YESTERDAY's file (via `save`, which writes the still-bound old path),
    /// then rebind to today and reload the editor. Guarded to the today-
    /// following `Day` binding by `day_has_rolled`, so an open note, fragment,
    /// or explicitly-opened past day is never dragged onto the new day.
    fn maybe_rebind_after_midnight(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.session.day_has_rolled(Utc::now()) {
            return;
        }
        // `save` reads the editor buffer, applies it to the session, and writes
        // it to the currently-bound (yesterday's) path; a no-op when unchanged.
        self.save(cx);
        // `bind_today` rebinds to the new day and loads its content into the
        // editor via `apply_loaded_content_to_editor`.
        self.bind_today(window, cx);
        tracing::info!(
            target: "script_kit::day_page",
            event = "day_page_rebound_after_midnight",
        );
    }

    fn maybe_autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.session.is_dirty() {
            return;
        }
        let due = self.last_autosave.map_or(true, |at| {
            at.elapsed() >= std::time::Duration::from_millis(Self::SAVE_DEBOUNCE_MS)
        });
        if !due {
            return;
        }
        self.last_autosave = Some(std::time::Instant::now());
        if self.save(cx) {
            tracing::debug!(
                target: "script_kit::day_page",
                event = "day_page_autosaved",
            );
        }
        self.sync_footer(window, cx);
    }

    fn schedule_autosave_flush(&mut self, cx: &mut Context<Self>) {
        if self.autosave_flush_scheduled {
            return;
        }
        self.autosave_flush_scheduled = true;
        let flush_delay = std::time::Duration::from_millis(Self::SAVE_DEBOUNCE_MS + 50);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(flush_delay).await;
            this.update(cx, |this, cx| {
                this.autosave_flush_scheduled = false;
                // Render side effects run the actual save; notify forces a
                // render even when no further input arrives.
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn poll_external_disk_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Throttle the filesystem stat: `render` calls this every frame, but a
        // background append (`;todo`, clipboard sediment, dictation) doesn't need
        // sub-250ms detection. Without this, a long day page does an
        // `fs::metadata` syscall on every single frame.
        let now = std::time::Instant::now();
        if let Some(last) = self.last_external_poll {
            if now.duration_since(last) < std::time::Duration::from_millis(250) {
                return;
            }
        }
        self.last_external_poll = Some(now);
        if let Ok(Some(content)) = self.session.maybe_refresh_from_disk() {
            let content = self.adopt_clipboard_shelf_from(&content);
            self.reset_day_page_spine_handoff_state(true, true);
            self.refresh_fragment_open_targets(&content);
            // External refresh is not typing: keep the growth detector quiet.
            self.last_editor_content_len = content.len();
            self.notes_editor.update(cx, |editor, cx| {
                editor.set_value(content, window, cx);
            });
            cx.notify();
        }
    }

    pub fn save(&mut self, cx: &mut Context<Self>) -> bool {
        let content = self.notes_editor.read(cx).content(cx);
        let content = self.canonical_content_with_shelf(&content);
        self.session.apply_editor_content(&content);
        match self.session.save_content(&content, Utc::now()) {
            Ok(()) => {
                wake_indexer();
                true
            }
            Err(error) => {
                tracing::error!(error = %error, "Failed to save day page");
                false
            }
        }
    }

    pub fn save_and_sync_footer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let saved = self.save(cx);
        self.sync_footer(window, cx);
        cx.notify();
        saved
    }

    pub fn is_dirty(&self) -> bool {
        self.session.is_dirty()
    }

    pub(crate) fn automation_input_value(&self, cx: &App) -> String {
        self.notes_editor.read(cx).content(cx)
    }

    pub(crate) fn automation_state(&self, cx: &App) -> serde_json::Value {
        let input = self.automation_input_value(cx);
        let task_stats = day_page_task_stats(&input);
        let preview_anchor = self.automation_preview_anchor(&input, cx);
        let kit_resource_preview = match self.kit_resource_preview.as_ref() {
            Some(preview) => {
                let availability = self
                    .kit_resource_preview_action_availability()
                    .expect("preview action availability exists when preview is open");
                serde_json::json!({
                    "schemaVersion": 1,
                    "active": true,
                    "redacted": true,
                    "title": preview.title,
                    "uri": preview.uri,
                    "mimeType": preview.mime_type,
                    "readOnly": true,
                    "truncated": preview.truncated,
                    "textLength": preview.text.chars().count(),
                    "actionAvailability": {
                        "addToAgentChat": availability.can_add_to_agent_chat,
                        "copyUri": availability.can_copy_uri,
                        "openSource": availability.open_source_target.is_some(),
                        "openSourceReason": availability.open_source_unavailable_reason,
                        "closePreview": availability.can_close,
                    },
                })
            }
            None => serde_json::json!({
                "schemaVersion": 1,
                "active": false,
                "redacted": true,
            }),
        };

        serde_json::json!({
            "schemaVersion": 1,
            "redacted": true,
            "inputLength": input.chars().count(),
            "readMode": self.read_mode,
            "mode": if self.kit_resource_preview.is_some() {
                "kitResourcePreview"
            } else if self.read_mode {
                "read"
            } else {
                "edit"
            },
            "previewAnchor": preview_anchor,
            "taskStats": task_stats,
            "contextReferenceLedger": self.spine_handoff.ledger_state(&input),
            "lastContextRoundTripReceipt": self.last_context_round_trip_receipt.clone(),
            "kitResourcePreview": kit_resource_preview,
            "deeplinkHoverHint": self.last_deeplink_hover_hint.clone(),
            "lastAgentChatHandoffReceipt": self.last_agent_chat_handoff_receipt.clone(),
        })
    }

    fn automation_preview_anchor(&self, input: &str, cx: &App) -> serde_json::Value {
        let preview_available = self.read_mode && self.kit_resource_preview.is_none();
        let scroll = if preview_available {
            let editor = self.notes_editor.read(cx);
            automation_scroll_handle_metrics(
                editor.preview_scroll_handle(),
                "runtime.components.notes_editor.preview.ScrollHandle",
            )
        } else {
            serde_json::Value::Null
        };

        serde_json::json!({
            "schemaVersion": 1,
            "source": "runtime.dayPage.automationState",
            "redacted": true,
            "available": preview_available,
            "previewEnabled": preview_available,
            "owner": crate::components::notes_editor::NOTES_EDITOR_STYLE_OWNER,
            "renderPath": crate::components::notes_editor::NOTES_EDITOR_PREVIEW_RENDER_PATH,
            "scrollSource": "runtime.components.notes_editor.preview.ScrollHandle",
            "scrollMetricsAvailable": preview_available,
            "scroll": scroll,
            "inputLength": input.chars().count(),
            "stopReason": if preview_available {
                serde_json::Value::Null
            } else if self.kit_resource_preview.is_some() {
                serde_json::Value::String("Day Page kit resource preview is mounted".to_string())
            } else {
                serde_json::Value::String("Day Page read mode is not enabled".to_string())
            },
        })
    }

    pub fn focus_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_mode {
            self.focus_handle.focus(window, cx);
            return;
        }
        self.focus_editor_at_end(window, cx);
        self.defer_editor_bottom_scroll(window, cx);
        self.schedule_editor_bottom_scroll_retries(cx);
        let day_page = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let Some(day_page) = day_page.upgrade() else {
                return;
            };
            day_page.update(cx, |view, cx| {
                view.focus_editor_at_end(window, cx);
            });
        });
    }

    fn focus_editor_at_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor.update(cx, |editor, cx| {
            editor.focus_with_cursor_at_end(window, cx);
        });
    }

    fn defer_editor_bottom_scroll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let day_page = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let Some(day_page) = day_page.upgrade() else {
                return;
            };
            day_page.update(cx, |view, cx| {
                view.notes_editor.update(cx, |editor, cx| {
                    editor.scroll_to_bottom(cx);
                });
                let day_page = cx.entity().downgrade();
                window.defer(cx, move |_window, cx| {
                    let Some(day_page) = day_page.upgrade() else {
                        return;
                    };
                    day_page.update(cx, |view, cx| {
                        view.notes_editor.update(cx, |editor, cx| {
                            editor.scroll_to_bottom(cx);
                        });
                    });
                });
            });
        });
    }

    fn schedule_editor_bottom_scroll_retries(&mut self, cx: &mut Context<Self>) {
        // The retries exist to force the bottom once late layout passes have
        // committed real bounds. They must stop the moment the user edits or
        // moves the cursor, otherwise a keystroke inside the retry window
        // gets its viewport yanked to the bottom up to 800ms later. Content
        // length + selection captured here identify "still the untouched,
        // freshly loaded buffer"; any edit or cursor move changes one of them.
        let anchor_len = self.notes_editor.read(cx).content(cx).len();
        let anchor_selection = self.notes_editor.read(cx).selection(cx);
        cx.spawn(async move |this, cx| {
            for delay_ms in [50_u64, 150, 350, 800] {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(delay_ms))
                    .await;
                let untouched = this
                    .update(cx, |view, cx| {
                        let len = view.notes_editor.read(cx).content(cx).len();
                        let selection = view.notes_editor.read(cx).selection(cx);
                        if len != anchor_len || selection != anchor_selection {
                            return false;
                        }
                        view.notes_editor.update(cx, |editor, cx| {
                            editor.scroll_to_bottom(cx);
                        });
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !untouched {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn set_input(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.kit_resource_preview = None;
        self.read_mode = false;
        self.notes_editor.update(cx, |editor, cx| {
            editor.set_value_with_cursor_at_end(text.clone(), window, cx);
        });
        let canonical = self.canonical_content_with_shelf(&text);
        self.session.apply_editor_content(&canonical);
        self.refresh_fragment_open_targets(&text);
        self.reset_day_page_spine_handoff_state(false, true);
        self.spine_handoff.sync_with_markdown_references(&text);
        self.sync_footer(window, cx);
        cx.notify();
    }

    fn reset_day_page_spine_handoff_state(&mut self, clear_cwd_anchor: bool, clear_mentions: bool) {
        self.spine_handoff.reset(clear_cwd_anchor, clear_mentions);
    }

    fn sync_footer(&self, window: &mut Window, cx: &mut Context<Self>) {
        // Deferred on purpose: several callers run while the ScriptListApp
        // lease is already held (e.g. `show_day_page_view` → `bind_today`
        // inside `app_entity.update`, including the hotkey gesture path).
        // A synchronous `app.update` here double-leases ScriptListApp and
        // panics ("cannot update ... while it is already being updated").
        // `window.defer` (not `cx.defer_in`) — the deferred closure must hold
        // NO entity lease, because `sync_main_footer_popup` reads this
        // DayPageView back (`is_dirty` for the footer save button).
        let Some(app) = self.app.upgrade() else {
            return;
        };
        window.defer(cx, move |window, cx| {
            app.update(cx, |app, cx| {
                app.sync_main_footer_popup(window, cx);
            });
        });
    }
}

fn should_normalize_day_page_references_after_edit(
    content: &str,
    previous_len: usize,
    cursor: usize,
) -> bool {
    if content.len() <= previous_len {
        return false;
    }
    let growth = content.len().saturating_sub(previous_len);
    if growth > 1 {
        return true;
    }
    let mut cursor = cursor.min(content.len());
    while cursor > 0 && !content.is_char_boundary(cursor) {
        cursor -= 1;
    }
    content[..cursor]
        .chars()
        .next_back()
        .is_some_and(char::is_whitespace)
}

fn automation_scroll_handle_metrics(
    handle: &gpui::ScrollHandle,
    source: &'static str,
) -> serde_json::Value {
    let offset = handle.offset();
    let max_offset = handle.max_offset();
    let viewport = handle.bounds().size;
    let max_scroll_top = max_offset.y.as_f32().max(0.0);
    let max_scroll_left = max_offset.x.as_f32().max(0.0);
    let scroll_top = (-offset.y.as_f32()).clamp(0.0, max_scroll_top);
    let scroll_left = (-offset.x.as_f32()).clamp(0.0, max_scroll_left);

    serde_json::json!({
        "schemaVersion": 1,
        "source": source,
        "available": true,
        "offsetUnit": "logicalPx",
        "scrollTop": scroll_top,
        "scrollLeft": scroll_left,
        "rawOffsetX": offset.x.as_f32(),
        "rawOffsetY": offset.y.as_f32(),
        "scrollHeight": viewport.height.as_f32() + max_scroll_top,
        "scrollWidth": viewport.width.as_f32() + max_scroll_left,
        "clientHeight": viewport.height.as_f32(),
        "clientWidth": viewport.width.as_f32(),
        "maxScrollTop": max_scroll_top,
        "maxScrollLeft": max_scroll_left,
        "canScrollY": max_scroll_top > 0.0,
        "canScrollX": max_scroll_left > 0.0,
    })
}

fn day_page_task_stats(content: &str) -> serde_json::Value {
    let mut total = 0_usize;
    let mut checked = 0_usize;
    let mut unchecked = 0_usize;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("- [ ] ") {
            total += 1;
            unchecked += 1;
        } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
            total += 1;
            checked += 1;
        }
    }

    serde_json::json!({
        "schemaVersion": 1,
        "total": total,
        "checked": checked,
        "unchecked": unchecked,
    })
}

impl Focusable for DayPageView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DayPageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.poll_external_disk_changes(window, cx);
        self.maybe_rebind_after_midnight(window, cx);
        self.maybe_autosave(window, cx);

        let app = self.app.upgrade().expect("DayPageView app entity dropped");

        let app_state = app.read(cx);
        let menu_def = app_state.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let search = menu_def.search;
        let tokens = get_tokens(app_state.current_design);
        let design_visual = tokens.visual();
        let is_default_design = app_state.current_design.is_default();
        let design_spacing = tokens.spacing();
        let text_primary = app_state.theme.colors.text.primary;
        let font_family = app_state.theme_font_family();

        let header_padding_x = shell.header_padding_x;
        let header_padding_y = if is_default_design {
            shell.header_padding_y
        } else {
            design_spacing.padding_sm
        };
        let header_gap = if is_default_design {
            shell.header_gap
        } else {
            design_spacing.gap_md
        };

        let columns = crate::components::main_view_chrome::main_view_content_columns(menu_def);
        let editor_layout = self.notes_editor.read(cx).layout();
        let viewport_height = window.viewport_size().height.as_f32();
        // Day Page renders the shared context row with an intentionally empty
        // input slot. Do not reserve a phantom search-input height here.
        let header_height =
            header_padding_y * 2.0 + menu_def.header_info_bar.height_px + header_gap;
        let footer_height = crate::components::footer_chrome::current_main_menu_footer_height();
        let shelf_count = if self.kit_resource_preview.is_none() {
            self.clipboard_shelf.len()
        } else {
            0
        };
        let layout_budget = day_page_layout_budget(
            viewport_height,
            header_height,
            footer_height,
            shelf_count,
            self.clipboard_shelf_expanded,
            editor_layout.padding_y,
        );
        let editor_input = self.notes_editor.read(cx).render_input(cx);
        // Hover discoverability: names the click action while the mouse is
        // over a deeplink (the vendored input paints underline + pointer
        // cursor). Absolute overlay — never reflows the editor. The receipt
        // records what this render actually built (only meaningful in edit
        // mode; preview/read branches below don't mount the editor).
        let hover_hint_model = if self.kit_resource_preview.is_some() || self.read_mode {
            None
        } else {
            crate::notes::deeplink_activation::hover_hint_model(
                self.notes_editor.read(cx).hovered_deeplink(cx),
                crate::notes::deeplink_activation::ActivationSurface::DayPage,
            )
        };
        self.last_deeplink_hover_hint = hover_hint_model
            .as_ref()
            .map(|(verb, href)| serde_json::json!({ "verb": verb, "href": href }));
        let deeplink_hover_hint = hover_hint_model.map(|(verb, href)| {
            crate::components::resource_preview::render_deeplink_hover_hint(
                "day-page-deeplink-hover-hint",
                verb,
                &href,
                cx,
            )
        });
        let editor_input = div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .h_full()
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                    this.activate_deeplink_from_mouse_up(event.clone(), window, cx);
                }),
            )
            .child(editor_input);
        let viewing_fragment = self.session.is_viewing_fragment();
        let theme = app_state.theme.clone();

        let local_today = Utc::now()
            .with_timezone(&self.session.substrate().timezone())
            .date_naive();
        let viewing_past_day = !viewing_fragment
            && self
                .session
                .bound_date()
                .is_some_and(|date| date != local_today);

        let back_bar = if viewing_fragment {
            let label = match self.session.binding() {
                DayPageBinding::Fragment {
                    return_day_date, ..
                } => {
                    format!("Today · {return_day_date}")
                }
                DayPageBinding::Day => "Today".to_string(),
                DayPageBinding::Note { title, .. } => title.clone(),
            };
            Some(crate::components::render_back_affordance(
                script_kit_gpui::day_page::FRAGMENT_BACK_ID.into(),
                label.into(),
                &theme,
                cx.listener(|this, _, window, cx| {
                    this.return_to_day_page(window, cx);
                }),
            ))
        } else if viewing_past_day {
            let label = self
                .session
                .bound_date()
                .map(|date| format!("Back to Today · viewing {date}"))
                .unwrap_or_else(|| "Back to Today".to_string());
            Some(crate::components::render_back_affordance(
                "day-page-past-day-back".into(),
                label.into(),
                &theme,
                cx.listener(|this, _, window, cx| {
                    this.bind_today(window, cx);
                    this.focus_editor(window, cx);
                }),
            ))
        } else if self.session.is_viewing_note() {
            let label = self
                .session
                .viewing_note_title()
                .map(|title| format!("Back to Today · viewing {title}"))
                .unwrap_or_else(|| "Back to Today".to_string());
            Some(crate::components::render_back_affordance(
                "day-page-note-back".into(),
                label.into(),
                &theme,
                cx.listener(|this, _, window, cx| {
                    this.return_to_day_page(window, cx);
                    this.focus_editor(window, cx);
                }),
            ))
        } else {
            None
        };

        let editor_content = if self.kit_resource_preview.is_some() {
            self.render_kit_resource_preview(cx)
        } else if self.read_mode {
            self.render_day_page_read_mode(cx)
        } else {
            div()
                .relative()
                .flex_1()
                .min_h(px(0.))
                .child(editor_input)
                .when_some(deeplink_hover_hint, |d, chip| d.child(chip))
                .into_any_element()
        };

        let clipboard_shelf = self
            .render_clipboard_shelf(layout_budget.shelf_list_height, cx)
            .map(|shelf| self.notes_editor.read(cx).render_content_accessory(shelf));

        let editor_body = div()
            .id(DAY_PAGE_EDITOR_ID)
            .flex_1()
            .min_h(px(0.))
            .h_full()
            // Symmetric content padding matching the notes/markdown editors,
            // rather than the launcher's list-text column inset
            // (`input_text_inset_left`) which pushed the day-page prose far to
            // the right and looked inconsistent with every other markdown view.
            .pl(px(columns.content_right_inset_x))
            .pr(px(columns.content_right_inset_x))
            .flex()
            .flex_col()
            .when_some(back_bar, |parent, bar| parent.child(bar))
            .child(
                div()
                    .flex_1()
                    .min_h(px(DAY_PAGE_MIN_EDITOR_HEIGHT_PX))
                    .child(editor_content),
            )
            .when_some(clipboard_shelf, |parent, shelf| parent.child(shelf));

        let context_zone = app.update(cx, |app, _cx| {
            app.render_inert_main_view_context_zone(menu_def)
        });

        let main = div()
            .flex()
            .flex_col()
            .h_full()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .min_h(px(search.height))
                    .flex_1()
                    .min_h(px(0.))
                    .child(editor_body),
            )
            .into_any_element();

        let header = crate::components::main_view_chrome::MainViewHeaderChrome {
            context: Some(context_zone),
            input: div().into_any_element(),
            padding_x: header_padding_x,
            padding_y: header_padding_y,
            gap: header_gap,
        };

        let divider = crate::components::main_view_chrome::MainViewDividerChrome {
            margin_x: shell.divider_margin_x,
            height: if is_default_design {
                shell.divider_height
            } else {
                design_visual.border_thin
            },
            visible: false,
        };

        let root = crate::components::main_view_chrome::render_main_view_shell()
            .text_color(rgb(text_primary))
            .font_family(font_family)
            .key_context("day_page")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }));

        crate::components::main_view_chrome::render_main_view_chrome(
            root,
            &theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header,
                divider,
                main,
                footer: Some(
                    crate::components::prompt_layout_shell::render_native_main_window_footer_spacer(
                    ),
                ),
                overlays: Vec::new(),
            },
        )
    }
}

impl DayPageView {
    pub(crate) fn execute_day_page_action_from_preview(
        &mut self,
        action_id: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        window.defer(cx, move |window, cx| {
            let _ = app.update(cx, |app, cx| {
                app.execute_day_page_action(action_id, window, cx);
            });
        });
    }

    fn render_kit_resource_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::components::resource_preview::{
            ResourcePreviewSurface, render_resource_preview,
        };

        let Some(preview) = self.kit_resource_preview.as_ref() else {
            return div().into_any_element();
        };

        // The main window owns a native footer, so preview actions live there
        // (see `day_page_footer_buttons`) instead of in-body link rows — same
        // footer language as every other main-window surface.
        render_resource_preview(
            ResourcePreviewSurface {
                id_prefix: "day-page-kit-resource-preview",
                title: preview.title.clone().into(),
                uri: preview.uri.clone().into(),
                mime_type: preview.mime_type.clone().into(),
                text: preview.text.clone().into(),
                truncated: preview.truncated,
                // Match the editor's own text inset so preview content aligns
                // with Day Page prose instead of hugging the window edge.
                inset_x: crate::notes::window::style::adopted_metrics().editor_padding_x,
                actions: Vec::new(),
                footer_hints: Vec::new(),
            },
            cx,
        )
    }

    /// Compact clipboard shelf under the editor. Kept clipboard entries stay
    /// part of the day note (rejoined into the file on save) but read as a
    /// quiet, collapsible strip instead of raw `[Clipboard entry](kit://…)`
    /// lines inside the prose. Rows open the shared kit:// resource preview.
    fn render_clipboard_shelf(
        &self,
        list_height: f32,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        use gpui_component::theme::ActiveTheme as _;

        if self.clipboard_shelf.is_empty() || self.kit_resource_preview.is_some() {
            return None;
        }
        let expanded = self.clipboard_shelf_expanded;
        let count = self.clipboard_shelf.len();
        let muted = cx.theme().muted_foreground;
        let hover_fg = cx.theme().foreground;

        let header = div()
            .id("day-page-clipboard-shelf-toggle")
            .flex()
            .items_center()
            .h(px(DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX))
            .flex_none()
            .gap_1()
            .text_xs()
            .text_color(muted)
            .cursor_pointer()
            .hover(move |style| style.text_color(hover_fg))
            .on_click(cx.listener(|this, _, _window, cx| {
                this.clipboard_shelf_expanded = !this.clipboard_shelf_expanded;
                cx.notify();
            }))
            .child(if expanded { "▾" } else { "▸" })
            .child(format!(
                "Clipboard · {count} kept {}",
                if count == 1 { "entry" } else { "entries" }
            ));

        let mut shelf = div()
            .id("day-page-clipboard-shelf")
            .flex()
            .flex_col()
            .gap(px(DAY_PAGE_CLIPBOARD_SHELF_GAP_PX))
            .pt(px(DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX))
            .child(header);

        if expanded && list_height > 0.0 {
            let mut list = div()
                .id("day-page-clipboard-shelf-list")
                .flex()
                .flex_col()
                .h(px(list_height))
                .max_h(px(list_height))
                .overflow_y_scroll();
            for (index, entry) in self.clipboard_shelf.iter().enumerate() {
                let uri = crate::clipboard_history::entry_resource_uri(&entry.item.entry_id);
                list = list.child(
                    div()
                        .h(px(DAY_PAGE_CLIPBOARD_SHELF_ROW_HEIGHT_PX))
                        .flex_none()
                        .child(
                            crate::components::resource_preview::render_compact_resource_row(
                                crate::components::resource_preview::CompactResourceRow {
                                    id: gpui::SharedString::from(format!(
                                        "day-page-clipboard-shelf-item-{index}"
                                    )),
                                    meta: entry.item.timestamp.clone().into(),
                                    preview: entry.preview.clone().into(),
                                },
                                cx,
                                cx.listener(move |this, _, window, cx| {
                                    this.open_kit_resource_preview(uri.clone(), true, window, cx);
                                }),
                            ),
                        ),
                );
            }
            shelf = shelf.child(list);
        }

        Some(shelf.into_any_element())
    }

    fn render_day_page_read_mode(&self, cx: &mut Context<Self>) -> AnyElement {
        let content = self.notes_editor.read(cx).content(cx);
        let day_page = cx.entity().downgrade();
        let on_toggle_task: crate::notes::markdown::TaskToggleHandler =
            std::rc::Rc::new(move |marker_range, checked, window, cx| {
                if let Some(day_page) = day_page.upgrade() {
                    day_page.update(cx, |view, cx| {
                        view.toggle_task_marker_from_read_mode(marker_range, checked, window, cx);
                    });
                }
            });

        self.notes_editor
            .read(cx)
            .render_preview(&content, on_toggle_task, cx.theme())
    }

    pub(crate) fn toggle_read_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.kit_resource_preview.is_some() {
            return;
        }
        self.read_mode = !self.read_mode;
        if self.read_mode {
            self.focus_handle.focus(window, cx);
        } else {
            self.focus_editor_at_end(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn toggle_task_marker_from_read_mode(
        &mut self,
        marker_range: std::ops::Range<usize>,
        checked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.kit_resource_preview.is_some() {
            return false;
        }
        let toggled = self.notes_editor.update(cx, |editor, cx| {
            editor.toggle_task_marker_at(marker_range, checked, window, cx)
        });
        if toggled {
            let content = self.notes_editor.read(cx).content(cx);
            self.last_editor_content_len = content.len();
            let canonical = self.canonical_content_with_shelf(&content);
            self.session.apply_editor_content(&canonical);
            self.refresh_fragment_open_targets(&content);
            self.spine_handoff.sync_with_markdown_references(&content);
            self.schedule_autosave_flush(cx);
            self.sync_footer(window, cx);
            tracing::info!(
                target: "script_kit::day_page",
                event = "day_page_read_mode_task_toggled",
            );
            cx.notify();
        }
        toggled
    }

    /// Where closing the kit resource preview returns to. Escape/Close never
    /// closes the window from a preview — it restores the editor underneath —
    /// so the affordance label must say "Back to …", not "Close".
    pub(crate) fn kit_resource_preview_return_label(&self) -> &'static str {
        if self.session.is_viewing_note() {
            return "Back to Note";
        }
        if self.session.is_viewing_fragment() {
            return "Back to Fragment";
        }
        let today = Utc::now()
            .with_timezone(&self.session.substrate().timezone())
            .date_naive();
        if self.session.bound_date().is_some_and(|date| date != today) {
            "Back to Day"
        } else {
            "Back to Today"
        }
    }

    pub(crate) fn kit_resource_preview_action_availability(
        &self,
    ) -> Option<DayPageKitPreviewActionAvailability> {
        let preview = self.kit_resource_preview.as_ref()?;
        let (open_source_target, open_source_unavailable_reason) =
            day_page_kit_resource_source_target_for_uri(&preview.uri);
        Some(DayPageKitPreviewActionAvailability {
            can_add_to_agent_chat: preview.allow_agent_chat_action,
            can_copy_uri: true,
            open_source_target,
            open_source_unavailable_reason,
            can_close: true,
        })
    }

    pub(crate) fn open_kit_resource_preview_source(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(availability) = self.kit_resource_preview_action_availability() else {
            return false;
        };
        match availability.open_source_target {
            Some(DayPageKitResourceSourceTarget::Note(note_id)) => {
                let Ok(Some(note)) = crate::notes::get_note(note_id) else {
                    return false;
                };
                let path = crate::notes::note_file_path(note.id).ok().flatten();
                if let Err(error) = self.session.bind_note_content(
                    note.id.as_str().to_string(),
                    note.title.clone(),
                    note.content.clone(),
                    path,
                    Utc::now(),
                ) {
                    tracing::error!(
                        target: "script_kit::day_page",
                        error = %error,
                        "day_page_kit_preview_open_source_failed"
                    );
                    return false;
                }
                self.apply_loaded_content_to_editor(window, cx);
                self.focus_editor(window, cx);
                cx.notify();
                true
            }
            None => false,
        }
    }

    fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.to_lowercase();
        self.handle_key_parts(
            &key,
            event.keystroke.modifiers.platform,
            event.keystroke.modifiers.shift,
            event.keystroke.modifiers.alt,
            event.keystroke.modifiers.control,
            window,
            cx,
        );
    }

    pub(crate) fn handle_key_parts(
        &mut self,
        key: &str,
        cmd: bool,
        shift: bool,
        alt: bool,
        control: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let exact_plain = !cmd && !shift && !alt && !control;
        let exact_cmd = cmd && !shift && !alt && !control;

        if exact_plain
            && crate::ui_foundation::is_key_escape(&key)
            && crate::confirm::is_confirm_window_open()
        {
            crate::confirm::route_key_to_confirm_popup("escape", cx);
            return;
        }

        if exact_plain
            && crate::ui_foundation::is_key_escape(&key)
            && self.kit_resource_preview.is_some()
        {
            self.close_kit_resource_preview(window, cx);
            return;
        }

        // Kit resource preview keyboard contract: the preview replaces the
        // editor, so its power-user keys must win before any editor handling
        // (but never over an open confirm popup). ⌘C copies the resource URI,
        // ↵ opens the editable source (when one exists), ⌘↵ stages the
        // resource in Agent Chat. These mirror the clickable footer hints
        // rendered by the shared preview component.
        if self.kit_resource_preview.is_some() && !crate::confirm::is_confirm_window_open() {
            if exact_cmd && key == "c" {
                self.execute_day_page_action_from_preview(
                    crate::DAY_PAGE_PREVIEW_COPY_URI_ACTION_ID,
                    window,
                    cx,
                );
                return;
            }
            if exact_plain && crate::ui_foundation::is_key_enter(&key) {
                // Swallow Enter even without a source target so it cannot
                // leak into the hidden editor behind the preview.
                self.execute_day_page_action_from_preview(
                    crate::DAY_PAGE_PREVIEW_OPEN_SOURCE_ACTION_ID,
                    window,
                    cx,
                );
                return;
            }
            if exact_cmd
                && crate::ui_foundation::is_key_enter(&key)
                && self
                    .kit_resource_preview_action_availability()
                    .is_some_and(|availability| availability.can_add_to_agent_chat)
            {
                self.execute_day_page_action_from_preview(
                    crate::DAY_PAGE_PREVIEW_ADD_TO_AGENT_CHAT_ACTION_ID,
                    window,
                    cx,
                );
                return;
            }
        }

        if self.is_day_switcher_open() {
            if self.handle_day_switcher_key(key, cmd, shift, alt, control, window, cx) {
                return;
            }
        }

        if exact_plain && crate::ui_foundation::is_key_escape(&key) {
            if self.session.is_viewing_fragment() || self.session.is_viewing_note() {
                self.return_to_day_page(window, cx);
                return;
            }
            // Escape from a past day returns to today before closing the
            // window, keeping the dismissal ladder predictable.
            let today = Utc::now()
                .with_timezone(&self.session.substrate().timezone())
                .date_naive();
            if self.session.bound_date().is_some_and(|date| date != today) {
                self.bind_today(window, cx);
                self.focus_editor(window, cx);
                return;
            }
            if let Some(app) = self.app.upgrade() {
                window.defer(cx, move |_window, cx| {
                    app.update(cx, |app, cx| {
                        app.close_and_reset_window(cx);
                    });
                });
            }
            return;
        }

        if exact_cmd && key == "s" {
            self.save_and_sync_footer(window, cx);
            return;
        }

        if exact_cmd && key == "p" {
            self.open_note_switcher(window, cx);
            return;
        }

        if exact_cmd && key == "." {
            self.activate_deeplink_under_cursor(window, cx);
            return;
        }

        if exact_cmd && (key == "enter" || key == "return") {
            self.open_agent_chat_about_current_line(window, cx);
            return;
        }

        // Markdown formatting shortcuts — same bindings as the Notes window
        // (`src/notes/window/keyboard.rs`), routed through the shared
        // NotesEditor toolbar action executor.
        if exact_cmd && key == "b" {
            self.run_shared_markdown_toolbar_action("bold", window, cx);
            return;
        }
        if exact_cmd && key == "i" {
            self.run_shared_markdown_toolbar_action("italic", window, cx);
            return;
        }
        if exact_cmd && key == "e" {
            self.run_shared_markdown_toolbar_action("code", window, cx);
            return;
        }
        if cmd && shift && !alt && !control && key == "x" {
            self.run_shared_markdown_toolbar_action("strikethrough", window, cx);
        }
    }

    fn activate_deeplink_under_cursor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let href = self.notes_editor.read(cx).activation_href_at_cursor(cx);
        let Some(href) = href else {
            return false;
        };
        let activation = resolve_activation(&href, ActivationSurface::DayPage);
        self.handle_deeplink_activation(activation, window, cx);
        true
    }

    fn activate_deeplink_from_mouse_up(
        &mut self,
        event: gpui::MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let day_page = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let Some(day_page) = day_page.upgrade() else {
                return;
            };
            day_page.update(cx, |this, cx| {
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
                self.open_deeplink_info_dialog(
                    "Can't open this link",
                    format!(
                        "{}\n\n{}",
                        error.raw_href,
                        day_page_activation_error_message(&error.reason)
                    ),
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
            Activation::KitResourcePreview {
                uri,
                allow_agent_chat_action,
            } => {
                self.open_kit_resource_preview(uri, allow_agent_chat_action, window, cx);
            }
        }
    }

    fn open_kit_resource_preview(
        &mut self,
        uri: String,
        allow_agent_chat_action: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match crate::notes::deeplink_activation::read_cheap_kit_resource_preview(&uri) {
            Ok(preview) => {
                tracing::info!(
                    event = "day_page_deeplink_kit_resource_preview_opened",
                    uri = %preview.uri,
                    mime_type = %preview.mime_type,
                    truncated = preview.truncated,
                );
                self.read_mode = false;
                self.kit_resource_preview = Some(DayPageKitResourcePreviewState::from_preview(
                    preview,
                    allow_agent_chat_action,
                ));
                self.focus_handle.focus(window, cx);
                self.sync_footer(window, cx);
                cx.notify();
            }
            Err(error) => self.open_deeplink_info_dialog(
                "Can't open this link",
                format!("{uri}\n\n{error}"),
                uri,
                window,
                cx,
            ),
        }
    }

    fn close_kit_resource_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.kit_resource_preview.take().is_some() {
            self.focus_editor(window, cx);
            self.sync_footer(window, cx);
            cx.notify();
        }
    }

    fn open_external_deeplink_url(
        &mut self,
        href: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match open::that(&href) {
            Ok(()) => {
                tracing::info!(event = "day_page_deeplink_url_opened", href = %href);
                self.sync_footer(window, cx);
            }
            Err(error) => self.open_deeplink_info_dialog(
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
                    event = "day_page_deeplink_file_opened",
                    path = %path_display,
                    raw_href = %raw_href,
                );
                self.sync_footer(window, cx);
            }
            Err(error) => self.open_deeplink_info_dialog(
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
        self.open_deeplink_info_dialog(
            "Can't open this link",
            format!("{raw_href}\n\nFile does not exist:\n{path_display}"),
            raw_href,
            window,
            cx,
        );
    }

    fn open_note_deeplink(
        &mut self,
        note_id: crate::notes::NoteId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match crate::notes::open_note_in_notes_window(cx, note_id) {
            Ok(()) => {
                tracing::info!(event = "day_page_deeplink_note_opened", note_id = %note_id);
                self.sync_footer(window, cx);
            }
            Err(error) => self.open_deeplink_info_dialog(
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
        if self.app.upgrade().is_none() {
            self.open_deeplink_info_dialog(
                "Can't open this link",
                format!(
                    "{}\n\nCould not attach the confirmation prompt to the Day Page.",
                    raw_href
                ),
                raw_href,
                window,
                cx,
            );
            return;
        }

        let command_id_for_confirm = command_id.clone();
        let command_id_for_cancel = command_id.clone();
        let app = self.app.clone();
        let (sender, receiver) = async_channel::bounded::<bool>(1);
        self.open_deferred_confirm_prompt(
            run_deeplink_confirm_options(&command_id, &raw_href),
            sender,
            cx,
        );
        cx.spawn(async move |_this, cx| {
            if receiver.recv().await.unwrap_or(false) {
                let executed = cx.update(|cx| {
                    app.upgrade()
                        .map(|app| {
                            app.update(cx, |app, cx| {
                                app.execute_by_command_id_or_path(&command_id_for_confirm, cx)
                            })
                        })
                        .unwrap_or(false)
                });
                tracing::info!(
                    event = "day_page_deeplink_run_confirmed",
                    command_id = %command_id_for_confirm,
                    executed,
                    "day_page_deeplink_run_confirmed",
                );
            } else {
                tracing::info!(
                    event = "day_page_deeplink_run_cancelled",
                    command_id = %command_id_for_cancel,
                    "day_page_deeplink_run_cancelled",
                );
            }
        })
        .detach();
    }

    fn open_deeplink_info_dialog(
        &mut self,
        title: &'static str,
        body: String,
        copy_link: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.app.upgrade().is_none() {
            tracing::warn!(event = "day_page_deeplink_modal_unavailable", title);
            self.sync_footer(window, cx);
            return;
        }

        let (sender, receiver) = async_channel::bounded::<bool>(1);
        self.open_deferred_confirm_prompt(
            crate::confirm::ParentConfirmOptions {
                title: title.into(),
                body: body.into(),
                confirm_text: "Copy link".into(),
                cancel_text: "Dismiss".into(),
                confirm_variant: gpui_component::button::ButtonVariant::Primary,
                width: gpui::px(crate::confirm::PARENT_MODAL_WIDTH_PX),
            },
            sender,
            cx,
        );
        cx.spawn(async move |_this, cx| {
            if receiver.recv().await.unwrap_or(false) {
                cx.update(|cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_link.clone()));
                });
            }
        })
        .detach();
    }

    fn open_deferred_confirm_prompt(
        &self,
        options: crate::confirm::ParentConfirmOptions,
        sender: async_channel::Sender<bool>,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1))
                .await;
            cx.update(|cx| {
                if let Some(app) = app.upgrade() {
                    app.update(cx, |app, cx| {
                        app.open_confirm_prompt(options, sender, cx);
                    });
                }
            });
        })
        .detach();
    }
}

fn day_page_activation_error_message(reason: &ActivationErrorReason) -> String {
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

/// Short single-line preview for a clipboard shelf row, resolved from
/// clipboard history at render-state time. UI-only: this text is shown in the
/// shelf but never written to the day file, which stays raw-free.
fn clipboard_shelf_preview_text(entry_id: &str) -> String {
    const MAX_CHARS: usize = 64;
    let fallback = || "Clipboard entry".to_string();
    let Some(content) = crate::clipboard_history::get_entry_content(entry_id) else {
        return fallback();
    };
    let line = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    if line.is_empty() {
        return fallback();
    }
    if line.chars().count() > MAX_CHARS {
        let truncated: String = line.chars().take(MAX_CHARS).collect();
        format!("{truncated}…")
    } else {
        line.to_string()
    }
}

pub(crate) fn day_page_footer_buttons(
    app: &ScriptListApp,
    cx: Option<&gpui::App>,
) -> Vec<FooterButtonConfig> {
    let footer_disabled = crate::confirm::is_confirm_window_open();
    let actions_open = app.show_actions_popup || crate::actions::is_actions_window_open();
    let enabled = !footer_disabled;

    // While a kit:// resource preview replaces the editor, the footer owns the
    // preview's actions — same anatomy as other main-window surfaces: primary
    // action first, Actions ⌘K, and a "Back to …" slot last so Close reads as
    // "return to where I was", never "close the window". The remaining preview
    // actions stay reachable through ⌘K and their shortcuts (slot cap is 3).
    if let (AppView::DayPage { entity }, Some(cx)) = (&app.current_view, cx) {
        let view = entity.read(cx);
        if let Some(availability) = view.kit_resource_preview_action_availability() {
            let primary = if availability.open_source_target.is_some() {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Open Source")
            } else if availability.can_add_to_agent_chat {
                FooterButtonConfig::new(FooterAction::Ai, "⌘↵", "Add to Agent Chat")
            } else {
                FooterButtonConfig::new(FooterAction::Copy, "⌘C", "Copy URI")
            };
            return vec![
                primary.enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
                FooterButtonConfig::new(
                    FooterAction::Close,
                    "Esc",
                    view.kit_resource_preview_return_label(),
                )
                .enabled(enabled),
            ];
        }
    }

    vec![
        FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
            .selected(actions_open)
            .enabled(enabled),
    ]
}

impl ScriptListApp {
    pub(crate) fn show_day_page_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_day_page_view_with_substrate(None, window, cx);
    }

    pub(crate) fn show_day_page_view_with_substrate(
        &mut self,
        substrate: Option<BrainSubstrate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let substrate = substrate.unwrap_or_else(BrainSubstrate::default_kit);
        let app_entity = cx.entity();

        // A pending Today → main-menu context round trip holds the live Day
        // Page entity; re-entering Today resumes it (and abandons the search)
        // instead of binding a fresh view.
        if let Some(pending) = self.day_page_context_return.take() {
            self.restore_day_page_view_after_round_trip(pending.entity, window, cx);
            return;
        }

        let entity = if let AppView::DayPage { entity } = &self.current_view {
            let entity = entity.clone();
            entity.update(cx, |view, cx| {
                if let Err(error) = view.rebind_substrate(substrate, window, cx) {
                    tracing::error!(
                        target: "script_kit::day_page",
                        event = "day_page_rebind_failed",
                        error = %error,
                    );
                }
            });
            entity
        } else {
            let day_page = cx.new(|cx| DayPageView::new(app_entity, substrate, window, cx));
            day_page.update(cx, |view, cx| view.bind_today(window, cx));
            day_page
        };

        entity.update(cx, |view, cx| view.focus_editor(window, cx));
        self.current_view = AppView::DayPage { entity };
        self.focused_input = FocusedInput::None;
        self.rekey_main_automation_surface_from_current_view();
        self.sync_main_footer_popup(window, cx);
        cx.notify();
    }
}
