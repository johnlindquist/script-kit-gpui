use crate::list_item::GroupedListItem;
use crate::main_window_preflight::types::{
    MainWindowPreflightAction, MainWindowPreflightActionKind, MainWindowPreflightReceipt,
    MainWindowPreflightResultRole, MainWindowPreflightVisibleResult, RootPassiveFrameReceipt,
    RootPassiveSourceReceipt,
};
use crate::scripts::SearchResult;
use crate::AppView;
use std::hash::{Hash, Hasher};

fn selected_result(app: &crate::ScriptListApp) -> Option<crate::scripts::SearchResult> {
    use crate::scripts::*;

    match &app.current_view {
        AppView::ScriptList if app.menu_syntax_capture_form_owns_input() => None,
        AppView::ScriptList => app
            .main_menu_result_caches
            .cloned_first_search_result_at_or_after_grouped_item(app.selected_index),
        AppView::BrowserHistoryView {
            filter,
            selected_index,
        } => {
            let filtered_entries = crate::browser_history::fuzzy_search_browser_history(
                &app.cached_browser_history,
                filter,
            );
            filtered_entries.get(*selected_index).map(|hit| {
                SearchResult::BrowserHistory(BrowserHistoryMatch {
                    hit: hit.entry.convert_to_root_hit(),
                    subtitle: crate::browser_history::format_browser_history_meta(&hit.entry),
                    score: hit.score,
                })
            })
        }
        AppView::AppLauncherView {
            filter,
            selected_index,
        } => {
            let filtered_apps =
                crate::ScriptListApp::app_launcher_filtered_entries(&app.apps, filter);
            filtered_apps.get(*selected_index).map(|(_, info)| {
                SearchResult::App(AppMatch {
                    app: (*info).clone(),
                    score: 0,
                    match_evidence: None,
                })
            })
        }
        AppView::WindowSwitcherView {
            filter,
            selected_index,
        } => {
            let filtered_windows: Vec<_> = if filter.is_empty() {
                app.cached_windows.iter().enumerate().collect()
            } else {
                let filter_lower = filter.to_lowercase();
                app.cached_windows
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| {
                        w.title.to_lowercase().contains(&filter_lower)
                            || w.app.to_lowercase().contains(&filter_lower)
                    })
                    .collect()
            };
            filtered_windows.get(*selected_index).map(|(_, info)| {
                SearchResult::Window(WindowMatch {
                    window: (*info).clone(),
                    app_icon: None,
                    subtitle: info.descriptor.clone(),
                    score: 0,
                    match_evidence: None,
                })
            })
        }
        _ => None,
    }
}

fn visible_result_keys(app: &crate::ScriptListApp) -> Vec<String> {
    match &app.current_view {
        AppView::ScriptList if app.menu_syntax_capture_form_owns_input() => Vec::new(),
        AppView::ScriptList => app
            .main_menu_result_caches
            .grouped_selectable_search_results()
            .filter_map(|result| result.stable_selection_key())
            .collect(),
        AppView::BrowserHistoryView { filter, .. } => {
            crate::browser_history::fuzzy_search_browser_history(
                &app.cached_browser_history,
                filter,
            )
            .iter()
            .map(|hit| hit.entry.history_key())
            .collect()
        }
        AppView::AppLauncherView { filter, .. } => {
            crate::ScriptListApp::app_launcher_filtered_entries(&app.apps, filter)
                .into_iter()
                .map(|(_, app)| app.path.to_string_lossy().to_string())
                .collect()
        }
        AppView::WindowSwitcherView { filter, .. } => {
            let filter_lower = filter.to_lowercase();
            app.cached_windows
                .iter()
                .filter(|w| {
                    filter.is_empty()
                        || w.title.to_lowercase().contains(&filter_lower)
                        || w.app.to_lowercase().contains(&filter_lower)
                })
                .map(|w| w.id.to_string())
                .collect()
        }
        _ => Vec::new(),
    }
}

fn visible_row_fingerprint(app: &crate::ScriptListApp) -> String {
    if matches!(app.current_view, AppView::ScriptList) && app.menu_syntax_capture_form_owns_input()
    {
        return "handler-form".to_string();
    }

    if !matches!(app.current_view, AppView::ScriptList) {
        return format!(
            "v:{}:{}",
            app.current_view.app_view_variant(),
            visible_result_keys(app).len()
        );
    }

    app.main_menu_result_caches
        .grouped_items()
        .iter()
        .enumerate()
        .map(|(grouped_index, item)| match item {
            GroupedListItem::SectionHeader(label, icon) => {
                format!("h:{grouped_index}:{label}:{icon:?}")
            }
            GroupedListItem::ReservedSectionSlot => {
                format!("r:{grouped_index}:section-slot")
            }
            GroupedListItem::Status(status) => {
                format!(
                    "s:{grouped_index}:{}:{}",
                    status.source.receipt_label(),
                    status.label
                )
            }
            GroupedListItem::Item(flat_index) => app
                .main_menu_result_caches
                .search_result_for_flat_index(*flat_index)
                .map(|result| {
                    format!(
                        "i:{grouped_index}:{flat_index}:{}:{:?}:{:?}:{}:{}",
                        result.stable_selection_key().unwrap_or_default(),
                        result_role(result),
                        enter_action_kind(result),
                        result.type_label(),
                        result.source_name().unwrap_or("")
                    )
                })
                .unwrap_or_else(|| format!("i:{grouped_index}:{flat_index}:missing")),
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn result_role(result: &crate::scripts::SearchResult) -> MainWindowPreflightResultRole {
    match result {
        crate::scripts::SearchResult::Flow(_)
        | crate::scripts::SearchResult::Script(_)
        | crate::scripts::SearchResult::Scriptlet(_)
        | crate::scripts::SearchResult::Skill(_)
        | crate::scripts::SearchResult::BuiltIn(_)
        | crate::scripts::SearchResult::App(_)
        | crate::scripts::SearchResult::Window(_) => MainWindowPreflightResultRole::Primary,
        crate::scripts::SearchResult::File(_) => MainWindowPreflightResultRole::RootFile,
        crate::scripts::SearchResult::Note(_)
        | crate::scripts::SearchResult::BrainHit(_)
        | crate::scripts::SearchResult::BrainInboxItem(_)
        | crate::scripts::SearchResult::Todo(_)
        | crate::scripts::SearchResult::AgentChatHistory(_)
        | crate::scripts::SearchResult::AiVault(_)
        | crate::scripts::SearchResult::ClipboardHistory(_)
        | crate::scripts::SearchResult::DictationHistory(_)
        | crate::scripts::SearchResult::BrowserTab(_)
        | crate::scripts::SearchResult::BrowserHistory(_) => {
            MainWindowPreflightResultRole::RootPassive
        }
        crate::scripts::SearchResult::Fallback(_) => MainWindowPreflightResultRole::Fallback,
        crate::scripts::SearchResult::ScriptIssue(_) => MainWindowPreflightResultRole::ScriptIssue,
        crate::scripts::SearchResult::Agent(_) => MainWindowPreflightResultRole::Agent,
        crate::scripts::SearchResult::SpineProjection(_) => {
            MainWindowPreflightResultRole::RootPassive
        }
    }
}

fn enter_action_kind(result: &crate::scripts::SearchResult) -> MainWindowPreflightActionKind {
    match result {
        // Flows converse via Threadline — the closest preflight kind is a command run.
        crate::scripts::SearchResult::Flow(_) => MainWindowPreflightActionKind::RunCommand,
        crate::scripts::SearchResult::Script(_) => MainWindowPreflightActionKind::RunScript,
        crate::scripts::SearchResult::Scriptlet(_) => MainWindowPreflightActionKind::RunSnippet,
        crate::scripts::SearchResult::BuiltIn(_) => MainWindowPreflightActionKind::RunCommand,
        crate::scripts::SearchResult::App(_) => MainWindowPreflightActionKind::LaunchApp,
        crate::scripts::SearchResult::Window(_) => MainWindowPreflightActionKind::SwitchWindow,
        crate::scripts::SearchResult::File(_) => MainWindowPreflightActionKind::OpenFile,
        crate::scripts::SearchResult::Note(_) => MainWindowPreflightActionKind::RunCommand,
        crate::scripts::SearchResult::BrainHit(_) => MainWindowPreflightActionKind::RunCommand,
        crate::scripts::SearchResult::BrainInboxItem(_) => {
            MainWindowPreflightActionKind::RunCommand
        }
        crate::scripts::SearchResult::Todo(_) => MainWindowPreflightActionKind::RunCommand,
        crate::scripts::SearchResult::AgentChatHistory(_) => {
            MainWindowPreflightActionKind::RunCommand
        }
        crate::scripts::SearchResult::AiVault(_) => {
            MainWindowPreflightActionKind::PasteResumeCommand
        }
        crate::scripts::SearchResult::ClipboardHistory(_) => {
            MainWindowPreflightActionKind::RunCommand
        }
        crate::scripts::SearchResult::DictationHistory(_) => {
            MainWindowPreflightActionKind::RunCommand
        }
        crate::scripts::SearchResult::BrowserTab(_) => MainWindowPreflightActionKind::SwitchWindow,
        crate::scripts::SearchResult::BrowserHistory(_) => {
            MainWindowPreflightActionKind::RunCommand
        }
        crate::scripts::SearchResult::Agent(_) => MainWindowPreflightActionKind::RunAgent,
        crate::scripts::SearchResult::Skill(_) => MainWindowPreflightActionKind::OpenSkill,
        crate::scripts::SearchResult::Fallback(_) => MainWindowPreflightActionKind::RunFallback,
        crate::scripts::SearchResult::ScriptIssue(_) => {
            MainWindowPreflightActionKind::InspectIssues
        }
        crate::scripts::SearchResult::SpineProjection(_) => {
            MainWindowPreflightActionKind::RunCommand
        }
    }
}

fn visible_result_receipts(app: &crate::ScriptListApp) -> Vec<MainWindowPreflightVisibleResult> {
    if matches!(app.current_view, AppView::ScriptList) && app.menu_syntax_capture_form_owns_input()
    {
        return Vec::new();
    }

    if !matches!(app.current_view, AppView::ScriptList) {
        // For built-in views, we just return the selected result as the only visible result for now
        // to satisfy the basic preflight requirements.
        if let Some(result) = selected_result(app) {
            return vec![MainWindowPreflightVisibleResult {
                visible_rank: 0,
                grouped_index: 0,
                stable_key: result.stable_selection_key(),
                role: result_role(&result),
                action_kind: enter_action_kind(&result),
                type_label: result.type_label().to_string(),
                source_name: result.source_name().map(ToString::to_string),
                description: result.description().map(ToString::to_string),
                leading_icon_present: leading_icon_present(&result),
                leading_icon_kind: leading_icon_kind(&result),
                leading_icon_bundle_id: leading_icon_bundle_id(&result),
            }];
        }
        return Vec::new();
    }

    app.main_menu_result_caches
        .grouped_items()
        .iter()
        .enumerate()
        .filter_map(|(grouped_index, item)| {
            let GroupedListItem::Item(flat_index) = item else {
                return None;
            };
            let result = app
                .main_menu_result_caches
                .search_result_for_flat_index(*flat_index)?;
            if matches!(
                result,
                SearchResult::SpineProjection(row) if !row.is_selectable
            ) {
                return None;
            }
            Some(MainWindowPreflightVisibleResult {
                visible_rank: 0,
                grouped_index,
                stable_key: result.stable_selection_key(),
                role: result_role(result),
                action_kind: enter_action_kind(result),
                type_label: result.type_label().to_string(),
                source_name: result.source_name().map(ToString::to_string),
                description: result.description().map(ToString::to_string),
                leading_icon_present: leading_icon_present(result),
                leading_icon_kind: leading_icon_kind(result),
                leading_icon_bundle_id: leading_icon_bundle_id(result),
            })
        })
        .enumerate()
        .map(|(visible_rank, mut receipt)| {
            receipt.visible_rank = visible_rank;
            receipt
        })
        .collect()
}

fn leading_icon_present(result: &SearchResult) -> bool {
    match result {
        SearchResult::App(app) => app.app.icon.is_some(),
        SearchResult::Window(window) => window.app_icon.is_some(),
        _ => false,
    }
}

fn leading_icon_kind(result: &SearchResult) -> Option<String> {
    match result {
        SearchResult::App(app) if app.app.icon.is_some() => Some("appImage".to_string()),
        SearchResult::Window(window) if window.app_icon.is_some() => Some("appImage".to_string()),
        SearchResult::Window(_) => Some("fallback".to_string()),
        _ => None,
    }
}

fn leading_icon_bundle_id(result: &SearchResult) -> Option<String> {
    match result {
        SearchResult::App(app) => app.app.bundle_id.clone(),
        SearchResult::Window(window) => window.window.bundle_id.clone(),
        _ => None,
    }
}

fn build_tab_action(app: &crate::ScriptListApp) -> Option<MainWindowPreflightAction> {
    if app.filter_text.trim().is_empty() {
        return None;
    }

    if app.menu_syntax_capture_form_owns_input() {
        return None;
    }

    Some(MainWindowPreflightAction {
        kind: MainWindowPreflightActionKind::AskAi,
        label: "Ask AI".to_string(),
        subject: app.filter_text.clone(),
        type_label: "AI".to_string(),
        source_name: None,
        description: Some(
            "Opens the AI window with the current query for review before submit.".to_string(),
        ),
    })
}

fn build_root_passive_frame_receipt(app: &crate::ScriptListApp) -> Option<RootPassiveFrameReceipt> {
    let frame = app.root_search.root_passive_frame()?;
    let browser_tabs_status = crate::browser_tabs::root_browser_tabs_snapshot_status();
    let browser_history_status = crate::browser_history::root_browser_history_snapshot_status();

    Some(RootPassiveFrameReceipt {
        query: frame.key.query.clone(),
        source_filters: frame.key.source_filters.labels(),
        notes: RootPassiveSourceReceipt {
            enabled: frame.key.notes_options.enabled,
            frame_count: frame.note_hits.len(),
            cache_generation: 0,
            frame_generation: 0,
            refreshing: false,
        },
        clipboard_history: RootPassiveSourceReceipt {
            enabled: frame.key.clipboard_history_options.enabled,
            frame_count: frame.clipboard_history_hits.len(),
            cache_generation: 0,
            frame_generation: 0,
            refreshing: false,
        },
        dictation_history: RootPassiveSourceReceipt {
            enabled: frame.key.dictation_history_options.enabled,
            frame_count: frame.dictation_history_hits.len(),
            cache_generation: 0,
            frame_generation: 0,
            refreshing: false,
        },
        agent_chat_history: RootPassiveSourceReceipt {
            enabled: frame.key.agent_chat_history_options.enabled,
            frame_count: frame.agent_chat_history_hits.len(),
            cache_generation: 0,
            frame_generation: 0,
            refreshing: false,
        },
        ai_vault: RootPassiveSourceReceipt {
            enabled: frame.key.ai_vault_options.enabled,
            frame_count: frame.ai_vault_hits.len(),
            cache_generation: 0,
            frame_generation: 0,
            refreshing: false,
        },
        browser_tabs: RootPassiveSourceReceipt {
            enabled: frame.key.browser_tabs_options.enabled,
            frame_count: frame.browser_tab_hits.len(),
            cache_generation: browser_tabs_status.generation,
            frame_generation: frame.browser_tabs_snapshot_generation,
            refreshing: browser_tabs_status.refreshing,
        },
        browser_history: RootPassiveSourceReceipt {
            enabled: frame.key.browser_history_options.enabled,
            frame_count: frame.browser_history_hits.len(),
            cache_generation: browser_history_status.generation,
            frame_generation: frame.browser_history_snapshot_generation,
            refreshing: browser_history_status.refreshing,
        },
    })
}

fn selection_warnings(
    app: &crate::ScriptListApp,
    result: Option<&crate::scripts::SearchResult>,
) -> Vec<String> {
    let mut warnings = Vec::new();

    if let Some(result) = result {
        if matches!(result, crate::scripts::SearchResult::Agent(_)) {
            warnings.push(
                "Agent execution is not fully implemented in execute_selected yet.".to_string(),
            );
        }
    }

    if app.filter_text.trim().is_empty() {
        warnings
            .push("Command+Enter Agent Chat is inactive until the filter has text.".to_string());
    } else if app.menu_syntax_capture_form_owns_input() {
        warnings.push(
            "Command+Enter Agent Chat is disabled while the handler form owns input.".to_string(),
        );
    }

    warnings
}

fn build_enter_action(
    app: &crate::ScriptListApp,
    result: Option<&crate::scripts::SearchResult>,
) -> Option<MainWindowPreflightAction> {
    result.map(|result| MainWindowPreflightAction {
        kind: enter_action_kind(result),
        label: app.main_window_primary_action_label(),
        subject: result.name().to_string(),
        type_label: result.type_label().to_string(),
        source_name: result.source_name().map(ToString::to_string),
        description: result.description().map(ToString::to_string),
    })
}

/// Refresh only the selection-dependent fields of a cached receipt.
///
/// Arrow-key navigation changes `selected_index` many times per second while
/// the visible rows, fingerprints, counts, and filter-derived fields stay
/// identical. Rebuilding the full receipt is O(visible rows) of string
/// allocation per keypress; this selection-only refresh is O(1).
pub(crate) fn refresh_main_window_preflight_selection(
    app: &crate::ScriptListApp,
    receipt: &mut MainWindowPreflightReceipt,
) {
    let result = selected_result(app);
    receipt.selected_index = app.selected_index;
    receipt.selected_result_key = result.as_ref().and_then(|r| r.stable_selection_key());
    receipt.selected_result_role = result.as_ref().map(result_role);
    receipt.enter_action = build_enter_action(app, result.as_ref());
    receipt.warnings = selection_warnings(app, result.as_ref());
}

pub(crate) fn build_main_window_preflight_receipt(
    app: &crate::ScriptListApp,
) -> Option<MainWindowPreflightReceipt> {
    if !matches!(
        app.current_view,
        AppView::ScriptList
            | AppView::AppLauncherView { .. }
            | AppView::WindowSwitcherView { .. }
            | AppView::BrowserTabsView { .. }
            | AppView::ClipboardHistoryView { .. }
            | AppView::BrowserHistoryView { .. }
            | AppView::EmojiPickerView { .. }
            | AppView::ProcessManagerView { .. }
            | AppView::FlowUxView { .. }
            | AppView::CurrentAppCommandsView { .. }
            | AppView::SettingsView { .. }
            | AppView::FavoritesBrowseView { .. }
            | AppView::AgentChatHistoryView { .. }
            | AppView::DictationHistoryView { .. }
            | AppView::NotesBrowseView { .. }
    ) {
        return None;
    }

    let result = selected_result(app);
    let warnings = selection_warnings(app, result.as_ref());
    let enter_action = build_enter_action(app, result.as_ref());
    let visible_results = visible_result_receipts(app);
    let selected_result_role = result.as_ref().map(result_role);
    let computed_search_text =
        crate::menu_syntax::free_text_for_search(&app.menu_syntax_mode, &app.filter_text)
            .to_string();
    let source_filters = app
        .menu_syntax_mode
        .advanced_query_for(&app.filter_text)
        .map(|query| query.source_filters.labels())
        .unwrap_or_default();
    let filter_indicators = app
        .menu_syntax_mode
        .advanced_query_for(&app.filter_text)
        .map(|query| query.filter_indicators())
        .unwrap_or_default();

    Some(MainWindowPreflightReceipt {
        filter_text: app.filter_text.clone(),
        computed_search_text,
        source_filters,
        filter_indicators,
        selected_index: app.selected_index,
        selected_result_key: result.as_ref().and_then(|r| r.stable_selection_key()),
        selected_result_role,
        visible_results,
        visible_result_key_fingerprint: visible_result_keys(app).join("|"),
        visible_row_fingerprint: visible_row_fingerprint(app),
        visible_result_count: app
            .main_menu_result_caches
            .grouped_selectable_result_count(),
        root_passive_frame: build_root_passive_frame_receipt(app),
        enter_action,
        tab_action: build_tab_action(app),
        warnings,
    })
}

pub(crate) fn log_main_window_preflight_receipt(receipt: &MainWindowPreflightReceipt) {
    let key_fingerprint_log = if crate::logging::preflight_deep_log_enabled() {
        receipt.visible_result_key_fingerprint.clone()
    } else {
        compact_log_fingerprint(&receipt.visible_result_key_fingerprint)
    };
    let row_fingerprint_log = if crate::logging::preflight_deep_log_enabled() {
        receipt.visible_row_fingerprint.clone()
    } else {
        compact_log_fingerprint(&receipt.visible_row_fingerprint)
    };
    let enter_subject = receipt
        .enter_action
        .as_ref()
        .map(|a| crate::logging::log_user_value_with_limit(&a.subject, 160).to_string())
        .unwrap_or_else(|| "none".to_string());
    tracing::info!(
        event = "main_window_preflight_receipt",
        selected_index = receipt.selected_index,
        selected_result_key = ?receipt.selected_result_key,
        selected_result_role = ?receipt.selected_result_role,
        visible_result_key_fingerprint = %key_fingerprint_log,
        visible_row_fingerprint = %row_fingerprint_log,
        visible_result_count = receipt.visible_result_count,
        enter_label = %receipt.enter_action.as_ref().map(|a| a.label.as_str()).unwrap_or("none"),
        enter_subject = %enter_subject,
        enter_type = %receipt.enter_action.as_ref().map(|a| a.type_label.as_str()).unwrap_or("none"),
        tab_enabled = receipt.tab_action.is_some(),
        warnings = ?receipt.warnings,
        "Built main window preflight receipt"
    );
}

fn compact_log_fingerprint(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("len:{}:hash:{:016x}", value.len(), hasher.finish())
}
