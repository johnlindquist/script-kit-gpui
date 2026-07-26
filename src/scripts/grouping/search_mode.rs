use std::sync::Arc;
use tracing::debug;

use crate::builtins::BuiltInGroup;
use crate::fallbacks::collector::collect_fallbacks;
use crate::frecency::FrecencyStore;
use crate::list_item::GroupedListItem;

use super::super::types::{FallbackMatch, Script, SearchResult};
use super::{MAX_MENU_BAR_ITEMS, MIN_MENU_BAR_SCORE};

pub(super) fn build_search_mode_results(
    mut results: Vec<SearchResult>,
    scripts: &[Arc<Script>],
    frecency_store: &FrecencyStore,
    filter_text: &str,
    preferred_result_key: Option<&str>,
    launcher_context: Option<&crate::context_snapshot::launcher_context::LauncherContextSnapshot>,
    suppress_fallbacks: bool,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    // Apply frecency boost: recently/frequently used items get a score bonus.
    // This is how modern launchers (Raycast, Alfred, Spotlight) work.
    // The bonus is capped so a good fuzzy match still beats a poor match with high frecency.
    {
        let max_frecency_bonus = 50i32;
        let preferred_match_bonus = 500i32;

        // Helper to get the frecency path for a result (mirrors grouped-view logic).
        // Skills and scriptlets use plugin-qualified keys.
        let get_path = |result: &SearchResult| -> Option<String> {
            match result {
                SearchResult::Script(sm) => Some(sm.script.path.to_string_lossy().to_string()),
                SearchResult::App(am) => Some(am.app.path.to_string_lossy().to_string()),
                SearchResult::BuiltIn(bm) => Some(format!("builtin:{}", bm.entry.id)),
                SearchResult::Scriptlet(sm) => Some(format!(
                    "scriptlet:{}:{}",
                    sm.scriptlet.plugin_id, sm.scriptlet.name
                )),
                SearchResult::Flow(fm) => fm.flow.as_ref().map(|flow| format!("flow:{}", flow.id)),
                SearchResult::Skill(sm) => Some(format!(
                    "skill:{}:{}",
                    sm.skill.plugin_id, sm.skill.skill_id
                )),
                SearchResult::Window(wm) => {
                    Some(format!("window:{}:{}", wm.window.app, wm.window.title))
                }
                SearchResult::File(fm) => Some(format!("file/{}", fm.file.path)),
                SearchResult::Note(nm) => Some(format!("note/{}", nm.hit.id.as_str())),
                // Brain rows never receive frecency boosts; passive search
                // must not feed usage memory (it would self-amplify).
                SearchResult::BrainHit(_) => None,
                // Brain inbox rows are pinned on the empty query only and
                // never receive frecency boosts.
                SearchResult::BrainInboxItem(_) => None,
                SearchResult::Todo(tm) => Some(tm.hit.stable_key.clone()),
                SearchResult::AgentChatHistory(am) => {
                    Some(format!("agent_chat-history/{}", am.entry.session_id))
                }
                SearchResult::AiVault(am) => Some(am.hit.stable_key.clone()),
                SearchResult::ClipboardHistory(cm) => {
                    Some(format!("clipboard-history/{}", cm.entry.id))
                }
                SearchResult::DictationHistory(dm) => Some(format!("dictation-history/{}", dm.id)),
                SearchResult::BrowserTab(_) => None,
                SearchResult::BrowserHistory(bm) => Some(bm.hit.stable_key.clone()),
                // Suppressed: agents don't participate in search-mode frecency
                SearchResult::Agent(_) => None,
                SearchResult::Fallback(_) => None,
                // Script issues row is pinned synthetically; no frecency
                SearchResult::ScriptIssue(_) => None,
                // Spine projections don't participate in search-mode frecency
                SearchResult::SpineProjection(_) => None,
            }
        };

        let reserved_builtin_key =
            reserved_exact_builtin_preferred_result_key(&results, filter_text);
        let effective_preferred_result_key =
            reserved_builtin_key.as_deref().or(preferred_result_key);

        // Pre-compute boosted score for every result
        let boosted: Vec<i32> = results
            .iter()
            .map(|result| {
                let frecency_bonus = if let Some(path) = get_path(result) {
                    let score = frecency_store.get_score(&path);
                    if score > 0.0 {
                        // Scale frecency (typically 0-100+) via log so very high values
                        // don't dominate. At least 1 point bonus for any frecency > 0.
                        let scaled =
                            (score.ln().max(0.0) * 10.0).min(max_frecency_bonus as f64) as i32;
                        scaled.max(1)
                    } else {
                        0
                    }
                } else {
                    0
                };
                let exact_query_bonus = effective_preferred_result_key
                    .and_then(|preferred| result.history_result_key().map(|key| key == preferred))
                    .map(|is_match| if is_match { preferred_match_bonus } else { 0 })
                    .unwrap_or(0);
                let context_bonus = launcher_context
                    .map(|ctx| {
                        crate::context_snapshot::launcher_context::context_boost_for_result(
                            result, ctx,
                        )
                    })
                    .unwrap_or(0);
                result
                    .score()
                    .saturating_add(frecency_bonus)
                    .saturating_add(exact_query_bonus)
                    .saturating_add(context_bonus)
            })
            .collect();

        // Build an index array sorted by relevance tier first. Frecency and
        // preferred-result memory only affect ordering inside the same tier.
        let mut sort_indices: Vec<usize> = (0..results.len()).collect();
        sort_indices.sort_by(|&a, &b| {
            results[b]
                .match_tier()
                .cmp(&results[a].match_tier())
                .then_with(|| boosted[b].cmp(&boosted[a]))
                .then_with(|| {
                    // Same type-priority tie-break as the raw unified sort, so
                    // equal-tier, equal-score rows keep builtin/app/window/script
                    // ordering instead of collapsing to alphabetical.
                    crate::scripts::search::result_type_order(&results[a])
                        .cmp(&crate::scripts::search::result_type_order(&results[b]))
                })
                .then_with(|| results[a].name().cmp(results[b].name()))
        });

        // Re-order results according to boosted sort
        let reordered: Vec<SearchResult> = sort_indices
            .into_iter()
            .map(|i| results[i].clone())
            .collect();
        results = reordered;
    }

    let mut grouped: Vec<GroupedListItem> = Vec::new();

    let mut menu_bar_count = 0usize;
    let mut in_menu_bar_run = false;

    for (idx, result) in results.iter().enumerate() {
        let is_menu_bar_result = matches!(
            result,
            SearchResult::BuiltIn(bm)
                if bm.entry.group == BuiltInGroup::MenuBar
                    && bm.score >= MIN_MENU_BAR_SCORE
                    && menu_bar_count < MAX_MENU_BAR_ITEMS
        );

        if matches!(
            result,
            SearchResult::BuiltIn(bm) if bm.entry.group == BuiltInGroup::MenuBar
        ) && !is_menu_bar_result
        {
            continue;
        }

        if is_menu_bar_result {
            if !in_menu_bar_run {
                grouped.push(GroupedListItem::SectionHeader(
                    "Menu Bar Actions".to_string(),
                    None,
                ));
            }
            in_menu_bar_run = true;
            menu_bar_count += 1;
        } else {
            in_menu_bar_run = false;
        }

        grouped.push(GroupedListItem::Item(idx));
    }

    let has_other_results = !grouped.is_empty();

    // Collect fallback commands and append as "Use {query} with..." section
    let fallbacks = if suppress_fallbacks {
        Vec::new()
    } else {
        collect_fallbacks(filter_text, scripts)
    };
    let fallback_count = fallbacks.len();

    if !fallbacks.is_empty() {
        // Always show "Use X with..." header (no icon)
        grouped.push(GroupedListItem::SectionHeader(
            format!("Use \"{}\" with...", filter_text),
            None,
        ));

        // Append fallback items to the results vec and add their indices to grouped
        for fallback in fallbacks {
            let idx = results.len();
            results.push(SearchResult::Fallback(FallbackMatch::new(fallback, 0)));
            grouped.push(GroupedListItem::Item(idx));
        }
    }

    if matches!(grouped.first(), Some(GroupedListItem::Item(_))) {
        grouped.insert(
            0,
            GroupedListItem::SectionHeader("Results".to_string(), None),
        );
    }

    let fallbacks_elevated = fallback_count > 0 && !has_other_results;
    debug!(
        result_count = results.len(),
        menu_bar_count,
        fallback_count,
        fallbacks_elevated,
        "Search mode: returning list with menu bar and fallback sections"
    );

    (grouped, results)
}

fn reserved_exact_builtin_preferred_result_key(
    results: &[SearchResult],
    filter_text: &str,
) -> Option<String> {
    let normalized = filter_text.trim().to_ascii_lowercase();
    if !matches!(normalized.as_str(), "vault" | "ai-vault" | "aivault") {
        return None;
    }

    results.iter().find_map(|result| match result {
        SearchResult::BuiltIn(builtin)
            if builtin.entry.feature == crate::builtins::BuiltInFeature::AiVault =>
        {
            result.history_result_key()
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::app_launcher::AppInfo;
    use crate::builtins::{BuiltInEntry, BuiltInFeature, BuiltInGroup};
    use crate::frecency::FrecencyStore;
    use crate::list_item::GroupedListItem;
    use crate::scripts::search::score_from_tier;
    use crate::scripts::{AppMatch, BuiltInMatch, SearchResult};

    use super::build_search_mode_results;

    fn builtin(name: &str, group: BuiltInGroup, score: i32) -> SearchResult {
        SearchResult::BuiltIn(BuiltInMatch {
            entry: BuiltInEntry {
                id: name.to_lowercase().replace(' ', "-"),
                name: name.to_string(),
                description: String::new(),
                keywords: Vec::new(),
                feature: BuiltInFeature::Settings,
                icon: None,
                group,
            },
            score,
            match_evidence: None,
        })
    }

    fn app(name: &str, score: i32) -> SearchResult {
        SearchResult::App(AppMatch {
            app: AppInfo {
                name: name.to_string(),
                path: PathBuf::from(format!("/Applications/{name}.app")),
                bundle_id: None,
                icon: None,
            },
            score,
            match_evidence: None,
        })
    }

    #[test]
    fn search_mode_adds_results_header_for_plain_results() {
        let results = vec![app("Plain Result", score_from_tier(900, 0))];

        let (grouped, sorted_results) = build_search_mode_results(
            results,
            &[],
            &FrecencyStore::new(),
            "plain",
            None,
            None,
            false,
        );

        assert!(matches!(
            grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None)) if label == "Results"
        ));
        assert!(matches!(grouped.get(1), Some(GroupedListItem::Item(0))));
        assert_eq!(sorted_results[0].name(), "Plain Result");
    }

    #[test]
    fn search_mode_does_not_add_results_header_to_empty_results() {
        let (grouped, _flat) = build_search_mode_results(
            Vec::new(),
            &[],
            &FrecencyStore::new(),
            "empty",
            None,
            None,
            true,
        );

        assert!(
            grouped.iter().all(|item| !matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Results"
            )),
            "empty search results must not emit a Results header, got {grouped:?}"
        );
    }

    #[test]
    fn search_mode_breaks_equal_score_ties_by_type_order_before_name() {
        // Same tier, same boosted score: the builtin must sort above the app
        // (type order 0 vs 1) even though the app name sorts first
        // alphabetically — mirroring the raw unified-search tie-break.
        let results = vec![
            app("Alpha", score_from_tier(900, 10)),
            builtin("Zed Command", BuiltInGroup::Core, score_from_tier(900, 10)),
        ];

        let (_grouped, sorted_results) =
            build_search_mode_results(results, &[], &FrecencyStore::new(), "z", None, None, true);

        assert_eq!(sorted_results[0].name(), "Zed Command");
        assert_eq!(sorted_results[1].name(), "Alpha");
    }

    #[test]
    fn search_mode_keeps_exact_menu_bar_action_above_weaker_results() {
        let results = vec![
            app("Position Helper", score_from_tier(700, 0)),
            builtin(
                "Reset Window Positions",
                BuiltInGroup::MenuBar,
                score_from_tier(1000, 0),
            ),
        ];

        let (grouped, sorted_results) = build_search_mode_results(
            results,
            &[],
            &FrecencyStore::new(),
            "position",
            None,
            None,
            false,
        );

        let first_item = grouped
            .iter()
            .find_map(|item| match item {
                GroupedListItem::Item(idx) => sorted_results.get(*idx),
                _ => None,
            })
            .expect("at least one grouped result");

        assert_eq!(first_item.name(), "Reset Window Positions");
        assert!(matches!(
            grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None)) if label == "Menu Bar Actions"
        ));
        assert!(grouped.iter().any(
            |item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Menu Bar Actions")
        ));
        assert!(
            grouped.iter().all(|item| !matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Results"
            )),
            "Menu Bar Actions header must not be preceded by duplicate Results, got {grouped:?}"
        );
    }

    #[test]
    fn search_mode_can_suppress_terminal_fallback_section() {
        let (grouped, flat) = build_search_mode_results(
            Vec::new(),
            &[],
            &FrecencyStore::new(),
            "deploy",
            None,
            None,
            true,
        );

        assert!(
            grouped.iter().all(|item| {
                !matches!(
                    item,
                    GroupedListItem::SectionHeader(label, None)
                        if label.starts_with("Use \"deploy\" with")
                )
            }),
            "advanced filters own their empty result state and must not append terminal fallback headers"
        );
        assert!(
            flat.iter()
                .all(|result| !matches!(result, SearchResult::Fallback(_))),
            "advanced filters must not append fallback rows"
        );
    }
}
