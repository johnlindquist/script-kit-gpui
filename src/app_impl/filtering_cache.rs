use super::*;

const INLINE_CALCULATOR_SECTION_LABEL: &str = "Calculator";
const INLINE_CALCULATOR_RESULT_INDEX: usize = usize::MAX;
const ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS: usize = 256;

pub(super) fn root_passive_search_needle(query: &str) -> &str {
    query
        .char_indices()
        .nth(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS)
        .map_or(query, |(byte_index, _)| &query[..byte_index])
}

fn timed_root_passive_source<T>(
    source: &'static str,
    query: &str,
    explicit: bool,
    f: impl FnOnce() -> Vec<T>,
) -> Vec<T> {
    let start = std::time::Instant::now();
    let rows = f();
    let elapsed = start.elapsed();
    if logging::filter_perf_trace_enabled() || elapsed >= std::time::Duration::from_millis(8) {
        logging::log(
            "FILTER_PERF",
            &format!(
                "[PASSIVE_SOURCE_DONE] source={} query_len={} explicit={} in {:.2}ms -> {} hits",
                source,
                query.chars().count(),
                explicit,
                elapsed.as_secs_f64() * 1000.0,
                rows.len()
            ),
        );
    }
    rows
}

fn grouped_selectable_bounds(
    grouped_items: &[GroupedListItem],
    flat_results: &[scripts::SearchResult],
) -> (Option<usize>, Option<usize>) {
    let mut first = None;
    let mut last = None;
    for (index, item) in grouped_items.iter().enumerate() {
        let GroupedListItem::Item(flat_idx) = item else {
            continue;
        };
        // SpineProjection rows carry their own is_selectable flag (Empty
        // placeholders are non-selectable but pushed as Items so they render).
        // Exclude them from selectable bounds so selectedIndex and
        // visibleChoiceCount don't treat them as targets.
        if let Some(scripts::SearchResult::SpineProjection(row)) = flat_results.get(*flat_idx) {
            if !row.is_selectable {
                continue;
            }
        }
        if first.is_none() {
            first = Some(index);
        }
        last = Some(index);
    }
    (first, last)
}

fn prepend_inline_calculator_group(
    grouped_items: Vec<GroupedListItem>,
    flat_results: Vec<scripts::SearchResult>,
    calculator: Option<&crate::calculator::CalculatorInlineResult>,
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let Some(_calculator) = calculator else {
        return (grouped_items, flat_results);
    };

    let mut merged_grouped_items = Vec::with_capacity(grouped_items.len() + 2);
    merged_grouped_items.push(GroupedListItem::SectionHeader(
        INLINE_CALCULATOR_SECTION_LABEL.to_string(),
        None,
    ));
    merged_grouped_items.push(GroupedListItem::Item(INLINE_CALCULATOR_RESULT_INDEX));
    merged_grouped_items.extend(grouped_items);

    (merged_grouped_items, flat_results)
}

fn build_menu_syntax_trigger_picker_main_list_results(
    snapshot: &crate::menu_syntax::TriggerPickerSnapshot,
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let section = snapshot.mode.main_list_section();
    let mut grouped_items = Vec::with_capacity(snapshot.rows.len() + 1);
    let mut flat_results = Vec::with_capacity(snapshot.rows.len());
    grouped_items.push(GroupedListItem::SectionHeader(
        section.0.to_string(),
        Some(section.1.to_string()),
    ));

    for row in &snapshot.rows {
        let flat_index = flat_results.len();
        flat_results.push(scripts::SearchResult::SpineProjection(
            crate::menu_syntax_trigger_picker::trigger_picker_row_to_main_list_row(row),
        ));
        grouped_items.push(GroupedListItem::Item(flat_index));
    }

    (grouped_items, flat_results)
}

fn build_menu_syntax_object_selector_main_list_results(
    snapshot: &crate::menu_syntax::ObjectSelectorSnapshot,
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let mut grouped_items = Vec::with_capacity(snapshot.rows.len() + 1);
    let mut flat_results = Vec::with_capacity(snapshot.rows.len());
    grouped_items.push(GroupedListItem::SectionHeader(
        "Objects".to_string(),
        Some("at-sign".to_string()),
    ));

    for row in &snapshot.rows {
        let flat_index = flat_results.len();
        flat_results.push(scripts::SearchResult::SpineProjection(
            crate::menu_syntax::object_selector_row_to_main_list_row(row),
        ));
        grouped_items.push(GroupedListItem::Item(flat_index));
    }

    (grouped_items, flat_results)
}

impl ScriptListApp {
    pub(crate) fn filter_text(&self) -> &str {
        self.filter_text.as_str()
    }

    pub(crate) fn install_root_windows(
        &mut self,
        windows: Vec<crate::window_control::WindowInfo>,
        cx: &mut Context<Self>,
    ) {
        let interaction_before = self.main_menu_interaction_snapshot();
        self.cached_windows = windows;
        self.root_search
            .install_root_windows(&self.cached_windows, &self.apps);
        self.invalidate_grouped_cache();
        self.reconcile_script_list_after_results_refresh(
            "root_windows_refresh_complete",
            interaction_before,
            cx,
        );
        cx.notify();
    }

    pub(crate) fn rebuild_root_windows_after_app_icon_cache_update(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        if self.cached_windows.is_empty() {
            return;
        }

        let interaction_before = self.main_menu_interaction_snapshot();
        self.root_search
            .rebuild_root_windows(&self.cached_windows, &self.apps);
        self.invalidate_grouped_cache();
        self.reconcile_script_list_after_results_refresh(reason, interaction_before, cx);
    }

    pub(crate) fn maybe_start_root_windows_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(advanced_query) = self.menu_syntax_mode.advanced_query_for(query_text) else {
            return;
        };
        let windows_explicit = advanced_query
            .source_filters
            .includes(crate::menu_syntax::RootUnifiedSourceFilter::Windows)
            && advanced_query
                .source_filters
                .allows(crate::menu_syntax::RootUnifiedSourceFilter::Windows);
        if !windows_explicit || !self.root_search.root_windows_refresh_needed() {
            return;
        }

        let token = self.root_search.begin_root_windows_refresh();
        self.invalidate_grouped_cache();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::window_control::list_windows() })
                .await;

            let _ = this.update(cx, |app, cx| {
                if !app.root_search.root_windows_refresh_token_matches(token) {
                    return;
                }
                match result {
                    Ok(windows) => app.install_root_windows(windows, cx),
                    Err(error) => {
                        let interaction_before = app.main_menu_interaction_snapshot();
                        let message = error.to_string();
                        let lower = message.to_ascii_lowercase();
                        let status =
                            if lower.contains("accessibility") || lower.contains("permission") {
                                crate::window_control::RootWindowsProviderStatus::PermissionRequired
                            } else {
                                crate::window_control::RootWindowsProviderStatus::ProviderError {
                                    message: message
                                        .lines()
                                        .next()
                                        .unwrap_or("unknown error")
                                        .to_string(),
                                }
                            };
                        app.root_search.fail_root_windows_refresh(status);
                        app.invalidate_grouped_cache();
                        app.reconcile_script_list_after_results_refresh(
                            "root_windows_refresh_error",
                            interaction_before,
                            cx,
                        );
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn current_query_includes_root_source(
        &self,
        query_text: &str,
        source: crate::menu_syntax::RootUnifiedSourceFilter,
    ) -> bool {
        self.menu_syntax_mode
            .advanced_query_for(query_text)
            .is_some_and(|advanced_query| {
                advanced_query.source_filters.includes(source)
                    && advanced_query.source_filters.allows(source)
            })
    }

    pub(crate) fn invalidate_root_passive_and_grouped_cache(&mut self) {
        self.root_search.clear_root_passive_frame();
        self.invalidate_grouped_cache();
        self.invalidate_main_window_preflight();
    }

    fn root_browser_tabs_refresh_options_for_query(
        &self,
        query_text: &str,
    ) -> Option<(crate::browser_tabs::RootBrowserTabsSectionOptions, bool)> {
        let source = crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs;
        let unified_search = self.config.get_unified_search();
        let mut options = unified_search.browser_tabs_section_options();
        let advanced_query = self.menu_syntax_mode.advanced_query_for(query_text);
        let source_filters = advanced_query
            .map(|query| query.source_filters.clone())
            .unwrap_or_default();
        let explicit_tabs = source_filters.includes(source) && source_filters.allows(source);

        if explicit_tabs {
            options.enabled = true;
            options.min_query_chars = 0;
            options.max_results = options
                .max_results
                .max(unified_search.passive_result_limits().max_total_results);
            return Some((options, true));
        }

        if !source_filters.allows(source) {
            return None;
        }

        if advanced_query.is_some_and(|query| query.has_predicates()) {
            return None;
        }

        if self.menu_syntax_object_selector_state.owns_main_list()
            || self.menu_syntax_trigger_picker_state.owns_main_list()
            || self
                .menu_syntax_mode
                .capture_composer_owns_input_for(query_text)
            || self.menu_syntax_mode.command_owns_input_for(query_text)
        {
            return None;
        }

        let search_text =
            crate::menu_syntax::free_text_for_search(&self.menu_syntax_mode, query_text);
        if !crate::browser_tabs::root_browser_tabs_query_is_eligible(search_text, options.clone()) {
            return None;
        }

        Some((options, false))
    }

    fn current_query_can_show_root_browser_tabs(&self, query_text: &str) -> bool {
        self.root_browser_tabs_refresh_options_for_query(query_text)
            .is_some()
    }

    pub(crate) fn maybe_start_root_browser_tabs_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        let Some((options, explicit_tabs)) =
            self.root_browser_tabs_refresh_options_for_query(query_text)
        else {
            return;
        };

        let providers = options.providers.clone();
        let reason = if explicit_tabs {
            "explicit_tabs_query"
        } else {
            "implicit_tabs_query"
        };
        let refresh = crate::browser_tabs::try_begin_root_browser_tabs_refresh(
            options.cache_ttl_ms,
            providers.len(),
            reason,
        );
        if explicit_tabs {
            // Explicit `tabs:` queries get the braille loading treatment.
            // Runs whether or not THIS call began the refresh: `None` with a
            // refresh already in flight (e.g. an implicit warm started it)
            // must still attach the animation; `None` with a fresh cache is
            // a no-op inside the helper.
            self.ensure_main_list_loading_animation(cx);
        }
        let Some(refresh) = refresh else {
            return;
        };

        self.invalidate_root_passive_and_grouped_cache();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result =
                cx.background_executor()
                    .spawn(async move {
                        crate::browser_tabs::refresh_root_browser_tabs_snapshot(providers)
                    })
                    .await;

            let _ = this.update(cx, |app, cx| {
                let changed =
                    crate::browser_tabs::finish_root_browser_tabs_refresh(refresh, result);
                if !changed {
                    return;
                }
                let interaction_before = app.main_menu_interaction_snapshot();
                app.invalidate_root_passive_and_grouped_cache();
                if app.current_query_can_show_root_browser_tabs(&app.computed_filter_text) {
                    app.reconcile_script_list_after_results_refresh(
                        "browser_tabs_refresh_complete",
                        interaction_before,
                        cx,
                    );
                } else {
                    app.rebuild_main_window_preflight_if_needed();
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Mirrors `root_browser_tabs_refresh_options_for_query`: explicit
    /// `history:` widens the options; an implicit (passive) eligible query
    /// keeps config options so the snapshot warms for the cached typing-path
    /// lookup. Returns `None` when history cannot surface for this query.
    fn root_browser_history_refresh_options_for_query(
        &self,
        query_text: &str,
    ) -> Option<(
        crate::browser_history::RootBrowserHistorySectionOptions,
        bool,
    )> {
        let source = crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory;
        let unified_search = self.config.get_unified_search();
        let mut options = unified_search.browser_history_section_options();
        let advanced_query = self.menu_syntax_mode.advanced_query_for(query_text);
        let source_filters = advanced_query
            .map(|query| query.source_filters.clone())
            .unwrap_or_default();
        let explicit_history = source_filters.includes(source) && source_filters.allows(source);

        if explicit_history {
            options.enabled = true;
            options.min_query_chars = 0;
            options.max_age_days = 365;
            options.max_results = options
                .max_results
                .max(unified_search.passive_result_limits().max_total_results);
            return Some((options, true));
        }

        if !source_filters.allows(source) {
            return None;
        }

        if advanced_query.is_some_and(|query| query.has_predicates()) {
            return None;
        }

        if self.menu_syntax_object_selector_state.owns_main_list()
            || self.menu_syntax_trigger_picker_state.owns_main_list()
            || self
                .menu_syntax_mode
                .capture_composer_owns_input_for(query_text)
            || self.menu_syntax_mode.command_owns_input_for(query_text)
        {
            return None;
        }

        let search_text =
            crate::menu_syntax::free_text_for_search(&self.menu_syntax_mode, query_text);
        if !crate::browser_history::root_browser_history_query_is_eligible(
            search_text,
            options.clone(),
        ) {
            return None;
        }

        Some((options, false))
    }

    pub(crate) fn maybe_start_root_browser_history_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        let Some((options, explicit_history)) =
            self.root_browser_history_refresh_options_for_query(query_text)
        else {
            return;
        };

        let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
            return;
        };
        let reason = if explicit_history {
            "explicit_history_query"
        } else {
            "implicit_history_query"
        };
        let refresh =
            crate::browser_history::try_begin_root_browser_history_refresh(&options, reason);
        if explicit_history {
            // Mirror of the tabs wiring: explicit `history:` queries attach
            // the loading treatment even when the refresh was already begun
            // elsewhere; a fresh cache leaves this a no-op.
            self.ensure_main_list_loading_animation(cx);
        }
        let Some(refresh) = refresh else {
            return;
        };

        self.invalidate_root_passive_and_grouped_cache();
        cx.notify();

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = crate::browser_history::refresh_root_browser_history_snapshot_from_home(
                &home, &options,
            );
            let _ = tx.send(result);
        });

        cx.spawn(async move |this, cx| {
            let result = loop {
                match rx.try_recv() {
                    Ok(result) => break result,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        break Err(anyhow::anyhow!(
                            "browser history refresh worker disconnected"
                        ));
                    }
                }
            };

            let _ = this.update(cx, |app, cx| {
                let changed =
                    crate::browser_history::finish_root_browser_history_refresh(refresh, result);
                if !changed {
                    return;
                }
                let interaction_before = app.main_menu_interaction_snapshot();
                app.invalidate_root_passive_and_grouped_cache();
                let query_text = app.computed_filter_text.clone();
                if app
                    .root_browser_history_refresh_options_for_query(&query_text)
                    .is_some()
                {
                    app.reconcile_script_list_after_results_refresh(
                        "browser_history_refresh_complete",
                        interaction_before,
                        cx,
                    );
                } else {
                    app.rebuild_main_window_preflight_if_needed();
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn root_passive_frame_for_current_query(
        &mut self,
        search_text: &str,
        advanced_query_active: bool,
        source_filters: crate::menu_syntax::RootUnifiedSourceFilterSet,
        todo_options: crate::menu_syntax::RootTodoSectionOptions,
        brain_options: crate::brain::RootBrainSectionOptions,
        notes_options: crate::notes::RootNotesSectionOptions,
        clipboard_history_options: crate::clipboard_history::RootClipboardHistorySectionOptions,
        dictation_history_options: crate::dictation::RootDictationHistorySectionOptions,
        agent_chat_history_options: crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions,
        ai_vault_options: crate::ai_vault::RootAiVaultSectionOptions,
        browser_tabs_options: crate::browser_tabs::RootBrowserTabsSectionOptions,
        browser_history_options: crate::browser_history::RootBrowserHistorySectionOptions,
    ) -> crate::RootPassiveFrame {
        let ai_vault_status = crate::ai_vault::root_ai_vault_snapshot_status();
        let browser_tabs_status = crate::browser_tabs::root_browser_tabs_snapshot_status();
        let browser_history_status = crate::browser_history::root_browser_history_snapshot_status();
        let key = crate::RootPassiveFrameKey {
            query: search_text.to_string(),
            advanced_query: advanced_query_active,
            source_filters: source_filters.clone(),
            todo_options,
            brain_options,
            brain_semantic_epoch: self.root_search.root_brain_semantic_epoch(),
            notes_options,
            clipboard_history_options,
            dictation_history_options,
            agent_chat_history_options,
            ai_vault_options: ai_vault_options.clone(),
            ai_vault_snapshot_generation: ai_vault_status.generation,
            browser_tabs_options: browser_tabs_options.clone(),
            browser_tabs_snapshot_generation: browser_tabs_status.generation,
            browser_history_options: browser_history_options.clone(),
            browser_history_snapshot_generation: browser_history_status.generation,
        };

        if let Some(frame) = self.root_search.cached_root_passive_frame(&key) {
            return frame;
        }

        let search_needle = root_passive_search_needle(search_text);

        let explicit_brain =
            source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Brain);
        let explicit_notes =
            source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Notes);
        let explicit_todos =
            source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Todo);
        let explicit_clipboard =
            source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory);
        let explicit_dictation =
            source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Dictation);
        let explicit_conversations =
            source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Conversations);
        let explicit_ai_vault =
            source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::AiVault);
        let explicit_browser_tabs =
            source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs);
        let explicit_browser_history =
            source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory);

        let allow_brain = source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Brain);
        let allow_notes = source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Notes);
        let allow_todos = source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Todo);
        let allow_clipboard =
            source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory);
        let allow_dictation =
            source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Dictation);
        let allow_conversations =
            source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Conversations);
        let allow_ai_vault =
            source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::AiVault);
        let allow_browser_tabs =
            source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs);
        let allow_browser_history =
            source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory);

        // Single shared eligibility decision for both brain passes (sync
        // lexical here, async semantic in `root_brain_search.rs`) so the two
        // can never drift. `RecentsOnly` covers the armed bare `brain:` case
        // (audit F6): show the most recent memories instead of a blank panel.
        let brain_plan = crate::brain::root_brain_query_plan(
            search_needle,
            explicit_brain,
            advanced_query_active,
            allow_brain,
            brain_options,
        );
        // Prefer the async hybrid (semantic) batch when it was computed for
        // exactly this query; the sync lexical pass below is the instant
        // first paint while semantic results are still in flight.
        let brain_semantic_hits = match &brain_plan {
            crate::brain::RootBrainQueryPlan::Search(brain_query) => {
                crate::brain::semantic_root_brain_hits_for_query(
                    brain_query,
                    self.root_search.root_brain_semantic_results(),
                    &brain_options,
                )
            }
            _ => None,
        };
        let brain_hits =
            timed_root_passive_source(
                "brain",
                search_needle,
                explicit_brain,
                || match &brain_plan {
                    crate::brain::RootBrainQueryPlan::Skip => Vec::new(),
                    crate::brain::RootBrainQueryPlan::RecentsOnly => {
                        crate::brain::recent_root_brain_hits(brain_options.max_results)
                    }
                    crate::brain::RootBrainQueryPlan::Search(brain_query) => brain_semantic_hits
                        .unwrap_or_else(|| {
                            crate::brain::search_root_brain_direct(brain_query, &brain_options)
                        }),
                },
            );

        let note_hits = timed_root_passive_source("notes", search_needle, explicit_notes, || {
            if !advanced_query_active
                && allow_notes
                && crate::notes::root_notes_query_is_eligible(search_needle, notes_options)
            {
                if explicit_notes {
                    crate::notes::search_root_notes_meta_direct(search_needle, notes_options)
                } else {
                    crate::notes::search_root_notes_meta_cached(search_needle, notes_options)
                }
            } else {
                Vec::new()
            }
        });

        let todo_hits = timed_root_passive_source("todo", search_needle, explicit_todos, || {
            if (!advanced_query_active || explicit_todos)
                && allow_todos
                && crate::menu_syntax::root_todo_query_is_eligible(search_needle, todo_options)
            {
                if explicit_todos {
                    crate::menu_syntax::search_root_todos_direct(search_needle, todo_options)
                } else {
                    // Implicit typing path: nonblocking snapshot filter. The
                    // day-page file scan happens on a detached refresh
                    // thread (self-guarded, at most once per TTL), never on
                    // the keystroke path.
                    crate::menu_syntax::ensure_root_todos_snapshot_refresh();
                    crate::menu_syntax::search_root_todos_cached(search_needle, todo_options)
                }
            } else {
                Vec::new()
            }
        });

        let clipboard_history_hits = timed_root_passive_source(
            "clipboard_history",
            search_needle,
            explicit_clipboard,
            || {
                if !advanced_query_active
                    && allow_clipboard
                    && crate::clipboard_history::root_clipboard_history_query_is_eligible(
                        search_needle,
                        clipboard_history_options,
                    )
                {
                    if explicit_clipboard {
                        crate::clipboard_history::search_root_clipboard_history_meta_direct(
                            search_needle,
                            clipboard_history_options,
                        )
                    } else {
                        crate::clipboard_history::search_root_clipboard_history_meta_cached(
                            search_needle,
                            clipboard_history_options,
                        )
                    }
                } else {
                    Vec::new()
                }
            },
        );

        let dictation_history_hits = timed_root_passive_source(
            "dictation_history",
            search_needle,
            explicit_dictation,
            || {
                if !advanced_query_active
                    && allow_dictation
                    && crate::dictation::root_dictation_history_query_is_eligible(
                        search_needle,
                        dictation_history_options,
                    )
                {
                    if explicit_dictation {
                        crate::dictation::search_root_dictation_history_direct(
                            search_needle,
                            dictation_history_options,
                        )
                    } else {
                        crate::dictation::search_root_dictation_history_cached(
                            search_needle,
                            dictation_history_options,
                        )
                    }
                } else {
                    Vec::new()
                }
            },
        );

        let agent_chat_history_hits = timed_root_passive_source(
            "agent_chat_history",
            search_needle,
            explicit_conversations,
            || {
                if !advanced_query_active
                    && allow_conversations
                    && crate::ai::agent_chat::ui::history::root_agent_chat_history_query_is_eligible(
                        search_needle,
                        agent_chat_history_options,
                    )
                {
                    if explicit_conversations {
                        crate::ai::agent_chat::ui::history::search_history_direct(
                            search_needle,
                            agent_chat_history_options.max_results,
                        )
                    } else {
                        crate::ai::agent_chat::ui::history::search_history_cached(
                            search_needle,
                            agent_chat_history_options.max_results,
                        )
                    }
                } else {
                    Vec::new()
                }
            },
        );

        let ai_vault_hits =
            timed_root_passive_source("ai_vault", search_needle, explicit_ai_vault, || {
                if explicit_ai_vault
                    && !advanced_query_active
                    && allow_ai_vault
                    && crate::ai_vault::root_ai_vault_query_is_eligible(
                        search_needle,
                        &ai_vault_options,
                    )
                {
                    crate::ai_vault::search_root_ai_vault_direct(search_needle, ai_vault_options)
                } else {
                    Vec::new()
                }
            });

        let browser_tab_hits =
            timed_root_passive_source("browser_tabs", search_needle, explicit_browser_tabs, || {
                if !advanced_query_active
                    && allow_browser_tabs
                    && crate::browser_tabs::root_browser_tabs_query_is_eligible(
                        search_needle,
                        browser_tabs_options.clone(),
                    )
                {
                    if explicit_browser_tabs {
                        crate::browser_tabs::search_root_browser_tabs_meta_direct(
                            search_needle,
                            browser_tabs_options.clone(),
                        )
                    } else {
                        crate::browser_tabs::search_root_browser_tabs_meta_cached(
                            search_needle,
                            browser_tabs_options.clone(),
                        )
                    }
                } else {
                    Vec::new()
                }
            });

        let browser_history_hits = timed_root_passive_source(
            "browser_history",
            search_needle,
            explicit_browser_history,
            || {
                if !advanced_query_active
                    && allow_browser_history
                    && crate::browser_history::root_browser_history_query_is_eligible(
                        search_needle,
                        browser_history_options.clone(),
                    )
                {
                    if explicit_browser_history {
                        crate::browser_history::search_root_browser_history_meta_direct(
                            search_needle,
                            browser_history_options.clone(),
                        )
                    } else {
                        // Implicit typing path mirrors browser tabs: a
                        // nonblocking snapshot-only lookup. The blocking-risk
                        // work (SQLite copies) stays on the background
                        // refresh thread, preserving the 13a417737 latency
                        // fix while letting history participate passively.
                        crate::browser_history::search_root_browser_history_meta_cached(
                            search_needle,
                            browser_history_options.clone(),
                        )
                    }
                } else {
                    Vec::new()
                }
            },
        );

        let frame = crate::RootPassiveFrame {
            key,
            note_hits,
            brain_hits,
            todo_hits,
            clipboard_history_hits,
            dictation_history_hits,
            agent_chat_history_hits,
            ai_vault_hits,
            browser_tab_hits,
            browser_history_hits,
            ai_vault_snapshot_generation: ai_vault_status.generation,
            browser_tabs_snapshot_generation: browser_tabs_status.generation,
            browser_history_snapshot_generation: browser_history_status.generation,
        };
        self.root_search.cache_root_passive_frame(frame)
    }

    fn root_file_frame_for_current_query(
        &mut self,
        search_text: &str,
        advanced_query_active: bool,
        source_filters: crate::menu_syntax::RootUnifiedSourceFilterSet,
        root_file_options: crate::file_search::RootFileSectionOptions,
    ) -> crate::RootFileFrame {
        let key = crate::RootFileFrameKey {
            query: search_text.to_string(),
            advanced_query: advanced_query_active,
            source_filters,
            mode: self.root_search.root_file_search_mode,
            options: root_file_options,
            search_generation: self.root_search.root_file_search_generation,
            recent_file_revision: self.root_search.root_recent_file_revision,
            visible_loading: self.root_search.root_file_search_loading,
        };

        if let Some(frame) = self.root_search.root_file_frame.as_ref() {
            if frame.key == key {
                return frame.clone();
            }
        }

        let frame = crate::RootFileFrame {
            key,
            mode: self.root_search.root_file_search_mode,
            visible_loading: self.root_search.root_file_search_loading,
            file_results: self.root_search.root_file_results.clone(),
            recent_file_results: self.root_search.root_recent_file_results.clone(),
        };
        self.root_search.root_file_frame = Some(frame.clone());
        frame
    }

    /// Shared recompute helper: every filtered search path routes through here
    /// so plugin skills are always included in main-menu results.
    fn recompute_filtered_results(&self, filter_text: &str) -> Vec<scripts::SearchResult> {
        let search_text =
            crate::menu_syntax::free_text_for_search(&self.menu_syntax_mode, filter_text);
        if self
            .menu_syntax_mode
            .advanced_query_for(filter_text)
            .is_some_and(|query| query.has_source_filters())
        {
            return Vec::new();
        }
        let search_start = std::time::Instant::now();
        let flows = self.flow_desk_corpus();
        let results = scripts::fuzzy_search_unified_all_with_skills_and_flows(
            &self.scripts,
            &self.scriptlets,
            &self.builtin_entries,
            &self.apps,
            &self.skills,
            &flows,
            search_text,
        );
        let results = match self.menu_syntax_mode.advanced_query_for(filter_text) {
            Some(query) => crate::menu_syntax::apply_advanced_query(results, query),
            None => results,
        };
        let search_elapsed = search_start.elapsed();

        if !filter_text.is_empty() {
            logging::log(
                "PERF",
                &format!(
                    "Search '{}' (computed '{}') took {:.2}ms ({} results from {} total, including {} skills)",
                    filter_text,
                    search_text,
                    search_elapsed.as_secs_f64() * 1000.0,
                    results.len(),
                    self.scripts.len()
                        + self.scriptlets.len()
                        + self.builtin_entries.len()
                        + self.apps.len()
                        + self.skills.len(),
                    self.skills.len(),
                ),
            );
        }

        tracing::info!(
            filter_text = %filter_text,
            search_text = %search_text,
            result_count = results.len(),
            script_count = self.scripts.len(),
            scriptlet_count = self.scriptlets.len(),
            builtin_count = self.builtin_entries.len(),
            app_count = self.apps.len(),
            skill_count = self.skills.len(),
            "main_menu_filtered_results_recomputed"
        );

        results
    }

    /// P1: Now uses caching - invalidates only when filter_text changes
    pub(crate) fn filtered_results(&self) -> Vec<scripts::SearchResult> {
        let filter_text = self.filter_text();
        // When a composer-style menu-syntax picker owns the input (e.g. `;t`
        // or `!dep`), the main launcher should not report or render fuzzy
        // matches — the popup is the sole surface for the typed characters.
        // Without this gate, `getState.visibleChoiceCount`, automation
        // `getElements`, and selection-coercion code would keep iterating
        // over stale fuzzy results (e.g. 8 semicolon-ish script matches) behind
        // the popup.
        if self.menu_syntax_object_selector_state.owns_main_list()
            || self.menu_syntax_trigger_picker_state.owns_main_list()
            || crate::menu_syntax::active_filter_head_owns_main_list(filter_text)
            || self
                .menu_syntax_mode
                .capture_composer_owns_input_for(filter_text)
            || self.menu_syntax_mode.command_owns_input_for(filter_text)
        {
            return Vec::new();
        }

        // P1: Return cached results if filter hasn't changed
        if self
            .main_menu_result_caches
            .has_filtered_results_for(filter_text)
        {
            logging::log_debug("CACHE", &format!("Filter cache HIT for '{}'", filter_text));
            return self.main_menu_result_caches.clone_filtered_results();
        }

        // P1: Cache miss - need to recompute (will be done by get_filtered_results_mut)
        logging::log_debug(
            "CACHE",
            &format!(
                "Filter cache MISS - need recompute for '{}' (cached key: '{}')",
                filter_text,
                self.main_menu_result_caches.filtered_cache_key()
            ),
        );

        self.recompute_filtered_results(filter_text)
    }

    /// Flow rosters land asynchronously (background `md roster` fetch); the
    /// main-menu result caches poll the catalog generation so a roster that
    /// arrives after a render still surfaces on the next cache read, without
    /// a background-thread cx handle.
    fn sync_flow_roster_cache_generation(&mut self) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEEN: AtomicU64 = AtomicU64::new(0);
        // Keep the roster fresh even when result caches stay hot: a cache
        // hit skips recompute (and therefore roster_for), so without this
        // poke a stale roster could be pinned indefinitely.
        crate::flows::catalog::flow_catalog().poke(&self.flow_ux_cwd());
        let generation = crate::flows::catalog::roster_generation();
        if SEEN.swap(generation, Ordering::Relaxed) != generation {
            self.invalidate_filter_cache();
            self.invalidate_grouped_cache();
        }
    }

    /// P1: Get filtered results with cache update (mutable version)
    /// Call this when you need to ensure cache is updated
    pub(crate) fn get_filtered_results_cached(&mut self) -> &Vec<scripts::SearchResult> {
        self.sync_flow_roster_cache_generation();
        if self.menu_syntax_object_selector_state.owns_main_list()
            || self.menu_syntax_trigger_picker_state.owns_main_list()
            || crate::menu_syntax::active_filter_head_owns_main_list(&self.filter_text)
            || self
                .menu_syntax_mode
                .capture_composer_owns_input_for(&self.filter_text)
            || self
                .menu_syntax_mode
                .command_owns_input_for(&self.filter_text)
        {
            self.main_menu_result_caches
                .store_filtered_results(self.filter_text.clone(), Vec::new());
            return self.main_menu_result_caches.filtered_results();
        }

        if !self
            .main_menu_result_caches
            .has_filtered_results_for(&self.filter_text)
        {
            let filter_text = self.filter_text.clone();
            if logging::filter_perf_trace_enabled() {
                logging::log(
                    "FILTER_PERF",
                    &format!(
                        "[4a/5] SEARCH_START for '{}' (scripts={}, scriptlets={}, builtins={}, apps={}, skills={})",
                        filter_text,
                        self.scripts.len(),
                        self.scriptlets.len(),
                        self.builtin_entries.len(),
                        self.apps.len(),
                        self.skills.len(),
                    ),
                );
            }
            let search_start = std::time::Instant::now();
            let filtered_results = self.recompute_filtered_results(&filter_text);
            let filtered_result_count = filtered_results.len();
            self.main_menu_result_caches
                .store_filtered_results(filter_text.clone(), filtered_results);
            let search_elapsed = search_start.elapsed();

            if logging::filter_perf_trace_enabled()
                || search_elapsed >= std::time::Duration::from_millis(8)
            {
                logging::log(
                    "FILTER_PERF",
                    &format!(
                        "[4a/5] SEARCH_DONE '{}' in {:.2}ms -> {} results (skills={})",
                        filter_text,
                        search_elapsed.as_secs_f64() * 1000.0,
                        filtered_result_count,
                        self.skills.len(),
                    ),
                );
            }
        }
        // NOTE: Removed cache HIT log - fires every render, only log MISS for diagnostics
        self.main_menu_result_caches.filtered_results()
    }

    /// P1: Invalidate filter cache (call when scripts/scriptlets change)
    #[allow(dead_code)]
    pub(crate) fn invalidate_filter_cache(&mut self) {
        logging::log_debug("CACHE", "Filter cache INVALIDATED");
        self.main_menu_result_caches.invalidate_filtered_results();
        self.main_menu_render_diagnostics
            .last_input_highlight_text
            .clear();
        self.main_menu_render_diagnostics
            .last_input_highlight_ranges
            .clear();
    }

    fn active_script_list_attachment_portal_kind(
        &self,
    ) -> Option<crate::ai::context_selector::types::ContextPortalKind> {
        use crate::ai::context_selector::types::ContextPortalKind;

        if !matches!(self.current_view, AppView::ScriptList) {
            return None;
        }

        match self.active_attachment_portal_kind {
            Some(
                kind @ (ContextPortalKind::ScriptSearch
                | ContextPortalKind::ScriptletSearch
                | ContextPortalKind::SkillSearch),
            ) => Some(kind),
            _ => None,
        }
    }

    fn script_list_result_matches_attachment_portal(
        kind: crate::ai::context_selector::types::ContextPortalKind,
        result: &scripts::SearchResult,
    ) -> bool {
        use crate::ai::context_selector::types::ContextPortalKind;

        matches!(
            (kind, result),
            (
                ContextPortalKind::ScriptSearch,
                scripts::SearchResult::Script(_)
            ) | (
                ContextPortalKind::ScriptletSearch,
                scripts::SearchResult::Scriptlet(_)
            ) | (
                ContextPortalKind::SkillSearch,
                scripts::SearchResult::Skill(_)
            )
        )
    }

    fn apply_script_list_attachment_portal_filter(
        &self,
        kind: crate::ai::context_selector::types::ContextPortalKind,
        flat_results: Vec<scripts::SearchResult>,
    ) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
        let filtered_results: Vec<scripts::SearchResult> = flat_results
            .into_iter()
            .filter(|result| Self::script_list_result_matches_attachment_portal(kind, result))
            .collect();
        let grouped_items: Vec<GroupedListItem> = filtered_results
            .iter()
            .enumerate()
            .map(|(index, _)| GroupedListItem::Item(index))
            .collect();

        (grouped_items, filtered_results)
    }

    /// P1: Get grouped results with caching - avoids recomputing 9+ times per keystroke
    ///
    /// This is the ONLY place that should call scripts::get_grouped_results().
    /// P3: Cache is keyed off computed_filter_text (not filter_text) for two-stage filtering.
    ///
    /// P1-Arc: Returns Arc clones for cheap sharing with render closures.
    pub(crate) fn get_grouped_results_cached(
        &mut self,
    ) -> (Arc<[GroupedListItem]>, Arc<[scripts::SearchResult]>) {
        self.sync_flow_roster_cache_generation();
        // The grouped cache is keyed by `computed_filter_text`. Menu syntax is
        // an ownership boundary, so never return stale grouped rows while the
        // live input is owned by the trigger picker or capture composer.
        let live_filter_text = self.filter_text.as_str();
        let computed_filter_text = self.computed_filter_text.as_str();
        let spine_owns_live_main_list =
            self.spine_projection_owns_main_list() && self.spine_parse.input == live_filter_text;
        let popup_owns_live_main_list = self.menu_syntax_object_selector_state.owns_main_list()
            || self.menu_syntax_trigger_picker_state.owns_main_list();
        let active_filter_head_owns_live_main_list =
            crate::menu_syntax::active_filter_head_owns_main_list(live_filter_text);
        let live_menu_syntax_owns_main_list = popup_owns_live_main_list
            || (!spine_owns_live_main_list
                && (self
                    .menu_syntax_mode
                    .capture_composer_owns_input_for(live_filter_text)
                    || self
                        .menu_syntax_mode
                        .command_owns_input_for(live_filter_text)
                    || active_filter_head_owns_live_main_list));
        if live_menu_syntax_owns_main_list && live_filter_text != computed_filter_text {
            if self.menu_syntax_trigger_picker_state.owns_main_list() {
                if let Some(snapshot) = self.menu_syntax_trigger_picker_state.snapshot.as_ref() {
                    let (grouped_items, flat_results) =
                        build_menu_syntax_trigger_picker_main_list_results(snapshot);
                    return (
                        Arc::<[GroupedListItem]>::from(grouped_items),
                        Arc::<[scripts::SearchResult]>::from(flat_results),
                    );
                }
            }
            if self.menu_syntax_object_selector_state.owns_main_list() {
                if let Some(snapshot) = self.menu_syntax_object_selector_state.snapshot.as_ref() {
                    let (grouped_items, flat_results) =
                        build_menu_syntax_object_selector_main_list_results(snapshot);
                    return (
                        Arc::<[GroupedListItem]>::from(grouped_items),
                        Arc::<[scripts::SearchResult]>::from(flat_results),
                    );
                }
            }
            return (
                Arc::<[GroupedListItem]>::from(Vec::new()),
                Arc::<[scripts::SearchResult]>::from(Vec::new()),
            );
        }

        // ── Spine projection path ──────────────────────────────────────
        // When a sigil segment owns the list, build rows from the Spine
        // model instead of running normal fuzzy/root grouping.
        if !popup_owns_live_main_list
            && !active_filter_head_owns_live_main_list
            && self.spine_projection_owns_main_list()
            && self.spine_parse.input == live_filter_text
        {
            if let Some(projection) = self.spine_projection.as_ref() {
                let preview_needs = match &projection.active_segment_kind {
                    crate::spine::SpineSegmentKind::Style { .. } => {
                        Some(crate::spine::live_preview::SpinePreviewNeeds::STYLE)
                    }
                    crate::spine::SpineSegmentKind::ContextMention { sub_query, .. }
                        if sub_query.is_none() =>
                    {
                        Some(crate::spine::live_preview::SpinePreviewNeeds::CONTEXT_ROOT)
                    }
                    _ => None,
                };
                if let Some(needs) = preview_needs {
                    if needs.cheap_context {
                        self.spine_live_preview_cache
                            .set_script_count(self.scripts.len());
                    }
                    self.spine_live_preview_cache
                        .refresh_preview_nonblocking(needs);
                }

                let preview_generation = preview_needs
                    .map(|_| self.spine_live_preview_cache.generation)
                    .unwrap_or(0);
                let spine_cache_key = format!(
                    "{}\x1Fpreview-gen={preview_generation}",
                    crate::spine::spine_projection_cache_key(
                        live_filter_text,
                        computed_filter_text,
                        &self.spine_parse,
                        projection,
                    ),
                );
                // Rich subsearch bypass: @file:/@clipboard:/etc. produce native
                // rows with proper icons and preview. An empty @source: prefix
                // renders recents UNARMED (no selected row) so accepting the
                // root subsearch row does not auto-arm the first concrete
                // file/clipboard/history result; Down/click remains explicit
                // (see spine_empty_subsearch_selection_suppressed). The typing
                // affordance is ghost text in the input; the choose affordance
                // rides the first section header.
                if let Some((rich_source, rich_query)) = active_rich_spine_subsearch(projection) {
                    let rich_gen = match rich_source {
                        crate::spine::catalog_subsearch::ContextSubsearchSource::File
                        | crate::spine::catalog_subsearch::ContextSubsearchSource::Project => {
                            self.spine_file_search_generation
                        }
                        _ => 0,
                    };
                    // Project results depend on the picked cwd; fold its
                    // revision into the key so a cwd switch mid-colon-mode
                    // can't serve rows from the previous scope.
                    let rich_scope_rev = match rich_source {
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Project => {
                            self.spine_cwd_revision
                        }
                        _ => 0,
                    };
                    let rich_cache_key = format!(
                        "{spine_cache_key}\x1Frich={rich_source:?}\x1Frich-gen={rich_gen}\x1Frich-scope-rev={rich_scope_rev}"
                    );
                    if self
                        .main_menu_result_caches
                        .has_grouped_results_for(&rich_cache_key)
                    {
                        return self.main_menu_result_caches.clone_grouped_results();
                    }

                    let (mut grouped_items, mut flat_results) = match rich_source {
                        crate::spine::catalog_subsearch::ContextSubsearchSource::File => {
                            let recent = self.recent_file_results_from_frecency(
                                crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT,
                            );
                            build_rich_file_subsearch_rows(
                                FileSubsearchFlavor::Global,
                                &rich_query,
                                self.spine_file_search_loading,
                                &self.spine_file_search_results,
                                &recent,
                            )
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Project => {
                            // Frecency recents are global; only the ones
                            // inside the scoped cwd belong in the project
                            // landing state.
                            let cwd_prefix = self
                                .spine_cwd
                                .as_ref()
                                .map(|p| p.to_string_lossy().to_string());
                            let recent: Vec<crate::file_search::FileResult> = self
                                .recent_file_results_from_frecency(
                                    crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT,
                                )
                                .into_iter()
                                .filter(|file| {
                                    cwd_prefix
                                        .as_deref()
                                        .is_some_and(|prefix| file.path.starts_with(prefix))
                                })
                                .collect();
                            build_rich_file_subsearch_rows(
                                FileSubsearchFlavor::Project,
                                &rich_query,
                                self.spine_file_search_loading,
                                &self.spine_file_search_results,
                                &recent,
                            )
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard => {
                            let options =
                                crate::clipboard_history::RootClipboardHistorySectionOptions {
                                    enabled: true,
                                    max_results:
                                        crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT,
                                    min_query_chars: 0,
                                    ..Default::default()
                                };
                            let hits =
                                crate::clipboard_history::search_root_clipboard_history_meta_direct(
                                    &rich_query,
                                    options,
                                );
                            build_rich_clipboard_subsearch_rows(&rich_query, &hits)
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::BrowserHistory => {
                            let options =
                                crate::browser_history::RootBrowserHistorySectionOptions {
                                    enabled: true,
                                    max_results:
                                        crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT,
                                    min_query_chars: 0,
                                    ..Default::default()
                                };
                            let hits =
                                crate::browser_history::search_root_browser_history_meta_direct(
                                    &rich_query,
                                    options,
                                );
                            build_rich_browser_history_rows(&rich_query, &hits)
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Notes => {
                            let options = crate::notes::RootNotesSectionOptions {
                                enabled: true,
                                max_results:
                                    crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT,
                                min_query_chars: 0,
                                ..Default::default()
                            };
                            let hits =
                                crate::notes::search_root_notes_meta_direct(&rich_query, options);
                            build_rich_notes_rows(&rich_query, &hits)
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Dictation => {
                            let options = crate::dictation::RootDictationHistorySectionOptions {
                                enabled: true,
                                max_results:
                                    crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT,
                                min_query_chars: 0,
                                ..Default::default()
                            };
                            let hits = crate::dictation::search_root_dictation_history_direct(
                                &rich_query,
                                options,
                            );
                            build_rich_dictation_rows(&rich_query, &hits)
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::History => {
                            let hits = crate::ai::agent_chat::ui::history::search_history_direct(
                                &rich_query,
                                crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT,
                            );
                            build_rich_agent_chat_history_rows(&rich_query, &hits)
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Scripts => {
                            build_rich_script_rows(&rich_query, &self.scripts)
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Scriptlets => {
                            build_rich_scriptlet_rows(&rich_query, &self.scriptlets)
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Skills => {
                            build_rich_skill_rows(&rich_query, &self.skills)
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Calendar => {
                            build_rich_provider_json_rows(
                                &rich_query,
                                crate::mcp_resources::ProviderJsonResourceKind::Calendar,
                                "Calendar Events",
                                "calendar",
                            )
                        }
                        crate::spine::catalog_subsearch::ContextSubsearchSource::Notifications => {
                            build_rich_provider_json_rows(
                                &rich_query,
                                crate::mcp_resources::ProviderJsonResourceKind::Notifications,
                                "Notifications",
                                "bell",
                            )
                        }
                    };

                    if rich_query.trim().is_empty() {
                        append_choose_hint_to_first_section_header(&mut grouped_items);
                    }

                    // Colon-mode parity with the Agent Chat context selector:
                    // inline `@file:` results keep an explicit full-portal
                    // fallback row that opens the built-in File Search
                    // surface with the current sub-query.
                    if rich_source == crate::spine::catalog_subsearch::ContextSubsearchSource::File
                    {
                        if let Some(segment) = self
                            .spine_parse
                            .segments
                            .get(projection.active_segment_index)
                        {
                            let idx = flat_results.len();
                            flat_results.push(scripts::SearchResult::SpineProjection(
                                crate::spine::SpineListRow {
                                    id: "spine:@:file-full-search".into(),
                                    kind: crate::spine::SpineListRowKind::ContextSubSearch {
                                        context_type: "file".into(),
                                    },
                                    title: "Open full File Search".into(),
                                    subtitle: Some("Browse files with preview".into()),
                                    meta: None,
                                    icon: Some("file-search".into()),
                                    badges: vec![],
                                    score: 0,
                                    is_selectable: true,
                                    action_label: Some("Search".into()),
                                    action: crate::spine::SpineListAction::OpenFileSearchPortal {
                                        segment_index: projection.active_segment_index,
                                        segment_byte_range: segment.byte_range.clone(),
                                        query: rich_query.clone().into(),
                                    },
                                },
                            ));
                            grouped_items.push(GroupedListItem::Item(idx));
                        }
                    }

                    let (first_sel, last_sel) =
                        grouped_selectable_bounds(&grouped_items, &flat_results);
                    self.main_menu_result_caches.store_grouped_results(
                        rich_cache_key,
                        grouped_items,
                        flat_results,
                        first_sel,
                        last_sel,
                    );
                    return self.main_menu_result_caches.clone_grouped_results();
                }

                if let crate::spine::SpineSegmentKind::ProjectCwd { sub_query } =
                    &projection.active_segment_kind
                {
                    let recent_dirs = self.recent_directory_results_from_frecency(
                        crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT,
                    );
                    let has_query = sub_query.as_ref().is_some_and(|q| !q.trim().is_empty());
                    if !recent_dirs.is_empty() {
                        let cwd_cache_key = format!(
                            "{spine_cache_key}\x1Fcwd-rich\x1Fcwd-rev={}",
                            self.spine_cwd_revision
                        );
                        if self
                            .main_menu_result_caches
                            .has_grouped_results_for(&cwd_cache_key)
                        {
                            return self.main_menu_result_caches.clone_grouped_results();
                        }
                        let (grouped_items, flat_results) = if has_query {
                            build_rich_cwd_subsearch_rows(
                                sub_query.as_deref().unwrap_or(""),
                                &recent_dirs,
                            )
                        } else {
                            build_rich_cwd_root_rows(&recent_dirs)
                        };
                        let (first_sel, last_sel) =
                            grouped_selectable_bounds(&grouped_items, &flat_results);
                        self.main_menu_result_caches.store_grouped_results(
                            cwd_cache_key,
                            grouped_items,
                            flat_results,
                            first_sel,
                            last_sel,
                        );
                        return self.main_menu_result_caches.clone_grouped_results();
                    }
                }

                if self
                    .main_menu_result_caches
                    .has_grouped_results_for(&spine_cache_key)
                {
                    return self.main_menu_result_caches.clone_grouped_results();
                }

                let live_preview = preview_needs.map(|_| &self.spine_live_preview_cache.current);
                let cwd_context = if matches!(
                    projection.active_segment_kind,
                    crate::spine::SpineSegmentKind::ProjectCwd { .. }
                ) {
                    main_menu_agent_chat_cwd_context()
                } else {
                    None
                };

                let sections =
                    crate::spine::list::build_spine_list_sections_full_with_resolved_tokens_and_context(
                        &self.spine_parse,
                        projection,
                        live_preview,
                        &|token| self.spine_mention_aliases.contains_key(token),
                        crate::spine::list::SpineListBuildContext {
                            current_cwd: cwd_context.as_ref().map(|context| context.0.as_path()),
                            cwd_recents: cwd_context
                                .as_ref()
                                .map(|context| context.1.as_slice())
                                .unwrap_or(&[]),
                        },
                    );
                let mut grouped_items = Vec::new();
                let mut flat_results: Vec<scripts::SearchResult> = Vec::new();
                for section in sections {
                    grouped_items.push(GroupedListItem::SectionHeader(
                        section.title.to_string(),
                        section.icon.as_ref().map(|icon| icon.as_ref().to_string()),
                    ));
                    for row in section.rows {
                        if !row.is_selectable {
                            // No informational list items in the main menu:
                            // non-selectable rows (empty placeholders, hints)
                            // render as section headers — visible guidance
                            // that can never look like an actionable result.
                            // Fold the subtitle hint ("Try @selection…") into
                            // the header label, matching the "· ↓ to choose"
                            // header-affordance pattern.
                            let mut label = row.title.to_string();
                            if let Some(subtitle) = row.subtitle.as_ref() {
                                if !subtitle.is_empty() {
                                    label.push_str(" \u{b7} ");
                                    label.push_str(subtitle.as_ref());
                                }
                            }
                            grouped_items.push(GroupedListItem::SectionHeader(
                                label,
                                row.icon.as_ref().map(|icon| icon.as_ref().to_string()),
                            ));
                            continue;
                        }
                        let flat_index = flat_results.len();
                        flat_results.push(scripts::SearchResult::SpineProjection(row));
                        grouped_items.push(GroupedListItem::Item(flat_index));
                    }
                }

                let (first_sel, last_sel) =
                    grouped_selectable_bounds(&grouped_items, &flat_results);
                self.main_menu_result_caches.store_grouped_results(
                    spine_cache_key,
                    grouped_items,
                    flat_results,
                    first_sel,
                    last_sel,
                );
                return self.main_menu_result_caches.clone_grouped_results();
            }
        }

        #[cfg(target_os = "macos")]
        let tracked_frontmost_app = frontmost_app_tracker::get_last_real_app();
        #[cfg(target_os = "macos")]
        let current_app_commands_app_name = tracked_frontmost_app
            .as_ref()
            .map(|app| app.name.clone())
            .filter(|name| !name.trim().is_empty());
        #[cfg(not(target_os = "macos"))]
        let current_app_commands_app_name: Option<String> = None;

        let grouped_advanced_query = self
            .menu_syntax_mode
            .advanced_query_for(&self.computed_filter_text)
            .cloned();
        let grouped_source_filters = grouped_advanced_query
            .as_ref()
            .map(|query| query.source_filters.clone())
            .unwrap_or_default();
        let ai_vault_generation = crate::ai_vault::root_ai_vault_snapshot_status().generation;
        let browser_tabs_generation =
            crate::browser_tabs::root_browser_tabs_snapshot_status().generation;
        let browser_history_generation =
            crate::browser_history::root_browser_history_snapshot_status().generation;
        let root_windows_generation = self.root_search.root_windows_refresh_generation();
        let brain_inbox_epoch = self.root_search.root_brain_inbox_epoch();
        let grouped_source_filter_key = format!("{grouped_source_filters:?}");
        let grouped_cache_key = match current_app_commands_app_name.as_deref() {
            Some(app_name) => format!(
                "{}\x1Fsource-filters={grouped_source_filter_key}\x1Fcurrent-app={app_name}\x1Fai-vault-gen={ai_vault_generation}\x1Fwindows-gen={root_windows_generation}\x1Fbrowser-tabs-gen={browser_tabs_generation}\x1Fbrowser-history-gen={browser_history_generation}\x1Fbrain-inbox-epoch={brain_inbox_epoch}",
                self.computed_filter_text
            ),
            None => format!(
                "{}\x1Fsource-filters={grouped_source_filter_key}\x1Fai-vault-gen={ai_vault_generation}\x1Fwindows-gen={root_windows_generation}\x1Fbrowser-tabs-gen={browser_tabs_generation}\x1Fbrowser-history-gen={browser_history_generation}\x1Fbrain-inbox-epoch={brain_inbox_epoch}",
                self.computed_filter_text
            ),
        };

        // P3: Key off computed_filter_text for two-stage filtering
        if self
            .main_menu_result_caches
            .has_grouped_results_for(&grouped_cache_key)
        {
            // NOTE: Removed cache HIT log - fires every render frame, causing log spam.
            // Cache hits are normal operation. Only log cache MISS (below) for diagnostics.
            return self.main_menu_result_caches.clone_grouped_results();
        }

        let should_refresh_root_recent_files = self.computed_filter_text.is_empty()
            || matches!(
                self.root_search.root_file_search_mode,
                Some(crate::file_search::RootFileSectionMode::GlobalQuery)
            )
            || self
                .menu_syntax_mode
                .advanced_query_for(&self.computed_filter_text)
                .is_some_and(|query| {
                    query.free_text.trim().is_empty()
                        && query
                            .source_filters
                            .includes(crate::menu_syntax::RootUnifiedSourceFilter::Files)
                });
        if should_refresh_root_recent_files {
            self.refresh_root_recent_file_results();
        }

        // Cache miss - need to recompute
        if logging::filter_perf_trace_enabled() {
            logging::log(
                "FILTER_PERF",
                &format!("[4b/5] GROUP_START for '{}'", self.computed_filter_text),
            );
        }

        let start = std::time::Instant::now();
        let suggested_config = self.config.get_suggested();

        // Get menu bar items from the background tracker (pre-fetched when apps activate)
        #[cfg(target_os = "macos")]
        let (menu_bar_items, menu_bar_bundle_id): (
            Vec<menu_bar::MenuBarItem>,
            Option<String>,
        ) = {
            let cached = frontmost_app_tracker::get_cached_menu_items();
            let bundle_id = tracked_frontmost_app
                .as_ref()
                .map(|app| app.bundle_id.clone());
            // No conversion needed - tracker is compiled as part of binary crate
            // so it already returns binary crate types
            (cached, bundle_id)
        };
        #[cfg(not(target_os = "macos"))]
        let (menu_bar_items, menu_bar_bundle_id): (
            Vec<menu_bar::MenuBarItem>,
            Option<String>,
        ) = (Vec::new(), None);

        if logging::filter_perf_trace_enabled() {
            logging::log(
                "APP",
                &format!(
                    "get_grouped_results: filter='{}', menu_bar_items={}, bundle_id={:?}",
                    self.computed_filter_text,
                    menu_bar_items.len(),
                    menu_bar_bundle_id
                ),
            );
        }
        let raw_filter_text = self.computed_filter_text.clone();
        let spine_owns_for_computed =
            self.spine_projection_owns_main_list() && self.spine_parse.input == raw_filter_text;
        let popup_owns_computed_main_list = self.menu_syntax_object_selector_state.owns_main_list()
            || self.menu_syntax_trigger_picker_state.owns_main_list();
        let active_filter_head_owns_computed_main_list =
            crate::menu_syntax::active_filter_head_owns_main_list(&raw_filter_text);
        let menu_syntax_owns_main_list = popup_owns_computed_main_list
            || (!spine_owns_for_computed
                && (self
                    .menu_syntax_mode
                    .capture_composer_owns_input_for(&raw_filter_text)
                    || self
                        .menu_syntax_mode
                        .command_owns_input_for(&raw_filter_text)
                    || active_filter_head_owns_computed_main_list));

        let (grouped_items, flat_results) = if self
            .menu_syntax_object_selector_state
            .owns_main_list()
        {
            if let Some(snapshot) = self.menu_syntax_object_selector_state.snapshot.as_ref() {
                build_menu_syntax_object_selector_main_list_results(snapshot)
            } else {
                (Vec::new(), Vec::new())
            }
        } else if self.menu_syntax_trigger_picker_state.owns_main_list() {
            if let Some(snapshot) = self.menu_syntax_trigger_picker_state.snapshot.as_ref() {
                build_menu_syntax_trigger_picker_main_list_results(snapshot)
            } else {
                (Vec::new(), Vec::new())
            }
        } else if menu_syntax_owns_main_list {
            // A composer-style menu-syntax surface owns the input. Suppress
            // the main launcher list so fuzzy search, capture-handler rows,
            // and the "Use X with…" fallback section do not leak through the
            // active composer/form surface. Refine (`:`) remains structured
            // launcher search and is handled below.
            (Vec::new(), Vec::new())
        } else if let Some(invocation) = self.menu_syntax_mode.capture_for(&raw_filter_text) {
            // Capture mode replaces the normal launcher grouping entirely.
            // Do not mix with Suggested/Favorites/Recent/menu-bar/fallback.
            crate::scripts::build_capture_mode_results(&self.scripts, invocation)
        } else if let Some(hint) = self.menu_syntax_mode.incomplete_hint_for(&raw_filter_text) {
            // Menu-syntax trigger picker rows are now owned by the main ScriptList surface.
            // The main launcher result list stays suppressed while trigger rows render
            // through menu_syntax_main_hint_snapshot.
            crate::scripts::build_menu_syntax_hint_results(hint)
        } else {
            let search_text_owned =
                crate::menu_syntax::free_text_for_search(&self.menu_syntax_mode, &raw_filter_text)
                    .to_string();
            let search_text = search_text_owned.as_str();
            let advanced_query_owned = self
                .menu_syntax_mode
                .advanced_query_for(&raw_filter_text)
                .cloned();
            let source_filters = advanced_query_owned
                .as_ref()
                .map(|query| query.source_filters.clone())
                .unwrap_or_default();
            let advanced_query = advanced_query_owned.as_ref();
            let advanced_predicate_query = advanced_query.filter(|query| query.has_predicates());
            let advanced_predicate_active = advanced_predicate_query.is_some();
            let unified_search = self.config.get_unified_search();
            let mut root_file_options = unified_search.root_file_section_options();
            let mut todo_options = unified_search.todo_section_options();
            let mut brain_options = unified_search.brain_section_options();
            let mut notes_options = unified_search.notes_section_options();
            let mut agent_chat_history_options =
                unified_search.agent_chat_history_section_options();
            let mut ai_vault_options = unified_search.ai_vault_section_options();
            let mut clipboard_history_options =
                self.config.root_clipboard_history_section_options();
            let mut dictation_history_options = unified_search.dictation_history_section_options();
            let mut browser_tabs_options = unified_search.browser_tabs_section_options();
            let mut browser_history_options = unified_search.browser_history_section_options();
            let root_passive_source_order = unified_search.passive_source_order();
            let root_passive_result_limits = unified_search.passive_result_limits();
            let explicit_source_result_target = root_passive_result_limits.max_total_results;
            if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Files) {
                root_file_options.files_enabled = true;
                root_file_options.global_search_enabled = true;
                root_file_options.directory_browse_enabled = true;
                root_file_options.recent_files_enabled = true;
                root_file_options.query_intent =
                    crate::file_search::RootFileQueryIntent::ExplicitFilesSourceFilter;
                let visible_limit = self.root_file_source_chip_visible_limit_for(
                    &raw_filter_text,
                    search_text,
                    advanced_predicate_active,
                    self.root_search.root_file_search_mode,
                );
                root_file_options.source_chip_visible_limit = Some(visible_limit);
                if search_text.trim().is_empty() && !advanced_predicate_active {
                    root_file_options.source_filter_browse_target_visible_rows =
                        Some(visible_limit);
                }
            }
            if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Brain) {
                brain_options.enabled = true;
                brain_options.min_query_chars = 0;
                brain_options.max_results =
                    brain_options.max_results.max(explicit_source_result_target);
            }
            if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Notes) {
                notes_options.enabled = true;
                notes_options.min_query_chars = 0;
                notes_options.max_results =
                    notes_options.max_results.max(explicit_source_result_target);
            }
            if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Todo) {
                todo_options.enabled = true;
                todo_options.min_query_chars = 0;
                todo_options.max_results =
                    todo_options.max_results.max(explicit_source_result_target);
            }
            if source_filters
                .includes(crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory)
            {
                clipboard_history_options.enabled = true;
                clipboard_history_options.min_query_chars = 0;
                clipboard_history_options.max_results = clipboard_history_options
                    .max_results
                    .max(explicit_source_result_target);
            }
            if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Dictation) {
                dictation_history_options.enabled = true;
                dictation_history_options.min_query_chars = 0;
                dictation_history_options.max_results = dictation_history_options
                    .max_results
                    .max(explicit_source_result_target);
            }
            if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Conversations) {
                agent_chat_history_options.enabled = true;
                agent_chat_history_options.min_query_chars = 0;
                agent_chat_history_options.max_results = agent_chat_history_options
                    .max_results
                    .max(explicit_source_result_target);
            }
            if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::AiVault) {
                ai_vault_options.enabled = true;
                ai_vault_options.min_query_chars = 0;
                ai_vault_options.max_results = ai_vault_options
                    .max_results
                    .max(explicit_source_result_target);
            }
            if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs) {
                browser_tabs_options.enabled = true;
                browser_tabs_options.min_query_chars = 0;
                browser_tabs_options.max_results = browser_tabs_options
                    .max_results
                    .max(explicit_source_result_target);
            }
            if source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory)
            {
                browser_history_options.enabled = true;
                browser_history_options.min_query_chars = 0;
                browser_history_options.max_age_days = 365;
                browser_history_options.max_results = browser_history_options
                    .max_results
                    .max(explicit_source_result_target);
            }
            let root_passive_frame = self.root_passive_frame_for_current_query(
                search_text,
                advanced_predicate_active,
                source_filters.clone(),
                todo_options,
                brain_options,
                notes_options,
                clipboard_history_options,
                dictation_history_options,
                agent_chat_history_options,
                ai_vault_options.clone(),
                browser_tabs_options.clone(),
                browser_history_options.clone(),
            );
            let root_file_frame = (matches!(
                self.root_search.root_file_search_mode,
                Some(crate::file_search::RootFileSectionMode::GlobalQuery)
            ) && source_filters
                .allows(crate::menu_syntax::RootUnifiedSourceFilter::Files))
            .then(|| {
                self.root_file_frame_for_current_query(
                    search_text,
                    advanced_predicate_active,
                    source_filters.clone(),
                    root_file_options,
                )
            });
            let root_file_search_mode_for_grouping = root_file_frame
                .as_ref()
                .map(|frame| frame.mode)
                .unwrap_or(self.root_search.root_file_search_mode);
            let root_file_search_loading_for_grouping = root_file_frame
                .as_ref()
                .map(|frame| frame.visible_loading)
                .unwrap_or(self.root_search.root_file_search_loading);
            let root_file_results_for_grouping = root_file_frame
                .as_ref()
                .map(|frame| frame.file_results.as_slice())
                .unwrap_or(self.root_search.root_file_results.as_slice());
            let root_recent_file_results_for_grouping = root_file_frame
                .as_ref()
                .map(|frame| frame.recent_file_results.as_slice())
                .unwrap_or(self.root_search.root_recent_file_results.as_slice());
            let dynamic_builtin_entries =
                current_app_commands_app_name.as_deref().map(|app_name| {
                    let mut entries = self.builtin_entries.clone();
                    let commands_label =
                        crate::menu_bar::current_app_commands::current_app_commands_launcher_label(
                            Some(app_name),
                        );
                    if let Some(entry) = entries
                        .iter_mut()
                        .find(|entry| entry.id == "builtin/do-in-current-app")
                    {
                        entry.name = commands_label;
                    }
                    if let Some(entry) = entries
                        .iter_mut()
                        .find(|entry| entry.id == "builtin/dictation")
                    {
                        entry.name = format!("Dictate to {app_name}");
                        entry.description = format!("Voice dictation for {app_name}");
                    }
                    entries
                });
            let builtins_for_grouping = dynamic_builtin_entries
                .as_deref()
                .unwrap_or(&self.builtin_entries);
            let (root_windows, root_windows_provider_status) = self.root_search.root_windows();
            // One roster read feeds both the flow corpus and the degraded
            // discovery note (loading/error/legacy) for the Flows section.
            let flow_roster = crate::flows::catalog::flow_catalog().roster_for(&self.flow_ux_cwd());
            let flow_corpus_for_grouping = crate::flows::catalog::desk_flows(&flow_roster);
            let flow_discovery_note = Some(crate::scripts::FlowDiscoveryNote {
                status: flow_roster.status,
                detail: flow_roster.warnings.first().cloned(),
            });
            crate::scripts::get_grouped_results_with_validation_query_and_root_files_with_options(
                &self.scripts,
                &self.scriptlets,
                builtins_for_grouping,
                &self.apps,
                root_windows,
                root_windows_provider_status,
                &self.skills,
                &flow_corpus_for_grouping,
                flow_discovery_note.as_ref(),
                &self.frecency_store,
                search_text,
                &suggested_config,
                &menu_bar_items,
                menu_bar_bundle_id.as_deref(),
                Some(&self.input_history),
                self.script_validation_report.as_deref(),
                advanced_predicate_query,
                &source_filters,
                root_file_search_mode_for_grouping,
                root_file_search_loading_for_grouping,
                root_file_results_for_grouping,
                root_recent_file_results_for_grouping,
                root_file_options,
                &root_passive_frame.todo_hits,
                todo_options,
                &root_passive_frame.brain_hits,
                brain_options,
                &root_passive_frame.note_hits,
                notes_options,
                &root_passive_frame.clipboard_history_hits,
                clipboard_history_options,
                &root_passive_frame.dictation_history_hits,
                dictation_history_options,
                &root_passive_frame.agent_chat_history_hits,
                agent_chat_history_options,
                &root_passive_frame.ai_vault_hits,
                ai_vault_options,
                &root_passive_frame.browser_tab_hits,
                browser_tabs_options,
                &root_passive_frame.browser_history_hits,
                browser_history_options,
                &root_passive_source_order,
                root_passive_result_limits,
            )
        };
        // A1 decision (2026-06-09): an exact alias match pins the aliased
        // command at the top of the list so Enter runs it. This replaces the
        // old alias-plus-trailing-space auto-execution.
        let (grouped_items, flat_results) = {
            let (mut grouped_items, mut flat_results) = (grouped_items, flat_results);
            if !menu_syntax_owns_main_list && !spine_owns_for_computed {
                let trimmed = raw_filter_text.trim();
                if !trimmed.is_empty() {
                    if let Some(alias_match) = self.find_alias_match(trimmed) {
                        self.pin_alias_match_into_grouped_results(
                            &alias_match,
                            &mut grouped_items,
                            &mut flat_results,
                        );
                    }
                }
            }
            (grouped_items, flat_results)
        };
        // Brain Inbox: pin open curator inbox items at the very top of the
        // empty-query grouped view (mirrors the Script Issues pinned row).
        // The prepend helper itself no-ops on non-empty queries.
        let (grouped_items, flat_results) = {
            let (mut grouped_items, mut flat_results) = (grouped_items, flat_results);
            if !menu_syntax_owns_main_list
                && !spine_owns_for_computed
                && !self.root_search.root_brain_inbox_items().is_empty()
            {
                crate::scripts::prepend_root_brain_inbox_section(
                    &mut grouped_items,
                    &mut flat_results,
                    &raw_filter_text,
                    self.root_search.root_brain_inbox_items(),
                    self.config
                        .get_unified_search()
                        .brain_inbox_section_options(),
                    chrono::Utc::now().timestamp(),
                );
            }
            (grouped_items, flat_results)
        };
        // Conversations are prepended after Brain so they outrank it:
        // ordinary root order is [Conversations, Brain Inbox, everything
        // else]. Suppression policy (explicit, not inherited): the section
        // never renders while menu syntax owns the main list or the spine
        // owns the computed list — those surfaces own their own row sets.
        let (grouped_items, flat_results) = {
            let (mut grouped_items, mut flat_results) = (grouped_items, flat_results);
            if !menu_syntax_owns_main_list && !spine_owns_for_computed {
                let records = self.conversations.ordered_rows();
                let flows = self.flow_desk_corpus();
                crate::scripts::prepend_root_conversations_section(
                    &mut grouped_items,
                    &mut flat_results,
                    &raw_filter_text,
                    &records,
                    &flows,
                    chrono::Utc::now().timestamp(),
                );
            }
            (grouped_items, flat_results)
        };
        let (grouped_items, flat_results) = if menu_syntax_owns_main_list {
            (grouped_items, flat_results)
        } else {
            prepend_inline_calculator_group(
                grouped_items,
                flat_results,
                self.inline_calculator.as_ref(),
            )
        };
        let (grouped_items, flat_results) =
            if let Some(kind) = self.active_script_list_attachment_portal_kind() {
                self.apply_script_list_attachment_portal_filter(kind, flat_results)
            } else {
                (grouped_items, flat_results)
            };
        let elapsed = start.elapsed();

        let (first_selectable_index, last_selectable_index) =
            grouped_selectable_bounds(&grouped_items, &flat_results);

        self.main_menu_result_caches.store_grouped_results(
            grouped_cache_key,
            grouped_items,
            flat_results,
            first_selectable_index,
            last_selectable_index,
        );

        self.refresh_ghost_from_cached_results();

        if logging::filter_perf_trace_enabled() || elapsed >= std::time::Duration::from_millis(8) {
            logging::log(
                "FILTER_PERF",
                &format!(
                    "[4b/5] GROUP_DONE '{}' in {:.2}ms -> {} items (from {} results)",
                    self.computed_filter_text,
                    elapsed.as_secs_f64() * 1000.0,
                    self.main_menu_result_caches.grouped_items().len(),
                    self.main_menu_result_caches.grouped_flat_result_count()
                ),
            );
        }

        // Log total time from input to grouped results if we have the start time
        if let Some(perf_start) = self.main_menu_render_diagnostics.filter_perf_start {
            let total_elapsed = perf_start.elapsed();
            if logging::filter_perf_trace_enabled()
                || total_elapsed >= std::time::Duration::from_millis(16)
            {
                logging::log(
                    "FILTER_PERF",
                    &format!(
                        "[5/5] TOTAL_TIME '{}': {:.2}ms (input->grouped)",
                        self.computed_filter_text,
                        total_elapsed.as_secs_f64() * 1000.0
                    ),
                );
            }
        }

        self.main_menu_result_caches.clone_grouped_results()
    }

    pub(crate) fn cached_grouped_results_snapshot(
        &self,
    ) -> (Arc<[GroupedListItem]>, Arc<[scripts::SearchResult]>) {
        self.main_menu_result_caches.clone_grouped_results()
    }

    pub(crate) fn cached_source_statuses_snapshot(
        &self,
    ) -> Arc<[crate::list_item::SourceChipStatusRow]> {
        self.main_menu_result_caches
            .grouped_source_statuses()
            .to_vec()
            .into()
    }

    /// P1: Invalidate grouped results cache (call when scripts/scriptlets/apps change).
    /// Main-window preflight is row-cache-derived, so it must be invalidated
    /// whenever grouped rows are invalidated.
    pub(crate) fn invalidate_grouped_cache(&mut self) {
        logging::log_debug("CACHE", "Grouped cache INVALIDATED");
        // Set grouped_cache_key to a sentinel that won't match computed_filter_text.
        // This ensures the cache check (computed_filter_text == grouped_cache_key) fails,
        // forcing a recompute on the next get_grouped_results_cached() call.
        // DO NOT set computed_filter_text here - that would cause both to match (false cache HIT).
        self.main_menu_result_caches.invalidate_grouped_results();
        self.invalidate_main_window_preflight();
    }

    /// Get the currently selected search result, correctly mapping from grouped index.
    ///
    /// This function handles the mapping from `selected_index` (which is the visual
    /// position in the grouped list including section headers) to the actual
    /// `SearchResult` in the flat results array.
    ///
    /// Returns `None` if:
    /// - The selected index points to a section header (headers aren't selectable)
    /// - The selected index is out of bounds
    /// - No results exist
    pub(crate) fn get_selected_result(&mut self) -> Option<scripts::SearchResult> {
        let selected_index = self.selected_index;
        self.get_grouped_results_cached();

        let result_idx = self
            .main_menu_result_caches
            .flat_result_index_for_grouped_item(selected_index)?;
        if self
            .inline_calculator_for_result_index(result_idx)
            .is_some()
        {
            None
        } else {
            self.main_menu_result_caches
                .cloned_search_result_for_flat_index(result_idx)
        }
    }

    pub(crate) fn inline_calculator_for_result_index(
        &self,
        result_idx: usize,
    ) -> Option<&crate::calculator::CalculatorInlineResult> {
        if result_idx == INLINE_CALCULATOR_RESULT_INDEX {
            self.inline_calculator.as_ref()
        } else {
            None
        }
    }

    /// Get or update the preview cache for syntax-highlighted code lines.
    /// Only re-reads and re-highlights when the script path actually changes.
    /// Returns cached lines if path matches, otherwise updates cache and returns new lines.
    pub(crate) fn get_or_update_preview_cache(
        &mut self,
        script_path: &str,
        lang: &str,
        is_dark: bool,
    ) -> &[syntax::HighlightedLine] {
        self.get_or_update_preview_cache_with_match(script_path, lang, is_dark, None)
    }

    /// Get or update the preview cache with optional content-match centering.
    /// When `content_match` is provided, the 15-line window is centered on the matched line
    /// and the matched span is emphasized with gold accent at ghost opacity.
    pub(crate) fn get_or_update_preview_cache_with_match(
        &mut self,
        script_path: &str,
        lang: &str,
        is_dark: bool,
        content_match: Option<&scripts::ScriptContentMatch>,
    ) -> &[syntax::HighlightedLine] {
        let match_signature = scripts::preview_match_signature(content_match);
        let matched_line = content_match.map(|cm| cm.line_number);

        let cached_path_matches = self.preview_cache_path.as_deref() == Some(script_path);
        let cached_signature_matches = self.preview_cache_match_signature == match_signature;
        let cache_has_lines = !self.preview_cache_lines.is_empty();

        // Check if cache is valid for this path and match signature
        if scripts::preview_cache_is_valid(
            self.preview_cache_path.as_deref(),
            self.preview_cache_match_signature,
            self.preview_cache_lines.is_empty(),
            script_path,
            content_match,
        ) {
            return &self.preview_cache_lines;
        }

        let miss_reason = if !cached_path_matches {
            "path_changed"
        } else if !cached_signature_matches {
            "match_signature_changed"
        } else if !cache_has_lines {
            "empty_cache"
        } else {
            "unknown"
        };

        // Cache miss - need to re-read and re-highlight
        let cache_miss_start = std::time::Instant::now();
        logging::log(
            "FILTER_PERF",
            &format!(
                "[PREVIEW_CACHE_MISS_REASON] path='{}' reason={} cached_path={:?} cached_match_signature={:?} requested_match_signature={:?}",
                script_path,
                miss_reason,
                self.preview_cache_path,
                self.preview_cache_match_signature,
                match_signature
            ),
        );
        logging::log(
            "FILTER_PERF",
            &format!(
                "[PREVIEW_CACHE_KEY] path='{}' match_signature={:?}",
                script_path, match_signature
            ),
        );
        logging::log(
            "FILTER_PERF",
            &format!(
                "[PREVIEW_CACHE_MISS] Loading '{}' matched_line={:?}",
                script_path, matched_line
            ),
        );

        self.preview_cache_path = Some(script_path.to_string());
        self.preview_cache_match_signature = match_signature;

        let read_start = std::time::Instant::now();
        self.preview_cache_lines = match std::fs::read_to_string(script_path) {
            Ok(content) => {
                let read_elapsed = read_start.elapsed();
                let all_lines: Vec<&str> = content.lines().collect();
                let total_lines = all_lines.len();

                // Compute the 15-line window: centered on match or starting from line 1
                let (window_start, window_lines) = if let Some(match_ln) = matched_line {
                    // match_ln is 1-based; center it in a 15-line window
                    let zero_idx = match_ln.saturating_sub(1);
                    let start = zero_idx.saturating_sub(7);
                    let end = (start + 15).min(total_lines);
                    let start = end.saturating_sub(15);
                    (start, &all_lines[start..end])
                } else {
                    let end = total_lines.min(15);
                    (0, &all_lines[..end])
                };

                let highlight_start = std::time::Instant::now();
                let preview: String = window_lines.join("\n");
                let mut lines = syntax::highlight_code_lines(&preview, lang, is_dark);
                let highlight_elapsed = highlight_start.elapsed();

                // Apply match emphasis to the matched line's spans
                if let Some(cm) = content_match {
                    let match_line_zero = cm.line_number.saturating_sub(1);
                    if match_line_zero >= window_start {
                        let line_idx_in_window = match_line_zero - window_start;
                        if line_idx_in_window < lines.len() {
                            let raw_line = window_lines[line_idx_in_window];
                            let leading_ws_chars =
                                raw_line.chars().take_while(|ch| ch.is_whitespace()).count();
                            // `line_match_indices` are relative to the trimmed snippet shown in
                            // the list row. Convert them back into offsets within the full preview
                            // line so indented matches highlight the correct span.
                            if let (Some(&first), Some(&last)) =
                                (cm.line_match_indices.first(), cm.line_match_indices.last())
                            {
                                Self::apply_match_emphasis_to_line(
                                    &mut lines[line_idx_in_window],
                                    leading_ws_chars + first,
                                    leading_ws_chars + last + 1,
                                );
                            }
                        }
                    }
                }

                logging::log(
                    "FILTER_PERF",
                    &format!(
                        "[PREVIEW_CACHE_MISS] read={:.2}ms highlight={:.2}ms ({} bytes, {} lines, window_start={})",
                        read_elapsed.as_secs_f64() * 1000.0,
                        highlight_elapsed.as_secs_f64() * 1000.0,
                        content.len(),
                        lines.len(),
                        window_start
                    ),
                );

                lines
            }
            Err(e) => {
                logging::log("ERROR", &format!("Failed to read preview: {}", e));
                Vec::new()
            }
        };

        let cache_miss_elapsed = cache_miss_start.elapsed();
        logging::log(
            "FILTER_PERF",
            &format!(
                "[PREVIEW_CACHE_MISS] Total={:.2}ms for '{}'",
                cache_miss_elapsed.as_secs_f64() * 1000.0,
                script_path
            ),
        );

        &self.preview_cache_lines
    }

    /// Apply match emphasis to a specific character range within a highlighted line.
    /// Splits spans as needed so that only the matched range gets `is_match_emphasis = true`.
    fn apply_match_emphasis_to_line(
        line: &mut syntax::HighlightedLine,
        match_start: usize,
        match_end: usize,
    ) {
        if match_start >= match_end {
            return;
        }
        let mut new_spans = Vec::new();
        let mut char_offset: usize = 0;

        for span in line.spans.drain(..) {
            let span_len = span.text.chars().count();
            let span_end = char_offset + span_len;

            if span_end <= match_start || char_offset >= match_end {
                // Entirely outside the match range — keep as-is
                new_spans.push(span);
            } else {
                // This span overlaps with the match range — split it
                let overlap_start = match_start.saturating_sub(char_offset);
                let overlap_end = (match_end - char_offset).min(span_len);

                let chars: Vec<char> = span.text.chars().collect();

                // Before-match portion
                if overlap_start > 0 {
                    let before: String = chars[..overlap_start].iter().collect();
                    new_spans.push(syntax::HighlightedSpan::new(before, span.color));
                }

                // Matched portion — with emphasis
                let matched: String = chars[overlap_start..overlap_end].iter().collect();
                new_spans.push(syntax::HighlightedSpan::with_match_emphasis(
                    matched, span.color,
                ));

                // After-match portion
                if overlap_end < span_len {
                    let after: String = chars[overlap_end..].iter().collect();
                    new_spans.push(syntax::HighlightedSpan::new(after, span.color));
                }
            }

            char_offset = span_end;
        }

        line.spans = new_spans;
    }

    /// Invalidate the preview cache (call when scripts are reloaded or selection changes)
    pub(crate) fn invalidate_preview_cache(&mut self) {
        self.preview_cache_path = None;
        self.preview_cache_match_signature = None;
        self.preview_cache_lines.clear();
    }

    /// Builds the matcher + synthetic-result fallback for the resolved alias
    /// target and pins it at grouped index 0 (A1: "alias means index 0").
    fn pin_alias_match_into_grouped_results(
        &self,
        alias_match: &AliasMatch,
        grouped_items: &mut Vec<crate::list_item::GroupedListItem>,
        flat_results: &mut Vec<crate::scripts::SearchResult>,
    ) {
        use crate::scripts::SearchResult;

        // Pinned positionally; the score only matters if a later pass re-sorts.
        let pin_score = i32::MAX;

        let is_alias_target: Box<dyn Fn(&SearchResult) -> bool> = match alias_match {
            AliasMatch::Script(script) => {
                let path = script.path.clone();
                Box::new(
                    move |result| matches!(result, SearchResult::Script(sm) if sm.script.path == path),
                )
            }
            AliasMatch::Scriptlet(scriptlet) => {
                let name = scriptlet.name.clone();
                let file_path = scriptlet.file_path.clone();
                Box::new(move |result| {
                    matches!(
                        result,
                        SearchResult::Scriptlet(sm)
                            if sm.scriptlet.name == name && sm.scriptlet.file_path == file_path
                    )
                })
            }
            AliasMatch::BuiltIn(entry) => {
                let id = entry.id.clone();
                Box::new(
                    move |result| matches!(result, SearchResult::BuiltIn(bm) if bm.entry.id == id),
                )
            }
            AliasMatch::App(app) => {
                let path = app.path.clone();
                Box::new(
                    move |result| matches!(result, SearchResult::App(am) if am.app.path == path),
                )
            }
        };

        let fallback: Box<dyn Fn() -> SearchResult> = match alias_match {
            AliasMatch::Script(script) => {
                let script = script.clone();
                Box::new(move || {
                    SearchResult::Script(crate::scripts::ScriptMatch {
                        script: script.clone(),
                        score: pin_score,
                        filename: script
                            .path
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        match_indices: crate::scripts::MatchIndices::default(),
                        match_kind: crate::scripts::ScriptMatchKind::default(),
                        content_match: None,
                        match_evidence: None,
                    })
                })
            }
            AliasMatch::Scriptlet(scriptlet) => {
                let scriptlet = scriptlet.clone();
                Box::new(move || {
                    SearchResult::Scriptlet(crate::scripts::ScriptletMatch {
                        scriptlet: scriptlet.clone(),
                        score: pin_score,
                        display_file_path: None,
                        match_indices: crate::scripts::MatchIndices::default(),
                        match_evidence: None,
                    })
                })
            }
            AliasMatch::BuiltIn(entry) => {
                let entry = entry.clone();
                Box::new(move || {
                    SearchResult::BuiltIn(crate::scripts::BuiltInMatch {
                        entry: (*entry).clone(),
                        score: pin_score,
                        match_evidence: None,
                    })
                })
            }
            AliasMatch::App(app) => {
                let app = app.clone();
                Box::new(move || {
                    SearchResult::App(crate::scripts::AppMatch {
                        app: (*app).clone(),
                        score: pin_score,
                        match_evidence: None,
                    })
                })
            }
        };

        crate::scripts::pin_alias_match_first(
            grouped_items,
            flat_results,
            is_alias_target.as_ref(),
            fallback.as_ref(),
        );
    }

    pub(crate) fn refresh_ghost_from_cached_results(&mut self) {
        self.refresh_ghost_from_cached_results_with_cx(None);
    }

    pub(crate) fn refresh_ghost_with_input(&mut self, cx: &mut gpui::Context<Self>) {
        self.refresh_ghost_from_cached_results_with_cx(Some(cx));
    }

    fn refresh_ghost_from_cached_results_with_cx(
        &mut self,
        mut cx: Option<&mut gpui::Context<Self>>,
    ) {
        let structural_clear = !matches!(self.current_view, AppView::ScriptList)
            || self.show_actions_popup
            || self.menu_syntax_trigger_picker_state.owns_main_list()
            || crate::menu_syntax::active_filter_head_owns_main_list(&self.filter_text)
            || self.menu_syntax_capture_form_owns_input()
            || self.inline_calculator.is_some();

        if structural_clear {
            self.cancel_ghost_llm_prediction();
            self.clear_ghost_prediction(cx.as_deref_mut());
            return;
        }

        // Spine colon mode owns the ghost slot, independent of the
        // LLM-prediction kill switch below: an empty sub-query shows the
        // decorative "search clipboard…" affordance (never Tab-acceptable),
        // and a typed sub-query clears the ghost entirely so result-derived
        // completions never dangle off a mention token.
        if let Some((source, sub_query)) = self.active_spine_context_subsearch() {
            self.cancel_ghost_llm_prediction();
            if sub_query.trim().is_empty() {
                let hint = crate::scripts::search::ghost::context_subsearch_hint_prediction(
                    &self.computed_filter_text,
                    source.search_hint_noun(),
                    crate::scripts::search::ghost::PredictionRevision {
                        query_rev: self.ghost_llm_generation,
                        catalog_rev: 0,
                        context_rev: 0,
                    },
                );
                self.apply_ghost_prediction(hint, cx.as_deref_mut());
            } else {
                self.clear_ghost_prediction(cx.as_deref_mut());
            }
            return;
        }

        if !crate::scripts::search::ghost::GHOST_PREDICTIONS_ENABLED {
            self.cancel_ghost_llm_prediction();
            self.clear_ghost_prediction(cx.as_deref_mut());
            return;
        }

        let query = self.computed_filter_text.clone();
        let (_, flat_results) = self.main_menu_result_caches.clone_grouped_results();
        // Resolve cwd, then go through the per-cwd cache so we only stat the
        // two context docs per keystroke instead of reading + parsing up to
        // 24k chars each time. `context_for_cwd` mutably borrows `self`, so the
        // query is cloned above to avoid an overlapping immutable borrow.
        let cwd = self
            .spine_cwd
            .clone()
            .or_else(|| std::env::current_dir().ok());
        let (ghost_context, context_rev) = cwd
            .as_deref()
            .map(|cwd| self.ghost_context_cache.context_for_cwd(cwd))
            .unwrap_or_else(|| (crate::scripts::search::ghost::GhostContext::default(), 0));
        // `query_rev` rides the LLM generation so revisions advance on every
        // input change; `context_rev` invalidates when the cwd docs change.
        let revision = crate::scripts::search::ghost::PredictionRevision {
            query_rev: self.ghost_llm_generation,
            catalog_rev: 0,
            context_rev,
        };

        // 1. Command completion always wins and suppresses any pending LLM call.
        if let Some(pred) = crate::scripts::search::ghost::compute_command_ghost_prediction(
            &query,
            &flat_results,
            revision,
        ) {
            self.cancel_ghost_llm_prediction();
            self.apply_ghost_prediction(pred, cx.as_deref_mut());
            return;
        }

        // 2. A cached LLM result wins over the deterministic starter.
        if let Some(pred) = self.cached_ghost_llm_prediction(&query, cwd.as_ref(), context_rev) {
            self.apply_ghost_prediction(pred, cx.as_deref_mut());
            // Keep the cached suffix; no need to spawn another request.
            return;
        }

        // 3. The deterministic starter shows instantly while the LLM is pending
        //    or unavailable. Never blank when a real starter exists.
        let starter = crate::scripts::search::ghost::fallback_prompt_starter_prediction(
            &query,
            revision,
            &ghost_context,
        );
        if let Some(pred) = starter {
            self.apply_ghost_prediction(pred, cx.as_deref_mut());
        } else {
            self.clear_ghost_prediction(cx.as_deref_mut());
        }

        // 4. Only an input-triggered refresh (cx present) may spawn async work.
        if let Some(cx) = cx {
            self.maybe_start_ghost_llm_prediction(
                query,
                flat_results,
                cwd,
                ghost_context,
                context_rev,
                cx,
            );
        }
    }

    /// Writes a prediction into both `ghost_prediction` and the inline
    /// completion suffix, skipping the GPUI update when nothing visible changed
    /// (avoids flicker between equal suffixes/kinds).
    fn apply_ghost_prediction(
        &mut self,
        pred: crate::scripts::search::ghost::GhostPrediction,
        cx: Option<&mut gpui::Context<Self>>,
    ) {
        let suffix = pred.ghost_suffix.clone();
        let suffix_changed = self
            .ghost_prediction
            .as_ref()
            .is_none_or(|current| current.ghost_suffix != suffix || current.kind != pred.kind);
        tracing::info!(
            target: "script_kit::ghost_text",
            query = %pred.query,
            ghost_suffix = %pred.ghost_suffix,
            full_label = %pred.full_label,
            confidence = %pred.confidence,
            ghost_id = pred.ghost_id,
            kind = pred.kind_label(),
            accepts_tab = pred.accepts_tab(),
            "ghost_prediction_applied"
        );
        self.ghost_prediction = Some(pred);
        if suffix_changed {
            if let Some(cx) = cx {
                self.gpui_input_state.update(cx, |state, cx| {
                    state.set_inline_completion_text(suffix, cx);
                });
            }
        }
    }

    fn clear_ghost_prediction(&mut self, cx: Option<&mut gpui::Context<Self>>) {
        self.ghost_prediction = None;
        if let Some(cx) = cx {
            self.gpui_input_state.update(cx, |state, cx| {
                if state.has_inline_completion() {
                    state.clear_inline_completion(cx);
                }
            });
        }
    }

    /// Cancels any in-flight LLM ghost request (best-effort) and bumps the
    /// generation so a late response is discarded on return.
    fn cancel_ghost_llm_prediction(&mut self) {
        if let Some(cancel) = self.ghost_llm_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.ghost_llm_generation = self.ghost_llm_generation.wrapping_add(1).max(1);
    }

    fn ghost_llm_model_id_hint(&self) -> String {
        // Ghost text is now served by the on-device GGUF model, so cache identity
        // is the local model's fingerprint (filename+len+mtime+sampling), not a
        // cloud provider/model id.
        crate::ai::local_llm::ghost_model_id_hint(&self.config)
    }

    fn cached_ghost_llm_prediction(
        &mut self,
        query: &str,
        cwd: Option<&std::path::PathBuf>,
        context_rev: u64,
    ) -> Option<crate::scripts::search::ghost::GhostPrediction> {
        let model_id = self.ghost_llm_model_id_hint();
        let key = crate::scripts::search::ghost::GhostLlmCacheKey {
            query: query.to_string(),
            cwd: cwd.cloned(),
            context_rev,
            model_id,
        };
        self.ghost_llm_cache.retain(|(_, entry)| entry.is_fresh());
        self.ghost_llm_cache
            .iter()
            .find_map(|(candidate_key, entry)| {
                (candidate_key == &key).then(|| entry.prediction.clone())
            })
    }

    fn cache_ghost_llm_prediction(
        &mut self,
        key: crate::scripts::search::ghost::GhostLlmCacheKey,
        prediction: crate::scripts::search::ghost::GhostPrediction,
    ) {
        if let Some(index) = self
            .ghost_llm_cache
            .iter()
            .position(|(candidate_key, _)| candidate_key == &key)
        {
            self.ghost_llm_cache.remove(index);
        }
        self.ghost_llm_cache.push_front((
            key,
            crate::scripts::search::ghost::GhostLlmCacheEntry {
                prediction,
                inserted_at: std::time::Instant::now(),
            },
        ));
        while self.ghost_llm_cache.len() > crate::scripts::search::ghost::GHOST_LLM_CACHE_LIMIT {
            self.ghost_llm_cache.pop_back();
        }
    }

    /// Debounced on-device (GGUF/llama.cpp) ghost prediction side-channel.
    /// Cancels any prior request,
    /// waits `GHOST_LLM_DEBOUNCE_MS`, calls the provider on the background
    /// executor, and writes the sanitized suffix back only if still current.
    fn maybe_start_ghost_llm_prediction(
        &mut self,
        query: String,
        flat_results: std::sync::Arc<[crate::scripts::SearchResult]>,
        cwd: Option<std::path::PathBuf>,
        ghost_context: crate::scripts::search::ghost::GhostContext,
        context_rev: u64,
        cx: &mut gpui::Context<Self>,
    ) {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        const GHOST_LLM_DEBOUNCE_MS: u64 = 320;

        let trimmed = query.trim();
        if !crate::scripts::search::ghost::is_safe_agent_prompt_seed(trimmed) {
            self.cancel_ghost_llm_prediction();
            return;
        }
        // Do not spend an LLM call when a command completion already applies.
        let probe_revision = crate::scripts::search::ghost::PredictionRevision {
            query_rev: self.ghost_llm_generation,
            catalog_rev: 0,
            context_rev,
        };
        if crate::scripts::search::ghost::compute_command_ghost_prediction(
            &query,
            &flat_results,
            probe_revision,
        )
        .is_some()
        {
            self.cancel_ghost_llm_prediction();
            return;
        }

        self.cancel_ghost_llm_prediction();
        self.ghost_llm_generation = self.ghost_llm_generation.wrapping_add(1).max(1);
        let generation = self.ghost_llm_generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.ghost_llm_cancel = Some(cancel.clone());
        let config = self.config.clone();

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(GHOST_LLM_DEBOUNCE_MS))
                .await;
            if cancel.load(Ordering::Relaxed) {
                return;
            }

            let query_for_model = query.trim_end().to_string();
            let config_for_model = config.clone();
            let ghost_context_for_model = ghost_context.clone();
            let cancel_for_model = cancel.clone();
            // On-device GGUF (llama.cpp) generation — no network. Runs on a
            // dedicated actor thread; this background task just awaits the reply.
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::ai::local_llm::generate_ghost_completion(
                        &config_for_model,
                        crate::ai::local_llm::LocalGhostRequest {
                            prompt: crate::ai::local_llm::GhostPromptSpec::Launcher {
                                partial_query: query_for_model,
                                context: ghost_context_for_model,
                            },
                            cancel: cancel_for_model,
                        },
                    )
                    .map(|response| (response.model_id, response.raw_completion))
                })
                .await;

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    if app.ghost_llm_generation != generation {
                        return;
                    }
                    app.ghost_llm_cancel = None;
                    if app.computed_filter_text != query {
                        return;
                    }
                    let (model_id, raw_response) = match result {
                        Ok(pair) => pair,
                        Err(err) => {
                            // Silent fallback: the starter remains visible.
                            tracing::warn!(
                                target: "script_kit::ghost_text",
                                error = %format!("{err:#}"),
                                query = %query,
                                "ghost local llm generation failed; keeping starter"
                            );
                            return;
                        }
                    };
                    let revision = crate::scripts::search::ghost::PredictionRevision {
                        query_rev: generation,
                        catalog_rev: 0,
                        context_rev,
                    };
                    let Some(prediction) =
                        crate::scripts::search::ghost::llm_prediction_from_response(
                            &query,
                            &raw_response,
                            revision,
                        )
                    else {
                        return;
                    };
                    // Final priority guard: don't replace a command completion
                    // that appeared while the LLM was running.
                    let (_, current_flat) = app.main_menu_result_caches.clone_grouped_results();
                    if crate::scripts::search::ghost::compute_command_ghost_prediction(
                        &app.computed_filter_text,
                        &current_flat,
                        revision,
                    )
                    .is_some()
                    {
                        return;
                    }
                    let key = crate::scripts::search::ghost::GhostLlmCacheKey {
                        query: query.clone(),
                        cwd: cwd.clone(),
                        context_rev,
                        model_id,
                    };
                    app.cache_ghost_llm_prediction(key, prediction.clone());
                    app.apply_ghost_prediction(prediction, Some(cx));
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_passive_search_needle_preserves_under_boundary_query() {
        let query = "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS - 1);

        assert_eq!(root_passive_search_needle(&query), query);
    }

    #[test]
    fn root_passive_search_needle_preserves_exact_boundary_query() {
        let query = "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS);

        assert_eq!(root_passive_search_needle(&query), query);
    }

    #[test]
    fn root_passive_search_needle_clamps_over_boundary_ascii_query() {
        let query = "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS + 1);
        let needle = root_passive_search_needle(&query);

        assert_eq!(needle, "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS));
        assert_eq!(needle.chars().count(), ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS);
    }

    #[test]
    fn root_passive_search_needle_clamps_at_multibyte_scalar_boundary() {
        let query = format!(
            "{}🦀trailing",
            "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS - 1)
        );
        let needle = root_passive_search_needle(&query);

        assert!(needle.ends_with('🦀'));
        assert_eq!(needle.chars().count(), ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS);
        assert_eq!(needle, &query[..needle.len()]);
    }

    #[test]
    fn root_passive_search_needle_leaves_original_full_string_unchanged() {
        let query = format!(
            "{}full-tail",
            "界".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS)
        );
        let original = query.clone();
        let needle = root_passive_search_needle(&query).to_string();

        assert_eq!(query, original);
        assert_eq!(needle, "界".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS));
        assert!(query.ends_with("full-tail"));
    }

    fn calculator_result() -> crate::calculator::CalculatorInlineResult {
        crate::calculator::CalculatorInlineResult {
            raw_input: "12 / 3".to_string(),
            normalized_expr: "12 / 3".to_string(),
            operation_name: "Divide".to_string(),
            value: 4.0,
            formatted: "4".to_string(),
            words: "Four".to_string(),
        }
    }

    #[test]
    fn test_prepend_inline_calculator_group_prepends_header_and_item() {
        let grouped_items = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
        ];
        let flat_results = Vec::new();

        let (grouped, flat) = prepend_inline_calculator_group(
            grouped_items,
            flat_results,
            Some(&calculator_result()),
        );

        assert!(matches!(
            grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None))
            if label == INLINE_CALCULATOR_SECTION_LABEL
        ));
        assert!(matches!(
            grouped.get(1),
            Some(GroupedListItem::Item(INLINE_CALCULATOR_RESULT_INDEX))
        ));
        assert!(matches!(
            grouped.get(2),
            Some(GroupedListItem::SectionHeader(_, _))
        ));
        assert!(matches!(grouped.get(3), Some(GroupedListItem::Item(0))));
        assert!(matches!(grouped.get(4), Some(GroupedListItem::Item(1))));
        assert!(flat.is_empty());
    }

    #[test]
    fn test_prepend_inline_calculator_group_is_noop_without_calculator() {
        let grouped_items = vec![GroupedListItem::Item(0)];
        let flat_results = Vec::new();

        let (grouped, flat) = prepend_inline_calculator_group(grouped_items, flat_results, None);

        assert_eq!(grouped.len(), 1);
        assert!(matches!(grouped.first(), Some(GroupedListItem::Item(0))));
        assert!(flat.is_empty());
    }

    #[test]
    fn test_apply_match_emphasis_handles_indented_match_offsets() {
        let mut line = syntax::HighlightedLine {
            spans: vec![syntax::HighlightedSpan::new(
                "    const superUniqueToken = value;",
                0xcccccc,
            )],
        };

        let leading_ws_chars = 4;
        let snippet_match_start = 6;
        let snippet_match_end = 22;
        ScriptListApp::apply_match_emphasis_to_line(
            &mut line,
            leading_ws_chars + snippet_match_start,
            leading_ws_chars + snippet_match_end,
        );

        let emphasized: String = line
            .spans
            .iter()
            .filter(|span| span.is_match_emphasis)
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(emphasized, "superUniqueToken");
    }

    #[test]
    fn preview_match_signature_changes_when_byte_range_changes() {
        let alpha = scripts::ScriptContentMatch {
            line_number: 4,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![6, 7, 8, 9, 10],
            byte_range: 20..25,
        };
        let beta = scripts::ScriptContentMatch {
            line_number: 4,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![14, 15, 16, 17],
            byte_range: 28..32,
        };
        assert_ne!(
            scripts::preview_match_signature(Some(&alpha)),
            scripts::preview_match_signature(Some(&beta))
        );
    }

    #[test]
    fn preview_match_signature_is_none_without_content_match() {
        assert_eq!(scripts::preview_match_signature(None), None);
    }

    #[test]
    fn preview_cache_is_valid_for_identical_match_signature() {
        let alpha = scripts::ScriptContentMatch {
            line_number: 1,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![6, 7, 8, 9, 10],
            byte_range: 6..11,
        };
        assert!(scripts::preview_cache_is_valid(
            Some("/tmp/demo.ts"),
            scripts::preview_match_signature(Some(&alpha)),
            false, // cached_lines_empty
            "/tmp/demo.ts",
            Some(&alpha),
        ));
    }

    #[test]
    fn preview_cache_is_invalid_when_same_line_match_moves_to_new_span() {
        let alpha = scripts::ScriptContentMatch {
            line_number: 1,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![6, 7, 8, 9, 10],
            byte_range: 6..11,
        };
        let beta = scripts::ScriptContentMatch {
            line_number: 1,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![14, 15, 16, 17],
            byte_range: 14..18,
        };
        assert!(!scripts::preview_cache_is_valid(
            Some("/tmp/demo.ts"),
            scripts::preview_match_signature(Some(&alpha)),
            false, // cached_lines_empty
            "/tmp/demo.ts",
            Some(&beta),
        ));
    }

    #[test]
    fn preview_cache_is_invalid_when_cached_lines_are_empty() {
        let alpha = scripts::ScriptContentMatch {
            line_number: 1,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![6, 7, 8, 9, 10],
            byte_range: 6..11,
        };
        assert!(!scripts::preview_cache_is_valid(
            Some("/tmp/demo.ts"),
            scripts::preview_match_signature(Some(&alpha)),
            true, // cached_lines_empty
            "/tmp/demo.ts",
            Some(&alpha),
        ));
    }

    #[test]
    fn empty_context_subsearch_prefix_routes_to_guarded_rich_rows() {
        for (input, expected_source) in [
            (
                "@file:",
                crate::spine::catalog_subsearch::ContextSubsearchSource::File,
            ),
            (
                "@clipboard:",
                crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard,
            ),
            (
                "@history:",
                crate::spine::catalog_subsearch::ContextSubsearchSource::History,
            ),
        ] {
            let parse = crate::spine::parse_spine(input);
            let projection = crate::spine::project_cursor(&parse, input.len());

            assert_eq!(
                active_rich_spine_subsearch(&projection),
                Some((expected_source, String::new())),
                "{input} must route to rich rows so unarmed recents render with the choose hint"
            );
        }
    }

    /// Colon-less root fragments must auto-enter the same guarded rich rows —
    /// typing `@files` IS file search; no "press Enter to refine" picker step
    /// and no informational list items (user rule).
    #[test]
    fn root_context_fragment_routes_to_guarded_rich_rows() {
        for (input, expected_source, expected_query) in [
            (
                "@files",
                crate::spine::catalog_subsearch::ContextSubsearchSource::File,
                "",
            ),
            (
                "@filesreadme",
                crate::spine::catalog_subsearch::ContextSubsearchSource::File,
                "readme",
            ),
            (
                "@clipboard",
                crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard,
                "",
            ),
            (
                "@history",
                crate::spine::catalog_subsearch::ContextSubsearchSource::History,
                "",
            ),
        ] {
            let parse = crate::spine::parse_spine(input);
            let projection = crate::spine::project_cursor(&parse, input.len());

            assert_eq!(
                active_rich_spine_subsearch(&projection),
                Some((expected_source, expected_query.to_string())),
                "{input} must auto-route to rich search rows"
            );
        }
    }

    #[test]
    fn empty_subsearch_choose_hint_rides_first_header_and_adds_no_row() {
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Recent Files".to_string(), Some("file".to_string())),
            GroupedListItem::Item(0),
        ];

        append_choose_hint_to_first_section_header(&mut grouped);

        assert_eq!(
            grouped.len(),
            2,
            "the choose hint must not add list rows; it rides the existing header"
        );
        let Some(GroupedListItem::SectionHeader(label, _)) = grouped.first() else {
            panic!("first grouped entry must remain the section header");
        };
        assert_eq!(label, "Recent Files \u{b7} \u{2193} to choose");
        assert!(matches!(grouped.get(1), Some(GroupedListItem::Item(0))));
    }

    #[test]
    fn choose_hint_skipped_when_no_selectable_rows() {
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), Some("file".to_string())),
            GroupedListItem::SectionHeader("No recent files".to_string(), None),
        ];

        append_choose_hint_to_first_section_header(&mut grouped);

        let Some(GroupedListItem::SectionHeader(label, _)) = grouped.first() else {
            panic!("first grouped entry must remain the section header");
        };
        assert_eq!(
            label, "Files",
            "an empty list must not advertise \u{2193} to choose"
        );
    }

    #[test]
    fn typed_context_subsearch_arms_rich_rows() {
        for (input, expected_source, expected_query) in [
            (
                "@file:readme",
                crate::spine::catalog_subsearch::ContextSubsearchSource::File,
                "readme",
            ),
            (
                "@clipboard:snippet",
                crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard,
                "snippet",
            ),
            (
                "@history:agent",
                crate::spine::catalog_subsearch::ContextSubsearchSource::History,
                "agent",
            ),
        ] {
            let parse = crate::spine::parse_spine(input);
            let projection = crate::spine::project_cursor(&parse, input.len());

            assert_eq!(
                active_rich_spine_subsearch(&projection),
                Some((expected_source, expected_query.to_string())),
                "{input} should still route typed subqueries to native rich rows"
            );
        }
    }
}

pub(crate) fn active_rich_spine_subsearch(
    projection: &crate::spine::SpineCursorProjection,
) -> Option<(
    crate::spine::catalog_subsearch::ContextSubsearchSource,
    String,
)> {
    let crate::spine::SpineSegmentKind::ContextMention {
        context_type,
        sub_query,
    } = &projection.active_segment_kind
    else {
        return None;
    };
    let (source, query) = crate::spine::catalog_subsearch::parse_context_subsearch(
        context_type,
        sub_query.as_deref(),
    )?;
    Some((source, query.trim().to_string()))
}

/// Empty colon-mode "press ↓ to choose" affordance, folded into the first
/// section header ("Recent Clipboard · ↓ to choose"). Headers are
/// non-selectable by construction, so unlike the old selectable guard row
/// this can never be accepted, and the recents list stays unarmed until an
/// explicit Down/click (see `spine_empty_subsearch_selection_suppressed`).
/// Skipped when the list has no selectable item — "↓ to choose" over an
/// empty list would be a lie.
pub(crate) fn append_choose_hint_to_first_section_header(grouped: &mut [GroupedListItem]) {
    if !grouped
        .iter()
        .any(|item| matches!(item, GroupedListItem::Item(_)))
    {
        return;
    }
    for item in grouped.iter_mut() {
        if let GroupedListItem::SectionHeader(label, _) = item {
            label.push_str(" \u{b7} \u{2193} to choose");
            return;
        }
    }
}

/// Which file-backed colon mode the rows belong to: `@file:` (global
/// Spotlight) or `@project:` (cwd-scoped). Same row anatomy, different
/// header language and icon.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileSubsearchFlavor {
    Global,
    Project,
}

impl FileSubsearchFlavor {
    fn icon(self) -> &'static str {
        match self {
            Self::Global => "file",
            Self::Project => "folder",
        }
    }

    fn recent_header(self) -> &'static str {
        match self {
            Self::Global => "Recent Files",
            Self::Project => "Recent Project Files",
        }
    }

    fn finding_header(self) -> &'static str {
        match self {
            Self::Global => "Finding recent files\u{2026}",
            Self::Project => "Finding project files\u{2026}",
        }
    }

    fn empty_section_header(self) -> &'static str {
        match self {
            Self::Global => "Files",
            Self::Project => "Project Files",
        }
    }

    fn no_recent_header(self) -> &'static str {
        match self {
            Self::Global => "No recent files",
            Self::Project => "No recent project files",
        }
    }

    fn matching_header(self, query: &str) -> String {
        match self {
            Self::Global => format!("Files matching \u{201c}{query}\u{201d}"),
            Self::Project => format!("Project files matching \u{201c}{query}\u{201d}"),
        }
    }

    fn searching_header(self) -> &'static str {
        match self {
            Self::Global => "Searching files\u{2026}",
            Self::Project => "Searching project files\u{2026}",
        }
    }

    fn no_match_header(self, query: &str) -> String {
        match self {
            Self::Global => format!("No files matching \u{201c}{query}\u{201d}"),
            Self::Project => format!("No project files matching \u{201c}{query}\u{201d}"),
        }
    }
}

pub(crate) fn build_rich_file_subsearch_rows(
    flavor: FileSubsearchFlavor,
    query: &str,
    loading: bool,
    provider_results: &[crate::file_search::FileResult],
    recent_results: &[crate::file_search::FileResult],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let query = query.trim();
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let icon = flavor.icon().to_string();

    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    if query.is_empty() {
        // A3 recent-files decision (2026-06-09): the empty landing state
        // blends frecency picks (files chosen through Script Kit) with the
        // provider's recently-used seed, deduped by path.
        let mut seen = std::collections::HashSet::new();
        let combined: Vec<&crate::file_search::FileResult> = recent_results
            .iter()
            .chain(provider_results.iter())
            .filter(|file| seen.insert(file.path.as_str()))
            .take(limit)
            .collect();
        if !combined.is_empty() {
            grouped.push(GroupedListItem::SectionHeader(
                flavor.recent_header().to_string(),
                Some(icon),
            ));
            for file in combined {
                let idx = flat.len();
                flat.push(scripts::SearchResult::File(scripts::FileMatch {
                    file: (*file).clone(),
                    score: 0,
                }));
                grouped.push(GroupedListItem::Item(idx));
            }
        } else if loading {
            grouped.push(GroupedListItem::SectionHeader(
                flavor.finding_header().to_string(),
                Some(icon),
            ));
        } else {
            grouped.push(GroupedListItem::SectionHeader(
                flavor.empty_section_header().to_string(),
                Some(icon),
            ));
            grouped.push(GroupedListItem::SectionHeader(
                flavor.no_recent_header().to_string(),
                None,
            ));
        }
    } else if !provider_results.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            flavor.matching_header(query),
            Some(icon),
        ));
        for file in provider_results.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::File(scripts::FileMatch {
                file: file.clone(),
                score: 0,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    } else if loading {
        grouped.push(GroupedListItem::SectionHeader(
            flavor.searching_header().to_string(),
            Some(icon),
        ));
    } else {
        grouped.push(GroupedListItem::SectionHeader(
            flavor.no_match_header(query),
            Some(icon),
        ));
    }

    (grouped, flat)
}

pub(crate) fn build_rich_clipboard_subsearch_rows(
    query: &str,
    hits: &[crate::clipboard_history::ClipboardEntryMeta],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    let header = if query.trim().is_empty() {
        "Recent Clipboard".to_string()
    } else {
        format!("Clipboard matching \u{201c}{}\u{201d}", query.trim())
    };

    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("clipboard".to_string()),
    ));

    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "Clipboard is empty".to_string()
            } else {
                format!(
                    "No clipboard entries matching \u{201c}{}\u{201d}",
                    query.trim()
                )
            },
            None,
        ));
    } else {
        for entry in hits.iter().take(limit) {
            let idx = flat.len();
            let title = crate::spine::text_preview::single_line_truncate(&entry.text_preview, 72);
            flat.push(scripts::SearchResult::ClipboardHistory(
                scripts::ClipboardHistoryMatch {
                    entry: entry.clone(),
                    title: title.clone(),
                    subtitle: "Clipboard History".to_string(),
                    score: 0,
                },
            ));
            grouped.push(GroupedListItem::Item(idx));
        }
    }

    (grouped, flat)
}

pub(crate) fn build_rich_browser_history_rows(
    query: &str,
    hits: &[crate::browser_history::RootBrowserHistorySearchHit],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Recent Browser History".to_string()
    } else {
        format!("Browser history matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("globe".to_string()),
    ));
    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "No browser history".to_string()
            } else {
                format!("No history matching \u{201c}{}\u{201d}", query.trim())
            },
            None,
        ));
    } else {
        for hit in hits.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::BrowserHistory(
                scripts::BrowserHistoryMatch {
                    hit: hit.clone(),
                    subtitle: hit.url.clone(),
                    score: 0,
                },
            ));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_notes_rows(
    query: &str,
    hits: &[crate::notes::RootNoteSearchHit],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Recent Notes".to_string()
    } else {
        format!("Notes matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("notebook-text".to_string()),
    ));
    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "No notes".to_string()
            } else {
                format!("No notes matching \u{201c}{}\u{201d}", query.trim())
            },
            None,
        ));
    } else {
        for hit in hits.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::Note(scripts::NoteMatch {
                hit: hit.clone(),
                title: hit.title.clone(),
                subtitle: format!("{} chars", hit.char_count),
                score: 0,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_dictation_rows(
    query: &str,
    hits: &[crate::dictation::RootDictationHistorySearchHit],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Recent Dictation".to_string()
    } else {
        format!("Dictation matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("mic".to_string()),
    ));
    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "No dictation history".to_string()
            } else {
                format!("No dictation matching \u{201c}{}\u{201d}", query.trim())
            },
            None,
        ));
    } else {
        for hit in hits.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::DictationHistory(
                scripts::DictationHistoryMatch {
                    id: hit.id.clone(),
                    preview: hit.preview.clone(),
                    target: hit.target.clone(),
                    timestamp: hit.timestamp.clone(),
                    audio_duration_ms: hit.audio_duration_ms,
                    subtitle: hit.target.clone(),
                    score: 0,
                    matched_field: hit.matched_field,
                    evidence: hit.evidence.clone(),
                },
            ));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_agent_chat_history_rows(
    query: &str,
    hits: &[crate::ai::agent_chat::ui::history::AgentChatHistorySearchHit],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Recent Agent Chat".to_string()
    } else {
        format!("Chat history matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("message-square".to_string()),
    ));
    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "No chat history".to_string()
            } else {
                format!("No history matching \u{201c}{}\u{201d}", query.trim())
            },
            None,
        ));
    } else {
        for hit in hits.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::AgentChatHistory(
                scripts::AgentChatHistoryMatch {
                    entry: hit.entry.clone(),
                    score: 0,
                    matched_field: hit.matched_field,
                    subtitle: hit.entry.title_display().to_string(),
                    evidence: hit.evidence.clone(),
                },
            ));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_script_rows(
    query: &str,
    all_scripts: &[std::sync::Arc<scripts::Script>],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Scripts".to_string()
    } else {
        format!("Scripts matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("code".to_string()),
    ));
    let query_lower = query.trim().to_lowercase();
    let matches: Vec<_> = all_scripts
        .iter()
        .filter(|s| {
            query_lower.is_empty()
                || s.name.to_lowercase().contains(&query_lower)
                || s.path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.to_lowercase().contains(&query_lower))
        })
        .take(limit)
        .collect();
    if matches.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            format!("No scripts matching \u{201c}{}\u{201d}", query.trim()),
            None,
        ));
    } else {
        for script in matches {
            let idx = flat.len();
            flat.push(scripts::SearchResult::Script(scripts::ScriptMatch {
                script: std::sync::Arc::clone(script),
                score: 0,
                filename: script
                    .path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(&script.name)
                    .to_string(),
                match_indices: scripts::MatchIndices::default(),
                match_kind: scripts::ScriptMatchKind::Name,
                content_match: None,
                match_evidence: None,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_scriptlet_rows(
    query: &str,
    all_scriptlets: &[std::sync::Arc<scripts::Scriptlet>],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Scriptlets".to_string()
    } else {
        format!("Scriptlets matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("workflow".to_string()),
    ));
    let query_lower = query.trim().to_lowercase();
    let matches: Vec<_> = all_scriptlets
        .iter()
        .filter(|s| query_lower.is_empty() || s.name.to_lowercase().contains(&query_lower))
        .take(limit)
        .collect();
    if matches.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            format!("No scriptlets matching \u{201c}{}\u{201d}", query.trim()),
            None,
        ));
    } else {
        for scriptlet in matches {
            let idx = flat.len();
            flat.push(scripts::SearchResult::Scriptlet(scripts::ScriptletMatch {
                scriptlet: std::sync::Arc::clone(scriptlet),
                score: 0,
                display_file_path: None,
                match_indices: scripts::MatchIndices::default(),
                match_evidence: None,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_skill_rows(
    query: &str,
    all_skills: &[std::sync::Arc<crate::plugins::PluginSkill>],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Skills".to_string()
    } else {
        format!("Skills matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("zap".to_string()),
    ));
    let query_lower = query.trim().to_lowercase();
    let matches: Vec<_> = all_skills
        .iter()
        .filter(|s| query_lower.is_empty() || s.title.to_lowercase().contains(&query_lower))
        .take(limit)
        .collect();
    if matches.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            format!("No skills matching \u{201c}{}\u{201d}", query.trim()),
            None,
        ));
    } else {
        for skill in matches {
            let idx = flat.len();
            flat.push(scripts::SearchResult::Skill(scripts::SkillMatch {
                skill: std::sync::Arc::clone(skill),
                score: 0,
                match_indices: scripts::MatchIndices::default(),
                match_evidence: None,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_provider_json_rows(
    query: &str,
    kind: crate::mcp_resources::ProviderJsonResourceKind,
    section_label: &str,
    icon: &str,
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    use crate::spine::list::{ss, SpineListAction, SpineListRow, SpineListRowKind};

    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    let items = crate::mcp_resources::read_provider_json_items(kind);
    let query_lower = query.trim().to_lowercase();

    let header = if query_lower.is_empty() {
        section_label.to_string()
    } else {
        format!("{section_label} matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some(icon.to_string()),
    ));

    let matches: Vec<_> = items
        .iter()
        .filter(|item| {
            query_lower.is_empty()
                || item.title.to_lowercase().contains(&query_lower)
                || item
                    .subtitle
                    .as_deref()
                    .is_some_and(|s| s.to_lowercase().contains(&query_lower))
        })
        .take(limit)
        .collect();

    if matches.is_empty() {
        if items.is_empty() {
            grouped.push(GroupedListItem::SectionHeader(
                format!("No {section_label} available"),
                None,
            ));
        } else {
            grouped.push(GroupedListItem::SectionHeader(
                format!(
                    "No {section_label} matching \u{201c}{}\u{201d}",
                    query.trim()
                ),
                None,
            ));
        }
    } else {
        for (rank, item) in matches.iter().enumerate() {
            let idx = flat.len();
            let prefix = match kind {
                crate::mcp_resources::ProviderJsonResourceKind::Calendar => "calendar",
                crate::mcp_resources::ProviderJsonResourceKind::Notifications => "notifications",
                crate::mcp_resources::ProviderJsonResourceKind::Dictation => "dictation",
            };
            flat.push(scripts::SearchResult::SpineProjection(SpineListRow {
                id: ss(format!("spine:provider-json:{prefix}:{rank}")),
                kind: SpineListRowKind::ContextResult {
                    context_type: ss(prefix),
                    result_id: ss(format!("{rank}")),
                },
                title: ss(item.title.clone()),
                subtitle: item.subtitle.clone().map(ss),
                meta: None,
                icon: Some(ss(icon.to_string())),
                badges: vec![ss("@")],
                score: i32::MAX.saturating_sub(rank as i32),
                is_selectable: true,
                action_label: Some(ss("Attach")),
                action: SpineListAction::Noop,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

fn main_menu_agent_chat_cwd_context() -> Option<(std::path::PathBuf, Vec<std::path::PathBuf>)> {
    let ai_preferences = crate::config::load_user_preferences().ai;
    let profile_ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
    let profile =
        crate::ai::agent_chat::profiles::resolve_effective_profile(&ai_preferences, &profile_ctx);
    // Pure cwd-only resolution: the full launch resolver runs create_dir_all
    // and logs per call, which is unacceptable in this per-keystroke path.
    let cwd =
        crate::ai::agent_chat::launch::resolve_selected_launch_cwd(&ai_preferences, &profile_ctx);
    let recents = crate::ai::agent_chat::ui::agent_chat_cwd_recents_for_profile(&profile.id);
    Some((cwd, recents))
}

pub(crate) fn build_rich_cwd_root_rows(
    recent_dirs: &[crate::file_search::FileResult],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    if !recent_dirs.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            "Recent Directories".to_string(),
            Some("folder".to_string()),
        ));
        for dir in recent_dirs.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::File(scripts::FileMatch {
                file: dir.clone(),
                score: 0,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    } else {
        grouped.push(GroupedListItem::SectionHeader(
            "Project / CWD".to_string(),
            Some("folder".to_string()),
        ));
        grouped.push(GroupedListItem::SectionHeader(
            "No recent directories".to_string(),
            None,
        ));
    }
    (grouped, flat)
}

pub(crate) fn build_rich_cwd_subsearch_rows(
    query: &str,
    recent_dirs: &[crate::file_search::FileResult],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    let q = query.trim().to_lowercase();
    let matches: Vec<_> = recent_dirs
        .iter()
        .filter(|d| d.name.to_lowercase().contains(&q) || d.path.to_lowercase().contains(&q))
        .take(limit)
        .collect();

    let header = if matches.is_empty() {
        format!("No directories matching \u{201c}{}\u{201d}", query.trim())
    } else {
        format!("Directories matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("folder".to_string()),
    ));

    for dir in matches {
        let idx = flat.len();
        flat.push(scripts::SearchResult::File(scripts::FileMatch {
            file: dir.clone(),
            score: 0,
        }));
        grouped.push(GroupedListItem::Item(idx));
    }
    (grouped, flat)
}
