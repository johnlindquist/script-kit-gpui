use super::*;
use crate::design_evaluation::search_fixtures::{
    self as owned_search, ProviderTerminal as PassiveProviderTerminal,
};
use crate::scripts::main_menu_rows::INLINE_CALCULATOR_RESULT_INDEX;
use crate::scripts::root_search_contract::{
    RootLocalContentOptions, RootLocalContentProvider, RootPrivateHistoryProvider,
};

#[path = "filtering_cache_rich_results.rs"]
mod rich_results;
use rich_results::{
    build_rich_agent_chat_history_rows, build_rich_browser_history_rows,
    build_rich_clipboard_subsearch_rows, build_rich_cwd_root_rows, build_rich_cwd_subsearch_rows,
    build_rich_dictation_rows, build_rich_file_subsearch_rows, build_rich_notes_rows,
    build_rich_provider_json_rows, build_rich_script_rows, build_rich_scriptlet_rows,
    build_rich_skill_rows, main_menu_agent_chat_cwd_context, FileSubsearchFlavor,
};

const INLINE_CALCULATOR_SECTION_LABEL: &str = "Calculator";
const ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS: usize = 256;

struct RootPassiveFrameOptions {
    advanced_query_active: bool,
    source_filters: crate::menu_syntax::RootUnifiedSourceFilterSet,
    todo_options: crate::menu_syntax::RootTodoSectionOptions,
    brain_options: crate::brain::RootBrainSectionOptions,
    notes_options: crate::notes::RootNotesSectionOptions,
    clipboard_history_options: crate::clipboard_history::RootClipboardHistorySectionOptions,
    dictation_history_options: crate::dictation::RootDictationHistorySectionOptions,
    agent_chat_history_options:
        crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions,
    ai_vault_options: crate::ai_vault::RootAiVaultSectionOptions,
    browser_tabs_options: crate::browser_tabs::RootBrowserTabsSectionOptions,
    browser_history_options: crate::browser_history::RootBrowserHistorySectionOptions,
}

pub(super) fn root_passive_search_needle(query: &str) -> &str {
    query
        .char_indices()
        .nth(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS)
        .map_or(query, |(byte_index, _)| &query[..byte_index])
}

async fn receive_root_passive_provider<T>(
    receiver: std::sync::mpsc::Receiver<T>,
    executor: &gpui::BackgroundExecutor,
) -> std::result::Result<T, std::sync::mpsc::TryRecvError> {
    loop {
        match receiver.try_recv() {
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                executor.timer(std::time::Duration::from_millis(16)).await;
            }
            terminal => return terminal,
        }
    }
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

include!("filtering_cache_main_list_rows.rs");

impl ScriptListApp {
    pub(crate) fn filter_text(&self) -> &str {
        self.filter_text.as_str()
    }

    pub(crate) fn install_root_windows(
        &mut self,
        windows: Vec<crate::window_control::WindowInfo>,
        cx: &mut Context<Self>,
    ) {
        self.commit_main_menu_results_refresh("root_windows_install", None, cx, |app, _cx| {
            app.cached_windows = windows;
            app.root_search
                .install_root_windows(&app.cached_windows, &app.apps);
            true
        });
    }

    pub(crate) fn rebuild_root_windows_after_app_icon_cache_update(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        if self.cached_windows.is_empty() {
            return;
        }
        self.commit_main_menu_results_refresh(reason, None, cx, |app, _cx| {
            app.root_search
                .rebuild_root_windows(&app.cached_windows, &app.apps);
            true
        });
    }

    pub(crate) fn refresh_root_windows_source(&mut self, cx: &mut Context<Self>) {
        if !self.root_search.query_is_current() {
            return;
        }
        self.root_search.invalidate_root_windows_source_snapshot();
        if let Some(generation) = self.root_search.active_named_provider_generation("windows") {
            self.root_search
                .detach_named_provider_consumer("windows", generation);
        }
        self.maybe_start_root_windows_refresh_for_query(&self.computed_filter_text.clone(), cx);
    }

    pub(crate) fn maybe_start_root_windows_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        if !self.root_search.query_is_current() || self.computed_filter_text != query_text {
            return;
        }
        let owned_gate = self.main_services.search_gate();
        if !self.main_services.is_production() && owned_gate.is_none() {
            return;
        }
        if !self.current_query_includes_root_source(
            query_text,
            crate::menu_syntax::RootUnifiedSourceFilter::Windows,
        ) {
            return;
        }
        if let Some(generation) = self.root_search.active_named_provider_generation("windows") {
            if !self
                .root_search
                .accepts_named_provider("windows", generation)
            {
                self.root_search.note_desired_provider(
                    "windows",
                    "window-snapshot",
                    "window-server",
                    RootProviderPublicationPolicy::Visible,
                );
            }
            return;
        }
        if !self.root_search.root_windows_refresh_needed() {
            return;
        }
        let mut token = 0;
        self.commit_main_menu_results_refresh(
            "root_windows_refresh_started",
            None,
            cx,
            |app, _cx| {
                token = app.root_search.begin_root_windows_refresh();
                app.root_search.begin_named_provider(
                    "windows",
                    token,
                    "window-snapshot",
                    "window-server",
                    RootProviderPublicationPolicy::Visible,
                    // Window discovery is a global snapshot, filtered only when rows commit.
                    false,
                );
                true
            },
        );
        let owned_run = if let Some(gate) = &owned_gate {
            match gate.begin(
                "windows",
                query_text,
                token,
                RootProviderPublicationPolicy::Visible,
            ) {
                Some(run) => Some(Arc::new(run)),
                None => {
                    self.finish_root_windows_worker(
                        token,
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            "owned_windows_gate_unavailable",
                        )
                        .into()),
                        None,
                        cx,
                    );
                    return;
                }
            }
        } else {
            None
        };
        self.ensure_root_app_icons(cx);
        let (tx, rx) = async_channel::bounded(1);
        if let Some(run) = owned_run.clone() {
            cx.background_executor()
                .spawn(async move {
                    run.deliver(
                        move |result| tx.try_send(result),
                        crate::design_evaluation::search_fixtures::window_result,
                    )
                    .await;
                })
                .detach();
        } else {
            cx.background_executor()
                .spawn(async move {
                    let _ = tx.try_send(crate::window_control::list_windows());
                })
                .detach();
        }
        cx.spawn(async move |this, cx| {
            let result = match rx.recv().await {
                Ok(result) => {
                    result.map_err(super::root_file_search::MainSearchWorkerFailure::Source)
                }
                Err(_) => Err(super::root_file_search::MainSearchWorkerFailure::Disconnected),
            };
            if this
                .update(cx, |app, cx| {
                    app.finish_root_windows_worker(token, result, owned_run.as_deref(), cx)
                })
                .is_err()
            {
                if let Some(run) = owned_run.as_deref() {
                    run.finish(
                        crate::design_evaluation::search_fixtures::ProviderTerminal::StaleDiscarded,
                        RootProviderPublicationPolicy::Visible,
                    );
                }
            }
        })
        .detach();
    }

    fn finish_root_windows_worker(
        &mut self,
        token: u64,
        result: super::root_file_search::MainSearchWorkerResult<crate::window_control::WindowInfo>,
        owned_run: Option<&crate::design_evaluation::search_fixtures::SearchRun>,
        cx: &mut Context<Self>,
    ) {
        let accepted = self.root_search.accepts_named_provider("windows", token)
            && self.root_search.root_windows_refresh_token_matches(token);
        if let Some(run) = owned_run {
            run.finish(
                if accepted {
                    super::root_file_search::main_search_fixture_terminal(&result)
                } else {
                    crate::design_evaluation::search_fixtures::ProviderTerminal::StaleDiscarded
                },
                RootProviderPublicationPolicy::Visible,
            );
        }
        if !self
            .root_search
            .named_provider_work_is_current("windows", token)
        {
            return;
        }
        let terminal = super::root_file_search::main_search_worker_terminal(&result);
        if accepted {
            self.commit_main_menu_results_refresh("root_windows_refresh_complete", Some(("windows", token)), cx, |app, _cx| {
                app.root_search.finish_named_provider("windows", token, terminal);
                match result {
                    Ok(windows) => {
                        app.cached_windows = windows;
                        app.root_search.install_root_windows(&app.cached_windows, &app.apps);
                    }
                    Err(error) => {
                        let message = error.to_string();
                        let permission_denied = matches!(&error, super::root_file_search::MainSearchWorkerFailure::Source(error) if error.downcast_ref::<std::io::Error>().is_some_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied));
                        let status = if permission_denied {
                            crate::window_control::RootWindowsProviderStatus::PermissionRequired
                        } else {
                            crate::window_control::RootWindowsProviderStatus::ProviderError { message: message.lines().next().unwrap_or("unknown error").to_owned() }
                        };
                        app.root_search.fail_root_windows_refresh(status);
                    }
                }
                true
            });
        } else {
            self.root_search.finish_named_provider(
                "windows",
                token,
                RootProviderTerminal::StaleDiscarded,
            );
            self.root_search.discard_root_windows_refresh(token);
        }
        if self.root_search.take_named_provider_desired("windows") {
            self.refresh_root_windows_source(cx);
        }
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

    fn record_root_passive_provider_terminal(
        &mut self,
        request: &sk_protocol::search_contract::ProviderRequest,
        owned_run: Option<&owned_search::SearchRun>,
        terminal: PassiveProviderTerminal,
    ) -> bool {
        let changed = self
            .root_search
            .finish_provider_request(request, terminal.into());
        if let Some(run) = owned_run {
            run.finish(terminal, RootProviderPublicationPolicy::Visible);
        }
        changed
    }

    /// Called only inside the accepted source publication transaction.
    fn apply_root_passive_provider_completion(
        &mut self,
        request: &sk_protocol::search_contract::ProviderRequest,
        owned_run: Option<&owned_search::SearchRun>,
        terminal: PassiveProviderTerminal,
        source_changed: bool,
    ) -> bool {
        let terminal =
            if !source_changed && matches!(terminal, PassiveProviderTerminal::Completed { .. }) {
                PassiveProviderTerminal::StaleDiscarded
            } else {
                terminal
            };
        let terminal_changed =
            self.record_root_passive_provider_terminal(request, owned_run, terminal);
        let observable_changed = source_changed
            || (terminal_changed && terminal != PassiveProviderTerminal::StaleDiscarded);
        if observable_changed {
            self.invalidate_root_passive_and_grouped_cache();
        }
        observable_changed
    }

    /// A cache alternative to new provider admission. This neither attaches an
    /// old worker to the query nor consumes demand or starts source work.
    pub(crate) fn owned_search_source_cache_readiness(
        &self,
        source: &str,
    ) -> Option<crate::root_search_store::RootSearchSourceCacheReadiness<'_>> {
        if !crate::runtime_policy::is_owned_evaluation()
            || !matches!(self.current_view, AppView::ScriptList)
            || !self.root_search.query_is_current()
            || self.filter_text != self.computed_filter_text
            || self.main_menu_committed_query_stamp() != Some(self.root_search.query_stamp())
        {
            return None;
        }
        match source {
            "tabs" => {
                if self.root_search.named_provider_in_flight(source) {
                    return None;
                }
                let (options, _) =
                    self.root_browser_tabs_refresh_options_for_query(&self.computed_filter_text)?;
                let cache = crate::browser_tabs::root_browser_tabs_fresh_cache_status(
                    options.cache_ttl_ms,
                )?;
                Some(crate::root_search_store::RootSearchSourceCacheReadiness {
                    query: self.root_search.query_stamp(),
                    identity: "browser-tabs-snapshot",
                    generation: Some(cache.generation),
                    row_count: cache.cached_count,
                })
            }
            "files" | "directory" => self.owned_root_file_cache_readiness(source),
            "history" | "notes" | "todos" | "clipboard" | "dictation" | "conversations"
            | "windows" => {
                if self.root_search.named_provider_in_flight(source) {
                    return None;
                }
                let query = &self.computed_filter_text;
                let (identity, (generation, row_count)) = match source {
                    "history" => {
                        let (options, _) =
                            self.root_browser_history_refresh_options_for_query(query)?;
                        let cache =
                            crate::browser_history::root_browser_history_fresh_cache_status(
                                options.cache_ttl_ms,
                            )?;
                        (
                            "browser-history-snapshot",
                            (cache.generation, cache.cached_count),
                        )
                    }
                    "notes" => {
                        let (needle, RootLocalContentOptions::Notes(options)) = self
                            .root_local_content_refresh_options_for_query(
                                RootLocalContentProvider::Notes,
                                query,
                            )?
                        else {
                            return None;
                        };
                        (
                            "notes-query-cache",
                            crate::notes::root_notes_search_fresh_cache_status(&needle, options)?,
                        )
                    }
                    "todos" => {
                        self.root_local_content_refresh_options_for_query(
                            RootLocalContentProvider::Todos,
                            query,
                        )?;
                        (
                            "todos-snapshot",
                            crate::menu_syntax::root_todos_fresh_cache_status()?,
                        )
                    }
                    "clipboard" => {
                        if !self.root_private_history_provider_is_eligible(
                            RootPrivateHistoryProvider::Clipboard,
                            query,
                        ) {
                            return None;
                        }
                        (
                            "clipboard-history-snapshot",
                            crate::clipboard_history::root_clipboard_history_fresh_cache_status()?,
                        )
                    }
                    "dictation" => {
                        if !self.root_private_history_provider_is_eligible(
                            RootPrivateHistoryProvider::Dictation,
                            query,
                        ) {
                            return None;
                        }
                        (
                            "dictation-history-snapshot",
                            crate::dictation::root_dictation_history_fresh_cache_status()?,
                        )
                    }
                    "conversations" => {
                        self.root_agent_chat_history_refresh_options_for_query(query)?;
                        ("conversation-history-snapshot", crate::ai::agent_chat::ui::history::root_agent_chat_history_fresh_cache_status()?)
                    }
                    "windows" => {
                        if !self.current_query_includes_root_source(
                            query,
                            crate::menu_syntax::RootUnifiedSourceFilter::Windows,
                        ) {
                            return None;
                        }
                        (
                            "window-snapshot",
                            self.root_search.root_windows_fresh_cache_status()?,
                        )
                    }
                    _ => unreachable!(),
                };
                Some(crate::root_search_store::RootSearchSourceCacheReadiness {
                    query: self.root_search.query_stamp(),
                    identity,
                    generation: Some(generation),
                    row_count,
                })
            }
            _ => None,
        }
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

    pub(crate) fn maybe_start_root_browser_tabs_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList)
            || !self.root_search.query_is_current()
            || query_text != self.computed_filter_text.as_str()
        {
            return;
        }
        let owned_gate = self.main_services.search_gate();
        if !self.main_services.is_production() && owned_gate.is_none() {
            return;
        }
        let source = sk_protocol::command_contract::CommandSource::BrowserTab;
        let Some((options, explicit_tabs)) =
            self.root_browser_tabs_refresh_options_for_query(query_text)
        else {
            self.root_search.invalidate_provider_request(source);
            return;
        };
        let providers = options.providers.clone();
        let reason = if explicit_tabs {
            "explicit_tabs_query"
        } else {
            "implicit_tabs_query"
        };
        self.root_search.note_desired_request(source, query_text);
        let refresh = crate::browser_tabs::try_begin_root_browser_tabs_refresh(
            options.cache_ttl_ms,
            providers.len(),
            reason,
        );
        if explicit_tabs {
            self.ensure_main_list_loading_animation(cx);
        }
        let Some(refresh) = refresh else {
            return;
        };
        let provider_request = self.root_search.begin_provider_request(source, query_text);
        let source_name = "tabs";
        let owned_run = if let Some(gate) = &owned_gate {
            let Some(run) = gate.begin(
                source_name,
                query_text,
                provider_request.generation,
                RootProviderPublicationPolicy::Visible,
            ) else {
                crate::browser_tabs::discard_root_browser_tabs_refresh(refresh);
                self.root_search
                    .finish_provider_request(&provider_request, RootProviderTerminal::Cancelled);
                return;
            };
            Some(run)
        } else {
            None
        };
        self.invalidate_root_passive_and_grouped_cache();
        cx.notify();

        let (tx, rx) = std::sync::mpsc::channel();
        let owned_sender = if owned_run.is_some() {
            Some(tx)
        } else {
            cx.background_executor()
                .spawn(async move {
                    let result = crate::browser_tabs::refresh_root_browser_tabs_snapshot(providers);
                    let _ = tx.send(result);
                })
                .detach();
            None
        };
        cx.spawn(async move |this, cx| {
            if let (Some(run), Some(tx)) = (&owned_run, owned_sender) {
                run.deliver(move |snapshot| tx.send(snapshot), owned_search::tab_result)
                    .await;
            }
            let received = receive_root_passive_provider(rx, cx.background_executor()).await;
            let terminal = match &received {
                Ok(Ok(snapshot)) => PassiveProviderTerminal::Completed {
                    count: snapshot.len(),
                },
                Ok(Err(error)) => PassiveProviderTerminal::for_error(error),
                Err(_) => PassiveProviderTerminal::Disconnected,
            };
            let updated = this.update(cx, |app, cx| {
                let released = if !matches!(app.current_view, AppView::ScriptList)
                    || !app
                        .root_search
                        .accepts_provider_request(&provider_request, &app.computed_filter_text)
                {
                    let released = crate::browser_tabs::discard_root_browser_tabs_refresh(refresh);
                    app.record_root_passive_provider_terminal(
                        &provider_request,
                        owned_run.as_ref(),
                        PassiveProviderTerminal::StaleDiscarded,
                    );
                    released
                } else {
                    let mut released = false;
                    app.commit_main_menu_results_refresh(
                        "browser_tabs_refresh_complete",
                        Some((source_name, provider_request.generation)),
                        cx,
                        |app, _cx| {
                            let source_changed = match received {
                                Ok(result) => {
                                    released = true;
                                    crate::browser_tabs::finish_root_browser_tabs_refresh(
                                        refresh, result,
                                    )
                                }
                                Err(_) => {
                                    released =
                                        crate::browser_tabs::discard_root_browser_tabs_refresh(
                                            refresh,
                                        );
                                    false
                                }
                            };
                            app.apply_root_passive_provider_completion(
                                &provider_request,
                                owned_run.as_ref(),
                                terminal,
                                source_changed,
                            )
                        },
                    );
                    released
                };
                if released {
                    if let Some(query_text) = app.root_search.take_desired_provider_query(source) {
                        app.maybe_start_root_browser_tabs_refresh_for_query(&query_text, cx);
                    }
                }
            });
            if updated.is_err() {
                crate::browser_tabs::discard_root_browser_tabs_refresh(refresh);
                if let Some(run) = &owned_run {
                    run.finish(
                        PassiveProviderTerminal::StaleDiscarded,
                        RootProviderPublicationPolicy::Visible,
                    );
                }
            }
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
        let rich_history =
            source_filters.allows(source) && active_rich_browser_history_subsearch(query_text);

        if explicit_history || rich_history {
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
        if !matches!(self.current_view, AppView::ScriptList)
            || !self.root_search.query_is_current()
            || query_text != self.computed_filter_text.as_str()
        {
            return;
        }
        let owned_gate = self.main_services.search_gate();
        if !self.main_services.is_production() && owned_gate.is_none() {
            return;
        }
        let source = sk_protocol::command_contract::CommandSource::BrowserHistory;
        let Some((options, explicit_history)) =
            self.root_browser_history_refresh_options_for_query(query_text)
        else {
            self.root_search.invalidate_provider_request(source);
            return;
        };
        let reason = if explicit_history {
            "explicit_history_query"
        } else {
            "implicit_history_query"
        };
        self.root_search.note_desired_request(source, query_text);
        let refresh =
            crate::browser_history::try_begin_root_browser_history_refresh(&options, reason);
        if explicit_history {
            self.ensure_main_list_loading_animation(cx);
        }
        let Some(refresh) = refresh else {
            return;
        };
        let provider_request = self.root_search.begin_provider_request(source, query_text);
        let source_name = "history";
        let owned_run = if let Some(gate) = &owned_gate {
            let Some(run) = gate.begin(
                source_name,
                query_text,
                provider_request.generation,
                RootProviderPublicationPolicy::Visible,
            ) else {
                crate::browser_history::discard_root_browser_history_refresh(refresh);
                self.root_search
                    .finish_provider_request(&provider_request, RootProviderTerminal::Cancelled);
                return;
            };
            Some(run)
        } else {
            None
        };
        self.invalidate_root_passive_and_grouped_cache();
        cx.notify();

        let (tx, rx) = std::sync::mpsc::channel();
        let owned_sender = if owned_run.is_some() {
            Some(tx)
        } else {
            std::thread::spawn(move || {
                let result = std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .ok_or_else(|| {
                        anyhow::Error::from(std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            "browser_history_home_unavailable",
                        ))
                    })
                    .and_then(|home| {
                        crate::browser_history::refresh_root_browser_history_snapshot_from_home(
                            &home, &options,
                        )
                    });
                let _ = tx.send(result);
            });
            None
        };
        cx.spawn(async move |this, cx| {
            if let (Some(run), Some(tx)) = (&owned_run, owned_sender) {
                run.deliver(
                    move |snapshot| tx.send(snapshot),
                    owned_search::history_result,
                ).await;
            }
            let received = receive_root_passive_provider(rx, cx.background_executor()).await;
            let terminal = match &received {
                Ok(Ok(snapshot)) => PassiveProviderTerminal::Completed { count: snapshot.len() },
                Ok(Err(error)) => PassiveProviderTerminal::for_error(error),
                Err(_) => PassiveProviderTerminal::Disconnected,
            };
            let updated = this.update(cx, |app, cx| {
                let released = if !matches!(app.current_view, AppView::ScriptList)
                    || !app.root_search.accepts_provider_request(&provider_request, &app.computed_filter_text)
                {
                    let released = crate::browser_history::discard_root_browser_history_refresh(refresh);
                    app.record_root_passive_provider_terminal(
                        &provider_request,
                        owned_run.as_ref(),
                        PassiveProviderTerminal::StaleDiscarded,
                    );
                    released
                } else {
                    let mut released = false;
                    app.commit_main_menu_results_refresh(
                        "browser_history_refresh_complete",
                        Some((source_name, provider_request.generation)),
                        cx,
                        |app, _cx| {
                            let source_changed = match received {
                                Ok(result) => {
                                    released = true;
                                    crate::browser_history::finish_root_browser_history_refresh(refresh, result)
                                }
                                Err(_) => {
                                    released = crate::browser_history::discard_root_browser_history_refresh(refresh);
                                    false
                                }
                            };
                            app.apply_root_passive_provider_completion(
                                &provider_request,
                                owned_run.as_ref(),
                                terminal,
                                source_changed,
                            )
                        },
                    );
                    released
                };
                if released {
                    if let Some(query_text) = app.root_search.take_desired_provider_query(source) {
                        app.maybe_start_root_browser_history_refresh_for_query(&query_text, cx);
                    }
                }
            });
            if updated.is_err() {
                crate::browser_history::discard_root_browser_history_refresh(refresh);
                if let Some(run) = &owned_run {
                    run.finish(PassiveProviderTerminal::StaleDiscarded, RootProviderPublicationPolicy::Visible);
                }
            }
        })
        .detach();
    }

    fn root_local_content_refresh_options_for_query(
        &self,
        provider: RootLocalContentProvider,
        query_text: &str,
    ) -> Option<(String, RootLocalContentOptions)> {
        let unified_search = self.config.get_unified_search();
        let advanced_query = self.menu_syntax_mode.advanced_query_for(query_text);
        let source_filters = advanced_query
            .map(|query| query.source_filters.clone())
            .unwrap_or_default();
        let source = provider.source_filter();
        if !source_filters.allows(source) {
            return None;
        }
        let explicit = source_filters.includes(source);
        if advanced_query.is_some_and(|query| query.has_predicates())
            && (!explicit || matches!(provider, RootLocalContentProvider::Notes))
        {
            return None;
        }
        if !explicit
            && (self.menu_syntax_object_selector_state.owns_main_list()
                || self.menu_syntax_trigger_picker_state.owns_main_list()
                || self
                    .menu_syntax_mode
                    .capture_composer_owns_input_for(query_text)
                || self.menu_syntax_mode.command_owns_input_for(query_text))
        {
            return None;
        }

        let search_text =
            crate::menu_syntax::free_text_for_search(&self.menu_syntax_mode, query_text);
        let needle = root_passive_search_needle(search_text).to_owned();
        match provider {
            RootLocalContentProvider::Notes => {
                let mut options = unified_search.notes_section_options();
                if explicit {
                    options.enabled = true;
                    options.min_query_chars = 0;
                    options.max_results = options
                        .max_results
                        .max(unified_search.passive_result_limits().max_total_results);
                }
                crate::notes::root_notes_query_is_eligible(&needle, options)
                    .then_some((needle, RootLocalContentOptions::Notes(options)))
            }
            RootLocalContentProvider::Todos => {
                let mut options = unified_search.todo_section_options();
                if explicit {
                    options.enabled = true;
                    options.min_query_chars = 0;
                    options.max_results = options
                        .max_results
                        .max(unified_search.passive_result_limits().max_total_results);
                }
                crate::menu_syntax::root_todo_query_is_eligible(&needle, options)
                    .then_some((needle, RootLocalContentOptions::Todos(options)))
            }
        }
    }

    fn maybe_start_root_local_content_refresh_for_query(
        &mut self,
        provider: RootLocalContentProvider,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList)
            || !self.root_search.query_is_current()
            || query_text != self.computed_filter_text.as_str()
        {
            return;
        }
        let owned_gate = self.main_services.search_gate();
        if !self.main_services.is_production() && owned_gate.is_none() {
            return;
        }
        let source = provider.source();
        let Some((needle, options)) =
            self.root_local_content_refresh_options_for_query(provider, query_text)
        else {
            self.root_search.invalidate_provider_request(source);
            return;
        };
        if provider.cache_is_fresh(&needle, options) {
            return;
        }
        self.root_search.note_desired_request(source, query_text);
        let Some(refresh) = provider.begin(&needle, options) else {
            return;
        };
        let provider_request = self.root_search.begin_provider_request(source, query_text);
        let source_name = match provider {
            RootLocalContentProvider::Notes => "notes",
            RootLocalContentProvider::Todos => "todos",
        };
        let owned_run = if let Some(gate) = &owned_gate {
            let Some(run) = gate.begin(
                source_name,
                query_text,
                provider_request.generation,
                RootProviderPublicationPolicy::Visible,
            ) else {
                provider.discard(&refresh);
                self.root_search
                    .finish_provider_request(&provider_request, RootProviderTerminal::Cancelled);
                return;
            };
            Some(run)
        } else {
            None
        };
        self.invalidate_root_passive_and_grouped_cache();
        cx.notify();

        let (tx, rx) = std::sync::mpsc::channel();
        let owned_sender = if owned_run.is_some() {
            Some(tx)
        } else {
            let worker_refresh = refresh.clone();
            cx.background_executor()
                .spawn(async move {
                    let snapshot = provider.read_snapshot(&worker_refresh).ok_or_else(|| {
                        anyhow::Error::from(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "root_local_content_snapshot_owner_mismatch",
                        ))
                    });
                    let _ = tx.send(snapshot);
                })
                .detach();
            None
        };
        cx.spawn(async move |this, cx| {
            if let (Some(run), Some(tx)) = (&owned_run, owned_sender) {
                run.deliver(
                    move |snapshot| tx.send(snapshot),
                    |outcome, run| owned_search::local_snapshot(&refresh, outcome, run),
                )
                .await;
            }
            let received = receive_root_passive_provider(rx, cx.background_executor()).await;
            let terminal = match &received {
                Ok(Ok(snapshot)) => match snapshot {
                    crate::scripts::root_search_contract::RootLocalContentSnapshot::Notes(
                        snapshot,
                    ) => PassiveProviderTerminal::for_read_outcome(snapshot.read_outcome()),
                    crate::scripts::root_search_contract::RootLocalContentSnapshot::Todos(
                        snapshot,
                    ) => PassiveProviderTerminal::for_read_outcome(snapshot.read_outcome()),
                },
                Ok(Err(error)) => PassiveProviderTerminal::for_error(error),
                Err(_) => PassiveProviderTerminal::Disconnected,
            };
            let updated = this.update(cx, |app, cx| {
                let released = if !matches!(app.current_view, AppView::ScriptList)
                    || !app
                        .root_search
                        .accepts_provider_request(&provider_request, &app.computed_filter_text)
                {
                    let released = provider.discard(&refresh);
                    app.record_root_passive_provider_terminal(
                        &provider_request,
                        owned_run.as_ref(),
                        PassiveProviderTerminal::StaleDiscarded,
                    );
                    released
                } else {
                    let mut released = false;
                    app.commit_main_menu_results_refresh(
                        provider.completion_reason(),
                        Some((source_name, provider_request.generation)),
                        cx,
                        |app, _cx| {
                            let source_changed = match received {
                                Ok(Ok(snapshot)) => {
                                    released = true;
                                    provider.finish(refresh.clone(), snapshot)
                                }
                                Ok(Err(_)) | Err(_) => {
                                    released = provider.discard(&refresh);
                                    false
                                }
                            };
                            app.apply_root_passive_provider_completion(
                                &provider_request,
                                owned_run.as_ref(),
                                terminal,
                                source_changed,
                            )
                        },
                    );
                    released
                };
                if released {
                    if let Some(query_text) = app.root_search.take_desired_provider_query(source) {
                        app.maybe_start_root_local_content_refresh_for_query(
                            provider,
                            &query_text,
                            cx,
                        );
                    }
                }
            });
            if updated.is_err() {
                provider.discard(&refresh);
                if let Some(run) = &owned_run {
                    run.finish(
                        PassiveProviderTerminal::StaleDiscarded,
                        RootProviderPublicationPolicy::Visible,
                    );
                }
            }
        })
        .detach();
    }

    pub(crate) fn maybe_start_root_notes_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        self.maybe_start_root_local_content_refresh_for_query(
            RootLocalContentProvider::Notes,
            query_text,
            cx,
        );
    }

    pub(crate) fn maybe_start_root_todos_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        self.maybe_start_root_local_content_refresh_for_query(
            RootLocalContentProvider::Todos,
            query_text,
            cx,
        );
    }

    fn root_private_history_provider_is_eligible(
        &self,
        provider: RootPrivateHistoryProvider,
        query_text: &str,
    ) -> bool {
        let unified_search = self.config.get_unified_search();
        let advanced_query = self.menu_syntax_mode.advanced_query_for(query_text);
        let source_filters = advanced_query
            .map(|query| query.source_filters.clone())
            .unwrap_or_default();
        let source = provider.source_filter();
        if !source_filters.allows(source) {
            return false;
        }
        let explicit = source_filters.includes(source);
        if !explicit
            && (advanced_query.is_some_and(|query| query.has_predicates())
                || self.menu_syntax_object_selector_state.owns_main_list()
                || self.menu_syntax_trigger_picker_state.owns_main_list()
                || self
                    .menu_syntax_mode
                    .capture_composer_owns_input_for(query_text)
                || self.menu_syntax_mode.command_owns_input_for(query_text))
        {
            return false;
        }
        let search_text =
            crate::menu_syntax::free_text_for_search(&self.menu_syntax_mode, query_text);
        let needle = root_passive_search_needle(search_text);

        match provider {
            RootPrivateHistoryProvider::Clipboard => {
                let mut options = self.config.root_clipboard_history_section_options();
                if explicit {
                    options.enabled = true;
                    options.min_query_chars = 0;
                    options.max_results = options
                        .max_results
                        .max(unified_search.passive_result_limits().max_total_results);
                }
                crate::clipboard_history::root_clipboard_history_query_is_eligible(needle, options)
            }
            RootPrivateHistoryProvider::Dictation => {
                let mut options = unified_search.dictation_history_section_options();
                if explicit {
                    options.enabled = true;
                    options.min_query_chars = 0;
                    options.max_results = options
                        .max_results
                        .max(unified_search.passive_result_limits().max_total_results);
                }
                crate::dictation::root_dictation_history_query_is_eligible(needle, options)
            }
        }
    }

    fn maybe_start_root_private_history_refresh_for_query(
        &mut self,
        provider: RootPrivateHistoryProvider,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList)
            || !self.root_search.query_is_current()
            || query_text != self.computed_filter_text.as_str()
        {
            return;
        }
        let owned_gate = self.main_services.search_gate();
        if !self.main_services.is_production() && owned_gate.is_none() {
            return;
        }
        let source = provider.source();
        if !self.root_private_history_provider_is_eligible(provider, query_text) {
            self.root_search.invalidate_provider_request(source);
            return;
        }
        if provider.cache_is_fresh() {
            return;
        }
        self.root_search.note_desired_request(source, query_text);
        let Some(refresh) = provider.begin() else {
            return;
        };
        let provider_request = self.root_search.begin_provider_request(source, query_text);
        let source_name = match provider {
            RootPrivateHistoryProvider::Clipboard => "clipboard",
            RootPrivateHistoryProvider::Dictation => "dictation",
        };
        let owned_run = if let Some(gate) = &owned_gate {
            let Some(run) = gate.begin(
                source_name,
                query_text,
                provider_request.generation,
                RootProviderPublicationPolicy::Visible,
            ) else {
                provider.discard(refresh);
                self.root_search
                    .finish_provider_request(&provider_request, RootProviderTerminal::Cancelled);
                return;
            };
            Some(run)
        } else {
            None
        };
        self.invalidate_root_passive_and_grouped_cache();
        cx.notify();

        let (tx, rx) = std::sync::mpsc::channel();
        let owned_sender = if owned_run.is_some() {
            Some(tx)
        } else {
            cx.background_executor()
                .spawn(async move {
                    let _ = tx.send(Ok(provider.read_snapshot()));
                })
                .detach();
            None
        };
        cx.spawn(async move |this, cx| {
            if let (Some(run), Some(tx)) = (&owned_run, owned_sender) {
                run.deliver(
                    move |snapshot| tx.send(snapshot),
                    |outcome, run| owned_search::private_snapshot(provider, outcome, run),
                )
                .await;
            }
            let received = receive_root_passive_provider(rx, cx.background_executor()).await;
            let terminal = match &received {
                Ok(Ok(snapshot)) => match snapshot {
                    crate::scripts::root_search_contract::RootPrivateHistorySnapshot::Clipboard(
                        snapshot,
                    ) => PassiveProviderTerminal::for_read_outcome(snapshot.read_outcome()),
                    crate::scripts::root_search_contract::RootPrivateHistorySnapshot::Dictation(
                        snapshot,
                    ) => PassiveProviderTerminal::for_read_outcome(snapshot.read_outcome()),
                },
                Ok(Err(error)) => PassiveProviderTerminal::for_error(error),
                Err(_) => PassiveProviderTerminal::Disconnected,
            };
            let updated = this.update(cx, |app, cx| {
                let released = if !matches!(app.current_view, AppView::ScriptList)
                    || !app
                        .root_search
                        .accepts_provider_request(&provider_request, &app.computed_filter_text)
                {
                    let released = provider.discard(refresh);
                    app.record_root_passive_provider_terminal(
                        &provider_request,
                        owned_run.as_ref(),
                        PassiveProviderTerminal::StaleDiscarded,
                    );
                    released
                } else {
                    let mut released = false;
                    app.commit_main_menu_results_refresh(
                        provider.completion_reason(),
                        Some((source_name, provider_request.generation)),
                        cx,
                        |app, _cx| {
                            let source_changed = match received {
                                Ok(Ok(snapshot)) => {
                                    released = true;
                                    provider.finish(refresh, snapshot)
                                }
                                Ok(Err(_)) | Err(_) => {
                                    released = provider.discard(refresh);
                                    false
                                }
                            };
                            app.apply_root_passive_provider_completion(
                                &provider_request,
                                owned_run.as_ref(),
                                terminal,
                                source_changed,
                            )
                        },
                    );
                    released
                };
                if released {
                    if let Some(query_text) = app.root_search.take_desired_provider_query(source) {
                        app.maybe_start_root_private_history_refresh_for_query(
                            provider,
                            &query_text,
                            cx,
                        );
                    }
                }
            });
            if updated.is_err() {
                provider.discard(refresh);
                if let Some(run) = &owned_run {
                    run.finish(
                        PassiveProviderTerminal::StaleDiscarded,
                        RootProviderPublicationPolicy::Visible,
                    );
                }
            }
        })
        .detach();
    }

    pub(crate) fn maybe_start_root_clipboard_history_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        self.maybe_start_root_private_history_refresh_for_query(
            RootPrivateHistoryProvider::Clipboard,
            query_text,
            cx,
        );
    }

    pub(crate) fn maybe_start_root_dictation_history_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        self.maybe_start_root_private_history_refresh_for_query(
            RootPrivateHistoryProvider::Dictation,
            query_text,
            cx,
        );
    }

    fn root_agent_chat_history_refresh_options_for_query(
        &self,
        query_text: &str,
    ) -> Option<crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions> {
        let source = crate::menu_syntax::RootUnifiedSourceFilter::Conversations;
        let unified_search = self.config.get_unified_search();
        let mut options = unified_search.agent_chat_history_section_options();
        let advanced_query = self.menu_syntax_mode.advanced_query_for(query_text);
        let source_filters = advanced_query
            .map(|query| query.source_filters.clone())
            .unwrap_or_default();

        if !source_filters.allows(source) {
            return None;
        }
        if source_filters.includes(source) {
            options.enabled = true;
            options.min_query_chars = 0;
            options.max_results = options
                .max_results
                .max(unified_search.passive_result_limits().max_total_results);
            return Some(options);
        }
        if advanced_query.is_some_and(|query| query.has_predicates())
            || self.menu_syntax_object_selector_state.owns_main_list()
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
        crate::ai::agent_chat::ui::history::root_agent_chat_history_query_is_eligible(
            root_passive_search_needle(search_text),
            options,
        )
        .then_some(options)
    }

    pub(crate) fn maybe_start_root_agent_chat_history_refresh_for_query(
        &mut self,
        query_text: &str,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList)
            || !self.root_search.query_is_current()
            || query_text != self.computed_filter_text.as_str()
        {
            return;
        }
        let owned_gate = self.main_services.search_gate();
        if !self.main_services.is_production() && owned_gate.is_none() {
            return;
        }
        let source = sk_protocol::command_contract::CommandSource::Conversation;
        if self
            .root_agent_chat_history_refresh_options_for_query(query_text)
            .is_none()
        {
            self.root_search.invalidate_provider_request(source);
            return;
        }
        if crate::ai::agent_chat::ui::history::root_agent_chat_history_cache_is_fresh() {
            return;
        }
        self.root_search.note_desired_request(source, query_text);
        let Some(refresh) =
            crate::ai::agent_chat::ui::history::try_begin_root_agent_chat_history_refresh()
        else {
            return;
        };
        let provider_request = self.root_search.begin_provider_request(source, query_text);
        let source_name = "conversations";
        let owned_run = if let Some(gate) = &owned_gate {
            let Some(run) = gate.begin(
                source_name,
                query_text,
                provider_request.generation,
                RootProviderPublicationPolicy::Visible,
            ) else {
                crate::ai::agent_chat::ui::history::discard_root_agent_chat_history_refresh(
                    refresh,
                );
                self.root_search
                    .finish_provider_request(&provider_request, RootProviderTerminal::Cancelled);
                return;
            };
            Some(run)
        } else {
            None
        };
        self.invalidate_root_passive_and_grouped_cache();
        cx.notify();

        let (tx, rx) = std::sync::mpsc::channel();
        let owned_sender = if owned_run.is_some() {
            Some(tx)
        } else {
            cx.background_executor()
                .spawn(async move {
                    let _ = tx.send(Ok(
                        crate::ai::agent_chat::ui::history::read_root_agent_chat_history_snapshot(),
                    ));
                })
                .detach();
            None
        };
        cx.spawn(async move |this, cx| {
            if let (Some(run), Some(tx)) = (&owned_run, owned_sender) {
                run.deliver(
                    move |snapshot| tx.send(snapshot),
                    owned_search::conversation_snapshot,
                ).await;
            }
            let received = receive_root_passive_provider(rx, cx.background_executor()).await;
            let terminal = match &received {
                Ok(Ok(snapshot)) => PassiveProviderTerminal::for_read_outcome(snapshot.read_outcome()),
                Ok(Err(error)) => PassiveProviderTerminal::for_error(error),
                Err(_) => PassiveProviderTerminal::Disconnected,
            };
            let updated = this.update(cx, |app, cx| {
                let released = if !matches!(app.current_view, AppView::ScriptList)
                    || !app.root_search.accepts_provider_request(&provider_request, &app.computed_filter_text)
                {
                    let released = crate::ai::agent_chat::ui::history::discard_root_agent_chat_history_refresh(refresh);
                    app.record_root_passive_provider_terminal(
                        &provider_request,
                        owned_run.as_ref(),
                        PassiveProviderTerminal::StaleDiscarded,
                    );
                    released
                } else {
                    let mut released = false;
                    app.commit_main_menu_results_refresh(
                        "agent_chat_history_refresh_complete",
                        Some((source_name, provider_request.generation)),
                        cx,
                        |app, _cx| {
                            let source_changed = match received {
                                Ok(Ok(snapshot)) => {
                                    released = true;
                                    crate::ai::agent_chat::ui::history::finish_root_agent_chat_history_refresh(refresh, snapshot)
                                }
                                Ok(Err(_)) | Err(_) => {
                                    released = crate::ai::agent_chat::ui::history::discard_root_agent_chat_history_refresh(refresh);
                                    false
                                }
                            };
                            app.apply_root_passive_provider_completion(
                                &provider_request,
                                owned_run.as_ref(),
                                terminal,
                                source_changed,
                            )
                        },
                    );
                    released
                };
                if released {
                    if let Some(query_text) = app.root_search.take_desired_provider_query(source) {
                        app.maybe_start_root_agent_chat_history_refresh_for_query(&query_text, cx);
                    }
                }
            });
            if updated.is_err() {
                crate::ai::agent_chat::ui::history::discard_root_agent_chat_history_refresh(refresh);
                if let Some(run) = &owned_run {
                    run.finish(PassiveProviderTerminal::StaleDiscarded, RootProviderPublicationPolicy::Visible);
                }
            }
        })
        .detach();
    }

    fn root_passive_frame_for_current_query(
        &mut self,
        search_text: &str,
        options: RootPassiveFrameOptions,
    ) -> crate::RootPassiveFrame {
        let RootPassiveFrameOptions {
            advanced_query_active,
            source_filters,
            todo_options,
            brain_options,
            notes_options,
            clipboard_history_options,
            dictation_history_options,
            agent_chat_history_options,
            ai_vault_options,
            browser_tabs_options,
            browser_history_options,
        } = options;
        let ai_vault_status = crate::ai_vault::root_ai_vault_snapshot_status();
        let browser_tabs_status = crate::browser_tabs::root_browser_tabs_snapshot_status();
        let browser_history_status = crate::browser_history::root_browser_history_snapshot_status();
        let key = crate::RootPassiveFrameKey {
            query: search_text.to_string(),
            advanced_query: advanced_query_active,
            source_filters: source_filters.clone(),
            todo_options,
            brain_options,
            brain_source_epoch: self.root_search.root_brain_source_epoch(),
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

        // Eligibility is shared with the source-owner reads. Grouping consumes
        // only accepted snapshots, including recents for an armed bare `brain:`.
        let brain_plan = crate::brain::root_brain_query_plan(
            search_needle,
            explicit_brain,
            advanced_query_active,
            allow_brain,
            brain_options,
        );
        // Preserve semantic preference for the exact current query; lexical is
        // the committed first-paint snapshot while the semantic worker is pending.
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
                || match brain_plan {
                    crate::brain::RootBrainQueryPlan::Skip => Vec::new(),
                    _ => brain_semantic_hits
                        .unwrap_or_else(|| self.root_search.root_brain_lexical_results().to_vec()),
                },
            );

        let note_hits = timed_root_passive_source("notes", search_needle, explicit_notes, || {
            if !advanced_query_active
                && allow_notes
                && crate::notes::root_notes_query_is_eligible(search_needle, notes_options)
            {
                crate::notes::search_root_notes_meta_cached(search_needle, notes_options)
            } else {
                Vec::new()
            }
        });

        let todo_hits = timed_root_passive_source("todo", search_needle, explicit_todos, || {
            if (!advanced_query_active || explicit_todos)
                && allow_todos
                && crate::menu_syntax::root_todo_query_is_eligible(search_needle, todo_options)
            {
                crate::menu_syntax::search_root_todos_cached(search_needle, todo_options)
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
                    crate::clipboard_history::search_root_clipboard_history_meta_cached(
                        search_needle,
                        clipboard_history_options,
                    )
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
                    crate::dictation::search_root_dictation_history_cached(
                        search_needle,
                        dictation_history_options,
                    )
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
                    crate::ai::agent_chat::ui::history::search_history_cached(
                        search_needle,
                        agent_chat_history_options.max_results,
                    )
                } else {
                    Vec::new()
                }
            },
        );

        let ai_vault_hits =
            timed_root_passive_source("ai_vault", search_needle, explicit_ai_vault, || {
                if self.main_services.is_production()
                    && explicit_ai_vault
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
                    if self.main_services.is_production() && explicit_browser_tabs {
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
                    if self.main_services.is_production() && explicit_browser_history {
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
        let safe_filter = logging::log_private_user_value(filter_text);
        let safe_search = logging::log_private_user_value(search_text);

        if !filter_text.is_empty() {
            logging::log(
                "PERF",
                &format!(
                    "Search {} ({} bytes; computed {} / {} bytes) took {:.2}ms ({} results from {} total, including {} skills)",
                    safe_filter,
                    safe_filter.raw_bytes,
                    safe_search,
                    safe_search.raw_bytes,
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
            filter_bytes = safe_filter.raw_bytes,
            filter_sha256 = %safe_filter.sha256,
            search_bytes = safe_search.raw_bytes,
            search_sha256 = %safe_search.sha256,
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
            logging::log_debug(
                "CACHE",
                &format!(
                    "Filter cache HIT for {} ({} bytes)",
                    logging::log_private_user_value(filter_text),
                    filter_text.len(),
                ),
            );
            return self.main_menu_result_caches.clone_filtered_results();
        }

        // P1: Cache miss - need to recompute (will be done by get_filtered_results_mut)
        let cached_key = self.main_menu_result_caches.filtered_cache_key();
        logging::log_debug(
            "CACHE",
            &format!(
                "Filter cache MISS - need recompute for {} / {} bytes (cached key: {} / {} bytes)",
                logging::log_private_user_value(filter_text),
                filter_text.len(),
                logging::log_private_user_value(cached_key),
                cached_key.len(),
            ),
        );

        self.recompute_filtered_results(filter_text)
    }

    /// Flow rosters land asynchronously (background `md roster` fetch); the
    /// main-menu result caches poll the catalog generation so a roster that
    /// arrives after a render still surfaces on the next cache read, without
    /// a background-thread cx handle.
    fn sync_flow_roster_cache_generation(&mut self) {
        if !self.main_services.is_production() {
            return;
        }
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
                        "[4a/5] SEARCH_START for {} ({} bytes; scripts={}, scriptlets={}, builtins={}, apps={}, skills={})",
                        logging::log_private_user_value(&filter_text),
                        filter_text.len(),
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
                        "[4a/5] SEARCH_DONE {} ({} bytes) in {:.2}ms -> {} results (skills={})",
                        logging::log_private_user_value(&filter_text),
                        filter_text.len(),
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

    /// Read the last committed rows. Observation never adopts source generations.
    pub(crate) fn get_grouped_results_cached(
        &self,
    ) -> (Arc<[GroupedListItem]>, Arc<[scripts::SearchResult]>) {
        self.main_menu_result_caches.clone_grouped_results()
    }

    /// Build through the production grouping path, publishing only a valid projection.
    pub(crate) fn rebuild_main_menu_results_cache(
        &mut self,
        _cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        if !self.root_search.query_is_current() {
            return Err("main_menu_query_pending".to_string());
        }
        let query_stamp = self
            .root_search
            .computed_query_stamp()
            .ok_or_else(|| "main_menu_query_uninitialized".to_string())?;
        if self.main_menu_committed_query_stamp() != Some(query_stamp) {
            self.main_menu_result_caches.invalidate_grouped_results();
        }
        self.sync_flow_roster_cache_generation();
        // One cheap handle keeps text and parser ownership on the same committed
        // query while collectors mutate their source-owned caches below.
        let computed_inputs = self.root_search.computed_query_inputs();
        let computed_filter_text = computed_inputs.raw.as_str();
        let spine_owns_for_computed = self.spine_projection_owns_main_list_for(
            &computed_inputs.spine_parse,
            computed_inputs.spine_projection.as_ref(),
        ) && computed_inputs.spine_parse.input
            == computed_filter_text;
        let popup_owns_computed_main_list = self.menu_syntax_object_selector_state.owns_main_list()
            || self.menu_syntax_trigger_picker_state.owns_main_list();
        let active_filter_head_owns_computed_main_list =
            crate::menu_syntax::active_filter_head_owns_main_list(computed_filter_text);

        // ── Spine projection path ──────────────────────────────────────
        // When a sigil segment owns the list, build rows from the Spine
        // model instead of running normal fuzzy/root grouping.
        if !popup_owns_computed_main_list
            && !active_filter_head_owns_computed_main_list
            && spine_owns_for_computed
        {
            if let Some(projection) = computed_inputs.spine_projection.as_ref() {
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
                        computed_filter_text,
                        computed_filter_text,
                        &computed_inputs.spine_parse,
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
                        return Ok(false);
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
                        if let Some(segment) = computed_inputs
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

                    self.main_menu_result_caches.store_grouped_results(
                        rich_cache_key,
                        grouped_items,
                        flat_results,
                        None,
                        query_stamp,
                        computed_filter_text,
                        Default::default(),
                    )?;
                    return Ok(true);
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
                            return Ok(false);
                        }
                        let (grouped_items, flat_results) = if has_query {
                            build_rich_cwd_subsearch_rows(
                                sub_query.as_deref().unwrap_or(""),
                                &recent_dirs,
                            )
                        } else {
                            build_rich_cwd_root_rows(&recent_dirs)
                        };
                        self.main_menu_result_caches.store_grouped_results(
                            cwd_cache_key,
                            grouped_items,
                            flat_results,
                            None,
                            query_stamp,
                            computed_filter_text,
                            Default::default(),
                        )?;
                        return Ok(true);
                    }
                }

                if self
                    .main_menu_result_caches
                    .has_grouped_results_for(&spine_cache_key)
                {
                    return Ok(false);
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
                        &computed_inputs.spine_parse,
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

                self.main_menu_result_caches.store_grouped_results(
                    spine_cache_key,
                    grouped_items,
                    flat_results,
                    None,
                    query_stamp,
                    computed_filter_text,
                    Default::default(),
                )?;
                return Ok(true);
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

        let grouped_advanced_query = computed_inputs
            .menu_syntax
            .advanced_query_for(computed_filter_text);
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
                computed_filter_text
            ),
            None => format!(
                "{}\x1Fsource-filters={grouped_source_filter_key}\x1Fai-vault-gen={ai_vault_generation}\x1Fwindows-gen={root_windows_generation}\x1Fbrowser-tabs-gen={browser_tabs_generation}\x1Fbrowser-history-gen={browser_history_generation}\x1Fbrain-inbox-epoch={brain_inbox_epoch}",
                computed_filter_text
            ),
        };

        // P3: Key off computed_filter_text for two-stage filtering
        if self
            .main_menu_result_caches
            .has_grouped_results_for(&grouped_cache_key)
        {
            // NOTE: Removed cache HIT log - fires every render frame, causing log spam.
            // Cache hits are normal operation. Only log cache MISS (below) for diagnostics.
            return Ok(false);
        }

        let should_refresh_root_recent_files = computed_filter_text.is_empty()
            || matches!(
                self.root_search.root_file_search_mode,
                Some(crate::file_search::RootFileSectionMode::GlobalQuery)
            )
            || computed_inputs
                .menu_syntax
                .advanced_query_for(computed_filter_text)
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
                &format!(
                    "[4b/5] GROUP_START for {} ({} bytes)",
                    logging::log_private_user_value(computed_filter_text),
                    computed_filter_text.len(),
                ),
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
                    "get_grouped_results: filter={} ({} bytes), menu_bar_items={}, bundle_id={:?}",
                    logging::log_private_user_value(computed_filter_text),
                    computed_filter_text.len(),
                    menu_bar_items.len(),
                    menu_bar_bundle_id
                ),
            );
        }
        let raw_filter_text = computed_filter_text;
        let menu_syntax_owns_main_list = popup_owns_computed_main_list
            || (!spine_owns_for_computed
                && (computed_inputs
                    .menu_syntax
                    .capture_composer_owns_input_for(raw_filter_text)
                    || computed_inputs
                        .menu_syntax
                        .command_owns_input_for(raw_filter_text)
                    || active_filter_head_owns_computed_main_list));

        let mut ranking_evidence = scripts::command_contract::MainMenuRankingEvidenceMap::new();
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
        } else if let Some(invocation) = computed_inputs.menu_syntax.capture_for(raw_filter_text) {
            // Capture mode replaces the normal launcher grouping entirely.
            // Do not mix with Suggested/Favorites/Recent/menu-bar/fallback.
            crate::scripts::build_capture_mode_results(&self.scripts, invocation)
        } else if let Some(hint) = computed_inputs
            .menu_syntax
            .incomplete_hint_for(raw_filter_text)
        {
            // Menu-syntax trigger picker rows are now owned by the main ScriptList surface.
            // The main launcher result list stays suppressed while trigger rows render
            // through menu_syntax_main_hint_snapshot.
            crate::scripts::build_menu_syntax_hint_results(hint)
        } else {
            let search_text = crate::menu_syntax::free_text_for_search(
                &computed_inputs.menu_syntax,
                raw_filter_text,
            );
            let advanced_query = computed_inputs
                .menu_syntax
                .advanced_query_for(raw_filter_text);
            let source_filters = advanced_query
                .as_ref()
                .map(|query| query.source_filters.clone())
                .unwrap_or_default();
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
                    raw_filter_text,
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
                RootPassiveFrameOptions {
                    advanced_query_active: advanced_predicate_active,
                    source_filters: source_filters.clone(),
                    todo_options,
                    brain_options,
                    notes_options,
                    clipboard_history_options,
                    dictation_history_options,
                    agent_chat_history_options,
                    ai_vault_options: ai_vault_options.clone(),
                    browser_tabs_options: browser_tabs_options.clone(),
                    browser_history_options: browser_history_options.clone(),
                },
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
                Some(&mut ranking_evidence),
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
                            &mut ranking_evidence,
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
                    raw_filter_text,
                    self.root_search.root_brain_inbox_items(),
                    self.config
                        .get_unified_search()
                        .brain_inbox_section_options(),
                    chrono::Utc::now().timestamp(),
                    Some(&mut ranking_evidence),
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
                    raw_filter_text,
                    &records,
                    &flows,
                    chrono::Utc::now().timestamp(),
                    Some(&mut ranking_evidence),
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

        self.main_menu_result_caches.store_grouped_results(
            grouped_cache_key,
            grouped_items,
            flat_results,
            self.inline_calculator.clone(),
            query_stamp,
            computed_filter_text,
            ranking_evidence,
        )?;

        if logging::filter_perf_trace_enabled() || elapsed >= std::time::Duration::from_millis(8) {
            logging::log(
                "FILTER_PERF",
                &format!(
                    "[4b/5] GROUP_DONE {} ({} bytes) in {:.2}ms -> {} items (from {} results)",
                    logging::log_private_user_value(computed_filter_text),
                    computed_filter_text.len(),
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
                        "[5/5] TOTAL_TIME {} ({} bytes): {:.2}ms (input->grouped)",
                        logging::log_private_user_value(computed_filter_text),
                        computed_filter_text.len(),
                        total_elapsed.as_secs_f64() * 1000.0
                    ),
                );
            }
        }

        Ok(true)
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
        // Retain the committed arrays until an explicit publication rebuild succeeds.
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
        match self.resolved_main_menu_selected_subject()? {
            ResolvedMainMenuSelection::SearchResult { result, .. } => Some(result.clone()),
            ResolvedMainMenuSelection::Calculator { .. } => None,
        }
    }

    pub(crate) fn sync_main_menu_preview_binding(&mut self) {
        let current = self
            .resolved_main_menu_selected_subject()
            .map(|subject| match subject {
                ResolvedMainMenuSelection::SearchResult { row, .. }
                | ResolvedMainMenuSelection::Calculator { row, .. } => row,
            });
        let query = self.root_search.query_stamp();
        let unchanged = match (current, self.main_menu_preview_binding()) {
            (Some(row), Some(binding)) => {
                binding.query == query
                    && binding.stable_key == row.stable_key
                    && binding.content_fingerprint == row.content_fingerprint
            }
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return;
        }
        let binding = current.map(|row| MainMenuPreviewBinding {
            query,
            stable_key: row.stable_key.clone(),
            content_fingerprint: row.content_fingerprint.clone(),
        });
        self.invalidate_preview_cache();
        self.scriptlet_preview_cache_key = None;
        self.scriptlet_preview_cache_lines.clear();
        self.set_main_menu_preview_binding(binding);
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
        self.sync_main_menu_preview_binding();
        let current_path = match self.resolved_main_menu_selected_subject() {
            Some(ResolvedMainMenuSelection::SearchResult {
                result: scripts::SearchResult::Script(script),
                ..
            }) => script.script.path.as_path(),
            _ => return &[],
        };
        if current_path != std::path::Path::new(script_path) {
            return &[];
        }
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
        let safe_script_path = logging::log_private_user_value(script_path);
        let safe_cached_path = self
            .preview_cache_path
            .as_deref()
            .map(logging::log_private_user_value);
        logging::log(
            "FILTER_PERF",
            &format!(
                "[PREVIEW_CACHE_MISS_REASON] path={} path_bytes={} reason={} cached_path={:?} cached_match_signature={:?} requested_match_signature={:?}",
                safe_script_path,
                safe_script_path.raw_bytes,
                miss_reason,
                safe_cached_path,
                self.preview_cache_match_signature,
                match_signature
            ),
        );
        logging::log(
            "FILTER_PERF",
            &format!(
                "[PREVIEW_CACHE_KEY] path={} path_bytes={} match_signature={:?}",
                safe_script_path, safe_script_path.raw_bytes, match_signature
            ),
        );
        logging::log(
            "FILTER_PERF",
            &format!(
                "[PREVIEW_CACHE_MISS] Loading {} ({} bytes) matched_line={:?}",
                safe_script_path, safe_script_path.raw_bytes, matched_line
            ),
        );

        self.preview_cache_path = Some(script_path.to_string());
        self.preview_cache_match_signature = match_signature;

        let read_start = std::time::Instant::now();
        self.preview_cache_lines = match std::fs::read_to_string(script_path) {
            Ok(content) => {
                self.set_main_menu_preview_load_state("ready");
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
                self.set_main_menu_preview_load_state("failed");
                let safe_error = logging::log_private_user_value(&e.to_string());
                logging::log(
                    "ERROR",
                    &format!(
                        "Failed to read preview: {} ({} bytes)",
                        safe_error, safe_error.raw_bytes
                    ),
                );
                Vec::new()
            }
        };

        let cache_miss_elapsed = cache_miss_start.elapsed();
        logging::log(
            "FILTER_PERF",
            &format!(
                "[PREVIEW_CACHE_MISS] Total={:.2}ms for {} ({} bytes)",
                cache_miss_elapsed.as_secs_f64() * 1000.0,
                safe_script_path,
                safe_script_path.raw_bytes,
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
        self.set_main_menu_preview_binding(None);
    }

    /// Builds the matcher + synthetic-result fallback for the resolved alias
    /// target and pins it at grouped index 0 (A1: "alias means index 0").
    fn pin_alias_match_into_grouped_results(
        &self,
        alias_match: &AliasMatch,
        grouped_items: &mut Vec<crate::list_item::GroupedListItem>,
        flat_results: &mut Vec<crate::scripts::SearchResult>,
        ranking_evidence: &mut scripts::command_contract::MainMenuRankingEvidenceMap,
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
            Some(ranking_evidence),
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
        let (ghost_context, context_rev) = match self.main_services.owned_sources() {
            Some(sources) => (sources.ghost_context.clone(), 0),
            None => cwd
                .as_deref()
                .map(|cwd| self.ghost_context_cache.context_for_cwd(cwd))
                .unwrap_or_else(|| (crate::scripts::search::ghost::GhostContext::default(), 0)),
        };
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
        let safe_query = logging::log_private_user_value(&pred.query);
        let safe_suffix = logging::log_private_user_value(&pred.ghost_suffix);
        let safe_label = logging::log_private_user_value(&pred.full_label);
        tracing::info!(
            target: "script_kit::ghost_text",
            query_bytes = safe_query.raw_bytes,
            query_sha256 = %safe_query.sha256,
            ghost_suffix_bytes = safe_suffix.raw_bytes,
            ghost_suffix_sha256 = %safe_suffix.sha256,
            full_label_bytes = safe_label.raw_bytes,
            full_label_sha256 = %safe_label.sha256,
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
        if !self.main_services.is_production() {
            return None;
        }
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
        if !self.main_services.is_production() {
            return;
        }
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
                            let safe_error = logging::log_private_user_value(&format!("{err:#}"));
                            let safe_query = logging::log_private_user_value(&query);
                            tracing::warn!(
                                target: "script_kit::ghost_text",
                                error_bytes = safe_error.raw_bytes,
                                error_sha256 = %safe_error.sha256,
                                query_bytes = safe_query.raw_bytes,
                                query_sha256 = %safe_query.sha256,
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
include!("filtering_cache_tests.rs");

fn active_rich_browser_history_subsearch(query_text: &str) -> bool {
    crate::spine::catalog_subsearch::active_browser_history_subsearch(query_text)
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
