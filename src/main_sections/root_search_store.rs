/// Root-launcher Windows, file, and Brain search state owned as one coherent async cohort.
pub(crate) struct RootSearchStore {
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
    /// Async hybrid brain hits keyed by the trimmed root-launcher search text.
    root_brain_semantic_results: Option<(String, Vec<crate::brain::RootBrainSearchHit>)>,
    /// Generation counter used to ignore stale semantic brain batches.
    root_brain_search_generation: u64,
    /// Last requested semantic brain search, used to avoid duplicate work.
    root_brain_search_request: Option<(String, crate::brain::RootBrainSectionOptions)>,
    /// Revision folded into the passive-frame key when semantic results change.
    root_brain_semantic_epoch: u64,
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
            root_passive_frame: None,
            cached_root_windows: Vec::new(),
            root_windows_provider_status: crate::window_control::RootWindowsProviderStatus::Unknown,
            root_windows_refresh_generation: 0,
            root_windows_refresh_token: 0,
            root_windows_refreshing: false,
            root_windows_last_completed_at: None,
            root_window_focus_recency: std::collections::HashMap::new(),
            root_window_focus_seq: 0,
            root_brain_semantic_results: None,
            root_brain_search_generation: 0,
            root_brain_search_request: None,
            root_brain_semantic_epoch: 0,
            root_brain_inbox_items: Vec::new(),
            root_brain_inbox_loaded_at: None,
            root_brain_inbox_epoch: 0,
            root_file_results: Vec::new(),
            root_file_result_cache: std::collections::VecDeque::new(),
            root_file_search_mode: None,
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
    pub(crate) fn root_brain_semantic_epoch(&self) -> u64 {
        self.root_brain_semantic_epoch
    }

    pub(crate) fn root_brain_semantic_results(
        &self,
    ) -> Option<&(String, Vec<crate::brain::RootBrainSearchHit>)> {
        self.root_brain_semantic_results.as_ref()
    }

    pub(crate) fn root_brain_semantic_request_matches(
        &self,
        query: &str,
        options: crate::brain::RootBrainSectionOptions,
    ) -> bool {
        self.root_brain_search_request.as_ref().is_some_and(
            |(requested_query, requested_options)| {
                requested_query == query && *requested_options == options
            },
        )
    }

    pub(crate) fn begin_root_brain_semantic_request(
        &mut self,
        query: String,
        options: crate::brain::RootBrainSectionOptions,
    ) -> u64 {
        self.root_brain_search_generation = self.root_brain_search_generation.wrapping_add(1);
        self.root_brain_search_request = Some((query, options));
        self.root_brain_search_generation
    }

    pub(crate) fn root_brain_semantic_generation_matches(&self, generation: u64) -> bool {
        self.root_brain_search_generation == generation
    }

    pub(crate) fn install_root_brain_semantic_results(
        &mut self,
        generation: u64,
        query: String,
        hits: Vec<crate::brain::RootBrainSearchHit>,
    ) -> bool {
        if !self.root_brain_semantic_generation_matches(generation) {
            return false;
        }
        self.root_brain_semantic_results = Some((query, hits));
        self.root_brain_semantic_epoch = self.root_brain_semantic_epoch.wrapping_add(1);
        true
    }

    pub(crate) fn clear_root_brain_semantic(&mut self) -> bool {
        self.root_brain_search_generation = self.root_brain_search_generation.wrapping_add(1);
        self.root_brain_search_request = None;
        let changed = self.root_brain_semantic_results.take().is_some();
        if changed {
            self.root_brain_semantic_epoch = self.root_brain_semantic_epoch.wrapping_add(1);
        }
        changed
    }

    pub(crate) fn root_brain_inbox_epoch(&self) -> u64 {
        self.root_brain_inbox_epoch
    }

    pub(crate) fn root_brain_inbox_items(&self) -> &[crate::brain::InboxItem] {
        &self.root_brain_inbox_items
    }

    pub(crate) fn root_brain_inbox_refresh_if_stale(
        &mut self,
        now: std::time::Instant,
        ttl: std::time::Duration,
    ) -> bool {
        let stale = self
            .root_brain_inbox_loaded_at
            .is_none_or(|loaded_at| now.saturating_duration_since(loaded_at) >= ttl);
        if stale {
            self.root_brain_inbox_loaded_at = Some(now);
        }
        stale
    }

    pub(crate) fn install_root_brain_inbox_items(
        &mut self,
        items: Vec<crate::brain::InboxItem>,
    ) -> bool {
        let ids_match = self.root_brain_inbox_items.len() == items.len()
            && self
                .root_brain_inbox_items
                .iter()
                .zip(&items)
                .all(|(current, fresh)| current.id == fresh.id);
        if ids_match {
            return false;
        }
        self.root_brain_inbox_items = items;
        self.root_brain_inbox_epoch = self.root_brain_inbox_epoch.wrapping_add(1);
        true
    }

    pub(crate) fn remove_root_brain_inbox_item(&mut self, id: i64) -> bool {
        let previous_len = self.root_brain_inbox_items.len();
        self.root_brain_inbox_items.retain(|item| item.id != id);
        let changed = self.root_brain_inbox_items.len() != previous_len;
        if changed {
            self.root_brain_inbox_epoch = self.root_brain_inbox_epoch.wrapping_add(1);
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
        self.root_windows_last_completed_at = Some(std::time::Instant::now());
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
            .map(|completed_at| completed_at.elapsed() >= std::time::Duration::from_secs(3))
            .unwrap_or(true);
        !self.root_windows_refreshing && (self.cached_root_windows.is_empty() || stale)
    }

    pub(crate) fn begin_root_windows_refresh(&mut self) -> u64 {
        self.root_windows_refreshing = true;
        self.root_windows_refresh_token = self.root_windows_refresh_token.wrapping_add(1);
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
                brain_semantic_epoch: 0,
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
        assert_eq!(store.root_brain_semantic_epoch, 0);
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
    fn stale_semantic_batches_are_rejected_and_current_batches_install() {
        let mut store = RootSearchStore::default();
        let options = crate::brain::RootBrainSectionOptions::default();
        let stale_generation = store.begin_root_brain_semantic_request("old".to_string(), options);
        let current_generation =
            store.begin_root_brain_semantic_request("current".to_string(), options);

        assert!(!store.install_root_brain_semantic_results(
            stale_generation,
            "old".to_string(),
            vec![semantic_hit("stale")],
        ));
        assert!(store.root_brain_semantic_results().is_none());
        assert_eq!(store.root_brain_semantic_epoch(), 0);

        assert!(store.install_root_brain_semantic_results(
            current_generation,
            "current".to_string(),
            vec![semantic_hit("installed")],
        ));
        let (query, hits) = store
            .root_brain_semantic_results()
            .expect("current semantic batch should install");
        assert_eq!(query, "current");
        assert_eq!(hits[0].title, "installed");
        assert_eq!(store.root_brain_semantic_epoch(), 1);
    }

    #[test]
    fn clearing_semantic_state_invalidates_generations() {
        let mut store = RootSearchStore::default();
        let options = crate::brain::RootBrainSectionOptions::default();
        let generation = store.begin_root_brain_semantic_request("query".to_string(), options);
        assert!(store.install_root_brain_semantic_results(
            generation,
            "query".to_string(),
            vec![semantic_hit("installed")],
        ));

        assert!(store.clear_root_brain_semantic());
        assert!(!store.root_brain_semantic_generation_matches(generation));
        assert!(store.root_brain_semantic_results().is_none());
        assert_eq!(store.root_brain_semantic_epoch(), 2);
        assert!(!store.clear_root_brain_semantic());
        assert!(!store.root_brain_semantic_generation_matches(generation));
        assert_eq!(store.root_brain_semantic_epoch(), 2);
    }

    #[test]
    fn brain_inbox_refresh_ttl_is_deterministic() {
        let mut store = RootSearchStore::default();
        let now = std::time::Instant::now();
        let ttl = std::time::Duration::from_secs(30);

        assert!(store.root_brain_inbox_refresh_if_stale(now, ttl));
        assert!(!store.root_brain_inbox_refresh_if_stale(
            now + ttl - std::time::Duration::from_nanos(1),
            ttl,
        ));
        assert!(store.root_brain_inbox_refresh_if_stale(now + ttl, ttl));
    }

    #[test]
    fn brain_inbox_identity_epoch_and_removal_follow_ordered_ids() {
        let mut store = RootSearchStore::default();

        assert!(
            store.install_root_brain_inbox_items(vec![inbox_item(1, "one"), inbox_item(2, "two"),])
        );
        assert_eq!(store.root_brain_inbox_epoch(), 1);

        assert!(!store.install_root_brain_inbox_items(vec![
            inbox_item(1, "updated title is the same identity"),
            inbox_item(2, "two"),
        ]));
        assert_eq!(store.root_brain_inbox_items()[0].title, "one");
        assert_eq!(store.root_brain_inbox_epoch(), 1);

        assert!(
            store.install_root_brain_inbox_items(vec![inbox_item(2, "two"), inbox_item(1, "one"),])
        );
        assert_eq!(store.root_brain_inbox_epoch(), 2);
        assert!(store.remove_root_brain_inbox_item(1));
        assert_eq!(
            store
                .root_brain_inbox_items()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(store.root_brain_inbox_epoch(), 3);
        assert!(!store.remove_root_brain_inbox_item(99));
        assert_eq!(store.root_brain_inbox_epoch(), 3);
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
