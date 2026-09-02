//! Root launcher "Brain Inbox" snapshot plumbing.
//!
//! The curator (src/brain/curator.rs) files small observations — commitments,
//! unanswered questions, drifting topics, stale pins — into the brain inbox.
//! This module keeps an app-state snapshot of the OPEN items so the grouped
//! empty-query view can pin a "Brain Inbox" section at the very top (see
//! `crate::scripts::prepend_root_brain_inbox_section`).
//!
//! Refresh model: cheap sqlite read, throttled to once per
//! [`ROOT_BRAIN_INBOX_TTL`]. Hooked where the main window becomes visible
//! (`show_main_window_helper`) and on filter-text changes, so the section is
//! current whenever the empty root query is shown. Resolving an item drops it
//! from the snapshot immediately — notification semantics: touching it clears
//! it.

use super::*;
use crate::design_evaluation::search_fixtures as F;

/// Cap on inbox items loaded into the snapshot per refresh. The grouped view
/// renders at most the configured max (default 3, clamped to 5); loading a
/// few extra keeps the section populated when items resolve between reloads.
const ROOT_BRAIN_INBOX_LOAD_LIMIT: usize = 8;

/// How long a loaded snapshot stays fresh before the next hook re-reads it.
const ROOT_BRAIN_INBOX_TTL: std::time::Duration = std::time::Duration::from_secs(30);

impl ScriptListApp {
    /// Read the synchronous source only when stale. `false` keeps stable source
    /// order and joins the caller's query publication; `true` publishes a source
    /// change atomically. Failed reads retain the last accepted inbox rows.
    pub(crate) fn refresh_root_brain_inbox_if_stale(
        &mut self,
        allow_reorder: bool,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList) || !self.root_search.query_is_current()
        {
            return;
        }
        let now = crate::runtime_policy::root_search_now();
        if self
            .root_search
            .root_brain_inbox_cache_is_fresh(now, ROOT_BRAIN_INBOX_TTL)
        {
            return;
        }
        let query = self.computed_filter_text.clone();
        let generation = self
            .root_search
            .allocate_named_provider_generation("brain-inbox");
        let owned_gate = self.main_services.search_gate();
        let owned_run = if let Some(gate) = &owned_gate {
            let Some(run) = gate.begin(
                "brain-inbox",
                &query,
                generation,
                RootProviderPublicationPolicy::VisibleSynchronous,
            ) else {
                return;
            };
            Some(run)
        } else {
            None
        };
        let result = if let Some(run) = &owned_run {
            let Ok(result) = run.read_synchronously(F::inbox_result) else {
                return;
            };
            result
        } else if let Some(sources) = self.main_services.owned_sources() {
            Ok(sources.brain_inbox.clone())
        } else if crate::runtime_policy::is_owned_evaluation() {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "owned_source_snapshot_required",
            )
            .into())
        } else {
            crate::brain::open_inbox_items(ROOT_BRAIN_INBOX_LOAD_LIMIT)
        };
        // The synchronous read returns the whole OPEN catalogue, independent of input.
        self.root_search.begin_named_provider(
            "brain-inbox",
            generation,
            &query,
            "",
            RootProviderPublicationPolicy::VisibleSynchronous,
            false,
        );
        let terminal = F::ProviderTerminal::for_result(&result);
        let apply = |app: &mut Self, _cx: &mut Context<Self>| {
            let changed = app.root_search.install_root_brain_inbox_read(
                generation,
                now,
                result,
                allow_reorder,
                ROOT_BRAIN_INBOX_LOAD_LIMIT,
            );
            app.root_search
                .finish_named_provider("brain-inbox", generation, terminal.into());
            if let Some(run) = &owned_run {
                run.finish(terminal, RootProviderPublicationPolicy::VisibleSynchronous);
            }
            if changed {
                app.invalidate_root_passive_and_grouped_cache();
            }
            changed
        };
        if allow_reorder {
            self.commit_main_menu_results_refresh(
                "brain_inbox_refresh_complete",
                Some(("brain-inbox", generation)),
                cx,
                apply,
            );
        } else {
            apply(self, cx);
        }
    }

    /// Resolve through the real source owner before dropping its accepted row.
    /// A refused or failed write leaves the last-good snapshot intact.
    pub(crate) fn resolve_root_brain_inbox_item(&mut self, id: i64, cx: &mut Context<Self>) {
        if !matches!(self.current_view, AppView::ScriptList) || !self.root_search.query_is_current()
        {
            return;
        }
        self.commit_main_menu_results_refresh("brain_inbox_item_resolved", None, cx, |app, _cx| {
            if let MainServices::OwnedFixtures(sources) = &mut app.main_services {
                Arc::make_mut(sources)
                    .brain_inbox
                    .retain(|item| item.id != id);
            } else if crate::runtime_policy::is_owned_evaluation() {
                return false;
            } else if crate::brain::resolve_inbox_item(id).is_err() {
                tracing::warn!(target: "script_kit::brain", code = "brain_inbox_resolve_failed");
                return false;
            }
            let changed = app.root_search.remove_root_brain_inbox_item(id);
            if changed {
                app.invalidate_root_passive_and_grouped_cache();
            }
            changed
        });
    }
}
