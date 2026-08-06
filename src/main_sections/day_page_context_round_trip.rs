// Today → main-menu `@context` search round trip.
//
// Requirement (.notes/today-requirements.md "Spine And Agent Chat"):
// triggering an `@context` search row from Today swaps to the main menu so
// the user gets the normal main-menu search experience; accepting a context
// row returns to Today with the accepted token spliced into the originating
// line; Escape (or anything that would close the launcher) cancels back to
// Today unchanged.
//
// The held `Entity<DayPageView>` keeps the unsaved editor session authoritative
// across the trip. A host-owned generation snapshot rejects stale callbacks;
// no save or disk reload occurs before the accepted token is spliced.

use std::ops::Range;

use crate::components::notes_editor::spine::{
    context_round_trip_request_for_contract, replace_segment_content, NotesEditorHostSpineContract,
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DayPageHostReturnSnapshot {
    pub(crate) entity_id: u64,
    pub(crate) host_return_generation: u64,
    pub(crate) document_id_fingerprint: String,
    pub(crate) document_generation: String,
    pub(crate) content_length: usize,
    pub(crate) content_fingerprint: String,
    pub(crate) dirty: bool,
    pub(crate) selection: Range<usize>,
    pub(crate) scroll_top: Option<f32>,
    pub(crate) mode: &'static str,
    pub(crate) alias_id_fingerprints: Vec<String>,
    pub(crate) focus_semantic_id: &'static str,
}

pub(crate) struct DayPageContextReturn {
    pub id: String,
    pub entity: Entity<DayPageView>,
    pub snapshot: DayPageHostReturnSnapshot,
    /// Byte range of the active line within the day content at hand-off.
    pub line_range: Range<usize>,
    /// Active `@context` segment byte range relative to the line text.
    pub segment_byte_range: Range<usize>,
    pub segment_chars: usize,
    pub segment_hash: String,
}

fn day_page_context_round_trip_fingerprint(value: &str) -> String {
    use sha2::{Digest, Sha256};

    format!("sha256:{:x}", Sha256::digest(value.as_bytes()))
}

fn day_page_context_round_trip_receipt(
    id: &str,
    status: &str,
    line_range: &Range<usize>,
    segment_byte_range: &Range<usize>,
    segment_chars: usize,
    segment_hash: &str,
    snapshot: Option<&DayPageHostReturnSnapshot>,
    token: Option<&str>,
    visible_reference: Option<&str>,
    has_alias: Option<bool>,
) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 1,
        "receiptKind": "dayPage.contextRoundTrip",
        "redacted": true,
        "id": id,
        "status": status,
        "lineRange": {
            "start": line_range.start,
            "end": line_range.end,
            "unit": "utf8ByteOffset",
        },
        "segmentRange": {
            "start": segment_byte_range.start,
            "end": segment_byte_range.end,
            "unit": "utf8ByteOffset",
        },
        "segmentChars": segment_chars,
        "segmentHash": segment_hash,
        "hostReturn": snapshot.map(|snapshot| serde_json::json!({
            "entityId": snapshot.entity_id,
            "generation": snapshot.host_return_generation,
            "documentIdFingerprint": snapshot.document_id_fingerprint,
            "documentGeneration": snapshot.document_generation,
            "contentLength": snapshot.content_length,
            "contentFingerprint": snapshot.content_fingerprint,
            "dirty": snapshot.dirty,
            "selectionLength": snapshot.selection.end.saturating_sub(snapshot.selection.start),
            "scrollTop": snapshot.scroll_top,
            "mode": snapshot.mode,
            "aliasCount": snapshot.alias_id_fingerprints.len(),
            "focusSemanticId": snapshot.focus_semantic_id,
        })),
        "tokenChars": token.map(|token| token.chars().count()),
        "tokenHash": token.map(day_page_context_round_trip_fingerprint),
        "visibleReferenceChars": visible_reference.map(|reference| reference.chars().count()),
        "visibleReferenceHash": visible_reference.map(day_page_context_round_trip_fingerprint),
        "hasAlias": has_alias,
    })
}

impl DayPageView {
    fn capture_host_return_snapshot(&mut self, cx: &App) -> DayPageHostReturnSnapshot {
        self.host_return_generation = self.host_return_generation.saturating_add(1);
        let content = self.notes_editor.read(cx).content(cx);
        let selection = self.notes_editor.read(cx).selection(cx);
        let scroll = self.editor_state.read(cx).automation_scroll_metrics();
        let mut alias_id_fingerprints = self
            .spine_handoff
            .mention_aliases
            .keys()
            .map(|token| day_page_context_round_trip_fingerprint(token))
            .collect::<Vec<_>>();
        alias_id_fingerprints.sort();
        let document_identity = self
            .session
            .path()
            .map(|path| path.display().to_string())
            .or_else(|| self.session.bound_date().map(|date| format!("day:{date}")))
            .unwrap_or_else(|| "unbound".to_string());
        let content_fingerprint = day_page_context_round_trip_fingerprint(&content);

        DayPageHostReturnSnapshot {
            entity_id: self.instance_id,
            host_return_generation: self.host_return_generation,
            document_id_fingerprint: day_page_context_round_trip_fingerprint(&document_identity),
            document_generation: content_fingerprint.clone(),
            content_length: content.chars().count(),
            content_fingerprint,
            dirty: self.session.is_dirty(),
            selection,
            scroll_top: scroll
                .get("scrollTop")
                .and_then(serde_json::Value::as_f64)
                .map(|value| value as f32),
            mode: if self.kit_resource_preview.is_some() {
                "kitResourcePreview"
            } else if self.read_mode {
                "read"
            } else {
                "edit"
            },
            alias_id_fingerprints,
            focus_semantic_id: "input:day-page-editor",
        }
    }

    pub(crate) fn record_context_round_trip_receipt(&mut self, receipt: serde_json::Value) {
        self.last_context_round_trip_receipt = Some(receipt);
    }

    /// Auto-trigger: typing into ANY `@` mention on the active line swaps to
    /// the real main menu (the exact launcher selection UX) — the Day Page
    /// renders no `@` selector of its own. Called from `on_editor_change`;
    /// only insertions trigger so deletions can still edit a mention inline.
    pub(crate) fn maybe_begin_day_page_context_round_trip_from_edit(
        &mut self,
        previous_len: usize,
        content: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let selection = self.notes_editor.read(cx).selection(cx);
        let Some(request) = context_round_trip_request_for_contract(
            NotesEditorHostSpineContract::day_page(),
            previous_len,
            content,
            selection,
        ) else {
            return false;
        };

        // The held entity is authoritative across the round trip. Do not save
        // here: unsaved editor state and its dirty generation must survive.
        let Some(app) = self.app.upgrade() else {
            return false;
        };
        let snapshot = self.capture_host_return_snapshot(cx);
        let entity = cx.entity();
        let id = format!(
            "{}:{}:{}:{}",
            chrono::Utc::now().timestamp_millis(),
            request.line_range.start,
            request.segment_byte_range.start,
            day_page_context_round_trip_fingerprint(&request.segment_text)
                .trim_start_matches("sha256:")
                .chars()
                .take(12)
                .collect::<String>()
        );
        let receipt = day_page_context_round_trip_receipt(
            &id,
            "pending",
            &request.line_range,
            &request.segment_byte_range,
            request.segment_text.chars().count(),
            &day_page_context_round_trip_fingerprint(&request.segment_text),
            Some(&snapshot),
            None,
            None,
            None,
        );
        self.record_context_round_trip_receipt(receipt.clone());
        tracing::info!(
            target: "script_kit::day_page",
            event = "day_page_context_round_trip_receipt",
            receipt = %receipt,
        );
        window.defer(cx, move |window, cx| {
            app.update(cx, |app, cx| {
                app.begin_day_page_context_round_trip(
                    id,
                    entity,
                    snapshot,
                    request.line_range,
                    request.segment_byte_range,
                    request.segment_text,
                    window,
                    cx,
                );
            });
        });
        true
    }

    /// Splice the accepted token into the originating line and re-arm state.
    pub(crate) fn complete_context_round_trip(
        &mut self,
        line_range: Range<usize>,
        segment_byte_range: Range<usize>,
        token: &str,
        alias: Option<crate::ai::message_parts::AiContextPart>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.notes_editor.read(cx).content(cx);
        let visible_reference = markdown_reference_for_day_page_context_part(token, alias.as_ref())
            .unwrap_or_else(|| token.to_string());
        let spliced = replace_segment_content(
            &content,
            line_range,
            segment_byte_range,
            &visible_reference,
            true,
        );
        let (new_content, cursor) = match spliced {
            Some(done) => done,
            None => {
                // Ranges no longer fit (external edit while away) — append to
                // the end rather than dropping the accepted context.
                let trimmed = content.trim_end();
                let new_content = if trimmed.is_empty() {
                    format!("{visible_reference} ")
                } else {
                    format!("{trimmed} {visible_reference} ")
                };
                let cursor = new_content.len();
                (new_content, cursor)
            }
        };
        // The splice is not typing: pre-set the length so its Change event
        // cannot read as growth and immediately re-open the main-menu search.
        self.last_editor_content_len = new_content.len();
        self.notes_editor.update(cx, |editor, cx| {
            editor.set_value(new_content.clone(), window, cx);
            editor.set_selection(cursor, cursor, window, cx);
        });
        self.session.apply_editor_content(&new_content);
        self.refresh_fragment_open_targets(&new_content);
        if let Some(part) = alias {
            self.spine_handoff
                .register_mention_alias(visible_reference, part);
        }
        self.spine_handoff
            .sync_with_markdown_references(&new_content);
        self.schedule_autosave_flush(cx);
        self.sync_footer(window, cx);
        cx.notify();
    }
}

fn day_page_host_return_is_current(
    snapshot: &DayPageHostReturnSnapshot,
    entity_id: u64,
    host_return_generation: u64,
) -> bool {
    snapshot.entity_id == entity_id
        && snapshot.host_return_generation == host_return_generation
}

impl ScriptListApp {
    pub(crate) fn begin_day_page_context_round_trip(
        &mut self,
        id: String,
        entity: Entity<DayPageView>,
        snapshot: DayPageHostReturnSnapshot,
        line_range: Range<usize>,
        segment_byte_range: Range<usize>,
        segment_text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.day_page_context_return = Some(DayPageContextReturn {
            id,
            entity,
            snapshot,
            line_range,
            segment_byte_range,
            segment_chars: segment_text.chars().count(),
            segment_hash: day_page_context_round_trip_fingerprint(&segment_text),
        });
        self.reset_to_script_list(cx);
        self.set_filter_text_immediate(segment_text, window, cx);
        self.request_script_list_main_filter_focus(cx);
        self.rekey_main_automation_surface_from_current_view();
        self.sync_main_footer_popup(window, cx);
        cx.notify();
    }

    /// Accept hook: a main-menu context row resolved into `token` while a
    /// Day Page round trip was pending. Returns true when the trip completed.
    pub(crate) fn try_complete_day_page_context_round_trip(
        &mut self,
        token: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.try_complete_day_page_context_round_trip_with_alias(token, None, window, cx)
    }

    pub(crate) fn try_complete_day_page_context_round_trip_with_alias(
        &mut self,
        token: &str,
        alias: Option<crate::ai::message_parts::AiContextPart>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(pending) = self.day_page_context_return.take() else {
            return false;
        };
        let token = token.trim();
        if token.is_empty() {
            self.day_page_context_return = Some(pending);
            return false;
        }
        let alias = alias.or_else(|| self.spine_mention_aliases.get(token).cloned());
        let has_alias = alias.is_some();
        let visible_reference = markdown_reference_for_day_page_context_part(token, alias.as_ref())
            .unwrap_or_else(|| token.to_string());
        let receipt = day_page_context_round_trip_receipt(
            &pending.id,
            "completed",
            &pending.line_range,
            &pending.segment_byte_range,
            pending.segment_chars,
            &pending.segment_hash,
            Some(&pending.snapshot),
            Some(token),
            Some(&visible_reference),
            Some(has_alias),
        );
        let entity = pending.entity.clone();
        let snapshot = pending.snapshot.clone();
        let completed_receipt = receipt.clone();
        entity.update(cx, |view, cx| {
            view.complete_context_round_trip(
                pending.line_range,
                pending.segment_byte_range,
                token,
                alias,
                window,
                cx,
            );
            view.record_context_round_trip_receipt(completed_receipt);
        });
        self.restore_day_page_view_after_round_trip(entity, snapshot, window, cx);
        tracing::info!(
            target: "script_kit::day_page",
            event = "day_page_context_round_trip_receipt",
            receipt = %receipt,
        );
        true
    }

    pub(crate) fn has_day_page_context_round_trip_pending(&self) -> bool {
        self.day_page_context_return.is_some()
    }

    /// Cancel hook: Escape/close from the main menu while a Day Page round
    /// trip is pending returns to Today unchanged instead of closing.
    pub(crate) fn try_cancel_day_page_context_round_trip(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(pending) = self.day_page_context_return.take() else {
            return false;
        };
        let receipt = day_page_context_round_trip_receipt(
            &pending.id,
            "cancelled",
            &pending.line_range,
            &pending.segment_byte_range,
            pending.segment_chars,
            &pending.segment_hash,
            Some(&pending.snapshot),
            None,
            None,
            None,
        );
        let entity = pending.entity.clone();
        let snapshot = pending.snapshot.clone();
        let cancelled_receipt = receipt.clone();
        entity.update(cx, |view, _cx| {
            view.record_context_round_trip_receipt(cancelled_receipt);
        });
        self.restore_day_page_view_after_round_trip(entity, snapshot, window, cx);
        tracing::info!(
            target: "script_kit::day_page",
            event = "day_page_context_round_trip_receipt",
            receipt = %receipt,
        );
        true
    }

    /// Deferred cancel for window-handle-less callers (`close_and_reset_window`).
    pub(crate) fn cancel_day_page_context_round_trip_deferred(&mut self, cx: &mut Context<Self>) {
        let app_entity = cx.entity();
        cx.defer(move |cx| {
            let Some(handle) = crate::get_main_window_handle() else {
                return;
            };
            let _ = handle.update(cx, |_, window, cx| {
                app_entity.update(cx, |app, cx| {
                    app.try_cancel_day_page_context_round_trip(window, cx);
                });
            });
        });
    }

    fn restore_day_page_view_after_round_trip(
        &mut self,
        entity: Entity<DayPageView>,
        snapshot: DayPageHostReturnSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = entity.read(cx);
        if !day_page_host_return_is_current(
            &snapshot,
            current.instance_id,
            current.host_return_generation,
        ) {
            tracing::info!(
                target: "script_kit::day_page",
                event = "day_page_context_return_ignored",
                reason = "staleGeneration",
                snapshot_entity_id = snapshot.entity_id,
                live_entity_id = current.instance_id,
            );
            return;
        }

        self.set_filter_text_immediate(String::new(), window, cx);
        self.current_view = AppView::DayPage {
            entity: entity.clone(),
        };
        self.focused_input = FocusedInput::None;
        self.rekey_main_automation_surface_from_current_view();
        entity.update(cx, |view, cx| {
            view.focus_editor_preserving_state(window, cx)
        });
        self.sync_main_footer_popup(window, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod day_page_host_return_tests {
    use super::*;

    fn snapshot() -> DayPageHostReturnSnapshot {
        DayPageHostReturnSnapshot {
            entity_id: 7,
            host_return_generation: 11,
            document_id_fingerprint: "sha256:document".to_string(),
            document_generation: "sha256:generation".to_string(),
            content_length: 42,
            content_fingerprint: "sha256:content".to_string(),
            dirty: true,
            selection: 4..9,
            scroll_top: Some(12.0),
            mode: "edit",
            alias_id_fingerprints: vec!["sha256:alias".to_string()],
            focus_semantic_id: "input:day-page-editor",
        }
    }

    #[test]
    fn day_page_return_accepts_only_the_exact_entity_generation() {
        let snapshot = snapshot();
        assert!(day_page_host_return_is_current(&snapshot, 7, 11));
        assert!(!day_page_host_return_is_current(&snapshot, 8, 11));
        assert!(!day_page_host_return_is_current(&snapshot, 7, 12));
    }
}
