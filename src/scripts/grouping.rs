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

use super::command_contract::{
    record_main_menu_ranking_sections, MainMenuRankingEvidence, MainMenuRankingEvidenceMap,
};
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

fn record_passive_ranking(
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
    rows: &[SearchResult],
    limit: Option<usize>,
    pin_reason: Option<&'static str>,
) {
    let Some(ranking) = ranking else {
        return;
    };
    for (provider_rank, row) in rows.iter().enumerate() {
        let Some(key) = row.stable_selection_key() else {
            continue;
        };
        let provider_score = match row {
            SearchResult::Note(item) => Some(f64::from(item.hit.score)),
            SearchResult::AiVault(item) => Some(f64::from(item.hit.score)),
            SearchResult::BrowserTab(item) => Some(f64::from(item.hit.score)),
            _ => None,
        };
        ranking.insert(
            key,
            MainMenuRankingEvidence {
                provider_rank: (!matches!(row, SearchResult::Flow(_))).then_some(provider_rank),
                provider_score,
                budget_limit: limit,
                admitted_count: Some(rows.len()),
                pin_reason,
                ..MainMenuRankingEvidence::default()
            },
        );
    }
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
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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
            ranking.as_deref_mut(),
        );
    }

    grouped_view::build_grouped_view_results(
        results,
        frecency_store,
        suggested_config,
        flow_discovery,
        ranking,
    )
}

/// Pins a synthetic `SearchResult::ScriptIssue` row at `flat_results[0]` and
/// shifts every existing `GroupedListItem::Item(idx)` by +1 so the rest of
/// the list continues to point at the original results.
///
/// Called from [`get_grouped_results_with_validation`] when validation has
/// recorded excluded scripts, retained blocked scriptlets, or actionable
/// warnings and the surface should show the launcher "Script Issues" row.
pub(crate) fn prepend_script_issues_row(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    validation: &ValidationReport,
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
) {
    let failed_count = validation.failed_scripts.len();
    if failed_count == 0 && validation.retained_issues.is_empty() && validation.warnings.is_empty()
    {
        return;
    }

    let affected_count = validation
        .failed_scripts
        .iter()
        .map(|script| (&script.path, script.name.as_str()))
        .chain(
            validation
                .retained_issues
                .iter()
                .map(|issue| (&issue.path, issue.script_name.as_str())),
        )
        .chain(
            validation
                .warnings
                .iter()
                .map(|issue| (&issue.path, issue.script_name.as_str())),
        )
        .collect::<std::collections::HashSet<_>>()
        .len();
    let retained_count = validation
        .retained_issues
        .iter()
        .map(|issue| (&issue.path, issue.script_name.as_str()))
        .collect::<std::collections::HashSet<_>>()
        .len();
    let fatal_count = validation.fatal_count;
    let warning_count = validation.warning_count;

    let description = if retained_count > 0 {
        Some(format!(
            "{} excluded · {} retained · {} blocking issue{} · {} warning{}",
            failed_count,
            retained_count,
            fatal_count,
            if fatal_count == 1 { "" } else { "s" },
            warning_count,
            if warning_count == 1 { "" } else { "s" }
        ))
    } else if failed_count == 0 {
        Some(format!(
            "{} script{} flagged · {} warning{}",
            affected_count,
            if affected_count == 1 { "" } else { "s" },
            warning_count,
            if warning_count == 1 { "" } else { "s" }
        ))
    } else if fatal_count > 0 && warning_count > 0 {
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
        title: format!("Script Issues ({affected_count})"),
        description,
        failed_count,
        fatal_count,
        warning_count,
        score: i32::MAX, // pinned to the top regardless of sort
    };

    flat_results.insert(0, SearchResult::ScriptIssue(issue));
    if let Some(ranking) = ranking {
        if let Some(key) = flat_results[0].stable_selection_key() {
            ranking.entry(key).or_default().pin_reason = Some("catalog-validation");
        }
    }

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
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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

    record_passive_ranking(
        ranking.as_deref_mut(),
        &rows,
        Some(options.max_results),
        Some("brain-inbox"),
    );
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
    record_main_menu_ranking_sections(ranking, grouped, flat_results);
}

/// Human-readable liveness lane for a Conversations row. The running dot is
/// literal text ("● Working") so it is visible AND readable by the element
/// collector — never color-only, never animated (reduced-motion safe).
fn conversation_state_text(
    liveness: &crate::ai::conversations::ConversationLiveness,
) -> &'static str {
    match liveness {
        crate::ai::conversations::ConversationLiveness::Live { .. } => "● Working",
        crate::ai::conversations::ConversationLiveness::Idle => "Ready",
        crate::ai::conversations::ConversationLiveness::Failed { .. } => "Needs attention",
    }
}

/// Compact relative activity age ("now", "3m", "2h", "5d") for the row's
/// trailing lane. Saturates at days; a negative delta (clock skew) reads
/// "now" rather than inventing future activity.
fn conversation_relative_age(last_activity: std::time::SystemTime, now_unix: i64) -> String {
    let activity_unix = last_activity
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0);
    let elapsed = now_unix.saturating_sub(activity_unix);
    if elapsed < 60 {
        "now".to_string()
    } else if elapsed < 3600 {
        format!("{}m", elapsed / 60)
    } else if elapsed < 86_400 {
        format!("{}h", elapsed / 3600)
    } else {
        format!("{}d", elapsed / 86_400)
    }
}

/// Pin every backgrounded AI conversation — Flow, Agent Chat, Quick AI —
/// above every other launcher row as ONE flat "Conversations" section
/// (spec §8 step 7; Oracle UX ruling 2026-07-25).
///
/// Contract:
/// - One ungrouped list ordered by semantic recency (`last_activity` desc,
///   tagged stable id desc). Surface kind lives in the SUBTITLE, never as a
///   grouping key. No running-first pinning — a stale running operation must
///   not outrank conversations the user touched afterward.
/// - A completed turn stays as an idle resumable row; a failed-but-resumable
///   turn stays marked "Needs attention"; an explicitly closed session is
///   already gone from the store and therefore from this section.
/// - No header and no placeholder when the store is empty — Brain Inbox
///   becomes the first section naturally.
/// - `records` MUST come from `BackgroundedSessionStore::ordered_rows()`;
///   this function never reads history, so a closed Quick AI session cannot
///   resurrect here by construction.
pub(crate) fn prepend_root_conversations_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    filter_text: &str,
    records: &[crate::ai::conversations::ConversationRecord],
    flows: &[crate::flows::model::FlowDescriptor],
    now_unix: i64,
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
) {
    let query = filter_text.trim().to_lowercase();
    let mut records: Vec<&crate::ai::conversations::ConversationRecord> = records
        .iter()
        .filter(|record| {
            query.is_empty()
                || record.title.to_lowercase().contains(&query)
                || record.subtitle.to_lowercase().contains(&query)
                || conversation_state_text(&record.liveness)
                    .to_lowercase()
                    .contains(&query)
        })
        .collect();
    // Defensive re-sort with the canonical rule: the store already orders,
    // but this function's contract must hold for any caller.
    records.sort_by(|a, b| {
        b.last_activity
            .cmp(&a.last_activity)
            .then_with(|| b.id.cmp(&a.id))
    });

    let rows: Vec<SearchResult> = records
        .into_iter()
        .map(|record| {
            // A flow conversation keeps its descriptor when the flow is
            // still on disk; a missing descriptor must NOT drop the row —
            // resume only needs the session id, and silently hiding a live
            // session would orphan its in-flight turn.
            let flow = match &record.surface {
                crate::ai::conversations::ConversationSurface::Flow { flow_id } => {
                    flows.iter().find(|flow| &flow.id == flow_id).cloned()
                }
                _ => None,
            };
            SearchResult::Flow(crate::scripts::FlowMatch {
                flow,
                target: crate::scripts::ConversationRowTarget::Conversation(record.id.clone()),
                display_name: record.title.clone(),
                subtitle: format!(
                    "{} · {} · {}",
                    record.subtitle,
                    conversation_state_text(&record.liveness),
                    conversation_relative_age(record.last_activity, now_unix),
                ),
                score: i32::MAX,
                match_indices: crate::scripts::MatchIndices::default(),
            })
        })
        .collect();
    if rows.is_empty() {
        return;
    }

    record_passive_ranking(
        ranking.as_deref_mut(),
        &rows,
        None,
        Some("background-conversation-recency"),
    );
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
        "Conversations".to_string(),
        Some("message-circle".to_string()),
    ));
    section.extend((0..shift).map(GroupedListItem::Item));
    grouped.splice(0..0, section);
    record_main_menu_ranking_sections(ranking, grouped, flat_results);
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
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
) {
    let flat_idx = match flat_results.iter().position(is_alias_target) {
        Some(idx) => idx,
        None => {
            flat_results.push(fallback());
            flat_results.len() - 1
        }
    };
    if let Some(ranking) = ranking {
        if let Some(key) = flat_results[flat_idx].stable_selection_key() {
            ranking.entry(key).or_default().pin_reason = Some("exact-alias");
        }
    }

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
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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
        ranking.as_deref_mut(),
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
            if (!report.failed_scripts.is_empty()
                || !report.retained_issues.is_empty()
                || !report.warnings.is_empty())
                && !advanced_query_rejects_issue(advanced_query)
            {
                prepend_script_issues_row(&mut grouped, &mut flat_results, report, ranking);
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
        None,
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
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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
        ranking.as_deref_mut(),
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
            ranking.as_deref_mut(),
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
            ranking.as_deref_mut(),
        );
        append_recent_root_file_section(
            &mut grouped,
            &mut flat_results,
            root_recent_file_results,
            filter_text,
            advanced_query,
            root_file_options,
            ranking.as_deref_mut(),
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
                    ranking.as_deref_mut(),
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
                    ranking.as_deref_mut(),
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
                    ranking.as_deref_mut(),
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
                    ranking.as_deref_mut(),
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
                    ranking.as_deref_mut(),
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
                    ranking.as_deref_mut(),
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
                    ranking.as_deref_mut(),
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
                    ranking.as_deref_mut(),
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
                    ranking.as_deref_mut(),
                );
            }
        }
    }

    append_missing_explicit_source_status_rows(&mut grouped, &flat_results, root_source_filters);
    record_main_menu_ranking_sections(ranking, &grouped, &flat_results);

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
            GroupedListItem::SectionHeader(..) | GroupedListItem::ReservedSectionSlot => false,
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
            GroupedListItem::ReservedSectionSlot => {
                pending_header = Some(GroupedListItem::ReservedSectionSlot);
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
            GroupedListItem::SectionHeader(_, _) | GroupedListItem::ReservedSectionSlot => {
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
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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

    record_passive_ranking(ranking.as_deref_mut(), &rows, Some(limit), None);
    if let Some(ranking) = ranking {
        for (row, hit) in rows.iter().zip(hits) {
            if let Some(facts) = row
                .stable_selection_key()
                .and_then(|key| ranking.get_mut(&key))
            {
                facts.provider_score = Some(f64::from(hit.score));
                facts.match_evidence = hit.evidence.as_ref().and_then(|original| {
                    let mut evidence = row.ranking_evidence();
                    evidence.score = i32::try_from(hit.score).ok()?;
                    evidence.tier = original.tier as i32;
                    Some(evidence)
                });
            }
        }
    }
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
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
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
    if ranking.is_some() {
        let pin = (insertion_index < root_file_passive_insertion_index(grouped, flat_results))
            .then_some("brain-before-file-handoff");
        record_passive_ranking(ranking, &rows, Some(limit), pin);
    }
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
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
) {
    if advanced_query.is_some() || !crate::notes::root_notes_query_is_eligible(filter_text, options)
    {
        return;
    }

    let limit = budget.limit_for_source(options.max_results);
    if limit == 0 && !explicit_source_filter {
        return;
    }

    let display_now_ms = crate::runtime_policy::root_search_display_unix_ms();
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
            let updated = crate::formatting::format_relative_time_short_millis_at(
                hit.updated_at.timestamp_millis(),
                display_now_ms,
            );
            SearchResult::Note(crate::scripts::NoteMatch {
                hit: hit.clone(),
                title,
                subtitle: format!("{pinned}Updated {updated} · {} chars", hit.char_count),
                score: root_passive_result_score(rank),
            })
        })
        .collect::<Vec<_>>();

    record_passive_ranking(ranking, &rows, Some(limit), None);
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
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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

    record_passive_ranking(ranking.as_deref_mut(), &rows, Some(limit), None);
    if let Some(ranking) = ranking {
        let provider_ranks: std::collections::HashMap<_, _> = hits
            .iter()
            .enumerate()
            .map(|(rank, hit)| (hit.stable_key.as_str(), rank))
            .collect();
        for row in &rows {
            if let SearchResult::Todo(item) = row {
                if let Some(facts) = ranking.get_mut(&item.hit.stable_key) {
                    facts.provider_rank = provider_ranks.get(item.hit.stable_key.as_str()).copied();
                }
            }
        }
    }
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
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
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

    let display_now_ms = crate::runtime_policy::root_search_display_unix_ms();
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
            let time = crate::formatting::format_relative_time_short_millis_at(
                entry.timestamp,
                display_now_ms,
            );
            SearchResult::ClipboardHistory(crate::scripts::ClipboardHistoryMatch {
                entry: entry.clone(),
                title: entry.display_preview(),
                subtitle: format!("{pinned}{content_type} · {time}"),
                score: root_passive_result_score(rank),
            })
        })
        .collect::<Vec<_>>();

    record_passive_ranking(ranking, &rows, Some(limit), None);
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
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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

    record_passive_ranking(ranking.as_deref_mut(), &rows, Some(limit), None);
    if let Some(ranking) = ranking {
        for (row, hit) in rows.iter().zip(hits) {
            if let Some(facts) = row
                .stable_selection_key()
                .and_then(|key| ranking.get_mut(&key))
            {
                facts.provider_score = Some(f64::from(hit.score));
                facts.match_evidence = hit.evidence.as_ref().and_then(|original| {
                    let mut evidence = row.ranking_evidence();
                    evidence.score = i32::try_from(hit.score).ok()?;
                    evidence.tier = original.tier as i32;
                    Some(evidence)
                });
            }
        }
    }
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
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
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

    record_passive_ranking(
        ranking,
        &rows,
        Some(limit),
        domain_intent.then_some("bare-domain-tabs"),
    );
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
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
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

    let display_now_ms = crate::runtime_policy::root_search_display_unix_ms();
    let rows = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank, hit)| {
            let time = crate::formatting::format_relative_time_short_millis_at(
                hit.last_visit_unix_ms,
                display_now_ms,
            );
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

    record_passive_ranking(ranking, &rows, Some(limit), None);
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
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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
        .enumerate()
        .filter(|(_, file)| {
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

    let admitted_count = eligible_recent_files.len();
    let mut recent_group = Vec::with_capacity(eligible_recent_files.len() + 2);
    recent_group.push(GroupedListItem::SectionHeader(
        "Recent Files".to_string(),
        None,
    ));
    for (rank, (provider_rank, file)) in eligible_recent_files.into_iter().enumerate() {
        let idx = flat_results.len();
        flat_results.push(SearchResult::File(crate::scripts::FileMatch {
            file: file.clone(),
            score: i32::MAX.saturating_sub(rank as i32),
        }));
        if let Some(ranking) = ranking.as_deref_mut() {
            if let Some(key) = flat_results[idx].stable_selection_key() {
                ranking.insert(
                    key,
                    MainMenuRankingEvidence {
                        provider_rank: Some(provider_rank),
                        budget_limit: Some(
                            options
                                .source_filter_browse_target_visible_rows
                                .unwrap_or(crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT),
                        ),
                        admitted_count: Some(admitted_count),
                        section: Some("Recent Files".to_string()),
                        ..MainMenuRankingEvidence::default()
                    },
                );
            }
        }
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
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
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

    record_passive_ranking(ranking, &rows, Some(limit), None);
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
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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
        if let Some(ranking) = ranking.as_deref_mut() {
            ranking.insert(
                format!("file/{}", file_match.file.path),
                MainMenuRankingEvidence {
                    score: Some(file_match.score),
                    section: Some(ui_state.section_label.clone()),
                    budget_limit: Some(options.source_chip_visible_limit.unwrap_or(match mode {
                        crate::file_search::RootFileSectionMode::GlobalQuery => {
                            crate::file_search::ROOT_FILE_RENDER_LIMIT
                        }
                        crate::file_search::RootFileSectionMode::DirectoryBrowse => {
                            crate::file_search::ROOT_FILE_BROWSE_RENDER_LIMIT
                        }
                    })),
                    admitted_count: Some(ui_state.visible_file_count),
                    pin_reason: promote.then_some("file-section-promotion"),
                    ..MainMenuRankingEvidence::default()
                },
            );
        }
        let idx = flat_results.len();
        flat_results.push(SearchResult::File(file_match));
        file_group.push(GroupedListItem::Item(idx));
    }
    if let Some(handoff) = handoff {
        if let (Some(ranking), Some(key)) = (ranking, handoff.stable_selection_key()) {
            ranking.entry(key).or_default().pin_reason = Some("file-search-handoff");
        }
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
            GroupedListItem::SectionHeader(_, Some(_)) | GroupedListItem::ReservedSectionSlot => {
                false
            }
        })
        .unwrap_or(grouped.len())
}

// Match the other root-section appenders without introducing a one-off argument bundle.
#[allow(clippy::too_many_arguments)]
fn append_root_windows_section(
    grouped: &mut Vec<GroupedListItem>,
    flat_results: &mut Vec<SearchResult>,
    windows: &[crate::scripts::RootWindowEntry],
    provider_status: crate::window_control::RootWindowsProviderStatus,
    filter_text: &str,
    advanced_query: Option<&crate::menu_syntax::AdvancedQuery>,
    explicit_source_filter: bool,
    mut ranking: Option<&mut MainMenuRankingEvidenceMap>,
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
        if let Some(ranking) = ranking.as_deref_mut() {
            if let Some(key) = flat_results[idx].stable_selection_key() {
                let mut facts = MainMenuRankingEvidence::active(&flat_results[idx]);
                facts.section = Some("Windows".to_string());
                facts.admitted_count = Some(shown);
                ranking.insert(key, facts);
            }
        }
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
include!("grouping_tests.rs");
