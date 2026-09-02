use super::*;

const ROOT_FILE_RESULT_CACHE_LIMIT: usize = 24;
const ROOT_FILE_SEARCH_DEBOUNCE_MS: u64 = 60;
const SPINE_FILE_SEARCH_DEBOUNCE_MS: u64 = 80;

enum MainFileWorkerEvent {
    Result(crate::file_search::FileResult),
    Done,
    Failed(MainSearchWorkerFailure),
}

#[derive(Debug)]
pub(super) enum MainSearchWorkerFailure {
    Source(anyhow::Error),
    Disconnected,
    Cancelled,
}

impl From<anyhow::Error> for MainSearchWorkerFailure {
    fn from(error: anyhow::Error) -> Self {
        Self::Source(error)
    }
}

impl From<std::io::Error> for MainSearchWorkerFailure {
    fn from(error: std::io::Error) -> Self {
        Self::Source(error.into())
    }
}

impl From<crate::file_search::SearchFailure> for MainSearchWorkerFailure {
    fn from(error: crate::file_search::SearchFailure) -> Self {
        match error {
            crate::file_search::SearchFailure::Source(error) => {
                Self::Source(std::io::Error::new(error.kind(), error).into())
            }
            crate::file_search::SearchFailure::Cancelled => Self::Cancelled,
            crate::file_search::SearchFailure::Disconnected => Self::Disconnected,
        }
    }
}

impl std::fmt::Display for MainSearchWorkerFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => std::fmt::Display::fmt(error, formatter),
            Self::Disconnected => formatter.write_str("search worker sender disconnected"),
            Self::Cancelled => formatter.write_str("search worker cancelled"),
        }
    }
}

pub(super) type MainSearchWorkerResult<T> = Result<Vec<T>, MainSearchWorkerFailure>;

pub(super) fn main_search_worker_terminal<T>(
    result: &MainSearchWorkerResult<T>,
) -> RootProviderTerminal {
    match result {
        Ok(rows) if rows.is_empty() => RootProviderTerminal::Empty,
        Ok(_) => RootProviderTerminal::Success,
        Err(MainSearchWorkerFailure::Cancelled) => RootProviderTerminal::Cancelled,
        Err(MainSearchWorkerFailure::Disconnected) => RootProviderTerminal::Disconnected,
        Err(MainSearchWorkerFailure::Source(error)) => {
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::Unsupported)
            {
                RootProviderTerminal::Unavailable
            } else {
                RootProviderTerminal::Failed
            }
        }
    }
}

pub(super) fn main_search_fixture_terminal<T>(
    result: &MainSearchWorkerResult<T>,
) -> crate::design_evaluation::search_fixtures::ProviderTerminal {
    use crate::design_evaluation::search_fixtures::ProviderTerminal as Terminal;
    match main_search_worker_terminal(result) {
        RootProviderTerminal::Success | RootProviderTerminal::Empty => Terminal::Completed {
            count: result.as_ref().map_or(0, Vec::len),
        },
        RootProviderTerminal::Cancelled => Terminal::Cancelled,
        RootProviderTerminal::Disconnected => Terminal::Disconnected,
        RootProviderTerminal::Unavailable => Terminal::Unavailable,
        _ => Terminal::Failed,
    }
}

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
                directory,
                show_hidden,
                ..
            } => format!("dir:{}:{directory}:{show_hidden}", directory.len()),
        }
    }

    fn source(&self) -> &'static str {
        match self {
            Self::GlobalQuery { .. } => "files",
            Self::DirectoryBrowse { .. } => "directory",
        }
    }

    fn work_scope(&self) -> String {
        match self {
            Self::GlobalQuery { .. } => "global".to_string(),
            Self::DirectoryBrowse { .. } => self.cache_key(),
        }
    }

    fn browse_scope(&self) -> Option<(String, bool)> {
        match self {
            Self::DirectoryBrowse {
                directory,
                show_hidden,
                ..
            } => Some((directory.clone(), *show_hidden)),
            Self::GlobalQuery { .. } => None,
        }
    }
}

impl ScriptListApp {
    fn root_file_request_for_input(
        &self,
        raw: &str,
    ) -> Option<(RootFileSearchRequest, RootProviderPublicationPolicy)> {
        if !matches!(self.current_view, AppView::ScriptList) {
            return None;
        }
        let syntax = crate::menu_syntax::MenuSyntaxMode::from_input(raw);
        let search_text = crate::menu_syntax::free_text_for_search(&syntax, raw).trim();
        let advanced = syntax.advanced_query_for(raw);
        let source = crate::menu_syntax::RootUnifiedSourceFilter::Files;
        let explicit = advanced.is_some_and(|query| query.source_filters.includes(source));
        let mut options = self.config.get_unified_search().root_file_section_options();
        if explicit {
            options.files_enabled = true;
            options.global_search_enabled = true;
            options.directory_browse_enabled = true;
            options.query_intent =
                crate::file_search::RootFileQueryIntent::ExplicitFilesSourceFilter;
        }
        if !options.files_enabled
            || advanced
                .is_some_and(|query| !query.source_filters.allows(source) || query.has_predicates())
            || syntax.capture_composer_owns_input_for(search_text)
            || syntax.command_owns_input_for(search_text)
        {
            return None;
        }
        if options.global_search_enabled
            && crate::file_search::should_search_root_files_for_intent(
                search_text,
                options.query_intent,
            )
        {
            return Some((
                RootFileSearchRequest::GlobalQuery {
                    query: search_text.to_owned(),
                },
                if explicit {
                    RootProviderPublicationPolicy::Visible
                } else {
                    RootProviderPublicationPolicy::CacheOnly
                },
            ));
        }
        if options.directory_browse_enabled
            && crate::file_search::looks_like_root_directory_browse_query(search_text)
        {
            let (directory, show_hidden) =
                if let Some(parsed) = crate::file_search::parse_directory_path(search_text) {
                    (parsed.directory, parsed.show_hidden)
                } else if self.root_search.root_file_search_query == search_text
                    && self.root_search.root_file_search_mode
                        == Some(crate::file_search::RootFileSectionMode::DirectoryBrowse)
                {
                    self.root_search.root_file_browse_scope.clone()?
                } else {
                    return None;
                };
            return Some((
                RootFileSearchRequest::DirectoryBrowse {
                    query: search_text.to_owned(),
                    directory,
                    show_hidden,
                },
                RootProviderPublicationPolicy::Visible,
            ));
        }
        None
    }

    pub(crate) fn root_file_work_identity_for_input(
        &self,
        raw: &str,
    ) -> Option<(String, String, RootProviderPublicationPolicy)> {
        self.root_file_request_for_input(raw)
            .map(|(request, policy)| (request.cache_key(), request.work_scope(), policy))
    }

    pub(crate) fn refresh_root_recent_file_results(&mut self) {
        let mut options = self.config.get_unified_search().root_file_section_options();
        if let Some(sources) = self.main_services.owned_sources() {
            self.root_search.root_recent_file_results = sources.files.clone();
            self.root_search.root_recent_file_revision = self.frecency_store.revision();
            return;
        }
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
        let changed =
            !root_file_results_equal(&self.root_search.root_recent_file_results, &next_results);
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
        if !self.root_search.query_is_current() {
            return false;
        }
        let current_search_text = crate::menu_syntax::free_text_for_search(
            self.root_search.computed_menu_syntax(),
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

        self.root_search
            .root_file_browse_scope
            .as_ref()
            .is_some_and(|(active_directory, active_hidden)| {
                active_directory == directory && *active_hidden == show_hidden
            })
    }

    fn reusable_root_file_cache_entry(
        &self,
        request: &RootFileSearchRequest,
        cache_key: &str,
    ) -> Option<(&str, &[crate::file_search::FileResult])> {
        let same_source = self.root_search.root_file_search_mode == Some(request.mode());
        let scope_matches = match request {
            RootFileSearchRequest::GlobalQuery { query } => {
                same_source && self.root_search.root_file_search_query == *query
            }
            RootFileSearchRequest::DirectoryBrowse {
                directory,
                show_hidden,
                ..
            } => {
                same_source
                    && self.active_root_directory_browse_source_matches(directory, *show_hidden)
                    && root_directory_browse_listing_is_fresh(
                        self.root_search.root_file_browse_listed_at,
                        self.main_services.search_now(),
                    )
            }
        };
        crate::file_search::reusable_root_file_cache_entry(
            &self.root_search.root_file_result_cache,
            cache_key,
            scope_matches,
        )
    }

    pub(super) fn owned_root_file_cache_readiness(
        &self,
        source: &str,
    ) -> Option<crate::root_search_store::RootSearchSourceCacheReadiness<'_>> {
        if self.root_search.named_provider_in_flight("files")
            || self.root_search.named_provider_in_flight("directory")
            || self.root_search.root_file_provider_loading
        {
            return None;
        }
        let (request, _) = self.root_file_request_for_input(&self.computed_filter_text)?;
        if request.source() != source || self.root_search.root_file_search_query != request.query()
        {
            return None;
        }
        let cache_key = request.cache_key();
        let (identity, rows) = self.reusable_root_file_cache_entry(&request, &cache_key)?;
        if !root_file_results_equal(&self.root_search.root_file_results, rows) {
            return None;
        }
        Some(crate::root_search_store::RootSearchSourceCacheReadiness {
            query: self.root_search.query_stamp(),
            identity,
            generation: None,
            row_count: rows.len(),
        })
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

    fn cache_root_file_results(
        &mut self,
        cache_key: String,
        results: Vec<crate::file_search::FileResult>,
    ) {
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
        self.start_root_file_search_for_query(query, false, cx);
    }

    pub(crate) fn refresh_root_file_source(&mut self, cx: &mut Context<Self>) {
        self.start_root_file_search_for_query(&self.computed_filter_text.clone(), true, cx);
    }

    fn start_root_file_search_for_query(
        &mut self,
        query: &str,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.root_search.query_is_current() || self.computed_filter_text != query {
            return;
        }
        let requested = self.root_file_request_for_input(query);
        let active = self.root_search.named_provider_in_flight("files")
            || self.root_search.named_provider_in_flight("directory");
        let Some((request, policy)) = requested else {
            self.cancel_root_file_search();
            let changed = self.root_search.root_file_search_mode.is_some()
                || !self.root_search.root_file_results.is_empty();
            if changed {
                self.commit_main_menu_results_refresh(
                    "root_file_scope_retired",
                    None,
                    cx,
                    |app, _cx| {
                        app.root_search.root_file_results.clear();
                        app.root_search.root_file_search_query.clear();
                        app.root_search.root_file_search_mode = None;
                        app.root_search.root_file_browse_scope = None;
                        app.root_search.root_file_browse_listed_at = None;
                        app.root_search.root_file_search_loading = false;
                        app.root_search.root_file_frame = None;
                        true
                    },
                );
            }
            return;
        };
        let source = request.source();
        let work_key = request.cache_key();
        let work_scope = request.work_scope();
        if active {
            if !force
                && self.root_search.named_provider_work_matches(
                    source,
                    self.root_search.root_file_search_generation,
                    &work_key,
                    &work_scope,
                )
            {
                // Only the accepted raw-input boundary can transfer an active Files attachment.
                self.root_search.root_file_search_query = request.query().to_owned();
                self.root_search.root_file_search_mode = Some(request.mode());
                self.root_search.root_file_frame = None;
                self.ensure_main_list_loading_animation(cx);
                return;
            }
            if force {
                for active_source in ["files", "directory"] {
                    self.root_search.detach_named_provider_consumer(
                        active_source,
                        self.root_search.root_file_search_generation,
                    );
                }
            }
            self.root_search
                .note_desired_provider(source, &work_key, &work_scope, policy);
            self.cancel_root_file_search();
            let apply = |app: &mut Self, _cx: &mut Context<Self>| {
                app.root_search.root_file_search_query = request.query().to_owned();
                app.root_search.root_file_search_mode = Some(request.mode());
                app.root_search.root_file_browse_scope = request.browse_scope();
                app.root_search.root_file_browse_listed_at = None;
                app.root_search.root_file_results =
                    app.cached_root_file_results_for_request(&request);
                app.root_search.root_file_search_loading =
                    app.root_search.root_file_results.is_empty();
                app.root_search.root_file_frame = None;
                true
            };
            if policy == RootProviderPublicationPolicy::Visible {
                self.commit_main_menu_results_refresh(
                    "root_file_waiting_for_worker",
                    None,
                    cx,
                    apply,
                );
            } else {
                apply(self, cx);
            }
            return;
        }
        let reusable_cache = if force {
            None
        } else {
            self.reusable_root_file_cache_entry(&request, &work_key)
        };
        if let Some((_, cached)) = reusable_cache {
            let adopt_cached =
                !root_file_results_equal(&self.root_search.root_file_results, cached);
            if self.root_search.root_file_search_query == request.query() && !adopt_cached {
                return;
            }
            let apply = |app: &mut Self, _cx: &mut Context<Self>| {
                app.root_search.root_file_search_query = request.query().to_owned();
                if adopt_cached {
                    app.root_search.root_file_results =
                        app.cached_root_file_results_for_request(&request);
                }
                app.root_search.root_file_search_loading = false;
                app.root_search.root_file_frame = None;
                true
            };
            if policy == RootProviderPublicationPolicy::Visible {
                self.commit_main_menu_results_refresh("root_file_cached_scope", None, cx, apply);
            } else {
                apply(self, cx);
            }
            return;
        }

        let generation = self.root_search.allocate_named_provider_generation(source);
        let cancel = crate::file_search::new_cancel_token();
        let apply = |app: &mut Self, _cx: &mut Context<Self>| {
            app.root_search.begin_named_provider(
                source,
                generation,
                &work_key,
                &work_scope,
                policy,
                true,
            );
            app.root_search.root_file_search_generation = generation;
            app.root_search.root_file_search_query = request.query().to_owned();
            app.root_search.root_file_search_mode = Some(request.mode());
            app.root_search.root_file_browse_scope = request.browse_scope();
            app.root_search.root_file_browse_listed_at = None;
            app.root_search.root_file_results = app.cached_root_file_results_for_request(&request);
            app.root_search.root_file_search_loading = app.root_search.root_file_results.is_empty();
            app.root_search.root_file_provider_loading = true;
            app.root_search.root_file_frame = None;
            app.root_search.root_file_search_cancel = Some(cancel.clone());
            true
        };
        if policy == RootProviderPublicationPolicy::Visible {
            self.commit_main_menu_results_refresh("root_file_search_started", None, cx, apply);
        } else {
            apply(self, cx);
        }
        self.ensure_main_list_loading_animation(cx);

        let services = self.main_services.clone();
        let owned_run = if let Some(gate) = self.main_services.search_gate() {
            match gate.begin(source, request.query(), generation, policy) {
                Some(run) => Some(Arc::new(run)),
                None => {
                    self.finish_root_file_worker(
                        generation,
                        &request,
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            "owned_file_gate_unavailable",
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
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    ROOT_FILE_SEARCH_DEBOUNCE_MS,
                ))
                .await;
            let (tx, rx) = std::sync::mpsc::channel::<MainFileWorkerEvent>();
            if let Some(run) = owned_run.clone() {
                cx.background_executor()
                    .spawn(async move {
                        run.deliver(
                            move |result: anyhow::Result<Vec<crate::file_search::FileResult>>| {
                                match result {
                                    Ok(files) => {
                                        for file in files {
                                            let _ = tx.send(MainFileWorkerEvent::Result(file));
                                        }
                                        let _ = tx.send(MainFileWorkerEvent::Done);
                                    }
                                    Err(error) => {
                                        let _ = tx.send(MainFileWorkerEvent::Failed(error.into()));
                                    }
                                }
                            },
                            crate::design_evaluation::search_fixtures::file_results,
                        )
                        .await;
                    })
                    .detach();
            } else if let Some(sources) = services.owned_sources() {
                cx.background_executor().timer(sources.file_delay).await;
                let files = sources
                    .root_file_provider_files
                    .as_deref()
                    .unwrap_or(&sources.files);
                let needle = request.query().to_lowercase();
                let directory = match &request {
                    RootFileSearchRequest::DirectoryBrowse {
                        directory,
                        show_hidden,
                        ..
                    } => crate::file_search::expand_path(directory)
                        .map(|path| (std::path::PathBuf::from(path), *show_hidden)),
                    _ => None,
                };
                for file in files.iter().filter(|file| {
                    if let Some((directory, hidden)) = &directory {
                        std::path::Path::new(&file.path).parent() == Some(directory.as_path())
                            && (*hidden || !file.name.starts_with('.'))
                    } else {
                        file.name.to_lowercase().contains(&needle)
                    }
                }) {
                    let _ = tx.send(MainFileWorkerEvent::Result(file.clone()));
                }
                let _ = tx.send(MainFileWorkerEvent::Done);
            } else {
                let producer_request = request.clone();
                let producer_cancel = cancel.clone();
                std::thread::spawn(move || match producer_request {
                    RootFileSearchRequest::GlobalQuery { query } => {
                        let mut emit = |event| {
                            let event = match event {
                                crate::file_search::SearchEvent::Result(file) => {
                                    MainFileWorkerEvent::Result(file)
                                }
                                crate::file_search::SearchEvent::Done(Ok(())) => {
                                    MainFileWorkerEvent::Done
                                }
                                crate::file_search::SearchEvent::Done(Err(error)) => {
                                    MainFileWorkerEvent::Failed(error.into())
                                }
                            };
                            let _ = tx.send(event);
                        };
                        if emit_root_file_search_test_fixture(&query, &producer_cancel, &mut emit) {
                            return;
                        }
                        let provider_query =
                            crate::file_search::root_file_provider_query_for_user_query(&query);
                        crate::file_search::search_files_streaming_with_options(
                            &provider_query,
                            None,
                            crate::file_search::ROOT_FILE_SOURCE_LIMIT,
                            producer_cancel,
                            crate::file_search::SearchFilesStreamingOptions::root_search(),
                            emit,
                        );
                    }
                    RootFileSearchRequest::DirectoryBrowse {
                        directory,
                        show_hidden,
                        ..
                    } => {
                        let files = match crate::file_search::list_directory_with_options(
                            &directory,
                            crate::file_search::ROOT_FILE_BROWSE_SOURCE_LIMIT,
                            show_hidden,
                        ) {
                            Ok(files) => files,
                            Err(error) => {
                                let _ = tx.send(MainFileWorkerEvent::Failed(error.into()));
                                return;
                            }
                        };
                        for file in files {
                            if producer_cancel.load(std::sync::atomic::Ordering::Relaxed) {
                                return;
                            }
                            let _ = tx.send(MainFileWorkerEvent::Result(file));
                        }
                        let _ = tx.send(MainFileWorkerEvent::Done);
                    }
                });
            }
            let mut batch = Vec::new();
            let result = loop {
                match rx.try_recv() {
                    Ok(MainFileWorkerEvent::Result(file)) => batch.push(file),
                    Ok(MainFileWorkerEvent::Done) => {
                        break if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            Err(MainSearchWorkerFailure::Cancelled)
                        } else {
                            Ok(batch)
                        }
                    }
                    Ok(MainFileWorkerEvent::Failed(error)) => break Err(error),
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        break Err(if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            MainSearchWorkerFailure::Cancelled
                        } else {
                            MainSearchWorkerFailure::Disconnected
                        });
                    }
                }
            };
            if this
                .update(cx, |app, cx| {
                    app.finish_root_file_worker(
                        generation,
                        &request,
                        result,
                        owned_run.as_deref(),
                        cx,
                    )
                })
                .is_err()
            {
                if let Some(run) = owned_run.as_deref() {
                    run.finish(
                        crate::design_evaluation::search_fixtures::ProviderTerminal::StaleDiscarded,
                        RootProviderPublicationPolicy::CacheOnly,
                    );
                }
            }
        })
        .detach();
    }

    fn finish_root_file_worker(
        &mut self,
        generation: u64,
        request: &RootFileSearchRequest,
        result: MainSearchWorkerResult<crate::file_search::FileResult>,
        owned_run: Option<&crate::design_evaluation::search_fixtures::SearchRun>,
        cx: &mut Context<Self>,
    ) {
        let source = request.source();
        let current_work = self
            .root_search
            .named_provider_work_is_current(source, generation);
        let accepted = self.root_search.accepts_named_provider(source, generation);
        let can_cache = self
            .root_search
            .named_provider_consumer_is_live(source, generation);
        let publish = accepted && self.root_file_request_should_publish_now(generation, request);
        let terminal = main_search_worker_terminal(&result);
        if let Some(run) = owned_run {
            run.finish(
                if can_cache {
                    main_search_fixture_terminal(&result)
                } else {
                    crate::design_evaluation::search_fixtures::ProviderTerminal::StaleDiscarded
                },
                if publish {
                    RootProviderPublicationPolicy::Visible
                } else {
                    RootProviderPublicationPolicy::CacheOnly
                },
            );
        }
        if !current_work {
            return;
        }
        let apply = |app: &mut Self, _cx: &mut Context<Self>| {
            app.root_search.finish_named_provider(
                source,
                generation,
                if can_cache {
                    terminal
                } else {
                    RootProviderTerminal::StaleDiscarded
                },
            );
            if app.root_search.root_file_search_generation != generation {
                return false;
            }
            app.root_search.root_file_provider_loading = false;
            app.root_search.root_file_search_loading = false;
            app.root_search.root_file_search_cancel = None;
            if can_cache {
                if let Ok(results) = result {
                    if publish {
                        app.cache_root_file_results(request.cache_key(), results.clone());
                        app.root_search.root_file_results = dedupe_root_file_results(results);
                    } else {
                        app.cache_root_file_results(request.cache_key(), results);
                    }
                    if matches!(request, RootFileSearchRequest::DirectoryBrowse { .. }) {
                        app.root_search.root_file_browse_listed_at =
                            Some(app.main_services.search_now());
                    }
                }
            }
            if publish {
                app.root_search.root_file_frame = None;
            }
            true
        };
        if publish {
            self.commit_main_menu_results_refresh(
                "root_file_results_publish",
                Some((source, generation)),
                cx,
                apply,
            );
        } else {
            apply(self, cx);
        }
        let restart_files = self.root_search.take_named_provider_desired("files");
        let restart_directory = self.root_search.take_named_provider_desired("directory");
        if restart_files || restart_directory {
            self.refresh_root_file_source(cx);
        } else if !can_cache && self.root_search.root_file_search_generation == generation {
            self.root_search.root_file_search_query.clear();
        }
    }
}

fn root_file_results_equal(
    left: &[crate::file_search::FileResult],
    right: &[crate::file_search::FileResult],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            a.path == b.path
                && a.name == b.name
                && a.size == b.size
                && a.modified == b.modified
                && a.file_type == b.file_type
        })
}

/// How long a same-directory browse may serve its cached readdir listing
/// before a repeat request re-runs the provider. Fragment typing inside one
/// directory stays on the fast path; the listing still tracks live
/// create/delete churn within this bound (chaos battery 05: deleted files
/// stayed listed and selectable forever while browsing the same directory).
const ROOT_FILE_BROWSE_REFRESH_TTL: std::time::Duration = std::time::Duration::from_secs(2);

/// Pure freshness gate for the same-directory browse fast path. `None`
/// (never listed this browse) is stale so the provider always runs once.
fn root_directory_browse_listing_is_fresh(
    listed_at: Option<std::time::Instant>,
    now: std::time::Instant,
) -> bool {
    listed_at.is_some_and(|at| now.saturating_duration_since(at) < ROOT_FILE_BROWSE_REFRESH_TTL)
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

    #[test]
    fn worker_control_outcomes_do_not_reclassify_native_io_errors() {
        for kind in [
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::BrokenPipe,
            std::io::ErrorKind::PermissionDenied,
        ] {
            let result: MainSearchWorkerResult<()> =
                Err(std::io::Error::new(kind, "native source read").into());
            assert_eq!(
                main_search_worker_terminal(&result),
                RootProviderTerminal::Failed
            );
        }
        let unavailable: MainSearchWorkerResult<()> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "native source unavailable",
        )
        .into());
        assert_eq!(
            main_search_worker_terminal(&unavailable),
            RootProviderTerminal::Unavailable
        );
        assert_eq!(
            main_search_worker_terminal::<()>(&Err(MainSearchWorkerFailure::Cancelled)),
            RootProviderTerminal::Cancelled
        );
        assert_eq!(
            main_search_worker_terminal::<()>(&Err(MainSearchWorkerFailure::Disconnected)),
            RootProviderTerminal::Disconnected
        );
    }

    #[test]
    fn file_semantics_include_equal_length_name_and_metadata_changes() {
        let original = crate::file_search::FileResult {
            path: "/owned/launch.md".into(),
            name: "Alpha".into(),
            size: 5,
            modified: 1,
            file_type: crate::file_search::FileType::Document,
        };
        let mut changed = original.clone();
        changed.name = "Bravo".into();
        assert!(!root_file_results_equal(
            std::slice::from_ref(&original),
            std::slice::from_ref(&changed)
        ));
        changed = original.clone();
        changed.modified = 2;
        assert!(!root_file_results_equal(
            std::slice::from_ref(&original),
            std::slice::from_ref(&changed)
        ));
        assert!(root_file_results_equal(
            std::slice::from_ref(&original),
            std::slice::from_ref(&original)
        ));
    }

    /// Chaos battery 05: same-directory fragment typing must reuse the cached
    /// readdir listing only within the refresh TTL; a never-listed browse and
    /// an aged listing must both re-run the provider so live filesystem
    /// create/delete churn (deleted rows staying selectable) stays bounded.
    #[test]
    fn directory_browse_listing_freshness_gate() {
        let now = std::time::Instant::now();
        assert!(!root_directory_browse_listing_is_fresh(None, now));
        assert!(root_directory_browse_listing_is_fresh(Some(now), now));
        let aged = now + ROOT_FILE_BROWSE_REFRESH_TTL;
        assert!(!root_directory_browse_listing_is_fresh(Some(now), aged));
        let within = now + ROOT_FILE_BROWSE_REFRESH_TTL / 2;
        assert!(root_directory_browse_listing_is_fresh(Some(now), within));
    }

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
    emit: &mut impl FnMut(crate::file_search::SearchEvent),
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
                emit(crate::file_search::SearchEvent::Done(Ok(())));
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
        emit(crate::file_search::SearchEvent::Done(Err(
            crate::file_search::SearchFailure::Cancelled,
        )));
        return true;
    }

    for result in fixture.results {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            emit(crate::file_search::SearchEvent::Done(Err(
                crate::file_search::SearchFailure::Cancelled,
            )));
            return true;
        }
        emit(crate::file_search::SearchEvent::Result(
            result.into_file_result(),
        ));
    }
    emit(crate::file_search::SearchEvent::Done(Ok(())));
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

#[derive(Clone)]
enum SpineFileSearchRequest {
    GlobalQuery(String),
    GlobalRecents(Option<String>),
    ProjectQuery { query: String, scope: String },
    ProjectRecents(String),
}

impl SpineFileSearchRequest {
    fn key(&self) -> String {
        match self {
            Self::GlobalQuery(query) => query.clone(),
            Self::GlobalRecents(scope) => format!(
                "{SPINE_FILE_RECENTS_SENTINEL}\u{1f}{}",
                scope.as_deref().unwrap_or("global")
            ),
            Self::ProjectQuery { query, scope } => format!(
                "{SPINE_PROJECT_SEARCH_KEY_PREFIX}{}:{scope}{query}",
                scope.len()
            ),
            Self::ProjectRecents(scope) => {
                format!("{SPINE_PROJECT_RECENTS_SENTINEL_PREFIX}{scope}")
            }
        }
    }

    fn scope(&self) -> &str {
        match self {
            Self::GlobalQuery(_) => "global",
            Self::GlobalRecents(scope) => scope.as_deref().unwrap_or("global"),
            Self::ProjectQuery { scope, .. } | Self::ProjectRecents(scope) => scope,
        }
    }

    fn query(&self) -> &str {
        match self {
            Self::GlobalQuery(query) | Self::ProjectQuery { query, .. } => query,
            Self::GlobalRecents(_) | Self::ProjectRecents(_) => "",
        }
    }

    fn recents(&self) -> bool {
        matches!(self, Self::GlobalRecents(_) | Self::ProjectRecents(_))
    }

    fn emit(
        &self,
        cancel: crate::file_search::CancelToken,
        tx: std::sync::mpsc::Sender<MainFileWorkerEvent>,
    ) {
        use crate::file_search::{SearchEvent, SearchFilesStreamingOptions};
        let mut hits = 0;
        let mut completion = Ok(());
        let mut emit = |event| match event {
            SearchEvent::Result(file) => {
                if !self.recents() || !crate::file_search::is_noisy_recent_file_path(&file.path) {
                    hits += 1;
                    let _ = tx.send(MainFileWorkerEvent::Result(file));
                }
            }
            SearchEvent::Done(result) => completion = result,
        };
        match self {
            Self::GlobalQuery(query) => {
                if !emit_root_file_search_test_fixture(query, &cancel, &mut emit) {
                    let query = crate::file_search::root_file_provider_query_for_user_query(query);
                    crate::file_search::search_files_streaming_with_options(
                        &query,
                        None,
                        crate::file_search::ROOT_FILE_SOURCE_LIMIT,
                        cancel.clone(),
                        SearchFilesStreamingOptions::root_search(),
                        &mut emit,
                    );
                }
            }
            Self::ProjectQuery { query, scope } => {
                crate::file_search::search_files_streaming_with_options(
                    query,
                    Some(scope),
                    crate::file_search::ROOT_FILE_SOURCE_LIMIT,
                    cancel.clone(),
                    SearchFilesStreamingOptions {
                        skip_metadata: true,
                        allow_filesystem_fallback: true,
                    },
                    &mut emit,
                )
            }
            Self::GlobalRecents(scope) => crate::file_search::search_files_streaming_with_options(
                crate::file_search::RECENTLY_USED_FILES_MDQUERY,
                scope.as_deref(),
                crate::file_search::RECENTLY_USED_FILES_SOURCE_LIMIT,
                cancel.clone(),
                SearchFilesStreamingOptions {
                    skip_metadata: false,
                    allow_filesystem_fallback: false,
                },
                &mut emit,
            ),
            Self::ProjectRecents(scope) => {
                if let Some(spotlight_scope) =
                    spotlight_indexable_scope(std::path::Path::new(scope))
                {
                    crate::file_search::search_files_streaming_with_options(
                        crate::file_search::RECENTLY_USED_FILES_MDQUERY,
                        Some(&spotlight_scope),
                        crate::file_search::RECENTLY_USED_FILES_SOURCE_LIMIT,
                        cancel.clone(),
                        SearchFilesStreamingOptions {
                            skip_metadata: false,
                            allow_filesystem_fallback: false,
                        },
                        &mut emit,
                    );
                }
                if completion.is_ok()
                    && hits == 0
                    && !cancel.load(std::sync::atomic::Ordering::Relaxed)
                {
                    match crate::file_search::recent_files_filesystem(
                        std::path::Path::new(scope),
                        crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT,
                    ) {
                        Ok(files) => {
                            for file in files {
                                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                                    break;
                                }
                                let _ = tx.send(MainFileWorkerEvent::Result(file));
                            }
                        }
                        Err(error) => completion = Err(error.into()),
                    }
                }
            }
        }
        let terminal = if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            MainFileWorkerEvent::Failed(MainSearchWorkerFailure::Cancelled)
        } else {
            match completion {
                Ok(()) => MainFileWorkerEvent::Done,
                Err(error) => MainFileWorkerEvent::Failed(error.into()),
            }
        };
        let _ = tx.send(terminal);
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
        if self.spine_file_search_query.is_empty()
            && self.spine_file_search_results.is_empty()
            && !self.spine_file_search_loading
        {
            return;
        }
        self.commit_main_menu_results_refresh("spine_file_scope_cleared", None, cx, |app, _cx| {
            app.spine_file_search_query.clear();
            app.spine_file_search_results.clear();
            app.spine_file_search_loading = false;
            true
        });
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
        let projection = self.root_search.computed_spine_projection()?;
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
        self.start_spine_file_subsearch_for_current_projection(false, cx);
    }

    pub(crate) fn refresh_spine_file_source(&mut self, cx: &mut Context<Self>) {
        self.start_spine_file_subsearch_for_current_projection(true, cx);
    }

    fn start_spine_file_subsearch_for_current_projection(
        &mut self,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.root_search.query_is_current() {
            return;
        }
        let Some((source, query)) = self.active_spine_context_subsearch() else {
            self.clear_spine_file_subsearch_state(cx);
            return;
        };
        use crate::spine::catalog_subsearch::ContextSubsearchSource;
        let query = query.trim().to_string();
        let request = match (source, query.is_empty()) {
            (ContextSubsearchSource::File, false) => SpineFileSearchRequest::GlobalQuery(query),
            (ContextSubsearchSource::File, true) => SpineFileSearchRequest::GlobalRecents(
                self.spine_cwd
                    .as_deref()
                    .and_then(spotlight_indexable_scope)
                    .or_else(|| dirs::home_dir().map(|home| home.to_string_lossy().to_string())),
            ),
            (ContextSubsearchSource::Project, empty) => {
                let Some(scope) = self.spine_project_scope_dir() else {
                    self.clear_spine_file_subsearch_state(cx);
                    return;
                };
                if empty {
                    SpineFileSearchRequest::ProjectRecents(scope)
                } else {
                    SpineFileSearchRequest::ProjectQuery { query, scope }
                }
            }
            _ => {
                self.clear_spine_file_subsearch_state(cx);
                return;
            }
        };
        self.start_spine_file_worker(request, force, cx);
    }

    /// Directory the `@project:` subsearch is scoped to: the global cwd chip,
    /// falling back to home when no cwd is set.
    fn spine_project_scope_dir(&self) -> Option<String> {
        self.spine_cwd
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .or_else(|| dirs::home_dir().map(|home| home.to_string_lossy().to_string()))
    }

    fn start_spine_file_worker(
        &mut self,
        request: SpineFileSearchRequest,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let key = request.key();
        if self.root_search.named_provider_in_flight("spine") {
            if force
                || !self.root_search.named_provider_work_matches(
                    "spine",
                    self.spine_file_search_generation,
                    &key,
                    request.scope(),
                )
            {
                if force {
                    self.root_search
                        .detach_named_provider_consumer("spine", self.spine_file_search_generation);
                }
                self.root_search.note_desired_provider(
                    "spine",
                    &key,
                    request.scope(),
                    RootProviderPublicationPolicy::Visible,
                );
                self.cancel_spine_file_subsearch();
                if self.spine_file_search_query != key || !self.spine_file_search_loading {
                    self.commit_main_menu_results_refresh(
                        "spine_file_waiting_for_worker",
                        None,
                        cx,
                        |app, _cx| {
                            if app.spine_file_search_query != key {
                                app.spine_file_search_results.clear();
                            }
                            app.spine_file_search_query = key;
                            app.spine_file_search_loading = true;
                            true
                        },
                    );
                }
            }
            return;
        }
        if !force && self.spine_file_search_query == key {
            return;
        }
        let generation = self.root_search.allocate_named_provider_generation("spine");
        let cancel = crate::file_search::new_cancel_token();
        self.commit_main_menu_results_refresh("spine_file_search_started", None, cx, |app, _cx| {
            app.root_search.begin_named_provider(
                "spine",
                generation,
                &key,
                request.scope(),
                RootProviderPublicationPolicy::Visible,
                true,
            );
            app.spine_file_search_generation = generation;
            if app.spine_file_search_query != key {
                app.spine_file_search_results.clear();
            }
            app.spine_file_search_query = key;
            app.spine_file_search_loading = true;
            app.spine_file_search_cancel = Some(cancel.clone());
            true
        });
        let services = self.main_services.clone();
        let owned_run = if let Some(gate) = services.search_gate() {
            match gate.begin(
                "spine",
                request.query(),
                generation,
                RootProviderPublicationPolicy::Visible,
            ) {
                Some(run) => Some(Arc::new(run)),
                None => {
                    self.finish_spine_file_worker(
                        generation,
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            "spine_fixture_source_unavailable",
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
        cx.spawn(async move |this, cx| {
            if !request.recents() {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(
                        SPINE_FILE_SEARCH_DEBOUNCE_MS,
                    ))
                    .await;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            if let Some(run) = owned_run.clone() {
                cx.background_executor()
                    .spawn(async move {
                        run.deliver(
                            move |result: anyhow::Result<Vec<crate::file_search::FileResult>>| {
                                match result {
                                    Ok(files) => {
                                        for file in files {
                                            let _ = tx.send(MainFileWorkerEvent::Result(file));
                                        }
                                        let _ = tx.send(MainFileWorkerEvent::Done);
                                    }
                                    Err(error) => {
                                        let _ = tx.send(MainFileWorkerEvent::Failed(error.into()));
                                    }
                                }
                            },
                            crate::design_evaluation::search_fixtures::file_results,
                        )
                        .await;
                    })
                    .detach();
            } else if let Some(sources) = services.owned_sources() {
                cx.background_executor().timer(sources.file_delay).await;
                let needle = request.query().to_lowercase();
                for file in &sources.files {
                    if file.name.to_lowercase().contains(&needle) {
                        let _ = tx.send(MainFileWorkerEvent::Result(file.clone()));
                    }
                }
                let _ = tx.send(MainFileWorkerEvent::Done);
            } else {
                let request = request.clone();
                let cancel = cancel.clone();
                std::thread::spawn(move || request.emit(cancel, tx));
            }
            let mut batch = Vec::new();
            let result = loop {
                match rx.try_recv() {
                    Ok(MainFileWorkerEvent::Result(file)) => {
                        if !request.recents()
                            || !crate::file_search::is_noisy_recent_file_path(&file.path)
                        {
                            batch.push(file);
                        }
                    }
                    Ok(MainFileWorkerEvent::Done) => {
                        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            break Err(MainSearchWorkerFailure::Cancelled);
                        }
                        if request.recents() {
                            batch.sort_by_key(|file| std::cmp::Reverse(file.modified));
                            batch.truncate(crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT);
                        }
                        break Ok(batch);
                    }
                    Ok(MainFileWorkerEvent::Failed(error)) => break Err(error),
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        break Err(if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            MainSearchWorkerFailure::Cancelled
                        } else {
                            MainSearchWorkerFailure::Disconnected
                        })
                    }
                }
            };
            if this
                .update(cx, |app, cx| {
                    app.finish_spine_file_worker(generation, result, owned_run.as_deref(), cx)
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

    fn finish_spine_file_worker(
        &mut self,
        generation: u64,
        result: MainSearchWorkerResult<crate::file_search::FileResult>,
        owned_run: Option<&crate::design_evaluation::search_fixtures::SearchRun>,
        cx: &mut Context<Self>,
    ) {
        let accepted = self.root_search.accepts_named_provider("spine", generation)
            && self.spine_file_search_generation == generation;
        if let Some(run) = owned_run {
            run.finish(
                if accepted {
                    main_search_fixture_terminal(&result)
                } else {
                    crate::design_evaluation::search_fixtures::ProviderTerminal::StaleDiscarded
                },
                RootProviderPublicationPolicy::Visible,
            );
        }
        if !self
            .root_search
            .named_provider_work_is_current("spine", generation)
        {
            return;
        }
        if accepted {
            let terminal = main_search_worker_terminal(&result);
            self.commit_main_menu_results_refresh(
                "spine_file_results_complete",
                Some(("spine", generation)),
                cx,
                |app, _cx| {
                    app.root_search
                        .finish_named_provider("spine", generation, terminal);
                    app.spine_file_search_loading = false;
                    app.spine_file_search_cancel = None;
                    if let Ok(results) = result {
                        app.spine_file_search_results = dedupe_root_file_results(results);
                    }
                    true
                },
            );
        } else {
            self.root_search.finish_named_provider(
                "spine",
                generation,
                RootProviderTerminal::StaleDiscarded,
            );
            if self.spine_file_search_generation == generation {
                self.spine_file_search_loading = false;
                self.spine_file_search_cancel = None;
            }
        }
        if self.root_search.take_named_provider_desired("spine") {
            self.refresh_spine_file_source(cx);
        } else if !accepted && self.spine_file_search_generation == generation {
            self.spine_file_search_query.clear();
        }
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
