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
    /// Persisted cart attachments could not be materialized before dispatch.
    CartLoadFailed,
    /// The main window refused or failed to stage the context.
    MainStagingFailed,
}

impl NotesAiHandoffError {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::NoNoteOrDraft => "noNoteOrDraft",
            Self::MainWindowUnavailable => "mainWindowUnavailable",
            Self::CartLoadFailed => "cartLoadFailed",
            Self::MainStagingFailed => "mainStagingFailed",
        }
    }
}

static NEXT_NOTES_HANDOFF_REQUEST_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

fn next_notes_handoff_request_id() -> String {
    let generation =
        NEXT_NOTES_HANDOFF_REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("notes-handoff-{generation}")
}

/// One persisted Notes cart row mapped to one pre-materialized Agent Chat
/// context item. The idempotency key remains stable across the bounded retry.
#[derive(Clone)]
pub(crate) struct NotesHandoffAttachment {
    pub(crate) cart_item_id: String,
    pub(crate) context_item: crate::ai::staged_context::StagedContextItem,
    pub(crate) idempotency_key: String,
}

impl std::fmt::Debug for NotesHandoffAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotesHandoffAttachment")
            .field("cart_item_id_length", &self.cart_item_id.chars().count())
            .field("context_item", &self.context_item)
            .field(
                "idempotency_key_fingerprint",
                &fnv1a64_fingerprint(&self.idempotency_key),
            )
            .finish()
    }
}

/// Per-item staging result returned synchronously by the main Agent Chat host.
#[derive(Debug, Clone)]
pub(crate) enum NotesContextStageOutcome {
    Accepted {
        context_item_id: crate::ai::staged_context::ContextItemId,
    },
    Duplicate {
        winner_id: crate::ai::staged_context::ContextItemId,
    },
    Failed {
        failure: crate::ai::reliability::AppFailureRecord,
    },
}

impl NotesContextStageOutcome {
    pub(crate) fn is_consumable(&self) -> bool {
        matches!(self, Self::Accepted { .. } | Self::Duplicate { .. })
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Accepted { .. } => "accepted",
            Self::Duplicate { .. } => "duplicate",
            Self::Failed { .. } => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NotesSupplementStageOutcome {
    pub(crate) cart_item_id: String,
    pub(crate) idempotency_key: String,
    pub(crate) outcome: NotesContextStageOutcome,
}

/// Typed main-host result. The Notes window consumes cart rows only from the
/// per-item outcomes; a successful open alone is never sufficient evidence.
#[derive(Debug, Clone)]
pub(crate) struct NotesAiMainHandoffOutcome {
    pub(crate) request_id: String,
    pub(crate) primary: NotesContextStageOutcome,
    pub(crate) supplements: Vec<NotesSupplementStageOutcome>,
    pub(crate) destination_thread_id: Option<String>,
    pub(crate) destination_generation: u64,
    pub(crate) reused_existing_chat: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct NotesAiMainHandoffFailure {
    pub(crate) kind: NotesAiHandoffError,
    pub(crate) failure: crate::ai::reliability::AppFailureRecord,
}

impl NotesAiMainHandoffFailure {
    pub(crate) fn new(kind: NotesAiHandoffError, detail: &str) -> Self {
        Self {
            kind,
            failure: crate::ai::reliability::context_unavailable_failure(detail),
        }
    }
}

/// Fully materialized handoff payload. Built BEFORE touching the main window
/// so no later closure re-reads the Notes editor or cart.
#[derive(Clone)]
pub(crate) struct NotesAiHandoffPayload {
    pub(crate) request_id: String,
    pub(crate) primary: crate::ai::staged_context::StagedContextItem,
    pub(crate) scope: crate::notes::ai_scope::NotesAiScope,
    pub(crate) return_snapshot: NotesHostReturnSnapshot,
    pub(crate) supplements: Vec<NotesHandoffAttachment>,
    pub(crate) cart_note_id: Option<crate::notes::model::NoteId>,
    pub(crate) source: &'static str,
    pub(crate) is_draft: bool,
}

impl NotesAiHandoffPayload {
    pub(crate) fn target(&self) -> &crate::ai::TabAiTargetContext {
        match &self.primary.part {
            crate::ai::message_parts::AiContextPart::FocusedTarget { target, .. } => target,
            _ => unreachable!("Notes primary handoff item must be a FocusedTarget"),
        }
    }
}

fn consumable_cart_item_ids(
    attachments: &[NotesHandoffAttachment],
    outcome: &NotesAiMainHandoffOutcome,
) -> Vec<String> {
    if !outcome.primary.is_consumable() {
        return Vec::new();
    }
    let mut seen = std::collections::HashSet::new();
    outcome
        .supplements
        .iter()
        .filter(|item| item.outcome.is_consumable())
        .filter(|item| {
            attachments.iter().any(|attachment| {
                attachment.cart_item_id == item.cart_item_id
                    && attachment.idempotency_key == item.idempotency_key
            })
        })
        .filter(|item| seen.insert(item.cart_item_id.clone()))
        .map(|item| item.cart_item_id.clone())
        .collect()
}

/// Host-owned return token for Notes → main Agent Chat → Notes.
///
/// The token carries only redacted identity and UI anchors. The live Notes
/// entity remains authoritative and is never reconstructed from this data.
/// Its instance/window/focus generations make delayed return callbacks fail
/// closed instead of focusing a newly opened Notes window or a newer panel.
#[derive(Clone, PartialEq)]
pub(crate) struct NotesHostReturnSnapshot {
    pub(crate) notes_instance_id: u64,
    pub(crate) window_generation: u64,
    pub(crate) focus_generation: u64,
    pub(crate) document_id: String,
    pub(crate) document_generation: String,
    pub(crate) content_length: usize,
    pub(crate) content_fingerprint: String,
    pub(crate) dirty: bool,
    pub(crate) selection: std::ops::Range<usize>,
    pub(crate) scroll_top: Option<f32>,
    pub(crate) mode: String,
    pub(crate) alias_id_fingerprints: Vec<String>,
    pub(crate) search_query_fingerprint: String,
    pub(crate) selected_result_id: Option<String>,
    pub(crate) search_scroll_anchor: Option<String>,
    pub(crate) focus_semantic_id: &'static str,
}

impl std::fmt::Debug for NotesHostReturnSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotesHostReturnSnapshot")
            .field("notes_instance_id", &self.notes_instance_id)
            .field("window_generation", &self.window_generation)
            .field("focus_generation", &self.focus_generation)
            .field("document_id_length", &self.document_id.chars().count())
            .field("document_generation", &self.document_generation)
            .field("content_length", &self.content_length)
            .field("content_fingerprint", &self.content_fingerprint)
            .field("dirty", &self.dirty)
            .field("selection_length", &self.selection.len())
            .field("scroll_top", &self.scroll_top)
            .field("mode", &self.mode)
            .field("alias_count", &self.alias_id_fingerprints.len())
            .field("search_query_fingerprint", &self.search_query_fingerprint)
            .field("has_selected_result", &self.selected_result_id.is_some())
            .field(
                "has_search_scroll_anchor",
                &self.search_scroll_anchor.is_some(),
            )
            .field("focus_semantic_id", &self.focus_semantic_id)
            .finish()
    }
}

/// Outcome states for the redacted receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotesAiHandoffStatus {
    Staged,
    /// Primary staged and some cart rows remained because their individual
    /// context items were refused.
    StagedPartial,
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
            Self::StagedPartial => "stagedPartial",
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
    pub(crate) scope_kind: &'static str,
    pub(crate) scope_document_id_length: usize,
    pub(crate) scope_document_id_fingerprint: String,
    pub(crate) scope_range_length: Option<usize>,
    pub(crate) content_length: usize,
    pub(crate) content_fingerprint: String,
    pub(crate) is_draft: bool,
    pub(crate) supplemental_part_count: usize,
    pub(crate) supplemental_accepted_count: usize,
    pub(crate) supplemental_duplicate_count: usize,
    pub(crate) supplemental_failed_count: usize,
    pub(crate) cart_consumed_count: usize,
    pub(crate) destination_window_id: &'static str,
    pub(crate) destination_surface: &'static str,
    pub(crate) staging_outcome: &'static str,
    pub(crate) return_route: &'static str,
    pub(crate) notes_instance_id: u64,
    pub(crate) window_generation: u64,
    pub(crate) focus_generation: u64,
    pub(crate) alias_count: usize,
    pub(crate) error_code: Option<&'static str>,
    pub(crate) recorded_at: std::time::Instant,
}

/// Binary-registered hook that stages a Notes handoff into the main
/// `ScriptListApp` and activates the main window on success. The main app
/// type lives in the binary crate, so this dual-compiled file cannot
/// downcast the main window's root view itself; the binary registers the
/// downcast-and-stage closure at app startup
/// (`register_notes_ai_main_handoff_hook`).
pub type NotesAiMainHandoffHook =
    fn(
        NotesAiHandoffPayload,
        &mut gpui::App,
    ) -> Result<NotesAiMainHandoffOutcome, NotesAiMainHandoffFailure>;

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
            "scope": "wholeNote",
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

    fn build_notes_host_return_snapshot(
        &self,
        target: &crate::ai::TabAiTargetContext,
        cx: &Context<Self>,
    ) -> NotesHostReturnSnapshot {
        let editor = self.editor_state.read(cx);
        let content = editor.value();
        let selection = editor.selection();
        let scroll = editor.automation_scroll_metrics();
        let mut alias_id_fingerprints = self
            .spine_runtime
            .mention_aliases
            .keys()
            .map(|token| fnv1a64_fingerprint(token))
            .collect::<Vec<_>>();
        alias_id_fingerprints.sort();

        let focus_semantic_id = match self.current_focus_surface() {
            super::focus::NotesFocusSurface::Editor => "input:notes-editor",
            super::focus::NotesFocusSurface::Preview => "notes-preview",
            super::focus::NotesFocusSurface::ActionsPanel => "notes-actions",
            super::focus::NotesFocusSurface::BrowsePanel => "notes-switcher",
            super::focus::NotesFocusSurface::Dialog => "notes-dialog",
        };
        let content_fingerprint = fnv1a64_fingerprint(&content);

        NotesHostReturnSnapshot {
            notes_instance_id: self.instance_id,
            window_generation: self.entry_reveal.generation,
            focus_generation: self.focus_transition_generation,
            document_id: target.semantic_id.clone(),
            document_generation: content_fingerprint.clone(),
            content_length: content.chars().count(),
            content_fingerprint,
            dirty: self.has_unsaved_changes,
            selection,
            scroll_top: scroll
                .get("scrollTop")
                .and_then(serde_json::Value::as_f64)
                .map(|value| value as f32),
            mode: format!("{:?}", self.view_mode),
            alias_id_fingerprints,
            search_query_fingerprint: fnv1a64_fingerprint(&self.search_query),
            selected_result_id: self
                .selected_note_id
                .map(|id| format!("note:{}", id.as_str())),
            search_scroll_anchor: None,
            focus_semantic_id,
        }
    }

    /// Materialize the complete immutable handoff payload: primary note item,
    /// generation-guarded Notes return token, and one explicit mapping for
    /// every persisted cart row. No main-window state is touched until this
    /// succeeds, and no later closure re-reads the editor or cart.
    pub(crate) fn build_notes_ai_handoff_payload(
        &self,
        source: &'static str,
        cx: &Context<Self>,
    ) -> Result<NotesAiHandoffPayload, NotesAiHandoffError> {
        let (target, is_draft) = self.build_selected_note_ai_target(cx)?;
        let cart_note_id = self.selected_note_id;
        let mut supplements = Vec::new();

        if let Some(note_id) = cart_note_id {
            let items = crate::notes::storage::list_note_cart_items(note_id).map_err(|error| {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "notes_ai_handoff_cart_load_failed",
                    note_id = %note_id,
                    error = %error,
                );
                NotesAiHandoffError::CartLoadFailed
            })?;
            supplements = items
                .into_iter()
                .map(|item| {
                    let context_part = item.to_ai_context_part();
                    let cart_item_id = item.id;
                    let idempotency_key = format!("notes-cart:{}:{cart_item_id}", note_id.as_str());
                    NotesHandoffAttachment {
                        cart_item_id,
                        context_item: crate::ai::staged_context::StagedContextItem::pending(
                            context_part,
                            crate::ai::staged_context::ContextProvenance::HostHandoff,
                            crate::ai::staged_context::ContextRole::Supplemental,
                        ),
                        idempotency_key,
                    }
                })
                .collect();
        }

        let scope = crate::notes::ai_scope::NotesAiScope::WholeNote {
            document_id: target.semantic_id.clone(),
        };
        let return_snapshot = self.build_notes_host_return_snapshot(&target, cx);
        let label = crate::ai::format_explicit_target_chip_label(&target);
        let primary = crate::ai::staged_context::StagedContextItem::pending(
            crate::ai::message_parts::AiContextPart::FocusedTarget { target, label },
            crate::ai::staged_context::ContextProvenance::HostHandoff,
            crate::ai::staged_context::ContextRole::Primary,
        );
        Ok(NotesAiHandoffPayload {
            request_id: next_notes_handoff_request_id(),
            primary,
            scope,
            return_snapshot,
            supplements,
            cart_note_id,
            source,
            is_draft,
        })
    }

    fn consume_staged_cart_rows(
        payload: &NotesAiHandoffPayload,
        outcome: &NotesAiMainHandoffOutcome,
    ) -> (NotesAiHandoffStatus, usize) {
        if !outcome.primary.is_consumable() {
            return (NotesAiHandoffStatus::Failed, 0);
        }
        let consumable_ids = consumable_cart_item_ids(&payload.supplements, outcome);
        let explicit_failed_count = outcome
            .supplements
            .iter()
            .filter(|item| !item.outcome.is_consumable())
            .count();
        let reported_ids = outcome
            .supplements
            .iter()
            .filter(|item| {
                payload.supplements.iter().any(|attachment| {
                    attachment.cart_item_id == item.cart_item_id
                        && attachment.idempotency_key == item.idempotency_key
                })
            })
            .map(|item| item.cart_item_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let failed_count =
            explicit_failed_count + payload.supplements.len().saturating_sub(reported_ids.len());
        if consumable_ids.is_empty() {
            return (
                if failed_count == 0 {
                    NotesAiHandoffStatus::Staged
                } else {
                    NotesAiHandoffStatus::StagedPartial
                },
                0,
            );
        }
        let Some(note_id) = payload.cart_note_id else {
            return (NotesAiHandoffStatus::StagedCartRetained, 0);
        };
        match crate::notes::storage::delete_note_cart_items(note_id, &consumable_ids) {
            Ok(consumed) => (
                if failed_count == 0 {
                    NotesAiHandoffStatus::Staged
                } else {
                    NotesAiHandoffStatus::StagedPartial
                },
                consumed,
            ),
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "notes_ai_handoff_cart_consume_failed",
                    note_id = %note_id,
                    attempted_count = consumable_ids.len(),
                    error = %error,
                );
                (NotesAiHandoffStatus::StagedCartRetained, 0)
            }
        }
    }

    fn finish_successful_ai_handoff(
        &mut self,
        payload: &NotesAiHandoffPayload,
        outcome: &NotesAiMainHandoffOutcome,
    ) -> bool {
        if outcome.request_id != payload.request_id {
            self.record_ai_handoff_receipt(
                Some(payload),
                None,
                0,
                payload.source,
                NotesAiHandoffStatus::Failed,
                Some(NotesAiHandoffError::MainStagingFailed.code()),
            );
            self.show_action_feedback("Agent unavailable", true);
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "notes_ai_handoff_outcome_request_mismatch",
                expected_request_id = %payload.request_id,
                received_request_id = %outcome.request_id,
            );
            return false;
        }
        if !outcome.primary.is_consumable() {
            self.record_ai_handoff_receipt(
                Some(payload),
                Some(outcome),
                0,
                payload.source,
                NotesAiHandoffStatus::Failed,
                Some(NotesAiHandoffError::MainStagingFailed.code()),
            );
            self.show_action_feedback("Agent unavailable", true);
            return false;
        }
        let (status, consumed_count) = Self::consume_staged_cart_rows(payload, outcome);
        self.record_ai_handoff_receipt(
            Some(payload),
            Some(outcome),
            consumed_count,
            payload.source,
            status,
            None,
        );
        match status {
            NotesAiHandoffStatus::Staged => {
                self.show_action_feedback("Opened in main Agent Chat", false)
            }
            NotesAiHandoffStatus::StagedPartial => {
                self.show_action_feedback("Opened in Agent Chat; some attachments remain", true)
            }
            NotesAiHandoffStatus::StagedCartRetained => {
                self.show_action_feedback("Opened in Agent Chat; cart was not cleared", true)
            }
            NotesAiHandoffStatus::Blocked | NotesAiHandoffStatus::Failed => {
                self.show_action_feedback("Agent unavailable", true)
            }
        }
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_ai_handoff_main_staged",
            source = payload.source,
            request_id = %payload.request_id,
            status = status.as_str(),
            primary_outcome = outcome.primary.kind(),
            supplemental_count = outcome.supplements.len(),
            consumed_count,
        );
        true
    }

    /// The ONE notes→AI command. Every affordance (Cmd+Enter, the Actions
    /// descriptor, the titlebar Ask AI button, and the automation key route)
    /// funnels here. Cmd+Shift+A is intentionally unowned.
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
                    None,
                    0,
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
            request_id = %payload.request_id,
            target_semantic_id = %payload.target().semantic_id,
            is_draft = payload.is_draft,
            supplemental_part_count = payload.supplements.len(),
        );

        match Self::dispatch_notes_ai_handoff_to_main(payload.clone(), cx) {
            Ok(outcome) => {
                let finished = self.finish_successful_ai_handoff(&payload, &outcome);
                cx.notify();
                finished
            }
            Err(error) => {
                // A missing/re-entrant main handle is the sole retryable host
                // boundary. The exact immutable payload (including request and
                // context item IDs) is retried once; no editor/cart reread.
                if error.kind == NotesAiHandoffError::MainWindowUnavailable {
                    if !crate::is_main_window_visible() {
                        crate::request_show_main_window();
                    }
                    self.schedule_ai_handoff_retry(payload, cx);
                    return false;
                }
                self.record_ai_handoff_receipt(
                    Some(&payload),
                    None,
                    0,
                    source,
                    NotesAiHandoffStatus::Failed,
                    Some(NotesAiHandoffError::MainStagingFailed.code()),
                );
                self.show_action_feedback("Agent unavailable", true);
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "notes_ai_handoff_dispatch_failed",
                    source,
                    failure_code = ?error.failure.failure.code,
                    diagnostic_fingerprint = ?error.failure.failure.diagnostic.as_ref().map(|diagnostic| &diagnostic.fingerprint),
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
                        Ok(outcome) => {
                            app.finish_successful_ai_handoff(&payload, &outcome);
                            tracing::info!(
                                target: "script_kit::tab_ai",
                                event = "notes_ai_handoff_retry_completed",
                                source,
                                request_id = %payload.request_id,
                            );
                        }
                        Err(error) => {
                            app.record_ai_handoff_receipt(
                                Some(&payload),
                                None,
                                0,
                                source,
                                NotesAiHandoffStatus::Failed,
                                Some(NotesAiHandoffError::MainWindowUnavailable.code()),
                            );
                            app.show_action_feedback("Agent unavailable", true);
                            tracing::warn!(
                                target: "script_kit::tab_ai",
                                event = "notes_ai_handoff_retry_failed",
                                source,
                                failure_code = ?error.failure.failure.code,
                                diagnostic_fingerprint = ?error.failure.failure.diagnostic.as_ref().map(|diagnostic| &diagnostic.fingerprint),
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
    ) -> Result<NotesAiMainHandoffOutcome, NotesAiMainHandoffFailure> {
        if crate::get_main_window_handle().is_none() {
            return Err(NotesAiMainHandoffFailure::new(
                NotesAiHandoffError::MainWindowUnavailable,
                "notes_main_window_handle_missing",
            ));
        }
        let Some(hook) = NOTES_AI_MAIN_HANDOFF_HOOK.get() else {
            return Err(NotesAiMainHandoffFailure::new(
                NotesAiHandoffError::MainStagingFailed,
                "notes_ai_handoff_hook_unregistered",
            ));
        };
        hook(payload, cx)
    }

    /// Record the redacted receipt (also projected into devtools state).
    fn record_ai_handoff_receipt(
        &mut self,
        payload: Option<&NotesAiHandoffPayload>,
        main_outcome: Option<&NotesAiMainHandoffOutcome>,
        cart_consumed_count: usize,
        source: &'static str,
        status: NotesAiHandoffStatus,
        error_code: Option<&'static str>,
    ) {
        self.ai_handoff_generation = self.ai_handoff_generation.wrapping_add(1);
        let (
            semantic_id,
            label,
            scope_kind,
            scope_document_id,
            scope_range_length,
            is_draft,
            part_count,
        ) = match payload {
            Some(payload) => (
                payload.target().semantic_id.clone(),
                payload.target().label.clone(),
                payload.scope.kind(),
                payload.scope.document_semantic_id().to_string(),
                payload.scope.range_length(),
                payload.is_draft,
                payload.supplements.len(),
            ),
            None => (
                String::new(),
                String::new(),
                "none",
                String::new(),
                None,
                false,
                0,
            ),
        };
        let return_snapshot = payload.map(|payload| &payload.return_snapshot);
        let supplemental_accepted_count = main_outcome.map_or(0, |outcome| {
            outcome
                .supplements
                .iter()
                .filter(|item| matches!(item.outcome, NotesContextStageOutcome::Accepted { .. }))
                .count()
        });
        let supplemental_duplicate_count = main_outcome.map_or(0, |outcome| {
            outcome
                .supplements
                .iter()
                .filter(|item| matches!(item.outcome, NotesContextStageOutcome::Duplicate { .. }))
                .count()
        });
        let supplemental_failed_count = main_outcome.map_or(0, |outcome| {
            outcome
                .supplements
                .iter()
                .filter(|item| matches!(item.outcome, NotesContextStageOutcome::Failed { .. }))
                .count()
        });
        self.last_ai_handoff = Some(NotesAiHandoffReceipt {
            generation: self.ai_handoff_generation,
            source,
            status,
            target_semantic_id: semantic_id,
            target_label_length: label.chars().count(),
            target_label_fingerprint: fnv1a64_fingerprint(&label),
            scope_kind,
            scope_document_id_length: scope_document_id.chars().count(),
            scope_document_id_fingerprint: fnv1a64_fingerprint(&scope_document_id),
            scope_range_length,
            content_length: return_snapshot.map_or(0, |snapshot| snapshot.content_length),
            content_fingerprint: return_snapshot
                .map(|snapshot| snapshot.content_fingerprint.clone())
                .unwrap_or_default(),
            is_draft,
            supplemental_part_count: part_count,
            supplemental_accepted_count,
            supplemental_duplicate_count,
            supplemental_failed_count,
            cart_consumed_count,
            destination_window_id: "main",
            destination_surface: "agentChat",
            staging_outcome: if matches!(
                status,
                NotesAiHandoffStatus::Staged
                    | NotesAiHandoffStatus::StagedPartial
                    | NotesAiHandoffStatus::StagedCartRetained
            ) {
                "composerOnly"
            } else {
                "notStaged"
            },
            return_route: if payload.is_some() { "notes" } else { "none" },
            notes_instance_id: self.instance_id,
            window_generation: return_snapshot.map_or(0, |snapshot| snapshot.window_generation),
            focus_generation: return_snapshot.map_or(0, |snapshot| snapshot.focus_generation),
            alias_count: return_snapshot.map_or(0, |snapshot| snapshot.alias_id_fingerprints.len()),
            error_code,
            recorded_at: std::time::Instant::now(),
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotesHostReturnDecision {
    Restore,
    StaleInstance,
    StaleWindow,
    StaleFocus,
}

fn notes_host_return_decision(
    snapshot: &NotesHostReturnSnapshot,
    instance_id: u64,
    window_generation: u64,
    focus_generation: u64,
) -> NotesHostReturnDecision {
    if snapshot.notes_instance_id != instance_id {
        NotesHostReturnDecision::StaleInstance
    } else if snapshot.window_generation != window_generation {
        NotesHostReturnDecision::StaleWindow
    } else if snapshot.focus_generation != focus_generation {
        NotesHostReturnDecision::StaleFocus
    } else {
        NotesHostReturnDecision::Restore
    }
}

/// Restore the exact Notes window that originated a main Agent Chat handoff.
/// A delayed callback never opens Notes and never targets a replacement
/// instance: every generation must still match the host-owned snapshot.
pub(crate) fn restore_notes_host_return(
    snapshot: NotesHostReturnSnapshot,
    cx: &mut gpui::App,
) -> bool {
    let Some((entity, handle)) = super::get_notes_app_entity_and_handle() else {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_ai_return_ignored",
            reason = "notesWindowClosed",
        );
        return false;
    };

    super::update_notes_window_detached(handle, cx, |window, cx| {
        entity.update(cx, |app, cx| {
            let decision = notes_host_return_decision(
                &snapshot,
                app.instance_id,
                app.entry_reveal.generation,
                app.focus_transition_generation,
            );
            if decision != NotesHostReturnDecision::Restore {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "notes_ai_return_ignored",
                    reason = ?decision,
                    snapshot_instance_id = snapshot.notes_instance_id,
                    live_instance_id = app.instance_id,
                );
                return false;
            }

            let surface = match snapshot.focus_semantic_id {
                "notes-preview" => super::focus::NotesFocusSurface::Preview,
                "notes-actions" => super::focus::NotesFocusSurface::ActionsPanel,
                "notes-switcher" => super::focus::NotesFocusSurface::BrowsePanel,
                "notes-dialog" => super::focus::NotesFocusSurface::Dialog,
                _ => super::focus::NotesFocusSurface::Editor,
            };
            window.activate_window();
            app.request_focus_surface(surface, window, cx);
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "notes_ai_return_restored",
                notes_instance_id = app.instance_id,
                focus_semantic_id = snapshot.focus_semantic_id,
            );
            true
        })
    })
    .unwrap_or_else(|error| {
        tracing::warn!(
            target: "script_kit::tab_ai",
            event = "notes_ai_return_failed",
            error = ?error,
        );
        false
    })
}

#[cfg(test)]
mod tests {
    use super::{
        compose_note_ai_target, consumable_cart_item_ids, fnv1a64_fingerprint,
        notes_host_return_decision, NotesAiHandoffError, NotesAiMainHandoffOutcome,
        NotesAiTargetSnapshot, NotesContextStageOutcome, NotesHandoffAttachment,
        NotesHostReturnDecision, NotesHostReturnSnapshot, NotesSupplementStageOutcome,
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

    fn return_snapshot() -> NotesHostReturnSnapshot {
        NotesHostReturnSnapshot {
            notes_instance_id: 7,
            window_generation: 11,
            focus_generation: 13,
            document_id: "note:semantic".to_string(),
            document_generation: "fnv1a64:document".to_string(),
            content_length: 42,
            content_fingerprint: "fnv1a64:content".to_string(),
            dirty: true,
            selection: 4..9,
            scroll_top: Some(12.0),
            mode: "AllNotes".to_string(),
            alias_id_fingerprints: vec!["fnv1a64:alias".to_string()],
            search_query_fingerprint: "fnv1a64:query".to_string(),
            selected_result_id: Some("note:selected".to_string()),
            search_scroll_anchor: None,
            focus_semantic_id: "input:notes-editor",
        }
    }

    #[test]
    fn host_return_snapshot_rejects_every_stale_generation() {
        let snapshot = return_snapshot();
        assert_eq!(
            notes_host_return_decision(&snapshot, 7, 11, 13),
            NotesHostReturnDecision::Restore,
        );
        assert_eq!(
            notes_host_return_decision(&snapshot, 8, 11, 13),
            NotesHostReturnDecision::StaleInstance,
        );
        assert_eq!(
            notes_host_return_decision(&snapshot, 7, 12, 13),
            NotesHostReturnDecision::StaleWindow,
        );
        assert_eq!(
            notes_host_return_decision(&snapshot, 7, 11, 14),
            NotesHostReturnDecision::StaleFocus,
        );
    }

    #[test]
    fn host_return_snapshot_debug_is_redacted() {
        let debug = format!("{:?}", return_snapshot());
        assert!(!debug.contains("note:semantic"));
        assert!(!debug.contains("note:selected"));
        assert!(debug.contains("document_id_length"));
        assert!(debug.contains("selection_length"));
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

    fn context_id(value: &str) -> crate::ai::staged_context::ContextItemId {
        crate::ai::staged_context::ContextItemId(value.to_string())
    }

    fn outcome(
        primary: NotesContextStageOutcome,
        supplements: Vec<NotesSupplementStageOutcome>,
    ) -> NotesAiMainHandoffOutcome {
        NotesAiMainHandoffOutcome {
            request_id: "request-1".to_string(),
            primary,
            supplements,
            destination_thread_id: Some("thread-1".to_string()),
            destination_generation: 7,
            reused_existing_chat: true,
        }
    }

    fn attachment(cart_item_id: &str, idempotency_key: &str) -> NotesHandoffAttachment {
        NotesHandoffAttachment {
            cart_item_id: cart_item_id.to_string(),
            context_item: crate::ai::staged_context::StagedContextItem::pending(
                crate::ai::message_parts::AiContextPart::TextBlock {
                    label: "Synthetic".to_string(),
                    source: format!("test://{cart_item_id}"),
                    text: "synthetic".to_string(),
                    mime_type: Some("text/plain".to_string()),
                },
                crate::ai::staged_context::ContextProvenance::HostHandoff,
                crate::ai::staged_context::ContextRole::Supplemental,
            ),
            idempotency_key: idempotency_key.to_string(),
        }
    }

    #[test]
    fn cart_consumption_requires_primary_and_uses_per_item_outcomes() {
        let accepted = NotesSupplementStageOutcome {
            cart_item_id: "accepted-row".to_string(),
            idempotency_key: "key-accepted".to_string(),
            outcome: NotesContextStageOutcome::Accepted {
                context_item_id: context_id("context-a"),
            },
        };
        let duplicate = NotesSupplementStageOutcome {
            cart_item_id: "duplicate-row".to_string(),
            idempotency_key: "key-duplicate".to_string(),
            outcome: NotesContextStageOutcome::Duplicate {
                winner_id: context_id("context-existing"),
            },
        };
        let failed = NotesSupplementStageOutcome {
            cart_item_id: "failed-row".to_string(),
            idempotency_key: "key-failed".to_string(),
            outcome: NotesContextStageOutcome::Failed {
                failure: crate::ai::reliability::context_unavailable_failure(
                    "synthetic_attachment_failure",
                ),
            },
        };
        let attachments = vec![
            attachment("accepted-row", "key-accepted"),
            attachment("duplicate-row", "key-duplicate"),
            attachment("failed-row", "key-failed"),
        ];
        let staged = outcome(
            NotesContextStageOutcome::Accepted {
                context_item_id: context_id("primary"),
            },
            vec![accepted.clone(), duplicate.clone(), failed.clone()],
        );
        assert_eq!(
            consumable_cart_item_ids(&attachments, &staged),
            vec!["accepted-row".to_string(), "duplicate-row".to_string()],
            "failed rows remain in the cart while accepted and canonical duplicates consume",
        );
        let forged = outcome(
            NotesContextStageOutcome::Accepted {
                context_item_id: context_id("primary"),
            },
            vec![NotesSupplementStageOutcome {
                cart_item_id: "accepted-row".to_string(),
                idempotency_key: "wrong-key".to_string(),
                outcome: NotesContextStageOutcome::Accepted {
                    context_item_id: context_id("forged"),
                },
            }],
        );
        assert!(
            consumable_cart_item_ids(&attachments, &forged).is_empty(),
            "a mismatched idempotency mapping must not consume a cart row",
        );

        let primary_failed = outcome(
            NotesContextStageOutcome::Failed {
                failure: crate::ai::reliability::context_unavailable_failure(
                    "synthetic_primary_failure",
                ),
            },
            vec![accepted, duplicate, failed],
        );
        assert!(
            consumable_cart_item_ids(&attachments, &primary_failed).is_empty(),
            "primary failure is atomic and consumes no cart row",
        );
    }

    #[test]
    fn attachment_debug_redacts_cart_and_idempotency_values() {
        let attachment = NotesHandoffAttachment {
            cart_item_id: "PRIVATE_CART_ROW".to_string(),
            context_item: crate::ai::staged_context::StagedContextItem::pending(
                crate::ai::message_parts::AiContextPart::TextBlock {
                    label: "Synthetic".to_string(),
                    source: "synthetic://attachment".to_string(),
                    text: "private body".to_string(),
                    mime_type: Some("text/plain".to_string()),
                },
                crate::ai::staged_context::ContextProvenance::HostHandoff,
                crate::ai::staged_context::ContextRole::Supplemental,
            ),
            idempotency_key: "PRIVATE_IDEMPOTENCY_KEY".to_string(),
        };
        let debug = format!("{attachment:?}");
        assert!(!debug.contains("PRIVATE_CART_ROW"));
        assert!(!debug.contains("PRIVATE_IDEMPOTENCY_KEY"));
        assert!(!debug.contains("private body"));
        assert!(debug.contains("cart_item_id_length"));
        assert!(debug.contains("idempotency_key_fingerprint"));
    }
}
