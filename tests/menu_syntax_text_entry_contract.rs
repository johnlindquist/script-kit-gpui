use std::fs;

fn source_after<'a>(source: &'a str, signature: &str) -> &'a str {
    let index = source
        .find(signature)
        .unwrap_or_else(|| panic!("source must contain `{signature}`"));
    let tail = &source[index..];
    let open = tail.find('{').expect("function should have body");
    let mut depth = 0usize;
    for (offset, ch) in tail[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return &tail[..open + offset + 1];
                }
            }
            _ => {}
        }
    }
    tail
}

#[test]
fn capture_composer_suppresses_main_list_before_capture_grouping() {
    let source = fs::read_to_string("src/app_impl/filtering_cache.rs")
        .expect("Failed to read src/app_impl/filtering_cache.rs");

    let ownership_gate = source
        .find("capture_composer_owns_input_for(&raw_filter_text)")
        .expect("filtering cache must gate capture composer ownership");
    let capture_grouping = source
        .find("capture_for(&raw_filter_text)")
        .expect("filtering cache must keep capture grouping for non-composer paths");

    assert!(
        ownership_gate < capture_grouping,
        "capture composer ownership must blank the main list before capture handler grouping can run"
    );
    assert!(
        source.contains("capture-handler rows"),
        "the suppression comment should document hidden capture-handler rows"
    );
}

#[test]
fn capture_composer_renders_hint_surface_instead_of_empty_state() {
    let source = fs::read_to_string("src/render_script_list/mod.rs")
        .expect("Failed to read src/render_script_list/mod.rs");

    assert!(
        source.contains("let menu_syntax_owns_main_list =")
            && source.contains("capture_composer_owns_input_for(&filter_text_for_render)")
            && source.contains("if menu_syntax_owns_main_list")
            && source.contains("menu_syntax_main_hint_snapshot(&filter_text_for_render, false)")
            && source.contains("render_menu_syntax_main_hint"),
        "menu syntax ownership should render the read-only grammar hint surface, not No results messaging"
    );

    let ownership_branch = source
        .find("if menu_syntax_owns_main_list")
        .expect("render should branch on menu syntax ownership");
    let empty_state_branch = source
        .find("else if item_count == 0")
        .expect("render should keep the normal empty state for ordinary searches");
    assert!(
        ownership_branch < empty_state_branch,
        "menu syntax ownership must hide stale grouped rows before item_count decides which list to render"
    );
}

#[test]
fn menu_syntax_hint_surface_has_dedicated_scroll_and_arrow_routing() {
    let render = fs::read_to_string("src/render_script_list/mod.rs")
        .expect("Failed to read src/render_script_list/mod.rs");
    let app_state = fs::read_to_string("src/main_sections/app_state.rs")
        .expect("Failed to read src/main_sections/app_state.rs");
    let arrow = fs::read_to_string("src/app_impl/startup_new_arrow.rs")
        .expect("Failed to read src/app_impl/startup_new_arrow.rs");
    let main_hint = fs::read_to_string("src/app_impl/menu_syntax_main_hint.rs")
        .expect("Failed to read src/app_impl/menu_syntax_main_hint.rs");

    assert!(
        app_state.contains("menu_syntax_main_hint_scroll_handle: ScrollHandle"),
        "the read-only menu syntax panel needs a dedicated free-scroll handle, separate from selectable lists"
    );
    assert!(
        render.contains("fn render_menu_syntax_main_hint(")
            && render.contains("scroll_handle: &ScrollHandle")
            && render.contains(".track_scroll(scroll_handle)")
            && render.contains(".overflow_y_scroll()")
            && render.contains("ScrollableElement::vertical_scrollbar"),
        "the menu syntax hint panel must scroll when its content is taller than the main list area"
    );
    assert!(
        main_hint.contains("fn scroll_menu_syntax_main_hint(&mut self, direction: f32)")
            && main_hint.contains("menu_syntax_main_hint_scroll_handle")
            && main_hint.contains("FREE_SCROLL_LINE_DELTA_PX"),
        "keyboard scroll should use the same free-scroll handle as the rendered hint panel"
    );
    assert!(
        arrow.contains("let menu_syntax_owns_main_list =")
            && arrow.contains("capture_composer_owns_input_for(&this.filter_text)")
            && arrow.contains("command_owns_input_for(&this.filter_text)")
            && arrow
                .contains("this.scroll_menu_syntax_main_hint(if is_down { 1.0 } else { -1.0 });"),
        "ScriptList Up/Down should scroll the owned menu syntax panel before normal history/list navigation"
    );
}

#[test]
fn refine_picker_owns_main_list_until_query_is_terminal() {
    let popup = fs::read_to_string("src/app_impl/menu_syntax_trigger_picker.rs")
        .expect("Failed to read src/app_impl/menu_syntax_trigger_picker.rs");
    let filtering_cache = fs::read_to_string("src/app_impl/filtering_cache.rs")
        .expect("Failed to read src/app_impl/filtering_cache.rs");
    let render = fs::read_to_string("src/render_script_list/mod.rs")
        .expect("Failed to read src/render_script_list/mod.rs");

    let owns_main_list_body = source_after(&popup, "fn owns_main_list(&self) -> bool");
    assert!(
        owns_main_list_body.contains("TriggerPickerMode::AdvancedQuery")
            && owns_main_list_body.contains("TriggerPickerMode::Capture")
            && owns_main_list_body.contains("TriggerPickerMode::Command"),
        "colon filter-head and value pickers should own ScriptList rows until the query is terminal"
    );
    assert!(
        filtering_cache.contains("self.menu_syntax_trigger_picker_state.owns_main_list()")
            && render.contains("self.menu_syntax_trigger_picker_state.owns_main_list()"),
        "refine (`:`) picker snapshots should suppress stale structured search results while selectable rows are open"
    );
    assert!(
        filtering_cache.contains("free_text_for_search(&self.menu_syntax_mode, filter_text)")
            && filtering_cache.contains("apply_advanced_query(results, query)"),
        "automation state counts should use the same advanced-query search text and predicates as rendered grouping"
    );
}

#[test]
fn exact_colon_drawer_uses_filter_head_catalog_not_static_examples() {
    let trigger_picker = fs::read_to_string("src/menu_syntax/trigger_picker.rs")
        .expect("Failed to read src/menu_syntax/trigger_picker.rs");

    assert!(
        trigger_picker.contains("fn is_exact_bare_colon(input: &str) -> bool")
            && trigger_picker.contains("fn bare_colon_filter_head_rows() -> Vec<TriggerPickerRow>")
            && trigger_picker.contains("ADVANCED_QUERY_HEAD_ROW_SPECS")
            && !source_after(&trigger_picker, "fn bare_colon_filter_head_rows()")
                .contains("SOURCE_HEAD_SPECS")
            && trigger_picker.contains("return bare_colon_filter_head_rows();"),
        "bare ':' should use only the dedicated filter-head catalog, not source heads like files:"
    );
    assert!(
        trigger_picker.contains(
            "advanced_query_active_token(input).is_empty() && !is_exact_bare_colon(input)"
        ),
        "exact ':' must not append recent advanced queries after the filter-head catalog"
    );
    assert!(
        trigger_picker.contains("display_token: \"meta.<path>:\",")
            && trigger_picker.contains("insert_token: \"meta.\","),
        "bare ':' should advertise the generic meta.<path>: head instead of a concrete metadata example"
    );
}

#[test]
fn capture_target_picker_closes_when_body_composition_starts() {
    let trigger_picker = fs::read_to_string("src/menu_syntax/trigger_picker.rs")
        .expect("Failed to read src/menu_syntax/trigger_picker.rs");
    let keys = fs::read_to_string("src/menu_syntax/trigger_picker_keys.rs")
        .expect("Failed to read src/menu_syntax/trigger_picker_keys.rs");

    assert!(
        trigger_picker
            .contains("capture_body_boundary_has_started_with_targets(input, &capture_targets)")
            && trigger_picker.contains("return None;"),
        "target picker should stop producing snapshots after ;target body composition starts"
    );
    assert!(
        trigger_picker.contains("keep_open: false"),
        "capture target InsertToken should commit ;target and close the picker"
    );
    assert!(
        !trigger_picker.contains("rows.extend(capture_handler_rows"),
        "capture handlers must not render inside the target picker"
    );
    assert!(
        keys.contains("keep_open: *keep_open"),
        "open-value qualifier rows should be able to keep the popup open on Accept or Apply"
    );
}

#[test]
fn trigger_picker_state_changes_rebuild_main_list() {
    let filter_input_change = fs::read_to_string("src/app_impl/filter_input_change.rs")
        .expect("Failed to read src/app_impl/filter_input_change.rs");

    assert!(
        filter_input_change.contains("let mut trigger_state_changed = false;")
            && filter_input_change.contains("trigger_state_changed = true;")
            && filter_input_change.contains("} else if trigger_state_changed {")
            && filter_input_change.contains("self.invalidate_grouped_cache();")
            && !filter_input_change.contains("sync_menu_syntax_trigger_picker_main_list_for_filter"),
        "filter changes should keep trigger rows in state and rebuild the main list without syncing a detached popup"
    );
}

#[test]
fn trigger_picker_main_list_contract_exposes_rows_without_detached_popup() {
    let trigger_owner = fs::read_to_string("src/app_impl/menu_syntax_trigger_picker_main_list.rs")
        .expect("Failed to read src/app_impl/menu_syntax_trigger_picker_main_list.rs");
    let prompt_handler =
        fs::read_to_string("src/prompt_handler/mod.rs").expect("Failed to read prompt_handler.rs");

    assert!(
        trigger_owner.contains("menu_syntax_trigger_picker_owns_main_keyboard")
            && trigger_owner.contains("self.menu_syntax_trigger_picker_state.owns_main_list()"),
        "trigger picker keyboard ownership should be derived from ScriptList state"
    );
    assert!(
        !prompt_handler.contains(
            "menu_syntax_trigger_picker_main_list::is_menu_syntax_trigger_picker_main_list_open"
        ),
        "PromptPopup automation target resolution must not include main-owned trigger rows"
    );
}

#[test]
fn stdin_setfilter_runs_menu_syntax_popup_state_machine() {
    let updates = fs::read_to_string("src/app_impl/filter_input_updates.rs")
        .expect("Failed to read src/app_impl/filter_input_updates.rs");
    let popup_window = fs::read_to_string("src/app_impl/menu_syntax_trigger_picker_main_list.rs")
        .expect("Failed to read src/app_impl/menu_syntax_trigger_picker_main_list.rs");

    assert!(
        updates.contains("self.run_menu_syntax_trigger_picker_state_machine(&text, window, cx);")
            && updates.contains("crate::menu_syntax::build_trigger_picker_snapshot(&text, &picker_ctx).is_some()")
            && updates.contains("self.invalidate_grouped_cache();"),
        "programmatic setFilter must run the same menu-syntax picker state machine as real typing and invalidate stale grouped rows"
    );
    assert!(
        popup_window.contains("pub(crate) fn run_menu_syntax_trigger_picker_state_machine"),
        "the popup state-machine helper should be shared by keyboard input and setFilter"
    );
}

#[test]
fn launcher_input_accents_power_syntax_prefixes() {
    let render = fs::read_to_string("src/render_script_list/mod.rs")
        .expect("Failed to read src/render_script_list/mod.rs");
    let input_state = fs::read_to_string("vendor/gpui-component/crates/ui/src/input/state.rs")
        .expect("Failed to read vendored input state");
    let input_element = fs::read_to_string("vendor/gpui-component/crates/ui/src/input/element.rs")
        .expect("Failed to read vendored input element");

    assert!(
        render.contains("input_spans_for_input_with_targets(")
            && render.contains("state.set_highlight_ranges_with_roles(input_highlight_ranges)"),
        "ScriptList input should replace the rendered menu-syntax input chip set every render tick"
    );
    assert!(
        input_state.contains("highlight_ranges: Vec<(Range<usize>, Hsla)>")
            && input_state.contains("pub fn set_highlight_ranges"),
        "the input component should expose plain-text highlight ranges"
    );
    assert!(
        input_state.contains("pub fn set_highlight_ranges_with_roles")
            && input_state.contains("pub fn clear_highlight_ranges"),
        "the input component should replace and clear rendered chip ranges, including empty sets"
    );
    assert!(
        input_element.contains("fn custom_highlight_styles")
            && input_element.contains("state.highlight_ranges"),
        "plain input highlight ranges should be converted into text runs"
    );
}

#[test]
fn typing_an_alias_never_auto_executes() {
    // A1 decision (2026-06-09): aliases pin the aliased command at index 0
    // (see `pin_alias_match_into_grouped_results`) instead of executing on a
    // trailing space. Typing in the main input must never run a command
    // directly — execution requires Enter on the pinned row.
    let source = fs::read_to_string("src/app_impl/filter_input_change.rs")
        .expect("Failed to read src/app_impl/filter_input_change.rs");

    assert!(
        !source.contains("find_alias_match"),
        "handle_filter_input_change must not resolve aliases for auto-execution; \
         alias handling lives in the grouping pin (pin_alias_match_into_grouped_results)"
    );

    let cache = fs::read_to_string("src/app_impl/filtering_cache.rs")
        .expect("Failed to read src/app_impl/filtering_cache.rs");
    assert!(
        cache.contains("fn pin_alias_match_into_grouped_results")
            && cache.contains("pin_alias_match_first"),
        "exact alias matches must pin the aliased command at the top of the grouped list"
    );
}

#[test]
fn enter_in_capture_composer_executes_capture_not_main_selection() {
    let source = fs::read_to_string("src/app_impl/selection_fallback.rs")
        .expect("Failed to read src/app_impl/selection_fallback.rs");
    let execute_selected = source_after(&source, "pub(crate) fn execute_selected");

    let capture_route = execute_selected
        .find("capture_for(&self.filter_text)")
        .expect("execute_selected must first check for capture composer input");
    let grouped_results = execute_selected
        .find("self.live_script_list_flat_selection_for_submit()")
        .expect("execute_selected still resolves main-list rows for normal search");

    assert!(
        capture_route < grouped_results,
        "Enter in capture composer must route to capture execution before main-list selection"
    );
    assert!(
        source.contains("rank_handlers_for_target")
            && source.contains("execute_menu_syntax_capture_script"),
        "capture composer Enter should execute the ranked capture handler"
    );
}

#[test]
fn command_invocation_suppresses_main_list_and_routes_without_shell() {
    let filtering_cache = fs::read_to_string("src/app_impl/filtering_cache.rs")
        .expect("Failed to read src/app_impl/filtering_cache.rs");
    let selection = fs::read_to_string("src/app_impl/selection_fallback.rs")
        .expect("Failed to read src/app_impl/selection_fallback.rs");
    let execution = fs::read_to_string("src/app_execute/menu_syntax_execution.rs")
        .expect("Failed to read src/app_execute/menu_syntax_execution.rs");

    assert!(
        filtering_cache.contains("command_owns_input_for(filter_text)")
            && filtering_cache.contains("command_owns_input_for(&raw_filter_text)"),
        "`!` command composition must blank normal launcher results"
    );

    let execute_selected = source_after(&selection, "pub(crate) fn execute_selected");
    let command_route = execute_selected
        .find("command_for(&self.filter_text)")
        .expect("execute_selected must first check command invocation input");
    let grouped_results = execute_selected
        .find("self.live_script_list_flat_selection_for_submit()")
        .expect("execute_selected still resolves main-list rows for normal search");
    assert!(
        command_route < grouped_results,
        "Enter in ! command mode must route before main-list selection"
    );

    assert!(
        execution.contains("execute_menu_syntax_command_invocation")
            && execution.contains("script_command_head")
            && execution.contains("scriptlet_command_head")
            && execution.contains("execute_interactive_with_env_and_args")
            && execution.contains("execute_scriptlet_with_env_and_args")
            && execution.contains("menu_syntax_command_ambiguous")
            && !execution.contains("Command::new(&invocation.head)")
            && !execution.contains("Command::new(invocation.head"),
        "`!` must resolve registered Script Kit commands, pass env to scripts/scriptlets, reject ambiguous heads, and never spawn typed text as shell"
    );
}

#[test]
fn power_syntax_tags_and_command_picker_are_first_class() {
    let query = fs::read_to_string("src/menu_syntax/query.rs")
        .expect("Failed to read src/menu_syntax/query.rs");
    let trigger_picker = fs::read_to_string("src/menu_syntax/trigger_picker.rs")
        .expect("Failed to read src/menu_syntax/trigger_picker.rs");
    let popup = fs::read_to_string("src/app_impl/menu_syntax_trigger_picker.rs")
        .expect("Failed to read src/app_impl/menu_syntax_trigger_picker.rs");

    assert!(
        query.contains("strip_prefix('#')")
            && query.contains("Predicate::Tag")
            && query.contains("\"tag\""),
        "`:#tag` and `:tag:value` should parse as refine tag predicates"
    );
    assert!(
        trigger_picker.contains("title: \"Filter by tag\"")
            && trigger_picker.contains("insert: \"#\"")
            && trigger_picker.contains("insert: \"tag:\"")
            && trigger_picker.contains("keep_open: true"),
        "`:` picker should teach both :#tag sugar and canonical tag: filters, keeping the popup open for a tag name"
    );
    assert!(
        trigger_picker.contains("TriggerPickerMode::Command")
            && trigger_picker.contains("build_command_snapshot")
            && trigger_picker.contains("script_command_head")
            && trigger_picker.contains("scriptlet_command_head"),
        "`!` should have a real trigger-picker mode backed by registered commands"
    );
    assert!(
        trigger_picker.contains("bang_command_snapshot(input, ctx)")
            && popup.contains("trigger != ';' && trigger != '+' && trigger != ':'"),
        "command trigger rows should be handled by the trigger picker while picker partial filtering stays scoped to text-composer triggers"
    );
}

#[test]
fn registered_capture_targets_extend_parser_popup_and_input_highlight() {
    let filter_input_core = fs::read_to_string("src/app_impl/filter_input_core.rs")
        .expect("Failed to read src/app_impl/filter_input_core.rs");
    let parse = fs::read_to_string("src/menu_syntax/parse.rs")
        .expect("Failed to read src/menu_syntax/parse.rs");
    let trigger_picker = fs::read_to_string("src/menu_syntax/trigger_picker.rs")
        .expect("Failed to read src/menu_syntax/trigger_picker.rs");
    let mode = fs::read_to_string("src/menu_syntax/mode.rs")
        .expect("Failed to read src/menu_syntax/mode.rs");

    assert!(
        filter_input_core.contains("registered_capture_targets_from_scripts(&self.scripts)")
            && filter_input_core.contains("from_input_with_capture_targets"),
        "ScriptList should parse ;target with registered capture targets from metadata"
    );
    assert!(
        parse.contains("parse_with_capture_targets")
            && parse.contains("is_capture_target_registered"),
        "the parser should distinguish unknown ;text search from registered ;target capture"
    );
    assert!(
        trigger_picker.contains("registered_capture_targets(ctx)")
            && trigger_picker.contains("capture_target_catalog(ctx, filter.is_some())"),
        "the picker should show registered capture target rows and hidden aliases only when filtering"
    );
    assert!(
        mode.contains("prefix_span_for_input_with_targets")
            && mode.contains("capture_body_boundary_has_started_with_targets"),
        "registered targets should get composer ownership and input accent spans"
    );
}

#[test]
fn menu_syntax_hint_surface_is_not_grouped_results() {
    let render = fs::read_to_string("src/render_script_list/mod.rs")
        .expect("Failed to read src/render_script_list/mod.rs");
    let filtering_cache = fs::read_to_string("src/app_impl/filtering_cache.rs")
        .expect("Failed to read src/app_impl/filtering_cache.rs");
    let main_hint = fs::read_to_string("src/menu_syntax/main_hint.rs")
        .expect("Failed to read src/menu_syntax/main_hint.rs");

    assert!(
        render.contains("fn render_menu_syntax_main_hint(")
            && render.contains("MenuSyntaxMainHintSnapshot")
            && render.contains("menu_syntax_main_hint_snapshot(&filter_text_for_render, true)"),
        "the grammar hint should be a rendered snapshot for owned states and advanced-query empty states"
    );
    assert!(
        filtering_cache.contains("Arc::<[GroupedListItem]>::from(Vec::new())")
            && !render.contains("build_menu_syntax_hint_results("),
        "grammar hints must not repopulate the selectable GroupedListItem list"
    );
    assert!(
        render.contains(".h_full()")
            && render.contains("hint.examples")
            && render.contains("advanced_query_guide_hint")
            && render.contains("MenuSyntaxMainHintKind::AdvancedQueryGuide")
            && !render.contains("MenuSyntaxMainHintKind::AdvancedQueryEmpty =>")
            && render.contains("Plain #tag is launcher search"),
        "hint cards should fill the list area, render persistent examples, keep completed advanced-query results visible until item_count is zero, and nudge top-level #tag search toward :#tag or ; capture labels"
    );
    assert!(
        main_hint.contains("AdvancedQueryGuide")
            && main_hint.contains("Filter by tag")
            && main_hint.contains("Refine launcher search"),
        "bare/partial `:` states should get guide hints before they become zero-result structured queries"
    );
}

#[test]
fn has_shortcut_accept_transition_cannot_reopen_popup() {
    let trigger_picker = fs::read_to_string("src/menu_syntax/trigger_picker.rs")
        .expect("Failed to read src/menu_syntax/trigger_picker.rs");
    let dispatcher = fs::read_to_string("src/app_impl/menu_syntax_trigger_picker_main_list.rs")
        .expect("Failed to read src/app_impl/menu_syntax_trigger_picker_main_list.rs");
    let filter_change = fs::read_to_string("src/app_impl/filter_input_change.rs")
        .expect("Failed to read src/app_impl/filter_input_change.rs");
    let startup = fs::read_to_string("src/app_impl/startup.rs")
        .expect("Failed to read src/app_impl/startup.rs");
    let startup_new_tab = fs::read_to_string("src/app_impl/startup_new_tab.rs")
        .expect("Failed to read src/app_impl/startup_new_tab.rs");
    let simulate_key = fs::read_to_string("src/main_entry/runtime_stdin_match_simulate_key.rs")
        .expect("Failed to read src/main_entry/runtime_stdin_match_simulate_key.rs");
    let app_run_setup = fs::read_to_string("src/main_entry/app_run_setup.rs")
        .expect("Failed to read src/main_entry/app_run_setup.rs");
    let runtime_stdin = fs::read_to_string("src/main_entry/runtime_stdin.rs")
        .expect("Failed to read src/main_entry/runtime_stdin.rs");

    assert!(
        trigger_picker.contains("fn should_show_has_field_completion(")
            && trigger_picker.contains("lookup_has_field(value).is_none()")
            && trigger_picker.contains("complete has:shortcut is a search predicate"),
        "exact has:shortcut must be terminal search input, while has:short/has:shortc remain completion states"
    );
    assert!(
        dispatcher.contains("self.filter_text = text.clone();")
            && dispatcher.contains("self.pending_filter_sync = true;")
            && dispatcher.contains("self.computed_filter_text = text.clone();")
            && dispatcher.contains("self.set_menu_syntax_mode_from_filter(&text);")
            && dispatcher.contains("self.invalidate_grouped_cache();")
            && dispatcher
                .contains("self.menu_syntax_trigger_picker_suppressed_filter = Some(text.clone());"),
        "Accept must atomically replace input, advance parser/cache state, and suppress immediate reopen"
    );
    assert!(
        filter_change.contains("popup_suppressed_for_this_text")
            && filter_change.contains("plan_trigger_picker_transition("),
        "filter input changes must honor post-Accept suppression before re-running the picker state machine"
    );
    assert!(
        startup.contains("InlinePickerKeyIntent::Accept")
            && startup.contains("is_plain_enter")
            && startup.contains("menu_syntax_trigger_picker_owns_main_keyboard()")
            && startup_new_tab.contains("InlinePickerKeyIntent::Accept")
            && startup_new_tab.contains("is_plain_enter")
            && startup_new_tab.contains("menu_syntax_trigger_picker_owns_main_keyboard()"),
        "physical Enter startup paths must route to menu-syntax trigger Accept before ordinary launcher Enter"
    );
    for source in [&simulate_key, &app_run_setup, &runtime_stdin] {
        let accept = source
            .find("SimulateKey: Enter - accept menu-syntax picker")
            .expect("simulateKey Enter must log picker acceptance");
        let execute = source
            .find("SimulateKey: Enter - execute selected")
            .expect("simulateKey Enter must retain ordinary execution fallback");
        assert!(
            accept < execute,
            "protocol simulateKey Enter must accept menu-syntax picker before ordinary launcher execution"
        );
    }
}

#[test]
fn has_shortcut_results_stay_on_refine_result_path() {
    let query = fs::read_to_string("src/menu_syntax/query.rs")
        .expect("Failed to read src/menu_syntax/query.rs");
    let filter = fs::read_to_string("src/menu_syntax/filter.rs")
        .expect("Failed to read src/menu_syntax/filter.rs");
    let filtering_cache = fs::read_to_string("src/app_impl/filtering_cache.rs")
        .expect("Failed to read src/app_impl/filtering_cache.rs");
    let render = fs::read_to_string("src/render_script_list/mod.rs")
        .expect("Failed to read src/render_script_list/mod.rs");
    let prompt_handler = fs::read_to_string("src/prompt_handler/mod.rs")
        .expect("Failed to read src/prompt_handler/mod.rs");

    assert!(
        query.contains("fn classify_has_predicate_value(")
            && query.contains("partial_has_shortcut_values_do_not_claim_filter_query")
            && query.contains("has_shortcut_complete_and_trailing_space_parse_same"),
        "parser must treat has:shortcut and has:shortcut-space as complete predicates while leaving has:short partial"
    );
    assert!(
        filter.contains("Predicate::Has(field) => has_field(result, field)")
            && filter.contains("\"shortcut\" => script.shortcut.is_some()")
            && filter.contains("\"shortcut\" => scriptlet.shortcut.is_some()")
            && filter.contains("has_shortcut_matches_scriptlet_or_snippet_shortcut_rows"),
        "advanced-query filtering must find script and scriptlet shortcut rows without seed-data hacks"
    );
    assert!(
        filtering_cache.contains("apply_advanced_query(results, query)")
            && filtering_cache.contains("advanced_predicate_query"),
        "grouped result count path must apply advanced predicates before render/getState count receipts"
    );
    assert!(
        render.contains("MenuSyntaxMainHintKind::AdvancedQueryGuide")
            && !render.contains("MenuSyntaxMainHintKind::AdvancedQueryEmpty =>")
            && render.contains("item_count == 0"),
        "render must keep completed advanced-query rows visible before falling back to empty hint cards"
    );
    assert!(
        prompt_handler.contains("advanced_query_has_results")
            && prompt_handler.contains("visible_choice_count > 0"),
        "getState must not report a structured-empty hint when a completed advanced query has visible rows"
    );
}

#[test]
fn state_result_exposes_menu_syntax_main_hint_for_agentic_tests() {
    let variants = fs::read_to_string("src/protocol/message/variants/query_ops.rs")
        .expect("Failed to read src/protocol/message/variants/query_ops.rs");
    let constructors = fs::read_to_string("src/protocol/message/constructors/query_ops.rs")
        .expect("Failed to read src/protocol/message/constructors/query_ops.rs");
    let prompt_handler = fs::read_to_string("src/prompt_handler/mod.rs")
        .expect("Failed to read src/prompt_handler/mod.rs");

    assert!(
        variants.contains("menuSyntaxMainHint") && variants.contains("MenuSyntaxMainHintSnapshot"),
        "stateResult should expose the same grammar hint snapshot the main menu renders"
    );
    assert!(
        constructors.contains(
            "menu_syntax_main_hint: Option<crate::menu_syntax::MenuSyntaxMainHintSnapshot>"
        ) && constructors.contains("            menu_syntax_main_hint,"),
        "Message::state_result should forward menu_syntax_main_hint without ad hoc protocol construction"
    );
    assert!(
        prompt_handler.contains("self.menu_syntax_main_hint_snapshot(")
            && prompt_handler.contains("advanced_query_results_empty"),
        "getState should compute owned-state and structured-empty grammar hints for automation"
    );
}

#[test]
fn capture_composer_hint_explains_handler_ranking_in_state_hint_surface() {
    let main_hint = fs::read_to_string("src/menu_syntax/main_hint.rs")
        .expect("Failed to read src/menu_syntax/main_hint.rs");
    let handler_index = fs::read_to_string("src/menu_syntax/handler_index.rs")
        .expect("Failed to read src/menu_syntax/handler_index.rs");

    assert!(
        handler_index.contains("explain_capture_handler_ranking")
            && handler_index.contains("rank_handlers_for_target")
            && handler_index.contains("dedupe_ranked_handlers_by_path"),
        "handler ranking explanation should be backed by the existing ranker and execution dedupe"
    );
    assert!(
        main_hint.contains("explain_capture_handler_ranking")
            && main_hint.contains("Handler")
            && main_hint.contains("Why selected")
            && main_hint.contains("Handler conflict"),
        "capture composer hint should expose handler ranking through MenuSyntaxMainHintSnapshot rows"
    );
}
