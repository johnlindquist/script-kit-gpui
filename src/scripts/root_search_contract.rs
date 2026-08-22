//! Pure, library-testable ownership of root-launcher provider generations.
//!
//! `RootSearchStore` lives in the application binary because its cached rows
//! depend on binary-only presentation types. Its correctness-critical async
//! request coordinator lives here so the exact production policy is compiled
//! and exercised by `cargo test --lib` as well as by the launcher.

use sk_protocol::command_contract::CommandSource;
use sk_protocol::search_contract::{ProviderGenerationFence, ProviderRequest};

/// Exact ownership of one bounded background provider worker. Source is part
/// of the ticket, so a stale Dictation/Clipboard/Conversation completion can
/// never release or publish another provider's active work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RootOwnedProviderRefresh {
    pub(crate) source: CommandSource,
    pub(crate) generation: u64,
}

#[derive(Debug, Default)]
pub(crate) struct RootOwnedProviderRefreshLifecycle {
    pub(crate) next_generation: u64,
    pub(crate) in_flight: Option<RootOwnedProviderRefresh>,
}

impl RootOwnedProviderRefreshLifecycle {
    pub(crate) fn begin(
        &mut self,
        source: CommandSource,
        cache_is_fresh: bool,
    ) -> Option<RootOwnedProviderRefresh> {
        if cache_is_fresh || self.in_flight.is_some() {
            return None;
        }
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let refresh = RootOwnedProviderRefresh {
            source,
            generation: self.next_generation,
        };
        self.in_flight = Some(refresh);
        Some(refresh)
    }

    pub(crate) fn finish(&mut self, refresh: RootOwnedProviderRefresh) -> bool {
        if self.in_flight != Some(refresh) {
            return false;
        }
        self.in_flight = None;
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RootLocalContentProvider {
    Notes,
    Todos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RootLocalContentOptions {
    Notes(crate::notes::RootNotesSectionOptions),
    Todos(crate::menu_syntax::RootTodoSectionOptions),
}

#[derive(Clone)]
pub(crate) enum RootLocalContentRefresh {
    Notes(crate::notes::RootNotesSearchRefresh),
    Todos(RootOwnedProviderRefresh),
}

pub(crate) enum RootLocalContentSnapshot {
    Notes(crate::notes::RootNotesSearchSnapshot),
    Todos(crate::menu_syntax::RootTodoRefreshSnapshot),
}

impl RootLocalContentProvider {
    pub(crate) fn source(self) -> CommandSource {
        match self {
            Self::Notes => CommandSource::Note,
            Self::Todos => CommandSource::Todo,
        }
    }

    pub(crate) fn source_filter(self) -> crate::menu_syntax::RootUnifiedSourceFilter {
        match self {
            Self::Notes => crate::menu_syntax::RootUnifiedSourceFilter::Notes,
            Self::Todos => crate::menu_syntax::RootUnifiedSourceFilter::Todo,
        }
    }

    pub(crate) fn cache_is_fresh(self, query: &str, options: RootLocalContentOptions) -> bool {
        match (self, options) {
            (Self::Notes, RootLocalContentOptions::Notes(options)) => {
                crate::notes::root_notes_search_cache_is_fresh(query, options)
            }
            (Self::Todos, RootLocalContentOptions::Todos(_)) => {
                crate::menu_syntax::root_todos_snapshot_is_fresh()
            }
            (Self::Notes, RootLocalContentOptions::Todos(_))
            | (Self::Todos, RootLocalContentOptions::Notes(_)) => false,
        }
    }

    pub(crate) fn begin(
        self,
        query: &str,
        options: RootLocalContentOptions,
    ) -> Option<RootLocalContentRefresh> {
        match (self, options) {
            (Self::Notes, RootLocalContentOptions::Notes(options)) => {
                crate::notes::try_begin_root_notes_search_refresh(query, options)
                    .map(RootLocalContentRefresh::Notes)
            }
            (Self::Todos, RootLocalContentOptions::Todos(_)) => {
                crate::menu_syntax::try_begin_root_todos_snapshot_refresh()
                    .map(RootLocalContentRefresh::Todos)
            }
            (Self::Notes, RootLocalContentOptions::Todos(_))
            | (Self::Todos, RootLocalContentOptions::Notes(_)) => None,
        }
    }

    pub(crate) fn read_snapshot(
        self,
        refresh: &RootLocalContentRefresh,
    ) -> Option<RootLocalContentSnapshot> {
        match (self, refresh) {
            (Self::Notes, RootLocalContentRefresh::Notes(refresh)) => {
                Some(RootLocalContentSnapshot::Notes(
                    crate::notes::read_root_notes_search_snapshot(refresh),
                ))
            }
            (Self::Todos, RootLocalContentRefresh::Todos(refresh)) => {
                Some(RootLocalContentSnapshot::Todos(
                    crate::menu_syntax::read_root_todos_snapshot(*refresh),
                ))
            }
            (Self::Notes, RootLocalContentRefresh::Todos(_))
            | (Self::Todos, RootLocalContentRefresh::Notes(_)) => None,
        }
    }

    pub(crate) fn finish(
        self,
        refresh: RootLocalContentRefresh,
        snapshot: RootLocalContentSnapshot,
    ) -> bool {
        match (self, refresh, snapshot) {
            (
                Self::Notes,
                RootLocalContentRefresh::Notes(refresh),
                RootLocalContentSnapshot::Notes(snapshot),
            ) => crate::notes::finish_root_notes_search_refresh(refresh, snapshot),
            (
                Self::Todos,
                RootLocalContentRefresh::Todos(refresh),
                RootLocalContentSnapshot::Todos(snapshot),
            ) => crate::menu_syntax::finish_root_todos_snapshot_refresh(refresh, snapshot),
            _ => false,
        }
    }

    pub(crate) fn discard(self, refresh: &RootLocalContentRefresh) -> bool {
        match (self, refresh) {
            (Self::Notes, RootLocalContentRefresh::Notes(refresh)) => {
                crate::notes::discard_root_notes_search_refresh(refresh.clone())
            }
            (Self::Todos, RootLocalContentRefresh::Todos(refresh)) => {
                crate::menu_syntax::discard_root_todos_snapshot_refresh(*refresh)
            }
            (Self::Notes, RootLocalContentRefresh::Todos(_))
            | (Self::Todos, RootLocalContentRefresh::Notes(_)) => false,
        }
    }

    pub(crate) fn completion_reason(self) -> &'static str {
        match self {
            Self::Notes => "notes_search_refresh_complete",
            Self::Todos => "todos_snapshot_refresh_complete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RootPrivateHistoryProvider {
    Clipboard,
    Dictation,
}

pub(crate) enum RootPrivateHistorySnapshot {
    Clipboard(crate::clipboard_history::RootClipboardHistorySnapshot),
    Dictation(crate::dictation::RootDictationHistorySnapshot),
}

impl RootPrivateHistoryProvider {
    pub(crate) fn source(self) -> CommandSource {
        match self {
            Self::Clipboard => CommandSource::Clipboard,
            Self::Dictation => CommandSource::Dictation,
        }
    }

    pub(crate) fn source_filter(self) -> crate::menu_syntax::RootUnifiedSourceFilter {
        match self {
            Self::Clipboard => crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory,
            Self::Dictation => crate::menu_syntax::RootUnifiedSourceFilter::Dictation,
        }
    }

    pub(crate) fn cache_is_fresh(self) -> bool {
        match self {
            Self::Clipboard => crate::clipboard_history::root_clipboard_history_cache_is_fresh(),
            Self::Dictation => crate::dictation::root_dictation_history_cache_is_fresh(),
        }
    }

    pub(crate) fn begin(self) -> Option<RootOwnedProviderRefresh> {
        match self {
            Self::Clipboard => crate::clipboard_history::try_begin_root_clipboard_history_refresh(),
            Self::Dictation => crate::dictation::try_begin_root_dictation_history_refresh(),
        }
    }

    pub(crate) fn read_snapshot(self) -> RootPrivateHistorySnapshot {
        match self {
            Self::Clipboard => RootPrivateHistorySnapshot::Clipboard(
                crate::clipboard_history::read_root_clipboard_history_snapshot(),
            ),
            Self::Dictation => RootPrivateHistorySnapshot::Dictation(
                crate::dictation::read_root_dictation_history_snapshot(),
            ),
        }
    }

    pub(crate) fn finish(
        self,
        refresh: RootOwnedProviderRefresh,
        snapshot: RootPrivateHistorySnapshot,
    ) -> bool {
        match (self, snapshot) {
            (Self::Clipboard, RootPrivateHistorySnapshot::Clipboard(snapshot)) => {
                crate::clipboard_history::finish_root_clipboard_history_refresh(refresh, snapshot)
            }
            (Self::Dictation, RootPrivateHistorySnapshot::Dictation(snapshot)) => {
                crate::dictation::finish_root_dictation_history_refresh(refresh, snapshot)
            }
            (Self::Clipboard, RootPrivateHistorySnapshot::Dictation(_))
            | (Self::Dictation, RootPrivateHistorySnapshot::Clipboard(_)) => false,
        }
    }

    pub(crate) fn discard(self, refresh: RootOwnedProviderRefresh) -> bool {
        match self {
            Self::Clipboard => {
                crate::clipboard_history::discard_root_clipboard_history_refresh(refresh)
            }
            Self::Dictation => crate::dictation::discard_root_dictation_history_refresh(refresh),
        }
    }

    pub(crate) fn completion_reason(self) -> &'static str {
        match self {
            Self::Clipboard => "clipboard_history_refresh_complete",
            Self::Dictation => "dictation_history_refresh_complete",
        }
    }
}

/// Compare the complete ordered inbox snapshot consumed by root-launcher
/// grouping. Stable row identity alone is insufficient: edits to an existing
/// title, detail, category, provenance, or resolution must invalidate the
/// visible frame without disturbing that identity's position.
pub(crate) fn brain_inbox_snapshot_matches(
    current: &[crate::brain::InboxItem],
    fresh: &[crate::brain::InboxItem],
) -> bool {
    current.len() == fresh.len()
        && current.iter().zip(fresh).all(|(current, fresh)| {
            current.id == fresh.id
                && current.kind == fresh.kind
                && current.title == fresh.title
                && current.detail == fresh.detail
                && current.source == fresh.source
                && current.source_id == fresh.source_id
                && current.created_at == fresh.created_at
                && current.resolved_at == fresh.resolved_at
        })
}

#[derive(Debug, Default)]
pub(crate) struct RootProviderCoordinator {
    generations: ProviderGenerationFence,
}

impl RootProviderCoordinator {
    /// Reuse an already-active exact request so repeated renders do not spawn
    /// duplicate work or invalidate the response that is currently in flight.
    pub(crate) fn begin(&mut self, source: CommandSource, query: &str) -> ProviderRequest {
        if let Some(current) = self.generations.current(source) {
            if current.query == query {
                return current.clone();
            }
        }
        self.generations.begin(source, query)
    }

    /// Provider completion, generation lineage, and live input must all agree.
    pub(crate) fn accepts(&self, request: &ProviderRequest, current_query: &str) -> bool {
        request.query == current_query && self.generations.accepts(request)
    }

    pub(crate) fn invalidate(&mut self, source: CommandSource) {
        self.generations.invalidate(source);
    }
}

#[cfg(test)]
mod root_search_store_tests {
    use super::*;

    fn inbox_item(id: i64, title: &str) -> crate::brain::InboxItem {
        crate::brain::InboxItem {
            id,
            kind: crate::brain::inbox::InboxKind::Question,
            title: title.to_owned(),
            detail: "original detail".to_owned(),
            source: "note".to_owned(),
            source_id: format!("source-{id}"),
            created_at: 100,
            resolved_at: None,
        }
    }

    #[test]
    fn owned_provider_worker_rejects_duplicate_fresh_or_cross_source_completion() {
        let mut lifecycle = RootOwnedProviderRefreshLifecycle::default();
        assert!(lifecycle.begin(CommandSource::Clipboard, true).is_none());

        let clipboard = lifecycle
            .begin(CommandSource::Clipboard, false)
            .expect("cold clipboard owns one worker");
        assert!(lifecycle.begin(CommandSource::Clipboard, false).is_none());
        assert!(!lifecycle.finish(RootOwnedProviderRefresh {
            source: CommandSource::Dictation,
            generation: clipboard.generation,
        }));
        assert!(lifecycle
            .begin(CommandSource::Conversation, false)
            .is_none());
        assert!(lifecycle.finish(clipboard));
    }

    #[test]
    fn owned_provider_worker_stale_completion_cannot_release_replacement() {
        let mut lifecycle = RootOwnedProviderRefreshLifecycle::default();
        let stale = lifecycle
            .begin(CommandSource::Dictation, false)
            .expect("first dictation worker");
        assert!(lifecycle.finish(stale));
        let current = lifecycle
            .begin(CommandSource::Dictation, false)
            .expect("replacement dictation worker");

        assert!(current.generation > stale.generation);
        assert!(!lifecycle.finish(stale));
        assert_eq!(lifecycle.in_flight, Some(current));
        assert!(lifecycle.finish(current));
    }

    #[test]
    fn local_content_providers_keep_note_and_todo_owners_and_options_separate() {
        let notes = RootLocalContentProvider::Notes;
        let todos = RootLocalContentProvider::Todos;
        assert_eq!(notes.source(), CommandSource::Note);
        assert_eq!(todos.source(), CommandSource::Todo);
        assert_eq!(
            notes.source_filter(),
            crate::menu_syntax::RootUnifiedSourceFilter::Notes
        );
        assert_eq!(
            todos.source_filter(),
            crate::menu_syntax::RootUnifiedSourceFilter::Todo
        );
        assert_eq!(notes.completion_reason(), "notes_search_refresh_complete");
        assert_eq!(todos.completion_reason(), "todos_snapshot_refresh_complete");

        let todo_options =
            RootLocalContentOptions::Todos(crate::menu_syntax::RootTodoSectionOptions::default());
        assert!(!notes.cache_is_fresh("private query", todo_options));
        assert!(notes.begin("private query", todo_options).is_none());
        let foreign_refresh = RootLocalContentRefresh::Todos(RootOwnedProviderRefresh {
            source: CommandSource::Todo,
            generation: 42,
        });
        assert!(notes.read_snapshot(&foreign_refresh).is_none());
        assert!(!notes.discard(&foreign_refresh));
    }

    #[test]
    fn brain_inbox_snapshot_accepts_identical_copies_without_rebuilding() {
        let current = vec![inbox_item(1, "one"), inbox_item(2, "two")];
        let identical = current.clone();

        assert!(brain_inbox_snapshot_matches(&current, &identical));
        assert!(brain_inbox_snapshot_matches(&[], &[]));
    }

    #[test]
    fn brain_inbox_snapshot_detects_every_identity_content_and_resolution_change() {
        let current = vec![inbox_item(1, "one"), inbox_item(2, "two")];
        let changes: [(&str, fn(&mut crate::brain::InboxItem)); 8] = [
            ("identity", |item| item.id = 3),
            ("kind", |item| {
                item.kind = crate::brain::inbox::InboxKind::Commitment
            }),
            ("title", |item| item.title = "updated title".to_owned()),
            ("detail", |item| item.detail = "updated detail".to_owned()),
            ("source", |item| item.source = "chat".to_owned()),
            ("source_id", |item| {
                item.source_id = "updated-source".to_owned()
            }),
            ("created_at", |item| item.created_at = 101),
            ("resolved_at", |item| item.resolved_at = Some(102)),
        ];

        for (field, change) in changes {
            let mut fresh = current.clone();
            change(&mut fresh[0]);
            assert!(
                !brain_inbox_snapshot_matches(&current, &fresh),
                "the production comparator must observe a changed {field}"
            );
            assert_eq!(fresh[1].id, 2, "{field} must not move unrelated rows");
        }
    }

    #[test]
    fn brain_inbox_snapshot_rejects_length_and_order_changes() {
        let first = inbox_item(1, "one");
        let second = inbox_item(2, "two");
        let current = vec![first.clone(), second.clone()];

        assert!(!brain_inbox_snapshot_matches(
            &current,
            std::slice::from_ref(&first)
        ));
        assert!(!brain_inbox_snapshot_matches(&current, &[second, first]));
    }

    #[test]
    fn repeated_exact_query_reuses_the_existing_generation() {
        let mut coordinator = RootProviderCoordinator::default();
        let first = coordinator.begin(CommandSource::BrowserHistory, "script");
        let second = coordinator.begin(CommandSource::BrowserHistory, "script");

        assert_eq!(first, second);
        assert!(coordinator.accepts(&first, "script"));
    }

    #[test]
    fn stale_provider_batches_cannot_replace_a_newer_query() {
        let mut coordinator = RootProviderCoordinator::default();
        let stale = coordinator.begin(CommandSource::BrowserTab, "s");
        let current = coordinator.begin(CommandSource::BrowserTab, "script");

        assert!(current.generation > stale.generation);
        assert!(!coordinator.accepts(&stale, "script"));
        assert!(!coordinator.accepts(&current, "s"));
        assert!(coordinator.accepts(&current, "script"));
    }

    #[test]
    fn clearing_a_query_invalidates_its_inflight_provider_response() {
        let mut coordinator = RootProviderCoordinator::default();
        let pending = coordinator.begin(CommandSource::BrowserHistory, "docs");
        coordinator.invalidate(CommandSource::BrowserHistory);

        assert!(!coordinator.accepts(&pending, "docs"));
    }

    #[test]
    fn one_passive_provider_cannot_cancel_another_providers_results() {
        let mut coordinator = RootProviderCoordinator::default();
        let tabs = coordinator.begin(CommandSource::BrowserTab, "docs");
        let history = coordinator.begin(CommandSource::BrowserHistory, "docs");

        coordinator.invalidate(CommandSource::BrowserTab);

        assert!(!coordinator.accepts(&tabs, "docs"));
        assert!(coordinator.accepts(&history, "docs"));
    }

    #[test]
    fn same_text_from_the_wrong_provider_or_generation_is_refused() {
        let mut coordinator = RootProviderCoordinator::default();
        let expected = coordinator.begin(CommandSource::Clipboard, "hello");
        let wrong_source = ProviderRequest {
            source: CommandSource::Dictation,
            ..expected.clone()
        };
        let wrong_generation = ProviderRequest {
            generation: expected.generation.wrapping_add(1),
            ..expected.clone()
        };

        assert!(!coordinator.accepts(&wrong_source, "hello"));
        assert!(!coordinator.accepts(&wrong_generation, "hello"));
        assert!(coordinator.accepts(&expected, "hello"));
    }
}
