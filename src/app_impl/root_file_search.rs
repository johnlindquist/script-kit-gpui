use super::*;

const ROOT_FILE_RESULT_CACHE_LIMIT: usize = 24;
const ROOT_FILE_SEARCH_DEBOUNCE_MS: u64 = 60;
const SPINE_FILE_SEARCH_DEBOUNCE_MS: u64 = 80;

#[derive(Clone)]
enum RootFileSearchRequest {
    GlobalQuery {
        query: String,
    },
    DirectoryBrowse {
        query: String,
        directory: String,
        show_hidden: bool,
    },
}

impl RootFileSearchRequest {
    fn query(&self) -> &str {
        match self {
            Self::GlobalQuery { query } | Self::DirectoryBrowse { query, .. } => query,
        }
    }

    fn mode(&self) -> crate::file_search::RootFileSectionMode {
        match self {
            Self::GlobalQuery { .. } => crate::file_search::RootFileSectionMode::GlobalQuery,
            Self::DirectoryBrowse { .. } => {
                crate::file_search::RootFileSectionMode::DirectoryBrowse
            }
        }
    }

    fn cache_key(&self) -> String {
        match self {
            Self::GlobalQuery { query } => format!("global:{query}"),
            Self::DirectoryBrowse {
                query,
                directory,
                show_hidden,
            } => format!("dir:{directory}:{show_hidden}:{query}"),
        }
    }
}

impl ScriptListApp {
    pub(crate) fn refresh_root_recent_file_results(&mut self) {
        let mut options = self.config.get_unified_search().root_file_section_options();
        if self
            .menu_syntax_mode
            .advanced_query_for(&self.computed_filter_text)
            .is_some_and(|query| {
                query
                    .source_filters
                    .includes(crate::menu_syntax::RootUnifiedSourceFilter::Files)
            })
        {
            options.files_enabled = true;
            options.recent_files_enabled = true;
        }
        if !options.files_enabled || !options.recent_files_enabled {
            if !self.root_search.root_recent_file_results.is_empty() {
                self.root_search.root_recent_file_results.clear();
                self.invalidate_grouped_cache();
            }
            self.root_search.root_recent_file_revision = u64::MAX;
            return;
        }

        let revision = self.frecency_store.revision();
        if self.root_search.root_recent_file_revision == revision {
            return;
        }

        let next_results =
            self.recent_file_results_from_frecency(crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT);
        let changed = root_file_result_fingerprint(&self.root_search.root_recent_file_results)
            != root_file_result_fingerprint(&next_results);
        self.root_search.root_recent_file_results = next_results;
        self.root_search.root_recent_file_revision = revision;
        if changed {
            self.invalidate_grouped_cache();
        }
    }

    fn cancel_root_file_search(&mut self) {
        if let Some(cancel) = self.root_search.root_file_search_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// True while the root file section is visibly empty AND a provider is
    /// still working for the query the user is currently looking at — the
    /// state that owns the shared main-list loading treatment. Cached rows
    /// showing while the provider warms must NOT read as loading, and
    /// passive global warms (no `files:` filter) never claim the treatment.
    pub(crate) fn visible_root_file_search_loading(&self) -> bool {
        let current_search_text = crate::menu_syntax::free_text_for_search(
            &self.menu_syntax_mode,
            &self.computed_filter_text,
        )
        .trim();
        root_file_visible_loading_decision(
            self.root_search.root_file_search_loading,
            self.root_search.root_file_provider_loading,
            self.root_search.root_file_search_query == current_search_text,
            self.root_search.root_file_search_mode,
            self.current_query_includes_root_source(
                &self.computed_filter_text,
                crate::menu_syntax::RootUnifiedSourceFilter::Files,
            ),
        )
    }

    /// Whether a finished provider batch for `request` should publish into
    /// the visible section right now. Re-evaluated at completion instead of
    /// captured at request start: source-filter ownership can change while
    /// the same free-text request stays in flight (e.g. the user adds a
    /// `files:` filter and the same-request reuse branch keeps the provider
    /// task), and the captured decision would silently cache rows while the
    /// visible section stays stuck in loading.
    fn root_file_request_should_publish_now(
        &self,
        generation: u64,
        request: &RootFileSearchRequest,
    ) -> bool {
        if self.root_search.root_file_search_generation != generation {
            return false;
        }
        match request {
            RootFileSearchRequest::GlobalQuery { query } => {
                self.root_search.root_file_search_mode
                    == Some(crate::file_search::RootFileSectionMode::GlobalQuery)
                    && self.root_search.root_file_search_query == *query
                    && self.current_query_includes_root_source(
                        &self.computed_filter_text,
                        crate::menu_syntax::RootUnifiedSourceFilter::Files,
                    )
            }
            RootFileSearchRequest::DirectoryBrowse {
                directory,
                show_hidden,
                ..
            } => self.active_root_directory_browse_source_matches(directory, *show_hidden),
        }
    }

    fn active_root_directory_browse_source_matches(
        &self,
        directory: &str,
        show_hidden: bool,
    ) -> bool {
        if self.root_search.root_file_search_mode
            != Some(crate::file_search::RootFileSectionMode::DirectoryBrowse)
        {
            return false;
        }

        crate::file_search::root_directory_browse_source_key(
            &self.root_search.root_file_search_query,
        )
        .map(|(active_directory, active_show_hidden)| {
            active_directory == directory && active_show_hidden == show_hidden
        })
        .unwrap_or(false)
    }

    fn refresh_root_file_grouping_after_query_only_change(&mut self, cx: &mut Context<Self>) {
        self.invalidate_grouped_cache();
        if matches!(self.current_view, AppView::ScriptList) {
            self.sync_list_state_for_filter_replacement();
            self.validate_selection_bounds(cx);
            self.rebuild_main_window_preflight_if_needed();
        }
        cx.notify();
    }

    fn apply_root_file_search_results_for_generation(
        &mut self,
        generation: u64,
        results: Vec<crate::file_search::FileResult>,
        loading: bool,
        clear_cancel: bool,
        cx: &mut Context<Self>,
    ) {
        if self.root_search.root_file_search_generation != generation {
            tracing::debug!(
                event = "root_file_provider_stale_drop",
                generation,
                active_generation = self.root_search.root_file_search_generation,
                query = %self.root_search.root_file_search_query,
            );
            return;
        }

        let results_changed = root_file_result_fingerprint(&self.root_search.root_file_results)
            != root_file_result_fingerprint(&results);
        let loading_changed = self.root_search.root_file_search_loading != loading
            || self.root_search.root_file_provider_loading != loading;
        if !results_changed && !loading_changed {
            if clear_cancel {
                self.root_search.root_file_search_cancel = None;
            }
            return;
        }

        let selection_before = if matches!(self.current_view, AppView::ScriptList) {
            Some(self.main_menu_selection_snapshot())
        } else {
            None
        };

        self.root_search.root_file_results = results;
        self.root_search.root_file_search_loading = loading;
        self.root_search.root_file_provider_loading = loading;
        self.root_search.root_file_frame = None;
        if clear_cancel {
            self.root_search.root_file_search_cancel = None;
        }
        self.invalidate_grouped_cache();
        if matches!(self.current_view, AppView::ScriptList) {
            self.sync_list_state_for_filter_replacement();
            match main_menu_refresh_selection_policy(self.main_menu_selection_user_moved) {
                MainMenuRefreshSelectionPolicy::RestoreIdentity => {
                    if let Some(snapshot) = selection_before {
                        self.restore_main_menu_selection_from_snapshot(snapshot);
                    }
                }
                MainMenuRefreshSelectionPolicy::SnapToFirst => {
                    self.snap_main_menu_selection_to_first();
                    tracing::debug!(
                        target: "script_kit::selection",
                        event = "main_menu_refresh_snap_to_first_untouched_selection",
                        reason = "root_file_results_publish",
                        selected_index = self.selected_index,
                    );
                }
            }
            self.validate_selection_bounds(cx);
            self.schedule_main_list_selection_reveal_above_footer(
                "root_file_active_publish_deferred",
                cx,
            );
            self.invalidate_main_window_preflight();
            self.rebuild_main_window_preflight_if_needed();
        }
        cx.notify();
    }

    fn cached_root_file_results_for_request(
        &self,
        request: &RootFileSearchRequest,
    ) -> Vec<crate::file_search::FileResult> {
        let cache_key = request.cache_key();
        self.root_search
            .root_file_result_cache
            .iter()
            .find_map(|(key, results)| {
                if key == &cache_key {
                    Some(results.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    fn cache_root_file_search_results_for_generation(
        &mut self,
        generation: u64,
        cache_key: String,
        results: Vec<crate::file_search::FileResult>,
        clear_cancel: bool,
    ) {
        if self.root_search.root_file_search_generation != generation {
            tracing::debug!(
                event = "root_file_provider_stale_drop",
                generation,
                active_generation = self.root_search.root_file_search_generation,
                query = %self.root_search.root_file_search_query,
            );
            return;
        }

        if let Some(index) = self
            .root_search
            .root_file_result_cache
            .iter()
            .position(|(key, _)| key == &cache_key)
        {
            self.root_search.root_file_result_cache.remove(index);
        }
        self.root_search
            .root_file_result_cache
            .push_front((cache_key, dedupe_root_file_results(results)));
        while self.root_search.root_file_result_cache.len() > ROOT_FILE_RESULT_CACHE_LIMIT {
            self.root_search.root_file_result_cache.pop_back();
        }

        if clear_cancel {
            self.root_search.root_file_search_cancel = None;
        }
        self.root_search.root_file_provider_loading = false;
    }

    pub(crate) fn active_root_file_cache_result_count(&self) -> usize {
        let Some(mode) = self.root_search.root_file_search_mode else {
            return 0;
        };
        let request = match mode {
            crate::file_search::RootFileSectionMode::GlobalQuery => {
                RootFileSearchRequest::GlobalQuery {
                    query: self.root_search.root_file_search_query.clone(),
                }
            }
            crate::file_search::RootFileSectionMode::DirectoryBrowse => return 0,
        };
        let cache_key = request.cache_key();
        self.root_search
            .root_file_result_cache
            .iter()
            .find_map(|(key, results)| (key == &cache_key).then_some(results.len()))
            .unwrap_or(0)
    }

    pub(crate) fn maybe_start_root_file_search(&mut self, query: &str, cx: &mut Context<Self>) {
        let search_text =
            crate::menu_syntax::free_text_for_search(&self.menu_syntax_mode, query).to_string();
        let trimmed = search_text.trim();
        let advanced_query_owned = self.menu_syntax_mode.advanced_query_for(query).cloned();
        let source_filters = advanced_query_owned
            .as_ref()
            .map(|advanced_query| advanced_query.source_filters.clone())
            .unwrap_or_default();
        let advanced_predicate_active = advanced_query_owned
            .as_ref()
            .is_some_and(|advanced_query| advanced_query.has_predicates());
        let mut root_file_options = self.config.get_unified_search().root_file_section_options();
        if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Files) {
            root_file_options.files_enabled = true;
            root_file_options.global_search_enabled = true;
            root_file_options.directory_browse_enabled = true;
            root_file_options.recent_files_enabled = true;
            root_file_options.query_intent =
                crate::file_search::RootFileQueryIntent::ExplicitFilesSourceFilter;
            root_file_options.source_chip_visible_limit =
                Some(self.root_file_source_chip_visible_limit_for(
                    query,
                    trimmed,
                    advanced_predicate_active,
                    self.root_search.root_file_search_mode,
                ));
        }
        if !root_file_options.files_enabled
            || !source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Files)
        {
            self.cancel_root_file_search();
            let had_results = !self.root_search.root_file_results.is_empty()
                || !self.root_search.root_recent_file_results.is_empty()
                || !self.root_search.root_file_search_query.is_empty()
                || self.root_search.root_file_search_loading
                || self.root_search.root_file_provider_loading
                || self.root_search.root_file_search_mode.is_some();
            self.root_search.root_file_results.clear();
            self.root_search.root_recent_file_results.clear();
            self.root_search.root_file_search_query.clear();
            self.root_search.root_file_search_mode = None;
            self.root_search.root_file_search_loading = false;
            self.root_search.root_file_provider_loading = false;
            self.root_search.root_file_frame = None;
            if had_results {
                self.root_search.root_file_search_generation =
                    self.root_search.root_file_search_generation.wrapping_add(1);
                self.invalidate_grouped_cache();
                cx.notify();
            }
            return;
        }

        let can_collect = matches!(self.current_view, AppView::ScriptList)
            && self
                .menu_syntax_mode
                .advanced_query_for(&self.filter_text)
                .is_none_or(|advanced_query| !advanced_query.has_predicates())
            && !self.menu_syntax_object_selector_state.owns_main_list()
            && !self.menu_syntax_trigger_picker_state.owns_main_list()
            && !self
                .menu_syntax_mode
                .capture_composer_owns_input_for(trimmed)
            && !self.menu_syntax_mode.command_owns_input_for(trimmed);

        let request = if !can_collect {
            None
        } else if root_file_options.global_search_enabled
            && crate::file_search::should_search_root_files_for_intent(
                trimmed,
                root_file_options.query_intent,
            )
        {
            Some(RootFileSearchRequest::GlobalQuery {
                query: trimmed.to_string(),
            })
        } else if root_file_options.directory_browse_enabled
            && crate::file_search::looks_like_root_directory_browse_query(trimmed)
        {
            crate::file_search::parse_directory_path(trimmed).map(|parsed| {
                RootFileSearchRequest::DirectoryBrowse {
                    query: trimmed.to_string(),
                    directory: parsed.directory,
                    show_hidden: parsed.show_hidden,
                }
            })
        } else {
            None
        };

        let Some(request) = request else {
            self.cancel_root_file_search();
            let had_results = !self.root_search.root_file_results.is_empty()
                || !self.root_search.root_file_search_query.is_empty()
                || self.root_search.root_file_search_loading
                || self.root_search.root_file_provider_loading
                || self.root_search.root_file_search_mode.is_some();
            self.root_search.root_file_results.clear();
            self.root_search.root_file_search_query.clear();
            self.root_search.root_file_search_mode = None;
            self.root_search.root_file_search_loading = false;
            self.root_search.root_file_provider_loading = false;
            self.root_search.root_file_frame = None;
            if had_results {
                self.root_search.root_file_search_generation =
                    self.root_search.root_file_search_generation.wrapping_add(1);
                self.invalidate_grouped_cache();
                cx.notify();
            }
            return;
        };

        let mode = request.mode();
        match &request {
            RootFileSearchRequest::GlobalQuery { .. }
                if self.root_search.root_file_search_query == request.query()
                    && self.root_search.root_file_search_mode == Some(mode) =>
            {
                let cached_results = self.cached_root_file_results_for_request(&request);
                if root_file_result_fingerprint(&self.root_search.root_file_results)
                    != root_file_result_fingerprint(&cached_results)
                {
                    self.root_search.root_file_results = cached_results;
                    self.root_search.root_file_search_loading =
                        self.root_search.root_file_results.is_empty();
                    self.root_search.root_file_frame = None;
                    self.invalidate_grouped_cache();
                }
                // The same request can become visibly loading through a
                // newly typed `files:` filter while its provider task is
                // already in flight — attach the loading treatment.
                self.ensure_main_list_loading_animation(cx);
                return;
            }
            RootFileSearchRequest::DirectoryBrowse {
                query,
                directory,
                show_hidden,
            } if self.active_root_directory_browse_source_matches(directory, *show_hidden) => {
                if self.root_search.root_file_search_query != *query {
                    self.root_search.root_file_search_query = query.clone();
                    self.refresh_root_file_grouping_after_query_only_change(cx);
                }
                self.ensure_main_list_loading_animation(cx);
                return;
            }
            _ => {}
        }

        self.cancel_root_file_search();
        self.root_search.root_file_search_generation =
            self.root_search.root_file_search_generation.wrapping_add(1);
        let generation = self.root_search.root_file_search_generation;
        self.root_search.root_file_search_query = request.query().to_string();
        self.root_search.root_file_search_mode = Some(mode);
        let cached_results = self.cached_root_file_results_for_request(&request);
        self.root_search.root_file_results = cached_results;
        self.root_search.root_file_search_loading = self.root_search.root_file_results.is_empty();
        self.root_search.root_file_provider_loading = true;
        self.invalidate_grouped_cache();
        // Start the loading clock at request acceptance, not after the
        // debounce or the first provider event.
        self.ensure_main_list_loading_animation(cx);

        let cancel = crate::file_search::new_cancel_token();
        self.root_search.root_file_search_cancel = Some(cancel.clone());
        let publish_active_results = source_filters
            .includes(crate::menu_syntax::RootUnifiedSourceFilter::Files)
            || matches!(&request, RootFileSearchRequest::DirectoryBrowse { .. });
        let request_cache_key = request.cache_key();

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    ROOT_FILE_SEARCH_DEBOUNCE_MS,
                ))
                .await;

            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }

            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn({
                let cancel = cancel.clone();
                let request = request.clone();
                move || match request {
                    RootFileSearchRequest::GlobalQuery { query } => {
                        if emit_root_file_search_test_fixture(&query, &cancel, &tx) {
                            return;
                        }

                        let provider_query =
                            crate::file_search::root_file_provider_query_for_user_query(&query);
                        crate::file_search::search_files_streaming_with_options(
                            &provider_query,
                            None,
                            crate::file_search::ROOT_FILE_SOURCE_LIMIT,
                            cancel,
                            crate::file_search::SearchFilesStreamingOptions::root_search(),
                            |event| {
                                let _ = tx.send(event);
                            },
                        );
                    }
                    RootFileSearchRequest::DirectoryBrowse {
                        directory,
                        show_hidden,
                        ..
                    } => {
                        for result in crate::file_search::list_directory_with_options(
                            &directory,
                            crate::file_search::ROOT_FILE_BROWSE_SOURCE_LIMIT,
                            show_hidden,
                        ) {
                            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                                return;
                            }
                            let _ = tx.send(crate::file_search::SearchEvent::Result(result));
                        }
                        let _ = tx.send(crate::file_search::SearchEvent::Done);
                    }
                }
            });

            let mut batch = Vec::new();
            loop {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }

                match rx.try_recv() {
                    Ok(crate::file_search::SearchEvent::Result(result)) => {
                        batch.push(result);
                    }
                    Ok(crate::file_search::SearchEvent::Done) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                }
            }

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    // Ownership can change while the request is in flight
                    // (e.g. a `files:` filter typed onto the same free text),
                    // so the publish decision is re-evaluated now instead of
                    // trusting the intent captured at request start.
                    let publish_now =
                        app.root_file_request_should_publish_now(generation, &request);
                    tracing::debug!(
                        event = "root_file_provider_done",
                        query = %app.root_search.root_file_search_query,
                        generation,
                        publish_active_results = publish_now,
                        requested_publish_at_start = publish_active_results,
                        result_count = batch.len(),
                        cache_key = %request_cache_key,
                        visible_frame_touched = publish_now,
                    );
                    if publish_now {
                        app.apply_root_file_search_results_for_generation(
                            generation, batch, false, true, cx,
                        );
                    } else {
                        app.cache_root_file_search_results_for_generation(
                            generation,
                            request_cache_key,
                            batch,
                            true,
                        );
                    }
                })
            });
        })
        .detach();
    }
}

fn root_file_result_fingerprint(files: &[crate::file_search::FileResult]) -> Vec<&str> {
    files.iter().map(|file| file.path.as_str()).collect()
}

/// Pure core of [`ScriptListApp::visible_root_file_search_loading`].
///
/// Both flags are required on purpose: `visible_batch_empty` (the section
/// has nothing to show) and `provider_loading` (work is actually still in
/// flight). Cached rows rendering while the provider warms fail the first;
/// a finished search that found nothing fails the second. Passive global
/// warms (no explicit `files:` filter) never own the loading treatment;
/// directory browsing always publishes actively so it can.
fn root_file_visible_loading_decision(
    visible_batch_empty: bool,
    provider_loading: bool,
    stored_query_matches_current: bool,
    mode: Option<crate::file_search::RootFileSectionMode>,
    explicit_files_filter: bool,
) -> bool {
    if !visible_batch_empty || !provider_loading || !stored_query_matches_current {
        return false;
    }
    match mode {
        Some(crate::file_search::RootFileSectionMode::DirectoryBrowse) => true,
        Some(crate::file_search::RootFileSectionMode::GlobalQuery) => explicit_files_filter,
        None => false,
    }
}

#[cfg(test)]
mod loading_decision_tests {
    use super::{root_file_visible_loading_decision, *};
    use crate::file_search::RootFileSectionMode;

    /// A passive global cache warm (no `files:` filter) must not surface
    /// the main-list loading treatment.
    #[test]
    fn passive_global_file_warm_does_not_surface_main_list_loading() {
        assert!(!root_file_visible_loading_decision(
            true,
            true,
            true,
            Some(RootFileSectionMode::GlobalQuery),
            false,
        ));
    }

    /// An explicit `files:` search with an empty visible batch owns the
    /// treatment; so does an empty directory browse.
    #[test]
    fn explicit_files_and_directory_browse_empty_batches_surface_loading() {
        assert!(root_file_visible_loading_decision(
            true,
            true,
            true,
            Some(RootFileSectionMode::GlobalQuery),
            true,
        ));
        assert!(root_file_visible_loading_decision(
            true,
            true,
            true,
            Some(RootFileSectionMode::DirectoryBrowse),
            false,
        ));
    }

    /// Cached rows on screen, a finished provider, a stale stored query, or
    /// no active mode all suppress the treatment.
    #[test]
    fn cached_rows_finished_provider_or_stale_query_suppress_loading() {
        let cases = [
            (
                false,
                true,
                true,
                Some(RootFileSectionMode::DirectoryBrowse),
            ),
            (
                true,
                false,
                true,
                Some(RootFileSectionMode::DirectoryBrowse),
            ),
            (
                true,
                true,
                false,
                Some(RootFileSectionMode::DirectoryBrowse),
            ),
            (true, true, true, None),
        ];
        for (visible_empty, provider, query_matches, mode) in cases {
            assert!(
                !root_file_visible_loading_decision(
                    visible_empty,
                    provider,
                    query_matches,
                    mode,
                    true
                ),
                "case: {visible_empty} {provider} {query_matches} {mode:?}"
            );
        }
    }

    #[gpui::test]
    fn same_query_root_file_publish_preserves_user_moved_selection(
        cx: &mut gpui::TestAppContext,
    ) {
        let app = main_menu_selection_test_app(cx);
        app.update(cx, |app, cx| {
            let query = "zzlauncherrefreshprobe";
            let handoff_key = "fallback/root-file-search-handoff/global";
            app.scripts = vec![main_menu_selection_test_script(
                "zzlauncherrefreshprobe first",
            )];
            app.scriptlets.clear();
            app.skills.clear();
            app.apps.clear();
            app.computed_filter_text = query.to_string();
            app.filter_text = query.to_string();
            app.menu_syntax_mode = crate::menu_syntax::MenuSyntaxMode::from_input(query);
            app.root_search.root_file_search_generation = 41;
            app.root_search.root_file_search_mode =
                Some(crate::file_search::RootFileSectionMode::GlobalQuery);
            app.root_search.root_file_search_query = query.to_string();
            app.root_search.root_file_search_loading = true;
            app.root_search.root_file_provider_loading = true;
            app.invalidate_grouped_cache();
            app.get_grouped_results_cached();
            app.selected_index = app
                .main_menu_result_caches
                .grouped_index_for_stable_selection_key(handoff_key)
                .expect("same query should include the Search Files handoff");
            app.mark_main_menu_selection_user_moved();
            let selected_before = selected_main_menu_stable_key(app);

            app.apply_root_file_search_results_for_generation(
                41,
                vec![crate::file_search::FileResult {
                    path: "/tmp/zzlauncherrefreshprobe.txt".to_string(),
                    name: "zzlauncherrefreshprobe.txt".to_string(),
                    size: 1,
                    modified: 1,
                    file_type: crate::file_search::FileType::File,
                }],
                false,
                true,
                cx,
            );

            let selected_after = selected_main_menu_stable_key(app);
            assert_eq!(selected_before.as_deref(), Some(handoff_key));
            assert_eq!(selected_after, selected_before);
            assert_ne!(selected_after, first_selectable_main_menu_stable_key(app));
        });
    }
}

fn dedupe_root_file_results(
    results: Vec<crate::file_search::FileResult>,
) -> Vec<crate::file_search::FileResult> {
    let mut seen = std::collections::HashSet::new();
    results
        .into_iter()
        .filter(|file| seen.insert(file.path.clone()))
        .collect()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RootFileSearchTestFixture {
    query: String,
    #[serde(default = "default_root_file_test_delay_ms")]
    delay_ms: u64,
    results: Vec<RootFileSearchTestFixtureResult>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum RootFileSearchTestProvider {
    Single(RootFileSearchTestFixture),
    Multi {
        fixtures: Vec<RootFileSearchTestFixture>,
        #[serde(default)]
        passthrough_unmatched: bool,
    },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RootFileSearchTestFixtureResult {
    path: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    file_type: Option<String>,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    modified: u64,
}

fn default_root_file_test_delay_ms() -> u64 {
    250
}

fn emit_root_file_search_test_fixture(
    query: &str,
    cancel: &crate::file_search::CancelToken,
    tx: &std::sync::mpsc::Sender<crate::file_search::SearchEvent>,
) -> bool {
    let Ok(raw) = std::env::var("SCRIPT_KIT_ROOT_FILE_SEARCH_TEST_PROVIDER") else {
        return false;
    };
    let Ok(provider) = serde_json::from_str::<RootFileSearchTestProvider>(&raw) else {
        return false;
    };
    let fixture = match provider {
        RootFileSearchTestProvider::Single(fixture) => {
            if fixture.query != query {
                return false;
            }
            Some(fixture)
        }
        RootFileSearchTestProvider::Multi {
            fixtures,
            passthrough_unmatched,
        } => {
            let found = fixtures.into_iter().find(|fixture| fixture.query == query);
            if found.is_none() && !passthrough_unmatched {
                let _ = tx.send(crate::file_search::SearchEvent::Done);
                return true;
            }
            found
        }
    };
    let Some(fixture) = fixture else {
        return false;
    };

    std::thread::sleep(std::time::Duration::from_millis(fixture.delay_ms));
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return true;
    }

    for result in fixture.results {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return true;
        }
        let _ = tx.send(crate::file_search::SearchEvent::Result(
            result.into_file_result(),
        ));
    }
    let _ = tx.send(crate::file_search::SearchEvent::Done);
    true
}

impl RootFileSearchTestFixtureResult {
    fn into_file_result(self) -> crate::file_search::FileResult {
        let name = self.name.unwrap_or_else(|| {
            std::path::Path::new(&self.path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&self.path)
                .to_string()
        });
        crate::file_search::FileResult {
            path: self.path,
            name,
            size: self.size,
            modified: self.modified,
            file_type: match self.file_type.as_deref() {
                Some("directory") => crate::file_search::FileType::Directory,
                Some("application") => crate::file_search::FileType::Application,
                Some("image") => crate::file_search::FileType::Image,
                Some("document") => crate::file_search::FileType::Document,
                Some("audio") => crate::file_search::FileType::Audio,
                Some("video") => crate::file_search::FileType::Video,
                Some("other") => crate::file_search::FileType::Other,
                _ => crate::file_search::FileType::File,
            },
        }
    }
}

/// Internal marker query for the empty `@file:` recents seed; cannot collide
/// with user-typed sub-queries (contains a control character).
const SPINE_FILE_RECENTS_SENTINEL: &str = "\u{1}spine-file-recents";

/// Dedup-key prefixes for the `@project:` subsearch. Both embed the scope
/// directory so a cwd switch mid-colon-mode restarts the search; the control
/// character keeps them disjoint from raw `@file:` sub-queries.
const SPINE_PROJECT_SEARCH_KEY_PREFIX: &str = "\u{1}spine-project\u{1f}";
const SPINE_PROJECT_RECENTS_SENTINEL_PREFIX: &str = "\u{1}spine-project-recents\u{1f}";

/// `mdfind -onlyin` scope for a directory, or `None` when Spotlight cannot
/// serve it. Spotlight skips hidden directories, so scoping to a path with
/// any dot-component (e.g. `~/.scriptkit`) returns zero results for every
/// query.
fn spotlight_indexable_scope(path: &std::path::Path) -> Option<String> {
    let hidden = path.components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name)
                if name.to_string_lossy().starts_with('.')
        )
    });
    if hidden {
        None
    } else {
        Some(path.to_string_lossy().to_string())
    }
}

impl ScriptListApp {
    // ── Spine @file: subsearch ───────────────────────────────────────

    fn cancel_spine_file_subsearch(&mut self) {
        if let Some(cancel) = self.spine_file_search_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn clear_spine_file_subsearch_state(&mut self, cx: &mut Context<Self>) {
        self.cancel_spine_file_subsearch();
        let had_state = !self.spine_file_search_query.is_empty()
            || !self.spine_file_search_results.is_empty()
            || self.spine_file_search_loading;
        self.spine_file_search_query.clear();
        self.spine_file_search_results.clear();
        self.spine_file_search_loading = false;
        if had_state {
            self.spine_file_search_generation = self.spine_file_search_generation.wrapping_add(1);
            self.invalidate_grouped_cache();
            cx.notify();
        }
    }

    pub(crate) fn active_spine_context_subsearch(
        &self,
    ) -> Option<(
        crate::spine::catalog_subsearch::ContextSubsearchSource,
        String,
    )> {
        if !self.spine_projection_owns_main_list() {
            return None;
        }
        let projection = self.spine_projection.as_ref()?;
        match &projection.active_segment_kind {
            crate::spine::SpineSegmentKind::ContextMention {
                context_type,
                sub_query,
            } => {
                let (source, query) = crate::spine::catalog_subsearch::parse_context_subsearch(
                    context_type,
                    sub_query.as_deref(),
                )?;
                Some((source, query.to_string()))
            }
            _ => None,
        }
    }

    pub(crate) fn maybe_start_spine_file_subsearch_for_current_projection(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some((source, query)) = self.active_spine_context_subsearch() else {
            self.clear_spine_file_subsearch_state(cx);
            return;
        };
        match source {
            crate::spine::catalog_subsearch::ContextSubsearchSource::File => {
                self.maybe_start_spine_file_subsearch(&query, cx);
            }
            crate::spine::catalog_subsearch::ContextSubsearchSource::Project => {
                self.maybe_start_spine_project_subsearch(&query, cx);
            }
            _ => self.clear_spine_file_subsearch_state(cx),
        }
    }

    /// Directory the `@project:` subsearch is scoped to: the global cwd chip,
    /// falling back to home when no cwd is set.
    fn spine_project_scope_dir(&self) -> Option<String> {
        self.spine_cwd
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .or_else(|| dirs::home_dir().map(|home| home.to_string_lossy().to_string()))
    }

    fn maybe_start_spine_file_subsearch(&mut self, query: &str, cx: &mut Context<Self>) {
        let query = query.trim();
        if query.is_empty() {
            // A3 recent-files decision (2026-06-09): an empty `@file:`
            // sub-query seeds Spotlight recently-used files instead of
            // clearing, so the colon-mode landing state shows real recents.
            self.maybe_start_spine_file_recents_search(cx);
            return;
        }
        if self.spine_file_search_query == query {
            return;
        }

        self.cancel_spine_file_subsearch();
        self.spine_file_search_generation = self.spine_file_search_generation.wrapping_add(1);
        let generation = self.spine_file_search_generation;
        self.spine_file_search_query = query.to_string();
        self.spine_file_search_loading = true;
        self.spine_file_search_results.clear();
        self.invalidate_grouped_cache();
        cx.notify();

        let cancel = crate::file_search::new_cancel_token();
        self.spine_file_search_cancel = Some(cancel.clone());
        let query_owned = query.to_string();

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    SPINE_FILE_SEARCH_DEBOUNCE_MS,
                ))
                .await;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }

            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn({
                let cancel = cancel.clone();
                let query_owned = query_owned.clone();
                move || {
                    if emit_root_file_search_test_fixture(&query_owned, &cancel, &tx) {
                        return;
                    }
                    let provider_query =
                        crate::file_search::root_file_provider_query_for_user_query(&query_owned);
                    // The spine cwd is the agent's working directory, not a
                    // search filter. Typed `@file:` sub-queries search
                    // globally, matching the "Open full File Search" portal
                    // row rendered alongside these results — scoping mdfind
                    // to the cwd (always set since the global cwd chip
                    // landed, defaulting to `~/.scriptkit`, which Spotlight
                    // does not index) emptied every inline result.
                    crate::file_search::search_files_streaming_with_options(
                        &provider_query,
                        None,
                        crate::file_search::ROOT_FILE_SOURCE_LIMIT,
                        cancel,
                        crate::file_search::SearchFilesStreamingOptions::root_search(),
                        |event| {
                            let _ = tx.send(event);
                        },
                    );
                }
            });

            let mut batch = Vec::new();
            loop {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                match rx.try_recv() {
                    Ok(crate::file_search::SearchEvent::Result(result)) => {
                        batch.push(result);
                    }
                    Ok(crate::file_search::SearchEvent::Done) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                }
            }

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    app.apply_spine_file_subsearch_results(generation, batch, cx);
                })
            });
        })
        .detach();
    }

    /// Kick the async Spotlight "recently used files" seed for the empty
    /// `@file:` colon mode. Results merge after frecency recents in
    /// `build_rich_file_subsearch_rows`.
    fn maybe_start_spine_file_recents_search(&mut self, cx: &mut Context<Self>) {
        if self.spine_file_search_query == SPINE_FILE_RECENTS_SENTINEL {
            return;
        }

        self.cancel_spine_file_subsearch();
        self.spine_file_search_generation = self.spine_file_search_generation.wrapping_add(1);
        let generation = self.spine_file_search_generation;
        self.spine_file_search_query = SPINE_FILE_RECENTS_SENTINEL.to_string();
        self.spine_file_search_loading = true;
        self.spine_file_search_results.clear();
        self.invalidate_grouped_cache();
        cx.notify();

        let cancel = crate::file_search::new_cancel_token();
        self.spine_file_search_cancel = Some(cancel.clone());
        // Recents may scope to the picked cwd (a project-local landing state
        // is useful), but only when Spotlight can actually serve it; hidden
        // dot-directories like the default `~/.scriptkit` are unindexed and
        // would yield a permanently empty "Recent Files" section.
        let onlyin = self
            .spine_cwd
            .as_deref()
            .and_then(spotlight_indexable_scope)
            .or_else(|| dirs::home_dir().map(|home| home.to_string_lossy().to_string()));

        cx.spawn(async move |this, cx| {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn({
                let cancel = cancel.clone();
                let onlyin = onlyin.clone();
                move || {
                    crate::file_search::search_files_streaming_with_options(
                        crate::file_search::RECENTLY_USED_FILES_MDQUERY,
                        onlyin.as_deref(),
                        crate::file_search::RECENTLY_USED_FILES_SOURCE_LIMIT,
                        cancel,
                        crate::file_search::SearchFilesStreamingOptions {
                            // Need real modified timestamps to order recents.
                            skip_metadata: false,
                            allow_filesystem_fallback: false,
                        },
                        |event| {
                            let _ = tx.send(event);
                        },
                    );
                }
            });

            let mut batch = Vec::new();
            loop {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                match rx.try_recv() {
                    Ok(crate::file_search::SearchEvent::Result(result)) => {
                        if !crate::file_search::is_noisy_recent_file_path(&result.path) {
                            batch.push(result);
                        }
                    }
                    Ok(crate::file_search::SearchEvent::Done) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                }
            }

            batch.sort_by(|a, b| b.modified.cmp(&a.modified));
            batch.truncate(crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT);

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    app.apply_spine_file_subsearch_results(generation, batch, cx);
                })
            });
        })
        .detach();
    }

    /// Typed `@project:` sub-query: mdfind scoped to the cwd, with the
    /// filesystem-walk fallback so Spotlight-blind dot-directory cwds
    /// (`~/.scriptkit`) still return results. The raw query is passed
    /// through (no provider-query rewrite) so the fallback stays eligible.
    fn maybe_start_spine_project_subsearch(&mut self, query: &str, cx: &mut Context<Self>) {
        let query = query.trim();
        if query.is_empty() {
            self.maybe_start_spine_project_recents_search(cx);
            return;
        }
        let Some(scope) = self.spine_project_scope_dir() else {
            self.clear_spine_file_subsearch_state(cx);
            return;
        };
        let dedup_key = format!("{SPINE_PROJECT_SEARCH_KEY_PREFIX}{scope}\u{1f}{query}");
        if self.spine_file_search_query == dedup_key {
            return;
        }

        self.cancel_spine_file_subsearch();
        self.spine_file_search_generation = self.spine_file_search_generation.wrapping_add(1);
        let generation = self.spine_file_search_generation;
        self.spine_file_search_query = dedup_key;
        self.spine_file_search_loading = true;
        self.spine_file_search_results.clear();
        self.invalidate_grouped_cache();
        cx.notify();

        let cancel = crate::file_search::new_cancel_token();
        self.spine_file_search_cancel = Some(cancel.clone());
        let query_owned = query.to_string();

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    SPINE_FILE_SEARCH_DEBOUNCE_MS,
                ))
                .await;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }

            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn({
                let cancel = cancel.clone();
                let query_owned = query_owned.clone();
                let scope = scope.clone();
                move || {
                    crate::file_search::search_files_streaming_with_options(
                        &query_owned,
                        Some(&scope),
                        crate::file_search::ROOT_FILE_SOURCE_LIMIT,
                        cancel,
                        crate::file_search::SearchFilesStreamingOptions {
                            skip_metadata: true,
                            allow_filesystem_fallback: true,
                        },
                        |event| {
                            let _ = tx.send(event);
                        },
                    );
                }
            });

            let mut batch = Vec::new();
            loop {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                match rx.try_recv() {
                    Ok(crate::file_search::SearchEvent::Result(result)) => {
                        batch.push(result);
                    }
                    Ok(crate::file_search::SearchEvent::Done) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                }
            }

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    app.apply_spine_file_subsearch_results(generation, batch, cx);
                })
            });
        })
        .detach();
    }

    /// Empty `@project:` landing state: Spotlight recently-used files scoped
    /// to the cwd when Spotlight can serve it, else (and when Spotlight
    /// returns nothing) a bounded filesystem walk sorted by mtime.
    fn maybe_start_spine_project_recents_search(&mut self, cx: &mut Context<Self>) {
        let Some(scope) = self.spine_project_scope_dir() else {
            self.clear_spine_file_subsearch_state(cx);
            return;
        };
        let sentinel = format!("{SPINE_PROJECT_RECENTS_SENTINEL_PREFIX}{scope}");
        if self.spine_file_search_query == sentinel {
            return;
        }

        self.cancel_spine_file_subsearch();
        self.spine_file_search_generation = self.spine_file_search_generation.wrapping_add(1);
        let generation = self.spine_file_search_generation;
        self.spine_file_search_query = sentinel;
        self.spine_file_search_loading = true;
        self.spine_file_search_results.clear();
        self.invalidate_grouped_cache();
        cx.notify();

        let cancel = crate::file_search::new_cancel_token();
        self.spine_file_search_cancel = Some(cancel.clone());
        let spotlight_scope = spotlight_indexable_scope(std::path::Path::new(&scope));

        cx.spawn(async move |this, cx| {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn({
                let cancel = cancel.clone();
                let scope = scope.clone();
                move || {
                    let mut spotlight_hits = 0usize;
                    if let Some(spotlight_scope) = &spotlight_scope {
                        crate::file_search::search_files_streaming_with_options(
                            crate::file_search::RECENTLY_USED_FILES_MDQUERY,
                            Some(spotlight_scope),
                            crate::file_search::RECENTLY_USED_FILES_SOURCE_LIMIT,
                            cancel,
                            crate::file_search::SearchFilesStreamingOptions {
                                skip_metadata: false,
                                allow_filesystem_fallback: false,
                            },
                            |event| {
                                if let crate::file_search::SearchEvent::Result(result) = event {
                                    if !crate::file_search::is_noisy_recent_file_path(&result.path)
                                    {
                                        spotlight_hits += 1;
                                        let _ = tx
                                            .send(crate::file_search::SearchEvent::Result(result));
                                    }
                                }
                            },
                        );
                    }
                    if spotlight_hits == 0 {
                        for result in crate::file_search::recent_files_filesystem(
                            std::path::Path::new(&scope),
                            crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT,
                        ) {
                            let _ = tx.send(crate::file_search::SearchEvent::Result(result));
                        }
                    }
                    let _ = tx.send(crate::file_search::SearchEvent::Done);
                }
            });

            let mut batch = Vec::new();
            loop {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                match rx.try_recv() {
                    Ok(crate::file_search::SearchEvent::Result(result)) => {
                        batch.push(result);
                    }
                    Ok(crate::file_search::SearchEvent::Done) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                }
            }

            batch.sort_by(|a, b| b.modified.cmp(&a.modified));
            batch.truncate(crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT);

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    app.apply_spine_file_subsearch_results(generation, batch, cx);
                })
            });
        })
        .detach();
    }

    fn apply_spine_file_subsearch_results(
        &mut self,
        generation: u64,
        results: Vec<crate::file_search::FileResult>,
        cx: &mut Context<Self>,
    ) {
        if self.spine_file_search_generation != generation {
            return;
        }
        let results = dedupe_root_file_results(results);
        self.spine_file_search_results = results;
        self.spine_file_search_loading = false;
        self.spine_file_search_cancel = None;
        self.invalidate_grouped_cache();
        if matches!(self.current_view, AppView::ScriptList) {
            self.sync_list_state_for_filter_replacement();
            self.validate_selection_bounds(cx);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod spine_file_scope_tests {
    use super::spotlight_indexable_scope;
    use std::path::Path;

    #[test]
    fn hidden_directories_are_not_spotlight_scopes() {
        // The default spine cwd (`~/.scriptkit`) lives in a dot-directory
        // that Spotlight never indexes; scoping recents there must fall back
        // instead of silently returning an empty list forever.
        assert_eq!(
            spotlight_indexable_scope(Path::new("/Users/me/.scriptkit")),
            None
        );
        assert_eq!(
            spotlight_indexable_scope(Path::new("/Users/me/.config/nested")),
            None
        );
    }

    #[test]
    fn visible_directories_are_usable_scopes() {
        assert_eq!(
            spotlight_indexable_scope(Path::new("/Users/me/dev/project")).as_deref(),
            Some("/Users/me/dev/project")
        );
    }
}
