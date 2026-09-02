//! Shared "main list is slow-filling" loading state.
//!
//! One derived kind, one clock, one ticker. The source-specific truth stays
//! where it already lives (browser tab/history snapshot stores, root-file
//! search flags); this module only derives the winner and drives the shared
//! braille loading treatment (constellation layer + footer spinner, see
//! `crate::components::braille_loading`). Nothing here stores the active
//! kind — deriving it avoids a synchronization problem between sources.

use super::*;

/// Which slow-filling source currently owns the main-list loading treatment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainListLoadingKind {
    BrowserTabs,
    BrowserHistory,
    RootFileSearch,
}

impl MainListLoadingKind {
    /// Footer status label paired with the braille spinner. Exhaustive on
    /// purpose: adding a kind must not compile until its label is decided.
    pub(crate) const fn footer_label(self) -> &'static str {
        match self {
            Self::BrowserTabs => "Fetching tabs",
            Self::BrowserHistory => "Fetching history",
            Self::RootFileSearch => "Searching files",
        }
    }
}

/// Fixed conflict policy when several sources load at once for one query:
/// explicit browser-source intent (tabs, then its structural twin history)
/// outranks inferred file loading. Exactly one kind wins so exactly one
/// constellation renders — stacking layers would multiply the calm
/// 0.10–0.13 cell opacities into a much stronger treatment.
fn resolve_main_list_loading_kind(
    tabs_loading: bool,
    history_loading: bool,
    root_file_loading: bool,
) -> Option<MainListLoadingKind> {
    if tabs_loading {
        Some(MainListLoadingKind::BrowserTabs)
    } else if history_loading {
        Some(MainListLoadingKind::BrowserHistory)
    } else if root_file_loading {
        Some(MainListLoadingKind::RootFileSearch)
    } else {
        None
    }
}

impl ScriptListApp {
    /// The loading kind that currently owns the main-list treatment, if any.
    /// Cheap enough for the 30fps animation hot path: view gate, then direct
    /// source-filter ownership checks that short-circuit before touching the
    /// snapshot stores.
    pub(crate) fn main_list_loading_kind(&self) -> Option<MainListLoadingKind> {
        if !matches!(self.current_view, AppView::ScriptList) {
            return None;
        }
        let query = self.computed_filter_text.as_str();
        let tabs_loading = self.current_query_includes_root_source(
            query,
            crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs,
        ) && crate::browser_tabs::root_browser_tabs_snapshot_status().refreshing;
        let history_loading = self.current_query_includes_root_source(
            query,
            crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory,
        ) && crate::browser_history::root_browser_history_snapshot_status()
            .refreshing;
        resolve_main_list_loading_kind(
            tabs_loading,
            history_loading,
            self.visible_root_file_search_loading(),
        )
    }

    /// Seconds since the loading treatment appeared (drives glyph rotation,
    /// breath, and the layer fade-in).
    pub(crate) fn main_list_loading_elapsed_secs(&self) -> f32 {
        self.main_list_loading_started_at
            .map(|started| started.elapsed().as_secs_f32())
            .unwrap_or(0.0)
    }

    /// Start (or adopt) the loading animation: seed the shared clock without
    /// restarting it — a handoff from one loading kind to another must not
    /// replay the fade-in — and spawn a fresh frame ticker. The epoch keeps
    /// an older ticker from clearing the clock a newer loading interval owns.
    /// No-op when nothing is visibly loading.
    pub(crate) fn ensure_main_list_loading_animation(&mut self, cx: &mut Context<Self>) {
        if self.main_list_loading_kind().is_none() {
            return;
        }
        if self.main_list_loading_started_at.is_none() { self.mark_main_presentation_changed(); }
        self.main_list_loading_started_at
            .get_or_insert_with(std::time::Instant::now);
        self.main_list_loading_ticker_epoch = self.main_list_loading_ticker_epoch.wrapping_add(1);
        let epoch = self.main_list_loading_ticker_epoch;
        let owned = self.main_services.host_policy().is_hidden();
        self._main_list_loading_ticker = Some(cx.spawn(async move |this, cx| loop {
            let interval_ms = if crate::is_main_window_visible() || owned {
                33
            } else {
                250
            };
            cx.background_executor()
                .timer(std::time::Duration::from_millis(interval_ms))
                .await;
            let should_stop = this
                .update(cx, |app, cx| {
                    if app.main_list_loading_ticker_epoch != epoch {
                        return true;
                    }
                    if app.main_list_loading_kind().is_some() {
                        if crate::is_main_window_visible() || owned {
                            cx.notify();
                        }
                        false
                    } else {
                        app.main_list_loading_started_at = None;
                        app.mark_main_presentation_changed();
                        cx.notify();
                        true
                    }
                })
                .unwrap_or(true);
            if should_stop {
                break;
            }
        }));
        // Show the treatment now, not after the first 33ms timer.
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_main_list_loading_kind, MainListLoadingKind};

    /// Priority is tabs > history > root files; each fallback takes over as
    /// higher-priority sources stop loading, and no source means no kind.
    #[test]
    fn main_list_loading_priority_is_tabs_then_history_then_root_files() {
        assert_eq!(
            resolve_main_list_loading_kind(true, true, true),
            Some(MainListLoadingKind::BrowserTabs)
        );
        assert_eq!(
            resolve_main_list_loading_kind(false, true, true),
            Some(MainListLoadingKind::BrowserHistory)
        );
        assert_eq!(
            resolve_main_list_loading_kind(false, false, true),
            Some(MainListLoadingKind::RootFileSearch)
        );
        assert_eq!(resolve_main_list_loading_kind(false, false, false), None);
    }

    /// The footer labels are the loading contract shown to the user; keep
    /// them stable per kind.
    #[test]
    fn main_list_loading_footer_labels_match_contract() {
        assert_eq!(
            MainListLoadingKind::BrowserTabs.footer_label(),
            "Fetching tabs"
        );
        assert_eq!(
            MainListLoadingKind::BrowserHistory.footer_label(),
            "Fetching history"
        );
        assert_eq!(
            MainListLoadingKind::RootFileSearch.footer_label(),
            "Searching files"
        );
    }
}
