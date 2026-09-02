const ARG_HELPERS: &str = include_str!("../src/render_prompts/arg/helpers.rs");
const MINI_RENDER: &str = include_str!("../src/render_prompts/mini.rs");
const STARTUP: &str = include_str!("../src/app_impl/startup.rs");
const STARTUP_NEW_PRELUDE: &str = include_str!("../src/app_impl/startup_new_prelude.rs");
const SELECTION_FALLBACK: &str = include_str!("../src/app_impl/selection_fallback.rs");
const SUBMIT_DIAGNOSTICS: &str = include_str!("../src/app_impl/submit_diagnostics.rs");
const FILTERING_CACHE: &str = include_str!("../src/app_impl/filtering_cache.rs");
const THEME_CHOOSER: &str = include_str!("../src/render_builtins/theme_chooser.rs");
const LIFECYCLE_RESET: &str = include_str!("../src/app_impl/lifecycle_reset.rs");
const PROTOCOL_QUERY_VARIANTS: &str = include_str!("../src/protocol/message/variants/query_ops.rs");
const PROMPT_HANDLER: &str = include_str!("../src/prompt_handler/mod.rs");
const DEVTOOLS_FOCUS: &str = include_str!("../scripts/devtools/focus.ts");
const DEVTOOLS_ACT: &str = include_str!("../scripts/devtools/act.ts");

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_idx = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source start: {start}"));
    let end_idx = source[start_idx..]
        .find(end)
        .unwrap_or_else(|| panic!("missing source end after {start}: {end}"));
    &source[start_idx..start_idx + end_idx]
}

fn source_function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing function signature: {signature}"));
    let after_signature = &source[start..];
    let open = after_signature
        .find('{')
        .unwrap_or_else(|| panic!("missing function body open: {signature}"));
    let body_start = start + open;
    let mut depth = 0usize;
    for (offset, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[body_start..=body_start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("missing function body close: {signature}");
}

fn source_window<'a>(source: &'a str, start: &str, len: usize) -> &'a str {
    let start_idx = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source start: {start}"));
    let end_idx = (start_idx + len).min(source.len());
    &source[start_idx..end_idx]
}

#[test]
fn brain_inbox_enter_stages_agent_chat_without_submit_or_resolve() {
    // Brain Inbox launcher rows are observations that require an explicit
    // user decision. Plain Enter may stage a follow-up draft, but must not
    // submit an Agent Chat turn or destructively resolve the inbox row.
    let execute_body = source_function_body(
        SELECTION_FALLBACK,
        "pub(crate) fn execute_root_brain_inbox_open",
    );
    assert!(
        execute_body.contains("brain_inbox_context_part(&item)")
            && execute_body.contains("open_agent_chat_with_staged_brain_inbox_prompt("),
        "Brain Inbox Enter should build context and delegate to the staging helper"
    );
    assert!(
        !execute_body.contains("resolve_root_brain_inbox_item")
            && !execute_body.contains("execute_root_note_open")
            && !execute_body.contains("brain_inbox_composer_draft")
            && !execute_body.contains("resume_agent_chat_conversation_with_followup"),
        "Brain Inbox Enter must not resolve, open notes directly, prefill the composer, or use the auto-submit follow-up route"
    );

    let staging_body = source_function_body(
        SELECTION_FALLBACK,
        "fn open_agent_chat_with_staged_brain_inbox_prompt",
    );
    assert!(
        staging_body.contains(
            "open_clean_agent_chat(cx)"
        ) && staging_body.contains("stage_brain_inbox_context_on_agent_chat(")
            && staging_body.contains("chat_view.resume_from_history(thread_id,"),
        "The Brain Inbox staging helper should open Agent Chat without entry intent, optionally resume chat history, and stage inbox context"
    );
    assert!(
        !staging_body.contains("submit_reused_entry_intent")
            && !staging_body.contains("submit_input")
            && !staging_body.contains("set_input(prompt")
            && !staging_body.contains("resolve_root_brain_inbox_item"),
        "The Brain Inbox staging helper must not auto-submit, put the full prompt in the composer, or resolve the inbox item"
    );
    assert!(
        staging_body.find("resume_from_history(thread_id,")
            < staging_body.find("stage_brain_inbox_context_on_agent_chat("),
        "Brain Inbox chat_turn handoff should resume history before staging the inbox context"
    );

    let context_part_body = source_function_body(SELECTION_FALLBACK, "fn brain_inbox_context_part");
    assert!(
        context_part_body.contains("AiContextPart::TextBlock")
            && context_part_body
                .contains("text: crate::brain::response_prompt_for_inbox_item(item)")
            && context_part_body.contains("mime_type: Some(\"text/markdown\".to_string())"),
        "The full Brain Inbox prompt should be staged as a markdown TextBlock context part"
    );

    let stage_context_body = source_function_body(
        SELECTION_FALLBACK,
        "fn stage_brain_inbox_context_on_agent_chat",
    );
    assert!(
        stage_context_body.contains("replace_pending_context_parts")
            && stage_context_body.contains("vec![context_part]")
            && stage_context_body.contains("brain_inbox_local_assistant_message(&context_part)")
            && stage_context_body.contains("push_local_assistant_message")
            && !stage_context_body.contains("set_input("),
        "Agent Chat staging should replace pending context and show local assistant guidance without preloading the composer"
    );
    assert!(
        !stage_context_body.contains("submit_input")
            && !stage_context_body.contains("submit_blocks")
            && !stage_context_body.contains("start_turn"),
        "Brain Inbox guidance must not start a provider turn"
    );

    let assistant_message_body =
        source_function_body(SELECTION_FALLBACK, "fn brain_inbox_local_assistant_message");
    assert!(
        assistant_message_body.contains("split_once(\"\\nTask:\\n\")")
            && assistant_message_body
                .contains("strip_prefix(\"Follow up on this Brain Inbox item.\\n\\n\")")
            && assistant_message_body.contains("What should we do with this?"),
        "Visible Brain Inbox assistant guidance should use clean intro copy and show the actual inbox context"
    );
}

#[test]
fn mini_prompt_enter_records_submit_owner_before_resetting_to_script_list() {
    let submit_fn = source_between(
        ARG_HELPERS,
        "fn submit_arg_prompt_from_current_state(",
        "\n    fn is_valid_builtin_mic_selection",
    );

    assert!(
        MINI_RENDER.contains("this.submit_arg_prompt_from_current_state(&prompt_id, cx);")
            && MINI_RENDER.contains("cx.stop_propagation();"),
        "MiniPrompt Enter must submit through the shared arg submit helper and stop key propagation"
    );
    assert!(
        submit_fn.contains("record_submit_diagnostic")
            && submit_fn.contains("BUILTIN_SNAP_MODE_PROMPT_ID")
            && submit_fn.contains("handle_builtin_snap_mode_selection(&value, cx);"),
        "built-in snap mode prompt submit must record ownership before returning to ScriptList"
    );
}

#[test]
fn builtin_submit_transitions_that_return_to_script_list_arm_enter_echo_guard() {
    let theme_submit = source_between(
        THEME_CHOOSER,
        "pub(crate) fn submit_theme_chooser_from_input_enter(",
        "\n    fn theme_chooser_match_summary",
    );
    assert!(
        SUBMIT_DIAGNOSTICS.contains("fn record_return_to_script_list_submit")
            && SUBMIT_DIAGNOSTICS
                .contains("self.record_submit_diagnostic(owner, route, None, value, true);")
            && theme_submit.contains("record_return_to_script_list_submit")
            && theme_submit.contains("\"theme_chooser\"")
            && theme_submit.contains("\"submit_theme_chooser_from_input_enter\"")
            && theme_submit.find("record_return_to_script_list_submit")
                < theme_submit.find("self.go_back_or_close(window, cx);"),
        "Theme Designer Apply must use the return-to-ScriptList submit helper before returning to the main menu"
    );

    let reset_positions = source_between(
        LIFECYCLE_RESET,
        "pub(crate) fn reset_window_positions_to_default_main_menu(",
        "\n    pub(crate) fn cancel_script_execution",
    );
    assert!(
        reset_positions.contains("record_return_to_script_list_submit")
            && reset_positions.contains("\"reset_window_positions_to_default_main_menu\"")
            && reset_positions.find("record_return_to_script_list_submit")
                < reset_positions.find("self.reset_to_script_list(cx);"),
        "Reset Window Positions must use the return-to-ScriptList submit helper before resetting to the main menu"
    );
}

#[test]
fn snap_mode_submit_closes_main_window_through_named_transition() {
    let transition = source_between(
        ARG_HELPERS,
        "enum BuiltinPromptSubmitTransition",
        "\n#[derive(Clone, Copy)]\nstruct PromptRenderContext",
    );
    assert!(
        transition.contains("ReturnToScriptList")
            && transition.contains("CloseMainWindow")
            && transition.contains("app.close_and_reset_window(cx);"),
        "built-in prompt submits must use explicit lifecycle transitions"
    );

    let snap_handler = source_between(
        ARG_HELPERS,
        "fn handle_builtin_snap_mode_selection(",
        "\n    #[inline]\n    fn apply_arg_tab_completion",
    );
    assert!(
        snap_handler.contains("BuiltinPromptSubmitTransition::CloseMainWindow.apply(self, cx);"),
        "snap mode final submit must close the main window instead of returning visibly to ScriptList"
    );
    assert!(
        !snap_handler.contains("reset_to_script_list(cx)"),
        "snap mode final submit must not directly reset to ScriptList"
    );
}

#[test]
fn script_list_enter_refuses_immediate_prompt_submit_echo() {
    for (name, source) in [
        ("startup.rs", STARTUP),
        ("startup_new_prelude.rs", STARTUP_NEW_PRELUDE),
    ] {
        let press_enter = source_window(source, "InputEvent::PressEnter", 4200);
        assert!(
            press_enter.contains("submit_arg_prompt_from_current_state(&prompt_id, cx);"),
            "{name} must route Arg/MiniPrompt Enter to the shared prompt submit helper"
        );
        assert!(
            press_enter.contains("should_consume_script_list_enter_after_submit"),
            "{name} must check the submit ownership guard before ScriptList execution"
        );
        assert!(
            press_enter.find("should_consume_script_list_enter_after_submit")
                < press_enter.find("this.execute_selected(cx)"),
            "{name} must run the submit ownership guard before execute_selected"
        );
    }

    assert!(
        SUBMIT_DIAGNOSTICS.contains("script_list_enter_echo_consumed")
            && SUBMIT_DIAGNOSTICS.contains("ENTER_ECHO_GUARD_MS")
            && SUBMIT_DIAGNOSTICS.contains("pendingEnterGuardActive"),
        "submit diagnostics must log consumed ScriptList Enter echoes and expose receipts"
    );
    let execute_selected =
        source_window(SELECTION_FALLBACK, "pub(crate) fn execute_selected", 1400);
    assert!(
        execute_selected
            .contains("should_consume_script_list_enter_after_submit(\"execute_selected\")")
            && execute_selected
                .contains("record_submit_diagnostic(\n                \"launcher\",")
            && execute_selected.find("should_consume_script_list_enter_after_submit")
                < execute_selected.find("execute_menu_syntax_command_invocation"),
        "execute_selected must be guarded and recorded before any ScriptList execution sink"
    );
    let execute_selected_fallback =
        source_window(SELECTION_FALLBACK, "pub fn execute_selected_fallback", 1800);
    assert!(
        execute_selected_fallback.contains(
            "should_consume_script_list_enter_after_submit(\"execute_selected_fallback\")"
        ) && execute_selected_fallback
            .contains("record_submit_diagnostic(\n                \"launcher_fallback\","),
        "execute_selected_fallback must be guarded and recorded before fallback execution"
    );
}

#[test]
fn script_list_submit_requires_live_filter_and_grouped_cache_domain() {
    let helper = source_window(
        SELECTION_FALLBACK,
        "fn live_script_list_flat_selection_for_submit",
        2600,
    );
    assert!(
        helper.contains("matches!(self.current_view, AppView::ScriptList)"),
        "ScriptList submit guard must only resolve main-list rows on ScriptList"
    );
    assert!(
        helper.contains("self.filter_text != self.computed_filter_text")
            && helper.find("self.filter_text != self.computed_filter_text")
                < helper.find("self.get_grouped_results_cached()"),
        "ScriptList submit must block while live filter_text and computed_filter_text disagree"
    );
    assert!(
        helper.contains("has_grouped_results_for_filter_text(&self.computed_filter_text)"),
        "ScriptList submit must verify grouped cache ownership for the current filter before resolving selected_index"
    );
    assert!(
        FILTERING_CACHE.contains("source-filters={grouped_source_filter_key}")
            && FILTERING_CACHE.contains("browser-tabs-gen={browser_tabs_generation}")
            && FILTERING_CACHE.contains("browser-history-gen={browser_history_generation}"),
        "Grouped cache domains may include source/current-app/generation fields, so submit guards must not require exact filter-only keys"
    );
    assert!(
        helper.contains("flat_result_index_for_coerced_grouped_selection(self.selected_index)"),
        "ScriptList submit must resolve through the grouped visible-row cache"
    );
    assert!(
        helper.contains("main_list_submit_blocked.no_live_selected_row"),
        "No visible/live selected row must produce an explicit submit diagnostic"
    );
}

#[test]
fn execute_selected_uses_live_selection_guard_before_row_execution() {
    let execute_selected =
        source_window(SELECTION_FALLBACK, "pub(crate) fn execute_selected", 14000);
    assert!(
        execute_selected.contains("live_script_list_flat_selection_for_submit()"),
        "execute_selected must route visible-row submit through the live selection guard"
    );
    assert!(
        execute_selected.find("live_script_list_flat_selection_for_submit()")
            < execute_selected.find("inline_calculator_for_result_index"),
        "live selection guard must run before calculator or row execution"
    );
    let guard_pos = execute_selected
        .find("live_script_list_flat_selection_for_submit()")
        .expect("execute_selected must call live selection guard");
    let after_guard = &execute_selected[guard_pos..];
    assert!(
        after_guard.find("live_script_list_flat_selection_for_submit()")
            < after_guard.find("mark_opened_from_main_menu"),
        "live selection guard must run before any launcher row dispatch"
    );
}

#[test]
fn selected_main_list_search_result_owned_uses_live_selection_guard() {
    let selected_owned = source_window(
        SELECTION_FALLBACK,
        "pub(crate) fn selected_main_list_search_result_owned",
        1400,
    );
    assert!(
        selected_owned.contains("live_script_list_flat_selection_for_submit()"),
        "all owned ScriptList result lookups must share the live selection guard"
    );
}

#[test]
fn fallback_enter_cannot_execute_legacy_cached_fallback_without_grouped_row() {
    let fallback_submit =
        source_window(SELECTION_FALLBACK, "pub fn execute_selected_fallback", 2200);
    assert!(
        fallback_submit.contains("scripts::SearchResult::Fallback"),
        "fallback Enter must execute only a live grouped fallback row"
    );
    assert!(
        !fallback_submit.contains("main_menu_fallback_state.selected_item().cloned()")
            && !fallback_submit.contains("execute_selected_fallback.legacy"),
        "fallback Enter must not execute legacy cached fallback state outside the visible grouped row domain"
    );
}

#[test]
fn get_state_exposes_submit_diagnostics_receipt() {
    assert!(
        PROTOCOL_QUERY_VARIANTS.contains("submit_diagnostics: Option<serde_json::Value>")
            && PROTOCOL_QUERY_VARIANTS.contains("rename = \"submitDiagnostics\""),
        "StateResult must include submitDiagnostics for state-first submit proof"
    );
    assert!(
        PROMPT_HANDLER.contains("self.submit_diagnostics_snapshot()"),
        "main-window getState must forward submit diagnostics"
    );
    assert!(
        PROMPT_HANDLER.contains("record_submit_diagnostic(\n                    \"protocol\",\n                    \"submit_current_value\""),
        "protocol-driven submit_current_value must record submit diagnostics"
    );
    assert!(
        PROMPT_HANDLER.contains("record_submit_diagnostic(\n                                            \"protocol\",\n                                            \"forceSubmit\""),
        "protocol forceSubmit must record submit diagnostics"
    );
    assert!(
        DEVTOOLS_FOCUS.contains("submitDiagnostics: state.submitDiagnostics ?? null")
            && DEVTOOLS_ACT.contains("focus.inspect.submitDiagnostics")
            && DEVTOOLS_ACT.contains("isPromptEntityTargetReceipt")
            && DEVTOOLS_ACT.contains("sourceAfter.submitDiagnostics")
            && DEVTOOLS_ACT.contains("submitDiagnostics: {\n      before: before.submitDiagnostics ?? null,\n      after: after.submitDiagnostics ?? null,"),
        "DevTools act/focus receipts must expose before/after submit diagnostics"
    );
}

/// WHY: an empty `@source:` colon mode (`@clipboard:` showing recents) must
/// never let a reflexive double-Enter attach a recent item the user never
/// chose. The old mechanism was a selectable "Type to search" guard row; the
/// current one renders recents UNARMED (no selected row, hint in ghost text +
/// section header) until an explicit Down/click. This locks the unarmed
/// mechanism: no fake guard row may return, Enter must be consumed while
/// suppressed, and Down must be the arming gesture.
#[test]
fn empty_spine_subsearch_prefixes_are_not_submit_armed() {
    const AGENT_HANDOFF: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");
    const IMPL_MOVEMENT: &str = include_str!("../src/app_navigation/impl_movement.rs");

    let rich_classifier = source_window(FILTERING_CACHE, "fn active_rich_spine_subsearch(", 900);
    assert!(
        rich_classifier.contains("query.trim().to_string()"),
        "empty @file:/@clipboard:/@history: prefixes must keep using the rich path so unarmed recents render"
    );

    // The hint affordance must never be a selectable list row again: no
    // guard-row id and no consuming placeholder action anywhere in the
    // grouped-results builder.
    assert!(
        !FILTERING_CACHE.contains("empty-guard")
            && !FILTERING_CACHE.contains("AwaitContextSubsearchInput"),
        "the empty colon-mode hint must ride ghost text + the section header, not a selectable first row"
    );

    let spine_enter = source_window(AGENT_HANDOFF, "fn try_handle_spine_enter(", 1400);
    assert!(
        spine_enter.contains("spine_empty_subsearch_selection_suppressed()")
            && spine_enter.contains("empty_context_subsearch_enter_consumed"),
        "Enter in an unarmed empty colon mode must be consumed before any row acceptance or plan submission"
    );

    let down = source_window(IMPL_MOVEMENT, "fn move_selection_down(", 1200);
    assert!(
        down.contains("spine_empty_subsearch_selection_suppressed()")
            && down.contains("arm_spine_empty_subsearch_selection()"),
        "Down must be the explicit arming gesture for the unarmed empty colon mode"
    );
}
