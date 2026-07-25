//! Result grouping for the main menu
//!
//! This module provides functions for grouping search results into
//! sections based on their source kit.
//!
//! When the filter is empty (grouped view), items are organized by their source kit:
//! - SUGGESTED (frecency-based recent items)
//! - {KIT_NAME} (e.g., CLEANSHOT, MAIN - containing scripts, scriptlets, AND agents from that kit)
//! - COMMANDS (built-ins and window controls)
//! - APPS (installed applications)
//!
//! Note: Scripts, scriptlets, and agents are all grouped under their source kit section.
//! The "main" kit appears last in the kit-based sections.

use std::sync::Arc;
use tracing::instrument;

use crate::app_launcher::AppInfo;
use crate::builtins::{menu_bar_items_to_entries, BuiltInEntry};
use crate::config::SuggestedConfig;
use crate::frecency::FrecencyStore;
use crate::list_item::{GroupedListItem, SourceChipStatusKind, SourceChipStatusRow};
use crate::menu_bar::MenuBarItem;
use crate::plugins::PluginSkill;

use super::search::{fuzzy_search_root_windows, fuzzy_search_unified_all_with_skills_and_flows};
use super::types::{
    FallbackMatch, MatchIndices, Script, ScriptIssueMatch, ScriptMatch, ScriptMatchKind, Scriptlet,
    SearchResult,
};
use super::validation::ValidationReport;

pub(crate) mod grouped_view;
mod search_mode;

/// Default maximum number of items to show in the RECENT section
pub const DEFAULT_MAX_RECENT_ITEMS: usize = 10;

/// Default suggested built-in names for new users without frecency data.
/// These appear in the SUGGESTED section when the user has no usage history.
/// Order matters - items appear in this order and must match built-in entry names.
pub const DEFAULT_SUGGESTED_ITEMS: &[&str] = &[
    "Do in Current App",
    "Agent Chat",
    "Search Files",
    "Clipboard History",
    "Search Browser Tabs",
    "Window Switcher",
    "Quick Terminal",
    "Open Notes",
    "New Script",
];

/// Maximum number of menu bar items to show in search results
/// This prevents menu bar actions from overwhelming the results
pub const MAX_MENU_BAR_ITEMS: usize = 5;

/// Minimum score required for a menu bar item to appear in results
/// This filters out weak matches that would clutter the list
pub const MIN_MENU_BAR_SCORE: i32 = 25;
pub const ROOT_PASSIVE_RESULT_SCORE_BASE: i32 = 100_000;

pub(crate) fn root_passive_result_score(rank: usize) -> i32 {
    ROOT_PASSIVE_RESULT_SCORE_BASE.saturating_sub(rank as i32)
}

/// Get grouped results with SUGGESTED/MAIN sections based on frecency.
#[instrument(level = "debug", skip_all, fields(filter_len = filter_text.len()))]
#[allow(clippy::too_many_arguments)]
pub fn get_grouped_results(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    skills: &[Arc<PluginSkill>],
    frecency_store: &FrecencyStore,
    filter_text: &str,
    suggested_config: &SuggestedConfig,
    menu_bar_items: &[MenuBarItem],
    menu_bar_bundle_id: Option<&str>,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    get_grouped_results_with_input_history(
        scripts,
        scriptlets,
        builtins,
        apps,
        skills,
        frecency_store,
        filter_text,
        suggested_config,
        menu_bar_items,
        menu_bar_bundle_id,
        None,
    )
}

#[instrument(level = "debug", skip_all, fields(filter_len = filter_text.len()))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn get_grouped_results_with_input_history(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    skills: &[Arc<PluginSkill>],
    frecency_store: &FrecencyStore,
    filter_text: &str,
    suggested_config: &SuggestedConfig,
    menu_bar_items: &[MenuBarItem],
    menu_bar_bundle_id: Option<&str>,
    input_history: Option<&crate::input_history::InputHistory>,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    get_grouped_results_with_input_history_and_query(
        scripts,
        scriptlets,
        builtins,
        apps,
        skills,
        &[],
        None,
        frecency_store,
        filter_text,
        suggested_config,
        menu_bar_items,
        menu_bar_bundle_id,
        input_history,
        None,
        None,
    )
}

/// Variant of [`get_grouped_results_with_input_history`] that also accepts an
/// optional [`crate::menu_syntax::AdvancedQuery`] for `:` prefix filtering.
///
/// When `advanced_query` is `Some`, the caller is expected to have already
/// substituted `filter_text` with the free-text portion (via
/// [`crate::menu_syntax::free_text_for_search`]). The fuzzy search runs against
/// `filter_text` and the predicate list post-filters the results before
/// search-mode frecency sorting or the grouped-view layout runs.
#[instrument(level = "debug", skip_all, fields(filter_len = filter_text.len()))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn get_grouped_results_with_input_history_and_query(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    skills: &[Arc<PluginSkill>],
    flows: &[crate::flows::model::FlowDescriptor],
    flow_discovery: Option<&grouped_view::FlowDiscoveryNote>,
    frecency_store: &FrecencyStore,
    filter_text: &str,
    suggested_config: &SuggestedConfig,
    menu_bar_items: &[MenuBarItem],
    menu_bar_bundle_id: Option<&str>,
    input_history: Option<&crate::input_history::InputHistory>,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    launcher_context: Option<&crate::context_snapshot::launcher_context::LauncherContextSnapshot>,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    // When filter is non-empty and we have menu bar items, include them in search.
    let all_builtins: Vec<BuiltInEntry>;
    let builtins_to_use: &[BuiltInEntry] = if let Some(bundle_id) =
        menu_bar_bundle_id.filter(|_| !filter_text.is_empty() && !menu_bar_items.is_empty())
    {
        let app_name = bundle_id.rsplit('.').next().unwrap_or(bundle_id);
        let menu_entries = menu_bar_items_to_entries(menu_bar_items, bundle_id, app_name);
        all_builtins = builtins.iter().cloned().chain(menu_entries).collect();
        &all_builtins
    } else {
        builtins
    };

    let results = fuzzy_search_unified_all_with_skills_and_flows(
        scripts,
        scriptlets,
        builtins_to_use,
        apps,
        skills,
        flows,
        filter_text,
    );

    let results = match advanced_query {
        Some(query) => crate::menu_syntax::apply_advanced_query(results, query),
        None => results,
    };

    if !filter_text.is_empty() {
        let preferred_result_key =
            input_history.and_then(|history| history.preferred_result_key(filter_text));
        return search_mode::build_search_mode_results(
            results,
            scripts,
            frecency_store,
            filter_text,
            preferred_result_key,
            launcher_context,
            advanced_query.is_some_and(|query| query.has_predicates()),
        );
    }

    grouped_view::build_grouped_view_results(
        results,
        frecency_store,
        suggested_config,
        flow_discovery,
    )
}

/// Pins a synthetic `SearchResult::ScriptIssue` row at `flat_results[0]` and
/// shifts every existing `GroupedListItem::Item(idx)` by +1 so the rest of
/// the list continues to point at the original results.
///
/// Called from [`get_grouped_results_with_validation`] when validation has
/// recorded one or more failed scripts and the surface should show the
/// launcher "Script Issues" repair row.
pub(crate) fn prepend_script_issues_row(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    validation: &ValidationReport,
) {
    let failed_count = validation.failed_scripts.len();
    if failed_count == 0 {
        return;
    }

    let fatal_count = validation.fatal_count;
    let warning_count = validation.warning_count;

    let description = if fatal_count > 0 && warning_count > 0 {
        Some(format!(
            "{} failed · {} fatal · {} warning{}",
            failed_count,
            fatal_count,
            warning_count,
            if warning_count == 1 { "" } else { "s" }
        ))
    } else if fatal_count > 0 {
        Some(format!(
            "{} script{} excluded · {} fatal issue{}",
            failed_count,
            if failed_count == 1 { "" } else { "s" },
            fatal_count,
            if fatal_count == 1 { "" } else { "s" }
        ))
    } else {
        Some(format!(
            "{} script{} flagged",
            failed_count,
            if failed_count == 1 { "" } else { "s" }
        ))
    };

    let issue = ScriptIssueMatch {
        title: format!("Script Issues ({failed_count})"),
        description,
        failed_count,
        fatal_count,
        warning_count,
        score: i32::MAX, // pinned to the top regardless of sort
    };

    flat_results.insert(0, SearchResult::ScriptIssue(issue));

    for entry in grouped.iter_mut() {
        if let GroupedListItem::Item(idx) = entry {
            *idx += 1;
        }
    }

    grouped.insert(0, GroupedListItem::Item(0));
}

/// Pins the "Brain Inbox" section (header + up to `options.max_results` open
/// inbox rows) at the very top of the empty-query grouped launcher view.
///
/// Mirrors [`prepend_script_issues_row`]: rows are inserted at the front of
/// `flat_results` and every existing `GroupedListItem::Item(idx)` shifts by
/// the number of inserted rows so the rest of the list keeps pointing at the
/// original results. No-op on non-empty queries, when the section is
/// disabled, or when there are no open items. `now` is a unix timestamp used
/// for relative-age subtitles (injectable for tests).
pub(crate) fn prepend_root_brain_inbox_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    items: &[crate::brain::InboxItem],
    options: crate::brain::RootBrainInboxSectionOptions,
    now: i64,
) {
    if !filter_text.trim().is_empty()
        || !options.enabled
        || options.max_results == 0
        || items.is_empty()
    {
        return;
    }

    let rows: Vec<SearchResult> = items
        .iter()
        .take(options.max_results)
        .enumerate()
        .map(|(rank, item)| {
            SearchResult::BrainInboxItem(crate::scripts::BrainInboxMatch {
                subtitle: crate::brain::root_brain_inbox_subtitle(item, now),
                item: item.clone(),
                score: root_passive_result_score(rank),
            })
        })
        .collect();

    let shift = rows.len();
    for entry in grouped.iter_mut() {
        if let GroupedListItem::Item(idx) = entry {
            *idx += shift;
        }
    }
    for (offset, row) in rows.into_iter().enumerate() {
        flat_results.insert(offset, row);
    }

    let mut section = Vec::with_capacity(shift + 1);
    section.push(GroupedListItem::SectionHeader(
        "Brain Inbox".to_string(),
        Some("inbox".to_string()),
    ));
    section.extend((0..shift).map(GroupedListItem::Item));
    grouped.splice(0..0, section);
}

/// Pin live flow conversations above every other launcher row, including the
/// synthetic Brain Inbox section. Session rows reuse the flow row language,
/// but carry a session id so Enter reattaches instead of starting over.
pub(crate) fn prepend_root_flow_sessions_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    sessions: &[crate::flows::session::FlowSessionMeta],
    flows: &[crate::flows::model::FlowDescriptor],
) {
    let query = filter_text.trim().to_lowercase();
    let mut live: Vec<&crate::flows::session::FlowSessionMeta> = sessions
        .iter()
        .filter(|meta| meta.state.is_live())
        .filter(|meta| {
            query.is_empty()
                || meta.friendly_name.to_lowercase().contains(&query)
                || meta.flow_name.to_lowercase().contains(&query)
                || meta.state.label().contains(&query)
        })
        .collect();
    // Semantic recency, not creation order: returning to an older session
    // moves it back to the top. Stable id descending breaks timestamp ties so
    // equal clocks cannot reorder rows frame-to-frame (keyboard selection
    // must never watch two rows swap under it).
    live.sort_by(|a, b| b.last_activity.cmp(&a.last_activity).then(b.id.cmp(&a.id)));

    let rows: Vec<SearchResult> = live
        .into_iter()
        .filter_map(|meta| {
            let flow = flows.iter().find(|flow| flow.id == meta.flow_id)?.clone();
            Some(SearchResult::Flow(crate::scripts::FlowMatch {
                flow,
                session_id: Some(meta.id),
                display_name: meta.friendly_name.clone(),
                subtitle: format!(
                    "{} · {} · active conversation",
                    meta.state.label(),
                    meta.engine
                ),
                score: i32::MAX,
                match_indices: crate::scripts::MatchIndices::default(),
            }))
        })
        .collect();
    if rows.is_empty() {
        return;
    }

    let shift = rows.len();
    for entry in grouped.iter_mut() {
        if let GroupedListItem::Item(idx) = entry {
            *idx += shift;
        }
    }
    for (offset, row) in rows.into_iter().enumerate() {
        flat_results.insert(offset, row);
    }
    let mut section = Vec::with_capacity(shift + 1);
    section.push(GroupedListItem::SectionHeader(
        "Active Flows".to_string(),
        Some("bot".to_string()),
    ));
    section.extend((0..shift).map(GroupedListItem::Item));
    grouped.splice(0..0, section);
}

/// Moves the launcher row identified by `is_alias_target` to the very top of
/// the grouped list so Enter runs it.
///
/// Decision (2026-06-09): typing text that exactly matches a registered alias
/// means "pin the aliased command at index 0, no matter what" — replacing the
/// old behavior where an alias plus a trailing space executed immediately.
/// When the aliased command is not present in `flat_results` (the raw query
/// may no longer fuzzy-match it, e.g. a trailing space), `fallback` supplies
/// a synthetic result so the pin always lands.
pub(crate) fn pin_alias_match_first(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    is_alias_target: &dyn Fn(&SearchResult) -> bool,
    fallback: &dyn Fn() -> SearchResult,
) {
    let flat_idx = match flat_results.iter().position(is_alias_target) {
        Some(idx) => idx,
        None => {
            flat_results.push(fallback());
            flat_results.len() - 1
        }
    };

    let pin_index = |grouped: &[GroupedListItem]| -> usize {
        match grouped.first() {
            Some(GroupedListItem::SectionHeader(label, _)) if label == "Results" => 1,
            _ => 0,
        }
    };

    let Some(pos) = grouped
        .iter()
        .position(|item| matches!(item, GroupedListItem::Item(idx) if *idx == flat_idx))
    else {
        let insert_at = pin_index(grouped).min(grouped.len());
        grouped.insert(insert_at, GroupedListItem::Item(flat_idx));
        return;
    };
    if pos == pin_index(grouped) {
        return;
    }

    let entry = grouped.remove(pos);
    // Drop a section header orphaned by the move (a header directly above the
    // pinned row with no row left underneath it).
    if pos > 0
        && matches!(
            grouped.get(pos - 1),
            Some(GroupedListItem::SectionHeader(..))
        )
        && !matches!(grouped.get(pos), Some(GroupedListItem::Item(_)))
    {
        grouped.remove(pos - 1);
    }
    let insert_at = pin_index(grouped).min(grouped.len());
    grouped.insert(insert_at, entry);
}

/// Validation-aware sibling of [`get_grouped_results_with_input_history`].
///
/// When `validation` is `Some` and it recorded failed scripts, a synthetic
/// `SearchResult::ScriptIssue` row is pinned at the top of the results so the
/// launcher surfaces "my script vanished" repair paths inline.
#[instrument(level = "debug", skip_all, fields(filter_len = filter_text.len()))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn get_grouped_results_with_validation(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    skills: &[Arc<PluginSkill>],
    frecency_store: &FrecencyStore,
    filter_text: &str,
    suggested_config: &SuggestedConfig,
    menu_bar_items: &[MenuBarItem],
    menu_bar_bundle_id: Option<&str>,
    input_history: Option<&crate::input_history::InputHistory>,
    validation: Option<&ValidationReport>,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    get_grouped_results_with_validation_and_query(
        scripts,
        scriptlets,
        builtins,
        apps,
        skills,
        &[],
        None,
        frecency_store,
        filter_text,
        suggested_config,
        menu_bar_items,
        menu_bar_bundle_id,
        input_history,
        validation,
        None,
    )
}

/// Validation-aware sibling of [`get_grouped_results_with_input_history_and_query`].
///
/// If `advanced_query` has predicates that reject `SearchResult::ScriptIssue`
/// (for example `:type:script` without `issue` anywhere), the synthetic issue
/// row is not prepended. Without this guard a filter like `:type:script git`
/// would leak an Issue-kind row into a script-only view.
#[instrument(level = "debug", skip_all, fields(filter_len = filter_text.len()))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn get_grouped_results_with_validation_and_query(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    skills: &[Arc<PluginSkill>],
    flows: &[crate::flows::model::FlowDescriptor],
    flow_discovery: Option<&grouped_view::FlowDiscoveryNote>,
    frecency_store: &FrecencyStore,
    filter_text: &str,
    suggested_config: &SuggestedConfig,
    menu_bar_items: &[MenuBarItem],
    menu_bar_bundle_id: Option<&str>,
    input_history: Option<&crate::input_history::InputHistory>,
    validation: Option<&ValidationReport>,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    let (mut grouped, mut flat_results) = get_grouped_results_with_input_history_and_query(
        scripts,
        scriptlets,
        builtins,
        apps,
        skills,
        flows,
        flow_discovery,
        frecency_store,
        filter_text,
        suggested_config,
        menu_bar_items,
        menu_bar_bundle_id,
        input_history,
        advanced_query,
        None,
    );

    // Show the pinned row unconditionally when the grouped view is active
    // (empty filter) and there are failures. Also show during search when the
    // query hints at "issues" so authors can Cmd-F to the repair row.
    let filter_hints_issues = {
        let q = filter_text.trim().to_lowercase();
        ["issue", "issues", "failed", "validation", "hidden"]
            .iter()
            .any(|needle| q.contains(*needle))
    };

    let should_show = filter_text.is_empty() || filter_hints_issues;

    if should_show {
        if let Some(report) = validation {
            if !report.failed_scripts.is_empty() && !advanced_query_rejects_issue(advanced_query) {
                prepend_script_issues_row(&mut grouped, &mut flat_results, report);
            }
        }
    }

    (grouped, flat_results)
}

#[instrument(level = "debug", skip_all, fields(filter_len = filter_text.len()))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn get_grouped_results_with_validation_query_and_root_files(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    skills: &[Arc<PluginSkill>],
    frecency_store: &FrecencyStore,
    filter_text: &str,
    suggested_config: &SuggestedConfig,
    menu_bar_items: &[MenuBarItem],
    menu_bar_bundle_id: Option<&str>,
    input_history: Option<&crate::input_history::InputHistory>,
    validation: Option<&ValidationReport>,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    root_file_search_mode: Option<crate::file_search::RootFileSectionMode>,
    root_file_search_loading: bool,
    root_file_results: &[crate::file_search::FileResult],
    root_recent_file_results: &[crate::file_search::FileResult],
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    get_grouped_results_with_validation_query_and_root_files_with_options(
        scripts,
        scriptlets,
        builtins,
        apps,
        &[],
        crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
        skills,
        &[],
        None,
        frecency_store,
        filter_text,
        suggested_config,
        menu_bar_items,
        menu_bar_bundle_id,
        input_history,
        validation,
        advanced_query,
        &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
        root_file_search_mode,
        root_file_search_loading,
        root_file_results,
        root_recent_file_results,
        crate::file_search::RootFileSectionOptions::default(),
        &[],
        crate::menu_syntax::RootTodoSectionOptions {
            enabled: false,
            ..Default::default()
        },
        &[],
        crate::brain::RootBrainSectionOptions {
            enabled: false,
            ..Default::default()
        },
        &[],
        crate::notes::RootNotesSectionOptions {
            enabled: false,
            ..Default::default()
        },
        &[],
        crate::clipboard_history::RootClipboardHistorySectionOptions {
            enabled: false,
            ..Default::default()
        },
        &[],
        crate::dictation::RootDictationHistorySectionOptions {
            enabled: false,
            max_results: 0,
            min_query_chars: usize::MAX,
            scan_limit: 0,
        },
        &[],
        crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
            enabled: false,
            ..Default::default()
        },
        &[],
        crate::ai_vault::RootAiVaultSectionOptions {
            enabled: false,
            ..Default::default()
        },
        &[],
        crate::browser_tabs::RootBrowserTabsSectionOptions {
            enabled: false,
            ..Default::default()
        },
        &[],
        crate::browser_history::RootBrowserHistorySectionOptions {
            enabled: false,
            ..Default::default()
        },
        &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
        crate::config::UnifiedSearchPassiveResultLimitsConfig::default(),
    )
}

#[instrument(level = "debug", skip_all, fields(filter_len = filter_text.len()))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn get_grouped_results_with_validation_query_and_root_files_with_options(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    windows: &[crate::scripts::RootWindowEntry],
    root_windows_provider_status: crate::window_control::RootWindowsProviderStatus,
    skills: &[Arc<PluginSkill>],
    flows: &[crate::flows::model::FlowDescriptor],
    flow_discovery: Option<&grouped_view::FlowDiscoveryNote>,
    frecency_store: &FrecencyStore,
    filter_text: &str,
    suggested_config: &SuggestedConfig,
    menu_bar_items: &[MenuBarItem],
    menu_bar_bundle_id: Option<&str>,
    input_history: Option<&crate::input_history::InputHistory>,
    validation: Option<&ValidationReport>,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    root_source_filters: &crate::menu_syntax::RootUnifiedSourceFilterSet,
    root_file_search_mode: Option<crate::file_search::RootFileSectionMode>,
    root_file_search_loading: bool,
    root_file_results: &[crate::file_search::FileResult],
    root_recent_file_results: &[crate::file_search::FileResult],
    root_file_options: crate::file_search::RootFileSectionOptions,
    root_todo_hits: &[crate::menu_syntax::RootTodoSearchHit],
    root_todo_options: crate::menu_syntax::RootTodoSectionOptions,
    root_brain_hits: &[crate::brain::RootBrainSearchHit],
    root_brain_options: crate::brain::RootBrainSectionOptions,
    root_note_hits: &[crate::notes::RootNoteSearchHit],
    root_notes_options: crate::notes::RootNotesSectionOptions,
    root_clipboard_history_hits: &[crate::clipboard_history::ClipboardEntryMeta],
    root_clipboard_history_options: crate::clipboard_history::RootClipboardHistorySectionOptions,
    root_dictation_history_hits: &[crate::dictation::RootDictationHistorySearchHit],
    root_dictation_history_options: crate::dictation::RootDictationHistorySectionOptions,
    root_agent_chat_history_hits: &[crate::ai::agent_chat::ui::history::AgentChatHistorySearchHit],
    root_agent_chat_history_options: crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions,
    root_ai_vault_hits: &[crate::ai_vault::AiVaultHit],
    root_ai_vault_options: crate::ai_vault::RootAiVaultSectionOptions,
    root_browser_tab_hits: &[crate::browser_tabs::RootBrowserTabSearchHit],
    root_browser_tabs_options: crate::browser_tabs::RootBrowserTabsSectionOptions,
    root_browser_history_hits: &[crate::browser_history::RootBrowserHistorySearchHit],
    root_browser_history_options: crate::browser_history::RootBrowserHistorySectionOptions,
    root_passive_source_order: &[crate::config::UnifiedSearchPassiveSource],
    root_passive_result_limits: crate::config::UnifiedSearchPassiveResultLimitsConfig,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    let (mut grouped, mut flat_results) = get_grouped_results_with_validation_and_query(
        scripts,
        scriptlets,
        builtins,
        apps,
        skills,
        flows,
        flow_discovery,
        frecency_store,
        filter_text,
        suggested_config,
        menu_bar_items,
        menu_bar_bundle_id,
        input_history,
        validation,
        advanced_query,
    );
    if root_source_filters.active() {
        filter_grouped_results_by_root_sources(
            &mut grouped,
            &mut flat_results,
            root_source_filters,
        );
    }
    if root_source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Windows) {
        append_root_windows_section(
            &mut grouped,
            &mut flat_results,
            windows,
            root_windows_provider_status,
            filter_text,
            advanced_query,
            root_source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Windows),
        );
    }

    if root_source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Files) {
        append_root_file_section(
            &mut grouped,
            &mut flat_results,
            root_file_search_mode,
            root_file_search_loading,
            root_file_results,
            root_recent_file_results,
            filter_text,
            frecency_store,
            advanced_query,
            root_file_options,
            root_source_filters.active(),
        );
        append_recent_root_file_section(
            &mut grouped,
            &mut flat_results,
            root_recent_file_results,
            filter_text,
            advanced_query,
            root_file_options,
        );
    }
    let mut passive_budget =
        RootPassiveResultBudget::for_results(&flat_results, root_passive_result_limits);
    let browser_tabs_domain_intent =
        !root_source_filters.active() && crate::browser_tabs::query_is_bare_domain(filter_text);
    for source in root_passive_source_order {
        match source {
            crate::config::UnifiedSearchPassiveSource::Todos => {
                if !root_source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Todo) {
                    continue;
                }
                append_root_todos_section(
                    &mut grouped,
                    &mut flat_results,
                    filter_text,
                    advanced_query,
                    root_todo_hits,
                    root_todo_options,
                    &mut passive_budget,
                    root_source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Todo),
                );
            }
            crate::config::UnifiedSearchPassiveSource::BrowserTabs => {
                if !root_source_filters
                    .allows(crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs)
                {
                    continue;
                }
                append_root_browser_tabs_section(
                    &mut grouped,
                    &mut flat_results,
                    filter_text,
                    advanced_query,
                    root_browser_tab_hits,
                    root_browser_tabs_options.clone(),
                    &mut passive_budget,
                    root_source_filters
                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs),
                    browser_tabs_domain_intent,
                );
            }
            crate::config::UnifiedSearchPassiveSource::Brain => {
                if !root_source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Brain) {
                    continue;
                }
                append_root_brain_section(
                    &mut grouped,
                    &mut flat_results,
                    filter_text,
                    advanced_query,
                    root_brain_hits,
                    root_brain_options,
                    &mut passive_budget,
                    root_source_filters
                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::Brain),
                );
            }
            crate::config::UnifiedSearchPassiveSource::Notes => {
                if !root_source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Notes) {
                    continue;
                }
                append_root_notes_section(
                    &mut grouped,
                    &mut flat_results,
                    filter_text,
                    advanced_query,
                    root_note_hits,
                    root_notes_options,
                    &mut passive_budget,
                    root_source_filters
                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::Notes),
                );
            }
            crate::config::UnifiedSearchPassiveSource::ClipboardHistory => {
                if !root_source_filters
                    .allows(crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory)
                {
                    continue;
                }
                append_root_clipboard_history_section(
                    &mut grouped,
                    &mut flat_results,
                    filter_text,
                    advanced_query,
                    root_clipboard_history_hits,
                    root_clipboard_history_options,
                    &mut passive_budget,
                    root_source_filters
                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory),
                );
            }
            crate::config::UnifiedSearchPassiveSource::DictationHistory => {
                if !root_source_filters
                    .allows(crate::menu_syntax::RootUnifiedSourceFilter::Dictation)
                {
                    continue;
                }
                append_root_dictation_history_section(
                    &mut grouped,
                    &mut flat_results,
                    filter_text,
                    advanced_query,
                    root_dictation_history_hits,
                    root_dictation_history_options,
                    &mut passive_budget,
                    root_source_filters
                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::Dictation),
                );
            }
            crate::config::UnifiedSearchPassiveSource::AgentChatHistory => {
                if !root_source_filters
                    .allows(crate::menu_syntax::RootUnifiedSourceFilter::Conversations)
                {
                    continue;
                }
                append_root_agent_chat_history_section(
                    &mut grouped,
                    &mut flat_results,
                    filter_text,
                    advanced_query,
                    root_agent_chat_history_hits,
                    root_agent_chat_history_options,
                    &mut passive_budget,
                    root_source_filters
                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::Conversations),
                );
            }
            crate::config::UnifiedSearchPassiveSource::AiVault => {
                if !root_source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::AiVault)
                {
                    continue;
                }
                append_root_ai_vault_section(
                    &mut grouped,
                    &mut flat_results,
                    filter_text,
                    advanced_query,
                    root_ai_vault_hits,
                    root_ai_vault_options.clone(),
                    &mut passive_budget,
                    root_source_filters
                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::AiVault),
                );
            }
            crate::config::UnifiedSearchPassiveSource::BrowserHistory => {
                if !root_source_filters
                    .allows(crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory)
                {
                    continue;
                }
                append_root_browser_history_section(
                    &mut grouped,
                    &mut flat_results,
                    filter_text,
                    advanced_query,
                    root_browser_history_hits,
                    root_browser_history_options.clone(),
                    &mut passive_budget,
                    root_source_filters
                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory),
                );
            }
        }
    }

    append_missing_explicit_source_status_rows(&mut grouped, &flat_results, root_source_filters);

    (grouped, flat_results)
}

/// Every explicitly-included source must leave visible feedback. Passive
/// sections early-return before building their status row when an
/// eligibility or advanced-query guard fires, so an explicit filter (e.g.
/// `history: tabs: foo` where only tabs matched) could otherwise vanish
/// silently while other sections render. Base sources (Apps/Scripts/
/// Commands) are covered by `append_base_source_status_rows`.
///
/// Skipped when the list has no selectable rows at all — the launcher's
/// zero-result info state is the better surface for that case.
fn append_missing_explicit_source_status_rows(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &[SearchResult],
    root_source_filters: &crate::menu_syntax::RootUnifiedSourceFilterSet,
) {
    if !grouped
        .iter()
        .any(|item| matches!(item, GroupedListItem::Item(_)))
    {
        return;
    }
    for source in root_source_filters.positive_includes() {
        if matches!(
            source,
            crate::menu_syntax::RootUnifiedSourceFilter::Apps
                | crate::menu_syntax::RootUnifiedSourceFilter::Scripts
                | crate::menu_syntax::RootUnifiedSourceFilter::Commands
        ) {
            continue;
        }
        let represented = grouped.iter().any(|item| match item {
            GroupedListItem::Status(status) => status.source == source,
            GroupedListItem::Item(index) => flat_results
                .get(*index)
                .and_then(SearchResult::root_unified_source)
                .is_some_and(|item_source| item_source == source),
            GroupedListItem::SectionHeader(..) => false,
        });
        if !represented {
            grouped.push(GroupedListItem::Status(source_chip_result_status(
                source, 0, 0, false,
            )));
        }
    }
}

fn filter_grouped_results_by_root_sources(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    root_source_filters: &crate::menu_syntax::RootUnifiedSourceFilterSet,
) {
    let mut remap: Vec<Option<usize>> = vec![None; flat_results.len()];
    let mut filtered_results = Vec::new();
    for (old_index, result) in flat_results.iter().enumerate() {
        let allowed = result
            .root_unified_source()
            .is_some_and(|source| root_source_filters.allows(source));
        if allowed {
            let new_index = filtered_results.len();
            remap[old_index] = Some(new_index);
            filtered_results.push(result.clone());
        }
    }

    let mut filtered_grouped = Vec::new();
    let mut pending_header: Option<GroupedListItem> = None;
    for item in grouped.iter() {
        match item {
            GroupedListItem::SectionHeader(label, icon) => {
                pending_header = Some(GroupedListItem::SectionHeader(label.clone(), icon.clone()));
            }
            GroupedListItem::Item(old_index) => {
                if let Some(Some(new_index)) = remap.get(*old_index) {
                    if let Some(header) = pending_header.take() {
                        filtered_grouped.push(header);
                    }
                    filtered_grouped.push(GroupedListItem::Item(*new_index));
                }
            }
            GroupedListItem::Status(status) => {
                if root_source_filters.allows(status.source) {
                    if let Some(header) = pending_header.take() {
                        filtered_grouped.push(header);
                    }
                    filtered_grouped.push(GroupedListItem::Status(status.clone()));
                }
            }
        }
    }

    append_base_source_status_rows(
        &mut filtered_grouped,
        &filtered_results,
        root_source_filters,
    );
    *flat_results = filtered_results;
    *grouped = filtered_grouped;
}

fn append_base_source_status_rows(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &[SearchResult],
    root_source_filters: &crate::menu_syntax::RootUnifiedSourceFilterSet,
) {
    for source in root_source_filters.positive_includes() {
        match source {
            crate::menu_syntax::RootUnifiedSourceFilter::Apps
            | crate::menu_syntax::RootUnifiedSourceFilter::Scripts
            | crate::menu_syntax::RootUnifiedSourceFilter::Commands => {
                let shown = flat_results
                    .iter()
                    .filter(|result| result.root_unified_source() == Some(source))
                    .count();
                grouped.push(GroupedListItem::Status(source_chip_result_status(
                    source, shown, shown, false,
                )));
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RootPassiveResultBudget {
    remaining_total: usize,
    max_per_source: usize,
}

impl RootPassiveResultBudget {
    fn unbounded() -> Self {
        Self {
            remaining_total: usize::MAX,
            max_per_source: usize::MAX,
        }
    }

    fn for_results(
        flat_results: &[SearchResult],
        limits: crate::config::UnifiedSearchPassiveResultLimitsConfig,
    ) -> Self {
        let primary_visible = flat_results.iter().any(is_primary_launcher_result);
        let remaining_total = if primary_visible {
            limits.max_total_results_when_primary_visible
        } else {
            limits.max_total_results
        };
        let max_per_source = if primary_visible {
            limits.max_results_per_source_when_primary_visible
        } else {
            usize::MAX
        };

        Self {
            remaining_total,
            max_per_source,
        }
    }

    fn limit_for_source(&self, source_max: usize) -> usize {
        source_max
            .min(self.remaining_total)
            .min(self.max_per_source)
    }

    fn consume(&mut self, rendered: usize) {
        self.remaining_total = self.remaining_total.saturating_sub(rendered);
    }
}

fn append_root_passive_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    label: &'static str,
    rows: Vec<SearchResult>,
    status: Option<SourceChipStatusRow>,
) {
    let insertion_index = root_file_passive_insertion_index(grouped, flat_results);
    append_root_passive_section_at(grouped, flat_results, label, rows, status, insertion_index);
}

fn append_root_passive_section_at(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    label: &'static str,
    rows: Vec<SearchResult>,
    status: Option<SourceChipStatusRow>,
    insertion_index: usize,
) {
    if rows.is_empty() && status.is_none() {
        return;
    }

    let mut grouped_rows = Vec::with_capacity(rows.len() + 2);
    grouped_rows.push(GroupedListItem::SectionHeader(label.to_string(), None));
    for row in rows {
        let idx = flat_results.len();
        flat_results.push(row);
        grouped_rows.push(GroupedListItem::Item(idx));
    }
    if let Some(status) = status {
        grouped_rows.push(GroupedListItem::Status(status));
    }
    grouped.splice(insertion_index..insertion_index, grouped_rows);
}

/// Insertion index for the "From Your Brain" section. Brain memories are
/// real matches, so when the files section holds nothing but the
/// "Search Files for …" handoff CTA (no actual file results yet), the brain
/// section is inserted ABOVE it — an exact memory must outrank a generic
/// redirect row (audit finding F2). With real file results present, brain
/// keeps its usual passive position.
fn root_brain_passive_insertion_index(
    grouped: &[GroupedListItem],
    flat_results: &[SearchResult],
) -> usize {
    let default_index = root_file_passive_insertion_index(grouped, flat_results);

    let is_file_handoff = |idx: &usize| {
        matches!(
            flat_results.get(*idx),
            Some(SearchResult::Fallback(fm))
                if fm
                    .stable_selection_key_override
                    .as_deref()
                    .is_some_and(|key| key.starts_with("fallback/root-file-search-handoff"))
        )
    };

    let mut section_start: Option<usize> = None;
    let mut item_indices: Vec<usize> = Vec::new();
    for (pos, entry) in grouped.iter().enumerate() {
        match entry {
            GroupedListItem::SectionHeader(_, _) => {
                if let Some(start) = section_start {
                    if !item_indices.is_empty() && item_indices.iter().all(is_file_handoff) {
                        return start.min(default_index);
                    }
                }
                section_start = Some(pos);
                item_indices.clear();
            }
            GroupedListItem::Item(idx) => item_indices.push(*idx),
            GroupedListItem::Status(_) => {}
        }
    }
    if let Some(start) = section_start {
        if !item_indices.is_empty() && item_indices.iter().all(is_file_handoff) {
            return start.min(default_index);
        }
    }
    default_index
}

#[allow(clippy::too_many_arguments)]
fn append_root_agent_chat_history_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    hits: &[crate::ai::agent_chat::ui::history::AgentChatHistorySearchHit],
    options: crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions,
    budget: &mut RootPassiveResultBudget,
    explicit_source_filter: bool,
) {
    if advanced_query.is_some()
        || !crate::ai::agent_chat::ui::history::root_agent_chat_history_query_is_eligible(
            filter_text,
            options,
        )
    {
        return;
    }

    let limit = budget.limit_for_source(options.max_results);
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let rows = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank, hit)| {
            let entry = hit.entry.clone();
            // Hidden-led matches say why they qualified instead of showing
            // an unrelated preview with no visible match.
            let hidden_excerpt = hit
                .evidence
                .as_ref()
                .filter(|evidence| {
                    evidence.title_indices.is_empty() && evidence.subtitle_indices.is_empty()
                })
                .and_then(|evidence| evidence.hidden_excerpt.as_ref());
            let subtitle = match hidden_excerpt {
                Some(excerpt) => format!(
                    "Transcript match · {} · {} message{}",
                    excerpt.text,
                    entry.message_count,
                    if entry.message_count == 1 { "" } else { "s" }
                ),
                None => format!(
                    "{} · {} message{}",
                    entry.preview_display(),
                    entry.message_count,
                    if entry.message_count == 1 { "" } else { "s" }
                ),
            };
            SearchResult::AgentChatHistory(crate::scripts::AgentChatHistoryMatch {
                entry,
                score: root_passive_result_score(rank),
                matched_field: hit.matched_field,
                subtitle,
                evidence: hit.evidence.clone(),
            })
        })
        .collect::<Vec<_>>();

    budget.consume(rows.len());
    let status = explicit_source_filter.then(|| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::Conversations,
            rows.len(),
            hits.len(),
            false,
        )
    });
    append_root_passive_section(
        grouped,
        flat_results,
        "Agent Chat Conversations",
        rows,
        status,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_root_brain_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    hits: &[crate::brain::RootBrainSearchHit],
    options: crate::brain::RootBrainSectionOptions,
    budget: &mut RootPassiveResultBudget,
    explicit_source_filter: bool,
) {
    if advanced_query.is_some() || !crate::brain::root_brain_query_is_eligible(filter_text, options)
    {
        return;
    }

    let limit = budget.limit_for_source(options.max_results);
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let rows = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank, hit)| {
            let subtitle = if hit.excerpt.is_empty() {
                hit.source_label.to_string()
            } else {
                format!("{} · {}", hit.source_label, hit.excerpt)
            };
            SearchResult::BrainHit(crate::scripts::BrainMatch {
                hit: hit.clone(),
                subtitle,
                score: root_passive_result_score(rank),
            })
        })
        .collect::<Vec<_>>();

    budget.consume(rows.len());
    let status = explicit_source_filter.then(|| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::Brain,
            rows.len(),
            hits.len(),
            false,
        )
    });
    let insertion_index = root_brain_passive_insertion_index(grouped, flat_results);
    append_root_passive_section_at(
        grouped,
        flat_results,
        "From Your Brain",
        rows,
        status,
        insertion_index,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_root_notes_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    hits: &[crate::notes::RootNoteSearchHit],
    options: crate::notes::RootNotesSectionOptions,
    budget: &mut RootPassiveResultBudget,
    explicit_source_filter: bool,
) {
    if advanced_query.is_some() || !crate::notes::root_notes_query_is_eligible(filter_text, options)
    {
        return;
    }

    let limit = budget.limit_for_source(options.max_results);
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let rows = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank, hit)| {
            let title = if hit.title.trim().is_empty() {
                "Untitled Note".to_string()
            } else {
                hit.title.clone()
            };
            let pinned = if hit.is_pinned { "Pinned · " } else { "" };
            let updated = crate::formatting::format_relative_time_short_dt(hit.updated_at);
            SearchResult::Note(crate::scripts::NoteMatch {
                hit: hit.clone(),
                title,
                subtitle: format!("{pinned}Updated {updated} · {} chars", hit.char_count),
                score: root_passive_result_score(rank),
            })
        })
        .collect::<Vec<_>>();

    budget.consume(rows.len());
    let status = explicit_source_filter.then(|| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::Notes,
            rows.len(),
            hits.len(),
            false,
        )
    });
    append_root_passive_section(grouped, flat_results, "Notes", rows, status);
}

#[allow(clippy::too_many_arguments)]
fn append_root_todos_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    hits: &[crate::menu_syntax::RootTodoSearchHit],
    options: crate::menu_syntax::RootTodoSectionOptions,
    budget: &mut RootPassiveResultBudget,
    explicit_source_filter: bool,
) {
    if !crate::menu_syntax::root_todo_query_is_eligible(filter_text, options) {
        return;
    }

    let limit = budget.limit_for_source(options.max_results);
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let mut rows = hits
        .iter()
        .enumerate()
        .map(|(rank, hit)| {
            SearchResult::Todo(crate::scripts::TodoMatch {
                hit: hit.clone(),
                score: root_passive_result_score(rank),
            })
        })
        .collect::<Vec<_>>();
    if let Some(query) = advanced_query {
        rows = crate::menu_syntax::apply_advanced_query(rows, query);
    }
    rows.truncate(limit);

    budget.consume(rows.len());
    let status = explicit_source_filter.then(|| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::Todo,
            rows.len(),
            hits.len(),
            false,
        )
    });
    append_root_passive_section(grouped, flat_results, "Todos", rows, status);
}

#[allow(clippy::too_many_arguments)]
fn append_root_clipboard_history_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    hits: &[crate::clipboard_history::ClipboardEntryMeta],
    options: crate::clipboard_history::RootClipboardHistorySectionOptions,
    budget: &mut RootPassiveResultBudget,
    explicit_source_filter: bool,
) {
    if advanced_query.is_some()
        || !crate::clipboard_history::root_clipboard_history_query_is_eligible(filter_text, options)
    {
        return;
    }

    let limit = budget.limit_for_source(options.max_results);
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let rows = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank, entry)| {
            let content_type = match entry.content_type {
                crate::clipboard_history::ContentType::Text => "Text",
                crate::clipboard_history::ContentType::Link => "Link",
                crate::clipboard_history::ContentType::File => "File",
                crate::clipboard_history::ContentType::Color => "Color",
                crate::clipboard_history::ContentType::Image => "Image",
            };
            let pinned = if entry.pinned { "Pinned · " } else { "" };
            let time = crate::formatting::format_relative_time_short_millis(entry.timestamp);
            SearchResult::ClipboardHistory(crate::scripts::ClipboardHistoryMatch {
                entry: entry.clone(),
                title: entry.display_preview(),
                subtitle: format!("{pinned}{content_type} · {time}"),
                score: root_passive_result_score(rank),
            })
        })
        .collect::<Vec<_>>();

    budget.consume(rows.len());
    let status = explicit_source_filter.then(|| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory,
            rows.len(),
            hits.len(),
            false,
        )
    });
    append_root_passive_section(grouped, flat_results, "Clipboard History", rows, status);
}

#[allow(clippy::too_many_arguments)]
fn append_root_dictation_history_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    hits: &[crate::dictation::RootDictationHistorySearchHit],
    options: crate::dictation::RootDictationHistorySectionOptions,
    budget: &mut RootPassiveResultBudget,
    explicit_source_filter: bool,
) {
    if advanced_query.is_some()
        || !crate::dictation::root_dictation_history_query_is_eligible(filter_text, options)
    {
        return;
    }

    let limit = budget.limit_for_source(options.max_results);
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let rows = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank, hit)| {
            let time = crate::dictation::format_history_timestamp(&hit.timestamp);
            let duration = crate::dictation::format_history_duration_ms(hit.audio_duration_ms);
            // Matches beyond the saved preview surface a transcript excerpt
            // so the row shows why it qualified.
            let hidden_excerpt = hit
                .evidence
                .as_ref()
                .filter(|evidence| {
                    evidence.title_indices.is_empty() && evidence.subtitle_indices.is_empty()
                })
                .and_then(|evidence| evidence.hidden_excerpt.as_ref());
            let subtitle = match hidden_excerpt {
                Some(excerpt) => {
                    format!("Transcript match · {} · {}", excerpt.text, time)
                }
                None => format!("{} · {} · {}", hit.target, duration, time),
            };
            SearchResult::DictationHistory(crate::scripts::DictationHistoryMatch {
                id: hit.id.clone(),
                preview: hit.preview.clone(),
                target: hit.target.clone(),
                timestamp: hit.timestamp.clone(),
                audio_duration_ms: hit.audio_duration_ms,
                subtitle,
                score: root_passive_result_score(rank),
                matched_field: hit.matched_field,
                evidence: hit.evidence.clone(),
            })
        })
        .collect::<Vec<_>>();

    budget.consume(rows.len());
    let status = explicit_source_filter.then(|| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::Dictation,
            rows.len(),
            hits.len(),
            false,
        )
    });
    append_root_passive_section(grouped, flat_results, "Dictation History", rows, status);
}

#[allow(clippy::too_many_arguments)]
fn append_root_browser_tabs_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    hits: &[crate::browser_tabs::RootBrowserTabSearchHit],
    options: crate::browser_tabs::RootBrowserTabsSectionOptions,
    budget: &mut RootPassiveResultBudget,
    explicit_source_filter: bool,
    domain_intent: bool,
) {
    if advanced_query.is_some()
        || !crate::browser_tabs::root_browser_tabs_query_is_eligible(filter_text, options.clone())
    {
        return;
    }

    // A bare domain is strong navigation intent. Give tabs their configured
    // source allowance even when earlier primary/passive rows exhausted the
    // shared discovery budget.
    let limit = if domain_intent {
        options.max_results
    } else {
        budget.limit_for_source(options.max_results)
    };
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let rows = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank, hit)| {
            let subtitle = if hit.domain.is_empty() {
                hit.provider_label.clone()
            } else {
                format!("{} · {}", hit.domain, hit.provider_label)
            };
            SearchResult::BrowserTab(crate::scripts::BrowserTabMatch {
                hit: hit.clone(),
                subtitle,
                score: root_passive_result_score(rank),
            })
        })
        .collect::<Vec<_>>();

    budget.consume(rows.len());
    let status = explicit_source_filter.then(|| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs,
            rows.len(),
            hits.len(),
            false,
        )
    });
    if domain_intent && !rows.is_empty() {
        tracing::info!(
            event = "root_browser_tabs_domain_intent_hoist",
            query = filter_text,
            row_count = rows.len(),
            "hoisting browser tabs for bare-domain query"
        );
        append_root_passive_section_at(grouped, flat_results, "Browser Tabs", rows, status, 0);
    } else {
        append_root_passive_section(grouped, flat_results, "Browser Tabs", rows, status);
    }
}

#[allow(clippy::too_many_arguments)]
fn append_root_browser_history_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    hits: &[crate::browser_history::RootBrowserHistorySearchHit],
    options: crate::browser_history::RootBrowserHistorySectionOptions,
    budget: &mut RootPassiveResultBudget,
    explicit_source_filter: bool,
) {
    if advanced_query.is_some()
        || !crate::browser_history::root_browser_history_query_is_eligible(
            filter_text,
            options.clone(),
        )
    {
        return;
    }

    let limit = budget.limit_for_source(options.max_results);
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let rows = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank, hit)| {
            let time = crate::formatting::format_relative_time_short_millis(hit.last_visit_unix_ms);
            SearchResult::BrowserHistory(crate::scripts::BrowserHistoryMatch {
                hit: hit.clone(),
                subtitle: format!(
                    "{} · {}/{} · {}",
                    hit.domain, hit.provider_label, hit.profile_label, time
                ),
                score: root_passive_result_score(rank),
            })
        })
        .collect::<Vec<_>>();

    budget.consume(rows.len());
    let status = explicit_source_filter.then(|| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory,
            rows.len(),
            hits.len(),
            false,
        )
    });
    append_root_passive_section(grouped, flat_results, "Browser History", rows, status);
}

fn append_recent_root_file_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    recent_file_results: &[crate::file_search::FileResult],
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    options: crate::file_search::RootFileSectionOptions,
) {
    if !options.files_enabled || !options.recent_files_enabled {
        return;
    }
    if advanced_query.is_some() || !filter_text.trim().is_empty() || recent_file_results.is_empty()
    {
        return;
    }

    let loaded_recent_files = recent_file_results
        .iter()
        .filter(|file| {
            crate::file_search::root_global_file_result_is_eligible(file)
                && file.file_type != crate::file_search::FileType::Directory
        })
        .count();
    let eligible_recent_files = recent_file_results
        .iter()
        .filter(|file| {
            crate::file_search::root_global_file_result_is_eligible(file)
                && file.file_type != crate::file_search::FileType::Directory
        })
        .take(
            options
                .source_filter_browse_target_visible_rows
                .unwrap_or(crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT),
        )
        .collect::<Vec<_>>();
    let source_status = options.source_chip_visible_limit.map(|_| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::Files,
            eligible_recent_files.len(),
            loaded_recent_files,
            false,
        )
    });
    if eligible_recent_files.is_empty() && source_status.is_none() {
        return;
    }

    let insertion_index = root_file_passive_insertion_index(grouped, flat_results);

    let mut recent_group = Vec::with_capacity(eligible_recent_files.len() + 2);
    recent_group.push(GroupedListItem::SectionHeader(
        "Recent Files".to_string(),
        None,
    ));
    for (rank, file) in eligible_recent_files.into_iter().enumerate() {
        let idx = flat_results.len();
        flat_results.push(SearchResult::File(crate::scripts::FileMatch {
            file: file.clone(),
            score: i32::MAX.saturating_sub(rank as i32),
        }));
        recent_group.push(GroupedListItem::Item(idx));
    }
    if let Some(status) = source_status {
        recent_group.push(GroupedListItem::Status(status));
    }

    grouped.splice(insertion_index..insertion_index, recent_group);
}

fn source_chip_status_row(
    source: crate::menu_syntax::RootUnifiedSourceFilter,
    status_kind: SourceChipStatusKind,
    shown: usize,
    loaded: usize,
    total: Option<usize>,
    label: String,
) -> SourceChipStatusRow {
    SourceChipStatusRow {
        source,
        source_name: source.label().to_string(),
        status_kind,
        label,
        shown,
        loaded,
        total,
    }
}

fn source_chip_result_status(
    source: crate::menu_syntax::RootUnifiedSourceFilter,
    shown: usize,
    loaded: usize,
    loading: bool,
) -> SourceChipStatusRow {
    if loading {
        return source_chip_status_row(
            source,
            SourceChipStatusKind::Loading,
            shown,
            loaded,
            None,
            "Loading more...".to_string(),
        );
    }

    if shown == 0 {
        return source_chip_status_row(
            source,
            SourceChipStatusKind::Exhausted,
            shown,
            loaded,
            Some(loaded),
            "No results".to_string(),
        );
    }

    let capped = loaded > shown;
    let label = if capped {
        format!("Showing {shown} of {loaded}")
    } else {
        format!("Showing {shown} of {loaded} · No more results")
    };
    source_chip_status_row(
        source,
        if capped {
            SourceChipStatusKind::Showing
        } else {
            SourceChipStatusKind::Exhausted
        },
        shown,
        loaded,
        Some(loaded),
        label,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_root_ai_vault_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    hits: &[crate::ai_vault::AiVaultHit],
    options: crate::ai_vault::RootAiVaultSectionOptions,
    budget: &mut RootPassiveResultBudget,
    explicit_source_filter: bool,
) {
    if advanced_query.is_some()
        || !crate::ai_vault::root_ai_vault_query_is_eligible(filter_text, &options)
    {
        return;
    }

    let limit = budget.limit_for_source(options.max_results);
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let rows = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank, hit): (usize, &crate::ai_vault::AiVaultHit)| {
            SearchResult::AiVault(crate::scripts::AiVaultMatch {
                hit: hit.clone(),
                subtitle: ai_vault_subtitle(hit),
                score: root_passive_result_score(rank),
            })
        })
        .collect::<Vec<_>>();

    budget.consume(rows.len());
    let status = explicit_source_filter.then(|| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::AiVault,
            rows.len(),
            hits.len(),
            false,
        )
    });
    append_root_passive_section(grouped, flat_results, "AI Vault", rows, status);
}

fn ai_vault_subtitle(hit: &crate::ai_vault::AiVaultHit) -> String {
    vec![
        hit.provider_display_name.as_str(),
        hit.model.as_deref().unwrap_or(""),
        hit.workspace_path.as_deref().unwrap_or(""),
        hit.modified_at.as_deref().unwrap_or(""),
    ]
    .into_iter()
    .filter(|part: &&str| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

#[allow(clippy::too_many_arguments)]
fn append_root_file_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    root_file_search_mode: Option<crate::file_search::RootFileSectionMode>,
    root_file_search_loading: bool,
    root_file_results: &[crate::file_search::FileResult],
    root_recent_file_results: &[crate::file_search::FileResult],
    filter_text: &str,
    frecency_store: &FrecencyStore,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    options: crate::file_search::RootFileSectionOptions,
    suppress_handoff: bool,
) {
    if !options.files_enabled {
        return;
    }
    let Some(mode) = root_file_search_mode else {
        return;
    };
    if advanced_query.is_some() {
        return;
    }
    match mode {
        crate::file_search::RootFileSectionMode::GlobalQuery if !options.global_search_enabled => {
            return;
        }
        crate::file_search::RootFileSectionMode::DirectoryBrowse
            if !options.directory_browse_enabled =>
        {
            return;
        }
        _ => {}
    }

    let files = match mode {
        crate::file_search::RootFileSectionMode::GlobalQuery => {
            let merged = merge_root_global_file_results_with_recent(
                root_file_results,
                root_recent_file_results,
                filter_text,
                options.query_intent,
            );
            let visible_limit = options
                .source_chip_visible_limit
                .unwrap_or(crate::file_search::ROOT_FILE_RENDER_LIMIT);
            crate::file_search::rank_root_file_results(&merged, filter_text, visible_limit, |key| {
                frecency_store.get_score(key)
            })
        }
        crate::file_search::RootFileSectionMode::DirectoryBrowse => {
            let child_filter = root_directory_browse_child_filter(filter_text);
            let visible_limit = options
                .source_chip_visible_limit
                .unwrap_or(crate::file_search::ROOT_FILE_BROWSE_RENDER_LIMIT);
            crate::file_search::root_directory_file_matches(
                root_file_results,
                child_filter.as_deref(),
                visible_limit,
            )
        }
    };
    let loaded_file_count = match mode {
        crate::file_search::RootFileSectionMode::GlobalQuery => {
            merge_root_global_file_results_with_recent(
                root_file_results,
                root_recent_file_results,
                filter_text,
                options.query_intent,
            )
            .len()
        }
        crate::file_search::RootFileSectionMode::DirectoryBrowse => root_file_results.len(),
    };
    let ui_state = RootFileSectionUiState::new(
        filter_text,
        mode,
        options.query_intent,
        root_file_search_loading,
        root_file_search_loading,
        options.source_chip_visible_limit.is_some(),
        files.len(),
        loaded_file_count,
        root_recent_file_results.len(),
        !suppress_handoff,
    );
    let handoff = if suppress_handoff {
        None
    } else {
        root_file_search_handoff_result(filter_text, mode, options.query_intent, &ui_state)
    };
    let source_status = options.source_chip_visible_limit.map(|_| {
        source_chip_result_status(
            crate::menu_syntax::RootUnifiedSourceFilter::Files,
            files.len(),
            loaded_file_count,
            root_file_search_loading,
        )
    });
    if files.is_empty() && handoff.is_none() && source_status.is_none() {
        return;
    }

    let promote = root_file_section_should_promote(
        options.promotion_policy,
        mode,
        root_file_search_loading,
        filter_text,
        &files,
        flat_results,
    );
    let insertion_index = root_file_section_insertion_index(grouped, flat_results, promote);

    let mut file_group = Vec::with_capacity(files.len() + 3);
    ui_state.log_built();
    file_group.push(GroupedListItem::SectionHeader(
        ui_state.section_label.clone(),
        None,
    ));
    for file_match in files {
        let idx = flat_results.len();
        flat_results.push(SearchResult::File(file_match));
        file_group.push(GroupedListItem::Item(idx));
    }
    if let Some(handoff) = handoff {
        let idx = flat_results.len();
        flat_results.push(handoff);
        file_group.push(GroupedListItem::Item(idx));
    }
    if let Some(status) = source_status {
        file_group.push(GroupedListItem::Status(status));
    }
    grouped.splice(insertion_index..insertion_index, file_group);
}

fn root_file_section_should_promote(
    policy: crate::file_search::RootFilePromotionPolicy,
    mode: crate::file_search::RootFileSectionMode,
    root_file_search_loading: bool,
    filter_text: &str,
    files: &[crate::scripts::FileMatch],
    flat_results: &[SearchResult],
) -> bool {
    if policy == crate::file_search::RootFilePromotionPolicy::Never {
        return false;
    }
    if root_file_search_loading {
        return false;
    }
    if mode != crate::file_search::RootFileSectionMode::GlobalQuery {
        return false;
    }

    let query = filter_text.trim();
    if !crate::file_search::root_file_global_query_is_eligible(query) {
        return false;
    }

    if flat_results.iter().any(is_primary_launcher_result) {
        return false;
    }

    let Some(first_file) = files.first() else {
        return false;
    };

    match policy {
        crate::file_search::RootFilePromotionPolicy::Never => false,
        crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly => {
            crate::file_search::root_file_name_exact_or_stem_matches_query(
                &first_file.file.name,
                query,
            )
        }
    }
}

fn is_primary_launcher_result(result: &SearchResult) -> bool {
    matches!(
        result,
        SearchResult::Script(_)
            | SearchResult::Scriptlet(_)
            | SearchResult::Skill(_)
            | SearchResult::BuiltIn(_)
            | SearchResult::App(_)
            | SearchResult::Window(_)
    )
}

fn root_file_section_insertion_index(
    grouped: &[GroupedListItem],
    flat_results: &[SearchResult],
    promote: bool,
) -> usize {
    if promote {
        return match grouped.first() {
            Some(GroupedListItem::Item(result_idx))
                if matches!(
                    flat_results.get(*result_idx),
                    Some(SearchResult::ScriptIssue(_))
                ) =>
            {
                1
            }
            _ => 0,
        };
    }

    root_file_passive_insertion_index(grouped, flat_results)
}

fn root_file_passive_insertion_index(
    grouped: &[GroupedListItem],
    _flat_results: &[SearchResult],
) -> usize {
    grouped
        .iter()
        .position(|item| match item {
            GroupedListItem::Item(_) => false,
            GroupedListItem::Status(_) => false,
            GroupedListItem::SectionHeader(label, None) => {
                label.starts_with("Use \"") && label.ends_with("\" with...")
            }
            GroupedListItem::SectionHeader(_, Some(_)) => false,
        })
        .unwrap_or(grouped.len())
}

fn append_root_windows_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    windows: &[crate::scripts::RootWindowEntry],
    provider_status: crate::window_control::RootWindowsProviderStatus,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    explicit_source_filter: bool,
) {
    if advanced_query.is_some_and(|query| query.has_predicates()) {
        return;
    }

    let source = crate::menu_syntax::RootUnifiedSourceFilter::Windows;
    if explicit_source_filter {
        match provider_status {
            crate::window_control::RootWindowsProviderStatus::PermissionRequired => {
                grouped.push(GroupedListItem::SectionHeader("Windows".to_string(), None));
                grouped.push(GroupedListItem::Status(source_chip_status_row(
                    source,
                    SourceChipStatusKind::ProviderUnavailable,
                    0,
                    0,
                    None,
                    "Accessibility permission required to list windows".to_string(),
                )));
                return;
            }
            crate::window_control::RootWindowsProviderStatus::ProviderError { message } => {
                grouped.push(GroupedListItem::SectionHeader("Windows".to_string(), None));
                grouped.push(GroupedListItem::Status(source_chip_status_row(
                    source,
                    SourceChipStatusKind::ProviderUnavailable,
                    0,
                    0,
                    None,
                    format!("Window provider failed: {message}"),
                )));
                return;
            }
            crate::window_control::RootWindowsProviderStatus::Unknown
            | crate::window_control::RootWindowsProviderStatus::Refreshing { .. }
            | crate::window_control::RootWindowsProviderStatus::Ready { .. } => {}
        }
    }

    let matches = fuzzy_search_root_windows(windows, filter_text);
    if matches.is_empty() && !explicit_source_filter {
        return;
    }

    grouped.push(GroupedListItem::SectionHeader("Windows".to_string(), None));
    let shown = matches.len();
    for window_match in matches {
        let idx = flat_results.len();
        flat_results.push(SearchResult::Window(window_match));
        grouped.push(GroupedListItem::Item(idx));
    }
    if explicit_source_filter {
        let status = match provider_status {
            crate::window_control::RootWindowsProviderStatus::Ready { count } if count == 0 => {
                source_chip_status_row(
                    source,
                    SourceChipStatusKind::Exhausted,
                    shown,
                    count,
                    Some(count),
                    "No windows found".to_string(),
                )
            }
            crate::window_control::RootWindowsProviderStatus::Ready { count }
                if shown == 0 && count > 0 =>
            {
                let query = filter_text.trim();
                source_chip_status_row(
                    source,
                    SourceChipStatusKind::Exhausted,
                    shown,
                    count,
                    Some(count),
                    format!("No window matches \"{query}\""),
                )
            }
            crate::window_control::RootWindowsProviderStatus::Ready { count } => {
                source_chip_result_status(source, shown, count, false)
            }
            crate::window_control::RootWindowsProviderStatus::Refreshing { count }
                if shown == 0 && count == 0 =>
            {
                source_chip_status_row(
                    source,
                    SourceChipStatusKind::Loading,
                    shown,
                    count,
                    Some(count),
                    "Loading windows...".to_string(),
                )
            }
            crate::window_control::RootWindowsProviderStatus::Refreshing { count } => {
                source_chip_status_row(
                    source,
                    SourceChipStatusKind::Loading,
                    shown,
                    count,
                    Some(count),
                    "Refreshing windows...".to_string(),
                )
            }
            crate::window_control::RootWindowsProviderStatus::Unknown => {
                source_chip_result_status(source, shown, shown, false)
            }
            crate::window_control::RootWindowsProviderStatus::PermissionRequired
            | crate::window_control::RootWindowsProviderStatus::ProviderError { .. } => {
                unreachable!("provider failures return before fuzzy window grouping")
            }
        };
        grouped.push(GroupedListItem::Status(status));
    }
}

fn merge_root_global_file_results_with_recent(
    provider_results: &[crate::file_search::FileResult],
    recent_results: &[crate::file_search::FileResult],
    filter_text: &str,
    query_intent: crate::file_search::RootFileQueryIntent,
) -> Vec<crate::file_search::FileResult> {
    let mut seen = std::collections::HashSet::new();
    let mut merged = Vec::with_capacity(provider_results.len() + recent_results.len());

    for file in provider_results
        .iter()
        .filter(|file| crate::file_search::root_global_file_result_is_eligible(file))
    {
        if seen.insert(file.path.clone()) {
            merged.push(file.clone());
        }
    }
    for file in recent_results.iter().filter(|file| {
        crate::file_search::root_global_file_result_is_eligible(file)
            && crate::file_search::root_file_recent_seed_matches_query_for_intent(
                file,
                filter_text,
                query_intent,
            )
    }) {
        if seen.insert(file.path.clone()) {
            merged.push(file.clone());
        }
    }

    merged
}

#[derive(Debug, Clone)]
struct RootFileSectionUiState {
    query: String,
    mode: crate::file_search::RootFileSectionMode,
    match_mode: Option<crate::file_search::RootFileInlineMatchMode>,
    visible_loading: bool,
    provider_loading: bool,
    explicit_files_source: bool,
    visible_file_count: usize,
    loaded_file_count: usize,
    recent_seed_count: usize,
    handoff_visible: bool,
    section_label: String,
    handoff_subtitle: String,
    source_status_label: Option<String>,
}

impl RootFileSectionUiState {
    #[allow(clippy::too_many_arguments)]
    fn new(
        query: &str,
        mode: crate::file_search::RootFileSectionMode,
        intent: crate::file_search::RootFileQueryIntent,
        visible_loading: bool,
        provider_loading: bool,
        explicit_files_source: bool,
        visible_file_count: usize,
        loaded_file_count: usize,
        recent_seed_count: usize,
        handoff_visible: bool,
    ) -> Self {
        let query = query.trim().to_string();
        let match_mode = crate::file_search::root_file_inline_match_mode_for_query(&query, intent);
        let section_label = match_mode
            .map(crate::file_search::RootFileInlineMatchMode::section_label)
            .unwrap_or("Files")
            .to_string();
        let handoff_subtitle = match_mode
            .map(crate::file_search::RootFileInlineMatchMode::handoff_subtitle)
            .unwrap_or("Open full File Search")
            .to_string();
        let source_status_label = explicit_files_source.then(|| {
            if visible_loading {
                "Searching files...".to_string()
            } else if loaded_file_count == 0 {
                "No files found".to_string()
            } else {
                format!("Showing {visible_file_count} of {loaded_file_count} files")
            }
        });

        Self {
            query,
            mode,
            match_mode,
            visible_loading,
            provider_loading,
            explicit_files_source,
            visible_file_count,
            loaded_file_count,
            recent_seed_count,
            handoff_visible,
            section_label,
            handoff_subtitle,
            source_status_label,
        }
    }

    fn log_built(&self) {
        tracing::debug!(
            event = "root_file_ui_state_built",
            query = %self.query,
            mode = ?self.mode,
            match_mode = ?self.match_mode,
            section_label = %self.section_label,
            visible_loading = self.visible_loading,
            provider_loading = self.provider_loading,
            explicit_files_source = self.explicit_files_source,
            visible_file_count = self.visible_file_count,
            loaded_file_count = self.loaded_file_count,
            recent_seed_count = self.recent_seed_count,
            handoff_visible = self.handoff_visible,
            source_status_label = ?self.source_status_label,
        );
    }
}

fn root_file_search_handoff_result(
    filter_text: &str,
    mode: crate::file_search::RootFileSectionMode,
    intent: crate::file_search::RootFileQueryIntent,
    ui_state: &RootFileSectionUiState,
) -> Option<SearchResult> {
    let query = filter_text.trim();
    if crate::file_search::root_file_section_mode_for_query_with_intent(query, intent) != Some(mode)
    {
        return None;
    }

    let fallback = crate::fallbacks::builtins::get_builtin_fallbacks()
        .into_iter()
        .find(|fallback| fallback.id == crate::fallbacks::builtins::SEARCH_FILES_FALLBACK_ID)?;

    let (title, subtitle) = match mode {
        crate::file_search::RootFileSectionMode::GlobalQuery => (
            format!("Search Files for \"{query}\""),
            ui_state.handoff_subtitle.clone(),
        ),
        crate::file_search::RootFileSectionMode::DirectoryBrowse => {
            let base = crate::file_search::root_directory_query_base(query)?;
            let label = crate::file_search::shorten_path(base.trim_end_matches('/'));
            (
                format!("Open File Search in \"{label}\""),
                ui_state.handoff_subtitle.clone(),
            )
        }
    };

    Some(SearchResult::Fallback(
        FallbackMatch::new(crate::fallbacks::FallbackItem::Builtin(fallback), 0)
            .with_display_overrides(title, subtitle)
            .with_stable_selection_key(match mode {
                crate::file_search::RootFileSectionMode::GlobalQuery => {
                    "fallback/root-file-search-handoff/global"
                }
                crate::file_search::RootFileSectionMode::DirectoryBrowse => {
                    "fallback/root-file-search-handoff/directory"
                }
            }),
    ))
}

fn root_directory_browse_child_filter(query: &str) -> Option<String> {
    let query = query.trim();
    let base = crate::file_search::root_directory_query_base(query)?;
    let child_filter = query.strip_prefix(&base)?.trim();
    (!child_filter.is_empty()).then(|| child_filter.to_string())
}

/// Incomplete menu-syntax hint row.
///
/// Returns a single non-selectable `GroupedListItem::SectionHeader(hint, None)`
/// and empty flat results. This is what renders when the user has typed a
/// power-syntax prefix that is not yet a complete invocation — for example
/// `:` (bare advanced query), `+` (bare capture prefix), or `+todo` (known
/// capture target without a body).
///
/// Selection maps through `GroupedListItem::Item(idx)` only, so a header is
/// automatically non-selectable. Do not reuse `SearchResult::ScriptIssue`:
/// that variant is selectable and routes to diagnostics.
pub(crate) fn build_menu_syntax_hint_results(
    hint: &str,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    (
        vec![GroupedListItem::SectionHeader(hint.to_string(), None)],
        Vec::new(),
    )
}

/// Capture-mode grouped results.
///
/// Replaces the normal launcher grouping entirely when the user typed a
/// `+<target>` or `<target>:` capture syntax. Do not mix with Suggested,
/// Favorites, Recent, menu-bar items, calculator, or fallbacks — capture
/// should render only handler scripts that opted into
/// `menuSyntax: [{ family: "capture.v1", targets: [...] }]`.
///
/// Returns a one-section layout:
/// - `SectionHeader("Capture <target>", None)` — always present
/// - `Item(i)` rows, one per handler script, in the order
///   `scripts_handling_capture` returns them (defaults first, then remaining)
///
/// When no handler scripts match, returns a single non-selectable help row
/// explaining that no scripts opted into `capture.v1/<target>`.
pub(crate) fn build_capture_mode_results(
    scripts: &[Arc<Script>],
    invocation: &crate::menu_syntax::CaptureInvocation,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    let handlers = crate::menu_syntax::rank_scripts_handling_capture(scripts, invocation);

    if handlers.is_empty() {
        return (
            vec![GroupedListItem::SectionHeader(
                format!("No scripts opted into capture.v1/{}", invocation.target),
                None,
            )],
            Vec::new(),
        );
    }

    let header = format!("Capture {}", invocation.target);
    let mut grouped: Vec<GroupedListItem> = Vec::with_capacity(handlers.len() + 1);
    grouped.push(GroupedListItem::SectionHeader(header, None));
    let mut flat_results: Vec<SearchResult> = Vec::with_capacity(handlers.len());

    for (idx, script) in handlers.into_iter().enumerate() {
        let filename = script
            .path
            .file_name()
            .map(|f: &std::ffi::OsStr| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        flat_results.push(SearchResult::Script(ScriptMatch {
            script,
            score: i32::MAX,
            filename,
            match_indices: MatchIndices::default(),
            match_kind: ScriptMatchKind::Name,
            content_match: None,
            match_evidence: None,
        }));
        grouped.push(GroupedListItem::Item(idx));
    }

    (grouped, flat_results)
}

/// Returns `true` when `advanced_query` has predicates that would exclude a
/// synthetic `SearchResult::ScriptIssue` row from results. Only the predicates
/// are checked (no free-text substring match), so `:type:script` suppresses the
/// issue row while `:issues` keeps it.
fn advanced_query_rejects_issue(
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
) -> bool {
    let Some(query) = advanced_query else {
        return false;
    };
    if query.predicates.is_empty() {
        return false;
    }
    let synthetic = SearchResult::ScriptIssue(ScriptIssueMatch {
        title: String::new(),
        description: None,
        failed_count: 0,
        fatal_count: 0,
        warning_count: 0,
        score: 0,
    });
    !query
        .predicates
        .iter()
        .all(|p| crate::menu_syntax::matches_predicate(&synthetic, p))
}

#[cfg(test)]
mod advanced_query_tests {
    use super::*;
    use crate::file_search::{FileResult, FileType};
    use crate::menu_syntax::{parse, AdvancedQuery, MenuSyntaxParse};
    use crate::scripts::types::{BuiltInMatch, MatchIndices};

    fn issue_row() -> SearchResult {
        SearchResult::ScriptIssue(ScriptIssueMatch {
            title: "Script Issues (1)".into(),
            description: None,
            failed_count: 1,
            fatal_count: 1,
            warning_count: 0,
            score: i32::MAX,
        })
    }

    fn advanced_query_from(raw: &str) -> AdvancedQuery {
        match parse(raw) {
            MenuSyntaxParse::AdvancedQuery(q) => q,
            other => panic!("expected AdvancedQuery for {raw:?}, got {other:?}"),
        }
    }

    /// Audit finding F2: a brain memory must outrank the generic
    /// "Search Files for …" handoff CTA, so when the files section holds
    /// nothing but that CTA the brain section inserts above it. With any
    /// non-CTA result present, brain keeps the default passive position.
    #[test]
    fn brain_insertion_index_promotes_above_cta_only_files_section() {
        let search_files = crate::fallbacks::builtins::get_builtin_fallbacks()
            .into_iter()
            .find(|f| f.id == crate::fallbacks::builtins::SEARCH_FILES_FALLBACK_ID)
            .expect("search files fallback");
        let handoff = SearchResult::Fallback(
            FallbackMatch::new(
                crate::fallbacks::FallbackItem::Builtin(search_files.clone()),
                0,
            )
            .with_stable_selection_key("fallback/root-file-search-handoff/global"),
        );
        let plain_fallback = SearchResult::Fallback(FallbackMatch::new(
            crate::fallbacks::FallbackItem::Builtin(search_files),
            0,
        ));

        // CTA-only files section: brain inserts above its header (index 0).
        let flat = vec![handoff.clone()];
        let grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(0),
        ];
        assert_eq!(root_brain_passive_insertion_index(&grouped, &flat), 0);

        // Section with a non-CTA row keeps the default (append) position.
        let flat = vec![handoff, plain_fallback];
        let grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
        ];
        assert_eq!(
            root_brain_passive_insertion_index(&grouped, &flat),
            grouped.len()
        );
    }

    #[test]
    fn rejects_issue_under_type_script_predicate() {
        let query = advanced_query_from(":type:script git");
        assert!(advanced_query_rejects_issue(Some(&query)));
    }

    #[test]
    fn allows_issue_under_type_issue_predicate() {
        let query = advanced_query_from(":type:issue");
        assert!(!advanced_query_rejects_issue(Some(&query)));
    }

    #[test]
    fn no_advanced_query_never_rejects_issue() {
        assert!(!advanced_query_rejects_issue(None));
    }

    #[test]
    fn empty_predicates_never_reject_issue() {
        // Grammar pivot (2026-06): a bare `: git` no longer parses as an
        // AdvancedQuery — the leading-colon form stays Incomplete
        // (BareQueryPrefix) until a concrete filter is chosen. A source-filter
        // query like `f: git` is the current spelling that still yields an
        // AdvancedQuery with empty predicates, which is what this invariant
        // is about: no predicates means the ScriptIssue row is never rejected.
        let query = advanced_query_from("f: git");
        assert!(query.predicates.is_empty());
        assert!(!advanced_query_rejects_issue(Some(&query)));
    }

    #[test]
    fn apply_advanced_query_drops_issue_with_type_script() {
        let query = advanced_query_from(":type:script git");
        let results = vec![issue_row()];
        let filtered = crate::menu_syntax::apply_advanced_query(results, &query);
        assert!(
            filtered.is_empty(),
            ":type:script must not leak a ScriptIssue row through grouping"
        );
    }

    #[test]
    fn apply_advanced_query_keeps_issue_with_type_issue() {
        let query = advanced_query_from(":type:issue");
        let results = vec![issue_row()];
        let filtered = crate::menu_syntax::apply_advanced_query(results, &query);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn _compile_time_signature_and_marker() {
        // Compile-time witness that the new entry points exist and accept Option<&AdvancedQuery>.
        // Exercising them at runtime requires fuzzy-search fixtures that pull in heavy state;
        // that surface is covered by the existing scripts::grouping chunked tests.
        let _: fn(
            &[Arc<Script>],
            &[Arc<Scriptlet>],
            &[BuiltInEntry],
            &[AppInfo],
            &[Arc<PluginSkill>],
            &[crate::flows::model::FlowDescriptor],
            Option<&grouped_view::FlowDiscoveryNote>,
            &FrecencyStore,
            &str,
            &SuggestedConfig,
            &[MenuBarItem],
            Option<&str>,
            Option<&crate::input_history::InputHistory>,
            Option<&AdvancedQuery>,
            Option<&crate::context_snapshot::launcher_context::LauncherContextSnapshot>,
        ) -> (Vec<GroupedListItem>, Vec<SearchResult>) =
            get_grouped_results_with_input_history_and_query;

        let _: fn(
            &[Arc<Script>],
            &[Arc<Scriptlet>],
            &[BuiltInEntry],
            &[AppInfo],
            &[Arc<PluginSkill>],
            &[crate::flows::model::FlowDescriptor],
            Option<&grouped_view::FlowDiscoveryNote>,
            &FrecencyStore,
            &str,
            &SuggestedConfig,
            &[MenuBarItem],
            Option<&str>,
            Option<&crate::input_history::InputHistory>,
            Option<&ValidationReport>,
            Option<&AdvancedQuery>,
        ) -> (Vec<GroupedListItem>, Vec<SearchResult>) =
            get_grouped_results_with_validation_and_query;

        let m = MatchIndices::default();
        let _ = m.clone();
    }

    #[test]
    fn pin_alias_match_first_moves_existing_result_to_top() {
        let mut flat = vec![
            builtin_result("Other Command"),
            builtin_result("Aliased Command"),
        ];
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Main".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
        ];

        pin_alias_match_first(
            &mut grouped,
            &mut flat,
            &|result| matches!(result, SearchResult::BuiltIn(bm) if bm.entry.id == "builtin/aliased-command"),
            &|| builtin_result("Aliased Command"),
        );

        assert!(
            matches!(grouped.first(), Some(GroupedListItem::Item(1))),
            "alias target must be the first grouped entry, got {grouped:?}"
        );
        assert_eq!(
            flat.len(),
            2,
            "no synthetic result when target already present"
        );
    }

    #[test]
    fn pin_alias_match_first_preserves_leading_results_header() {
        let mut flat = vec![
            builtin_result("Other Command"),
            builtin_result("Aliased Command"),
        ];
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Results".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
        ];

        pin_alias_match_first(
            &mut grouped,
            &mut flat,
            &|result| matches!(result, SearchResult::BuiltIn(bm) if bm.entry.id == "builtin/aliased-command"),
            &|| builtin_result("Aliased Command"),
        );

        assert!(matches!(
            grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None)) if label == "Results"
        ));
        assert!(
            matches!(grouped.get(1), Some(GroupedListItem::Item(1))),
            "alias target must be pinned under Results, got {grouped:?}"
        );
    }

    #[test]
    fn pin_alias_match_first_inserts_fallback_when_target_missing() {
        let mut flat = vec![builtin_result("Other Command")];
        let mut grouped = vec![GroupedListItem::Item(0)];

        pin_alias_match_first(
            &mut grouped,
            &mut flat,
            &|result| matches!(result, SearchResult::BuiltIn(bm) if bm.entry.id == "builtin/aliased-command"),
            &|| builtin_result("Aliased Command"),
        );

        assert_eq!(flat.len(), 2, "fallback result must be appended");
        assert!(
            matches!(grouped.first(), Some(GroupedListItem::Item(1))),
            "fallback alias target must be pinned first, got {grouped:?}"
        );
        assert!(
            matches!(flat[1], SearchResult::BuiltIn(ref bm) if bm.entry.id == "builtin/aliased-command")
        );
    }

    #[test]
    fn pin_alias_match_first_drops_orphaned_section_header() {
        let mut flat = vec![
            builtin_result("Other Command"),
            builtin_result("Aliased Command"),
        ];
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Lonely".to_string(), None),
            GroupedListItem::Item(1),
        ];

        pin_alias_match_first(
            &mut grouped,
            &mut flat,
            &|result| matches!(result, SearchResult::BuiltIn(bm) if bm.entry.id == "builtin/aliased-command"),
            &|| builtin_result("Aliased Command"),
        );

        assert!(matches!(grouped.first(), Some(GroupedListItem::Item(1))));
        assert!(
            !grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, _) if label == "Lonely"
            )),
            "header left without rows must be dropped, got {grouped:?}"
        );
    }

    #[test]
    fn query_predicates_suppress_issue_row_prepend() {
        // Deep proof that `:type:script <text>` never prepends an issue row even
        // when validation reports failed scripts. We inspect the shared helper
        // to avoid spinning up a full frecency/scripts fixture.
        let query = advanced_query_from(":type:script something");
        assert!(advanced_query_rejects_issue(Some(&query)));
    }

    fn root_file(path: &str, name: &str) -> FileResult {
        root_file_with_type(path, name, FileType::Document)
    }

    fn root_file_with_type(path: &str, name: &str, file_type: FileType) -> FileResult {
        FileResult {
            path: path.to_string(),
            name: name.to_string(),
            size: 0,
            modified: 0,
            file_type,
        }
    }

    fn root_file_search_handoff_result_for_test(
        query: &str,
        mode: crate::file_search::RootFileSectionMode,
    ) -> Option<SearchResult> {
        let ui_state = RootFileSectionUiState::new(
            query,
            mode,
            crate::file_search::RootFileQueryIntent::OrdinaryRoot,
            false,
            false,
            false,
            0,
            0,
            0,
            true,
        );
        root_file_search_handoff_result(
            query,
            mode,
            crate::file_search::RootFileQueryIntent::OrdinaryRoot,
            &ui_state,
        )
    }

    fn builtin_result(name: &str) -> SearchResult {
        SearchResult::BuiltIn(BuiltInMatch {
            entry: BuiltInEntry {
                id: format!("builtin/{}", name.to_lowercase().replace(' ', "-")),
                name: name.to_string(),
                description: "Test built-in".to_string(),
                keywords: Vec::new(),
                feature: crate::builtins::BuiltInFeature::AppLauncher,
                icon: None,
                group: crate::builtins::BuiltInGroup::Core,
            },
            score: i32::MAX,
            match_evidence: None,
        })
    }

    fn builtin_entry(name: &str) -> BuiltInEntry {
        match builtin_result(name) {
            SearchResult::BuiltIn(bm) => bm.entry,
            _ => unreachable!("builtin_result always returns a BuiltIn row"),
        }
    }

    fn agent_chat_history_hit(
        session_id: &str,
        title: &str,
    ) -> crate::ai::agent_chat::ui::history::AgentChatHistorySearchHit {
        crate::ai::agent_chat::ui::history::AgentChatHistorySearchHit {
            entry: crate::ai::agent_chat::ui::history::AgentChatHistoryEntry {
                timestamp: "2026-05-10T17:13:06Z".to_string(),
                first_message: title.to_string(),
                message_count: 3,
                session_id: session_id.to_string(),
                title: title.to_string(),
                custom_title: None,
                preview: "Prior assistant reply".to_string(),
                search_text: title.to_lowercase(),
            },
            score: 100,
            matched_field: crate::ai::agent_chat::ui::history::AgentChatHistorySearchField::Title,
            evidence: None,
        }
    }

    fn clipboard_history_entry(
        id: &str,
        preview: &str,
        pinned: bool,
    ) -> crate::clipboard_history::ClipboardEntryMeta {
        crate::clipboard_history::ClipboardEntryMeta {
            id: id.to_string(),
            content_type: crate::clipboard_history::ContentType::Text,
            timestamp: chrono::Utc::now().timestamp_millis(),
            pinned,
            text_preview: preview.to_string(),
            image_width: None,
            image_height: None,
            byte_size: preview.len(),
            ocr_text: None,
        }
    }

    fn root_note_hit(id: &str, title: &str, pinned: bool) -> crate::notes::RootNoteSearchHit {
        crate::notes::RootNoteSearchHit {
            id: crate::notes::NoteId::parse(id).unwrap_or_else(crate::notes::NoteId::new),
            title: title.to_string(),
            updated_at: chrono::Utc::now(),
            is_pinned: pinned,
            char_count: 42,
            score: 100,
        }
    }

    fn root_brain_hit(
        source: crate::brain::DocSource,
        source_id: &str,
        title: &str,
    ) -> crate::brain::RootBrainSearchHit {
        crate::brain::RootBrainSearchHit {
            title: title.to_string(),
            excerpt: "remembered context".to_string(),
            source_label: source.label(),
            source,
            source_id: source_id.to_string(),
        }
    }

    fn root_browser_tab_hit(
        stable_key: &str,
        title: &str,
    ) -> crate::browser_tabs::RootBrowserTabSearchHit {
        crate::browser_tabs::RootBrowserTabSearchHit {
            stable_key: stable_key.to_string(),
            tab: crate::browser_tabs::BrowserTabInfo {
                browser_name: "Safari".into(),
                browser_bundle_id: "com.apple.Safari".into(),
                window_index: 1,
                tab_index: 1,
                title: title.into(),
                url: "https://example.com/design".into(),
            },
            title: title.to_string(),
            url: "https://example.com/design".to_string(),
            domain: "example.com".to_string(),
            provider_label: "Safari".to_string(),
            score: 100.0,
        }
    }

    #[test]
    fn bare_domain_hoists_browser_tabs_ahead_of_every_group() {
        let hits = vec![root_browser_tab_hit("tab/x", "X")];
        let options = crate::browser_tabs::RootBrowserTabsSectionOptions {
            enabled: true,
            ..Default::default()
        };

        let mut domain_grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(0),
        ];
        let mut domain_flat = vec![builtin_result("Existing result")];
        let mut exhausted_budget = RootPassiveResultBudget {
            remaining_total: 0,
            max_per_source: 0,
        };
        append_root_browser_tabs_section(
            &mut domain_grouped,
            &mut domain_flat,
            "x.com",
            None,
            &hits,
            options.clone(),
            &mut exhausted_budget,
            false,
            true,
        );

        assert!(matches!(
            domain_grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None)) if label == "Browser Tabs"
        ));
        let first_item = domain_grouped
            .iter()
            .find_map(|item| match item {
                GroupedListItem::Item(index) => domain_flat.get(*index),
                _ => None,
            })
            .expect("first selectable row");
        assert!(matches!(first_item, SearchResult::BrowserTab(_)));

        let mut ordinary_grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(0),
        ];
        let mut ordinary_flat = vec![builtin_result("Existing result")];
        append_root_browser_tabs_section(
            &mut ordinary_grouped,
            &mut ordinary_flat,
            "design",
            None,
            &hits,
            options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
            false,
        );
        assert!(matches!(
            ordinary_grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None)) if label == "Files"
        ));
    }

    #[test]
    fn missing_explicit_passive_source_gets_a_status_row() {
        use crate::menu_syntax::RootUnifiedSourceFilter;

        let mut filters = crate::menu_syntax::RootUnifiedSourceFilterSet::default();
        filters.insert(RootUnifiedSourceFilter::BrowserTabs);
        filters.insert(RootUnifiedSourceFilter::BrowserHistory);

        // Tabs matched; history's section early-returned before its status.
        let flat = vec![SearchResult::BrowserTab(crate::scripts::BrowserTabMatch {
            hit: root_browser_tab_hit("tab/1", "Design Doc"),
            subtitle: "Safari".to_string(),
            score: 100,
        })];
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Browser Tabs".to_string(), None),
            GroupedListItem::Item(0),
        ];
        append_missing_explicit_source_status_rows(&mut grouped, &flat, &filters);

        let history_status = grouped.iter().any(|item| {
            matches!(
                item,
                GroupedListItem::Status(status)
                    if status.source == RootUnifiedSourceFilter::BrowserHistory && status.shown == 0
            )
        });
        assert!(
            history_status,
            "silently-empty explicit source must leave a status row, got {grouped:?}"
        );
        assert!(
            !grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::Status(status)
                    if status.source == RootUnifiedSourceFilter::BrowserTabs
            )),
            "sources represented by result rows must not get a duplicate status"
        );

        // With zero selectable rows the launcher empty state owns feedback.
        let mut empty: Vec<GroupedListItem> = Vec::new();
        append_missing_explicit_source_status_rows(&mut empty, &[], &filters);
        assert!(empty.is_empty());
    }

    fn root_browser_history_hit(
        stable_key: &str,
        title: &str,
    ) -> crate::browser_history::RootBrowserHistorySearchHit {
        crate::browser_history::RootBrowserHistorySearchHit {
            stable_key: stable_key.to_string(),
            provider_label: "Safari".to_string(),
            profile_label: "Default".to_string(),
            title: title.to_string(),
            url: "https://example.com/design-history".to_string(),
            domain: "example.com".to_string(),
            last_visit_unix_ms: chrono::Utc::now().timestamp_millis(),
            visit_count: 3,
        }
    }

    fn root_dictation_history_hit(
        id: &str,
        preview: &str,
    ) -> crate::dictation::RootDictationHistorySearchHit {
        crate::dictation::RootDictationHistorySearchHit {
            id: id.to_string(),
            preview: preview.to_string(),
            target: "Main Filter".to_string(),
            timestamp: "2026-05-10T17:13:06Z".to_string(),
            audio_duration_ms: 1200,
            score: 100,
            matched_field: crate::dictation::DictationHistorySearchField::Transcript,
            evidence: None,
        }
    }

    fn grouped_result_roles(
        grouped: &[GroupedListItem],
        flat: &[SearchResult],
    ) -> Vec<(usize, &'static str)> {
        grouped
            .iter()
            .enumerate()
            .filter_map(|(grouped_index, item)| {
                let GroupedListItem::Item(flat_index) = item else {
                    return None;
                };
                let role = match flat.get(*flat_index)? {
                    SearchResult::Flow(_)
                    | SearchResult::Script(_)
                    | SearchResult::Scriptlet(_)
                    | SearchResult::Skill(_)
                    | SearchResult::BuiltIn(_)
                    | SearchResult::App(_)
                    | SearchResult::Window(_) => "primary",
                    SearchResult::File(_) => "rootFile",
                    SearchResult::Note(_)
                    | SearchResult::BrainHit(_)
                    | SearchResult::Todo(_)
                    | SearchResult::AgentChatHistory(_)
                    | SearchResult::AiVault(_)
                    | SearchResult::ClipboardHistory(_)
                    | SearchResult::DictationHistory(_)
                    | SearchResult::BrowserTab(_)
                    | SearchResult::BrowserHistory(_) => "rootPassive",
                    SearchResult::Fallback(_) => "fallback",
                    SearchResult::ScriptIssue(_) => "scriptIssue",
                    SearchResult::BrainInboxItem(_) => "brainInbox",
                    SearchResult::Agent(_) => "agent",
                    SearchResult::SpineProjection(_) => "spine",
                };
                Some((grouped_index, role))
            })
            .collect()
    }

    /// First grouped index of a terminal fallback row (the "Use … with"
    /// section at the bottom of the list).
    ///
    /// The Files section appends its own "Search Files for …" handoff CTA,
    /// which is also a `SearchResult::Fallback` but belongs to the Files
    /// section by design (it must not split file rows; see
    /// `root_agent_chat_history_rows_do_not_split_files_section_or_file_handoff`
    /// and the F2 audit note on `root_brain_passive_insertion_index`).
    /// Ordering invariants about fallbacks must therefore compare against the
    /// first fallback that is NOT the Files handoff.
    fn first_terminal_fallback_index(
        grouped: &[GroupedListItem],
        flat: &[SearchResult],
    ) -> Option<usize> {
        grouped.iter().enumerate().find_map(|(index, item)| {
            let GroupedListItem::Item(flat_index) = item else {
                return None;
            };
            match flat.get(*flat_index)? {
                SearchResult::Fallback(fallback)
                    if !fallback
                        .stable_selection_key_override
                        .as_deref()
                        .is_some_and(|key| {
                            key.starts_with("fallback/root-file-search-handoff")
                        }) =>
                {
                    Some(index)
                }
                _ => None,
            }
        })
    }

    fn passive_source_counts(
        flat: &[SearchResult],
    ) -> std::collections::HashMap<&'static str, usize> {
        let mut counts = std::collections::HashMap::new();
        for result in flat {
            let source = match result {
                SearchResult::Note(_) => "Notes",
                SearchResult::Todo(_) => "Todos",
                SearchResult::AgentChatHistory(_) => "Agent Chat Conversations",
                SearchResult::AiVault(_) => "AI Vault",
                SearchResult::ClipboardHistory(_) => "Clipboard History",
                SearchResult::DictationHistory(_) => "Dictation History",
                SearchResult::BrowserTab(_) => "Browser Tabs",
                SearchResult::BrowserHistory(_) => "Browser History",
                _ => continue,
            };
            *counts.entry(source).or_insert(0) += 1;
        }
        counts
    }

    fn passive_result_count(flat: &[SearchResult]) -> usize {
        passive_source_counts(flat).values().sum()
    }

    #[test]
    fn root_file_rows_append_files_section_for_eligible_search() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/Desktop/fix spelling.png",
            "fix spelling.png",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "fix",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")),
            "eligible root queries should append a Files section"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/fix spelling.png"
            )),
            "Files section should point at the ranked root file row"
        );
    }

    #[test]
    fn root_file_match_mode_labels_and_handoff_metadata_stay_aligned() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/dev/node_modules/why-is-node-running/index.js",
            "why-is-node-running",
        )];

        let (phrase_grouped, phrase_flat) =
            get_grouped_results_with_validation_query_and_root_files(
                &[],
                &[],
                &[],
                &[],
                &[],
                &frecency_store,
                "why i",
                &SuggestedConfig::default(),
                &[],
                None,
                None,
                None,
                None,
                Some(crate::file_search::RootFileSectionMode::GlobalQuery),
                true,
                &[],
                &[],
            );
        assert!(phrase_grouped.iter().any(|item| matches!(
            item,
            GroupedListItem::SectionHeader(label, None) if label == "Files · Phrase match"
        )));
        assert!(phrase_flat.iter().any(|result| matches!(
            result,
            SearchResult::Fallback(fallback)
                if fallback.display_label() == "Search Files for \"why i\""
                    && fallback.display_description()
                        == "Open full File Search · preview matches typed phrase"
        )));

        let (word_grouped, word_flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "why is",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &root_files,
            &[],
        );
        assert!(word_grouped.iter().any(|item| matches!(
            item,
            GroupedListItem::SectionHeader(label, None) if label == "Files · Word match"
        )));
        assert!(word_flat.iter().any(|result| matches!(
            result,
            SearchResult::Fallback(fallback)
                if fallback.display_label() == "Search Files for \"why is\""
                    && fallback.display_description()
                        == "Open full File Search · preview matches filename words"
        )));

        let roles = grouped_result_roles(&word_grouped, &word_flat);
        let first_root_file = roles
            .iter()
            .find_map(|(idx, role)| (*role == "rootFile").then_some(*idx))
            .expect("root file row");
        let first_handoff = roles
            .iter()
            .find_map(|(idx, role)| (*role == "fallback").then_some(*idx))
            .expect("file handoff row");
        assert!(
            first_root_file < first_handoff,
            "file rows should precede handoff rows"
        );
    }

    #[test]
    fn root_passive_sources_never_precede_primary_launcher_rows_for_same_query() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let root_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];
        let browser_tabs = vec![root_browser_tab_hit("tab/design", "design tab")];
        let notes = vec![root_note_hit(
            "33333333-3333-3333-3333-333333333333",
            "design note",
            false,
        )];
        let clipboard = vec![clipboard_history_entry(
            "clip-design",
            "design copied text",
            false,
        )];
        let dictation = vec![root_dictation_history_hit(
            "dictation-design",
            "design transcript",
        )];
        let agent_chat = vec![agent_chat_history_hit(
            "session-design",
            "design conversation",
        )];
        let browser_history = vec![root_browser_history_hit(
            "history/design",
            "design history page",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files_with_options(
            &[],
            &[],
            &[builtin_entry("Design Gallery")],
            &[],
            &[],
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
            &[],
            &[],
            None,
            &frecency_store,
            query,
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
            crate::file_search::RootFileSectionOptions::default(),
            &[],
            crate::menu_syntax::RootTodoSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &[],
            crate::brain::RootBrainSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &clipboard,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                ..Default::default()
            },
            &dictation,
            crate::dictation::RootDictationHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                scan_limit: 10,
            },
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &[],
            crate::ai_vault::RootAiVaultSectionOptions::default(),
            &browser_tabs,
            crate::browser_tabs::RootBrowserTabsSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &browser_history,
            crate::browser_history::RootBrowserHistorySectionOptions {
                enabled: true,
                min_query_chars: 3,
                ..Default::default()
            },
            &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
            crate::config::UnifiedSearchPassiveResultLimitsConfig {
                max_total_results: 12,
                max_total_results_when_primary_visible: 12,
                max_results_per_source_when_primary_visible: 5,
            },
        );

        let roles = grouped_result_roles(&grouped, &flat);
        let first_primary = roles
            .iter()
            .find_map(|(index, role)| (*role == "primary").then_some(*index))
            .expect("collision fixture should include a primary launcher row");
        let first_root_file = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootFile").then_some(*index))
            .expect("collision fixture should include a root file row");
        let first_passive = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootPassive").then_some(*index))
            .expect("collision fixture should include a passive row");
        // The Files section carries its own "Search Files for …" handoff CTA
        // (a Fallback row that is part of the Files section by design), so the
        // passive-vs-fallback invariant is scoped to the terminal "Use … with"
        // fallback section.
        let first_fallback = first_terminal_fallback_index(&grouped, &flat)
            .expect("collision fixture should include a terminal fallback row");

        assert!(first_primary < first_root_file);
        assert!(first_primary < first_passive);
        assert!(first_root_file < first_fallback);
        assert!(first_passive < first_fallback);
        assert!(
            roles
                .iter()
                .all(|(index, role)| *role != "rootPassive" || *index > first_primary),
            "no passive root row should appear before the first primary launcher row"
        );

        let section_labels = grouped
            .iter()
            .filter_map(|item| match item {
                GroupedListItem::SectionHeader(label, None) => Some(label.as_str()),
                GroupedListItem::SectionHeader(_, Some(_))
                | GroupedListItem::Item(_)
                | GroupedListItem::Status(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            section_labels,
            vec![
                "Results",
                "Files",
                "Browser Tabs",
                "Notes",
                "Clipboard History",
                "Dictation History",
                "Agent Chat Conversations",
                "Browser History",
                "Use \"design\" with...",
            ]
        );
    }

    #[test]
    fn root_passive_source_order_reorders_only_passive_sections() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let root_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];
        let browser_tabs = vec![root_browser_tab_hit("tab/design", "design tab")];
        let notes = vec![root_note_hit(
            "33333333-3333-3333-3333-333333333333",
            "design note",
            false,
        )];
        let clipboard = vec![clipboard_history_entry(
            "clip-design",
            "design copied text",
            false,
        )];
        let dictation = vec![root_dictation_history_hit(
            "dictation-design",
            "design transcript",
        )];
        let agent_chat = vec![agent_chat_history_hit(
            "session-design",
            "design conversation",
        )];
        let browser_history = vec![root_browser_history_hit(
            "history/design",
            "design history page",
        )];
        let passive_order = [
            crate::config::UnifiedSearchPassiveSource::AgentChatHistory,
            crate::config::UnifiedSearchPassiveSource::BrowserHistory,
            crate::config::UnifiedSearchPassiveSource::Notes,
            crate::config::UnifiedSearchPassiveSource::BrowserTabs,
            crate::config::UnifiedSearchPassiveSource::ClipboardHistory,
            crate::config::UnifiedSearchPassiveSource::DictationHistory,
        ];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files_with_options(
            &[],
            &[],
            &[builtin_entry("Design Gallery")],
            &[],
            &[],
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
            &[],
            &[],
            None,
            &frecency_store,
            query,
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
            crate::file_search::RootFileSectionOptions::default(),
            &[],
            crate::menu_syntax::RootTodoSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &[],
            crate::brain::RootBrainSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &clipboard,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                ..Default::default()
            },
            &dictation,
            crate::dictation::RootDictationHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                scan_limit: 10,
            },
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &[],
            crate::ai_vault::RootAiVaultSectionOptions::default(),
            &browser_tabs,
            crate::browser_tabs::RootBrowserTabsSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &browser_history,
            crate::browser_history::RootBrowserHistorySectionOptions {
                enabled: true,
                min_query_chars: 3,
                ..Default::default()
            },
            &passive_order,
            crate::config::UnifiedSearchPassiveResultLimitsConfig {
                max_total_results: 12,
                max_total_results_when_primary_visible: 12,
                max_results_per_source_when_primary_visible: 5,
            },
        );

        let roles = grouped_result_roles(&grouped, &flat);
        let first_primary = roles
            .iter()
            .find_map(|(index, role)| (*role == "primary").then_some(*index))
            .expect("collision fixture should include a primary launcher row");
        let first_root_file = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootFile").then_some(*index))
            .expect("collision fixture should include a root file row");
        let first_passive = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootPassive").then_some(*index))
            .expect("collision fixture should include a passive row");
        // Scoped to the terminal "Use … with" fallback section; the Files
        // handoff CTA is a Fallback row owned by the Files section.
        let first_fallback = first_terminal_fallback_index(&grouped, &flat)
            .expect("collision fixture should include a terminal fallback row");

        assert!(first_primary < first_root_file);
        assert!(first_root_file < first_passive);
        assert!(first_passive < first_fallback);

        let section_labels = grouped
            .iter()
            .filter_map(|item| match item {
                GroupedListItem::SectionHeader(label, None) => Some(label.as_str()),
                GroupedListItem::SectionHeader(_, Some(_))
                | GroupedListItem::Item(_)
                | GroupedListItem::Status(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            section_labels,
            vec![
                "Results",
                "Files",
                "Agent Chat Conversations",
                "Browser History",
                "Notes",
                "Browser Tabs",
                "Clipboard History",
                "Dictation History",
                "Use \"design\" with...",
            ]
        );
    }

    #[test]
    fn root_brain_section_appends_only_when_enabled_with_hits() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let brain_hits = vec![root_brain_hit(
            crate::brain::DocSource::Note,
            "44444444-4444-4444-4444-444444444444",
            "design memory",
        )];

        let run = |hits: &[crate::brain::RootBrainSearchHit], enabled: bool| {
            get_grouped_results_with_validation_query_and_root_files_with_options(
                &[],
                &[],
                &[builtin_entry("Design Gallery")],
                &[],
                &[],
                crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
                &[],
                &[],
                None,
                &frecency_store,
                query,
                &SuggestedConfig::default(),
                &[],
                None,
                None,
                None,
                None,
                &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
                None,
                false,
                &[],
                &[],
                crate::file_search::RootFileSectionOptions::default(),
                &[],
                crate::menu_syntax::RootTodoSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                hits,
                crate::brain::RootBrainSectionOptions {
                    enabled,
                    ..Default::default()
                },
                &[],
                crate::notes::RootNotesSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::clipboard_history::RootClipboardHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::dictation::RootDictationHistorySectionOptions {
                    enabled: false,
                    max_results: 0,
                    min_query_chars: usize::MAX,
                    scan_limit: 0,
                },
                &[],
                crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::ai_vault::RootAiVaultSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::browser_tabs::RootBrowserTabsSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::browser_history::RootBrowserHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
                crate::config::UnifiedSearchPassiveResultLimitsConfig {
                    max_total_results: 12,
                    max_total_results_when_primary_visible: 12,
                    max_results_per_source_when_primary_visible: 5,
                },
            )
        };

        let has_brain_header = |grouped: &[GroupedListItem]| {
            grouped.iter().any(|item| {
                matches!(
                    item,
                    GroupedListItem::SectionHeader(label, None) if label == "From Your Brain"
                )
            })
        };

        let (grouped, flat) = run(&brain_hits, true);
        assert!(
            has_brain_header(&grouped),
            "enabled brain section with hits should append a From Your Brain header"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::BrainHit(bm) if bm.hit.title == "design memory"
            )),
            "From Your Brain section should surface the brain hit row"
        );

        let (grouped, flat) = run(&brain_hits, false);
        assert!(
            !has_brain_header(&grouped),
            "disabled brain section must not append a header"
        );
        assert!(
            !flat
                .iter()
                .any(|result| matches!(result, SearchResult::BrainHit(_))),
            "disabled brain section must not surface rows"
        );

        let (grouped, flat) = run(&[], true);
        assert!(
            !has_brain_header(&grouped),
            "empty brain hits must not append a header"
        );
        assert!(
            !flat
                .iter()
                .any(|result| matches!(result, SearchResult::BrainHit(_))),
            "empty brain hits must not surface rows"
        );
    }

    #[test]
    fn active_source_filters_select_matching_passive_sources() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let browser_tabs = vec![root_browser_tab_hit("tab/design", "design tab")];
        let notes = vec![root_note_hit(
            "33333333-3333-3333-3333-333333333333",
            "design note",
            false,
        )];
        let clipboard = vec![clipboard_history_entry(
            "clip-design",
            "design copied text",
            false,
        )];
        let dictation = vec![root_dictation_history_hit(
            "dictation-design",
            "design transcript",
        )];
        let agent_chat = vec![agent_chat_history_hit(
            "session-design",
            "design conversation",
        )];
        let browser_history = vec![root_browser_history_hit(
            "history/design",
            "design history page",
        )];

        for (source, expected_section, expected_source) in [
            (
                crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs,
                "Browser Tabs",
                "Browser Tabs",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::Notes,
                "Notes",
                "Notes",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory,
                "Clipboard History",
                "Clipboard History",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::Dictation,
                "Dictation History",
                "Dictation History",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::Conversations,
                "Agent Chat Conversations",
                "Agent Chat Conversations",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory,
                "Browser History",
                "Browser History",
            ),
        ] {
            let mut source_filters = crate::menu_syntax::RootUnifiedSourceFilterSet::default();
            source_filters.insert(source);

            let (grouped, flat) =
                get_grouped_results_with_validation_query_and_root_files_with_options(
                    &[],
                    &[],
                    &[builtin_entry("Design Gallery")],
                    &[],
                    &[],
                    crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
                    &[],
                    &[],
                    None,
                    &frecency_store,
                    query,
                    &SuggestedConfig::default(),
                    &[],
                    None,
                    None,
                    None,
                    None,
                    &source_filters,
                    None,
                    false,
                    &[],
                    &[],
                    crate::file_search::RootFileSectionOptions::default(),
                    &[],
                    crate::menu_syntax::RootTodoSectionOptions {
                        enabled: false,
                        ..Default::default()
                    },
                    &[],
                    crate::brain::RootBrainSectionOptions {
                        enabled: false,
                        ..Default::default()
                    },
                    &notes,
                    crate::notes::RootNotesSectionOptions {
                        enabled: true,
                        ..Default::default()
                    },
                    &clipboard,
                    crate::clipboard_history::RootClipboardHistorySectionOptions {
                        enabled: true,
                        ..Default::default()
                    },
                    &dictation,
                    crate::dictation::RootDictationHistorySectionOptions {
                        enabled: true,
                        max_results: 3,
                        min_query_chars: 3,
                        scan_limit: 10,
                    },
                    &agent_chat,
                    crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(
                    ),
                    &[],
                    crate::ai_vault::RootAiVaultSectionOptions::default(),
                    &browser_tabs,
                    crate::browser_tabs::RootBrowserTabsSectionOptions {
                        enabled: true,
                        ..Default::default()
                    },
                    &browser_history,
                    crate::browser_history::RootBrowserHistorySectionOptions {
                        enabled: true,
                        min_query_chars: 3,
                        ..Default::default()
                    },
                    &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
                    crate::config::UnifiedSearchPassiveResultLimitsConfig::default(),
                );

            let section_labels = grouped
                .iter()
                .filter_map(|item| match item {
                    GroupedListItem::SectionHeader(label, None) => Some(label.as_str()),
                    GroupedListItem::SectionHeader(_, Some(_))
                    | GroupedListItem::Item(_)
                    | GroupedListItem::Status(_) => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(section_labels, vec![expected_section], "{source:?}");
            assert!(
                flat.iter()
                    .all(|result| result.source_name() == Some(expected_source)),
                "{source:?}: unexpected rows {flat:?}"
            );
        }
    }

    #[test]
    fn root_passive_budget_caps_rows_when_primary_launcher_results_exist() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let root_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];
        let browser_tabs = (0..3)
            .map(|i| root_browser_tab_hit(&format!("tab/design-{i}"), "design tab"))
            .collect::<Vec<_>>();
        let notes = vec![
            root_note_hit("33333333-3333-3333-3333-333333333331", "design note", false),
            root_note_hit("33333333-3333-3333-3333-333333333332", "design note", false),
            root_note_hit("33333333-3333-3333-3333-333333333333", "design note", false),
        ];
        let clipboard = (0..3)
            .map(|i| {
                clipboard_history_entry(&format!("clip-design-{i}"), "design copied text", false)
            })
            .collect::<Vec<_>>();
        let dictation = (0..3)
            .map(|i| {
                root_dictation_history_hit(&format!("dictation-design-{i}"), "design transcript")
            })
            .collect::<Vec<_>>();
        let agent_chat = (0..3)
            .map(|i| agent_chat_history_hit(&format!("session-design-{i}"), "design conversation"))
            .collect::<Vec<_>>();
        let browser_history = (0..3)
            .map(|i| {
                root_browser_history_hit(&format!("history/design-{i}"), "design history page")
            })
            .collect::<Vec<_>>();

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files_with_options(
            &[],
            &[],
            &[builtin_entry("Design Gallery")],
            &[],
            &[],
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
            &[],
            &[],
            None,
            &frecency_store,
            query,
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
            crate::file_search::RootFileSectionOptions::default(),
            &[],
            crate::menu_syntax::RootTodoSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &[],
            crate::brain::RootBrainSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &clipboard,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &dictation,
            crate::dictation::RootDictationHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                scan_limit: 10,
            },
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
            },
            &[],
            crate::ai_vault::RootAiVaultSectionOptions::default(),
            &browser_tabs,
            crate::browser_tabs::RootBrowserTabsSectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &browser_history,
            crate::browser_history::RootBrowserHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                ..Default::default()
            },
            &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
            crate::config::UnifiedSearchPassiveResultLimitsConfig {
                max_total_results: 12,
                max_total_results_when_primary_visible: 4,
                max_results_per_source_when_primary_visible: 1,
            },
        );

        let roles = grouped_result_roles(&grouped, &flat);
        let first_primary = roles
            .iter()
            .find_map(|(index, role)| (*role == "primary").then_some(*index))
            .unwrap();
        let first_root_file = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootFile").then_some(*index))
            .unwrap();
        let first_passive = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootPassive").then_some(*index))
            .unwrap();
        // Scoped to the terminal "Use … with" fallback section; the Files
        // handoff CTA is a Fallback row owned by the Files section.
        let first_fallback = first_terminal_fallback_index(&grouped, &flat).unwrap();
        assert!(first_primary < first_root_file);
        assert!(first_root_file < first_passive);
        assert!(first_passive < first_fallback);
        assert_eq!(passive_result_count(&flat), 4);
        assert!(passive_source_counts(&flat)
            .values()
            .all(|count| *count <= 1));
    }

    #[test]
    fn root_passive_budget_allows_larger_passive_set_without_primary_launcher_results() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let browser_tabs = (0..3)
            .map(|i| root_browser_tab_hit(&format!("tab/design-{i}"), "design tab"))
            .collect::<Vec<_>>();
        let notes = vec![
            root_note_hit("33333333-3333-3333-3333-333333333331", "design note", false),
            root_note_hit("33333333-3333-3333-3333-333333333332", "design note", false),
            root_note_hit("33333333-3333-3333-3333-333333333333", "design note", false),
        ];
        let clipboard = (0..3)
            .map(|i| {
                clipboard_history_entry(&format!("clip-design-{i}"), "design copied text", false)
            })
            .collect::<Vec<_>>();

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files_with_options(
            &[],
            &[],
            &[],
            &[],
            &[],
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
            &[],
            &[],
            None,
            &frecency_store,
            query,
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
            None,
            false,
            &[],
            &[],
            crate::file_search::RootFileSectionOptions::default(),
            &[],
            crate::menu_syntax::RootTodoSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &[],
            crate::brain::RootBrainSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &clipboard,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &[],
            crate::dictation::RootDictationHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                scan_limit: 10,
            },
            &[],
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
            },
            &[],
            crate::ai_vault::RootAiVaultSectionOptions::default(),
            &browser_tabs,
            crate::browser_tabs::RootBrowserTabsSectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &[],
            crate::browser_history::RootBrowserHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                ..Default::default()
            },
            &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
            crate::config::UnifiedSearchPassiveResultLimitsConfig {
                max_total_results: 5,
                max_total_results_when_primary_visible: 1,
                max_results_per_source_when_primary_visible: 1,
            },
        );

        assert_eq!(passive_result_count(&flat), 5);
        let section_labels = grouped
            .iter()
            .filter_map(|item| match item {
                GroupedListItem::SectionHeader(label, None) => Some(label.as_str()),
                GroupedListItem::SectionHeader(_, Some(_))
                | GroupedListItem::Item(_)
                | GroupedListItem::Status(_) => None,
            })
            .collect::<Vec<_>>();
        // The passive budget is consumed greedily in passive-source order:
        // Browser Tabs takes 3 rows, Notes takes the remaining 2, and the
        // Clipboard History section is skipped once the total budget (5) is
        // exhausted. The invariant under test is the total cap, asserted via
        // passive_result_count above.
        assert_eq!(
            section_labels,
            vec!["Browser Tabs", "Notes", "Use \"design\" with..."]
        );
    }

    #[test]
    fn root_passive_budget_zero_hides_passive_rows_during_primary_collision() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let root_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];
        let notes = vec![root_note_hit(
            "33333333-3333-3333-3333-333333333333",
            "design note",
            false,
        )];

        let (_grouped, flat) =
            get_grouped_results_with_validation_query_and_root_files_with_options(
                &[],
                &[],
                &[builtin_entry("Design Gallery")],
                &[],
                &[],
                crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
                &[],
                &[],
                None,
                &frecency_store,
                query,
                &SuggestedConfig::default(),
                &[],
                None,
                None,
                None,
                None,
                &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
                Some(crate::file_search::RootFileSectionMode::GlobalQuery),
                false,
                &root_files,
                &[],
                crate::file_search::RootFileSectionOptions::default(),
                &[],
                crate::menu_syntax::RootTodoSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::brain::RootBrainSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &notes,
                crate::notes::RootNotesSectionOptions {
                    enabled: true,
                    ..Default::default()
                },
                &[],
                crate::clipboard_history::RootClipboardHistorySectionOptions {
                    enabled: true,
                    ..Default::default()
                },
                &[],
                crate::dictation::RootDictationHistorySectionOptions {
                    enabled: true,
                    max_results: 3,
                    min_query_chars: 3,
                    scan_limit: 10,
                },
                &[],
                crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
                &[],
                crate::ai_vault::RootAiVaultSectionOptions::default(),
                &[],
                crate::browser_tabs::RootBrowserTabsSectionOptions {
                    enabled: true,
                    ..Default::default()
                },
                &[],
                crate::browser_history::RootBrowserHistorySectionOptions {
                    enabled: true,
                    min_query_chars: 3,
                    ..Default::default()
                },
                &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
                crate::config::UnifiedSearchPassiveResultLimitsConfig {
                    max_total_results: 12,
                    max_total_results_when_primary_visible: 0,
                    max_results_per_source_when_primary_visible: 1,
                },
            );

        assert!(flat.iter().any(is_primary_launcher_result));
        assert!(flat
            .iter()
            .any(|result| matches!(result, SearchResult::File(_))));
        assert!(flat
            .iter()
            .any(|result| matches!(result, SearchResult::Fallback(_))));
        assert_eq!(passive_result_count(&flat), 0);
    }

    #[test]
    fn root_agent_chat_history_rows_insert_after_primary_rows_before_fallbacks() {
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Use \"search\" with...".to_string(), None),
            GroupedListItem::Item(1),
        ];
        let mut flat = vec![
            builtin_result("Search Files"),
            root_file_search_handoff_result_for_test(
                "search",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
        ];
        let hits = vec![agent_chat_history_hit("session-1", "search design notes")];

        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );

        assert!(
            matches!(&grouped[1], GroupedListItem::SectionHeader(label, None) if label == "Agent Chat Conversations")
        );
        assert!(matches!(
            flat.get(2),
            Some(SearchResult::AgentChatHistory(hit)) if hit.entry.session_id == "session-1"
        ));
        assert!(
            matches!(&grouped[3], GroupedListItem::SectionHeader(label, None) if label.starts_with("Use \""))
        );
    }

    #[test]
    fn root_agent_chat_history_rows_do_not_append_for_short_or_advanced_query() {
        let hits = vec![agent_chat_history_hit("session-1", "search design notes")];

        let mut grouped = Vec::new();
        let mut flat = Vec::new();
        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "ai",
            None,
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        let query = advanced_query_from(":type:agent_chat-history search");
        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            Some(&query),
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());
    }

    #[test]
    fn root_agent_chat_history_rows_do_not_append_when_disabled() {
        let hits = vec![agent_chat_history_hit("session-1", "search design notes")];
        let mut grouped = Vec::new();
        let mut flat = Vec::new();

        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                enabled: false,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );

        assert!(grouped.is_empty());
        assert!(flat.is_empty());
    }

    #[test]
    fn root_clipboard_history_rows_insert_before_agent_chat_and_fallbacks() {
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Use \"search\" with...".to_string(), None),
            GroupedListItem::Item(1),
        ];
        let mut flat = vec![
            builtin_result("Search Files"),
            root_file_search_handoff_result_for_test(
                "search",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
        ];
        let clips = vec![clipboard_history_entry(
            "clip-1",
            "search copied text",
            true,
        )];
        let agent_chat = vec![agent_chat_history_hit("session-1", "search design notes")];

        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &clips,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );

        assert!(
            matches!(&grouped[1], GroupedListItem::SectionHeader(label, None) if label == "Clipboard History")
        );
        assert!(
            matches!(&grouped[3], GroupedListItem::SectionHeader(label, None) if label == "Agent Chat Conversations")
        );
        assert!(matches!(
            flat.get(2),
            Some(SearchResult::ClipboardHistory(hit)) if hit.entry.id == "clip-1"
        ));
        assert!(matches!(
            flat.get(3),
            Some(SearchResult::AgentChatHistory(hit)) if hit.entry.session_id == "session-1"
        ));
    }

    #[test]
    fn root_notes_rows_insert_after_primary_before_clipboard_agent_chat_and_fallbacks() {
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Use \"search\" with...".to_string(), None),
            GroupedListItem::Item(1),
        ];
        let mut flat = vec![
            builtin_result("Search Files"),
            root_file_search_handoff_result_for_test(
                "search",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
        ];
        let notes = vec![root_note_hit(
            "11111111-1111-1111-1111-111111111111",
            "search note",
            true,
        )];
        let clips = vec![clipboard_history_entry(
            "clip-1",
            "search copied text",
            true,
        )];
        let agent_chat = vec![agent_chat_history_hit("session-1", "search design notes")];

        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &clips,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );

        assert!(
            matches!(&grouped[1], GroupedListItem::SectionHeader(label, None) if label == "Notes")
        );
        assert!(
            matches!(&grouped[3], GroupedListItem::SectionHeader(label, None) if label == "Clipboard History")
        );
        assert!(
            matches!(&grouped[5], GroupedListItem::SectionHeader(label, None) if label == "Agent Chat Conversations")
        );
        assert!(matches!(
            flat.get(2),
            Some(SearchResult::Note(hit)) if hit.title == "search note"
        ));
    }

    #[test]
    fn root_notes_rows_do_not_append_for_empty_short_disabled_or_advanced_query() {
        let notes = vec![root_note_hit(
            "22222222-2222-2222-2222-222222222222",
            "search note",
            false,
        )];
        let enabled_options = crate::notes::RootNotesSectionOptions {
            enabled: true,
            ..Default::default()
        };

        let mut grouped = Vec::new();
        let mut flat = Vec::new();
        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "",
            None,
            &notes,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "no",
            None,
            &notes,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        let query = advanced_query_from(":type:note search");
        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "search",
            Some(&query),
            &notes,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());
    }

    #[test]
    fn root_clipboard_history_rows_do_not_append_for_empty_short_disabled_or_advanced_query() {
        let clips = vec![clipboard_history_entry(
            "clip-1",
            "search copied text",
            false,
        )];
        let enabled_options = crate::clipboard_history::RootClipboardHistorySectionOptions {
            enabled: true,
            ..Default::default()
        };

        let mut grouped = Vec::new();
        let mut flat = Vec::new();
        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "",
            None,
            &clips,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "se",
            None,
            &clips,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &clips,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: false,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        let query = advanced_query_from(":type:clipboard search");
        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "search",
            Some(&query),
            &clips,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());
    }

    #[test]
    fn root_agent_chat_history_rows_do_not_split_files_section_or_file_handoff() {
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(1),
            GroupedListItem::Item(2),
            GroupedListItem::SectionHeader("Use \"design\" with...".to_string(), None),
            GroupedListItem::Item(3),
        ];
        let mut flat = vec![
            builtin_result("Open Notes"),
            SearchResult::File(crate::scripts::FileMatch {
                file: root_file("/Users/example/Desktop/design.md", "design.md"),
                score: 50,
            }),
            root_file_search_handoff_result_for_test(
                "design",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
            root_file_search_handoff_result_for_test(
                "design",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
        ];
        let hits = vec![agent_chat_history_hit("session-1", "design notes")];

        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "design",
            None,
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
        );

        assert!(
            matches!(&grouped[1], GroupedListItem::SectionHeader(label, None) if label == "Files")
        );
        assert!(matches!(&grouped[2], GroupedListItem::Item(1)));
        assert!(matches!(&grouped[3], GroupedListItem::Item(2)));
        assert!(
            matches!(&grouped[4], GroupedListItem::SectionHeader(label, None) if label == "Agent Chat Conversations"),
            "Agent Chat Conversations should insert after the Files handoff, not between file rows"
        );
    }

    #[test]
    fn root_global_file_rows_seed_matching_recent_files_while_provider_loading() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")),
            "global root search should keep the stable Files header while recent seeds render"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/design-notes.md"
            )),
            "matching recent files should seed non-empty global root file results before provider rows arrive"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"design\""
            )),
            "seeded global file rows should keep the full File Search handoff"
        );
    }

    #[test]
    fn root_global_recent_seed_rejects_path_only_match_while_loading() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Design/archive/readme.md",
            "readme.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            flat.iter().all(|result| !matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Design/archive/readme.md"
            )),
            "path-only recent files should not seed non-empty global root search"
        );
        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")),
            "the stable Files section should remain visible for the continuation row"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"design\""
            )),
            "path-only recent rejection should still keep the dedicated File Search handoff"
        );
    }

    #[test]
    fn root_global_recent_seed_accepts_ordered_directory_context() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/dev/script-kit/README.md",
            "README.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "script kit readme",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Files · Word match"
            )),
            "directory-context recent seeds should render under the stable Word match Files header"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/dev/script-kit/README.md"
            )),
            "ordered directory-context recent files should seed non-empty global root results"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"script kit readme\""
            )),
            "seeded directory-context rows should keep the full File Search handoff"
        );
    }

    #[test]
    fn root_global_recent_seed_rejects_path_only_directory_context() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/dev/script-kit/readme/archive.txt",
            "archive.txt",
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "script kit readme",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            flat.iter().all(|result| !matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/dev/script-kit/readme/archive.txt"
            )),
            "path-only directory-context recents must not seed while the provider is loading"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"script kit readme\""
            )),
            "path-only rejection should still keep the dedicated File Search handoff"
        );
    }

    #[test]
    fn root_global_provider_path_only_match_still_renders() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/Design/archive/readme.md",
            "readme.md",
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Design/archive/readme.md"
            )),
            "provider-returned path-only matches should still render after the provider answers"
        );
    }

    #[test]
    fn root_global_file_rows_dedupe_provider_and_recent_by_path() {
        let frecency_store = FrecencyStore::new();
        let shared = root_file("/Users/example/Desktop/design-notes.md", "design-notes.md");
        let provider_files = vec![shared.clone()];
        let recent_files = vec![shared];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &provider_files,
            &recent_files,
        );

        let duplicate_count = flat
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    SearchResult::File(file) if file.file.path == "/Users/example/Desktop/design-notes.md"
                )
            })
            .count();

        assert_eq!(
            duplicate_count, 1,
            "provider and recent rows with the same full path should render once"
        );
    }

    #[test]
    fn root_global_exact_stem_match_promotes_files_section_when_opted_in() {
        let files = vec![SearchResult::File(crate::scripts::FileMatch {
            file: root_file("/Users/example/Desktop/design-notes.md", "design-notes.md"),
            score: 100,
        })];
        let grouped = vec![GroupedListItem::SectionHeader("Commands".to_string(), None)];
        let file_matches = files
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design-notes",
            &file_matches,
            &[],
        ));
        assert_eq!(
            root_file_section_insertion_index(&grouped, &files, true),
            0,
            "exact filename/stem matches can insert Files above ordinary launcher groups only when opted in"
        );
    }

    #[test]
    fn root_directory_browse_never_promotes_files_section() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/dev/design-notes.md", "design-notes.md"),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::DirectoryBrowse,
            false,
            "design",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_boundary_filename_token_match_does_not_promote_exact_policy() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/client-design-notes.md",
                "client-design-notes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_camel_case_filename_token_match_does_not_promote_exact_policy() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/ClientDesignNotes.md",
                "ClientDesignNotes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_recent_seed_accepts_camel_case_filename_token() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/ClientDesignNotes.md",
            "ClientDesignNotes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Files"
            )),
            "global root search should keep the stable Files header while camel-case recent seeds render"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/ClientDesignNotes.md"
            )),
            "camel-case filename token matches should seed non-empty global root file results before provider rows arrive"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"design\""
            )),
            "seeded global file rows should keep the full File Search handoff"
        );
    }

    #[test]
    fn root_global_multiword_recent_seed_uses_filename_tokens() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/client-design-notes.md",
            "client-design-notes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "client notes",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Files · Word match"
            )),
            "multi-word recent seeds should render under the stable Word match Files header"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/client-design-notes.md"
            )),
            "ordered multi-word filename tokens should seed non-empty global root file results"
        );
    }

    #[test]
    fn root_global_multiword_token_match_does_not_promote_exact_policy() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/client-design-notes.md",
                "client-design-notes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design notes",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_multiword_mid_token_match_does_not_promote() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/redesign-notes.md",
                "redesign-notes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design notes",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_recent_seed_directory_context_does_not_promote_files_section() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/dev/script-kit/README.md", "README.md"),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "script kit readme",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_short_digit_recent_seed_uses_filename_tokens() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/2026-q2-report.xlsx",
            "2026-q2-report.xlsx",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "q2",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Files"
            )),
            "short digit recent seeds should render under the stable Files header"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.name == "2026-q2-report.xlsx"
            )),
            "short digit filename tokens should seed non-empty global root file results"
        );
    }

    #[test]
    fn root_global_short_digit_token_match_does_not_promote_exact_policy() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/Desktop/Q2Report.pdf", "Q2Report.pdf"),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "q2",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_two_letter_query_still_does_not_promote() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/Desktop/ai-notes.md", "ai-notes.md"),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "ai",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_mid_token_contains_does_not_promote_files_section() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/redesign-notes.md",
                "redesign-notes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_strong_launcher_match_blocks_file_section_promotion() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/fix spelling.png",
                "fix spelling.png",
            ),
            score: 100,
        }];
        let launcher_results = vec![builtin_result("Fix Spelling and Grammar")];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "spelling",
            &files,
            &launcher_results,
        ));
    }

    #[test]
    fn root_global_weak_launcher_match_blocks_file_section_promotion() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/Desktop/design-notes.md", "design-notes.md"),
            score: 100,
        }];
        let launcher_results = vec![builtin_result("Redesign Theme")];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design",
            &files,
            &launcher_results,
        ));
    }

    #[test]
    fn root_file_rows_precede_fallback_rows_for_file_only_search() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/Desktop/unique report name.pdf",
            "unique report name.pdf",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "unique report name",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        let file_grouped_index = grouped
            .iter()
            .position(|item| {
                matches!(
                    item,
                    GroupedListItem::Item(idx)
                        if matches!(flat.get(*idx), Some(SearchResult::File(_)))
                )
            })
            .expect("file result should be grouped");
        let fallback_grouped_index = grouped
            .iter()
            .position(|item| {
                matches!(
                    item,
                    GroupedListItem::Item(idx)
                        if matches!(flat.get(*idx), Some(SearchResult::Fallback(_)))
                )
            })
            .expect("fallback result should still be grouped");

        assert!(
            file_grouped_index < fallback_grouped_index,
            "root file results should appear before fallback actions so Enter opens the file first"
        );
    }

    #[test]
    fn root_file_rows_do_not_append_for_advanced_queries() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/Desktop/fix spelling.png",
            "fix spelling.png",
        )];
        let query = advanced_query_from(":type:file fix");

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "fix",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            Some(&query),
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        assert!(
            !grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")),
            "advanced query mode should not mix in root Spotlight file rows"
        );
        assert!(
            flat.iter()
                .all(|result| !matches!(result, SearchResult::File(_))),
            "advanced query mode should not append file results"
        );
    }

    #[test]
    fn root_global_file_rows_exclude_application_bundles() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type("/Applications/Zed.app", "Zed.app", FileType::Application),
            root_file_with_type(
                "/Users/example/Desktop/zed-notes.md",
                "zed-notes.md",
                FileType::Document,
            ),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "zed",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        let rendered_files = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_files,
            vec!["zed-notes.md"],
            "global root Files should not duplicate app launcher results as .app file rows"
        );
    }

    #[test]
    fn root_global_file_rows_exclude_app_bundle_contents() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type(
                "/Applications/Zed.app/Contents/Info.plist",
                "Info.plist",
                FileType::Document,
            ),
            root_file_with_type(
                "/Users/example/Desktop/zed-notes.md",
                "zed-notes.md",
                FileType::Document,
            ),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "zed",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        let rendered_files = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.path.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_files,
            vec!["/Users/example/Desktop/zed-notes.md"],
            "global root Files should not render files nested inside .app bundles"
        );
    }

    #[test]
    fn root_global_app_bundle_filter_keeps_search_files_handoff() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file_with_type(
            "/Applications/Zed.app",
            "Zed.app",
            FileType::Application,
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "zed",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        assert!(
            flat.iter().all(|result| !matches!(
                result,
                SearchResult::File(file) if file.file.name == "Zed.app"
            )),
            "filtered application bundles should not render as root global file rows"
        );
        assert!(
            grouped.iter().any(|item| {
                matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")
            }),
            "the Files section should still be allowed to show the handoff row"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"zed\""
            )),
            "app-bundle filtering should not remove the full File Search handoff"
        );
    }

    #[test]
    fn root_directory_browse_keeps_app_bundle_contents_for_explicit_paths() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file_with_type(
            "/Applications/Zed.app/Contents/Info.plist",
            "Info.plist",
            FileType::Document,
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "/Applications/Zed.app/Contents/",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            false,
            &root_files,
            &[],
        );

        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Applications/Zed.app/Contents/Info.plist"
            )),
            "explicit directory browse should still render already-collected direct children inside .app bundles"
        );
    }

    #[test]
    fn root_directory_browse_rows_append_files_section_for_path_query() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type("/Users/example/dev/app", "app", FileType::Directory),
            root_file_with_type(
                "/Users/example/dev/Zed.app",
                "Zed.app",
                FileType::Application,
            ),
        ];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "~/dev/",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            false,
            &root_files,
            &[],
        );

        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files · Folder")),
            "directory path queries should append the folder-specific Files section"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/dev/Zed.app"
            )),
            "directory browse should render provider-ordered rows, including app bundles"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Open File Search in \"~/dev\""
            )),
            "directory browse should append a folder-scoped File Search handoff"
        );
    }

    #[test]
    fn root_directory_browse_does_not_mix_recent_files() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file("/Users/example/dev/design.md", "design.md")];
        let recent_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "~/dev/design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            true,
            &root_files,
            &recent_files,
        );

        let rendered_paths = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.path.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_paths,
            vec!["/Users/example/dev/design.md"],
            "directory browse should render direct children only and ignore recent file seeds"
        );
    }

    #[test]
    fn root_directory_browse_rows_use_provider_order_without_fuzzy_filtering() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type(
                "/Users/example/dev/beta.txt",
                "beta.txt",
                FileType::Document,
            ),
            root_file_with_type(
                "/Users/example/dev/alpha.txt",
                "alpha.txt",
                FileType::Document,
            ),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "~/dev/",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            false,
            &root_files,
            &[],
        );

        let rendered_files = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_files,
            vec!["beta.txt", "alpha.txt"],
            "directory browse should preserve provider order instead of fuzzy re-ranking"
        );
    }

    #[test]
    fn root_directory_browse_rows_filter_by_child_fragment() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type(
                "/Users/example/dev/beta-notes.md",
                "beta-notes.md",
                FileType::Document,
            ),
            root_file_with_type(
                "/Users/example/dev/alpha-report.md",
                "alpha-report.md",
                FileType::Document,
            ),
            root_file_with_type(
                "/Users/example/dev/alpha-folder",
                "alpha-folder",
                FileType::Directory,
            ),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "~/dev/al",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            false,
            &root_files,
            &[],
        );

        let rendered_files = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_files,
            vec!["alpha-folder", "alpha-report.md"],
            "directory browse child fragments should filter direct children inline"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Open File Search in \"~/dev\""
            )),
            "filtered directory browse should keep the handoff scoped to the containing folder"
        );
    }

    #[test]
    fn empty_root_appends_recent_files_section() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Recent Files")),
            "empty root should append a Recent Files section"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/recent design notes.md"
            )),
            "Recent Files should render real SearchResult::File rows"
        );
    }

    #[test]
    fn recent_files_grouping_filters_directories_and_app_bundle_contents() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![
            root_file_with_type(
                "/Users/example/Desktop/example-folder",
                "example-folder",
                FileType::Directory,
            ),
            root_file_with_type(
                "/Applications/Zed.app/Contents/Info.plist",
                "Info.plist",
                FileType::Document,
            ),
            root_file("/Users/example/Desktop/design-notes.md", "design-notes.md"),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        let rendered_paths = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.path.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rendered_paths,
            vec!["/Users/example/Desktop/design-notes.md"],
            "Recent Files should filter directories and app bundle internals"
        );
    }

    #[test]
    fn empty_root_recent_files_suppress_section_when_all_rows_ineligible() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![
            root_file_with_type("/Applications/Zed.app", "Zed.app", FileType::Application),
            root_file_with_type(
                "/Applications/Zed.app/Contents/Info.plist",
                "Info.plist",
                FileType::Document,
            ),
        ];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            !grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Recent Files")),
            "empty-root Recent Files should omit the section when every row is ineligible"
        );
        assert!(
            flat.iter()
                .all(|result| !matches!(result, SearchResult::File(_))),
            "all-ineligible recent files should not render file rows"
        );
    }

    #[test]
    fn root_global_recent_seed_can_match_beyond_empty_recent_render_limit() {
        let frecency_store = FrecencyStore::new();
        let mut recent_files = Vec::new();
        for idx in 0..crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT {
            recent_files.push(root_file(
                &format!("/Users/example/Desktop/other-{idx}.md"),
                &format!("other-{idx}.md"),
            ));
        }
        recent_files.push(root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        ));

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/design-notes.md"
            )),
            "non-empty global Files should seed from the deeper recent pool, not only the empty-root render cap"
        );
    }

    #[test]
    fn empty_root_recent_files_stay_render_capped_with_deeper_recent_pool() {
        let frecency_store = FrecencyStore::new();
        let recent_files = (0..crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT + 3)
            .map(|idx| {
                root_file(
                    &format!("/Users/example/Desktop/recent-{idx}.md"),
                    &format!("recent-{idx}.md"),
                )
            })
            .collect::<Vec<_>>();

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        let file_count = flat
            .iter()
            .filter(|result| matches!(result, SearchResult::File(_)))
            .count();
        assert_eq!(
            file_count,
            crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT,
            "empty-root Recent Files should remain visually capped"
        );
    }

    #[test]
    fn source_filter_files_empty_browse_uses_browse_target_not_recent_render_cap() {
        let recent_files = (0..crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT + 8)
            .map(|idx| {
                root_file(
                    &format!("/Users/example/Desktop/recent-{idx}.md"),
                    &format!("recent-{idx}.md"),
                )
            })
            .collect::<Vec<_>>();
        let mut source_filters = crate::menu_syntax::RootUnifiedSourceFilterSet::default();
        source_filters.insert(crate::menu_syntax::RootUnifiedSourceFilter::Files);
        let target = crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT + 8;

        let (_grouped, flat) =
            get_grouped_results_with_validation_query_and_root_files_with_options(
                &[],
                &[],
                &[],
                &[],
                &[],
                crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
                &[],
                &[],
                None,
                &FrecencyStore::new(),
                "",
                &SuggestedConfig::default(),
                &[],
                None,
                None,
                None,
                None,
                &source_filters,
                None,
                false,
                &[],
                &recent_files,
                crate::file_search::RootFileSectionOptions {
                    source_filter_browse_target_visible_rows: Some(target),
                    ..Default::default()
                },
                &[],
                crate::menu_syntax::RootTodoSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::brain::RootBrainSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::notes::RootNotesSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::clipboard_history::RootClipboardHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::dictation::RootDictationHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::ai_vault::RootAiVaultSectionOptions::default(),
                &[],
                crate::browser_tabs::RootBrowserTabsSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::browser_history::RootBrowserHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
                crate::config::UnifiedSearchPassiveResultLimitsConfig::default(),
            );

        let file_count = flat
            .iter()
            .filter(|result| matches!(result, SearchResult::File(_)))
            .count();
        assert_eq!(
            file_count, target,
            "explicit Files source-only browse should use the source-filter target, not the empty-root cap"
        );
    }

    #[test]
    fn recent_files_insert_after_icon_suggested_section() {
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), Some("StarFilled".to_string())),
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Commands".to_string(), Some("Terminal".to_string())),
            GroupedListItem::Item(1),
        ];
        let mut flat = vec![
            SearchResult::File(crate::scripts::FileMatch {
                file: root_file("/Users/example/Desktop/suggested.txt", "suggested.txt"),
                score: 10,
            }),
            SearchResult::File(crate::scripts::FileMatch {
                file: root_file("/Users/example/Desktop/command.txt", "command.txt"),
                score: 9,
            }),
        ];
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];

        append_recent_root_file_section(
            &mut grouped,
            &mut flat,
            &recent_files,
            "",
            None,
            crate::file_search::RootFileSectionOptions::default(),
        );

        assert!(
            matches!(&grouped[4], GroupedListItem::SectionHeader(label, None) if label == "Recent Files"),
            "Recent Files should insert after primary launcher groups"
        );
    }

    #[test]
    fn non_empty_search_does_not_append_recent_files_section() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "recent",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            !grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Recent Files")),
            "non-empty root search should use Files, not Recent Files"
        );
        assert!(
            flat.iter()
                .all(|result| !matches!(result, SearchResult::File(_))),
            "recent files should not leak into non-empty root search"
        );
    }

    #[test]
    fn advanced_query_does_not_append_recent_files_section() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];
        let query = advanced_query_from(":type:file");

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            Some(&query),
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            !grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Recent Files")),
            "advanced query mode should not mix in recent root files"
        );
        assert!(
            flat.iter()
                .all(|result| !matches!(result, SearchResult::File(_))),
            "advanced query mode should not append recent file rows"
        );
    }

    #[test]
    fn recent_files_do_not_create_search_files_handoff_row() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            flat.iter().all(|result| !matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label().starts_with("Search Files for")
            )),
            "empty recent file rows should not create a Search Files continuation row"
        );
    }
}

#[cfg(test)]
mod capture_mode_tests {
    use super::*;
    use crate::menu_syntax::{parse, CaptureInvocation, MenuSyntaxParse};
    use crate::metadata_parser::TypedMetadata;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn capture_from(raw: &str) -> CaptureInvocation {
        match parse(raw) {
            MenuSyntaxParse::Capture(c) => c,
            other => panic!("expected Capture for {raw:?}, got {other:?}"),
        }
    }

    fn script_with_menu_syntax(name: &str, menu_syntax: serde_json::Value) -> Arc<Script> {
        let mut extra: HashMap<String, serde_json::Value> = HashMap::new();
        extra.insert("menuSyntax".to_string(), menu_syntax);
        let mut meta = TypedMetadata::default();
        meta.extra = extra;
        Arc::new(Script {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}.ts")),
            extension: "ts".to_string(),
            description: Some(format!("{name} description")),
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: Some(meta),
            schema: None,
            plugin_id: "main".to_string(),
            plugin_title: None,
            kit_name: None,
            body: None,
        })
    }

    fn plain_script(name: &str) -> Arc<Script> {
        Arc::new(Script {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}.ts")),
            extension: "ts".to_string(),
            description: None,
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: None,
            plugin_id: "main".to_string(),
            plugin_title: None,
            kit_name: None,
            body: None,
        })
    }

    #[test]
    fn zero_handlers_returns_single_help_header_and_no_flat_results() {
        let invocation = capture_from(";todo Renew passport");
        let scripts: Vec<Arc<Script>> = vec![plain_script("unrelated")];
        let (grouped, flat) = build_capture_mode_results(&scripts, &invocation);
        assert_eq!(flat.len(), 0, "no selectable results");
        assert_eq!(grouped.len(), 1, "exactly one help header");
        match &grouped[0] {
            GroupedListItem::SectionHeader(label, None) => {
                assert!(
                    label.contains("capture.v1/todo"),
                    "help header must name the target, got {label:?}"
                );
                assert!(
                    label.contains("No scripts opted"),
                    "help header must explain why"
                );
            }
            other => panic!("expected SectionHeader, got {other:?}"),
        }
    }

    #[test]
    fn only_opted_in_handlers_appear_and_shape_is_header_then_items() {
        let todo_handler = script_with_menu_syntax(
            "todo-handler",
            json!([
                { "family": "capture.v1", "targets": ["todo"] }
            ]),
        );
        let note_handler = script_with_menu_syntax(
            "note-handler",
            json!([
                { "family": "capture.v1", "targets": ["note"] }
            ]),
        );
        let wildcard_handler = script_with_menu_syntax(
            "wildcard-handler",
            json!([
                { "family": "capture.v1", "targets": ["*"] }
            ]),
        );
        let unrelated = plain_script("unrelated");

        let scripts = vec![todo_handler, note_handler, wildcard_handler, unrelated];
        let invocation = capture_from(";todo Renew passport");
        let (grouped, flat) = build_capture_mode_results(&scripts, &invocation);

        assert_eq!(
            flat.len(),
            2,
            "todo + wildcard must match, note and plain must not"
        );
        // First item in grouped must be the section header.
        match &grouped[0] {
            GroupedListItem::SectionHeader(label, None) => {
                assert_eq!(label, "Capture todo");
            }
            other => panic!("first grouped entry must be the capture header, got {other:?}"),
        }
        // The rest must be Item rows in index-order.
        for (expected_idx, entry) in grouped.iter().skip(1).enumerate() {
            match entry {
                GroupedListItem::Item(i) => assert_eq!(*i, expected_idx),
                other => panic!("expected Item({expected_idx}), got {other:?}"),
            }
        }
        let names: Vec<&str> = flat
            .iter()
            .filter_map(|r| match r {
                SearchResult::Script(sm) => Some(sm.script.name.as_str()),
                _ => None,
            })
            .collect();
        assert!(names.contains(&"todo-handler"));
        assert!(names.contains(&"wildcard-handler"));
        assert!(!names.contains(&"note-handler"));
        assert!(!names.contains(&"unrelated"));
    }

    #[test]
    fn non_capture_family_never_matches_even_if_targets_include_target() {
        let impostor = script_with_menu_syntax(
            "impostor",
            json!([
                { "family": "query.v1", "targets": ["todo"] }
            ]),
        );
        let scripts = vec![impostor];
        let invocation = capture_from(";todo Renew passport");
        let (_grouped, flat) = build_capture_mode_results(&scripts, &invocation);
        assert_eq!(
            flat.len(),
            0,
            "non-capture family must never match capture mode"
        );
    }

    #[test]
    fn keyword_alias_matches_same_handlers_as_plus_alias() {
        let handler = script_with_menu_syntax(
            "note-handler",
            json!([{ "family": "capture.v1", "targets": ["note"] }]),
        );
        let scripts = vec![handler];
        let plus = capture_from(";note buy batteries");
        let keyword = capture_from("note: buy batteries");
        let (_, flat_plus) = build_capture_mode_results(&scripts, &plus);
        let (_, flat_keyword) = build_capture_mode_results(&scripts, &keyword);
        assert_eq!(flat_plus.len(), 1);
        assert_eq!(flat_keyword.len(), 1);
    }

    #[test]
    fn incomplete_hint_row_is_single_non_selectable_header() {
        let (grouped, flat) =
            build_menu_syntax_hint_results("Type a capture target: todo, cal, note, social, link");
        assert!(
            flat.is_empty(),
            "incomplete rows never yield selectable results"
        );
        assert_eq!(grouped.len(), 1);
        match &grouped[0] {
            GroupedListItem::SectionHeader(label, None) => {
                assert!(label.contains("todo"));
            }
            other => panic!("expected SectionHeader, got {other:?}"),
        }
        for entry in grouped.iter() {
            assert!(
                !matches!(entry, GroupedListItem::Item(_)),
                "hint rows must never be Item entries (Item maps to a selectable flat result)"
            );
        }
    }

    #[test]
    fn menu_syntax_parse_incomplete_wires_into_hint_helper() {
        use crate::menu_syntax::MenuSyntaxParse;
        match parse("+") {
            MenuSyntaxParse::Incomplete(s) => {
                let (grouped, flat) = build_menu_syntax_hint_results(&s.hint);
                assert!(flat.is_empty());
                assert_eq!(grouped.len(), 1);
                let GroupedListItem::SectionHeader(label, None) = &grouped[0] else {
                    panic!("expected header")
                };
                assert_eq!(label, &s.hint);
            }
            other => panic!("expected Incomplete for '+' , got {other:?}"),
        }
    }

    #[test]
    fn every_result_carries_max_score_for_deterministic_order() {
        let a = script_with_menu_syntax(
            "a",
            json!([{ "family": "capture.v1", "targets": ["todo"] }]),
        );
        let b = script_with_menu_syntax(
            "b",
            json!([{ "family": "capture.v1", "targets": ["todo"] }]),
        );
        let scripts = vec![a, b];
        let invocation = capture_from(";todo something");
        let (_grouped, flat) = build_capture_mode_results(&scripts, &invocation);
        for r in flat {
            match r {
                SearchResult::Script(sm) => {
                    assert_eq!(sm.score, i32::MAX);
                    assert_eq!(sm.match_kind, ScriptMatchKind::Name);
                    assert!(sm.content_match.is_none());
                }
                other => panic!("expected Script, got {other:?}"),
            }
        }
    }
}

#[cfg(test)]
mod active_flow_session_section_tests {
    use super::*;
    use crate::brain::inbox::InboxKind;
    use crate::brain::{InboxItem, RootBrainInboxSectionOptions};
    use crate::flows::model::{FlowDescriptor, FlowSource};
    use crate::flows::session::{FlowSessionMeta, SessionState, SessionTransport};

    fn flow() -> FlowDescriptor {
        FlowDescriptor {
            id: "project:test".into(),
            path: "/tmp/flow-test.md".into(),
            source: FlowSource::Project,
            name: "flow-test".into(),
            description: Some("Test helper".into()),
            engine: "codex".into(),
            engine_source: None,
            inputs: vec![],
            is_workflow: false,
            interactive: true,
            mtime_ms: 0,
            origin: None,
            wrapper_command: None,
        }
    }

    fn session() -> FlowSessionMeta {
        FlowSessionMeta {
            id: 42,
            flow_id: "project:test".into(),
            flow_name: "flow-test".into(),
            friendly_name: "Test".into(),
            origin: "Project".into(),
            engine: "codex".into(),
            flow_path: "/tmp/flow-test.md".into(),
            flow_mtime_ms: 0,
            cwd: "/tmp".into(),
            transport: SessionTransport::CodexThread,
            state: SessionState::NeedsYou,
            started_at: std::time::Instant::now(),
            last_activity: std::time::SystemTime::now(),
            turns: vec![],
            active_turn: None,
            thread_ready: true,
            needs_rethread: false,
            reliability: crate::flows::session::FlowReliability::new(
                "project:test",
                "/tmp/flow-test.md",
                "codex",
            ),
        }
    }

    #[test]
    fn live_sessions_pin_above_brain_and_drop_out_after_termination() {
        let inbox = InboxItem {
            id: 1,
            kind: InboxKind::Commitment,
            title: "Brain row".into(),
            detail: String::new(),
            source: "test".into(),
            source_id: "test-1".into(),
            created_at: 1,
            resolved_at: None,
        };
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &[inbox],
            RootBrainInboxSectionOptions::default(),
            2,
        );
        prepend_root_flow_sessions_section(&mut grouped, &mut flat, "", &[session()], &[flow()]);
        assert!(
            matches!(&grouped[0], GroupedListItem::SectionHeader(label, _) if label == "Active Flows")
        );
        assert!(matches!(&flat[0], SearchResult::Flow(row) if row.session_id == Some(42)));
        assert!(matches!(&flat[1], SearchResult::BrainInboxItem(_)));

        let mut after_grouped = vec![];
        let mut after_flat = vec![];
        prepend_root_flow_sessions_section(&mut after_grouped, &mut after_flat, "", &[], &[flow()]);
        assert!(
            after_grouped.is_empty(),
            "terminated sessions add no launcher row"
        );
        assert!(after_flat.is_empty());
    }

    /// Oracle step 5 (submission 98cab5e5): the section orders by SEMANTIC
    /// recency, not creation order. These use `touch_at` with a controlled
    /// clock so ordering is provable without sleeping.
    #[test]
    fn returning_to_an_older_session_moves_it_back_to_the_top() {
        let epoch = std::time::SystemTime::UNIX_EPOCH;
        let at = |secs: u64| epoch + std::time::Duration::from_secs(secs);

        let mut older = session();
        older.id = 1;
        older.touch_at(at(100));
        let mut newer = session();
        newer.id = 2;
        newer.touch_at(at(200));

        // Creation order alone: newer (id 2) first.
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_flow_sessions_section(
            &mut grouped,
            &mut flat,
            "",
            &[older.clone(), newer.clone()],
            &[flow()],
        );
        assert!(matches!(&flat[0], SearchResult::Flow(row) if row.session_id == Some(2)));

        // The user returns to the OLDER session: it must move to the top even
        // though its id is smaller. This is the observable behavior the spec's
        // build order names as step 1's proof.
        older.touch_at(at(300));
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_flow_sessions_section(&mut grouped, &mut flat, "", &[older, newer], &[flow()]);
        assert!(
            matches!(&flat[0], SearchResult::Flow(row) if row.session_id == Some(1)),
            "resumed session must outrank a newer-created but untouched one"
        );
    }

    #[test]
    fn equal_activity_falls_back_to_stable_id_order() {
        // Equal timestamps must produce deterministic order (id descending),
        // or keyboard selection could watch two rows swap frame-to-frame.
        let epoch = std::time::SystemTime::UNIX_EPOCH;
        let same = epoch + std::time::Duration::from_secs(500);

        let mut a = session();
        a.id = 1;
        a.touch_at(same);
        let mut b = session();
        b.id = 2;
        b.touch_at(same);

        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_flow_sessions_section(&mut grouped, &mut flat, "", &[a, b], &[flow()]);
        assert!(matches!(&flat[0], SearchResult::Flow(row) if row.session_id == Some(2)));
        assert!(matches!(&flat[1], SearchResult::Flow(row) if row.session_id == Some(1)));
    }

    #[test]
    fn live_sessions_match_typed_state_query() {
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_flow_sessions_section(
            &mut grouped,
            &mut flat,
            "needs you",
            &[session()],
            &[flow()],
        );
        assert!(matches!(&flat[0], SearchResult::Flow(row) if row.session_id == Some(42)));
    }
}

#[cfg(test)]
mod brain_inbox_section_tests {
    use super::*;
    use crate::brain::inbox::InboxKind;
    use crate::brain::{InboxItem, RootBrainInboxSectionOptions};

    const NOW: i64 = 1_000_000;

    fn inbox_item(id: i64, title: &str) -> InboxItem {
        InboxItem {
            id,
            kind: InboxKind::Commitment,
            title: title.to_string(),
            detail: String::new(),
            source: "chat_turn".to_string(),
            source_id: format!("thread-{id}#0"),
            created_at: NOW - 3_600,
            resolved_at: None,
        }
    }

    fn existing_row() -> SearchResult {
        SearchResult::ScriptIssue(ScriptIssueMatch {
            title: "Script Issues (1)".into(),
            description: None,
            failed_count: 1,
            fatal_count: 1,
            warning_count: 0,
            score: i32::MAX,
        })
    }

    fn base_view() -> (Vec<GroupedListItem>, Vec<SearchResult>) {
        (
            vec![
                GroupedListItem::SectionHeader("Main".to_string(), None),
                GroupedListItem::Item(0),
            ],
            vec![existing_row()],
        )
    }

    /// Asserts the view still looks exactly like [`base_view`] (no pin).
    fn assert_unpinned(grouped: &[GroupedListItem], flat: &[SearchResult], context: &str) {
        assert_eq!(grouped.len(), 2, "{context}: grouped length changed");
        assert!(
            matches!(&grouped[0], GroupedListItem::SectionHeader(label, None) if label == "Main"),
            "{context}: header changed: {:?}",
            grouped[0]
        );
        assert!(
            matches!(grouped[1], GroupedListItem::Item(0)),
            "{context}: item index shifted: {:?}",
            grouped[1]
        );
        assert_eq!(flat.len(), 1, "{context}: flat length changed");
        assert!(
            matches!(flat[0], SearchResult::ScriptIssue(_)),
            "{context}: flat row replaced"
        );
    }

    #[test]
    fn prepends_header_and_rows_at_top_and_shifts_existing_indices() {
        let (mut grouped, mut flat) = base_view();
        let items = vec![
            inbox_item(1, "follow up with sam"),
            inbox_item(2, "answer rust question"),
        ];
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &items,
            RootBrainInboxSectionOptions::default(),
            NOW,
        );

        assert!(
            matches!(
                &grouped[0],
                GroupedListItem::SectionHeader(label, Some(icon))
                    if label == "Brain Inbox" && icon == "inbox"
            ),
            "section header must be pinned at index 0, got {:?}",
            grouped[0]
        );
        assert!(matches!(grouped[1], GroupedListItem::Item(0)));
        assert!(matches!(grouped[2], GroupedListItem::Item(1)));
        // Existing rows keep pointing at the original results (shifted by 2).
        assert!(matches!(
            &grouped[3],
            GroupedListItem::SectionHeader(label, None) if label == "Main"
        ));
        assert!(matches!(grouped[4], GroupedListItem::Item(2)));
        assert!(matches!(flat[2], SearchResult::ScriptIssue(_)));

        // Rows preserve newest-first input order and carry inbox identity.
        match &flat[0] {
            SearchResult::BrainInboxItem(row) => {
                assert_eq!(row.item.id, 1);
                assert_eq!(
                    flat[0].history_result_key().as_deref(),
                    Some("brain-inbox/1")
                );
                assert!(
                    row.subtitle.starts_with("Commitment · "),
                    "subtitle should lead with the kind label, got {:?}",
                    row.subtitle
                );
            }
            other => panic!("expected BrainInboxItem at flat[0], got {other:?}"),
        }
        assert!(matches!(&flat[1], SearchResult::BrainInboxItem(row) if row.item.id == 2));
    }

    #[test]
    fn caps_rows_at_max_results() {
        let (mut grouped, mut flat) = base_view();
        let items: Vec<InboxItem> = (1..=5)
            .map(|id| inbox_item(id, &format!("item {id}")))
            .collect();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &items,
            RootBrainInboxSectionOptions {
                enabled: true,
                max_results: 3,
            },
            NOW,
        );
        let inbox_rows = flat
            .iter()
            .filter(|row| matches!(row, SearchResult::BrainInboxItem(_)))
            .count();
        assert_eq!(inbox_rows, 3, "rows must be capped at max_results");
    }

    #[test]
    fn no_op_on_non_empty_query_disabled_section_or_empty_items() {
        let items = vec![inbox_item(1, "follow up with sam")];

        // Non-empty query (including whitespace-only being treated as empty).
        let (mut grouped, mut flat) = base_view();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "git",
            &items,
            RootBrainInboxSectionOptions::default(),
            NOW,
        );
        assert_unpinned(&grouped, &flat, "non-empty query");

        // Disabled section.
        let (mut grouped, mut flat) = base_view();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &items,
            RootBrainInboxSectionOptions {
                enabled: false,
                max_results: 3,
            },
            NOW,
        );
        assert_unpinned(&grouped, &flat, "disabled section");

        // max_results == 0.
        let (mut grouped, mut flat) = base_view();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &items,
            RootBrainInboxSectionOptions {
                enabled: true,
                max_results: 0,
            },
            NOW,
        );
        assert_unpinned(&grouped, &flat, "max_results=0");

        // No open items.
        let (mut grouped, mut flat) = base_view();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &[],
            RootBrainInboxSectionOptions::default(),
            NOW,
        );
        assert_unpinned(&grouped, &flat, "empty items");
    }

    #[test]
    fn whitespace_only_query_counts_as_empty() {
        let (mut grouped, mut flat) = base_view();
        let items = vec![inbox_item(1, "follow up with sam")];
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "   ",
            &items,
            RootBrainInboxSectionOptions::default(),
            NOW,
        );
        assert!(
            matches!(
                &grouped[0],
                GroupedListItem::SectionHeader(label, _) if label == "Brain Inbox"
            ),
            "whitespace-only filter is the empty query"
        );
    }
}
