static NEXT_ROOT_SEARCH_LIFETIME: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

fn next_root_search_lifetime() -> u64 {
    NEXT_ROOT_SEARCH_LIFETIME
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |value| value.checked_add(1),
        )
        .expect("root query lifetime exhausted")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RootSearchQueryStamp {
    pub(crate) lifetime: u64,
    pub(crate) revision: u64,
    pub(crate) scope_revision: u64,
}

/// Present source-cache evidence for the current committed query, not the
/// identity or consumer attachment of the worker that originally filled it.
pub(crate) struct RootSearchSourceCacheReadiness<'a> {
    pub(crate) query: RootSearchQueryStamp,
    pub(crate) identity: &'a str,
    pub(crate) generation: Option<u64>,
    pub(crate) row_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RootProviderPublicationPolicy {
    Visible,
    CacheOnly,
    VisibleSynchronous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RootProviderTerminal {
    Success,
    Empty,
    Failed,
    Unavailable,
    Disconnected,
    Cancelled,
    StaleDiscarded,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RootProviderOwnership {
    pub(crate) source: &'static str,
    pub(crate) generation: u64,
    work_query: String,
    work_scope: String,
    pub(crate) consumer: Option<RootSearchQueryStamp>,
    publication_policy: RootProviderPublicationPolicy,
    pub(crate) query_bound: bool,
    pub(crate) terminal: Option<RootProviderTerminal>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RootProviderDesired {
    source: &'static str,
    query: RootSearchQueryStamp,
    work_query: String,
    work_scope: String,
    publication_policy: RootProviderPublicationPolicy,
}

fn passive_provider_source(source: sk_protocol::command_contract::CommandSource) -> &'static str {
    use sk_protocol::command_contract::CommandSource;
    match source {
        CommandSource::BrowserTab => "tabs",
        CommandSource::BrowserHistory => "history",
        CommandSource::Note => "notes",
        CommandSource::Todo => "todos",
        CommandSource::Clipboard => "clipboard",
        CommandSource::Dictation => "dictation",
        CommandSource::Conversation => "conversations",
        source => source.prefix(),
    }
}

/// Parser inputs committed together for one root-search query computation.
pub(crate) struct RootSearchComputedInputs {
    pub(crate) raw: String,
    pub(crate) menu_syntax: crate::menu_syntax::MenuSyntaxMode,
    pub(crate) spine_parse: crate::spine::SpineParse,
    pub(crate) spine_projection: Option<crate::spine::SpineCursorProjection>,
}

/// Root-launcher Windows, file, and Brain search state owned as one coherent async cohort.
pub(crate) struct RootSearchStore {
    query_stamp: RootSearchQueryStamp,
    computed_query_stamp: Option<RootSearchQueryStamp>,
    query_owner_active: bool,
    accepted_query: String,
    accepted_scope: String,
    computed_query_inputs: std::rc::Rc<RootSearchComputedInputs>,
    provider_ownership: std::collections::BTreeMap<&'static str, RootProviderOwnership>,
    provider_desired: std::collections::BTreeMap<&'static str, RootProviderDesired>,
    named_provider_generations: std::collections::BTreeMap<&'static str, u64>,
    script_catalogue_candidates: std::sync::Arc<[std::sync::Arc<crate::scripts::Script>]>,
    script_catalogue_revision: u64,
    /// Query-scoped generation fences for every asynchronous passive provider.
    /// Existing Windows/Brain/Files tokens remain authoritative for their
    /// specialized stores; this extends the same guarantee to newer sources.
    provider_generations: crate::scripts::root_search_contract::RootProviderCoordinator,
    /// Frozen cache-refreshable passive rows for the current root-search query frame.
    root_passive_frame: Option<crate::RootPassiveFrame>,
    /// App-layer enriched rows for root/unified `windows:` search.
    cached_root_windows: Vec<crate::scripts::RootWindowEntry>,
    /// Last provider state for root unified `windows:` search.
    root_windows_provider_status: crate::window_control::RootWindowsProviderStatus,
    /// Generation bumped when root unified search refreshes cached windows.
    root_windows_refresh_generation: u64,
    /// Token used to drop stale async root window refresh results.
    root_windows_refresh_token: u64,
    /// True while an async root window refresh is in flight.
    root_windows_refreshing: bool,
    /// Last successful root window refresh completion.
    root_windows_last_completed_at: Option<std::time::Instant>,
    /// In-memory local recency for windows focused through Script Kit.
    root_window_focus_recency: std::collections::HashMap<String, u64>,
    /// Sequence number for in-memory root window recency.
    root_window_focus_seq: u64,
    /// One accepted lexical snapshot, never read from IO during grouping.
    root_brain_lexical_results:
        Option<(RootSearchQueryStamp, Vec<crate::brain::RootBrainSearchHit>)>,
    root_brain_lexical_request_stamp: Option<RootSearchQueryStamp>,
    /// Async hybrid brain hits keyed by the trimmed root-launcher search text.
    root_brain_semantic_results: Option<(String, Vec<crate::brain::RootBrainSearchHit>)>,
    root_brain_semantic_results_stamp: Option<RootSearchQueryStamp>,
    /// Generation counter used to ignore stale semantic brain batches.
    root_brain_search_generation: u64,
    /// Last requested semantic brain search, used to avoid duplicate work.
    root_brain_search_request: Option<(String, crate::brain::RootBrainSectionOptions)>,
    root_brain_semantic_request_stamp: Option<RootSearchQueryStamp>,
    /// Exact worker ticket retained until its own receiver completes or disconnects.
    root_brain_semantic_in_flight: Option<u64>,
    /// Revision folded into frame keys for accepted lexical and semantic snapshots.
    root_brain_source_epoch: u64,
    /// Open brain-inbox items pinned above the empty root-launcher query.
    root_brain_inbox_items: Vec<crate::brain::InboxItem>,
    /// When the root brain-inbox snapshot was last loaded.
    root_brain_inbox_loaded_at: Option<std::time::Instant>,
    /// Revision folded into grouped cache keys when inbox items change.
    root_brain_inbox_epoch: u64,
    /// Latest capped Spotlight results appended to eligible root launcher searches.
    pub(crate) root_file_results: Vec<crate::file_search::FileResult>,
    /// Bounded completed global root file batches, keyed by root search request.
    pub(crate) root_file_result_cache:
        std::collections::VecDeque<(String, Vec<crate::file_search::FileResult>)>,
    /// Source mode currently backing `root_file_results`.
    pub(crate) root_file_search_mode: Option<crate::file_search::RootFileSectionMode>,
    /// Accepted directory source identity, independent of later filesystem availability.
    pub(crate) root_file_browse_scope: Option<(String, bool)>,
    /// When the active directory-browse listing last (re)ran readdir. Bounds
    /// how stale same-directory fragment typing can leave the file rows.
    pub(crate) root_file_browse_listed_at: Option<std::time::Instant>,
    /// Frecency-backed file rows shown on the empty root launcher.
    pub(crate) root_recent_file_results: Vec<crate::file_search::FileResult>,
    /// Frecency revision currently backing `root_recent_file_results`.
    pub(crate) root_recent_file_revision: u64,
    /// Query currently backing `root_file_results`.
    pub(crate) root_file_search_query: String,
    /// Generation counter used to ignore stale root file search batches.
    pub(crate) root_file_search_generation: u64,
    /// Cancel token for in-flight root file search.
    pub(crate) root_file_search_cancel: Option<crate::file_search::CancelToken>,
    /// True while a root file search task is collecting its one stable batch.
    pub(crate) root_file_search_loading: bool,
    /// True while the root file provider is still collecting/cache-warming.
    pub(crate) root_file_provider_loading: bool,
    /// Frozen global root file rows for the current root-search query frame.
    pub(crate) root_file_frame: Option<crate::RootFileFrame>,
    /// Page key for the explicit Files source-chip visible-row budget.
    pub(crate) root_file_source_chip_page_key: Option<String>,
    /// Current visible-row budget for the explicit Files source-chip page.
    pub(crate) root_file_source_chip_visible_limit: usize,
}

impl Default for RootSearchStore {
    fn default() -> Self {
        Self {
            query_stamp: RootSearchQueryStamp {
                lifetime: next_root_search_lifetime(),
                ..RootSearchQueryStamp::default()
            },
            computed_query_stamp: None,
            query_owner_active: true,
            accepted_query: String::new(),
            accepted_scope: String::new(),
            computed_query_inputs: std::rc::Rc::new(RootSearchComputedInputs {
                raw: String::new(),
                menu_syntax: crate::menu_syntax::MenuSyntaxMode::default(),
                spine_parse: crate::spine::parse_spine(""),
                spine_projection: None,
            }),
            provider_ownership: std::collections::BTreeMap::new(),
            provider_desired: std::collections::BTreeMap::new(),
            named_provider_generations: std::collections::BTreeMap::new(),
            script_catalogue_candidates: std::sync::Arc::from([]),
            script_catalogue_revision: 0,
            provider_generations:
                crate::scripts::root_search_contract::RootProviderCoordinator::default(),
            root_passive_frame: None,
            cached_root_windows: Vec::new(),
            root_windows_provider_status: crate::window_control::RootWindowsProviderStatus::Unknown,
            root_windows_refresh_generation: 0,
            root_windows_refresh_token: 0,
            root_windows_refreshing: false,
            root_windows_last_completed_at: None,
            root_window_focus_recency: std::collections::HashMap::new(),
            root_window_focus_seq: 0,
            root_brain_lexical_results: None,
            root_brain_lexical_request_stamp: None,
            root_brain_semantic_results: None,
            root_brain_semantic_results_stamp: None,
            root_brain_search_generation: 0,
            root_brain_search_request: None,
            root_brain_semantic_request_stamp: None,
            root_brain_semantic_in_flight: None,
            root_brain_source_epoch: 0,
            root_brain_inbox_items: Vec::new(),
            root_brain_inbox_loaded_at: None,
            root_brain_inbox_epoch: 0,
            root_file_results: Vec::new(),
            root_file_result_cache: std::collections::VecDeque::new(),
            root_file_search_mode: None,
            root_file_browse_scope: None,
            root_file_browse_listed_at: None,
            root_recent_file_results: Vec::new(),
            root_recent_file_revision: u64::MAX,
            root_file_search_query: String::new(),
            root_file_search_generation: 0,
            root_file_search_cancel: None,
            root_file_search_loading: false,
            root_file_provider_loading: false,
            root_file_frame: None,
            root_file_source_chip_page_key: None,
            root_file_source_chip_visible_limit:
                crate::file_search::ROOT_FILE_SOURCE_CHIP_INITIAL_VISIBLE_ROWS,
        }
    }
}

impl RootSearchStore {
    pub(crate) fn install_script_catalogue_candidates(
        &mut self,
        candidates: std::sync::Arc<[std::sync::Arc<crate::scripts::Script>]>,
    ) -> u64 {
        self.script_catalogue_revision = self
            .script_catalogue_revision
            .checked_add(1)
            .expect("script catalogue revision exhausted");
        self.script_catalogue_candidates = candidates;
        self.script_catalogue_revision
    }

    pub(crate) fn script_catalogue_candidates(
        &self,
    ) -> (
        u64,
        std::sync::Arc<[std::sync::Arc<crate::scripts::Script>]>,
    ) {
        (
            self.script_catalogue_revision,
            self.script_catalogue_candidates.clone(),
        )
    }

    pub(crate) fn script_catalogue_revision(&self) -> u64 {
        self.script_catalogue_revision
    }

    pub(crate) fn query_stamp(&self) -> RootSearchQueryStamp {
        self.query_stamp
    }

    pub(crate) fn computed_query_stamp(&self) -> Option<RootSearchQueryStamp> {
        self.computed_query_stamp
    }

    pub(crate) fn query_is_current(&self) -> bool {
        self.query_owner_active && self.computed_query_stamp == Some(self.query_stamp)
    }

    pub(crate) fn accepted_query(&self) -> &str {
        &self.accepted_query
    }

    pub(crate) fn accepted_scope(&self) -> &str {
        &self.accepted_scope
    }

    pub(crate) fn computed_query_inputs(&self) -> std::rc::Rc<RootSearchComputedInputs> {
        std::rc::Rc::clone(&self.computed_query_inputs)
    }

    pub(crate) fn computed_menu_syntax(&self) -> &crate::menu_syntax::MenuSyntaxMode {
        &self.computed_query_inputs.menu_syntax
    }

    pub(crate) fn computed_spine_parse(&self) -> &crate::spine::SpineParse {
        &self.computed_query_inputs.spine_parse
    }

    pub(crate) fn computed_spine_projection(&self) -> Option<&crate::spine::SpineCursorProjection> {
        self.computed_query_inputs.spine_projection.as_ref()
    }

    fn retire_passive_consumers(&mut self) {
        use sk_protocol::command_contract::CommandSource;
        for source in [
            CommandSource::BrowserTab,
            CommandSource::BrowserHistory,
            CommandSource::Note,
            CommandSource::Todo,
            CommandSource::Clipboard,
            CommandSource::Dictation,
            CommandSource::Conversation,
        ] {
            self.provider_generations.invalidate(source);
        }
        self.provider_desired.retain(|source, _| {
            self.provider_ownership
                .get(source)
                .is_some_and(|run| !run.query_bound)
        });
    }

    /// Accept input before computation. A detached query-bound run can never be
    /// reattached by a later equal string (A→B→A). Compatible pending Files work
    /// may transfer its still-current attachment exactly once at this boundary.
    pub(crate) fn accept_query_intent(
        &mut self,
        raw: &str,
        scope: &str,
        compatible_file_work: Option<(&str, &str, RootProviderPublicationPolicy)>,
    ) -> bool {
        if self.query_owner_active && self.accepted_query == raw && self.accepted_scope == scope {
            return false;
        }
        let previous = self.query_stamp;
        if !self.query_owner_active {
            self.query_stamp.lifetime = next_root_search_lifetime();
            self.query_owner_active = true;
        }
        self.query_stamp.revision = self
            .query_stamp
            .revision
            .checked_add(1)
            .expect("root query revision exhausted");
        if self.accepted_scope != scope {
            self.query_stamp.scope_revision = self
                .query_stamp
                .scope_revision
                .checked_add(1)
                .expect("root query scope exhausted");
        }
        self.accepted_query.clear();
        self.accepted_query.push_str(raw);
        self.accepted_scope.clear();
        self.accepted_scope.push_str(scope);
        self.retire_passive_consumers();
        for run in self.provider_ownership.values_mut() {
            let compatible_file = matches!(run.source, "files" | "directory")
                && run.terminal.is_none()
                && run.consumer == Some(previous)
                && compatible_file_work.is_some_and(|(query, scope, _)| {
                    run.work_query == query && run.work_scope == scope
                });
            run.consumer = if !run.query_bound || compatible_file {
                Some(self.query_stamp)
            } else {
                None
            };
            if compatible_file {
                run.publication_policy = compatible_file_work.expect("matched compatible work").2;
            }
        }
        true
    }

    pub(crate) fn retire_query_owner(&mut self) {
        if !self.query_owner_active {
            return;
        }
        self.query_owner_active = false;
        self.query_stamp.lifetime = next_root_search_lifetime();
        self.query_stamp.revision = self
            .query_stamp
            .revision
            .checked_add(1)
            .expect("root query revision exhausted");
        self.computed_query_stamp = None;
        self.retire_passive_consumers();
        for run in self.provider_ownership.values_mut() {
            run.consumer = None;
        }
    }

    pub(crate) fn commit_query_inputs(
        &mut self,
        raw: &str,
        menu_syntax: crate::menu_syntax::MenuSyntaxMode,
        spine_parse: crate::spine::SpineParse,
        spine_projection: Option<crate::spine::SpineCursorProjection>,
    ) -> bool {
        if !self.query_owner_active || self.accepted_query != raw {
            return false;
        }
        self.computed_query_inputs = std::rc::Rc::new(RootSearchComputedInputs {
            raw: raw.to_owned(),
            menu_syntax,
            spine_parse,
            spine_projection,
        });
        for run in self
            .provider_ownership
            .values_mut()
            .filter(|run| !run.query_bound)
        {
            run.consumer = Some(self.query_stamp);
        }
        self.computed_query_stamp = Some(self.query_stamp);
        true
    }

    pub(crate) fn note_desired_provider(
        &mut self,
        source: &'static str,
        work_query: &str,
        work_scope: &str,
        publication_policy: RootProviderPublicationPolicy,
    ) {
        self.provider_desired.insert(
            source,
            RootProviderDesired {
                source,
                query: self.query_stamp,
                work_query: work_query.to_owned(),
                work_scope: work_scope.to_owned(),
                publication_policy,
            },
        );
    }

    pub(crate) fn begin_named_provider(
        &mut self,
        source: &'static str,
        generation: u64,
        work_query: &str,
        work_scope: &str,
        publication_policy: RootProviderPublicationPolicy,
        query_bound: bool,
    ) {
        self.provider_desired.remove(source);
        self.provider_ownership.insert(
            source,
            RootProviderOwnership {
                source,
                generation,
                work_query: work_query.to_owned(),
                work_scope: work_scope.to_owned(),
                consumer: self.query_is_current().then_some(self.query_stamp),
                publication_policy,
                query_bound,
                terminal: None,
            },
        );
    }

    pub(crate) fn detach_named_provider_consumer(&mut self, source: &str, generation: u64) -> bool {
        let Some(run) = self.provider_ownership.get_mut(source) else {
            return false;
        };
        if run.generation != generation || run.terminal.is_some() {
            return false;
        }
        run.consumer = None;
        true
    }

    pub(crate) fn allocate_named_provider_generation(&mut self, source: &'static str) -> u64 {
        let generation = self.named_provider_generations.entry(source).or_default();
        *generation = generation
            .checked_add(1)
            .expect("root provider generation exhausted");
        *generation
    }

    pub(crate) fn named_provider_work_is_current(&self, source: &str, generation: u64) -> bool {
        self.provider_ownership
            .get(source)
            .is_some_and(|run| run.generation == generation && run.terminal.is_none())
    }

    pub(crate) fn named_provider_in_flight(&self, source: &str) -> bool {
        self.provider_ownership
            .get(source)
            .is_some_and(|run| run.terminal.is_none())
    }

    pub(crate) fn active_named_provider_generation(&self, source: &str) -> Option<u64> {
        self.provider_ownership
            .get(source)
            .filter(|run| run.terminal.is_none())
            .map(|run| run.generation)
    }

    pub(crate) fn named_provider_consumer_is_live(&self, source: &str, generation: u64) -> bool {
        self.provider_ownership.get(source).is_some_and(|run| {
            run.generation == generation
                && run.terminal.is_none()
                && run.consumer == Some(self.query_stamp)
        })
    }

    pub(crate) fn named_provider_has_current_consumer(&self, source: &str, scope: &str) -> bool {
        self.query_owner_active
            && self.provider_ownership.get(source).is_some_and(|run| {
                run.consumer == Some(self.query_stamp)
                    && run.work_scope == scope
                    && run.terminal != Some(RootProviderTerminal::StaleDiscarded)
            })
    }

    /// Read the actual native owner, including work admitted before its IO gate.
    /// A detached or absent consumer is not evidence that its worker has ended.
    pub(crate) fn provider_admission_owner(&self, source: &str) -> Option<&RootProviderOwnership> {
        self.provider_ownership.get(source)
    }

    /// Observe the same demand the completion path would consume, without
    /// changing the queue. Query-independent catalogue demand survives input.
    pub(crate) fn provider_has_pending_desire(&self, source: &str) -> bool {
        self.provider_desired.get(source).is_some_and(|desired| {
            self.provider_ownership
                .get(source)
                .is_some_and(|run| !run.query_bound)
                || (self.query_is_current() && desired.query == self.query_stamp)
        })
    }

    pub(crate) fn take_named_provider_desired(&mut self, source: &str) -> bool {
        let Some(desired) = self.provider_desired.remove(source) else {
            return false;
        };
        self.provider_ownership
            .get(source)
            .is_some_and(|run| !run.query_bound)
            || (self.query_is_current() && desired.query == self.query_stamp)
    }

    pub(crate) fn accepts_named_provider(&self, source: &str, generation: u64) -> bool {
        self.query_is_current()
            && self.provider_ownership.get(source).is_some_and(|run| {
                run.generation == generation
                    && run.consumer == Some(self.query_stamp)
                    && run.terminal.is_none()
            })
    }

    pub(crate) fn named_provider_work_matches(
        &self,
        source: &str,
        generation: u64,
        query: &str,
        scope: &str,
    ) -> bool {
        self.accepts_named_provider(source, generation)
            && self
                .provider_ownership
                .get(source)
                .is_some_and(|run| run.work_query == query && run.work_scope == scope)
    }

    pub(crate) fn finish_named_provider(
        &mut self,
        source: &str,
        generation: u64,
        terminal: RootProviderTerminal,
    ) -> bool {
        let Some(run) = self.provider_ownership.get_mut(source) else {
            return false;
        };
        if run.generation != generation || run.terminal.is_some() {
            return false;
        }
        run.terminal = Some(terminal);
        true
    }

    pub(crate) fn provider_observation(&self) -> serde_json::Value {
        serde_json::json!({
            "version": 1,
            "runs": self.provider_ownership.values().collect::<Vec<_>>(),
            "desired": self.provider_desired.values().collect::<Vec<_>>(),
        })
    }

    /// Retire one owned fixture's source stores without reusing specialized tickets.
    pub(crate) fn reset_owned_fixture(&mut self) {
        assert!(crate::runtime_policy::is_owned_evaluation());
        if let Some(cancel) = self.root_file_search_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.retire_query_owner();
        let stamp = RootSearchQueryStamp {
            lifetime: next_root_search_lifetime(),
            revision: self
                .query_stamp
                .revision
                .checked_add(1)
                .expect("root query revision exhausted"),
            scope_revision: self
                .query_stamp
                .scope_revision
                .checked_add(1)
                .expect("root query scope exhausted"),
        };
        let files = self
            .root_file_search_generation
            .checked_add(1)
            .expect("root file generation exhausted");
        let brain = self
            .root_brain_search_generation
            .checked_add(1)
            .expect("root brain generation exhausted");
        let windows = self
            .root_windows_refresh_token
            .checked_add(1)
            .expect("root window generation exhausted");
        let coordinator = std::mem::take(&mut self.provider_generations);
        let named_generations = std::mem::take(&mut self.named_provider_generations);
        let script_catalogue_revision = self
            .script_catalogue_revision
            .checked_add(1)
            .expect("script catalogue revision exhausted");
        *self = Self::default();
        self.query_stamp = stamp;
        self.provider_generations = coordinator;
        self.named_provider_generations = named_generations;
        self.script_catalogue_revision = script_catalogue_revision;
        self.root_file_search_generation = files;
        self.root_brain_search_generation = brain;
        self.root_windows_refresh_token = windows;
    }
    pub(crate) fn begin_provider_request(
        &mut self,
        source: sk_protocol::command_contract::CommandSource,
        query: &str,
    ) -> sk_protocol::search_contract::ProviderRequest {
        // Acquisition, not equal query text, owns a worker identity. Retire the
        // previous run before the coordinator's same-query reuse optimization.
        self.provider_generations.invalidate(source);
        let request = self.provider_generations.begin(source, query);
        let scope = self.accepted_scope.clone();
        self.begin_named_provider(
            passive_provider_source(source),
            request.generation,
            query,
            &scope,
            RootProviderPublicationPolicy::Visible,
            true,
        );
        request
    }

    pub(crate) fn note_desired_request(
        &mut self,
        source: sk_protocol::command_contract::CommandSource,
        query: &str,
    ) {
        let source = passive_provider_source(source);
        self.provider_desired.insert(
            source,
            RootProviderDesired {
                source,
                query: self.query_stamp,
                work_query: query.to_owned(),
                work_scope: self.accepted_scope.clone(),
                publication_policy: RootProviderPublicationPolicy::Visible,
            },
        );
    }

    pub(crate) fn finish_provider_request(
        &mut self,
        request: &sk_protocol::search_contract::ProviderRequest,
        terminal: RootProviderTerminal,
    ) -> bool {
        self.finish_named_provider(
            passive_provider_source(request.source),
            request.generation,
            terminal,
        )
    }

    pub(crate) fn take_desired_provider_query(
        &mut self,
        source: sk_protocol::command_contract::CommandSource,
    ) -> Option<String> {
        let desired = self
            .provider_desired
            .remove(passive_provider_source(source))?;
        (self.query_is_current()
            && desired.query == self.query_stamp
            && desired.work_scope == self.accepted_scope)
            .then_some(desired.work_query)
    }

    pub(crate) fn accepts_provider_request(
        &self,
        request: &sk_protocol::search_contract::ProviderRequest,
        current_query: &str,
    ) -> bool {
        self.accepts_named_provider(passive_provider_source(request.source), request.generation)
            && self.accepted_query == current_query
            && self.provider_generations.accepts(request, current_query)
    }

    pub(crate) fn invalidate_provider_request(
        &mut self,
        source: sk_protocol::command_contract::CommandSource,
    ) {
        self.provider_generations.invalidate(source);
        let source = passive_provider_source(source);
        self.provider_desired.remove(source);
        if let Some(run) = self.provider_ownership.get_mut(source) {
            run.consumer = None;
        }
    }

    pub(crate) fn root_brain_source_epoch(&self) -> u64 {
        self.root_brain_source_epoch
    }

    pub(crate) fn root_brain_lexical_request_is_current(&self) -> bool {
        self.query_is_current() && self.root_brain_lexical_request_stamp == Some(self.query_stamp)
    }

    pub(crate) fn root_brain_lexical_results(&self) -> &[crate::brain::RootBrainSearchHit] {
        match &self.root_brain_lexical_results {
            Some((stamp, hits)) if self.query_is_current() && *stamp == self.query_stamp => hits,
            _ => &[],
        }
    }

    pub(crate) fn invalidate_root_brain_lexical_freshness(&mut self) {
        self.root_brain_lexical_request_stamp = None;
    }

    pub(crate) fn install_root_brain_lexical_results(
        &mut self,
        generation: u64,
        result: anyhow::Result<Vec<crate::brain::RootBrainSearchHit>>,
        max_results: usize,
    ) -> bool {
        if !self.accepts_named_provider("brain-lexical", generation) {
            return false;
        }
        self.root_brain_lexical_request_stamp = Some(self.query_stamp);
        let Ok(mut hits) = result else {
            return false;
        };
        hits.truncate(max_results);
        if self
            .root_brain_lexical_results
            .as_ref()
            .is_some_and(|(stamp, previous)| *stamp == self.query_stamp && *previous == hits)
        {
            return false;
        }
        self.root_brain_lexical_results = Some((self.query_stamp, hits));
        self.root_brain_source_epoch = self
            .root_brain_source_epoch
            .checked_add(1)
            .expect("root brain source epoch exhausted");
        true
    }

    pub(crate) fn root_brain_semantic_results(
        &self,
    ) -> Option<&(String, Vec<crate::brain::RootBrainSearchHit>)> {
        if !self.query_is_current()
            || self.root_brain_semantic_results_stamp != Some(self.query_stamp)
        {
            return None;
        }
        self.root_brain_semantic_results.as_ref()
    }

    pub(crate) fn root_brain_semantic_request_matches(
        &self,
        query: &str,
        options: crate::brain::RootBrainSectionOptions,
    ) -> bool {
        self.root_brain_semantic_request_stamp == Some(self.query_stamp)
            && self.root_brain_search_request.as_ref().is_some_and(
                |(requested_query, requested_options)| {
                    requested_query == query && *requested_options == options
                },
            )
    }

    pub(crate) fn begin_root_brain_semantic_request(
        &mut self,
        query: String,
        options: crate::brain::RootBrainSectionOptions,
    ) -> Option<u64> {
        if !self.query_is_current() || self.root_brain_semantic_in_flight.is_some() {
            return None;
        }
        self.root_brain_search_generation = self.root_brain_search_generation.checked_add(1)?;
        let generation = self.root_brain_search_generation;
        self.begin_named_provider(
            "brain-semantic",
            generation,
            &query,
            "",
            RootProviderPublicationPolicy::Visible,
            true,
        );
        self.root_brain_search_request = Some((query, options));
        self.root_brain_semantic_request_stamp = Some(self.query_stamp);
        self.root_brain_semantic_in_flight = Some(generation);
        Some(generation)
    }

    pub(crate) fn root_brain_semantic_generation_matches(&self, generation: u64) -> bool {
        self.root_brain_search_generation == generation
            && self.root_brain_semantic_in_flight == Some(generation)
            && self.root_brain_semantic_request_stamp == Some(self.query_stamp)
            && self.accepts_named_provider("brain-semantic", generation)
    }

    pub(crate) fn finish_root_brain_semantic_request(
        &mut self,
        generation: u64,
        terminal: RootProviderTerminal,
    ) -> bool {
        if self.root_brain_semantic_in_flight != Some(generation) {
            return false;
        }
        self.root_brain_semantic_in_flight = None;
        self.finish_named_provider("brain-semantic", generation, terminal);
        true
    }

    pub(crate) fn install_root_brain_semantic_results(
        &mut self,
        generation: u64,
        query: String,
        mut hits: Vec<crate::brain::RootBrainSearchHit>,
    ) -> bool {
        if !self.root_brain_semantic_generation_matches(generation) {
            return false;
        }
        let Some((requested_query, options)) = &self.root_brain_search_request else {
            return false;
        };
        if requested_query != &query {
            return false;
        }
        hits.truncate(options.max_results);
        if self.root_brain_semantic_results_stamp == Some(self.query_stamp)
            && self.root_brain_semantic_results.as_ref().is_some_and(
                |(previous_query, previous_hits)| {
                    *previous_query == query && *previous_hits == hits
                },
            )
        {
            return false;
        }
        self.root_brain_semantic_results = Some((query, hits));
        self.root_brain_semantic_results_stamp = Some(self.query_stamp);
        self.root_brain_source_epoch = self
            .root_brain_source_epoch
            .checked_add(1)
            .expect("root brain source epoch exhausted");
        true
    }

    pub(crate) fn invalidate_root_brain_semantic_freshness(&mut self) {
        if self.root_brain_search_request.take().is_some() {
            self.root_brain_search_generation = self.root_brain_search_generation.saturating_add(1);
        }
        self.root_brain_semantic_request_stamp = None;
    }

    pub(crate) fn root_brain_inbox_epoch(&self) -> u64 {
        self.root_brain_inbox_epoch
    }

    pub(crate) fn root_brain_inbox_items(&self) -> &[crate::brain::InboxItem] {
        &self.root_brain_inbox_items
    }

    pub(crate) fn root_brain_inbox_cache_is_fresh(
        &self,
        now: std::time::Instant,
        ttl: std::time::Duration,
    ) -> bool {
        self.root_brain_inbox_loaded_at
            .is_some_and(|loaded_at| now.saturating_duration_since(loaded_at) < ttl)
    }

    pub(crate) fn invalidate_root_brain_inbox_freshness(&mut self) {
        self.root_brain_inbox_loaded_at = None;
    }

    pub(crate) fn install_root_brain_inbox_read(
        &mut self,
        generation: u64,
        now: std::time::Instant,
        result: anyhow::Result<Vec<crate::brain::InboxItem>>,
        allow_reorder: bool,
        max_results: usize,
    ) -> bool {
        if !self.accepts_named_provider("brain-inbox", generation) {
            return false;
        }
        self.root_brain_inbox_loaded_at = Some(now);
        let Ok(mut items) = result else {
            return false;
        };
        items.truncate(max_results);
        if !allow_reorder {
            items = crate::brain::stable_merge_open_inbox(&self.root_brain_inbox_items, items);
        }
        self.install_root_brain_inbox_items(items)
    }

    pub(crate) fn install_root_brain_inbox_items(
        &mut self,
        items: Vec<crate::brain::InboxItem>,
    ) -> bool {
        if crate::scripts::root_search_contract::brain_inbox_snapshot_matches(
            &self.root_brain_inbox_items,
            &items,
        ) {
            return false;
        }
        self.root_brain_inbox_items = items;
        self.root_brain_inbox_epoch = self
            .root_brain_inbox_epoch
            .checked_add(1)
            .expect("root brain inbox epoch exhausted");
        true
    }

    pub(crate) fn remove_root_brain_inbox_item(&mut self, id: i64) -> bool {
        let previous_len = self.root_brain_inbox_items.len();
        self.root_brain_inbox_items.retain(|item| item.id != id);
        let changed = self.root_brain_inbox_items.len() != previous_len;
        if changed {
            self.root_brain_inbox_epoch = self
                .root_brain_inbox_epoch
                .checked_add(1)
                .expect("root brain inbox epoch exhausted");
        }
        changed
    }

    pub(crate) fn with_root_windows(
        windows: &[crate::window_control::WindowInfo],
        apps: &[crate::app_launcher::AppInfo],
        root_windows_provider_status: crate::window_control::RootWindowsProviderStatus,
    ) -> Self {
        Self {
            cached_root_windows: Self::build_root_window_entries(
                windows,
                apps,
                &std::collections::HashMap::new(),
            ),
            root_windows_provider_status,
            ..Self::default()
        }
    }

    fn root_window_duplicate_key(window: &crate::window_control::WindowInfo) -> (String, String) {
        (
            window
                .bundle_id
                .clone()
                .unwrap_or_else(|| window.app.to_lowercase()),
            window.title.to_lowercase(),
        )
    }

    fn root_window_duplicate_counts(
        windows: &[crate::window_control::WindowInfo],
    ) -> std::collections::HashMap<(String, String), usize> {
        let mut counts = std::collections::HashMap::new();
        for window in windows {
            *counts
                .entry(Self::root_window_duplicate_key(window))
                .or_insert(0) += 1;
        }
        counts
    }

    fn build_root_window_entries(
        windows: &[crate::window_control::WindowInfo],
        apps: &[crate::app_launcher::AppInfo],
        recency: &std::collections::HashMap<String, u64>,
    ) -> Vec<crate::scripts::RootWindowEntry> {
        let lookup = crate::app_launcher::AppIconLookup::from_apps(apps);
        let duplicate_counts = Self::root_window_duplicate_counts(windows);
        let mut duplicate_seen = std::collections::HashMap::<(String, String), usize>::new();

        let mut entries = windows
            .iter()
            .cloned()
            .map(|window| {
                let duplicate_key = Self::root_window_duplicate_key(&window);
                let duplicate_count = duplicate_counts.get(&duplicate_key).copied().unwrap_or(1);
                let duplicate_rank = if duplicate_count > 1 {
                    let rank = duplicate_seen.entry(duplicate_key).or_insert(0);
                    *rank += 1;
                    Some(*rank)
                } else {
                    None
                };
                let duplicate_label =
                    duplicate_rank.map(|rank| format!("Window {rank} of {duplicate_count}"));
                let subtitle = crate::window_control::build_window_descriptor(
                    &window.app,
                    window.pid,
                    window.bounds,
                    window.is_frontmost_app,
                    window.is_focused,
                    window.is_main,
                    window.is_minimized,
                    window.is_on_current_space,
                    duplicate_label.as_deref(),
                );
                let local_recency_seq = recency.get(&window.selection_key()).copied();
                crate::scripts::RootWindowEntry {
                    app_icon: lookup.icon_for_window(&window),
                    subtitle,
                    duplicate_rank,
                    duplicate_count,
                    local_recency_seq,
                    window,
                }
            })
            .collect::<Vec<_>>();

        entries.sort_by(|a, b| {
            b.window
                .is_frontmost_app
                .cmp(&a.window.is_frontmost_app)
                .then_with(|| b.window.is_focused.cmp(&a.window.is_focused))
                .then_with(|| b.window.is_main.cmp(&a.window.is_main))
                .then_with(|| b.local_recency_seq.cmp(&a.local_recency_seq))
                .then_with(|| a.window.is_minimized.cmp(&b.window.is_minimized))
                .then_with(|| a.window.app_order.cmp(&b.window.app_order))
                .then_with(|| a.window.window_index.cmp(&b.window.window_index))
                .then_with(|| a.window.title.cmp(&b.window.title))
                .then_with(|| a.window.id.cmp(&b.window.id))
        });

        entries
    }

    pub(crate) fn root_windows(
        &self,
    ) -> (
        &[crate::scripts::RootWindowEntry],
        crate::window_control::RootWindowsProviderStatus,
    ) {
        (
            &self.cached_root_windows,
            self.root_windows_provider_status.clone(),
        )
    }

    pub(crate) fn root_windows_refresh_generation(&self) -> u64 {
        self.root_windows_refresh_generation
    }

    pub(crate) fn clear_root_passive_frame(&mut self) {
        self.root_passive_frame = None;
    }

    pub(crate) fn cached_root_passive_frame(
        &self,
        key: &crate::RootPassiveFrameKey,
    ) -> Option<crate::RootPassiveFrame> {
        self.root_passive_frame
            .as_ref()
            .filter(|frame| &frame.key == key)
            .cloned()
    }

    pub(crate) fn cache_root_passive_frame(
        &mut self,
        frame: crate::RootPassiveFrame,
    ) -> crate::RootPassiveFrame {
        self.root_passive_frame = Some(frame.clone());
        frame
    }

    pub(crate) fn root_passive_frame(&self) -> Option<&crate::RootPassiveFrame> {
        self.root_passive_frame.as_ref()
    }

    pub(crate) fn install_root_windows(
        &mut self,
        windows: &[crate::window_control::WindowInfo],
        apps: &[crate::app_launcher::AppInfo],
    ) {
        self.cached_root_windows =
            Self::build_root_window_entries(windows, apps, &self.root_window_focus_recency);
        self.root_windows_refreshing = false;
        self.bump_root_windows_refresh_generation();
        self.root_windows_provider_status =
            crate::window_control::RootWindowsProviderStatus::Ready {
                count: windows.len(),
            };
        self.root_windows_last_completed_at = Some(crate::runtime_policy::root_search_now());
    }

    pub(crate) fn rebuild_root_windows(
        &mut self,
        windows: &[crate::window_control::WindowInfo],
        apps: &[crate::app_launcher::AppInfo],
    ) {
        self.cached_root_windows =
            Self::build_root_window_entries(windows, apps, &self.root_window_focus_recency);
        self.bump_root_windows_refresh_generation();
    }

    pub(crate) fn root_windows_refresh_needed(&self) -> bool {
        let stale = self
            .root_windows_last_completed_at
            .map(|completed_at| {
                crate::runtime_policy::root_search_now().saturating_duration_since(completed_at)
                    >= std::time::Duration::from_secs(3)
            })
            .unwrap_or(true);
        !self.root_windows_refreshing && (self.cached_root_windows.is_empty() || stale)
    }

    /// Positive current window snapshot evidence, independent of provider attachment.
    pub(crate) fn root_windows_fresh_cache_status(&self) -> Option<(u64, usize)> {
        if self.root_windows_refreshing
            || self.named_provider_in_flight("windows")
            || self.root_windows_refresh_needed()
            || self.root_windows_last_completed_at.is_none()
            || !matches!(
                self.root_windows_provider_status,
                crate::window_control::RootWindowsProviderStatus::Ready { .. }
            )
        {
            return None;
        }
        Some((
            self.root_windows_refresh_generation,
            self.cached_root_windows.len(),
        ))
    }

    pub(crate) fn begin_root_windows_refresh(&mut self) -> u64 {
        self.root_windows_refreshing = true;
        self.root_windows_refresh_token = self.allocate_named_provider_generation("windows");
        self.root_windows_provider_status =
            crate::window_control::RootWindowsProviderStatus::Refreshing {
                count: self.cached_root_windows.len(),
            };
        self.bump_root_windows_refresh_generation();
        self.root_windows_refresh_token
    }

    pub(crate) fn root_windows_refresh_token_matches(&self, token: u64) -> bool {
        self.root_windows_refresh_token == token
    }

    pub(crate) fn discard_root_windows_refresh(&mut self, token: u64) {
        if self.root_windows_refresh_token != token {
            return;
        }
        self.root_windows_refreshing = false;
        self.root_windows_provider_status =
            crate::window_control::RootWindowsProviderStatus::Ready {
                count: self.cached_root_windows.len(),
            };
    }

    pub(crate) fn invalidate_root_windows_source_snapshot(&mut self) {
        self.root_windows_last_completed_at = None;
    }

    pub(crate) fn fail_root_windows_refresh(
        &mut self,
        status: crate::window_control::RootWindowsProviderStatus,
    ) {
        self.root_windows_refreshing = false;
        self.root_windows_provider_status = status;
        self.bump_root_windows_refresh_generation();
    }

    pub(crate) fn record_root_window_focus(&mut self, selection_key: String) {
        self.root_window_focus_seq = self.root_window_focus_seq.wrapping_add(1);
        self.root_window_focus_recency
            .insert(selection_key, self.root_window_focus_seq);
    }

    fn bump_root_windows_refresh_generation(&mut self) {
        self.root_windows_refresh_generation = self.root_windows_refresh_generation.wrapping_add(1);
    }
}

#[cfg(test)]
mod root_search_store_tests {
    use super::*;

    fn commit_brain_query(store: &mut RootSearchStore, query: &str, scope: &str) {
        store.accept_query_intent(query, scope, None);
        assert!(store.commit_query_inputs(
            query,
            crate::menu_syntax::MenuSyntaxMode::default(),
            crate::spine::parse_spine(query),
            None,
        ));
    }

    fn begin_brain_sync_read(store: &mut RootSearchStore, source: &'static str) -> u64 {
        let generation = store.allocate_named_provider_generation(source);
        let query = store.accepted_query().to_owned();
        store.begin_named_provider(
            source,
            generation,
            &query,
            "",
            RootProviderPublicationPolicy::VisibleSynchronous,
            true,
        );
        generation
    }

    fn semantic_hit(title: &str) -> crate::brain::RootBrainSearchHit {
        crate::brain::RootBrainSearchHit {
            title: title.to_string(),
            excerpt: String::new(),
            source_label: "Note",
            source: crate::brain::DocSource::Note,
            source_id: title.to_string(),
        }
    }

    fn inbox_item(id: i64, title: &str) -> crate::brain::InboxItem {
        crate::brain::InboxItem {
            id,
            kind: crate::brain::inbox::InboxKind::Question,
            title: title.to_string(),
            detail: String::new(),
            source: "note".to_string(),
            source_id: id.to_string(),
            created_at: 0,
            resolved_at: None,
        }
    }

    fn passive_frame(query: &str) -> crate::RootPassiveFrame {
        crate::RootPassiveFrame {
            key: crate::RootPassiveFrameKey {
                query: query.to_string(),
                advanced_query: false,
                source_filters: Default::default(),
                todo_options: Default::default(),
                brain_options: Default::default(),
                brain_source_epoch: 0,
                notes_options: Default::default(),
                clipboard_history_options: Default::default(),
                dictation_history_options: Default::default(),
                agent_chat_history_options: Default::default(),
                ai_vault_options: Default::default(),
                ai_vault_snapshot_generation: 0,
                browser_tabs_options: Default::default(),
                browser_tabs_snapshot_generation: 0,
                browser_history_options: Default::default(),
                browser_history_snapshot_generation: 0,
            },
            note_hits: Vec::new(),
            brain_hits: Vec::new(),
            todo_hits: Vec::new(),
            clipboard_history_hits: Vec::new(),
            dictation_history_hits: Vec::new(),
            agent_chat_history_hits: Vec::new(),
            ai_vault_hits: Vec::new(),
            browser_tab_hits: Vec::new(),
            browser_history_hits: Vec::new(),
            ai_vault_snapshot_generation: 0,
            browser_tabs_snapshot_generation: 0,
            browser_history_snapshot_generation: 0,
        }
    }

    #[test]
    fn default_preserves_root_search_startup_contract() {
        let store = RootSearchStore::default();

        assert!(store.root_passive_frame().is_none());
        assert!(store.cached_root_windows.is_empty());
        assert!(matches!(
            store.root_windows_provider_status,
            crate::window_control::RootWindowsProviderStatus::Unknown
        ));
        assert_eq!(store.root_windows_refresh_generation, 0);
        assert_eq!(store.root_windows_refresh_token, 0);
        assert!(!store.root_windows_refreshing);
        assert!(store.root_windows_last_completed_at.is_none());
        assert!(store.root_window_focus_recency.is_empty());
        assert_eq!(store.root_window_focus_seq, 0);
        assert!(store.root_brain_semantic_results.is_none());
        assert_eq!(store.root_brain_search_generation, 0);
        assert!(store.root_brain_search_request.is_none());
        assert_eq!(store.root_brain_source_epoch, 0);
        assert!(store.root_brain_inbox_items.is_empty());
        assert!(store.root_brain_inbox_loaded_at.is_none());
        assert_eq!(store.root_brain_inbox_epoch, 0);
        assert!(store.root_file_results.is_empty());
        assert!(store.root_file_result_cache.is_empty());
        assert_eq!(store.root_file_search_mode, None);
        assert!(store.root_recent_file_results.is_empty());
        assert_eq!(store.root_recent_file_revision, u64::MAX);
        assert!(store.root_file_search_query.is_empty());
        assert_eq!(store.root_file_search_generation, 0);
        assert!(store.root_file_search_cancel.is_none());
        assert!(!store.root_file_search_loading);
        assert!(!store.root_file_provider_loading);
        assert!(store.root_file_frame.is_none());
        assert!(store.root_file_source_chip_page_key.is_none());
        assert_eq!(
            store.root_file_source_chip_visible_limit,
            crate::file_search::ROOT_FILE_SOURCE_CHIP_INITIAL_VISIBLE_ROWS
        );
    }

    #[test]
    fn settled_catalogue_consumer_survives_queries_without_restarting_work() {
        let mut store = RootSearchStore::default();
        commit_brain_query(&mut store, "first", "all");
        let generation = store.allocate_named_provider_generation("icons");
        store.begin_named_provider(
            "icons",
            generation,
            "app-icons",
            "catalogue",
            RootProviderPublicationPolicy::Visible,
            false,
        );
        assert!(store.named_provider_has_current_consumer("icons", "catalogue"));
        assert!(store.finish_named_provider("icons", generation, RootProviderTerminal::Success));
        commit_brain_query(&mut store, "second", "windows");
        assert!(store.named_provider_has_current_consumer("icons", "catalogue"));
        assert!(!store.named_provider_has_current_consumer("icons", "different-installation-paths"));
        assert!(!store.named_provider_in_flight("icons"));
        store.retire_query_owner();
        assert!(!store.named_provider_has_current_consumer("icons", "catalogue"));
    }

    #[test]
    fn detached_query_bound_consumer_never_returns_through_equal_query_text() {
        let mut store = RootSearchStore::default();
        commit_brain_query(&mut store, "first", "windows");
        let generation = store.allocate_named_provider_generation("windows");
        store.begin_named_provider(
            "windows",
            generation,
            "window-snapshot",
            "window-server",
            RootProviderPublicationPolicy::Visible,
            true,
        );
        commit_brain_query(&mut store, "second", "windows");
        commit_brain_query(&mut store, "first", "windows");
        assert!(!store.named_provider_has_current_consumer("windows", "window-server"));
        assert!(store.named_provider_in_flight("windows"));
        assert!(store.finish_named_provider(
            "windows",
            generation,
            RootProviderTerminal::StaleDiscarded
        ));
        assert!(!store.named_provider_has_current_consumer("windows", "window-server"));
    }

    #[test]
    fn stale_semantic_batches_release_only_their_bounded_worker() {
        let mut store = RootSearchStore::default();
        let options = crate::brain::RootBrainSectionOptions::default();
        commit_brain_query(&mut store, "old", "brain");
        let stale = store
            .begin_root_brain_semantic_request("old".into(), options)
            .unwrap();
        commit_brain_query(&mut store, "current", "brain");
        assert!(store
            .begin_root_brain_semantic_request("current".into(), options)
            .is_none());
        assert!(!store.install_root_brain_semantic_results(
            stale,
            "old".into(),
            vec![semantic_hit("stale")]
        ));
        assert!(
            store.finish_root_brain_semantic_request(stale, RootProviderTerminal::StaleDiscarded)
        );
        let current = store
            .begin_root_brain_semantic_request("current".into(), options)
            .unwrap();
        assert!(
            !store.finish_root_brain_semantic_request(stale, RootProviderTerminal::StaleDiscarded)
        );
        assert!(store.root_brain_semantic_generation_matches(current));
        assert!(store.install_root_brain_semantic_results(
            current,
            "current".into(),
            vec![semantic_hit("installed")]
        ));
        assert!(store.finish_root_brain_semantic_request(current, RootProviderTerminal::Success));
        assert!(!store.install_root_brain_semantic_results(
            current,
            "current".into(),
            vec![semantic_hit("duplicate")]
        ));
        assert_eq!(
            store.root_brain_semantic_results().unwrap().1[0].title,
            "installed"
        );
        assert_eq!(store.root_brain_source_epoch(), 1);
    }

    #[test]
    fn semantic_source_changes_and_failed_reads_preserve_last_good() {
        let mut store = RootSearchStore::default();
        let options = crate::brain::RootBrainSectionOptions::default();
        commit_brain_query(&mut store, "query", "brain");
        let generation = store
            .begin_root_brain_semantic_request("query".into(), options)
            .unwrap();
        assert!(store.install_root_brain_semantic_results(
            generation,
            "query".into(),
            vec![semantic_hit("installed")]
        ));
        assert!(store.finish_root_brain_semantic_request(generation, RootProviderTerminal::Success));
        store.invalidate_root_brain_semantic_freshness();
        assert_eq!(
            store.root_brain_semantic_results().unwrap().1[0].title,
            "installed"
        );
        let replacement = store
            .begin_root_brain_semantic_request("query".into(), options)
            .unwrap();
        assert!(store.finish_root_brain_semantic_request(replacement, RootProviderTerminal::Failed));
        assert_eq!(
            store.root_brain_semantic_results().unwrap().1[0].title,
            "installed"
        );
        assert_eq!(store.root_brain_source_epoch(), 1);
        store.invalidate_root_brain_semantic_freshness();
        let current = store
            .begin_root_brain_semantic_request("query".into(), options)
            .unwrap();
        store.invalidate_root_brain_semantic_freshness();
        assert!(!store.root_brain_semantic_generation_matches(current));
        assert!(store
            .begin_root_brain_semantic_request("query".into(), options)
            .is_none());
        assert!(
            store.finish_root_brain_semantic_request(current, RootProviderTerminal::StaleDiscarded)
        );
    }

    #[test]
    fn brain_inbox_freshness_checks_do_not_fake_reads_or_drop_failed_snapshots() {
        let mut store = RootSearchStore::default();
        commit_brain_query(&mut store, "", "brain");
        let now = std::time::Instant::now();
        let ttl = std::time::Duration::from_secs(30);
        assert!(!store.root_brain_inbox_cache_is_fresh(now, ttl));
        assert!(store.root_brain_inbox_loaded_at.is_none());
        let first = begin_brain_sync_read(&mut store, "brain-inbox");
        assert!(store.install_root_brain_inbox_read(
            first,
            now,
            Ok(vec![inbox_item(1, "good")]),
            true,
            8
        ));
        store.finish_named_provider("brain-inbox", first, RootProviderTerminal::Success);
        assert!(store
            .root_brain_inbox_cache_is_fresh(now + ttl - std::time::Duration::from_nanos(1), ttl));
        assert!(!store.root_brain_inbox_cache_is_fresh(now + ttl, ttl));
        store.invalidate_root_brain_inbox_freshness();
        let failed = begin_brain_sync_read(&mut store, "brain-inbox");
        assert!(!store.install_root_brain_inbox_read(
            failed,
            now + ttl,
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into()),
            true,
            8
        ));
        store.finish_named_provider("brain-inbox", failed, RootProviderTerminal::Failed);
        assert!(store.root_brain_inbox_cache_is_fresh(now + ttl, ttl));
        assert_eq!(store.root_brain_inbox_items()[0].title, "good");
        assert_eq!(store.root_brain_inbox_epoch(), 1);
    }

    #[test]
    fn lexical_source_failure_retains_bounded_rows_but_successful_empty_replaces_them() {
        let mut store = RootSearchStore::default();
        commit_brain_query(&mut store, "query", "brain");
        let first = begin_brain_sync_read(&mut store, "brain-lexical");
        assert!(store.install_root_brain_lexical_results(
            first,
            Ok(vec![semantic_hit("good"), semantic_hit("capped")]),
            1
        ));
        store.finish_named_provider("brain-lexical", first, RootProviderTerminal::Success);
        assert_eq!(store.root_brain_lexical_results().len(), 1);
        store.invalidate_root_brain_lexical_freshness();
        let failed = begin_brain_sync_read(&mut store, "brain-lexical");
        assert!(!store.install_root_brain_lexical_results(
            failed,
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into()),
            1
        ));
        store.finish_named_provider("brain-lexical", failed, RootProviderTerminal::Failed);
        assert_eq!(store.root_brain_lexical_results()[0].title, "good");
        assert_eq!(store.root_brain_source_epoch(), 1);
        let empty = begin_brain_sync_read(&mut store, "brain-lexical");
        assert!(store.install_root_brain_lexical_results(empty, Ok(Vec::new()), 1));
        store.finish_named_provider("brain-lexical", empty, RootProviderTerminal::Empty);
        assert!(store.root_brain_lexical_results().is_empty());
        assert_eq!(store.root_brain_source_epoch(), 2);
    }

    #[test]
    fn brain_query_aba_scope_and_lifetime_cannot_reuse_old_snapshots_or_workers() {
        let mut store = RootSearchStore::default();
        let options = crate::brain::RootBrainSectionOptions::default();
        commit_brain_query(&mut store, "query", "brain");
        let lexical = begin_brain_sync_read(&mut store, "brain-lexical");
        assert!(store.install_root_brain_lexical_results(
            lexical,
            Ok(vec![semantic_hit("lexical")]),
            4
        ));
        store.finish_named_provider("brain-lexical", lexical, RootProviderTerminal::Success);
        let semantic = store
            .begin_root_brain_semantic_request("query".into(), options)
            .unwrap();
        assert!(store.install_root_brain_semantic_results(
            semantic,
            "query".into(),
            vec![semantic_hit("semantic")]
        ));
        store.finish_root_brain_semantic_request(semantic, RootProviderTerminal::Success);
        commit_brain_query(&mut store, "other", "brain");
        commit_brain_query(&mut store, "query", "brain");
        assert!(store.root_brain_lexical_results().is_empty());
        assert!(store.root_brain_semantic_results().is_none());
        assert!(!store.root_brain_semantic_request_matches("query", options));
        let held = store
            .begin_root_brain_semantic_request("query".into(), options)
            .unwrap();
        commit_brain_query(&mut store, "query", "other-scope");
        assert!(!store.root_brain_semantic_generation_matches(held));
        store.retire_query_owner();
        commit_brain_query(&mut store, "query", "other-scope");
        assert!(
            store.finish_root_brain_semantic_request(held, RootProviderTerminal::StaleDiscarded)
        );
        let replacement = store
            .begin_root_brain_semantic_request("query".into(), options)
            .unwrap();
        assert!(!store.install_root_brain_semantic_results(
            held,
            "query".into(),
            vec![semantic_hit("stale")]
        ));
        assert!(
            !store.finish_root_brain_semantic_request(held, RootProviderTerminal::StaleDiscarded)
        );
        assert!(store.root_brain_semantic_generation_matches(replacement));
    }

    #[test]
    fn brain_inbox_identity_epoch_and_removal_follow_ordered_ids() {
        let mut store = RootSearchStore::default();

        assert!(
            store.install_root_brain_inbox_items(vec![inbox_item(1, "one"), inbox_item(2, "two"),])
        );
        assert_eq!(store.root_brain_inbox_epoch(), 1);

        assert!(!store
            .install_root_brain_inbox_items(vec![inbox_item(1, "one"), inbox_item(2, "two"),]));
        assert_eq!(store.root_brain_inbox_epoch(), 1);

        assert!(store.install_root_brain_inbox_items(vec![
            inbox_item(1, "updated title is the same identity"),
            inbox_item(2, "two"),
        ]));
        assert_eq!(
            store.root_brain_inbox_items()[0].title,
            "updated title is the same identity"
        );
        assert_eq!(
            store
                .root_brain_inbox_items()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(store.root_brain_inbox_epoch(), 2);

        assert!(
            store.install_root_brain_inbox_items(vec![inbox_item(2, "two"), inbox_item(1, "one"),])
        );
        assert_eq!(store.root_brain_inbox_epoch(), 3);
        assert!(store.remove_root_brain_inbox_item(1));
        assert_eq!(
            store
                .root_brain_inbox_items()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(store.root_brain_inbox_epoch(), 4);
        assert!(!store.remove_root_brain_inbox_item(99));
        assert_eq!(store.root_brain_inbox_epoch(), 4);
    }

    #[test]
    fn brain_inbox_all_row_content_changes_refresh_once_without_moving_selection() {
        let mut store = RootSearchStore::default();
        assert!(
            store.install_root_brain_inbox_items(vec![inbox_item(1, "one"), inbox_item(2, "two")])
        );

        let changes: [fn(&mut crate::brain::InboxItem); 7] = [
            |item| item.title = "updated title".to_owned(),
            |item| item.detail = "updated detail".to_owned(),
            |item| item.kind = crate::brain::inbox::InboxKind::Commitment,
            |item| item.source = "chat".to_owned(),
            |item| item.source_id = "updated-source".to_owned(),
            |item| item.created_at = 42,
            |item| item.resolved_at = Some(84),
        ];

        for (index, change) in changes.into_iter().enumerate() {
            let mut fresh = store.root_brain_inbox_items().to_vec();
            change(&mut fresh[0]);
            let unchanged = fresh.clone();
            let previous_epoch = store.root_brain_inbox_epoch();

            assert!(
                store.install_root_brain_inbox_items(fresh),
                "content update {index} must replace the existing row"
            );
            assert_eq!(store.root_brain_inbox_epoch(), previous_epoch + 1);
            assert_eq!(
                store
                    .root_brain_inbox_items()
                    .iter()
                    .map(|item| item.id)
                    .collect::<Vec<_>>(),
                vec![1, 2],
                "content update {index} must preserve selected row identity and order"
            );
            assert!(
                !store.install_root_brain_inbox_items(unchanged),
                "identical snapshot after update {index} must remain a no-op"
            );
            assert_eq!(store.root_brain_inbox_epoch(), previous_epoch + 1);
        }
    }

    #[test]
    fn root_windows_refresh_and_focus_lifecycle_stays_cohesive() {
        let mut store = RootSearchStore::with_root_windows(
            &[],
            &[],
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
        );

        let token = store.begin_root_windows_refresh();
        assert!(store.root_windows_refresh_token_matches(token));
        assert!(store.root_windows_refreshing);
        assert_eq!(store.root_windows_refresh_generation, 1);
        assert!(matches!(
            store.root_windows_provider_status,
            crate::window_control::RootWindowsProviderStatus::Refreshing { count: 0 }
        ));

        store.fail_root_windows_refresh(
            crate::window_control::RootWindowsProviderStatus::PermissionRequired,
        );
        assert!(!store.root_windows_refreshing);
        assert_eq!(store.root_windows_refresh_generation, 2);

        let previous_token = token;
        let token = store.begin_root_windows_refresh();
        assert!(!store.root_windows_refresh_token_matches(previous_token));
        assert!(store.root_windows_refresh_token_matches(token));
        store.install_root_windows(&[], &[]);
        assert!(!store.root_windows_refreshing);
        assert_eq!(store.root_windows_refresh_generation, 4);
        assert!(matches!(
            store.root_windows_provider_status,
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 }
        ));
        assert!(store.root_windows_last_completed_at.is_some());

        store.rebuild_root_windows(&[], &[]);
        assert_eq!(store.root_windows_refresh_generation, 5);

        store.record_root_window_focus("window-key".to_string());
        assert_eq!(store.root_window_focus_seq, 1);
        assert_eq!(store.root_window_focus_recency.get("window-key"), Some(&1));
    }

    #[test]
    fn root_windows_enrichment_orders_frontmost_then_local_recency_and_labels_duplicates() {
        fn window(id: u32, title: &str) -> crate::window_control::WindowInfo {
            crate::window_control::WindowInfo::for_test(
                id,
                "Example".to_string(),
                title.to_string(),
                crate::window_control::Bounds::new(0, 0, 800, 600),
                42,
            )
        }

        let duplicate_first = window(1, "Shared");
        let duplicate_recent = window(2, "shared");
        let mut frontmost = window(3, "Frontmost");
        frontmost.is_frontmost_app = true;

        let mut store = RootSearchStore::default();
        store.record_root_window_focus(duplicate_recent.selection_key());
        store.install_root_windows(&[duplicate_first, duplicate_recent, frontmost], &[]);

        let (entries, status) = store.root_windows();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.window.id)
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        assert_eq!(entries[1].duplicate_rank, Some(2));
        assert_eq!(entries[1].duplicate_count, 2);
        assert_eq!(entries[1].local_recency_seq, Some(1));
        assert!(entries[1].subtitle.contains("Window 2 of 2"));
        assert_eq!(entries[2].duplicate_rank, Some(1));
        assert!(matches!(
            status,
            crate::window_control::RootWindowsProviderStatus::Ready { count: 3 }
        ));
    }

    #[test]
    fn passive_frame_cache_reuses_only_the_matching_query_frame() {
        let mut store = RootSearchStore::default();
        let first = passive_frame("first");
        let other = passive_frame("other");

        assert!(store.cached_root_passive_frame(&first.key).is_none());
        let returned = store.cache_root_passive_frame(first.clone());
        assert_eq!(returned.key, first.key);
        assert_eq!(
            store
                .cached_root_passive_frame(&first.key)
                .map(|frame| frame.key),
            Some(first.key)
        );
        assert!(store.cached_root_passive_frame(&other.key).is_none());

        store.clear_root_passive_frame();
        assert!(store.root_passive_frame().is_none());
    }
}
