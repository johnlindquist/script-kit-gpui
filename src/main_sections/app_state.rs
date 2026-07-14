#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FileSearchSelectionMode {
    AutoFirst,
    UserLockedPath,
}

/// Render and filter-performance diagnostics owned by the main script list surface.
#[derive(Debug)]
struct MainMenuRenderDiagnosticsState {
    /// Last filter value that produced render diagnostics.
    last_render_log_filter: String,
    /// Last selection index that produced render diagnostics.
    last_render_log_selection: usize,
    /// Last item count that produced render diagnostics.
    last_render_log_item_count: usize,
    /// True when the current render changed enough to log preview diagnostics.
    log_this_render: bool,
    /// Start time for the current input-to-grouped-results performance sample.
    filter_perf_start: Option<std::time::Instant>,
    /// Cache fields for highlighting
    last_input_highlight_text: String,
    last_input_highlight_ranges: Vec<(std::ops::Range<usize>, gpui::Hsla, String)>,
}

#[derive(Clone, Debug)]
struct SubmitDiagnosticEvent {
    generation: u64,
    owner: &'static str,
    route: &'static str,
    surface: String,
    prompt_id: Option<String>,
    value: Option<String>,
    selected_index: Option<usize>,
    consumed_enter: bool,
}

#[derive(Debug, Default)]
struct SubmitDiagnosticsState {
    generation: u64,
    last: Option<SubmitDiagnosticEvent>,
    pending_enter_consumed_at: Option<std::time::Instant>,
}

impl Default for MainMenuRenderDiagnosticsState {
    fn default() -> Self {
        Self {
            last_render_log_filter: String::new(),
            last_render_log_selection: usize::MAX,
            last_render_log_item_count: usize::MAX,
            log_this_render: true,
            filter_perf_start: None,
            last_input_highlight_text: String::new(),
            last_input_highlight_ranges: Vec::new(),
        }
    }
}

/// Fallback commands shown when the main-menu search has no direct matches.
#[derive(Default)]
struct MainMenuFallbackState {
    active: bool,
    selected_index: usize,
    cached_items: Vec<crate::fallbacks::FallbackItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RootPassiveFrameKey {
    pub(crate) query: String,
    pub(crate) advanced_query: bool,
    pub(crate) source_filters: crate::menu_syntax::RootUnifiedSourceFilterSet,
    pub(crate) todo_options: crate::menu_syntax::RootTodoSectionOptions,
    pub(crate) brain_options: crate::brain::RootBrainSectionOptions,
    /// Bumped whenever async semantic brain results change, so a cached frame
    /// holding lexical-only brain hits can never be served after semantic
    /// results land for the same query.
    pub(crate) brain_semantic_epoch: u64,
    pub(crate) notes_options: crate::notes::RootNotesSectionOptions,
    pub(crate) clipboard_history_options:
        crate::clipboard_history::RootClipboardHistorySectionOptions,
    pub(crate) dictation_history_options: crate::dictation::RootDictationHistorySectionOptions,
    pub(crate) agent_chat_history_options:
        crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions,
    pub(crate) ai_vault_options: crate::ai_vault::RootAiVaultSectionOptions,
    pub(crate) ai_vault_snapshot_generation: u64,
    pub(crate) browser_tabs_options: crate::browser_tabs::RootBrowserTabsSectionOptions,
    pub(crate) browser_tabs_snapshot_generation: u64,
    pub(crate) browser_history_options: crate::browser_history::RootBrowserHistorySectionOptions,
    pub(crate) browser_history_snapshot_generation: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct RootPassiveFrame {
    pub(crate) key: RootPassiveFrameKey,
    pub(crate) note_hits: Vec<crate::notes::RootNoteSearchHit>,
    pub(crate) brain_hits: Vec<crate::brain::RootBrainSearchHit>,
    pub(crate) todo_hits: Vec<crate::menu_syntax::RootTodoSearchHit>,
    pub(crate) clipboard_history_hits: Vec<crate::clipboard_history::ClipboardEntryMeta>,
    pub(crate) dictation_history_hits: Vec<crate::dictation::RootDictationHistorySearchHit>,
    pub(crate) agent_chat_history_hits:
        Vec<crate::ai::agent_chat::ui::history::AgentChatHistorySearchHit>,
    pub(crate) ai_vault_hits: Vec<crate::ai_vault::AiVaultHit>,
    pub(crate) browser_tab_hits: Vec<crate::browser_tabs::RootBrowserTabSearchHit>,
    pub(crate) browser_history_hits: Vec<crate::browser_history::RootBrowserHistorySearchHit>,
    pub(crate) ai_vault_snapshot_generation: u64,
    pub(crate) browser_tabs_snapshot_generation: u64,
    pub(crate) browser_history_snapshot_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RootFileFrameKey {
    pub(crate) query: String,
    pub(crate) advanced_query: bool,
    pub(crate) source_filters: crate::menu_syntax::RootUnifiedSourceFilterSet,
    pub(crate) mode: Option<crate::file_search::RootFileSectionMode>,
    pub(crate) options: crate::file_search::RootFileSectionOptions,
    pub(crate) search_generation: u64,
    pub(crate) recent_file_revision: u64,
    pub(crate) visible_loading: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct RootFileFrame {
    pub(crate) key: RootFileFrameKey,
    pub(crate) mode: Option<crate::file_search::RootFileSectionMode>,
    pub(crate) visible_loading: bool,
    pub(crate) file_results: Vec<crate::file_search::FileResult>,
    pub(crate) recent_file_results: Vec<crate::file_search::FileResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MiniAiCloseSource {
    Escape,
    Actions,
    ModeToggle,
    Hide,
}

impl MiniAiCloseSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Escape => "escape",
            Self::Actions => "actions",
            Self::ModeToggle => "mode_toggle",
            Self::Hide => "hide",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MiniAiCloseSnapshot {
    pub(crate) prompt_id: String,
    pub(crate) main_window_mode: MainWindowMode,
    pub(crate) source: MiniAiCloseSource,
    pub(crate) draft_len: usize,
    pub(crate) pending_submit: bool,
    pub(crate) handoff_source: Option<String>,
    pub(crate) return_origin: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) enum MiniAiUiRequest {
    ToggleActions {
        prompt_id: String,
        source: &'static str,
    },
}

impl MainMenuFallbackState {
    fn is_active(&self) -> bool {
        self.active && !self.cached_items.is_empty()
    }

    fn clear(&mut self) {
        self.active = false;
        self.selected_index = 0;
        self.cached_items.clear();
    }

    fn replace_items(&mut self, items: Vec<crate::fallbacks::FallbackItem>) {
        self.active = !items.is_empty();
        self.selected_index = 0;
        self.cached_items = items;
    }

    fn selected_item(&self) -> Option<&crate::fallbacks::FallbackItem> {
        if !self.is_active() {
            return None;
        }
        self.cached_items.get(self.selected_index)
    }

    fn move_up(&mut self) -> bool {
        if !self.is_active() || self.selected_index == 0 {
            return false;
        }
        self.selected_index -= 1;
        true
    }

    fn move_down(&mut self) -> bool {
        if !self.is_active() {
            return false;
        }
        if self.selected_index >= self.cached_items.len().saturating_sub(1) {
            return false;
        }
        self.selected_index += 1;
        true
    }
}

const MAIN_MENU_RESULT_CACHE_UNINITIALIZED_KEY: &str = "\0_UNINITIALIZED_\0";
const MAIN_MENU_RESULT_CACHE_INVALIDATED_KEY: &str = "\0_INVALIDATED_\0";
const MAIN_MENU_RESULT_CACHE_APPS_LOADED_KEY: &str = "\0_APPS_LOADED_\0";
const QUICK_TERMINAL_INITIAL_COLS: u16 = 80;
const QUICK_TERMINAL_INITIAL_ROWS: u16 = 24;
const QUICK_TERMINAL_WARM_TTL: std::time::Duration = std::time::Duration::from_secs(600);

/// Search and grouped-result caches owned by the main script-list surface.
struct MainMenuResultCacheState {
    cached_filtered_results: Vec<scripts::SearchResult>,
    filter_cache_key: String,
    cached_grouped_items: Arc<[GroupedListItem]>,
    cached_grouped_flat_results: Arc<[scripts::SearchResult]>,
    cached_grouped_source_statuses: Arc<[crate::list_item::SourceChipStatusRow]>,
    cached_grouped_first_selectable_index: Option<usize>,
    cached_grouped_last_selectable_index: Option<usize>,
    grouped_cache_key: String,
}

impl Default for MainMenuResultCacheState {
    fn default() -> Self {
        Self {
            cached_filtered_results: Vec::new(),
            filter_cache_key: String::from(MAIN_MENU_RESULT_CACHE_UNINITIALIZED_KEY),
            cached_grouped_items: Arc::from([]),
            cached_grouped_flat_results: Arc::from([]),
            cached_grouped_source_statuses: Arc::from([]),
            cached_grouped_first_selectable_index: None,
            cached_grouped_last_selectable_index: None,
            grouped_cache_key: String::from(MAIN_MENU_RESULT_CACHE_UNINITIALIZED_KEY),
        }
    }
}

impl MainMenuResultCacheState {
    fn has_filtered_results_for(&self, filter_text: &str) -> bool {
        self.filter_cache_key == filter_text
    }

    fn filtered_cache_key(&self) -> &str {
        &self.filter_cache_key
    }

    fn clone_filtered_results(&self) -> Vec<scripts::SearchResult> {
        self.cached_filtered_results.clone()
    }

    fn filtered_results(&self) -> &Vec<scripts::SearchResult> {
        &self.cached_filtered_results
    }

    fn store_filtered_results(&mut self, filter_text: String, results: Vec<scripts::SearchResult>) {
        self.cached_filtered_results = results;
        self.filter_cache_key = filter_text;
    }

    fn has_grouped_results_for(&self, computed_filter_text: &str) -> bool {
        self.grouped_cache_key == computed_filter_text
    }

    fn has_grouped_results_for_filter_text(&self, computed_filter_text: &str) -> bool {
        self.grouped_cache_key == computed_filter_text
            || self
                .grouped_cache_key
                .strip_prefix(computed_filter_text)
                .is_some_and(|suffix| suffix.starts_with('\x1F'))
    }

    fn grouped_cache_key(&self) -> &str {
        &self.grouped_cache_key
    }

    fn clone_grouped_results(&self) -> (Arc<[GroupedListItem]>, Arc<[scripts::SearchResult]>) {
        (
            self.cached_grouped_items.clone(),
            self.cached_grouped_flat_results.clone(),
        )
    }

    fn grouped_items(&self) -> &[GroupedListItem] {
        &self.cached_grouped_items
    }

    fn grouped_flat_results(&self) -> &[scripts::SearchResult] {
        &self.cached_grouped_flat_results
    }

    fn grouped_source_statuses(&self) -> &[crate::list_item::SourceChipStatusRow] {
        &self.cached_grouped_source_statuses
    }

    fn grouped_flat_result_count(&self) -> usize {
        self.cached_grouped_flat_results.len()
    }

    fn flat_result_index_for_grouped_item(&self, grouped_index: usize) -> Option<usize> {
        match self.cached_grouped_items.get(grouped_index) {
            Some(GroupedListItem::Item(result_idx)) => Some(*result_idx),
            _ => None,
        }
    }

    fn flat_result_index_for_coerced_grouped_selection(
        &self,
        selected_index: usize,
    ) -> Option<(usize, usize)> {
        let grouped_index =
            crate::list_item::coerce_selection(self.grouped_items(), selected_index)?;
        let result_idx = self.flat_result_index_for_grouped_item(grouped_index)?;
        Some((grouped_index, result_idx))
    }

    fn search_result_for_flat_index(
        &self,
        flat_result_index: usize,
    ) -> Option<&scripts::SearchResult> {
        self.cached_grouped_flat_results.get(flat_result_index)
    }

    fn cloned_search_result_for_flat_index(
        &self,
        flat_result_index: usize,
    ) -> Option<scripts::SearchResult> {
        self.search_result_for_flat_index(flat_result_index)
            .cloned()
    }

    fn search_result_for_grouped_item(
        &self,
        grouped_index: usize,
    ) -> Option<&scripts::SearchResult> {
        let result_idx = self.flat_result_index_for_grouped_item(grouped_index)?;
        self.search_result_for_flat_index(result_idx)
    }

    fn cloned_search_result_for_grouped_item(
        &self,
        grouped_index: usize,
    ) -> Option<scripts::SearchResult> {
        self.search_result_for_grouped_item(grouped_index).cloned()
    }

    fn first_search_result_at_or_after_grouped_item(
        &self,
        grouped_index: usize,
    ) -> Option<&scripts::SearchResult> {
        (grouped_index..self.cached_grouped_items.len())
            .find_map(|index| self.search_result_for_grouped_item(index))
    }

    fn cloned_first_search_result_at_or_after_grouped_item(
        &self,
        grouped_index: usize,
    ) -> Option<scripts::SearchResult> {
        self.first_search_result_at_or_after_grouped_item(grouped_index)
            .cloned()
    }

    fn grouped_search_results(&self) -> impl Iterator<Item = &scripts::SearchResult> {
        self.cached_grouped_items
            .iter()
            .filter_map(|item| match item {
                GroupedListItem::Item(result_idx) => self.search_result_for_flat_index(*result_idx),
                GroupedListItem::SectionHeader(..) | GroupedListItem::Status(..) => None,
            })
    }

    fn is_selectable_result(result: &scripts::SearchResult) -> bool {
        !matches!(
            result,
            scripts::SearchResult::SpineProjection(row) if !row.is_selectable
        )
    }

    fn grouped_selectable_search_results(&self) -> impl Iterator<Item = &scripts::SearchResult> {
        self.grouped_search_results()
            .filter(|result| Self::is_selectable_result(result))
    }

    fn grouped_selectable_result_count(&self) -> usize {
        self.grouped_selectable_search_results().count()
    }

    fn grouped_index_for_stable_selection_key(&self, key: &str) -> Option<usize> {
        self.cached_grouped_items
            .iter()
            .enumerate()
            .find_map(|(grouped_index, item)| {
                let GroupedListItem::Item(result_idx) = item else {
                    return None;
                };
                self.cached_grouped_flat_results
                    .get(*result_idx)
                    .and_then(|result| result.stable_selection_key())
                    .filter(|candidate| candidate == key)
                    .map(|_| grouped_index)
            })
    }

    fn selectable_bounds(&self) -> (Option<usize>, Option<usize>) {
        (
            self.cached_grouped_first_selectable_index,
            self.cached_grouped_last_selectable_index,
        )
    }

    fn first_selectable_index(&self) -> Option<usize> {
        self.cached_grouped_first_selectable_index
    }

    fn last_selectable_index(&self) -> Option<usize> {
        self.cached_grouped_last_selectable_index
    }

    fn has_selectable_grouped_item(&self) -> bool {
        self.cached_grouped_first_selectable_index.is_some()
    }

    fn store_grouped_results(
        &mut self,
        computed_filter_text: String,
        grouped_items: Vec<GroupedListItem>,
        flat_results: Vec<scripts::SearchResult>,
        _first_selectable_index: Option<usize>,
        _last_selectable_index: Option<usize>,
    ) {
        let mut display_items = Vec::with_capacity(grouped_items.len());
        let mut source_statuses = Vec::new();
        for item in grouped_items {
            match item {
                GroupedListItem::Status(status) => source_statuses.push(status),
                GroupedListItem::SectionHeader(..) | GroupedListItem::Item(_) => {
                    display_items.push(item)
                }
            }
        }

        let mut first_selectable_index = None;
        let mut last_selectable_index = None;
        for (index, grouped_item) in display_items.iter().enumerate() {
            let GroupedListItem::Item(result_idx) = grouped_item else {
                continue;
            };
            let Some(result) = flat_results.get(*result_idx) else {
                continue;
            };
            if !Self::is_selectable_result(result) {
                continue;
            }
            if first_selectable_index.is_none() {
                first_selectable_index = Some(index);
            }
            last_selectable_index = Some(index);
        }

        self.cached_grouped_first_selectable_index = first_selectable_index;
        self.cached_grouped_last_selectable_index = last_selectable_index;
        self.cached_grouped_items = display_items.into();
        self.cached_grouped_flat_results = flat_results.into();
        self.cached_grouped_source_statuses = source_statuses.into();
        self.grouped_cache_key = computed_filter_text;
    }

    fn mark_apps_loaded(&mut self) {
        self.filter_cache_key = String::from(MAIN_MENU_RESULT_CACHE_APPS_LOADED_KEY);
        self.grouped_cache_key = String::from(MAIN_MENU_RESULT_CACHE_APPS_LOADED_KEY);
    }

    fn invalidate_filtered_results(&mut self) {
        self.filter_cache_key = String::from(MAIN_MENU_RESULT_CACHE_INVALIDATED_KEY);
    }

    fn invalidate_grouped_results(&mut self) {
        self.cached_grouped_first_selectable_index = None;
        self.cached_grouped_last_selectable_index = None;
        self.grouped_cache_key = String::from(MAIN_MENU_RESULT_CACHE_INVALIDATED_KEY);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MainMenuSelectionSnapshot {
    query: String,
    selected_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainMenuRefreshSelectionPolicy {
    SnapToFirst,
    RestoreIdentity,
}

pub(crate) fn main_menu_refresh_selection_policy(
    user_moved_selection: bool,
) -> MainMenuRefreshSelectionPolicy {
    if user_moved_selection {
        MainMenuRefreshSelectionPolicy::RestoreIdentity
    } else {
        MainMenuRefreshSelectionPolicy::SnapToFirst
    }
}

impl ScriptListApp {
    pub(crate) fn mark_main_menu_selection_user_moved(&mut self) {
        if matches!(self.current_view, AppView::ScriptList) {
            self.main_menu_selection_user_moved = true;
        }
    }

    pub(crate) fn reset_main_menu_selection_user_moved(&mut self) {
        self.main_menu_selection_user_moved = false;
    }

    pub(crate) fn snap_main_menu_selection_to_first(&mut self) -> bool {
        self.get_grouped_results_cached();
        let first = self
            .main_menu_result_caches
            .first_selectable_index()
            .unwrap_or(0);
        let changed = self.selected_index != first;
        self.selected_index = first;
        self.hovered_index = None;
        self.last_scrolled_index = None;
        changed
    }

    pub(crate) fn root_file_source_chip_page_key_for(
        raw_filter_text: &str,
        stripped_query: &str,
        advanced_predicate_active: bool,
        mode: Option<crate::file_search::RootFileSectionMode>,
    ) -> String {
        format!(
            "files|raw={}|query={}|pred={}|mode={:?}",
            raw_filter_text, stripped_query, advanced_predicate_active, mode
        )
    }

    pub(crate) fn root_file_source_chip_visible_limit_for(
        &mut self,
        raw_filter_text: &str,
        stripped_query: &str,
        advanced_predicate_active: bool,
        mode: Option<crate::file_search::RootFileSectionMode>,
    ) -> usize {
        let key = Self::root_file_source_chip_page_key_for(
            raw_filter_text,
            stripped_query,
            advanced_predicate_active,
            mode,
        );
        if self.root_search.root_file_source_chip_page_key.as_deref() != Some(key.as_str()) {
            self.root_search.root_file_source_chip_page_key = Some(key);
            self.root_search.root_file_source_chip_visible_limit =
                crate::file_search::ROOT_FILE_SOURCE_CHIP_INITIAL_VISIBLE_ROWS;
        }

        self.root_search
            .root_file_source_chip_visible_limit
            .max(crate::file_search::ROOT_FILE_SOURCE_CHIP_INITIAL_VISIBLE_ROWS)
    }

    pub(crate) fn main_menu_selection_snapshot(&mut self) -> MainMenuSelectionSnapshot {
        self.get_grouped_results_cached();
        let selected_key = self
            .main_menu_result_caches
            .flat_result_index_for_coerced_grouped_selection(self.selected_index)
            .and_then(|(_, result_idx)| {
                self.main_menu_result_caches
                    .search_result_for_flat_index(result_idx)
            })
            .and_then(|result| result.stable_selection_key());

        MainMenuSelectionSnapshot {
            query: self.computed_filter_text.clone(),
            selected_key,
        }
    }

    pub(crate) fn restore_main_menu_selection_from_snapshot(
        &mut self,
        snapshot: MainMenuSelectionSnapshot,
    ) -> bool {
        if snapshot.query != self.computed_filter_text {
            return false;
        }
        let Some(selected_key) = snapshot.selected_key else {
            return false;
        };

        self.get_grouped_results_cached();
        let Some(grouped_index) = self
            .main_menu_result_caches
            .grouped_index_for_stable_selection_key(&selected_key)
        else {
            return false;
        };

        if self.selected_index == grouped_index {
            return false;
        }

        self.selected_index = grouped_index;
        self.hovered_index = None;
        self.last_scrolled_index = None;
        true
    }

}

#[cfg(test)]
struct MainMenuSelectionTestHost;

#[cfg(test)]
impl gpui::Render for MainMenuSelectionTestHost {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
    }
}

#[cfg(test)]
fn main_menu_selection_test_app(
    cx: &mut gpui::TestAppContext,
) -> gpui::Entity<ScriptListApp> {
    use gpui::AppContext as _;

    let flow_cwd = crate::flows::resolve_flow_cwd(
        crate::config::load_user_preferences()
            .ai
            .cwd
            .filter(|cwd| std::path::Path::new(cwd).is_dir()),
    );
    let catalog = crate::flows::catalog::flow_catalog();
    catalog.set_notify_hook(|| {});
    while catalog.roster_for(&flow_cwd).status == crate::flows::catalog::RosterStatus::Loading {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let app = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app_for_window = app.clone();
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.open_window(Default::default(), |window, cx| {
            let entity = cx.new(|cx| {
                ScriptListApp::new(crate::config::Config::default(), false, window, cx)
            });
            *app_for_window.lock().expect("lock selection test app") = Some(entity);
            cx.new(|_| MainMenuSelectionTestHost)
        })
        .expect("open main-menu selection test window");
    });
    let entity = app
        .lock()
        .expect("lock initialized selection test app")
        .take()
        .expect("selection test app initialized");
    entity
}

#[cfg(test)]
fn main_menu_selection_test_script(name: &str) -> std::sync::Arc<crate::scripts::Script> {
    std::sync::Arc::new(crate::scripts::Script {
        name: name.to_string(),
        path: std::path::PathBuf::from(format!("/tmp/{name}.ts")),
        extension: "ts".to_string(),
        plugin_id: "main".to_string(),
        ..Default::default()
    })
}

#[cfg(test)]
fn selected_main_menu_stable_key(app: &mut ScriptListApp) -> Option<String> {
    app.get_grouped_results_cached();
    app.main_menu_result_caches
        .flat_result_index_for_coerced_grouped_selection(app.selected_index)
        .and_then(|(_, result_index)| {
            app.main_menu_result_caches
                .search_result_for_flat_index(result_index)
        })
        .and_then(crate::scripts::SearchResult::stable_selection_key)
}

#[cfg(test)]
fn first_selectable_main_menu_stable_key(app: &mut ScriptListApp) -> Option<String> {
    app.get_grouped_results_cached();
    let first = app.main_menu_result_caches.first_selectable_index()?;
    app.main_menu_result_caches
        .search_result_for_grouped_item(first)
        .and_then(crate::scripts::SearchResult::stable_selection_key)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReturnToScriptListKeyGuardSource {
    ProfileSearch,
}

#[derive(Debug, Clone)]
pub(crate) struct ReturnToScriptListKeyGuard {
    pub(crate) key: &'static str,
    pub(crate) source: ReturnToScriptListKeyGuardSource,
    pub(crate) reason: &'static str,
    pub(crate) armed_at: std::time::Instant,
    pub(crate) consumed_count: u8,
}

pub(crate) struct ScriptListApp {
    /// H1 Optimization: Arc-wrapped scripts for cheap cloning during filter operations
    scripts: Vec<std::sync::Arc<scripts::Script>>,
    /// H1 Optimization: Arc-wrapped scriptlets for cheap cloning during filter operations
    scriptlets: Vec<std::sync::Arc<scripts::Scriptlet>>,
    /// Plugin-owned skills for main-menu search and Agent Chat skill launch
    skills: Vec<std::sync::Arc<crate::plugins::PluginSkill>>,
    /// Latest validation report describing scripts that were excluded from the
    /// catalog (e.g., binding collisions). Used to surface the launcher
    /// "Script Issues" row and feed the diagnostic view.
    script_validation_report: Option<std::sync::Arc<crate::scripts::ValidationReport>>,
    builtin_entries: Vec<builtins::BuiltInEntry>,
    /// Cached list of installed applications for main search and AppLauncherView
    apps: Vec<app_launcher::AppInfo>,
    /// P0 FIX: Cached clipboard entries for ClipboardHistoryView (avoids cloning per frame)
    cached_clipboard_entries: Vec<clipboard_history::ClipboardEntryMeta>,
    /// Sequential paste state machine (Raycast-style paste-one-at-a-time)
    #[allow(dead_code)]
    paste_sequential_state: Option<clipboard_history::PasteSequentialState>,
    /// Focused clipboard entry ID for action handling in ClipboardHistoryView
    #[allow(dead_code)]
    focused_clipboard_entry_id: Option<String>,
    /// P0 FIX: Cached windows for WindowSwitcherView (avoids cloning per frame)
    cached_windows: Vec<window_control::WindowInfo>,
    /// Cached browser tabs for BrowserTabsView (avoids repeated AppleScript calls while open)
    cached_browser_tabs: Vec<browser_tabs::BrowserTabInfo>,
    /// Cached browser history entries for BrowserHistoryView.
    cached_browser_history: Vec<browser_history::BrowserHistoryEntry>,
    /// True while browser history is loading for the portal.
    browser_history_loading: bool,
    /// Cached file results for FileSearchView (avoids cloning per frame)
    cached_file_results: Vec<file_search::FileResult>,
    /// Cohesive async state for root-launcher file search and visible paging.
    root_search: RootSearchStore,
    // ── Spine @file: subsearch async state ─────────────────────────
    spine_file_search_query: String,
    pub(crate) spine_file_search_generation: u64,
    pub(crate) spine_file_search_loading: bool,
    pub(crate) spine_file_search_results: Vec<file_search::FileResult>,
    spine_file_search_cancel: Option<file_search::CancelToken>,
    /// Which empty `@source:` colon mode the user has explicitly armed with
    /// Down/click. While the active subsearch has an empty sub-query and this
    /// does NOT name its source, the recents list renders with no selected
    /// row and Enter is consumed, so a reflexive double-Enter can never
    /// attach an unseen recent item. Cleared whenever the empty colon-mode
    /// context changes (typing, deleting, switching sources, exiting).
    pub(crate) spine_empty_subsearch_armed_for:
        Option<crate::spine::catalog_subsearch::ContextSubsearchSource>,

    /// File row captured when opening the root-file actions palette.
    pending_root_file_actions_file: Option<file_search::FileResult>,
    /// Root unified-search row captured when opening the MainList actions palette.
    pending_root_unified_actions_subject:
        Option<crate::root_unified_result_actions::RootUnifiedActionSubject>,
    /// Cached process list for ProcessManagerView (avoids cloning per frame)
    cached_processes: Vec<crate::process_manager::ProcessInfo>,
    /// Background refresh task for ProcessManagerView (dropped on view change)
    process_manager_refresh_task: Option<gpui::Task<()>>,
    /// Cached menu bar entries for CurrentAppCommandsView (populated on open)
    cached_current_app_entries: Vec<builtins::BuiltInEntry>,
    /// Session metadata for CurrentAppCommandsView, including the source app identity.
    current_app_commands_session:
        Option<crate::menu_bar::current_app_commands::CurrentAppCommandsSession>,
    selected_index: usize,
    /// True after the user deliberately moves the launcher selection for the
    /// current filter. Async provider refreshes preserve row identity only in
    /// that case; untouched auto-selection stays pinned to the first row.
    main_menu_selection_user_moved: bool,
    /// Main menu filter text (mirrors gpui-component input state)
    filter_text: String,
    /// Inline calculator result derived from filter_text when expression is math-like
    pub inline_calculator: Option<crate::calculator::CalculatorInlineResult>,
    /// gpui-component input state for the main filter
    gpui_input_state: Entity<InputState>,
    gpui_input_focused: bool,
    pub(crate) ghost_prediction: Option<crate::scripts::search::ghost::GhostPrediction>,
    pub(crate) prediction_revision: crate::scripts::search::ghost::PredictionRevision,
    /// Monotonic generation guarding the debounced LLM ghost prediction. Bumped
    /// on every keystroke / cancel so stale background responses are discarded.
    ghost_llm_generation: u64,
    /// Cancel flag for the in-flight LLM ghost request (best-effort: set before
    /// dispatch; stale responses are always discarded by generation compare).
    ghost_llm_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// LRU cache of LLM ghost predictions keyed by (query, cwd, context_rev, model).
    ghost_llm_cache: std::collections::VecDeque<(
        crate::scripts::search::ghost::GhostLlmCacheKey,
        crate::scripts::search::ghost::GhostLlmCacheEntry,
    )>,
    pub(crate) launcher_context: crate::context_snapshot::launcher_context::LauncherContextSnapshot,
    launcher_context_generation: u64,
    #[allow(dead_code)]
    gpui_input_subscriptions: Vec<Subscription>,
    /// Subscription for window bounds changes (saves position on drag)
    #[allow(dead_code)]
    bounds_subscription: Option<Subscription>,
    /// Subscription for window appearance changes (light/dark mode)
    #[allow(dead_code)]
    appearance_subscription: Option<Subscription>,
    /// Suppress handling of programmatic InputEvent::Change updates.
    suppress_filter_events: bool,
    /// Programmatic filter value whose delayed InputEvent::Change echo should be ignored.
    pending_programmatic_filter_echo: Option<String>,
    /// Sync gpui input text on next render when window access is available.
    pending_filter_sync: bool,
    /// History recall filter that must render before another key-repeat recall is accepted.
    history_filter_render_pending: Option<String>,
    /// Plain Enter consumed by a child filterable surface while it transitions
    /// back to ScriptList. Prevents the same physical keydown from launching
    /// the highlighted main-menu row after the view reset.
    return_to_script_list_key_guard: Option<ReturnToScriptListKeyGuard>,
    /// Pending placeholder text to set on next render (needs Window access).
    pending_placeholder: Option<String>,
    last_output: Option<SharedString>,
    focus_handle: FocusHandle,
    show_logs: bool,
    /// Whether the focused-info panel is visible (toggled via Cmd+I / "Show Info" action)
    show_info_panel: bool,
    /// Single warm PTY reserved for the next launcher Quick Terminal open.
    quick_terminal_warm_pty: Option<crate::terminal::TerminalHandle>,
    /// True while the one-slot Quick Terminal warm PTY pool is being refilled.
    quick_terminal_warm_inflight: bool,
    /// Creation timestamp for TTL validation before consuming the warm PTY.
    quick_terminal_warm_created_at: Option<std::time::Instant>,
    /// Theme wrapped in Arc for cheap cloning when passing to prompts/dialogs
    theme: std::sync::Arc<theme::Theme>,
    #[allow(dead_code)]
    config: config::Config,
    // Scroll activity tracking for scrollbar fade
    /// Current animated visibility for scrollbar fade (0.0 invisible .. 1.0 visible)
    scrollbar_visibility: crate::transitions::Opacity,
    /// Generation counter for cancelling stale fade-out tasks when new scroll activity occurs
    scrollbar_fade_gen: u64,
    /// Timestamp of last scroll activity (for fade-out timer)
    last_scroll_time: Option<std::time::Instant>,
    /// Selected index most recently chosen by a selection-owned builtin wheel scroll.
    builtin_wheel_owned_selected_index: Option<usize>,
    // Interactive script state
    current_view: AppView,
    last_logged_app_view_variant: Option<&'static str>,
    submit_diagnostics: SubmitDiagnosticsState,
    pub(crate) main_window_mode: MainWindowMode,
    script_session: SharedSession,
    // Prompt-specific state (used when view is ArgPrompt or DivPrompt)
    // Uses TextInputState for selection and clipboard support
    arg_input: TextInputState,
    arg_selected_index: usize,
    // Channel for receiving prompt messages from script thread (async_channel for event-driven)
    prompt_receiver: Option<async_channel::Receiver<PromptMessage>>,
    // Channel for sending responses back to script
    // FIX: Use SyncSender (bounded channel) to prevent OOM from slow scripts
    response_sender: Option<mpsc::SyncSender<Message>>,
    // Default stdout-backed response channel for stdin/session RPCs when no script owns stdin.
    default_response_sender: Option<mpsc::SyncSender<Message>>,
    // List state for variable-height list (supports section headers at 24px + items at 48px)
    main_list_state: ListState,
    /// Generation bumped when filter replacement changes row identity without relying on full measurement.
    main_list_row_generation: u64,
    // Free-scroll handle for the read-only menu syntax hint panel rendered in the main list area.
    menu_syntax_main_hint_scroll_handle: ScrollHandle,
    // Shared free-scroll handle for non-virtualized builtin row stacks.
    builtin_row_stack_scroll_handle: ScrollHandle,
    /// Window-space bounds of each menu-syntax form field, recorded at
    /// prepaint. Lets focus-driven reveal scroll to the field's real position
    /// instead of assuming a fixed per-field height.
    menu_syntax_form_field_bounds: std::rc::Rc<std::cell::RefCell<Vec<gpui::Bounds<gpui::Pixels>>>>,
    // Scroll handle for uniform_list (still used for backward compat in some views)
    list_scroll_handle: UniformListScrollHandle,
    // P0: Scroll handle for virtualized arg prompt choices
    arg_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for clipboard history list
    clipboard_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for emoji picker grid (uniform_list virtualized rows)
    emoji_scroll_handle: UniformListScrollHandle,
    // Frozen frequent-emoji snapshot for the currently open EmojiPickerView.
    // Rebuilt from ~/.kenv/emoji-usage.json when the picker opens; render +
    // navigation + Enter all consume the same Vec so selection indices stay
    // stable while the view is open. See Oracle-Session
    // `emoji-picker-frecency-recency` — "freeze ranking at view-open time".
    emoji_frequent_snapshot: Vec<String>,
    // Scroll handle for window switcher list
    window_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for the Tips browser list
    tips_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for browser tabs list
    browser_tabs_scroll_handle: UniformListScrollHandle,
    // Scroll handle for process manager list
    process_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for the Flow Desk list
    flow_ux_scroll_handle: UniformListScrollHandle,
    // Registry generation last painted by a Flow UX surface; the tick task
    // uses this to repaint only when run state actually changed.
    flow_ux_seen_generation: u64,
    // Guards the single Flow UX repaint tick task (spawned on first open).
    flow_ux_tick_running: bool,
    // Live conversational flow sessions (Enter = converse). Each pairs
    // metadata with its Threadline ChatPrompt entity; backgrounding keeps
    // the entity (and any in-flight turn) alive, re-entering an Active row
    // restores the SAME transcript.
    pub(crate) flow_sessions: Vec<(
        crate::flows::session::FlowSessionMeta,
        Entity<crate::prompts::ChatPrompt>,
    )>,
    // Monotonic id source for flow sessions.
    pub(crate) flow_session_counter: u64,
    /// Escape origin for the CURRENT flow session view: true when the
    /// session was entered from the Conversation Desk (FlowUxView), false
    /// when entered from the main launcher or anywhere else. Escape must
    /// return exactly one step to the surface the user actually came from —
    /// a main-menu-launched session escaping through the desk reads as a
    /// swallowed Escape (see `flow_session_returns_to_desk`).
    pub(crate) flow_session_return_to_desk: bool,
    /// Sender for flow-session chat requests (submit/background). Cloned
    /// into ChatPrompt callbacks, which have no app access.
    pub(crate) flow_chat_sender: mpsc::SyncSender<crate::flows::session::FlowChatRequest>,
    /// Receiver drained by the flow tick on the main thread.
    pub(crate) flow_chat_receiver: mpsc::Receiver<crate::flows::session::FlowChatRequest>,
    // Scroll handle for current app commands list
    current_app_commands_scroll_handle: UniformListScrollHandle,
    // Scroll handle for Agent Chat history list
    agent_chat_history_scroll_handle: ScrollHandle,
    // Scroll handle for browser history list
    browser_history_scroll_handle: ScrollHandle,
    // Scroll handle for dictation history list
    dictation_history_scroll_handle: ScrollHandle,
    // Scroll handle for notes browse portal list
    notes_browse_scroll_handle: ScrollHandle,
    // Scroll handle for design gallery list
    design_gallery_scroll_handle: UniformListScrollHandle,
    // Scroll handle for footer gallery list
    footer_gallery_scroll_handle: UniformListScrollHandle,
    // Scroll handle for file search list
    file_search_scroll_handle: UniformListScrollHandle,
    // Variable-height list state for the theme chooser
    theme_chooser_list_state: ListState,
    // File search loading state (true while mdfind is running)
    file_search_loading: bool,
    // Debounce task for file search (cancelled when new input arrives)
    file_search_debounce_task: Option<gpui::Task<()>>,
    // Current directory being listed (for instant filter mode)
    file_search_current_dir: Option<String>,
    // Whether the current directory cache includes dot-prefixed entries
    file_search_current_dir_show_hidden: bool,
    // Frozen filter during directory transitions (prevents wrong results flash)
    // When Some, use this filter instead of deriving from query
    // Outer Option: None = use query filter, Some = use frozen filter
    // Inner Option: None = no filter, Some(s) = filter by s
    file_search_frozen_filter: Option<Option<String>>,
    // Path of the file selected for actions (for file search actions handling)
    file_search_actions_path: Option<String>,
    // Active sort mode for directory-browse file search views (session-local)
    file_search_sort_mode: crate::actions::FileSearchSortMode,
    // Generation counter for ignoring stale search results
    // Incremented on each new search, results with old gen are discarded
    file_search_gen: u64,
    // Cancel token for in-flight search (set to true to stop mdfind)
    file_search_cancel: Option<file_search::CancelToken>,
    // Pre-computed display indices after Nucleo filtering/sorting
    // This is computed once when results change or filter changes (not in render)
    // Vec of indices into cached_file_results, sorted by match quality
    file_search_display_indices: Vec<usize>,
    // Whether file-search selection should stay pinned to the first visible row
    // or follow the user's explicit selection across streamed updates.
    file_search_selection_mode: FileSearchSelectionMode,
    // Right-side preview panel thumbnail state for selected image files.
    file_search_preview_thumbnail: FileSearchThumbnailPreviewState,
    // Actions popup overlay
    show_actions_popup: bool,
    /// Displayed main-list action shortcuts registered into GPUI's focused keymap.
    registered_main_list_displayed_shortcuts: std::collections::HashSet<String>,
    /// Identity of the focused row the displayed-shortcut keybindings were
    /// last synced for. Skips rebuilding the full action vectors on every
    /// render frame (the rebuild is the arrow-key scroll render hotspot).
    main_list_shortcut_sync_key: Option<String>,
    /// Timestamp of the last actions popup close, used to debounce reopen from
    /// activation-triggered close racing with the footer button click handler.
    actions_closed_at: Option<std::time::Instant>,
    // ActionsDialog entity for focus management
    actions_dialog: Option<Entity<ActionsDialog>>,
    // Cursor blink state and focus tracking
    cursor_visible: bool,
    /// Which input currently has focus (for cursor display)
    focused_input: FocusedInput,
    // Current script process PID for explicit cleanup (belt-and-suspenders)
    current_script_pid: Option<u32>,
    /// Main menu filtered and grouped result caches.
    main_menu_result_caches: MainMenuResultCacheState,
    // P3: Two-stage filter - display vs search separation with coalescing
    /// What the search cache is built from (may lag behind filter_text during rapid typing)
    computed_filter_text: String,
    /// Coalesces filter updates and keeps only the latest value per tick
    filter_coalescer: FilterCoalescer,
    /// Raw-guarded parse of the current filter text for power syntax
    /// (`:` advanced query, `;target` / `target:` capture, `;` hint rows).
    /// Parsed at input-change time in `handle_filter_input_change` and
    /// `set_filter_text_immediate` so result grouping and execution can consume
    /// a stable snapshot without racing the filter coalescer.
    menu_syntax_mode: crate::menu_syntax::MenuSyntaxMode,
    /// Spine parse: parallel projection-based input model.
    /// When `spine_enabled` is true, sigils drive the main list in-place
    /// instead of navigating to Agent Chat picker views.
    spine_enabled: bool,
    spine_parse: crate::spine::SpineParse,
    spine_projection: Option<crate::spine::SpineCursorProjection>,
    spine_cwd: Option<std::path::PathBuf>,
    /// Human-readable label for the resolved CWD (e.g. "Home", "Desktop").
    /// Surfaced as the footer chip so the user sees their working directory
    /// even though the `>:home` segment is stripped from the input bar.
    spine_cwd_label: Option<String>,
    spine_cwd_revision: u64,
    /// Caches the parsed ghost-text context digest (project name, task
    /// phrases, topic keywords) per cwd, keyed by AGENTS.md/README.md
    /// existence + length + mtime. Lets `refresh_ghost_*` avoid re-reading
    /// and re-parsing up to 24k chars of docs on every keystroke.
    ghost_context_cache: crate::scripts::search::ghost::GhostContextCache,
    /// True while the user is inside FileSearchView for the purpose of
    /// picking a working directory (entered by typing `>` in the main
    /// menu). Enter on a directory in this mode sets `spine_cwd` and
    /// returns to ScriptList instead of opening the directory with the
    /// default app.
    pub(crate) cwd_pick_mode: bool,
    /// True while the global Agent & Model picker (Shift+Tab) owns the shared
    /// actions dialog. Gates the MainList activation path so agent/model
    /// selections persist to user preferences instead of dispatching as
    /// ordinary launcher actions. Cleared when the picker closes.
    pub(crate) agent_model_picker_active: bool,
    /// Resolved display name of the globally-selected Agent Chat agent (e.g.
    /// "Claude Code"), shown in the footer marker. Sourced from
    /// `ai.selectedAgentChatAgentId` via the agent catalog; refreshed on startup and
    /// whenever the Agent & Model picker persists a selection.
    pub(crate) spine_agent_label: Option<String>,
    /// Resolved display name of the globally-selected model (e.g. "Sonnet
    /// 4.6"), shown alongside the agent in the footer marker.
    pub(crate) spine_model_label: Option<String>,
    spine_live_preview_cache: crate::spine::live_preview::SpineLivePreviewCache,
    /// Passive AX-only sniff of the frontmost app's selected text, refreshed
    /// on every main-window show. Powers the "Rewrite selection" hint chip in
    /// the main-view context zone. MUST stay on `get_selected_text_ax_only`:
    /// a passive hint may never post keystrokes or touch the pasteboard.
    pub(crate) shown_selection_hint_text: Option<String>,
    /// Monotonic token guarding stale async sniff results (bumped per show).
    pub(crate) shown_selection_hint_token: u64,
    /// Cached state for the menu-syntax trigger picker. `filter_input_change` runs
    /// `plan_trigger_picker_transition` on every filter update and keeps this
    /// field in sync while the detached popup window renders from the snapshot
    /// plus selected row id.
    menu_syntax_trigger_picker_state:
        crate::menu_syntax_trigger_picker::MenuSyntaxTriggerPickerState,
    menu_syntax_object_selector_state: crate::menu_syntax::MenuSyntaxObjectSelectorState,
    /// Focused field index for the grammar-derived handler form shown in
    /// capture composer mode. Tab/Shift-Tab mutate this instead of opening
    /// Tab AI while handler mode owns the main input.
    menu_syntax_form_focused_index: usize,
    /// Stable signature for the currently rendered handler form. Field input
    /// entities are recreated only when the handler/schema changes, not when
    /// field values change.
    menu_syntax_form_signature: Option<String>,
    /// Real focusable input entities for handler form fields.
    menu_syntax_form_inputs: Vec<(String, Entity<InputState>)>,
    /// Subscriptions that sync real handler field edits back into the parser
    /// backed filter text.
    menu_syntax_form_input_subscriptions: Vec<Subscription>,
    /// Guard for field-input initiated serialization so form input echoes do
    /// not recursively rewrite the same focused editor.
    menu_syntax_form_syncing_from_input: bool,
    /// True after Tab has explicitly moved text entry from the main filter
    /// into the handler form. Before this, the form can be visible while the
    /// real cursor and text entry remain in the main input.
    menu_syntax_form_input_active: bool,
    /// Draft value for the active handler form field. This preserves transient
    /// entry state like a trailing space while the canonical filter text is
    /// continuously rewritten from parser-backed field edits.
    menu_syntax_form_draft_field_id: Option<String>,
    menu_syntax_form_draft_value: String,
    /// Selected inline autocomplete suggestion for the active handler form
    /// field. Kept separate from popup object-selector state so handler form
    /// autocomplete does not claim the main result list.
    menu_syntax_form_suggestion_field_id: Option<String>,
    menu_syntax_form_suggestion_selected_index: Option<usize>,
    /// Run 12 Pass 11 — pending Cmd+Enter inline AI proposal for
    /// `cmd-enter-inline-ai-proposal`. Set by the Cmd+Enter handler when the
    /// user is composing power syntax; threaded into the snapshot so the hint
    /// card can render the proposal title + accept-label inline. Cleared on
    /// filter change or Esc/Tab dismissal. Pass 11 ships a deterministic stub
    /// proposal so the receipt is observable without an LLM round-trip; the
    /// real Agent Chat/LLM call wiring is a follow-up.
    pub(crate) pending_menu_syntax_ai_proposal:
        Option<crate::menu_syntax_ai::PendingMenuSyntaxAiProposal>,
    /// When `Some(filter)`, the menu-syntax trigger picker must NOT
    /// automatically re-open for that exact filter text. Set by the
    /// keyboard-apply dispatcher after an Accept (Enter) outcome so the
    /// user does not see the popup "flicker" back open immediately after
    /// they committed a target selection — e.g. typing `+`, pressing
    /// Enter on `Todo inbox` sets the filter to `;todo ` which would
    /// otherwise re-trigger `plan_trigger_picker_transition` → `Open`
    /// with the handler snapshot. The suppression is single-use: any
    /// filter change that produces a DIFFERENT raw text clears it so the
    /// popup can open again when the user keeps typing or deletes back
    /// to a partial trigger.
    menu_syntax_trigger_picker_suppressed_filter: Option<String>,
    /// One-shot footer/Enter hint after accepting a menu-syntax filter
    /// qualifier such as `type:script`. The list is filtered and a row is
    /// selected for keyboard continuity, but Enter should not advertise or
    /// perform that row's action until the user moves selection.
    menu_syntax_filter_accept_hint_label: Option<String>,
    menu_syntax_filter_accept_hint_filter: Option<String>,
    menu_syntax_filter_accept_hint_selected_index: Option<usize>,
    // Scroll stabilization: track last scrolled-to index to avoid redundant scroll_to_item calls
    last_scrolled_index: Option<usize>,
    // Preview cache: avoid re-reading file and re-highlighting on every render
    preview_cache_path: Option<String>,
    preview_cache_match_signature: Option<(usize, usize, usize)>,
    preview_cache_lines: Vec<syntax::HighlightedLine>,
    // Scriptlet preview cache: avoid re-highlighting scriptlet code on every render
    // Key is scriptlet name (unique within session), value is highlighted lines
    scriptlet_preview_cache_key: Option<String>,
    scriptlet_preview_cache_lines: Vec<syntax::HighlightedLine>,
    // Current design variant for hot-swappable UI designs
    current_design: DesignVariant,
    // Live cohesive theme variation for the main menu.
    pub(crate) current_main_menu_theme: crate::designs::MainMenuThemeVariant,
    // Toast manager for notification queue
    toast_manager: ToastManager,
    // Cache for decoded clipboard images (entry_id -> RenderImage)
    clipboard_image_cache: std::collections::HashMap<String, Arc<gpui::RenderImage>>,
    // Frecency store for tracking script usage
    frecency_store: FrecencyStore,
    // Mouse hover tracking - independent from selected_index (keyboard focus)
    // hovered_index shows subtle visual feedback, selected_index shows full focus styling
    hovered_index: Option<usize>,
    // Input mode: Mouse vs Keyboard - when Keyboard, hover effects are disabled
    // to prevent dual-highlight. Mouse movement switches back to Mouse mode.
    input_mode: InputMode,
    /// Main-menu fallback commands for no-match script searches.
    main_menu_fallback_state: MainMenuFallbackState,
    // Theme before chooser was opened (for cancel/restore)
    theme_before_chooser: Option<std::sync::Arc<theme::Theme>>,
    /// Active procedural background effect, if any.
    pub(crate) background_effect: Option<crate::effects::BackgroundEffect>,
    /// Effect strength (0.0..=1.0), loaded from preferences.
    pub(crate) background_effect_intensity: f32,
    /// Animation clock origin for the active background effect.
    pub(crate) background_effect_started_at: Option<std::time::Instant>,
    /// Frame ticker that re-renders while a background effect is active.
    /// Dropping the task cancels the loop.
    pub(crate) _background_effect_ticker: Option<gpui::Task<()>>,
    /// Animation clock origin for the shared main-list loading treatment
    /// (braille constellation + footer spinner) while a slow source fills.
    pub(crate) main_list_loading_started_at: Option<std::time::Instant>,
    /// Monotonic guard so an older loading ticker can never clear the clock
    /// a newer loading interval owns.
    pub(crate) main_list_loading_ticker_epoch: u64,
    /// Frame ticker that re-renders while the loading treatment is active.
    /// Dropping the task cancels the loop.
    pub(crate) _main_list_loading_ticker: Option<gpui::Task<()>>,
    /// Theme Designer save/manage status for user-authored themes.
    pub(crate) theme_chooser_management: Option<ThemeChooserManagementState>,
    /// Theme Chooser's cached view-local component controls (Sliders & ColorPickers)
    pub(crate) theme_chooser_controls: Option<ThemeChooserControls>,
    /// Theme Designer right-panel mode (Preview gallery vs Customize controls)
    pub(crate) theme_chooser_panel_mode: ThemeChooserPanelMode,
    /// Main script-list render diagnostics and input-to-render timing receipts.
    main_menu_render_diagnostics: MainMenuRenderDiagnosticsState,
    // Pending path action - when set, show ActionsDialog for this path
    // Uses Arc<Mutex<>> so callbacks can write to it
    pending_path_action: Arc<Mutex<Option<PathInfo>>>,
    // Signal to close path actions dialog (set by callback on Escape/__cancel__)
    close_path_actions: Arc<Mutex<bool>>,
    // Shared state: whether path actions dialog is currently showing
    // Used by PathPrompt to implement toggle behavior for Cmd+K
    path_actions_showing: Arc<Mutex<bool>>,
    // Shared state: current search text in path actions dialog
    // Used by PathPrompt to display search in header (like main menu does)
    path_actions_search_text: Arc<Mutex<String>>,
    // DEPRECATED: These mutexes were used for polling in render before event-based refactor.
    // Kept for reset_to_script_list cleanup. Will be removed in future cleanup pass.
    #[allow(dead_code)]
    pending_path_action_result: Arc<Mutex<Option<(String, PathInfo)>>>,
    /// Alias registry: lowercase_alias -> script_path (for O(1) lookup)
    /// Conflict rule: first-registered wins
    alias_registry: std::collections::HashMap<String, String>,
    /// Shortcut registry: shortcut -> script_path (for O(1) lookup)
    /// Conflict rule: first-registered wins
    shortcut_registry: std::collections::HashMap<String, String>,
    /// Alias/shortcut conflicts already surfaced (startup log or HUD).
    /// Watcher refreshes re-detect persistent conflicts on every rebuild;
    /// only conflicts NOT in this set toast, so a standing conflict shows
    /// once instead of spamming a HUD per refresh.
    announced_registry_conflicts: std::collections::HashSet<String>,
    /// SDK actions set via setActions() - stored for trigger_action_by_name lookup
    sdk_actions: Option<Vec<protocol::ProtocolAction>>,
    /// SDK action shortcuts: normalized_shortcut -> action_name (for O(1) lookup)
    action_shortcuts: std::collections::HashMap<String, String>,
    /// Debug grid overlay configuration (None = hidden)
    grid_config: Option<debug_grid::GridConfig>,
    // Navigation coalescing for rapid arrow key events (20ms window)
    // NOTE: Currently unused - arrow keys handled in interceptor without coalescing
    #[allow(dead_code)]
    nav_coalescer: NavCoalescer,
    // Wheel scroll accumulator for smooth trackpad scrolling
    // Accumulates fractional deltas until they cross 1.0, then converts to item steps
    wheel_accum: f32,
    /// Visual-only boundary gesture state. Logical ListState and selection stay clamped.
    main_list_boundary_affordance: crate::scrolling::boundary_affordance::BoundaryAffordanceState,
    main_list_suppress_hover_until_mouse_move: bool,
    /// Armed when a menu-syntax picker row accepts on mouse down. The same
    /// physical click can finish after the list has re-rendered to normal
    /// launcher rows; consume that trailing click so it cannot submit row 0.
    menu_syntax_trigger_picker_suppress_next_launcher_click: bool,
    /// Armed when a menu-syntax trigger picker row accepts on the global
    /// Enter key-down path. GPUI's input component can emit a follow-up
    /// PressEnter for the same physical key after the picker has closed; this
    /// consumes that echo so it cannot submit the first filtered launcher row.
    menu_syntax_trigger_picker_enter_guard: Option<std::time::Instant>,
    // Window focus tracking - for detecting focus lost and auto-dismissing prompts
    // When window loses focus while in a dismissable prompt, close and reset
    was_window_focused: bool,
    /// Pin state - when true, window stays open on blur (only closes via ESC/Cmd+W)
    /// Toggle with Cmd+Shift+P
    is_pinned: bool,
    /// Editor prompt Escape guard: first Escape arms (HUD "Esc again to
    /// discard"), a second Escape within the guard window cancels the prompt
    /// (script receives None). Cleared on view reset.
    editor_escape_armed_at: Option<std::time::Instant>,
    /// Pending focus target - when set, focus will be applied once on next render
    /// then cleared. This avoids the "perpetually enforce focus in render()" anti-pattern.
    /// DEPRECATED: Use focus_coordinator instead. This remains for gradual migration.
    pending_focus: Option<FocusTarget>,
    /// Focus coordinator - centralized focus management with push/pop overlay semantics.
    /// This is the new unified focus system that replaces focused_input + pending_focus.
    focus_coordinator: focus_coordinator::FocusCoordinator,
    // Show warning banner when bun is not available
    show_bun_warning: bool,
    // Scroll stabilization: track last scrolled-to index for each scroll handle
    #[allow(dead_code)]
    last_scrolled_main: Option<usize>,
    #[allow(dead_code)]
    last_scrolled_arg: Option<usize>,
    #[allow(dead_code)]
    last_scrolled_clipboard: Option<usize>,
    #[allow(dead_code)]
    last_scrolled_window: Option<usize>,
    #[allow(dead_code)]
    last_scrolled_design_gallery: Option<usize>,
    // Menu bar integration: Now handled by frontmost_app_tracker module
    // which pre-fetches menu items in background when apps activate
    /// Shortcut recorder state - when Some, shows the inline recorder overlay
    shortcut_recorder_state: Option<ShortcutRecorderState>,
    /// The shortcut recorder entity (persisted to maintain focus)
    shortcut_recorder_entity:
        Option<Entity<crate::components::shortcut_recorder::ShortcutRecorder>>,
    /// Alias input state - when Some, shows the alias input modal
    alias_input_state: Option<AliasInputState>,
    /// The alias input entity (persisted to maintain focus)
    alias_input_entity: Option<Entity<crate::components::alias_input::AliasInput>>,
    /// Most recent Tab AI execution waiting for success/failure accounting.
    pending_tab_ai_execution: Option<crate::ai::TabAiExecutionRecord>,
    /// Tab AI save-offer overlay state — when Some, prompts to persist the last successful ephemeral script.
    tab_ai_save_offer_state: Option<TabAiSaveOfferState>,
    /// Persistent harness terminal session — reused across Tab presses.
    pub(crate) tab_ai_harness: Option<crate::ai::TabAiHarnessSessionState>,
    /// Generation counter for cancelling stale deferred capture results.
    /// Incremented on each new `begin_tab_ai_harness_entry()` call.
    pub(crate) tab_ai_harness_capture_generation: u64,
    /// Previous surface to restore when leaving Tab AI quick terminal.
    pub(crate) tab_ai_harness_return_view: Option<AppView>,
    /// Previous focus target to restore when leaving Tab AI quick terminal.
    pub(crate) tab_ai_harness_return_focus_target: Option<FocusTarget>,
    /// Main-menu trigger that launched the current Agent Chat session, if any.
    pub(crate) tab_ai_harness_script_list_trigger: Option<char>,
    /// Pending explicit apply-back route for the active Tab AI harness session.
    pub(crate) tab_ai_harness_apply_back_route: Option<crate::ai::TabAiApplyBackRoute>,
    /// Persistent embedded Agent Chat chat entity so repeated Tab opens can reuse
    /// the same live Agent Chat connection instead of cold-starting a new one.
    pub(crate) embedded_agent_chat: Option<Entity<crate::ai::agent_chat::ui::view::AgentChatView>>,
    /// Cached focus handle for the embedded Agent Chat chat. Focus restoration happens
    /// from parent render paths, so it must not read the child while Agent Chat is
    /// updating its picker/composer state.
    pub(crate) embedded_agent_chat_focus_handle: Option<gpui::FocusHandle>,
    /// Hidden, never-shown Agent Chat chat entity warmed at startup. It is consumed
    /// by the first compatible Agent Chat open so prompt submit avoids initialization.
    pub(crate) prewarmed_agent_chat: Option<Entity<crate::ai::agent_chat::ui::view::AgentChatView>>,
    /// Active Pi-backed Agent Chat warm lease, reset on chat dismissal so the
    /// next Agent Chat open starts from a fresh warm session.
    pub(crate) active_agent_chat_warm_lease:
        Option<crate::ai::agent_chat::warm_session::AgentChatWarmSessionLease>,
    /// Cached ready script path from the Agent Chat `SCRIPT_READY` receipt. Updated
    /// whenever the Agent Chat observer fires. Used by footer button resolution
    /// (which only has `&self`) without needing a `cx` to read the Agent Chat entity.
    pub(crate) agent_chat_ready_script_path: Option<std::path::PathBuf>,
    /// Cached Agent Chat footer dot status so child Agent Chat notifications only repaint
    /// the parent-owned native footer when the visible footer state changes.
    pub(crate) agent_chat_footer_dot_status: Option<crate::footer_popup::FooterDotStatus>,
    /// Cached Agent Chat model label paired with `agent_chat_footer_dot_status`.
    pub(crate) agent_chat_footer_model_display: Option<String>,
    /// Cached Agent Chat footer state so native footer labels refresh when composer
    /// or response-derived actions change.
    pub(crate) agent_chat_footer_snapshot:
        Option<crate::ai::agent_chat::ui::view::AgentChatFooterSnapshot>,
    /// Snapshot of shared launcher host state while an attachment portal owns
    /// the main window.
    pub(crate) attachment_portal_host_snapshot: Option<AttachmentPortalHostSnapshot>,
    /// Previous surface to restore when leaving an attachment portal (file search / clipboard).
    pub(crate) attachment_portal_return_view: Option<AppView>,
    /// Previous focus target to restore when leaving an attachment portal.
    pub(crate) attachment_portal_return_focus_target: Option<FocusTarget>,
    /// Window width to restore when leaving an attachment portal.
    pub(crate) attachment_portal_return_width: Option<f32>,
    /// Which attachment portal is currently active, when any.
    pub(crate) active_attachment_portal_kind:
        Option<crate::ai::context_selector::types::ContextPortalKind>,
    /// Byte range of the `@file` spine segment that opened a ScriptList-hosted
    /// file-search attachment portal. `Some` marks the portal as spine-hosted:
    /// accept resolves the segment into a compact `@file:basename` token
    /// instead of attaching to Agent Chat. Mirrors the agent-chat host fields
    /// above; both feed `is_in_attachment_portal`.
    pub(crate) spine_mention_portal_segment: Option<std::ops::Range<usize>>,
    /// Session alias registry for compact spine mention tokens inserted into
    /// the main filter (`@file:basename` → full `AiContextPart::FilePath`).
    /// The launcher-side mirror of `AgentChatView::typed_mention_aliases`.
    pub(crate) spine_mention_aliases:
        std::collections::HashMap<String, crate::ai::message_parts::AiContextPart>,
    /// Pending Today → main-menu `@context` round trip. `Some` while the user
    /// searches context in the main menu on behalf of the Day Page; accepting
    /// a context row returns to the held Day Page entity with the resolved
    /// token spliced into the originating line. Escape cancels back to Today.
    pub(crate) day_page_context_return: Option<DayPageContextReturn>,
    /// App-owned placement machine for the Agent Chat surface. Source of
    /// truth for the `blocks_launcher_ai_entry` and
    /// `is_attachment_portal` predicates. Written only through
    /// `transition_agent_chat_surface`; do not mutate directly.
    pub(crate) agent_chat_surface_state:
        crate::ai::agent_chat::ui::surface_state::AgentChatSurfaceState,
    /// Input history for shell-like up/down navigation through previous inputs
    input_history: input_history::InputHistory,
    /// Pending API key configuration - tracks which provider is being configured
    /// Used to show success toast after EnvPrompt completes
    pending_api_key_config: Option<String>,
    /// Sender for API key configuration completion signals
    /// The EnvPrompt callback uses this to signal when done
    api_key_completion_sender: mpsc::SyncSender<(String, bool)>,
    /// Receiver for API key configuration completion signals
    /// Checked by timer to trigger toast and view reset
    api_key_completion_receiver: mpsc::Receiver<(String, bool)>,
    /// Whether the current built-in view was opened from the main menu.
    /// When true, ESC returns to main menu. When false (opened via hotkey/protocol), ESC closes window.
    opened_from_main_menu: bool,
    /// When Some, the script list is filtered to only show scripts whose names
    /// match one of these IDs. Set by the Favorites builtin, cleared on reset.
    active_favorites: Option<Vec<String>>,
    /// Sender for inline chat escape signals
    /// The ChatPrompt escape callback uses this to signal when ESC is pressed
    inline_chat_escape_sender: mpsc::SyncSender<()>,
    /// Receiver for inline chat escape signals
    /// Checked by timer to trigger view reset
    inline_chat_escape_receiver: mpsc::Receiver<()>,
    /// Sender for inline chat UI requests that need parent window access.
    inline_chat_actions_sender: mpsc::SyncSender<MiniAiUiRequest>,
    /// Receiver for inline chat UI requests that need parent window access.
    inline_chat_actions_receiver: mpsc::Receiver<MiniAiUiRequest>,
    /// Last close snapshot emitted by the inline Mini AI ChatPrompt path.
    mini_ai_last_close_snapshot: Option<MiniAiCloseSnapshot>,
    /// Sender for inline chat continue signals
    /// The ChatPrompt continue callback uses this to signal "Continue in Harness Terminal"
    #[allow(dead_code)]
    inline_chat_continue_sender: mpsc::SyncSender<()>,
    /// Receiver for inline chat continue signals
    /// Checked by timer to hide main window when transferring to AI window
    inline_chat_continue_receiver: mpsc::Receiver<()>,
    /// Sender for inline chat configure signals
    /// The ChatPrompt configure callback uses this to signal when user wants to configure API key
    inline_chat_configure_sender: mpsc::SyncSender<()>,
    /// Receiver for inline chat configure signals
    /// Checked by timer to trigger API key configuration prompt
    inline_chat_configure_receiver: mpsc::Receiver<()>,
    /// Sender for inline chat Claude Code signals
    /// The ChatPrompt Claude Code callback uses this to signal when user wants to enable Claude Code
    inline_chat_claude_code_sender: mpsc::SyncSender<()>,
    /// Receiver for inline chat Claude Code signals
    /// Checked by timer to trigger Claude Code enablement
    inline_chat_claude_code_receiver: mpsc::Receiver<()>,
    /// Sender for naming dialog completion signals
    /// Some(json_payload) = user submitted a name, None = user cancelled
    naming_submit_sender: mpsc::SyncSender<Option<String>>,
    /// Receiver for naming dialog completion signals
    /// Checked in render loop to handle naming dialog submit/cancel
    naming_submit_receiver: mpsc::Receiver<Option<String>>,
    /// Opacity offset for light theme adjustment
    /// Use Cmd+Shift+[ to decrease and Cmd+Shift+] to increase
    /// Range: -0.5 to +0.5 (added to base opacity values)
    light_opacity_offset: f32,
    /// Whether the mouse cursor is currently hidden (hidden while typing, shown on mouse move)
    mouse_cursor_hidden: bool,
    /// Cached provider registry built in background at startup.
    /// Avoids blocking the UI thread when opening inline AI chat.
    cached_provider_registry: Option<crate::ai::ProviderRegistry>,
    /// Cached preflight receipt for the main-window Execution Contract rail.
    /// Rebuilt on selection/filter changes; consumed read-only in render().
    cached_main_window_preflight: Option<crate::main_window_preflight::MainWindowPreflightReceipt>,
    /// Cache key for preflight receipt (filter_text + selected_index + view).
    /// Cleared by `invalidate_main_window_preflight()`.
    main_window_preflight_cache_key: String,
    /// Window orchestrator — pure state machine for window visibility and focus.
    window_orchestrator: crate::window_orchestrator::OrchestratorState,
}

#[derive(Clone, Debug)]
struct AttachmentPortalHostSnapshot {
    filter_text: String,
    computed_filter_text: String,
    pending_filter_sync: bool,
    pending_placeholder: Option<String>,
    hovered_index: Option<usize>,
    selected_index: usize,
    opened_from_main_menu: bool,
    focused_input: FocusedInput,
    pending_focus: Option<FocusTarget>,
    width_before_portal: Option<f32>,
    width_after_portal_open: Option<f32>,
}

/// Result of alias matching - either a Script or Scriptlet
#[derive(Clone, Debug)]
enum AliasMatch {
    Script(Arc<scripts::Script>),
    Scriptlet(Arc<scripts::Scriptlet>),
    BuiltIn(Arc<builtins::BuiltInEntry>),
    App(Arc<app_launcher::AppInfo>),
}

pub(crate) const ROOT_LAUNCHER_PLACEHOLDER: &str =
    "Search • @ context • / commands • ; capture • : filters";

#[cfg(test)]
mod app_state_selection_tests {
    use super::{main_menu_refresh_selection_policy, MainMenuRefreshSelectionPolicy};

    #[test]
    fn async_refresh_snaps_untouched_selection_and_restores_user_selection() {
        assert_eq!(
            main_menu_refresh_selection_policy(false),
            MainMenuRefreshSelectionPolicy::SnapToFirst
        );
        assert_eq!(
            main_menu_refresh_selection_policy(true),
            MainMenuRefreshSelectionPolicy::RestoreIdentity
        );
    }
}
