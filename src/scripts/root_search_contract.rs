//! App-owned root-launcher adapters over GPUI-free provider-generation policy.
//!
//! `RootSearchStore` lives in the application binary because its cached rows
//! depend on binary-only presentation types. Its correctness-critical async
//! request coordinator and worker lifecycle live in `sk-protocol`, so they
//! can be exercised without compiling or linking the application.

use sk_protocol::command_contract::CommandSource;
pub(crate) use sk_protocol::search_contract::{
    RootOwnedProviderRefresh, RootOwnedProviderRefreshLifecycle,
};
// RootSearchStore exists only in the application binary; retain its shared
// compatibility path without warning in the separately compiled library.
#[allow(unused_imports)]
pub(crate) use sk_protocol::search_contract::RootProviderCoordinator;

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
}
