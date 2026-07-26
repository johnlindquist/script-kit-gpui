use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use tracing::debug;

use crate::config::SuggestedConfig;
use crate::frecency::FrecencyStore;
use crate::list_item::GroupedListItem;

use super::super::types::SearchResult;
use super::DEFAULT_SUGGESTED_ITEMS;

/// A plugin-based section in the grouped launcher view.
/// `key` is the stable identity (plugin_id or "main"); `label` is the display title.
#[derive(Debug, Clone)]
struct PluginGroupSection {
    label: String,
    indices: Vec<usize>,
}

/// Intra-section type ordering: skills first, then scripts, then scriptlets.
fn plugin_section_type_order(result: &SearchResult) -> u8 {
    match result {
        SearchResult::Skill(_) => 0,
        SearchResult::Script(_) => 1,
        SearchResult::Scriptlet(_) => 2,
        _ => 3,
    }
}

fn fallback_plugin_label(key: &str) -> String {
    if key == "main" {
        "Main".to_string()
    } else {
        key.to_string()
    }
}

/// Returns `(grouping_key, display_label)` for plugin-owned result types.
/// Uses plugin_title for display when available; falls back to the grouping key.
fn plugin_group_identity(result: &SearchResult) -> Option<(String, String)> {
    match result {
        SearchResult::Script(sm) => {
            let key = if sm.script.plugin_id.is_empty() {
                sm.script
                    .kit_name
                    .clone()
                    .unwrap_or_else(|| "main".to_string())
            } else {
                sm.script.plugin_id.clone()
            };
            let label = sm
                .script
                .plugin_title
                .clone()
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| fallback_plugin_label(&key));
            Some((key, label))
        }
        SearchResult::Scriptlet(sm) => {
            let key = if sm.scriptlet.plugin_id.is_empty() {
                sm.scriptlet
                    .group
                    .clone()
                    .unwrap_or_else(|| "main".to_string())
            } else {
                sm.scriptlet.plugin_id.clone()
            };
            let label = sm
                .scriptlet
                .plugin_title
                .clone()
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| fallback_plugin_label(&key));
            Some((key, label))
        }
        SearchResult::Skill(sm) => {
            let key = sm.skill.plugin_id.clone();
            let label = if sm.skill.plugin_title.is_empty() {
                fallback_plugin_label(&key)
            } else {
                sm.skill.plugin_title.clone()
            };
            Some((key, label))
        }
        _ => None,
    }
}

fn default_suggested_lookup_name(result: &SearchResult) -> &str {
    match result {
        SearchResult::BuiltIn(bm) if bm.entry.id == "builtin/do-in-current-app" => {
            crate::menu_bar::current_app_commands::DO_IN_CURRENT_APP_LABEL
        }
        _ => result.name(),
    }
}

/// Degraded flow-discovery signal for the empty-query Flows section: when
/// no flow rows exist, the section still tells the user WHY (loading,
/// mdflow missing/broken, pre-protocol mdflow) instead of silently
/// vanishing. `None`/`Ready` with zero flows hides the section — a user
/// with genuinely no flows shouldn't see an empty header.
#[derive(Debug, Clone)]
pub struct FlowDiscoveryNote {
    pub status: crate::flows::catalog::RosterStatus,
    pub detail: Option<String>,
}

pub(super) fn build_grouped_view_results(
    results: Vec<SearchResult>,
    frecency_store: &FrecencyStore,
    suggested_config: &SuggestedConfig,
    flow_discovery: Option<&FlowDiscoveryNote>,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    // Grouped view mode: create SUGGESTED and plugin-based sections
    let mut grouped = Vec::new();

    // Get suggested items from frecency store (respecting config)
    let suggested_items = if suggested_config.enabled {
        frecency_store.get_recent_items(suggested_config.max_items)
    } else {
        Vec::new()
    };

    // Build a set of paths that are "suggested" (have frecency score above min_score)
    let min_score = suggested_config.min_score;
    let suggested_paths: HashSet<String> = suggested_items
        .iter()
        .filter(|(_, score): &&(String, f64)| *score >= min_score)
        .map(|(path, _): &(String, f64)| path.clone())
        .collect();

    // Map each result to its frecency key.
    // Skills and scriptlets use plugin-qualified keys.
    // Agents are suppressed — they return None so they are skipped from frecency and grouping.
    let get_result_path = |result: &SearchResult| -> Option<String> {
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
            // Brain rows never join frecency/grouping; passive search must not
            // feed usage memory (it would self-amplify).
            SearchResult::BrainHit(_) => None,
            // Brain inbox rows are pinned synthetically, never grouped.
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
            // Suppressed: agents are not top-level launcher artifacts
            SearchResult::Agent(_) => None,
            // Fallbacks don't have paths - they're only shown in search mode, not grouped view
            SearchResult::Fallback(_) => None,
            // Script issues row is pinned synthetically, not through frecency/grouping
            SearchResult::ScriptIssue(_) => None,
            // Spine projections don't participate in frecency/grouping
            SearchResult::SpineProjection(_) => None,
        }
    };

    // Find indices of results that are "suggested" and categorize non-suggested by plugin or type
    let mut suggested_indices: Vec<(usize, f64)> = Vec::new();
    // Plugin-based grouping: HashMap<plugin_key, PluginGroupSection>
    let mut plugin_groups: HashMap<String, PluginGroupSection> = HashMap::new();
    let mut commands_indices: Vec<usize> = Vec::new();
    let mut apps_indices: Vec<usize> = Vec::new();
    // Flows are the primary launcher experience — they get the leading section.
    let mut flows_indices: Vec<usize> = Vec::new();

    // Get excluded commands for filtering builtins from SUGGESTED section
    let excluded_commands = &suggested_config.excluded_commands;

    for (idx, result) in results.iter().enumerate() {
        if let Some(path) = get_result_path(result) {
            let score = frecency_store.get_score(&path);

            // Check if this builtin should be excluded from SUGGESTED
            // (e.g., "Quit Script Kit" shouldn't appear in suggested even if it has frecency)
            let is_excluded_builtin = match result {
                SearchResult::BuiltIn(bm) => {
                    bm.entry.should_exclude_from_frecency(excluded_commands)
                }
                _ => false,
            };

            if score >= min_score && suggested_paths.contains(&path) && !is_excluded_builtin {
                suggested_indices.push((idx, score));
            } else {
                // Categorize by plugin (for scripts/scriptlets/skills) or by type (for others)
                match result {
                    SearchResult::Script(_)
                    | SearchResult::Scriptlet(_)
                    | SearchResult::Skill(_) => {
                        let (key, label) = plugin_group_identity(result)
                            .unwrap_or_else(|| ("main".to_string(), "Main".to_string()));
                        plugin_groups
                            .entry(key)
                            .or_insert_with(|| PluginGroupSection {
                                label,
                                indices: Vec::new(),
                            })
                            .indices
                            .push(idx);
                    }
                    SearchResult::Flow(_) => flows_indices.push(idx),
                    SearchResult::BuiltIn(_) | SearchResult::Window(_) => {
                        commands_indices.push(idx)
                    }
                    SearchResult::File(_) => {}
                    SearchResult::Note(_) => {}
                    SearchResult::BrainHit(_) => {}
                    SearchResult::BrainInboxItem(_) => {}
                    SearchResult::Todo(_) => {}
                    SearchResult::AgentChatHistory(_) => {}
                    SearchResult::AiVault(_) => {}
                    SearchResult::ClipboardHistory(_) => {}
                    SearchResult::DictationHistory(_) => {}
                    SearchResult::BrowserTab(_) => {}
                    SearchResult::BrowserHistory(_) => {}
                    SearchResult::App(_) => apps_indices.push(idx),
                    // Suppressed: agents are not top-level launcher artifacts
                    SearchResult::Agent(_) => {
                        tracing::info!(
                            event = "legacy_agent_result_suppressed",
                            agent_name = result.name(),
                            "Agent result skipped in grouped view"
                        );
                    }
                    // Fallbacks should never appear in grouped view - they're search-mode only
                    SearchResult::Fallback(_) => {}
                    // Script issues row is pinned synthetically, not by grouping logic
                    SearchResult::ScriptIssue(_) => {}
                    // Spine projections don't participate in grouping
                    SearchResult::SpineProjection(_) => {}
                }
            }
        } else {
            // If no path, categorize by type (shouldn't happen, but handle gracefully)
            match result {
                SearchResult::Script(_) | SearchResult::Scriptlet(_) | SearchResult::Skill(_) => {
                    let (key, label) = plugin_group_identity(result)
                        .unwrap_or_else(|| ("main".to_string(), "Main".to_string()));
                    plugin_groups
                        .entry(key)
                        .or_insert_with(|| PluginGroupSection {
                            label,
                            indices: Vec::new(),
                        })
                        .indices
                        .push(idx);
                }
                SearchResult::Flow(_) => flows_indices.push(idx),
                SearchResult::BuiltIn(_) | SearchResult::Window(_) => commands_indices.push(idx),
                SearchResult::File(_) => {}
                SearchResult::Note(_) => {}
                SearchResult::BrainHit(_) => {}
                SearchResult::BrainInboxItem(_) => {}
                SearchResult::Todo(_) => {}
                SearchResult::AgentChatHistory(_) => {}
                SearchResult::AiVault(_) => {}
                SearchResult::ClipboardHistory(_) => {}
                SearchResult::DictationHistory(_) => {}
                SearchResult::BrowserTab(_) => {}
                SearchResult::BrowserHistory(_) => {}
                SearchResult::App(_) => apps_indices.push(idx),
                // Suppressed: agents are not top-level launcher artifacts
                SearchResult::Agent(_) => {}
                // Fallbacks should never appear in grouped view - they're search-mode only
                SearchResult::Fallback(_) => {}
                // Script issues row is pinned synthetically, not by grouping logic
                SearchResult::ScriptIssue(_) => {}
                // Spine projections don't participate in grouping
                SearchResult::SpineProjection(_) => {}
            }
        }
    }

    // Sort suggested items by frecency score (highest first)
    suggested_indices.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

    // Limit suggested items to max_items from config
    suggested_indices.truncate(suggested_config.max_items);

    // If no frecency-based suggestions and frecency store is empty (new user),
    // populate with default suggested items to help users discover features
    let default_suggested_indices: Vec<usize> =
        if suggested_indices.is_empty() && frecency_store.is_empty() && suggested_config.enabled {
            let mut first_index_by_name: HashMap<&str, usize> =
                HashMap::with_capacity(results.len());
            for (idx, result) in results.iter().enumerate() {
                first_index_by_name
                    .entry(default_suggested_lookup_name(result))
                    .or_insert(idx);
            }

            // Find indices of default suggested items by name (preserve order from DEFAULT_SUGGESTED_ITEMS)
            DEFAULT_SUGGESTED_ITEMS
                .iter()
                .filter_map(|&default_name| first_index_by_name.get(default_name).copied())
                .collect()
        } else {
            Vec::new()
        };

    // Remove default suggested items from other sections to avoid duplication
    if !default_suggested_indices.is_empty() {
        let default_set: HashSet<usize> = default_suggested_indices.iter().copied().collect();
        commands_indices.retain(|idx| !default_set.contains(idx));
        apps_indices.retain(|idx| !default_set.contains(idx));
        flows_indices.retain(|idx| !default_set.contains(idx));
        for section in plugin_groups.values_mut() {
            section.indices.retain(|idx| !default_set.contains(idx));
        }
    }

    // Sort each section alphabetically by name (case-insensitive)
    let sort_alphabetically = |indices: &mut Vec<usize>| {
        indices.sort_by_cached_key(|&idx| results[idx].name().to_lowercase());
    };

    // Sort items within each plugin section: skills first, then scripts, then scriptlets,
    // with alphabetical sub-ordering within each type group.
    for (plugin_key, section) in plugin_groups.iter_mut() {
        section.indices.sort_by(|a, b| {
            let type_a = plugin_section_type_order(&results[*a]);
            let type_b = plugin_section_type_order(&results[*b]);
            type_a.cmp(&type_b).then_with(|| {
                results[*a]
                    .name()
                    .to_lowercase()
                    .cmp(&results[*b].name().to_lowercase())
            })
        });
        let skill_count = section
            .indices
            .iter()
            .filter(|&&idx| matches!(results[idx], SearchResult::Skill(_)))
            .count();
        tracing::info!(
            event = "main_menu_plugin_section_sorted",
            plugin_key = %plugin_key,
            label = %section.label,
            item_count = section.indices.len(),
            skill_count,
            "Sorted plugin section with skills promoted to the top"
        );
    }
    sort_alphabetically(&mut commands_indices);
    sort_alphabetically(&mut apps_indices);
    sort_alphabetically(&mut flows_indices);

    // Get non-main plugin keys sorted alphabetically by display label
    let mut other_plugin_keys: Vec<&String> = plugin_groups
        .keys()
        .filter(|k| k.as_str() != "main")
        .collect();
    other_plugin_keys.sort_by_cached_key(|k| plugin_groups[*k].label.to_lowercase());

    // Build grouped list in order: Flows, Suggested, Main, Commands, other
    // kits, Apps. Flows lead — they are the primary launcher experience
    // (mdflow pivot, 2026-07-10).
    if !flows_indices.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            "Flows".to_string(),
            Some("flow".to_string()),
        ));
        for idx in &flows_indices {
            grouped.push(GroupedListItem::Item(*idx));
        }
    } else if let Some(note) = flow_discovery {
        use crate::flows::catalog::RosterStatus;
        use crate::list_item::{SourceChipStatusKind, SourceChipStatusRow};
        let status_row = |kind: SourceChipStatusKind, label: String| {
            GroupedListItem::Status(SourceChipStatusRow {
                source: crate::menu_syntax::RootUnifiedSourceFilter::Commands,
                source_name: "Flows".to_string(),
                status_kind: kind,
                label,
                shown: 0,
                loaded: 0,
                total: None,
            })
        };
        let row = match note.status {
            // Zero flows + Ready is a real answer, not a degraded state.
            RosterStatus::Ready => None,
            RosterStatus::Loading => Some(status_row(
                SourceChipStatusKind::Loading,
                "Loading flows…".to_string(),
            )),
            RosterStatus::Error => Some(status_row(
                SourceChipStatusKind::Disabled,
                note.detail
                    .clone()
                    .unwrap_or_else(|| "Flow discovery failed — check mdflow".to_string()),
            )),
            RosterStatus::Legacy => Some(status_row(
                SourceChipStatusKind::Disabled,
                "mdflow predates the flows protocol — npm i -g mdflow@next".to_string(),
            )),
        };
        if let Some(row) = row {
            grouped.push(GroupedListItem::SectionHeader(
                "Flows".to_string(),
                Some("flow".to_string()),
            ));
            grouped.push(row);
        }
    }

    // Each section header includes an item count suffix (e.g., "Suggested · 5")
    // 1. Suggested (frecency-based, or default items for new users)
    if suggested_config.enabled {
        if !suggested_indices.is_empty() {
            // User has frecency data - show their frequently used items
            grouped.push(GroupedListItem::SectionHeader(
                "Suggested".to_string(),
                Some("StarFilled".to_string()),
            ));
            for (idx, _score) in &suggested_indices {
                grouped.push(GroupedListItem::Item(*idx));
            }
        } else if !default_suggested_indices.is_empty() {
            // New user with no frecency - show default suggestions
            grouped.push(GroupedListItem::SectionHeader(
                "Suggested".to_string(),
                Some("StarFilled".to_string()),
            ));
            for idx in &default_suggested_indices {
                grouped.push(GroupedListItem::Item(*idx));
            }
        }
    }

    // 2. Main plugin section (if it has items)
    if let Some(main_section) = plugin_groups.get("main") {
        if !main_section.indices.is_empty() {
            tracing::info!(
                plugin_key = "main",
                plugin_label = %main_section.label,
                item_count = main_section.indices.len(),
                "main_menu_plugin_section_built"
            );
            grouped.push(GroupedListItem::SectionHeader(
                main_section.label.clone(),
                Some("Code".to_string()),
            ));
            for idx in &main_section.indices {
                grouped.push(GroupedListItem::Item(*idx));
            }
        }
    }

    // 3. Commands (built-ins and window controls)
    if !commands_indices.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            "Commands".to_string(),
            Some("Terminal".to_string()),
        ));
        for idx in &commands_indices {
            grouped.push(GroupedListItem::Item(*idx));
        }
    }

    // 4. Other plugin sections - sorted alphabetically by display label
    for plugin_key in &other_plugin_keys {
        if let Some(section) = plugin_groups.get(*plugin_key) {
            if !section.indices.is_empty() {
                tracing::info!(
                    plugin_key = %plugin_key,
                    plugin_label = %section.label,
                    item_count = section.indices.len(),
                    "main_menu_plugin_section_built"
                );
                grouped.push(GroupedListItem::SectionHeader(
                    section.label.clone(),
                    Some("BoltFilled".to_string()),
                ));
                for idx in &section.indices {
                    grouped.push(GroupedListItem::Item(*idx));
                }
            }
        }
    }

    // 5. Apps (installed applications)
    if !apps_indices.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            "Apps".to_string(),
            Some("Folder".to_string()),
        ));
        for idx in &apps_indices {
            grouped.push(GroupedListItem::Item(*idx));
        }
    }

    // Note: Agents are suppressed from the grouped view; skills replace them

    // Calculate plugin section counts for logging
    let plugin_items_count: usize = plugin_groups.values().map(|s| s.indices.len()).sum();

    debug!(
        suggested_count = suggested_indices.len(),
        plugin_sections = plugin_groups.len(),
        plugin_items_count = plugin_items_count,
        commands_count = commands_indices.len(),
        apps_count = apps_indices.len(),
        total_grouped = grouped.len(),
        "Grouped view: created plugin-based sections (scripts, scriptlets, skills grouped by plugin)"
    );

    (grouped, results)
}
