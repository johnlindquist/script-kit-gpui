//! Notes → main-window Agent Chat handoff.
//!
//! The Notes window never hosts an Agent Chat surface. Every notes→AI
//! affordance builds ONE immutable [`NotesAiHandoffPayload`] from the live
//! editor snapshot, dispatches it to the main `ScriptListApp` (which stages
//! the note as an explicit `@note` context chip), then activates the main
//! window. The Notes window always stays open.
//!
//! Receipts are redacted: lengths and fingerprints only, never note content.

use super::*;

/// Why a handoff could not be built or dispatched. Stable codes surface in
/// the redacted receipt and devtools state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotesAiHandoffError {
    /// Nothing to send: no saved note, no day binding, and an empty editor.
    NoNoteOrDraft,
    /// The main window handle is not registered (even after one retry).
    MainWindowUnavailable,
    /// The main window refused or failed to stage the context.
    MainStagingFailed,
}

impl NotesAiHandoffError {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::NoNoteOrDraft => "noNoteOrDraft",
            Self::MainWindowUnavailable => "mainWindowUnavailable",
            Self::MainStagingFailed => "mainStagingFailed",
        }
    }
}

/// Fully materialized handoff payload. Built BEFORE touching the main window
/// so no later closure re-reads the Notes editor.
#[derive(Clone)]
pub(crate) struct NotesAiHandoffPayload {
    pub(crate) target: crate::ai::TabAiTargetContext,
    pub(crate) supplemental_parts: Vec<crate::ai::message_parts::AiContextPart>,
    pub(crate) cart_note_id: Option<crate::notes::model::NoteId>,
    pub(crate) cart_item_ids: Vec<String>,
    pub(crate) source: &'static str,
    pub(crate) is_draft: bool,
}

/// Outcome states for the redacted receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotesAiHandoffStatus {
    Staged,
    /// Context staged in main, but the persisted cart rows could not be
    /// deleted — the cart is retained for manual recovery.
    StagedCartRetained,
    Blocked,
    Failed,
}

impl NotesAiHandoffStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::StagedCartRetained => "stagedCartRetained",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }
}

/// Redacted handoff receipt: identity, shape, and destination — never note
/// content, selection text, or raw cart payloads.
#[derive(Debug, Clone)]
pub(crate) struct NotesAiHandoffReceipt {
    pub(crate) generation: u64,
    pub(crate) source: &'static str,
    pub(crate) status: NotesAiHandoffStatus,
    pub(crate) target_semantic_id: String,
    pub(crate) target_label_length: usize,
    pub(crate) target_label_fingerprint: String,
    pub(crate) is_draft: bool,
    pub(crate) supplemental_part_count: usize,
    pub(crate) destination_window_id: &'static str,
    pub(crate) destination_surface: &'static str,
    pub(crate) notes_instance_id: u64,
    pub(crate) error_code: Option<&'static str>,
    pub(crate) recorded_at: std::time::Instant,
}

/// Binary-registered hook that stages a Notes handoff into the main
/// `ScriptListApp` and activates the main window on success. The main app
/// type lives in the binary crate, so this dual-compiled file cannot
/// downcast the main window's root view itself; the binary registers the
/// downcast-and-stage closure at app startup
/// (`register_notes_ai_main_handoff_hook`).
pub type NotesAiMainHandoffHook = fn(
    crate::ai::TabAiTargetContext,
    Vec<crate::ai::message_parts::AiContextPart>,
    &'static str,
    &mut gpui::App,
) -> Result<bool, String>;

static NOTES_AI_MAIN_HANDOFF_HOOK: std::sync::OnceLock<NotesAiMainHandoffHook> =
    std::sync::OnceLock::new();

pub fn register_notes_ai_main_handoff_hook(hook: NotesAiMainHandoffHook) {
    let _ = NOTES_AI_MAIN_HANDOFF_HOOK.set(hook);
}

/// FNV-1a 64-bit fingerprint for redacted label identity.
pub(crate) fn fnv1a64_fingerprint(value: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Pure inputs for composing the canonical note target — extracted so the
/// identity/selection invariants are unit-testable without a GPUI context.
pub(crate) struct NotesAiTargetSnapshot {
    pub(crate) content: String,
    pub(crate) selection: std::ops::Range<usize>,
    /// `(note_id, title, is_pinned)` of the selected SAVED note, if any.
    pub(crate) saved_note: Option<(String, String, bool)>,
    /// Stable date string of the active day binding, if any.
    pub(crate) day_date: Option<String>,
    pub(crate) instance_id: u64,
    pub(crate) has_unsaved_changes: bool,
    pub(crate) view_mode: String,
}

/// Compose the canonical live-editor note target. Returns `(target, is_draft)`.
///
/// Identity invariants:
/// - a saved note uses its real `NoteId` (`note:<id>`), even when the live
///   editor content differs from disk (the live content is sent);
/// - a day binding uses a stable `day:<date>` identity;
/// - an unsaved draft uses `draft:<instance_id>` — never a shared constant —
///   and is fully supported (no forced save).
pub(crate) fn compose_note_ai_target(
    snapshot: NotesAiTargetSnapshot,
) -> Result<(crate::ai::TabAiTargetContext, bool), NotesAiHandoffError> {
    let NotesAiTargetSnapshot {
        content,
        selection,
        saved_note,
        day_date,
        instance_id,
        has_unsaved_changes,
        view_mode,
    } = snapshot;

    if saved_note.is_none() && day_date.is_none() && content.trim().is_empty() {
        return Err(NotesAiHandoffError::NoNoteOrDraft);
    }

    let (identity, note_id, day_date, title, is_pinned, is_draft) =
        if let Some((id, title, is_pinned)) = saved_note {
            (
                format!("note:{id}"),
                Some(id),
                None,
                Some(title.trim().to_string())
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| "Untitled Note".to_string()),
                is_pinned,
                false,
            )
        } else if let Some(date) = day_date {
            (
                format!("day:{date}"),
                None,
                Some(date.clone()),
                format!("Day Note — {date}"),
                false,
                false,
            )
        } else {
            (
                format!("draft:{instance_id}"),
                None,
                None,
                "Untitled Note".to_string(),
                false,
                true,
            )
        };

    let selected_text = content
        .get(selection.clone())
        .filter(|text| !text.is_empty())
        .map(str::to_string);

    let target = crate::ai::TabAiTargetContext {
        source: "Notes".to_string(),
        kind: "note".to_string(),
        semantic_id: crate::protocol::generate_semantic_id("note", 0, &identity),
        label: title.clone(),
        metadata: Some(serde_json::json!({
            "noteId": note_id,
            "dayDate": day_date,
            "draft": is_draft,
            "title": title,
            "content": content,
            "contentSource": "liveEditorSnapshot",
            "hasUnsavedChanges": has_unsaved_changes,
            "selection": {
                "start": selection.start,
                "end": selection.end,
                "text": selected_text,
            },
            "preview": NotesApp::strip_markdown_for_preview(
                selected_text.as_deref().unwrap_or(&content),
            ),
            "isPinned": is_pinned,
            "viewMode": view_mode,
        })),
    };

    Ok((target, is_draft))
}

impl NotesApp {
    /// Build the canonical live-editor note target for the main-window
    /// handoff. Returns `(target, is_draft)`. See [`compose_note_ai_target`]
    /// for the identity invariants.
    pub(crate) fn build_selected_note_ai_target(
        &self,
        cx: &Context<Self>,
    ) -> Result<(crate::ai::TabAiTargetContext, bool), NotesAiHandoffError> {
        let editor = self.editor_state.read(cx);
        let content = editor.value().to_string();
        let selection = editor.selection();
        let saved_note = self
            .selected_note_id
            .and_then(|id| self.notes.iter().find(|note| note.id == id))
            .map(|note| {
                (
                    note.id.as_str().to_string(),
                    note.title.clone(),
                    note.is_pinned,
                )
            });

        compose_note_ai_target(NotesAiTargetSnapshot {
            content,
            selection,
            saved_note,
            day_date: self
                .active_day_binding
                .as_ref()
                .map(|day| day.date.to_string()),
            instance_id: self.instance_id,
            has_unsaved_changes: self.has_unsaved_changes,
            view_mode: format!("{:?}", self.view_mode),
        })
    }

    /// Materialize the complete immutable handoff payload: canonical target,
    /// deduplicated persisted cart parts (the note body is NEVER duplicated
    /// as a supplemental part — the target metadata already carries the live
    /// note and selection), and the original cart row IDs for post-success
    /// consumption.
    pub(crate) fn build_notes_ai_handoff_payload(
        &self,
        source: &'static str,
        cx: &Context<Self>,
    ) -> Result<NotesAiHandoffPayload, NotesAiHandoffError> {
        let (target, is_draft) = self.build_selected_note_ai_target(cx)?;

        let cart_note_id = self.selected_note_id;
        let mut supplemental_parts: Vec<crate::ai::message_parts::AiContextPart> = Vec::new();
        let mut cart_item_ids: Vec<String> = Vec::new();

        if let Some(note_id) = cart_note_id {
            match crate::notes::storage::list_note_cart_items_deduped(note_id) {
                Ok(items) => {
                    for item in &items {
                        let part = item.to_ai_context_part();
                        if !supplemental_parts.iter().any(|existing| existing == &part) {
                            supplemental_parts.push(part);
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "notes_ai_handoff_cart_load_failed",
                        note_id = %note_id,
                        error = %error,
                    );
                }
            }
            if let Ok(items) = crate::notes::storage::list_note_cart_items(note_id) {
                cart_item_ids = items.into_iter().map(|item| item.id).collect();
            }
        }

        Ok(NotesAiHandoffPayload {
            target,
            supplemental_parts,
            cart_note_id,
            cart_item_ids,
            source,
            is_draft,
        })
    }

    /// The ONE notes→AI command. Every affordance (Cmd+Enter, the actions
    /// panel `send_to_ai`, Cmd+Shift+A, the titlebar Ask AI button, and the
    /// automation key route) funnels here.
    ///
    /// Never closes, hides, or orders out the Notes window.
    pub(crate) fn handoff_selected_note_to_main_agent_chat(
        &mut self,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> bool {
        let payload = match self.build_notes_ai_handoff_payload(source, cx) {
            Ok(payload) => payload,
            Err(error) => {
                self.record_ai_handoff_receipt(
                    None,
                    source,
                    NotesAiHandoffStatus::Blocked,
                    Some(error.code()),
                );
                self.show_action_feedback("Nothing to send", true);
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "notes_ai_handoff_blocked",
                    source,
                    error_code = error.code(),
                );
                cx.notify();
                return false;
            }
        };

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_ai_handoff_requested",
            source,
            target_semantic_id = %payload.target.semantic_id,
            is_draft = payload.is_draft,
            supplemental_part_count = payload.supplemental_parts.len(),
        );

        match Self::dispatch_notes_ai_handoff_to_main(payload.clone(), cx) {
            Ok(true) => {
                let mut status = NotesAiHandoffStatus::Staged;
                if let Some(note_id) = payload.cart_note_id {
                    if !payload.cart_item_ids.is_empty() {
                        match crate::notes::storage::delete_note_cart_items(
                            note_id,
                            &payload.cart_item_ids,
                        ) {
                            Ok(_) => {}
                            Err(error) => {
                                // A cart-delete failure never retroactively
                                // fails the staging: report and retain.
                                status = NotesAiHandoffStatus::StagedCartRetained;
                                tracing::warn!(
                                    target: "script_kit::tab_ai",
                                    event = "notes_ai_handoff_cart_consume_failed",
                                    note_id = %note_id,
                                    error = %error,
                                );
                            }
                        }
                    }
                }
                self.record_ai_handoff_receipt(Some(&payload), source, status, None);
                match status {
                    NotesAiHandoffStatus::StagedCartRetained => self
                        .show_action_feedback("Opened in Agent Chat; cart was not cleared", true),
                    _ => self.show_action_feedback("Opened in main Agent Chat", false),
                }
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "notes_ai_handoff_main_staged",
                    source,
                    status = status.as_str(),
                    target_semantic_id = %payload.target.semantic_id,
                );
                cx.notify();
                true
            }
            Ok(false) => {
                self.record_ai_handoff_receipt(
                    Some(&payload),
                    source,
                    NotesAiHandoffStatus::Failed,
                    Some(NotesAiHandoffError::MainStagingFailed.code()),
                );
                self.show_action_feedback("Agent unavailable", true);
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "notes_ai_handoff_main_stage_refused",
                    source,
                );
                cx.notify();
                false
            }
            Err(error) => {
                if error == "main_window_handle_missing"
                    || error.starts_with("main_window_update_failed")
                {
                    // Recoverable: either no live main window yet, or the
                    // dispatch arrived while the main window was already on
                    // the GPUI update stack (automation simulateKey routes
                    // through the main app), which reads as "window not
                    // found" from a re-entrant update. Retry exactly once
                    // outside the update stack. Only request a show when the
                    // main window is genuinely not visible — showing the
                    // launcher resets its view and would destroy a reusable
                    // Agent Chat before the retry.
                    if !crate::is_main_window_visible() {
                        crate::request_show_main_window();
                    }
                    self.schedule_ai_handoff_retry(payload, cx);
                    return false;
                }
                self.record_ai_handoff_receipt(
                    Some(&payload),
                    source,
                    NotesAiHandoffStatus::Failed,
                    Some(NotesAiHandoffError::MainStagingFailed.code()),
                );
                self.show_action_feedback("Agent unavailable", true);
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "notes_ai_handoff_dispatch_failed",
                    source,
                    error = %error,
                );
                cx.notify();
                false
            }
        }
    }

    /// One bounded next-frame retry for a missing main window handle. Cart
    /// rows are never consumed on the failure path.
    fn schedule_ai_handoff_retry(
        &mut self,
        payload: NotesAiHandoffPayload,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(50))
                .await;
            let _ = cx.update(|cx| {
                let Some(entity) = this.upgrade() else {
                    return;
                };
                entity.update(cx, |app, cx| {
                    let source = payload.source;
                    match Self::dispatch_notes_ai_handoff_to_main(payload.clone(), cx) {
                        Ok(true) => {
                            let mut status = NotesAiHandoffStatus::Staged;
                            if let Some(note_id) = payload.cart_note_id {
                                if !payload.cart_item_ids.is_empty()
                                    && crate::notes::storage::delete_note_cart_items(
                                        note_id,
                                        &payload.cart_item_ids,
                                    )
                                    .is_err()
                                {
                                    status = NotesAiHandoffStatus::StagedCartRetained;
                                }
                            }
                            app.record_ai_handoff_receipt(Some(&payload), source, status, None);
                            app.show_action_feedback("Opened in main Agent Chat", false);
                            tracing::info!(
                                target: "script_kit::tab_ai",
                                event = "notes_ai_handoff_main_staged",
                                source,
                                status = status.as_str(),
                                retried = true,
                            );
                        }
                        Ok(false) | Err(_) => {
                            app.record_ai_handoff_receipt(
                                Some(&payload),
                                source,
                                NotesAiHandoffStatus::Failed,
                                Some(NotesAiHandoffError::MainWindowUnavailable.code()),
                            );
                            app.show_action_feedback("Agent unavailable", true);
                            tracing::warn!(
                                target: "script_kit::tab_ai",
                                event = "notes_ai_handoff_retry_failed",
                                source,
                            );
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// Cross-window dispatch: stage into the main `ScriptListApp` through the
    /// binary-registered hook, which shows and activates the main window
    /// AFTER staging so its first visible frame already has the note chip and
    /// composer prefill.
    ///
    /// Deliberately contains no Notes close/hide/order-out/exit-reveal call.
    fn dispatch_notes_ai_handoff_to_main(
        payload: NotesAiHandoffPayload,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        if crate::get_main_window_handle().is_none() {
            return Err("main_window_handle_missing".to_string());
        }
        let Some(hook) = NOTES_AI_MAIN_HANDOFF_HOOK.get() else {
            return Err("notes_ai_handoff_hook_unregistered".to_string());
        };
        hook(
            payload.target,
            payload.supplemental_parts,
            payload.source,
            cx,
        )
    }

    /// Record the redacted receipt (also projected into devtools state).
    fn record_ai_handoff_receipt(
        &mut self,
        payload: Option<&NotesAiHandoffPayload>,
        source: &'static str,
        status: NotesAiHandoffStatus,
        error_code: Option<&'static str>,
    ) {
        self.ai_handoff_generation = self.ai_handoff_generation.wrapping_add(1);
        let (semantic_id, label, is_draft, part_count) = match payload {
            Some(payload) => (
                payload.target.semantic_id.clone(),
                payload.target.label.clone(),
                payload.is_draft,
                payload.supplemental_parts.len(),
            ),
            None => (String::new(), String::new(), false, 0),
        };
        self.last_ai_handoff = Some(NotesAiHandoffReceipt {
            generation: self.ai_handoff_generation,
            source,
            status,
            target_semantic_id: semantic_id,
            target_label_length: label.chars().count(),
            target_label_fingerprint: fnv1a64_fingerprint(&label),
            is_draft,
            supplemental_part_count: part_count,
            destination_window_id: "main",
            destination_surface: "agentChat",
            notes_instance_id: self.instance_id,
            error_code,
            recorded_at: std::time::Instant::now(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compose_note_ai_target, fnv1a64_fingerprint, NotesAiHandoffError, NotesAiTargetSnapshot,
    };

    fn snapshot(content: &str, selection: std::ops::Range<usize>) -> NotesAiTargetSnapshot {
        NotesAiTargetSnapshot {
            content: content.to_string(),
            selection,
            saved_note: None,
            day_date: None,
            instance_id: 7,
            has_unsaved_changes: false,
            view_mode: "AllNotes".to_string(),
        }
    }

    #[test]
    fn saved_note_target_uses_real_note_id_and_live_editor_content() {
        let mut snap = snapshot("live edited body", 0..0);
        snap.saved_note = Some(("abc-123".to_string(), "My Note".to_string(), true));
        snap.has_unsaved_changes = true;
        let (target, is_draft) = compose_note_ai_target(snap).expect("saved note composes");
        assert!(!is_draft);
        assert_eq!(target.source, "Notes");
        assert_eq!(target.kind, "note");
        assert_eq!(target.label, "My Note");
        let metadata = target.metadata.expect("metadata present");
        assert_eq!(metadata["noteId"], "abc-123");
        assert_eq!(metadata["draft"], false);
        assert_eq!(
            metadata["content"], "live edited body",
            "live editor content is sent, not disk content",
        );
        assert_eq!(metadata["contentSource"], "liveEditorSnapshot");
        assert_eq!(metadata["hasUnsavedChanges"], true);
        assert_eq!(metadata["isPinned"], true);
    }

    #[test]
    fn day_note_target_has_stable_day_identity() {
        let mut snap = snapshot("today's entries", 0..0);
        snap.day_date = Some("2026-07-26".to_string());
        let (target, is_draft) = compose_note_ai_target(snap).expect("day note composes");
        assert!(!is_draft);
        assert_eq!(target.label, "Day Note — 2026-07-26");
        let metadata = target.metadata.expect("metadata present");
        assert_eq!(metadata["dayDate"], "2026-07-26");
        assert_eq!(metadata["noteId"], serde_json::Value::Null);
    }

    #[test]
    fn unsaved_draft_is_accepted_and_uses_instance_scoped_identity() {
        let (target, is_draft) =
            compose_note_ai_target(snapshot("a fresh draft", 0..0)).expect("draft composes");
        assert!(is_draft, "unsaved drafts are supported without a save");
        assert_eq!(target.label, "Untitled Note");
        let metadata = target.metadata.as_ref().expect("metadata present");
        assert_eq!(metadata["draft"], true);
        // Identity derives from the Notes instance, never a shared constant:
        // a second instance must produce a different semantic id.
        let mut other = snapshot("a fresh draft", 0..0);
        other.instance_id = 8;
        let (other_target, _) = compose_note_ai_target(other).expect("draft composes");
        assert_ne!(
            target.semantic_id, other_target.semantic_id,
            "draft identity must be instance-scoped",
        );
    }

    #[test]
    fn empty_editor_with_no_note_or_day_fails_closed() {
        let result = compose_note_ai_target(snapshot("   \n", 0..0));
        assert!(matches!(result, Err(NotesAiHandoffError::NoNoteOrDraft)));
    }

    #[test]
    fn selection_is_captured_in_target_metadata() {
        let (target, _) = compose_note_ai_target(snapshot("hello world", 6..11)).expect("composes");
        let metadata = target.metadata.expect("metadata present");
        assert_eq!(metadata["selection"]["start"], 6);
        assert_eq!(metadata["selection"]["end"], 11);
        assert_eq!(metadata["selection"]["text"], "world");
    }

    #[test]
    fn fingerprint_is_stable_and_redacted() {
        let fp = fnv1a64_fingerprint("Test Note");
        assert!(fp.starts_with("fnv1a64:"), "fingerprint carries its scheme");
        assert_eq!(fp, fnv1a64_fingerprint("Test Note"), "deterministic");
        assert_ne!(
            fp,
            fnv1a64_fingerprint("Other Note"),
            "distinct labels fingerprint differently"
        );
        assert!(
            !fp.contains("Test"),
            "fingerprint must not leak label content"
        );
    }
}
