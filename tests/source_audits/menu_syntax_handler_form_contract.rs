use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

#[test]
fn menu_syntax_handler_form_snapshot_is_state_first_and_autocomplete_ready() {
    let form = read("src/menu_syntax/form.rs");
    assert!(
        form.contains("pub struct MenuSyntaxFormSnapshot")
            && form.contains("pub struct MenuSyntaxFormFieldSnapshot")
            && form.contains("pub struct MenuSyntaxFormSuggestion"),
        "menu syntax handler mode must expose a structured form snapshot for getState/devtools"
    );
    assert!(
        form.contains("tab_ai_disabled: true"),
        "handler form snapshots must explicitly record that Tab AI is disabled while the form owns Tab"
    );
    assert!(
        form.contains("tagHistory") && form.contains("schema"),
        "handler form fields must expose suggestion sources for autocomplete"
    );
    assert!(
        form.contains("apply_capture_form_field_edit")
            && form.contains("serialize_capture_invocation"),
        "handler form must own a parser-derived form-field edit path back to canonical capture text"
    );
    assert!(
        form.contains("empty_capture_invocation"),
        "bare committed handlers like ;todo and ;note must synthesize an empty invocation so fields render before a body exists"
    );
    let app = read("src/app_impl/menu_syntax_main_hint.rs");
    assert!(
        app.contains("menu_syntax_capture_form_invocation")
            && app.contains("menu_syntax_capture_form_target_for")
            && app.contains("menu_syntax_capture_form_owns_input_for")
            && app.contains("builtin_schema")
            && app.contains("IncompleteKind::MissingCaptureBody")
            && app.contains("empty_capture_invocation"),
        "app form ownership, snapshots, and form-field edits must share the same incomplete-handler invocation path and only claim handlers with schemas"
    );
    let state = read("src/main_sections/app_state.rs");
    assert!(
        state.contains("menu_syntax_form_input_active")
            && state.contains("menu_syntax_form_draft_field_id")
            && state.contains("menu_syntax_form_draft_value"),
        "handler forms need separate visible-form, active-field, and draft-value state so main input can keep focus until Tab arms the first field"
    );
}

#[test]
fn rendered_capture_composer_uses_form_before_instruction_rows() {
    let render = read("src/render_script_list/mod.rs");
    let form_render = render
        .find("fn render_menu_syntax_form(")
        .expect("render_menu_syntax_form must exist");
    let form_child = render
        .find("hint.form.as_ref()")
        .expect("capture hint render must include hint.form");
    let row_child = render
        .find("hint.rows.is_empty()")
        .expect("capture hint render must still include legacy rows");
    assert!(
        form_render < form_child && form_child < row_child,
        "handler form should render as the primary composer surface before legacy instructional rows"
    );
}

#[test]
fn semicolon_capture_composer_outer_shell_consumes_non_list_language_without_owning_fields() {
    let render = read("src/render_script_list/mod.rs");
    let hint_start = render
        .find("fn render_menu_syntax_main_hint(")
        .expect("render_menu_syntax_main_hint must exist");
    let hint_end = render[hint_start..]
        .find("fn render_script_list_empty_state(")
        .map(|offset| hint_start + offset)
        .expect("empty-state renderer must follow menu syntax hint renderer");
    let hint_renderer = &render[hint_start..hint_end];

    assert!(
        hint_renderer.contains("crate::components::non_list_palette(theme)")
            && hint_renderer.contains(
                "crate::components::non_list_metrics(crate::components::NonListDensity::Compact)"
            )
            && hint_renderer.contains("crate::components::non_list_content_stack(")
            && hint_renderer.contains("\"menu-syntax-main-hint-content\"")
            && hint_renderer.contains("metrics.max_width")
            && hint_renderer.contains("metrics.block_gap"),
        "semicolon capture composer outer shell should use the shared compact non-list language"
    );
    assert!(
        hint_renderer.contains(".id(\"menu-syntax-main-hint-scroll\")")
            && hint_renderer.contains(".track_scroll(scroll_handle)")
            && hint_renderer.contains(".overflow_y_scroll()"),
        "non-list shell adoption must preserve the main-hint scroll owner"
    );
    assert!(
        hint_renderer.contains("hint.form.as_ref()")
            && hint_renderer.contains("render_menu_syntax_form("),
        "non-list shell adoption must keep the parser-backed form renderer in place"
    );

    let field_start = render
        .find("fn render_menu_syntax_form_field(")
        .expect("handler form field renderer must exist");
    let form_start = render
        .find("fn render_menu_syntax_form(")
        .expect("handler form renderer must exist");
    let field_renderer = &render[field_start..form_start];
    assert!(
        field_renderer.contains("FormFieldMetrics::from_theme_and_design")
            && !field_renderer.contains("non_list_palette")
            && !field_renderer.contains("non_list_metrics")
            && !field_renderer.contains("non_list_card"),
        "handler field internals should remain owned by shared form-field metrics, not the non-list shell"
    );
}

#[test]
fn form_mode_hides_ask_ai_hint_and_uses_accent_focus_style() {
    let render = read("src/render_script_list/mod.rs");
    let form_guard = render
        .find("handler_form_owns_input_for_render")
        .expect("render should compute handler form ownership for header chrome");
    let show_hint = render
        .find("show_launcher_ask_ai_hint")
        .expect("render should name the Ask AI hint visibility gate");
    assert!(
        form_guard < show_hint && render.contains("let show_launcher_ask_ai_hint = false;"),
        "the three-zone launcher must keep the old top-right Ask/Tab header affordance disabled"
    );
    assert!(
        !render.contains("render_launcher_ask_ai_hint"),
        "ScriptList header must not reintroduce the removed Ask AI hint renderer"
    );

    // Shared-shell focus, validation, and placeholder behavior is covered by
    // behavior tests in `components::form_fields`; do not couple this source
    // audit to the shell renderer's internal color or layout expressions.
    assert!(
        !render.contains("Tab moves fields"),
        "handler form should avoid visible keyboard-instruction copy"
    );
    assert!(
        read("src/app_impl/menu_syntax_main_hint.rs")
            .contains("state.set_tab_navigation(handler_form_owns_input, window, cx)")
            && read("src/app_impl/menu_syntax_main_hint.rs").contains(
                "state.set_tab_navigation_space_as_tab(false, window, cx)"
            )
            && read("vendor/gpui-component/crates/ui/src/input/state.rs")
                .contains("InputEvent::PressTab { secondary: false }")
            && read("src/app_impl/filter_input_updates.rs")
                .contains("self.sync_menu_syntax_form_inputs_from_filter(window, cx);")
            && render.contains("sk_is_key_tab(key_str)")
            && render.contains("this.focus_next_menu_syntax_form_field(window, cx);"),
        "main filter input must propagate Tab while handler forms own input so the form can take real cursor focus"
    );

    let filter_change = read("src/app_impl/filter_input_change.rs");
    assert!(
        !filter_change.contains("trim_end_matches(' ').to_string()")
            && !filter_change.contains("self.focus_next_menu_syntax_form_field(window, cx);\n                return;"),
        "typing a trailing space in the main input must not trim the command or auto-focus the first handler field"
    );
}

#[test]
fn snippet_form_body_is_state_visible_multiline_with_clean_placeholders() {
    let form = read("src/menu_syntax/form.rs");
    assert!(
        form.contains("pub multiline: bool")
            && form.contains("fn menu_syntax_form_field_is_multiline(")
            && form.contains("matches!(requirement, FieldRequirement::Body)")
            && form.contains("schema.target.eq_ignore_ascii_case(\"snippet\")"),
        "snippet body multiline support must be state-visible and classified from schema/requirement"
    );
    assert!(
        form.contains("fn kv_placeholder_for_key(")
            && form.contains("\"Snippet name\".to_string()")
            && form.contains("\"Expansion keyword\".to_string()")
            && form.contains("\"Optional description\".to_string()")
            && form.contains("\"Shortcut or @existing\".to_string()"),
        "capture form placeholders should describe values without duplicating label/key prefixes"
    );
    assert!(
        !form.contains("format!(\"{key}:value\")")
            && !form.contains("\"name:Snippet name\"")
            && !form.contains("\"trigger:shortcut or @existing\"")
            && !form.contains("\"keyword:value\"")
            && !form.contains("\"description:value\""),
        "placeholder strings must not regress to duplicated key prefixes"
    );
    assert!(
        form.contains("fn serialize_snippet_create_capture_invocation(")
            && form.contains("parts.push(\"--\".to_string());")
            && form.contains("parse_snippet_scriptlet_capture(invocation)")
            && form.contains("SnippetScriptletOperation::Create"),
        "snippet create serialization must isolate multiline body text after the -- delimiter"
    );
}

#[test]
fn snippet_form_multiline_input_uses_chat_pattern_and_enter_routing() {
    let app = read("src/app_impl/menu_syntax_main_hint.rs");
    assert!(
        app.contains("field.multiline")
            && app.contains("crate::components::FormFieldMetrics::MULTILINE_MIN_ROWS")
            && app.contains("crate::components::FormFieldMetrics::MULTILINE_MAX_ROWS")
            && app.contains(".submit_on_enter(true)"),
        "multiline menu-syntax fields should use a two-row minimum and the agent-chat submit-on-enter pattern"
    );
    assert!(
        !app.contains(".auto_grow(2, 6)"),
        "multiline row bounds should come from shared form metrics"
    );
    assert!(
        app.contains("InputEvent::PressEnter { secondary }")
            && app.contains("submit_menu_syntax_form_enter")
            && app.contains("if *secondary")
            && app.contains("input.read(cx).value().to_string()")
            && app.contains("this.menu_syntax_form_syncing_from_input = true;")
            && app.contains("update_menu_syntax_form_field(\n                                        Some(&field_id),"),
        "multiline field Enter events should submit on plain Enter and sync secondary Enter newline insertion from InputState"
    );
    assert!(
        app.contains("modifiers.shift && active_field.is_some_and(|field| field.multiline)")
            && app.contains("return false;"),
        "physical Shift+Enter on multiline fields must fall through to InputState before form-level Enter handling"
    );
    assert!(
        app.contains("field.multiline")
            && app.contains("value.push('\\n');")
            && app.contains("modifiers.shift || modifiers.platform"),
        "protocol/simulateKey secondary Enter should mirror newline insertion only for multiline fields"
    );
}

#[test]
fn snippet_multiline_values_are_sanitized_before_plain_gpui_text_children() {
    let render = read("src/render_script_list/mod.rs");
    assert!(
        render.contains("fn menu_syntax_single_line_text_for_gpui("),
        "menu syntax render must sanitize newline-bearing values before GPUI plain text children"
    );
    assert!(
        render.contains("fn menu_syntax_text_contains_line_break("),
        "menu syntax render must detect newline-bearing serialized filters before single-line input rendering"
    );

    let row_start = render
        .find("fn render_menu_syntax_hint_row(")
        .expect("hint row renderer must exist");
    let preview_start = render
        .find("fn render_menu_syntax_fragment_preview_row(")
        .expect("fragment preview renderer must exist");
    let field_start = render
        .find("fn render_menu_syntax_form_field(")
        .expect("form field renderer must exist");
    let form_start = render
        .find("fn render_menu_syntax_form(")
        .expect("form renderer must exist");
    let row_renderer = &render[row_start..preview_start];
    let preview_renderer = &render[preview_start..field_start];
    let field_renderer = &render[field_start..form_start];

    assert!(
        row_renderer.contains("menu_syntax_single_line_text_for_gpui(&row.value)")
            && !row_renderer.contains(".child(row.value.clone())"),
        "hint rows must not pass raw newline-bearing row values into GPUI text"
    );
    assert!(
        preview_renderer.contains("menu_syntax_single_line_text_for_gpui(&format!(")
            && !preview_renderer.contains(".child(format!(\"{}: {}\", row.label, row.value))"),
        "fragment preview rows must not pass raw newline-bearing values into GPUI text"
    );
    assert!(
        field_renderer.contains("menu_syntax_single_line_text_for_gpui(&field.value)"),
        "form fallback display must sanitize multiline values without mutating field state"
    );
    assert!(
        render.contains("let input_body = if handler_form_owns_input_for_render")
            && render.contains("&& menu_syntax_text_contains_line_break(&filter_text_for_render)")
            && render.contains("self.render_search_input_with_ghost(cx).into_any_element()")
            && render.contains("menu_syntax_single_line_text_for_gpui(\n                    &filter_text_for_render,\n                )"),
        "newline-bearing capture filters must bypass the single-line header input renderer with display-only sanitized text"
    );
}

#[test]
fn capture_form_required_state_moves_to_labels_and_header_chips_are_empty() {
    let form = read("src/menu_syntax/form.rs");
    assert!(
        form.contains("fn required_form_label")
            && form.contains("format!(\"{label} *\")")
            && form.contains("let label = required_form_label(label, required);")
            && form.contains("required,")
            && form.contains("satisfied,"),
        "required handler form fields should keep semantic booleans while adding the visible `*` in snapshot labels"
    );

    let capture_schema = read("src/menu_syntax/capture_schema.rs");
    assert!(
        capture_schema.contains("FieldRequirement::Body => \"body\"")
            && capture_schema.contains(
                "FieldRequirement::SnippetNameOrSelection => \"name or selected snippet\""
            )
            && !capture_schema.contains("\"body *\"")
            && !capture_schema.contains("name or selected snippet *"),
        "capture validation labels must stay semantic and unstarred"
    );

    let hint = read("src/menu_syntax/main_hint.rs");
    let capture_start = hint
        .find("kind: MenuSyntaxMainHintKind::CaptureComposer")
        .expect("capture composer snapshot must exist");
    let capture_body = &hint[capture_start..];
    assert!(
        capture_body.contains("mode_chip: None")
            && capture_body.contains("status_chip: None")
            && capture_body.contains("status_chips: Vec::new()")
            && capture_body.contains("capture_validation,"),
        "capture form state should keep capture_validation but remove header mode/status chips"
    );
    let validation_start = hint
        .find("fn capture_validation_snapshot(")
        .expect("capture validation helper must exist");
    let validation_end = hint[validation_start..]
        .find("fn payload_for_capture_validation(")
        .map(|offset| validation_start + offset)
        .expect("payload helper must follow capture validation helper");
    let validation_helper = &hint[validation_start..validation_end];
    assert!(
        !validation_helper.contains("chip(\"; capture\"")
            && !validation_helper.contains("format!(\"needs {label}\")")
            && !validation_helper.contains("chip(\"malformed\""),
        "capture validation helper should no longer construct visible header chips"
    );
}

#[test]
fn capture_form_scroll_owner_and_tab_reveal_are_natural() {
    let render = read("src/render_script_list/mod.rs");
    assert!(
        render.contains("gpui_component::scroll::ScrollableElement::vertical_scrollbar")
            && render.contains(".id(\"menu-syntax-main-hint-scroll\")")
            && render.contains(".track_scroll(scroll_handle)")
            && render.contains(".overflow_y_scroll()"),
        "capture form should keep the main-hint scroll surface as the single scroll owner"
    );
    assert!(
        render.contains(".id(\"menu-syntax-handler-form-fields\")")
            && render.contains(".on_children_prepainted(")
            && render.contains("*field_bounds.borrow_mut() = bounds;"),
        "form fields container must record real prepaint bounds for focus reveal"
    );

    let form_owner = read("src/app_impl/menu_syntax_main_hint.rs");
    let reveal_start = form_owner
        .find("fn reveal_menu_syntax_form_field_at(")
        .expect("Tab focus reveal helper must exist");
    let reveal_end = form_owner[reveal_start..]
        .find("fn focus_menu_syntax_form_input_at(")
        .map(|offset| reveal_start + offset)
        .expect("focus helper must follow reveal helper");
    let reveal = &form_owner[reveal_start..reveal_end];
    assert!(
        !reveal.contains("target_y = gpui::px(-((index as f32) * APPROX_FIELD_HEIGHT_PX))")
            && !reveal.contains("APPROX_FIELD_HEIGHT_PX")
            && reveal.contains("const REVEAL_MARGIN_PX: f32 = 20.0;")
            && reveal.contains("menu_syntax_form_field_bounds")
            && reveal.contains("menu_syntax_main_hint_scroll_handle.bounds()")
            && reveal.contains("visible_top")
            && reveal.contains("visible_bottom"),
        "Tab reveal should scroll measured field bounds into view instead of top-aligning every focused field from a fixed per-field height estimate"
    );
}

#[test]
fn stdin_can_edit_menu_syntax_form_fields_for_runtime_proof() {
    let commands = read("src/stdin_commands/mod.rs");
    assert!(
        commands.contains("SetMenuSyntaxFormField")
            && commands.contains("\"setMenuSyntaxFormField\""),
        "stdin automation needs a direct form-field edit command for state-first runtime proof"
    );

    for path in [
        "src/main_entry/runtime_stdin.rs",
        "src/main_entry/runtime_stdin_match_core.rs",
        "src/main_entry/app_run_setup.rs",
    ] {
        let source = read(path);
        assert!(
            source.contains("ExternalCommand::SetMenuSyntaxFormField")
                && source.contains("update_menu_syntax_form_field"),
            "{path}: setMenuSyntaxFormField must route through the app's form sync method"
        );
    }
}

#[test]
fn tab_routes_to_handler_form_before_tab_ai_paths() {
    let form_owner = read("src/app_impl/menu_syntax_main_hint.rs");
    assert!(
        form_owner.contains("gpui_component::input::InputState::new(window, cx)")
            && form_owner.contains(".tab_navigation(true)")
            && form_owner.contains("state.set_selection(len, len, window, cx)")
            && form_owner.contains("state.focus(window, cx)")
            && form_owner.contains("actual_menu_syntax_form_focused_index"),
        "Tab routing must create real focusable handler field inputs, place the cursor at the end, and move actual focus handles"
    );
    assert!(
        form_owner.contains("focus_menu_syntax_main_input")
            && form_owner.contains("self.gpui_input_state")
            && form_owner.contains("state.focus(window, cx)"),
        "handler form traversal must be able to return actual focus to the main input"
    );

    let preflight = read("src/main_window_preflight/build.rs");
    assert!(
        preflight.contains("AppView::ScriptList if app.menu_syntax_capture_form_owns_input() => None")
            && preflight.contains("return \"handler-form\".to_string();")
            && preflight.contains("return Vec::new();"),
        "handler forms must not expose stale launcher selected/visible rows or Enter targets through preflight"
    );
    let form_disabled = preflight
        .find("menu_syntax_capture_form_owns_input()")
        .expect("main-window preflight must disable Tab AI while handler form owns Tab");
    let ask_ai = preflight
        .find("MainWindowPreflightActionKind::AskAi")
        .expect("main-window preflight must still define Ask AI for normal launcher text");
    assert!(
        form_disabled < ask_ai,
        "preflight tabAction must be None for handler forms before emitting Ask AI"
    );

    for path in ["src/app_impl/startup.rs", "src/app_impl/startup_new_tab.rs"] {
        let source = read(path);
        assert!(
            source.contains("menu_syntax_capture_form_owns_input()"),
            "{path}: physical Tab path must check handler form ownership"
        );
        assert!(
            !source.contains("try_route_plain_tab_to_agent_chat_context_capture"),
            "{path}: physical Tab must not keep a Tab-to-Agent-Chat route"
        );
    }

    for path in [
        "src/main_entry/runtime_stdin.rs",
        "src/main_entry/runtime_stdin_match_simulate_key.rs",
        "src/main_entry/app_run_setup.rs",
    ] {
        let source = read(path);
        assert!(
            !source.contains("try_route_plain_tab_to_agent_chat_context_capture"),
            "{path}: simulateKey Tab must not keep a Tab-to-Agent-Chat route"
        );
    }
}

#[test]
fn committed_handler_form_ownership_suppresses_global_popups_on_all_filter_paths() {
    let immediate = read("src/app_impl/filter_input_updates.rs");
    assert!(
        immediate.contains("let mut handler_form_owns_input = false;")
            && immediate
                .contains("handler_form_owns_input = self.menu_syntax_capture_form_owns_input_for(&text);")
            && immediate.contains("let handler_form_field_owns_input =")
            && immediate.contains("self.menu_syntax_form_input_active && handler_form_owns_input")
            && immediate.contains("if handler_form_field_owns_input")
            && immediate.contains("self.menu_syntax_object_selector_state = Default::default();")
            && immediate.contains("self.menu_syntax_trigger_picker_state = Default::default();")
            && !immediate.contains("close_menu_syntax_object_selector_main_list(cx)")
            && immediate.contains("if !handler_form_field_owns_input")
            && immediate.contains("self.menu_syntax_object_selector_state.snapshot.is_none()")
            && immediate.contains("self.run_menu_syntax_trigger_picker_state_machine(&text, window, cx);"),
        "programmatic setFilter/setInput must suppress trigger/object popups only while a focused handler-form field owns input"
    );

    let input_change = read("src/app_impl/filter_input_change.rs");
    let owner = input_change
        .find("let capture_composer_owns_input =")
        .expect("typed input path must compute committed handler-form ownership");
    let object = input_change
        .find("run_menu_syntax_object_selector_state_machine")
        .expect("typed input path should still run object selector outside handler forms");
    let trigger = input_change
        .find("plan_trigger_picker_transition")
        .expect("typed input path should still plan trigger picker outside handler forms");
    assert!(
        owner < object && owner < trigger,
        "typed input must decide handler-form ownership before object/trigger picker machines can claim the surface"
    );
    assert!(
        input_change.contains("let capture_form_field_owns_input =")
            && input_change
                .contains("self.menu_syntax_form_input_active && capture_composer_owns_input")
            && input_change.contains("} else if capture_form_field_owns_input {\n                crate::menu_syntax_trigger_picker::TriggerPickerTransition::NoChange")
            && input_change.contains("!capture_form_field_owns_input\n                && self.menu_syntax_object_selector_state.snapshot.is_some()"),
        "focused handler-form fields must prevent stale popup transitions while plain object-ref segments can own the main list"
    );

    let render = read("src/render_script_list/mod.rs");
    assert!(
        render.contains("self.menu_syntax_capture_form_owns_input_for(&filter_text_for_render)")
            && render.contains("let popup_owns_main_list = !handler_form_owns_input_for_render")
            && render.contains("|| handler_form_owns_input_for_render")
            && render.contains("let menu_syntax_owns_main_list = popup_owns_main_list")
            && render.contains("capture_composer_owns_main_list"),
        "render ownership must use the app-level form owner and give handler forms precedence over stale popups"
    );
}

#[test]
fn batch_set_input_uses_window_aware_immediate_filter_writer() {
    let prompt = read("src/prompt_handler/mod.rs");
    assert!(
        prompt.contains("fn set_main_window_input_text_for_batch(")
            && prompt.contains("app.set_input_text_in_window(&text, window, cx);")
            && prompt.contains("fn set_input_text_in_window(")
            && prompt.contains("self.set_filter_text_immediate(text.to_string(), window, cx);"),
        "batch input updates need the main window so ScriptList setInput can run the same immediate filter/form sync path as user typing"
    );

    let main_batch = prompt
        .find("// ── Main-window batch path")
        .expect("main-window batch path must be named");
    let set_input = prompt[main_batch..]
        .find("protocol::BatchCommand::SetInput { text }")
        .map(|idx| main_batch + idx)
        .expect("main-window batch setInput arm must exist");
    let select_by_value = prompt[set_input..]
        .find("protocol::BatchCommand::SelectByValue")
        .map(|idx| set_input + idx)
        .expect("setInput arm should be followed by selectByValue");
    let set_input_arm = &prompt[set_input..select_by_value];
    assert!(
        set_input_arm.contains("set_main_window_input_text_for_batch(")
            && !set_input_arm.contains("set_input_text(text, cx)"),
        "batch setInput must not bypass set_filter_text_immediate on ScriptList"
    );

    for (arm, next) in [
        (
            "protocol::BatchCommand::FilterAndSelect",
            "protocol::BatchCommand::SelectIndex",
        ),
        (
            "protocol::BatchCommand::TypeAndSubmit",
            "protocol::BatchCommand::WaitFor",
        ),
    ] {
        let start = prompt[main_batch..]
            .find(arm)
            .map(|idx| main_batch + idx)
            .unwrap_or_else(|| panic!("{arm} must exist"));
        let end = prompt[start..]
            .find(next)
            .map(|idx| start + idx)
            .unwrap_or(prompt.len());
        let body = &prompt[start..end];
        assert!(
            body.contains("set_main_window_input_text_for_batch("),
            "{arm} must share the same window-aware input path before selecting/submitting"
        );
    }
}

#[test]
fn script_list_printable_simulate_key_can_update_filter_text() {
    let updates = read("src/app_impl/filter_input_updates.rs");
    let helper_start = updates
        .find("pub(crate) fn handle_script_list_printable_simulate_key(")
        .expect("ScriptList printable simulateKey helper must exist");
    let helper_end = updates[helper_start..]
        .find("/// Write the given filter text")
        .map(|idx| helper_start + idx)
        .expect("helper should stay before builtin subview input writer");
    let helper = &updates[helper_start..helper_end];
    assert!(
        helper.contains("key_char: Option<&str>")
            && helper.contains("!matches!(self.current_view, AppView::ScriptList)")
            && helper.contains("modifiers.platform || modifiers.alt || modifiers.control")
            && helper.contains("menu_syntax_form_input_active && self.menu_syntax_capture_form_owns_input()")
            && helper.contains("next.push_str(ch);")
            && helper.contains("self.set_filter_text_immediate(next, window, cx);"),
        "printable simulateKey support must append plain characters through the immediate filter writer and stay out of active handler fields"
    );
}

#[test]
fn handler_form_autocomplete_is_state_first_and_trigger_picker_owned() {
    let form = read("src/menu_syntax/form.rs");
    for symbol in [
        "pub struct MenuSyntaxFormSuggestionApplication",
        "pub fn apply_menu_syntax_form_suggestion",
        "pub struct MenuSyntaxFormSuggestionPools",
        "pub struct MenuSyntaxFormSuggestion",
        "pub detail: Option<String>",
        "pub suggestion_query: String",
        "pub selected_suggestion_index: Option<usize>",
        "pub objects: Vec<crate::menu_syntax::ObjectSelectorCandidate>",
        "filter_tag_suggestions",
        "filter_object_suggestions",
        "object_selector_candidate_matches",
        "first_object_token_from_invocation",
        "object_refs_for_raw_capture",
    ] {
        assert!(
            form.contains(symbol),
            "form autocomplete missing `{symbol}`"
        );
    }

    let app = read("src/app_impl/menu_syntax_main_hint.rs");
    let popup = read("src/app_impl/menu_syntax_trigger_picker_main_list.rs");
    let object_selector = read("src/app_impl/menu_syntax_object_selector_main_list.rs");
    for symbol in [
        "handle_menu_syntax_form_control_key_input",
        "move_menu_syntax_form_suggestion_selection",
        "accept_menu_syntax_form_suggestion",
        "annotate_menu_syntax_form_suggestion_selection",
        "apply_menu_syntax_form_suggestion",
        "update_menu_syntax_form_field",
        "menu_syntax_form_suggestion_field_id",
        "menu_syntax_form_suggestion_selected_index",
        "sync_menu_syntax_form_suggestions_from_main_input",
        "build_menu_syntax_form_trigger_picker_snapshot",
        "sync_menu_syntax_form_trigger_picker_state",
        "menu_syntax_form_trigger_picker_row_id",
        "main_input_form_completion_field",
        "active_menu_syntax_form_suggestion_field",
        "search_root_object_candidates_direct",
    ] {
        assert!(app.contains(symbol), "app autocomplete missing `{symbol}`");
    }
    assert!(
        app.contains("token.len() == 1 || !token[1..].chars().all"),
        "a bare # in the main input must open tag autocomplete instead of being treated like a numeric token"
    );

    let accept_start = app
        .find("fn accept_menu_syntax_form_suggestion(")
        .expect("accept_menu_syntax_form_suggestion must exist");
    let sync_start = app
        .find("fn sync_menu_syntax_form_draft_from_form(")
        .expect("sync_menu_syntax_form_draft_from_form must exist");
    let accept_body = &app[accept_start..sync_start];
    assert!(
        accept_body.contains("update_menu_syntax_form_field")
            && !accept_body.contains("menu_syntax_object_selector_state")
            && !accept_body.contains("plan_object_selector_transition"),
        "form suggestion acceptance must stay state-owned and sync through the form field edit path"
    );

    assert!(
        popup.contains("accept_menu_syntax_form_trigger_picker_suggestion")
            && popup.contains("parse_trigger_picker_form_suggestion_row_id")
            && popup.contains("menu_syntax_trigger_picker_state_is_form_suggestion")
            && popup.contains("close_menu_syntax_form_trigger_picker")
            && popup.contains("apply_menu_syntax_form_suggestion")
            && popup.contains("update_menu_syntax_form_field"),
        "handler form suggestions must accept through the shared Trigger Popup window and sync through canonical form edits"
    );

    assert!(
        object_selector.contains("self.menu_syntax_form_input_active")
            && object_selector.contains("self.menu_syntax_capture_form_owns_input()")
            && !object_selector.contains("close_menu_syntax_object_selector_main_list(cx)")
            && object_selector
                .contains("self.menu_syntax_object_selector_state = Default::default();")
            && object_selector.contains("run_menu_syntax_object_selector_state_machine"),
        "handler form @ autocomplete must suppress the global object selector state machine"
    );
}

#[test]
fn handler_form_control_keys_preserve_standard_form_navigation() {
    let render = read("src/render_script_list/mod.rs");
    let control_key = render
        .find("handle_menu_syntax_form_control_key_input")
        .expect("physical key path must call form control-key handler");
    let tab_key = render
        .find("sk_is_key_tab(key_str)")
        .expect("physical key path must still route Tab as form navigation");
    assert!(
        control_key < tab_key,
        "suggestion Enter/Arrow handling should run before Tab navigation, while Tab stays traversal"
    );

    let app = read("src/app_impl/menu_syntax_main_hint.rs");
    let control_start = app
        .find("pub(crate) fn handle_menu_syntax_form_control_key_input(")
        .expect("control key handler must exist");
    let key_input_start = app
        .find("pub(crate) fn handle_menu_syntax_form_key_input(")
        .expect("printable key handler must still exist");
    let control_body = &app[control_start..key_input_start];
    assert!(
        control_body.contains("is_menu_syntax_form_tab_key")
            && control_body.contains("focus_previous_menu_syntax_form_field")
            && control_body.contains("focus_next_menu_syntax_form_field")
            && control_body.contains("\"up\" | \"arrowup\"")
            && control_body.contains("\"down\" | \"arrowdown\"")
            && control_body.contains("\"enter\" | \"return\"")
            && control_body.contains("\"escape\" | \"esc\""),
        "Tab/Shift+Tab must be consumed as active-field form navigation while Arrow/Enter/Escape keep controlling form suggestions"
    );
    let tab_pos = control_body
        .find("is_menu_syntax_form_tab_key")
        .expect("Tab control check must exist");
    let snapshot_pos = control_body
        .find("menu_syntax_main_hint_snapshot")
        .expect("suggestion snapshot lookup must still exist");
    assert!(
        tab_pos < snapshot_pos,
        "Tab must be consumed before suggestion snapshot lookup so it cannot fall through as printable text"
    );

    let move_start = app
        .find("fn move_menu_syntax_form_focus(")
        .expect("focus move helper must exist");
    let key_body = &app[key_input_start..move_start];
    assert!(
        key_body.contains("handle_menu_syntax_form_control_key_input(")
            && key_body.contains("key_char")
            && key_body.contains(".or_else(||")
            && key_body.contains("is_control"),
        "handler form text input must route control keys first, consume printable key fallbacks without key_char, and reject literal control characters like tab"
    );

    // Focus and validation semantics are proved through the runtime
    // `getElements` receipt; avoid pinning collector implementation strings.

    let state = read("src/main_sections/app_state.rs");
    assert!(
        state.contains("menu_syntax_form_suggestion_field_id")
            && state.contains("menu_syntax_form_suggestion_selected_index"),
        "selected form suggestion state must survive snapshot/render/control-key round trips"
    );
}

#[test]
fn handler_form_focus_reveal_has_scroll_and_layout_contract() {
    let app = read("src/app_impl/menu_syntax_main_hint.rs");
    assert!(
        app.contains("fn reveal_menu_syntax_form_field_at(")
            && app.contains("menu_syntax_main_hint_scroll_handle")
            && app.contains("set_offset")
            && app.contains("focus_menu_syntax_form_input_at")
            && app.contains("reveal_menu_syntax_form_field_at(index, form, cx)"),
        "Tab focus movement must reveal the active handler form field in the main hint scroll container"
    );

    let layout = read("src/app_layout/build_layout_info.rs");
    assert!(
        layout.contains("handlerFormFocusedVisibility")
            && layout.contains("handlerFormFieldBounds")
            && layout.contains("scrollOffsetY")
            && layout.contains("scrollContainerId")
            && read("src/protocol/types/grid_layout.rs").contains("handler_form"),
        "getLayoutInfo must expose handler form field bounds and focused-field visibility for DevTools proof"
    );
}

#[test]
fn live_input_field_updates_do_not_reset_existing_focus_selection() {
    let app = read("src/app_impl/menu_syntax_main_hint.rs");
    let update_start = app
        .find("pub(crate) fn update_menu_syntax_form_field(")
        .expect("form field update method must exist");
    let tab_key_start = app[update_start..]
        .find("fn is_menu_syntax_form_tab_key(")
        .map(|offset| update_start + offset)
        .expect("tab key helper must follow form update method");
    let update_body = &app[update_start..tab_key_start];

    assert!(
        update_body.contains("let sync_from_focused_live_input = self.menu_syntax_form_syncing_from_input")
            && update_body.contains("menu_syntax_form_input_active")
            && update_body.contains("menu_syntax_form_draft_field_id")
            && update_body.contains("id == resolved_field_id"),
        "live InputEvent::Change updates must identify the already-focused form field before syncing serialized text"
    );
    assert!(
        update_body.contains("if !sync_from_focused_live_input {\n                    self.focus_menu_syntax_form_input_at(index, &form, window, cx);\n                }"),
        "updates coming from the already-focused live input must not re-run focus_menu_syntax_form_input_at and reset selection/scroll"
    );
    assert!(
        update_body.contains("self.focus_menu_syntax_form_input_preserving_selection(\n                        index, &form, window, cx,\n                    );"),
        "updates coming from the already-focused live input must restore field focus without resetting the cursor"
    );
    assert!(
        update_body.contains("self.defer_menu_syntax_form_input_focus_restore(\n                        index,\n                        resolved_field_id.clone(),\n                        window,\n                        cx,\n                    );"),
        "focused live-input updates must defer a second focus restore so Shift+Enter cannot fall back to the main input after serialized text sync"
    );
    assert!(
        update_body.contains("self.ensure_menu_syntax_form_inputs(&form, window, cx);")
            && update_body.contains("self.menu_syntax_form_draft_field_id = Some(resolved_field_id.clone());")
            && update_body.contains("self.sync_menu_syntax_form_trigger_picker_state(&form, window, cx);"),
        "skipping live-input refocus must still sync form inputs, draft state, and autocomplete popup state"
    );
}

#[test]
fn priority_field_uses_sidebar_autocomplete_not_inline_choice_chips() {
    let form = read("src/menu_syntax/form.rs");
    assert!(
        form.contains("MenuSyntaxFormFieldKind::Priority")
            && form.contains("fn priority_suggestions(")
            && !form.contains(".filter(|value| text_matches_query(value, &raw_query))"),
        "priority suggestions must expose the schema choice set instead of filtering the active value"
    );

    let render = read("src/render_script_list/mod.rs");
    assert!(
        !render.contains("fn render_menu_syntax_form_choice_field(")
            && !render.contains("handler-form-choice-option")
            && !render.contains("fn render_menu_syntax_form_suggestion_popup(")
            && !render.contains("handler-form-autocomplete-popup")
            && read("src/app_impl/menu_syntax_main_hint.rs")
                .contains("build_menu_syntax_form_trigger_picker_snapshot"),
        "priority must use the shared Trigger Popup surface instead of inline choice chips or a bespoke sidebar"
    );

    let elements = read("src/app_layout/collect_elements.rs");
    assert!(
        !elements.contains("handlerFormChoiceField")
        && !elements.contains("handlerFormChoiceOption")
        && !elements.contains("handlerFormAutocompleteOption")
        && !elements.contains("\"radiogroup\"")
        && elements.contains("MenuSyntaxFormFieldKind::Priority")
        && elements.contains("handlerFormAutocompleteField")
        && elements.contains("\"combobox\""),
        "getElements must expose priority as a combobox while picker choices come from the shared Trigger Popup target"
    );
}

#[test]
fn handler_form_uses_projected_invocation_for_natural_dates_and_main_input_tokens() {
    let app = read("src/app_impl/menu_syntax_main_hint.rs");
    assert!(
        app.contains("fn menu_syntax_capture_form_invocation_for_form(")
            && app.contains("project_menu_syntax_main_input_completion")
            && app.contains("resolve_menu_syntax_form_invocation_dates")
            && app.contains("builtin_capture_accepts_for_target")
            && app.contains("crate::menu_syntax::payload::DatePhrase"),
        "handler form snapshots must project main-input autocomplete tokens and natural dates before building fields"
    );
    assert!(
        app.contains("let invocation_for_form =")
            && app.contains("self.menu_syntax_capture_form_invocation_for_form(raw_filter_text)")
            && app.contains("self.menu_syntax_capture_form_invocation_for_form(&self.filter_text)"),
        "snapshot and form-field edits must share the projected invocation path"
    );
    assert!(
        app.contains("token.to_ascii_lowercase().as_str()")
            && app.contains("(\"tags\", \"#\")")
            && app.contains("(\"priority\", \"p\")"),
        "active main-input completion projection must only strip bare tag/priority fragments from the body"
    );
}

#[test]
fn autocomplete_renders_owned_popup_list_and_escape_dismisses_first() {
    let app = read("src/app_impl/menu_syntax_main_hint.rs");
    assert!(
        app.contains("fn menu_syntax_form_field_uses_suggestions(")
            && app.contains("fn open_menu_syntax_form_suggestions_for(")
            && app.contains("fn close_menu_syntax_form_suggestions(")
            && app.contains("menu_syntax_form_suggestion_field_id")
            && app.contains("close_menu_syntax_form_suggestions_and_trigger_picker(cx);")
            && app.contains("focus_menu_syntax_main_input(window, cx)"),
        "autocomplete popup ownership must be explicit and Escape must dismiss it before returning to the main input"
    );

    let render = read("src/render_script_list/mod.rs");
    assert!(
        !render.contains("fn render_menu_syntax_form_suggestion_popup(")
            && !render.contains("handler-form-autocomplete-popup")
            && !render.contains("handler-form-autocomplete-option")
            && !render.contains(".when_some(popup_field"),
        "tags/object/priority autocomplete must not render a bespoke inline/sidebar list"
    );

    let elements = read("src/app_layout/collect_elements.rs");
    assert!(
        elements.contains("handlerFormAutocompleteField")
            && !elements.contains("handlerFormAutocompleteOption")
            && elements.contains("\"combobox\""),
        "getElements must expose form combobox fields while picker options are exposed by the shared Trigger Popup automation target"
    );

    let layout = read("src/app_layout/build_layout_info.rs");
    assert!(
        layout.contains("\"surface\": \"menuSyntaxTriggerPicker\""),
        "layout receipts must identify form autocomplete as the shared Trigger Popup surface"
    );
}
