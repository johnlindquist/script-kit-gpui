//! Source-level contract for the triggerBuiltin filterable route state machine.
//!
//! `show_filterable_view` is only the entrypoint. Cache preload, shared filter
//! reset, placeholder/focus cleanup, current-view assignment, and deferred
//! resize must stay behind one explicit route-entry state machine so future
//! agents can audit the transition without reintroducing per-view assignment
//! arms in the entrypoint.

const DISPATCH: &str = include_str!("../src/app_impl/trigger_builtin_dispatch.rs");

fn body_of<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing function signature: {signature}"));
    let open_rel = source[start..]
        .find('{')
        .unwrap_or_else(|| panic!("missing function body open: {signature}"));
    let open = start + open_rel;
    let mut depth = 0usize;
    for (offset, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..open + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("missing function body close: {signature}");
}

fn compact(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn show_filterable_view_delegates_to_state_machine() {
    let body = body_of(DISPATCH, "fn show_filterable_view(");
    assert!(
        body.contains("self.run_filterable_route_state_machine(view, window, cx)"),
        "show_filterable_view must delegate route entry to the state-machine owner"
    );
    assert!(
        !body.contains("self.current_view = AppView::"),
        "show_filterable_view must not grow per-view direct AppView assignment arms"
    );
    assert!(
        !body.contains("update_automation_semantic_surface(")
            && !body.contains("semantic_surface_for_main_view(&self.current_view)"),
        "filterable route entry must not take over triggerBuiltin semantic-surface re-keying"
    );
}

#[test]
fn route_state_machine_has_explicit_terminal_states() {
    for required in [
        "enum FilterableRouteState",
        "Start(FilterableView)",
        "Prepared(FilterableRoutePlan)",
        "Failed {",
        "Applied {",
        "surface_kind: SurfaceKind",
    ] {
        assert!(
            DISPATCH.contains(required),
            "route machine must expose explicit state marker `{required}`"
        );
    }

    let body = body_of(DISPATCH, "fn run_filterable_route_state_machine(");
    let compact = compact(body);
    for required in [
        "FilterableRouteState::Start(view)=>matchself.prepare_filterable_route(view)",
        "Ok(plan)=>FilterableRouteState::Prepared(plan)",
        "Err(reason)=>FilterableRouteState::Failed{view,reason}",
        "FilterableRouteState::Prepared(plan)=>{self.apply_filterable_route_plan(plan,window,cx)}",
        "terminal@FilterableRouteState::Applied{..}=>returnterminal",
    ] {
        assert!(
            compact.contains(required),
            "state-machine body must contain ordered transition `{required}`"
        );
    }
}

#[test]
fn filterable_view_preparation_preserves_route_specific_side_effects() {
    let prepare = body_of(DISPATCH, "fn prepare_filterable_route(");

    for required in [
        "FilterableView::ClipboardHistory",
        "FilterableView::AppLauncher",
        "FilterableView::BrowserTabs",
        "FilterableView::EmojiPicker",
        "FilterableView::WindowSwitcher",
        "FilterableView::ProcessManager",
        "FilterableView::KitStoreBrowse",
        "FilterableView::KitStoreInstalled",
        "FilterableView::Favorites",
        "FilterableView::SearchAiPresets",
        "crate::clipboard_history::get_cached_entries(100)",
        "crate::browser_tabs::list_open_tabs()",
        "crate::window_control::list_windows()",
        "crate::process_manager::PROCESS_MANAGER.get_active_processes_sorted()",
        "Search open browser tabs...",
        "Search Emoji & Symbols...",
        "Search windows...",
        "Search running scripts...",
        "Search Kit Store...",
        "Search installed kits...",
        "Search favorites...",
        "Search AI presets...",
    ] {
        assert!(
            prepare.contains(required),
            "prepare_filterable_route must retain `{required}`"
        );
    }

    assert!(
        prepare.contains("return Err(reason);") && prepare.contains("Failed to list windows"),
        "fallible preload routes must return Err instead of mutating current_view"
    );
    assert!(
        !prepare.contains("self.current_view ="),
        "preparation may seed caches but must not assign current_view"
    );
}

#[test]
fn apply_filterable_route_plan_is_the_single_assignment_step() {
    let apply = body_of(DISPATCH, "fn apply_filterable_route_plan(");
    assert!(
        apply.contains("self.current_view = plan.next_view;"),
        "apply step must own the single filterable-route current_view assignment"
    );
    assert!(
        apply.contains("self.restore_current_view_with_focus(plan.next_view, focus);"),
        "apply step must synchronize pending_focus and focused_input through the shared focus restore owner"
    );
    assert!(
        apply.contains("let surface_kind = self.current_view.surface_kind();"),
        "apply step must derive the applied SurfaceKind from the assigned AppView"
    );
    assert!(
        apply.contains("self.update_window_size_deferred(window, cx);"),
        "apply step must preserve deferred resize ownership"
    );

    let assignment_count = DISPATCH.matches("self.current_view =").count();
    assert_eq!(
        assignment_count, 1,
        "trigger_builtin_dispatch.rs should expose exactly one filterable-route current_view assignment"
    );

    for forbidden in [
        "self.current_view = AppView::ClipboardHistoryView",
        "self.current_view = AppView::AppLauncherView",
        "self.current_view = AppView::BrowserTabsView",
        "self.current_view = AppView::EmojiPickerView",
        "self.current_view = AppView::WindowSwitcherView",
        "self.current_view = AppView::ProcessManagerView",
    ] {
        assert!(
            !DISPATCH.contains(forbidden),
            "direct per-view assignment arm must stay collapsed: {forbidden}"
        );
    }
}
