//! Source audits for builtin dispatch context and structured outcome adoption.
//!
//! Ensures that `execute_builtin_with_query` uses `DispatchContext::for_builtin`,
//! that `dispatch_system_action` returns `DispatchOutcome`, and that the
//! `log_builtin_outcome` helper emits all required structured fields.

use super::read_source as read;

fn compact_source(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

fn compact_app_view_match_arm<'a>(compacted: &'a str, view: &str) -> &'a str {
    let start = compacted
        .find(view)
        .unwrap_or_else(|| panic!("calculate_window_size_params missing {view}"));
    let tail = &compacted[start..];
    let end = tail[view.len()..]
        .find("AppView::")
        .map(|offset| view.len() + offset)
        .unwrap_or(tail.len());
    assert!(
        tail[..end].contains("=>"),
        "calculate_window_size_params arm for {view} has no =>"
    );
    &tail[..end]
}

fn compact_surface_kind_match_arm<'a>(compacted: &'a str, kind: &str) -> &'a str {
    let start = compacted
        .find(kind)
        .unwrap_or_else(|| panic!("surface contract missing {kind}"));
    let tail = &compacted[start..];
    let end = tail[kind.len()..]
        .find("SurfaceKind::")
        .map(|offset| kind.len() + offset)
        .unwrap_or(tail.len());
    assert!(
        tail[..end].contains("=>"),
        "surface contract arm for {kind} has no =>"
    );
    &tail[..end]
}

fn source_between<'a>(source: &'a str, start_marker: &str, end_marker: &str) -> &'a str {
    let start = source
        .find(start_marker)
        .unwrap_or_else(|| panic!("missing source marker: {start_marker}"));
    let tail = &source[start..];
    let end = tail
        .find(end_marker)
        .unwrap_or_else(|| panic!("missing end marker after {start_marker}: {end_marker}"));
    &tail[..end]
}

#[test]
fn builtin_execution_uses_dispatch_context_for_builtin_surface() {
    let content = read("src/app_execute/builtin_execution.rs");

    assert!(
        content.contains("DispatchContext::for_builtin("),
        "builtin_execution.rs must create DispatchContext::for_builtin(...)"
    );

    assert!(
        !content.contains("let trace_id = uuid::Uuid::new_v4().to_string();"),
        "builtin_execution.rs should not mint raw builtin trace ids directly"
    );
}

#[test]
fn builtin_execution_has_structured_outcome_logger() {
    let content = read("src/app_execute/builtin_execution.rs");

    let fn_start = content
        .find("fn log_builtin_outcome(")
        .expect("Expected log_builtin_outcome helper in builtin_execution.rs");
    let body = &content[fn_start..content.len().min(fn_start + 900)];

    for field in [
        "builtin_id =",
        "trace_id =",
        "surface =",
        "handler",
        "status =",
        "error_code =",
        "duration_ms",
    ] {
        assert!(
            body.contains(field),
            "log_builtin_outcome must include structured field `{field}`"
        );
    }
}

#[test]
fn dispatch_system_action_returns_dispatch_outcome() {
    let content = read("src/app_execute/builtin_execution.rs");

    let fn_start = content
        .find("fn dispatch_system_action(")
        .expect("Expected dispatch_system_action");
    let signature = &content[fn_start..content.len().min(fn_start + 260)];

    assert!(
        signature.contains("-> crate::action_helpers::DispatchOutcome"),
        "dispatch_system_action should return DispatchOutcome"
    );
}

#[test]
fn dispatch_system_action_accepts_dispatch_context() {
    let content = read("src/app_execute/builtin_execution.rs");

    let fn_start = content
        .find("fn dispatch_system_action(")
        .expect("Expected dispatch_system_action");
    let signature = &content[fn_start..content.len().min(fn_start + 260)];

    assert!(
        signature.contains("dctx: &crate::action_helpers::DispatchContext"),
        "dispatch_system_action must accept &DispatchContext"
    );
}

#[test]
fn handle_system_action_result_returns_dispatch_outcome() {
    let content = read("src/app_execute/builtin_execution.rs");

    let fn_start = content
        .find("fn handle_system_action_result(")
        .expect("Expected handle_system_action_result");
    let signature = &content[fn_start..content.len().min(fn_start + 400)];

    assert!(
        signature.contains("-> crate::action_helpers::DispatchOutcome"),
        "handle_system_action_result should return DispatchOutcome"
    );
}

#[test]
fn builtin_success_and_error_helpers_exist() {
    let content = read("src/app_execute/builtin_execution.rs");

    assert!(
        content.contains("fn builtin_success("),
        "Expected builtin_success helper in builtin_execution.rs"
    );
    assert!(
        content.contains("fn builtin_error("),
        "Expected builtin_error helper in builtin_execution.rs"
    );
}

#[test]
fn execute_builtin_with_query_logs_outcome_at_boundary() {
    let content = read("src/app_execute/builtin_execution.rs");

    let fn_start = content
        .find("fn execute_builtin_with_query(")
        .expect("Expected execute_builtin_with_query");
    let fn_body = &content[fn_start..content.len().min(fn_start + 4000)];

    assert!(
        fn_body.contains("Self::log_builtin_outcome("),
        "execute_builtin_with_query must log outcome at the dispatch boundary"
    );
}

// ---------------------------------------------------------------------------
// execute_builtin_inner — returns real DispatchOutcome
// ---------------------------------------------------------------------------

#[test]
fn execute_builtin_inner_returns_dispatch_outcome() {
    let content = read("src/app_execute/builtin_execution.rs");

    let fn_start = content
        .find("fn execute_builtin_inner(")
        .expect("Expected execute_builtin_inner");
    let signature = &content[fn_start..content.len().min(fn_start + 500)];

    assert!(
        signature.contains("-> crate::action_helpers::DispatchOutcome"),
        "execute_builtin_inner must return DispatchOutcome"
    );
}

#[test]
fn execute_builtin_with_query_uses_real_inner_outcome() {
    let content = read("src/app_execute/builtin_execution.rs");

    let fn_start = content
        .find("fn execute_builtin_with_query(")
        .expect("Expected execute_builtin_with_query");
    let fn_body = source_between(
        &content[fn_start..],
        "fn execute_builtin_with_query(",
        "    /// Open a filterable main-window builtin view with a consistent UX contract.",
    );

    // Must NOT contain the old synthetic success pattern
    assert!(
        !fn_body.contains("Self::builtin_success(&dctx, \"execute_builtin_inner\")"),
        "execute_builtin_with_query must not log unconditional success for non-system builtins"
    );

    // Must use the real outcome returned by execute_builtin_inner
    assert!(
        fn_body.contains("self.execute_builtin_inner(entry, query_override, &dctx, cx)"),
        "execute_builtin_with_query must use the real outcome returned by execute_builtin_inner"
    );
}

// ---------------------------------------------------------------------------
// open_builtin_filterable_view — shared filterable view helper
// ---------------------------------------------------------------------------

#[test]
fn filterable_view_builtins_use_shared_helper() {
    let content = read("src/app_execute/builtin_execution.rs");

    assert!(
        content.contains("fn open_builtin_filterable_view("),
        "Expected shared open_builtin_filterable_view helper"
    );

    // Named filterable builtins that must each route through the shared helper:
    // the helper call's first argument is the AppView the builtin opens.
    fn helper_opens_view(content: &str, view: &str) -> bool {
        let callee = "self.open_builtin_filterable_view(";
        let mut search = content;
        while let Some(index) = search.find(callee) {
            let after = &search[index + callee.len()..];
            if after.trim_start().starts_with(view) {
                return true;
            }
            search = after;
        }
        false
    }

    for view in [
        "AppView::FavoritesBrowseView",
        "AppView::AppLauncherView",
        "AppView::EmojiPickerView",
    ] {
        assert!(
            helper_opens_view(&content, view),
            "Expected the {view} builtin arm to open through open_builtin_filterable_view"
        );
    }
}

#[test]
fn open_builtin_filterable_view_sets_shared_focus_contract() {
    let content = read("src/app_execute/builtin_execution.rs");

    let fn_start = content
        .find("fn open_builtin_filterable_view(")
        .expect("Expected open_builtin_filterable_view");
    let body = &content[fn_start..content.len().min(fn_start + 1200)];

    for expected in [
        "self.filter_text.clear();",
        "self.pending_filter_sync = true;",
        "self.pending_placeholder = Some(",
        "self.current_view = view;",
        "self.hovered_index = None;",
        "self.opened_from_main_menu = true;",
        "MainWindowMode::Full",
        "\"open_builtin_filterable_view\"",
        "resize_to_view_sync(ViewType::MainWindow, 0);",
        "MainWindowMode::Mini",
        "self.pending_focus = Some(FocusTarget::MainFilter);",
        "self.focused_input = FocusedInput::MainFilter;",
        "cx.notify();",
    ] {
        assert!(
            body.contains(expected),
            "Missing required shared view-contract line: {expected}"
        );
    }
}

#[test]
fn deferred_sizing_keeps_mini_filterable_builtins_narrow() {
    let content = read("src/app_impl/ui_window.rs");
    let fn_start = content
        .find("pub(crate) fn calculate_window_size_params")
        .expect("Expected calculate_window_size_params");
    let fn_end = content[fn_start..]
        .find("    /// Returns the focused button when the active view is `ConfirmPrompt`.")
        .expect("Expected next method after calculate_window_size_params");
    let body = &content[fn_start..fn_start + fn_end];
    let compacted = compact_source(body);

    for view in [
        "AppView::EmojiPickerView{",
        "AppView::AppLauncherView{",
        "AppView::WindowSwitcherView{",
        "AppView::ProcessManagerView{",
        "AppView::CurrentAppCommandsView{",
        "AppView::BrowserTabsView{",
        "AppView::BrowseKitsView{",
        "AppView::InstalledKitsView{",
        "AppView::SearchAiPresetsView{",
        "AppView::SettingsView{",
        "AppView::FavoritesBrowseView{",
    ] {
        let arm = compact_app_view_match_arm(&compacted, view);
        assert!(
            arm.contains("ViewType::MainWindow"),
            "calculate_window_size_params must keep single-column builtin {view} on MainWindow"
        );
        assert!(
            !arm.contains("ViewType::ScriptList"),
            "calculate_window_size_params must not widen single-column builtin {view} to ScriptList"
        );
    }
}

#[test]
fn deferred_sizing_keeps_preview_builtins_wide() {
    let content = read("src/app_impl/ui_window.rs");
    let fn_start = content
        .find("pub(crate) fn calculate_window_size_params")
        .expect("Expected calculate_window_size_params");
    let fn_end = content[fn_start..]
        .find("    /// Returns the focused button when the active view is `ConfirmPrompt`.")
        .expect("Expected next method after calculate_window_size_params");
    let body = &content[fn_start..fn_start + fn_end];
    let compacted = compact_source(body);

    for view in [
        "AppView::ClipboardHistoryView{",
        "AppView::ThemeChooserView{",
        "AppView::SdkReferenceView{",
        "AppView::TipsView{",
        "AppView::ScriptTemplateCatalogView{",
        "AppView::AgentChatHistoryView{",
        "AppView::BrowserHistoryView{",
        "AppView::DictationHistoryView{",
        "AppView::NotesBrowseView{",
    ] {
        let arm = compact_app_view_match_arm(&compacted, view);
        assert!(
            arm.contains("ViewType::MainWindow"),
            "calculate_window_size_params must keep preview/detail builtin {view} on MainWindow"
        );
        assert!(
            !arm.contains("ViewType::ScriptList"),
            "calculate_window_size_params must not size preview/detail builtin {view} as ScriptList"
        );
    }
}

#[test]
fn file_search_sizing_tracks_presentation_contract() {
    let content = read("src/app_impl/ui_window.rs");
    let body = source_between(
        &content,
        "pub(crate) fn calculate_window_size_params",
        "    /// Returns the focused button when the active view is `ConfirmPrompt`.",
    );
    let compacted = compact_source(body);
    let arm = compact_app_view_match_arm(&compacted, "AppView::FileSearchView{");
    assert!(
        arm.contains("FileSearchPresentation::Mini=>ViewType::MainWindow")
            && arm.contains("FileSearchPresentation::Full=>ViewType::MainWindow"),
        "FileSearchView must size Mini and Full presentations through MainWindow"
    );
}

#[test]
fn surface_contracts_match_main_window_sizing_shape() {
    let app_view_state_source = read("src/main_sections/app_view_state.rs");
    let contract_start = app_view_state_source
        .find("impl SurfaceKind")
        .expect("app_view_state.rs must define SurfaceKind contracts");
    let app_view_state = compact_source(&app_view_state_source[contract_start..]);
    let layout_info = compact_source(&read("src/app_layout/build_layout_info.rs"));

    for (kind, preview, visual, view_type) in [
        (
            "SurfaceKind::ThemeChooser",
            "RequiredSplitPreview",
            "SplitPreviewVisual",
            "AppView::ThemeChooserView{..}",
        ),
        (
            "SurfaceKind::FileSearchMini",
            "NoPersistentPreview",
            "CompactLauncherVisual",
            "FileSearchPresentation::Mini=>crate::window_resize::ViewType::MainWindow",
        ),
        (
            "SurfaceKind::FileSearchFull",
            "RequiredSplitPreview",
            "SplitPreviewVisual",
            "FileSearchPresentation::Full=>crate::window_resize::ViewType::MainWindow",
        ),
    ] {
        let contract = compact_surface_kind_match_arm(&app_view_state, kind);
        assert!(
            contract.contains(preview) && contract.contains(visual),
            "{kind} contract must declare {preview} and {visual}"
        );
        assert!(
            layout_info.contains(view_type),
            "build_layout_info must map {kind} to the matching main-window view type"
        );
    }
}

#[test]
fn filterable_view_builtins_are_silent_on_success() {
    let content = read("src/app_execute/builtin_execution.rs");

    for needle in [
        "builtins::BuiltInFeature::ClipboardHistory",
        "builtins::BuiltInFeature::Favorites",
        "builtins::BuiltInFeature::AppLauncher",
        "builtins::BuiltInFeature::WindowSwitcher",
    ] {
        let start = content
            .find(needle)
            .unwrap_or_else(|| panic!("Missing match arm: {needle}"));
        let block = &content[start..content.len().min(start + 1600)];

        assert!(
            !block.contains("show_hud(") && !block.contains("Toast::"),
            "{needle} should stay silent on success; the view transition is the feedback"
        );
    }
}

#[test]
fn app_launch_failure_returns_error_outcome() {
    let content = read("src/app_execute/builtin_execution.rs");

    let app_launch = content
        .find("fn execute_app_launch_builtin(")
        .expect("Expected execute_app_launch_builtin helper");
    let block = &content[app_launch..content.len().min(app_launch + 2600)];

    // Failed app launches must produce an error outcome
    assert!(
        block.contains("ERROR_LAUNCH_FAILED"),
        "Failed app launches must use ERROR_LAUNCH_FAILED error code"
    );
    assert!(
        block.contains("Self::builtin_error("),
        "Failed app launches must return builtin_error outcome"
    );
}

#[test]
fn window_switcher_failure_returns_error_outcome() {
    let content = read("src/app_execute/builtin_execution.rs");

    let ws_branch = content
        .find("builtins::BuiltInFeature::WindowSwitcher")
        .expect("Expected WindowSwitcher branch");
    let block = source_between(
        &content[ws_branch..],
        "builtins::BuiltInFeature::WindowSwitcher",
        "            builtins::BuiltInFeature::BrowserTabs",
    );

    // list_windows failure must produce an error outcome
    assert!(
        block.contains("Self::builtin_error("),
        "Window switcher failure must return builtin_error outcome"
    );
    assert!(
        block.contains("open_window_switcher_failed"),
        "Window switcher failure detail must indicate what failed"
    );
}
