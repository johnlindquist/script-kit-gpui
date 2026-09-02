use std::cmp::Reverse;
use std::sync::Arc;

use crate::app_launcher::AppInfo;
use crate::builtins::BuiltInEntry;
use crate::plugins::PluginSkill;
use crate::window_control::WindowInfo;

use super::super::types::{Script, Scriptlet, SearchResult};
use super::flows::fuzzy_search_flows;
use super::prefix_filters::flow_passes_prefix_filter;
use super::{
    app_passes_prefix_filter, builtin_passes_prefix_filter, fuzzy_search_apps,
    fuzzy_search_builtins, fuzzy_search_scriptlets, fuzzy_search_scripts, fuzzy_search_skills,
    fuzzy_search_windows, parse_query_prefix, script_passes_prefix_filter,
    scriptlet_passes_prefix_filter, should_search_scriptlets, should_search_scripts,
    skill_passes_prefix_filter, window_passes_prefix_filter,
};

use crate::flows::model::FlowDescriptor;

#[inline]
pub(crate) fn result_type_order(r: &SearchResult) -> i32 {
    match r {
        SearchResult::Flow(_) => -1, // Flows are the primary experience — above everything
        SearchResult::BuiltIn(_) => 0, // Built-ins first
        SearchResult::App(_) => 1,   // Apps second
        SearchResult::Window(_) => 2, // Windows third
        SearchResult::Skill(_) => 3, // Skills promoted above scripts/scriptlets
        SearchResult::Script(_) => 4,
        SearchResult::Scriptlet(_) => 5,
        SearchResult::File(_) => 6,
        SearchResult::BrowserTab(_) => 7,
        SearchResult::Todo(_) => 8,
        SearchResult::BrainHit(_) => 9,
        // Brain inbox rows are pinned separately via grouping; if encountered
        // in sort, keep them beside other brain rows.
        SearchResult::BrainInboxItem(_) => 9,
        SearchResult::Note(_) => 10,
        SearchResult::ClipboardHistory(_) => 11,
        SearchResult::DictationHistory(_) => 12,
        SearchResult::AgentChatHistory(_) => 13,
        SearchResult::AiVault(_) => 14,
        SearchResult::BrowserHistory(_) => 15,
        SearchResult::Agent(_) => 16,
        SearchResult::Fallback(_) => 17, // Fallbacks always last
        // Script issues are pinned separately via grouping; if encountered in
        // sort, treat like a high-priority header (above built-ins).
        SearchResult::ScriptIssue(_) => -1,
        // Spine projections sort after fallbacks
        SearchResult::SpineProjection(_) => 18,
    }
}

/// Perform unified fuzzy search across scripts, scriptlets, and built-ins
/// Returns combined and ranked results sorted by relevance
/// Built-ins appear at the TOP of results (before scripts) when scores are equal
///
/// H1 Optimization: Accepts Arc<Script> and Arc<Scriptlet> to avoid expensive clones.
pub fn fuzzy_search_unified(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    query: &str,
) -> Vec<SearchResult> {
    fuzzy_search_unified_with_builtins(scripts, scriptlets, &[], query)
}

/// Perform unified fuzzy search across scripts, scriptlets, and built-ins
/// Returns combined and ranked results sorted by relevance
/// Built-ins appear at the TOP of results (before scripts) when scores are equal
///
/// H1 Optimization: Accepts Arc<Script> and Arc<Scriptlet> to avoid expensive clones.
pub fn fuzzy_search_unified_with_builtins(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    query: &str,
) -> Vec<SearchResult> {
    // Use the new function with empty apps list for backwards compatibility
    fuzzy_search_unified_all(scripts, scriptlets, builtins, &[], query)
}

/// Perform unified fuzzy search across scripts, scriptlets, skills, built-ins, and apps
/// Returns combined and ranked results sorted by relevance
/// Built-ins appear at the TOP of results (before scripts) when scores are equal
/// Apps appear after built-ins but before scripts when scores are equal
///
/// H1 Optimization: Accepts Arc<Script> and Arc<Scriptlet> to avoid expensive clones.
pub fn fuzzy_search_unified_all(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    query: &str,
) -> Vec<SearchResult> {
    fuzzy_search_unified_all_with_skills(scripts, scriptlets, builtins, apps, &[], query)
}

/// Perform unified fuzzy search including plugin skills.
pub fn fuzzy_search_unified_all_with_skills(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    skills: &[Arc<PluginSkill>],
    query: &str,
) -> Vec<SearchResult> {
    fuzzy_search_unified_all_with_skills_and_flows(
        scripts,
        scriptlets,
        builtins,
        apps,
        skills,
        &[],
        query,
    )
}

/// Perform unified fuzzy search including plugin skills and mdflow flows.
/// Flows are the primary launcher rows and rank above every other kind on
/// equal scores (see `result_type_order`).
#[allow(clippy::too_many_arguments)]
pub fn fuzzy_search_unified_all_with_skills_and_flows(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    skills: &[Arc<PluginSkill>],
    flows: &[FlowDescriptor],
    query: &str,
) -> Vec<SearchResult> {
    use crate::logging;
    let total_start = std::time::Instant::now();
    let mut results = Vec::new();

    // Parse prefix filter from query
    let parsed = parse_query_prefix(query);
    let search_query = if parsed.filter_kind.is_some() {
        parsed.remainder.as_str()
    } else {
        query
    };

    // Search built-ins first (skip if prefix filter excludes them)
    let builtin_start = std::time::Instant::now();
    if builtin_passes_prefix_filter(&parsed) {
        let builtin_matches = fuzzy_search_builtins(builtins, search_query);
        for bm in builtin_matches {
            results.push(SearchResult::BuiltIn(bm));
        }
    }
    let builtin_elapsed = builtin_start.elapsed();

    // Search apps (skip if prefix filter excludes them)
    let apps_start = std::time::Instant::now();
    if app_passes_prefix_filter(&parsed) {
        let app_matches = fuzzy_search_apps(apps, search_query);
        for am in app_matches {
            results.push(SearchResult::App(am));
        }
    }
    let apps_elapsed = apps_start.elapsed();

    // Search scripts (skip if filter excludes scripts as a category)
    let scripts_start = std::time::Instant::now();
    if should_search_scripts(&parsed) {
        let script_matches = fuzzy_search_scripts(scripts, search_query);
        for sm in script_matches {
            // Post-filter by prefix filter (tag, author, kit, is)
            if script_passes_prefix_filter(&sm.script, &parsed) {
                results.push(SearchResult::Script(sm));
            }
        }
    }
    let scripts_elapsed = scripts_start.elapsed();

    // Search scriptlets (skip if filter excludes scriptlets as a category)
    let scriptlets_start = std::time::Instant::now();
    if should_search_scriptlets(&parsed) {
        let scriptlet_matches = fuzzy_search_scriptlets(scriptlets, search_query);
        for sm in scriptlet_matches {
            if scriptlet_passes_prefix_filter(&sm.scriptlet, &parsed) {
                results.push(SearchResult::Scriptlet(sm));
            }
        }
    }
    let scriptlets_elapsed = scriptlets_start.elapsed();

    // Search plugin skills (always included, not affected by prefix filters)
    let skills_start = std::time::Instant::now();
    if !skills.is_empty() && skill_passes_prefix_filter(&parsed) {
        let skill_matches = fuzzy_search_skills(skills, search_query);
        for sm in skill_matches {
            tracing::info!(
                plugin_id = %sm.skill.plugin_id,
                skill_id = %sm.skill.skill_id,
                score = sm.score,
                "main_menu_skill_ranked"
            );
            results.push(SearchResult::Skill(sm));
        }
    }
    let skills_elapsed = skills_start.elapsed();

    // Search mdflow flows — the primary experience; included unless a
    // prefix filter targets another kind.
    if !flows.is_empty() && flow_passes_prefix_filter(&parsed) {
        let flow_matches = fuzzy_search_flows(flows, search_query);
        for fm in flow_matches {
            results.push(SearchResult::Flow(fm));
        }
    }

    // Log search timing breakdown
    if !query.is_empty() && logging::filter_perf_trace_enabled() {
        logging::log(
            "FILTER_PERF",
            &format!(
                "[SEARCH_BREAKDOWN] '{}': builtins={:.2}ms apps={:.2}ms scripts={:.2}ms scriptlets={:.2}ms skills={:.2}ms",
                query,
                builtin_elapsed.as_secs_f64() * 1000.0,
                apps_elapsed.as_secs_f64() * 1000.0,
                scripts_elapsed.as_secs_f64() * 1000.0,
                scriptlets_elapsed.as_secs_f64() * 1000.0,
                skills_elapsed.as_secs_f64() * 1000.0,
            ),
        );
    }

    // Suppress legacy Agent results from the launcher pipeline.
    // Agent artifacts are handled inside Agent Chat internals, not the main menu.
    // Skills replace agents as the first-class reusable AI artifact.
    let pre_suppress_len = results.len();
    results.retain(|r| {
        if r.is_suppressed_agent() {
            tracing::info!(
                event = "legacy_agent_result_suppressed",
                agent_name = r.name(),
                "Agent result filtered from launcher search pipeline"
            );
            false
        } else {
            true
        }
    });
    let suppressed_count = pre_suppress_len - results.len();
    if suppressed_count > 0 {
        tracing::info!(
            suppressed_count,
            event = "legacy_agent_results_filtered",
            "Filtered legacy agent results from launcher pipeline"
        );
    }

    // Durable identity is only the final tie-break; cache source hashes once.
    let sort_start = std::time::Instant::now();
    results.sort_by_cached_key(|result| {
        (
            Reverse(result.match_tier()),
            Reverse(result.score()),
            result_type_order(result),
            result.name().to_owned(),
            result.stable_selection_key(),
        )
    });

    // Emit post-sort top-skill signal for observability
    if let Some((rank, SearchResult::Skill(sm))) = results
        .iter()
        .enumerate()
        .find(|(_, result)| matches!(result, SearchResult::Skill(_)))
    {
        tracing::info!(
            event = "main_menu_top_skill_rank_after_sort",
            rank,
            plugin_id = %sm.skill.plugin_id,
            skill_id = %sm.skill.skill_id,
            score = sm.score,
            "Top-ranked skill position after unified search sort"
        );
    }

    // Log sort and total timing
    if !query.is_empty() {
        let sort_elapsed = sort_start.elapsed();
        let total_elapsed = total_start.elapsed();
        if logging::filter_perf_trace_enabled()
            || total_elapsed >= std::time::Duration::from_millis(8)
        {
            logging::log(
                "FILTER_PERF",
                &format!(
                    "[SEARCH_TOTAL] '{}': sort={:.2}ms total={:.2}ms ({} results)",
                    query,
                    sort_elapsed.as_secs_f64() * 1000.0,
                    total_elapsed.as_secs_f64() * 1000.0,
                    results.len()
                ),
            );
        }
    }

    results
}

/// Perform unified fuzzy search across scripts, scriptlets, built-ins, apps, and windows
/// Returns combined and ranked results sorted by relevance
/// Order by type when scores are equal: Built-ins > Apps > Windows > Scripts > Scriptlets
///
/// H1 Optimization: Accepts Arc<Script> and Arc<Scriptlet> to avoid expensive clones.
pub fn fuzzy_search_unified_with_windows(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    windows: &[WindowInfo],
    query: &str,
) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // Parse prefix filter from query
    let parsed = parse_query_prefix(query);
    let search_query = if parsed.filter_kind.is_some() {
        parsed.remainder.as_str()
    } else {
        query
    };

    // Search built-ins first (skip if prefix filter excludes them)
    if builtin_passes_prefix_filter(&parsed) {
        let builtin_matches = fuzzy_search_builtins(builtins, search_query);
        for bm in builtin_matches {
            results.push(SearchResult::BuiltIn(bm));
        }
    }

    // Search apps (skip if prefix filter excludes them)
    if app_passes_prefix_filter(&parsed) {
        let app_matches = fuzzy_search_apps(apps, search_query);
        for am in app_matches {
            results.push(SearchResult::App(am));
        }
    }

    // Search windows (skip if prefix filter excludes them)
    if window_passes_prefix_filter(&parsed) {
        let window_matches = fuzzy_search_windows(windows, search_query);
        for wm in window_matches {
            results.push(SearchResult::Window(wm));
        }
    }

    // Search scripts (skip if filter excludes scripts as a category)
    if should_search_scripts(&parsed) {
        let script_matches = fuzzy_search_scripts(scripts, search_query);
        for sm in script_matches {
            if script_passes_prefix_filter(&sm.script, &parsed) {
                results.push(SearchResult::Script(sm));
            }
        }
    }

    // Search scriptlets (skip if filter excludes scriptlets as a category)
    if should_search_scriptlets(&parsed) {
        let scriptlet_matches = fuzzy_search_scriptlets(scriptlets, search_query);
        for sm in scriptlet_matches {
            if scriptlet_passes_prefix_filter(&sm.scriptlet, &parsed) {
                results.push(SearchResult::Scriptlet(sm));
            }
        }
    }

    // Suppress legacy Agent results (defensive — same as primary search path)
    results.retain(|r| {
        if r.is_suppressed_agent() {
            tracing::info!(
                event = "legacy_agent_result_suppressed",
                agent_name = r.name(),
                "Agent result filtered from window search pipeline"
            );
            false
        } else {
            true
        }
    });

    // Preserve tier, score, type and name precedence before exact identity.
    results.sort_by_cached_key(|result| {
        (
            Reverse(result.match_tier()),
            Reverse(result.score()),
            result_type_order(result),
            result.name().to_owned(),
            result.stable_selection_key(),
        )
    });

    results
}

#[cfg(test)]
mod deterministic_tie_tests {
    use super::*;

    fn app(path: &str, name: &str) -> AppInfo {
        AppInfo {
            name: name.into(),
            path: path.into(),
            bundle_id: None,
            icon: None,
        }
    }

    #[test]
    fn equal_name_apps_have_deterministic_final_ties_without_changing_relevance() {
        let apps = [
            app("/Applications/A/Editor.app", "Editor"),
            app("/Applications/B/Editor.app", "Editor"),
            app("/Applications/Tools.app", "Editor Tools"),
        ];
        let orders = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let mut expected = None;
        for order in orders {
            let input: Vec<_> = order.into_iter().map(|i| apps[i].clone()).collect();
            for results in [
                fuzzy_search_unified_all_with_skills_and_flows(
                    &[],
                    &[],
                    &[],
                    &input,
                    &[],
                    &[],
                    "Editor",
                ),
                fuzzy_search_unified_with_windows(&[], &[], &[], &input, &[], "Editor"),
            ] {
                assert_eq!(results.len(), 3);
                assert_eq!(results[0].name(), "Editor");
                assert_eq!(results[1].name(), "Editor");
                assert_eq!(results[0].score(), results[1].score());
                assert!(results[1].score() > results[2].score());
                let keys: Vec<_> = results
                    .iter()
                    .map(|result| result.stable_selection_key().unwrap())
                    .collect();
                assert!(keys[0] < keys[1]);
                match &expected {
                    Some(expected) => assert_eq!(&keys, expected),
                    None => expected = Some(keys),
                }
            }
        }
    }
}
