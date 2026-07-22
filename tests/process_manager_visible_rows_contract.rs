//! Source-level contract for AURP-18 Process Manager row projection.
//!
//! Process Manager filtering should have one visible-row owner shared by render,
//! keyboard navigation, getState counts, getElements rows, and Tab AI targets.

const PROCESS_MANAGER: &str = include_str!("../src/render_builtins/process_manager.rs");
const COLLECT_ELEMENTS: &str = include_str!("../src/app_layout/collect_elements.rs");
const PROMPT_HANDLER: &str = include_str!("../src/prompt_handler/mod.rs");
const TAB_AI_MODE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");

fn source_between<'a>(source: &'a str, start_pat: &str, end_pat: &str) -> &'a str {
    let start = source
        .find(start_pat)
        .unwrap_or_else(|| panic!("missing source start: {start_pat}"));
    let end_rel = source[start..]
        .find(end_pat)
        .unwrap_or_else(|| panic!("missing source end: {end_pat}"));
    &source[start..start + end_rel]
}

#[test]
fn process_manager_declares_visible_row_helper_family() {
    for required in [
        "fn process_manager_filter_matches(",
        "fn process_manager_filtered_entries",
        "fn process_manager_visible_row_names(&self, filter: &str) -> Vec<String>",
        "fn process_manager_dataset_and_visible_counts(&self, filter: &str) -> (usize, usize)",
        "fn process_manager_selected_visible_entry(",
        "fn process_manager_selected_visible_row_name(",
        "fn process_manager_visible_target_rows(",
        "process.script_path.to_lowercase().contains(filter_lower)",
        "process.pid.to_string().contains(filter_lower)",
    ] {
        assert!(
            PROCESS_MANAGER.contains(required),
            "process manager visible-row helper family must contain: {required}"
        );
    }
}

#[test]
fn process_manager_render_and_keyboard_use_visible_entry_helper() {
    let render_body = source_between(
        PROCESS_MANAGER,
        "fn render_process_manager(",
        "        // Pre-compute colors",
    );

    assert!(
        render_body
            .matches("process_manager_filtered_entries(")
            .count()
            >= 2,
        "render and keyboard paths should both use process_manager_filtered_entries"
    );
}

#[test]
fn process_manager_refresh_is_visible_row_owned() {
    for required in [
        "self.builtin_uniform_list_scrollbar(&self.process_list_scroll_handle",
        "Clamp selection index against the visible filtered rows.",
        "Self::process_manager_filtered_entries(\n                                        &app.cached_processes,\n                                        filter,",
        "Self::process_manager_filtered_entries(\n                                        &this.cached_processes,\n                                        &current_filter,",
    ] {
        assert!(
            PROCESS_MANAGER.contains(required),
            "Process Manager scrolling/refresh should stay visible-row-owned: {required}"
        );
    }
}

#[test]
fn process_manager_chrome_and_clicks_use_shared_contracts() {
    for required in [
        "AppChromeColors::from_theme(&self.theme)",
        "rgba(chrome.text_hint_rgba)",
        ".h_full()\n                .flex()\n                .flex_col()\n                .items_center()\n                .justify_center()",
        "cx.stop_propagation();",
        "trigger_process_manager_stop_all(cx)",
    ] {
        assert!(
            PROCESS_MANAGER.contains(required),
            "Process Manager chrome/click handling should use shared contract: {required}"
        );
    }
    for legacy in [
        "let text_dimmed = self.theme.colors.text.dimmed;",
        ".text_color(rgb(self.theme.colors.text.muted))",
    ] {
        assert!(
            !PROCESS_MANAGER.contains(legacy),
            "Process Manager should not use raw muted/dimmed theme text: {legacy}"
        );
    }
}

#[test]
fn process_manager_state_and_elements_use_visible_row_helpers() {
    let elements_body = source_between(
        COLLECT_ELEMENTS,
        "AppView::ProcessManagerView {\n                filter,",
        "\n            AppView::CurrentAppCommandsView",
    );
    assert!(
        elements_body.contains("let rows = self.process_manager_visible_row_names(filter);"),
        "getElements must read Process Manager rows from the shared helper"
    );

    let state_body = source_between(
        PROMPT_HANDLER,
        "AppView::ProcessManagerView {\n                        filter,",
        "\n                    AppView::CurrentAppCommandsView",
    );
    assert!(
        state_body.contains("self.process_manager_dataset_and_visible_counts(filter)"),
        "getState must read Process Manager counts from the shared helper"
    );
    assert!(
        state_body
            .contains("self.process_manager_selected_visible_row_name(filter, *selected_index)"),
        "getState must resolve selectedValue from the same visible-row helper"
    );
}

#[test]
fn process_manager_tab_ai_targets_use_visible_row_helpers() {
    let arm_body = source_between(
        TAB_AI_MODE,
        "AppView::ProcessManagerView {\n                filter,",
        "\n            AppView::CurrentAppCommandsView",
    );

    assert!(
        TAB_AI_MODE.contains("fn tab_ai_target_from_process_manager_row("),
        "Tab AI target shaping should have a named Process Manager adapter"
    );
    assert!(
        arm_body.contains(".process_manager_selected_visible_entry(filter, *selected_index)"),
        "focused Tab AI target must resolve selected_index against filtered visible rows"
    );
    assert!(
        arm_body
            .contains(".process_manager_visible_target_rows(filter, TAB_AI_VISIBLE_TARGET_LIMIT)"),
        "visible Tab AI targets must come from the shared Process Manager row projection"
    );
    assert!(
        !arm_body.contains("self.cached_processes.get(*selected_index)"),
        "Process Manager Tab AI must not read selected_index from the raw process dataset"
    );
    assert!(
        !arm_body.contains("self.cached_processes\n                    .iter()"),
        "Process Manager Tab AI must not build visible targets from the raw process dataset"
    );
}
