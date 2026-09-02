// Day Page view entity (included before AppView so the enum can hold Entity<DayPageView>).

use std::path::PathBuf;

use gpui::WeakEntity;

use crate::components::notes_editor::NotesEditor;
use script_kit_gpui::day_page::DayPageDocumentSession;

pub(crate) const DAY_PAGE_EDITOR_ID: &str = "day-page-editor";

/// Minimal Day Page state for accepted `@` mention aliases. It intentionally
/// does not carry list rows, selection, hover, submit anchors, or dismissed-cache
/// state because Day must not own a local inline Spine UI or Agent handoff.
#[derive(Default)]
pub(crate) struct DayPageSpineHandoffState {
    pub(crate) mention_aliases:
        std::collections::HashMap<String, crate::ai::message_parts::AiContextPart>,
}

impl DayPageSpineHandoffState {
    pub(crate) fn reset(&mut self, _clear_cwd_anchor: bool, clear_mentions: bool) {
        if clear_mentions {
            self.mention_aliases.clear();
        }
    }

    pub(crate) fn register_mention_alias(
        &mut self,
        token: String,
        part: crate::ai::message_parts::AiContextPart,
    ) {
        self.mention_aliases.insert(token, part);
    }

    pub(crate) fn prune_mention_aliases_for_content(&mut self, content: &str) {
        crate::components::notes_editor::spine::prune_mention_aliases(
            &mut self.mention_aliases,
            content,
        );
    }

    pub(crate) fn sync_with_markdown_references(&mut self, content: &str) {
        let inline_tokens = crate::ai::context_mentions::inline_token_spans(content)
            .into_iter()
            .map(|span| span.token)
            .collect::<std::collections::HashSet<_>>();
        let previous = std::mem::take(&mut self.mention_aliases);
        self.mention_aliases = day_page_context_reference_aliases_from_markdown(content);
        for (token, part) in previous {
            if inline_tokens.contains(&token) {
                self.mention_aliases.insert(token, part);
            }
        }
    }

    pub(crate) fn ledger_state(&self, content: &str) -> serde_json::Value {
        let markdown_reference_count = day_page_context_reference_spans(content).len();
        let inline_reference_count = crate::ai::context_mentions::inline_token_spans(content)
            .into_iter()
            .filter(|span| self.mention_aliases.contains_key(&span.token))
            .count();
        serde_json::json!({
            "schemaVersion": 1,
            "source": "runtime.dayPage.contextReferenceLedger",
            "redacted": true,
            "aliasCount": self.mention_aliases.len(),
            "markdownReferenceCount": markdown_reference_count,
            "inlineReferenceCount": inline_reference_count,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DayPageKitResourcePreviewState {
    pub(crate) title: String,
    pub(crate) uri: String,
    pub(crate) mime_type: String,
    pub(crate) text: String,
    pub(crate) truncated: bool,
    pub(crate) allow_agent_chat_action: bool,
}

impl DayPageKitResourcePreviewState {
    pub(crate) fn from_preview(
        preview: crate::notes::deeplink_activation::KitResourcePreview,
        allow_agent_chat_action: bool,
    ) -> Self {
        Self {
            title: preview.title,
            uri: preview.uri,
            mime_type: preview.mime_type,
            text: preview.text,
            truncated: preview.truncated,
            allow_agent_chat_action,
        }
    }
}

/// One clipboard shelf row: the lifted day-file line plus a short content
/// preview resolved from clipboard history (UI-only — the preview text is
/// never persisted to the brain, keeping day files raw-free).
#[derive(Debug, Clone)]
pub(crate) struct DayPageClipboardShelfEntry {
    pub(crate) item: script_kit_gpui::day_page::ClipboardShelfItem,
    pub(crate) preview: String,
}

pub(crate) struct DayPageInitialData {
    pub session: DayPageDocumentSession,
    pub now: Option<chrono::DateTime<chrono::Utc>>,
    pub shelf_previews: Option<std::collections::HashMap<String, String>>,
    pub host_policy: crate::runtime_policy::WindowHostPolicy,
}

/// Host for today's day page inside the main launcher window.
pub struct DayPageView {
    pub(crate) app: WeakEntity<ScriptListApp>,
    /// Stable identity of this exact held Day Page entity for return routing.
    pub(crate) instance_id: u64,
    /// Monotonic host-return generation; delayed callbacks must match it.
    pub(crate) host_return_generation: u64,
    pub(crate) session: DayPageDocumentSession,
    pub(crate) clock_now: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) fixture_shelf_previews: Option<std::collections::HashMap<String, String>>,
    pub(crate) theme_revision_seen: u64,
    pub(crate) document_revision: u64,
    pub(crate) loaded_document: Option<(script_kit_gpui::day_page::DayPageBinding, Option<PathBuf>)>,
    pub(crate) notes_editor: Entity<NotesEditor>,
    pub(crate) editor_state: Entity<InputState>,
    pub(crate) editor_subscription: Subscription,
    /// Observes the editor `InputState` for deeplink hover changes, which
    /// arrive as plain `cx.notify` (no `InputEvent`) — required for the
    /// hover hint chip to appear/disappear live.
    pub(crate) editor_hover_observation: Subscription,
    /// Render receipt: the deeplink hover hint chip built during the last
    /// render (`{"verb", "href"}`), or `None` when no chip was rendered.
    /// Written only on the render path so automation can prove the
    /// hover→re-render→chip pipeline, not just the hover state.
    pub(crate) last_deeplink_hover_hint: Option<serde_json::Value>,
    pub(crate) focus_handle: FocusHandle,
    /// Resolved fragment paths aligned with parsed fragment reference indices.
    pub(crate) fragment_open_targets: Vec<PathBuf>,
    pub(crate) spine_handoff: DayPageSpineHandoffState,
    /// Last debounced autosave write (Notes-parity SAVE_DEBOUNCE_MS throttle).
    pub(crate) last_autosave: Option<std::time::Instant>,

    /// Throttle for the external-disk poll: `render` runs it every frame, but
    /// the underlying `fs::metadata` stat only needs to run a few times a
    /// second to catch background appends.
    pub(crate) last_external_poll: Option<std::time::Instant>,
    /// True while a trailing autosave flush timer is pending.
    pub(crate) autosave_flush_scheduled: bool,
    /// Shared Notes Cmd+P switcher component hosted by Day Page (the live
    /// day/note switcher — opened as a centered popup).
    pub(crate) note_switcher: crate::actions::CommandBar,
    /// Editor byte length at the last observed change. The `@context`
    /// main-menu swap only triggers on growth so deleting inside an existing
    /// mention never re-opens the search (day_page_context_round_trip.rs).
    pub(crate) last_editor_content_len: usize,
    /// Read-only preview opened from a `kit://` resource link in Day Page markdown.
    pub(crate) kit_resource_preview: Option<DayPageKitResourcePreviewState>,
    /// Clipboard sediment refs lifted out of the editor buffer (Day binding
    /// only). They stay in the day file — rejoined at the end on every save —
    /// but render as a compact shelf below the editor instead of raw
    /// `[Clipboard entry](kit://…)` lines inside the note.
    pub(crate) clipboard_shelf: Vec<DayPageClipboardShelfEntry>,
    /// Whether the clipboard shelf list is expanded (collapsed by default).
    pub(crate) clipboard_shelf_expanded: bool,
    /// True when Day Page content is rendered through the shared Notes
    /// Markdown preview renderer instead of the editable Notes editor input.
    pub(crate) read_mode: bool,
    /// Last Day Page → Agent Chat handoff receipt exposed to automation.
    /// Redacted: carries scope/count/hash metadata only, never raw markdown.
    pub(crate) last_agent_chat_handoff_receipt: Option<serde_json::Value>,
    /// Last Today `@context` round-trip receipt exposed to automation.
    /// Redacted: carries counts/ranges/hashes only, never raw context text.
    pub(crate) last_context_round_trip_receipt: Option<serde_json::Value>,
}
