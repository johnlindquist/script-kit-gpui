#[test]
fn source_filters_are_frame_keyed_and_gate_async_sources() {
    let app_state = include_str!("../../src/main_sections/app_state.rs");
    let filtering = include_str!("../../src/app_impl/filtering_cache.rs");
    let root_file = include_str!("../../src/app_impl/root_file_search.rs");

    assert!(app_state
        .contains("pub(crate) source_filters: crate::menu_syntax::RootUnifiedSourceFilterSet"));
    assert!(filtering.contains("source_filters: source_filters.clone()"));
    assert!(filtering
        .contains("source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Notes)"));
    assert!(filtering
        .contains("source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Todo)"));
    assert!(filtering.contains(
        "source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory)"
    ));
    assert!(filtering.contains(
        "source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Conversations)"
    ));
    assert!(filtering.contains(
        "source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs)"
    ));
    assert!(filtering.contains(".allows(crate::menu_syntax::RootUnifiedSourceFilter::Files))"));
    assert!(filtering.contains("clipboard_history_options.enabled = true;"));
    assert!(filtering.contains("todo_options.enabled = true;"));
    assert!(filtering.contains("notes_options.enabled = true;"));
    assert!(filtering.contains("dictation_history_options.enabled = true;"));
    assert!(filtering.contains("browser_tabs_options.enabled = true;"));
    assert!(filtering.contains("browser_history_options.enabled = true;"));
    assert!(filtering.contains("agent_chat_history_options.enabled = true;"));
    assert!(filtering.contains("maybe_start_root_notes_refresh_for_query("));
    assert!(filtering.contains("maybe_start_root_todos_refresh_for_query("));
    assert!(filtering.contains("crate::notes::search_root_notes_meta_cached("));
    assert!(filtering.contains("crate::menu_syntax::search_root_todos_cached("));
    assert!(
        filtering.contains("crate::clipboard_history::search_root_clipboard_history_meta_direct(")
    );
    assert!(filtering.contains("crate::dictation::search_root_dictation_history_direct("));
    assert!(filtering.contains("crate::ai::agent_chat::ui::history::search_history_direct("));
    assert!(filtering.contains("search_root_browser_tabs_meta_direct"));
    assert!(filtering.contains("search_root_browser_history_meta_direct"));
    assert!(filtering.contains(".is_some_and(|query| query.has_source_filters())"));

    assert!(root_file.contains("free_text_for_search(&self.menu_syntax_mode, query)"));
    assert!(root_file
        .contains("!source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Files)"));
    assert!(root_file.contains("root_file_options.files_enabled = true;"));
    assert!(root_file.contains("let publish_active_results = source_filters"));
    assert!(root_file.contains(".includes(crate::menu_syntax::RootUnifiedSourceFilter::Files)"));
    assert!(root_file.contains("|| matches!(&request, RootFileSearchRequest::DirectoryBrowse"));
    assert!(root_file.contains("self.root_search.root_file_frame = None;"));
    assert!(root_file.contains(".is_none_or(|advanced_query| !advanced_query.has_predicates())"));
    assert!(root_file.contains("self.cached_root_file_results_for_request(&request)"));
    assert!(root_file.contains("root_file_result_fingerprint(&self.root_search.root_file_results)"));
}

#[test]
fn explicit_source_filters_raise_passive_source_caps() {
    let filtering = include_str!("../../src/app_impl/filtering_cache.rs");

    assert!(filtering.contains(
        "let explicit_source_result_target = root_passive_result_limits.max_total_results"
    ));
    for source in [
        "notes_options.max_results",
        "todo_options.max_results",
        "clipboard_history_options.max_results",
        "dictation_history_options.max_results",
        "agent_chat_history_options.max_results",
        "browser_tabs_options.max_results",
        "browser_history_options.max_results",
    ] {
        assert!(
            filtering.contains(source) && filtering.contains(".max(explicit_source_result_target)"),
            "explicit source filters must raise {source} above passive preview defaults"
        );
    }
}

#[test]
fn implicit_browser_tabs_warmup_respects_source_filters() {
    let filtering = include_str!("../../src/app_impl/filtering_cache.rs");
    let helper = filtering
        .split("fn root_browser_tabs_refresh_options_for_query(")
        .nth(1)
        .and_then(|rest| {
            rest.split("fn current_query_can_show_root_browser_tabs(")
                .next()
        })
        .expect("browser tabs refresh eligibility helper should exist");

    assert!(
        helper.contains("source_filters.includes(source)")
            && helper.contains("source_filters.allows(source)"),
        "explicit tabs: should force-enable browser tabs only when tabs are allowed"
    );
    assert!(
        helper.contains("if !source_filters.allows(source)") && helper.contains("return None;"),
        "ordinary implicit warmup should not run when tabs are filtered out"
    );
    assert!(
        helper.contains("query.has_predicates()"),
        "implicit warmup should not run for predicate-owned advanced queries"
    );
}

#[test]
fn grouping_suppresses_primary_and_disallowed_sources_when_filter_active() {
    let grouping = include_str!("../../src/scripts/grouping.rs");
    let app_state = include_str!("../../src/main_sections/app_state.rs");
    let collect_elements = include_str!("../../src/app_layout/collect_elements.rs");

    assert!(
        grouping.contains("root_source_filters: &crate::menu_syntax::RootUnifiedSourceFilterSet")
    );
    assert!(grouping.contains("filter_grouped_results_by_root_sources"));
    assert!(grouping.contains("root_unified_source()"));
    assert!(grouping.contains(
        "root_source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Files)"
    ));
    assert!(grouping.contains("root_source_filters.active(),"));
    assert!(grouping.contains("let handoff = if suppress_handoff"));
    assert!(grouping.contains(
        "root_source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Notes)"
    ));
    assert!(grouping
        .contains("root_source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Todo)"));
    assert!(grouping.contains("root_source_filters\n                    .allows(crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory)"));
    assert!(grouping.contains(
        "root_source_filters\n                    .allows(crate::menu_syntax::RootUnifiedSourceFilter::Conversations)"
    ));
    assert!(grouping.contains(
        "root_source_filters\n                    .allows(crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory)"
    ));
    assert!(grouping.contains(
        "root_source_filters\n                    .allows(crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs)"
    ));
    assert!(grouping.contains("append_base_source_status_rows"));
    assert!(grouping.contains("root_source_filters.positive_includes()"));
    assert!(grouping.contains("GroupedListItem::Status(source_chip_result_status("));
    assert!(app_state.contains("cached_grouped_source_statuses"));
    assert!(app_state.contains("GroupedListItem::Status(status) => source_statuses.push(status)"));
    assert!(collect_elements.contains("cached_source_statuses_snapshot()"));
    assert!(collect_elements.contains("index: None"));
    assert!(grouping.contains("if limit == 0 && !explicit_source_filter"));
    assert!(grouping.contains(
        "root_source_filters\n                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs)"
    ));
    assert!(grouping.contains(
        "root_source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Windows)"
    ));
}

#[test]
fn preflight_receipt_exposes_source_filter_state_for_agentic_proof() {
    let types = include_str!("../../src/main_window_preflight/types.rs");
    let build = include_str!("../../src/main_window_preflight/build.rs");

    assert!(types.contains("pub computed_search_text: String"));
    assert!(types.contains("pub source_filters: Vec<String>"));
    assert!(types.contains("pub filter_indicators: Vec<crate::menu_syntax::FilterIndicator>"));
    assert!(build.contains("free_text_for_search(&app.menu_syntax_mode, &app.filter_text)"));
    assert!(build.contains("query.source_filters.labels()"));
    assert!(build.contains("query.filter_indicators()"));
    assert!(build.contains("source_filters: frame"));
}

#[test]
fn colon_opens_discoverability_picker_while_source_filters_do_not_open_hint() {
    let trigger = include_str!("../../src/menu_syntax/trigger_picker.rs");
    let popup = include_str!("../../src/app_impl/menu_syntax_trigger_picker.rs");
    let hint = include_str!("../../src/menu_syntax/main_hint.rs");
    let render = include_str!("../../src/render_script_list/mod.rs");
    let prompt_handler = include_str!("../../src/prompt_handler/mod.rs");

    assert!(trigger.contains("SOURCE_HEAD_SPECS"));
    assert!(popup.contains("fn source_filter_query_does_not_open_power_popup()"));
    assert!(hint.contains("if query.is_source_filter_only()"));
    assert!(hint.contains("return None;"));
    assert!(hint.contains("pub fn active_head_is_source_filter(raw: &str) -> bool"));
    let source_filter_only_guard = hint
        .find("if query.is_source_filter_only()")
        .map(|index| &hint[index..hint.len().min(index + 120)])
        .unwrap_or("");
    assert!(
        source_filter_only_guard.contains("return None;"),
        "build_menu_syntax_main_hint must suppress source-filter-only main hints"
    );
    let source_head_has_results_gate = prompt_handler
        .find("let source_head_has_results")
        .map(|index| &prompt_handler[index..prompt_handler.len().min(index + 260)])
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(source_head_has_results_gate
        .contains("crate::menu_syntax::main_hint::active_head_is_source_filter"));
    assert!(source_head_has_results_gate.contains("&self.filter_text"));
    assert!(source_head_has_results_gate.contains("&& visible_choice_count > 0"));
    let advanced_query_results_empty_gate = prompt_handler
        .find("let advanced_query_results_empty")
        .map(|index| &prompt_handler[index..prompt_handler.len().min(index + 320)])
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        advanced_query_results_empty_gate.contains(
            "detector_owns_head && !source_head_has_results && !advanced_query_has_results"
        ),
        "getState empty-hint gate must be detector-owned, suppressed when source heads already have results, and suppressed when advanced-query results exist"
    );
    assert!(render.contains("There are no search results with this filter applied."));
    assert!(render.contains("query.has_source_filters() || query.has_predicates()"));
}
