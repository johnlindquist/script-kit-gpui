//! Source-level contract test for the `main-menu-cmd-enter-ai` user story
//! (Run 2 Pass #24) updated for unified simulateKey dispatcher.

const CANONICAL_SIMULATEKEY: &str = include_str!("../src/app_impl/simulate_key_dispatch.rs");
const TAB_AI_MODE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");
const STARTUP_SOURCE: &str = include_str!("../src/app_impl/startup.rs");
const APP_RUN_SETUP: &str = include_str!("../src/main_entry/app_run_setup.rs");
const RUNTIME_MATCH_SIMULATEKEY: &str =
    include_str!("../src/main_entry/runtime_stdin_match_simulate_key.rs");

fn fn_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source.find(signature).expect("signature must exist");
    let rest = &source[start..];
    let open = rest.find('{').expect("function body must start");
    let mut depth = 0usize;

    for (idx, ch) in rest[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &rest[..open + idx + 1];
                }
            }
            _ => {}
        }
    }

    panic!("function body must close");
}

fn assert_before(source: &str, first: &str, second: &str) {
    let first_pos = source.find(first).expect("missing first marker");
    let second_pos = source.find(second).expect("missing second marker");
    assert!(
        first_pos < second_pos,
        "expected `{first}` before `{second}`"
    );
}

#[test]
fn simulate_key_dispatchers_route_cmd_enter_in_scriptlist_to_agent_chat_context_capture() {
    // 1. Verify the canonical simulateKey dispatcher contains the correct Cmd+Enter routing inside AppView::ScriptList.
    let scriptlist_anchor = "AppView::ScriptList => {";
    let Some(anchor_idx) = CANONICAL_SIMULATEKEY.find(scriptlist_anchor) else {
        panic!(
            "src/app_impl/simulate_key_dispatch.rs is missing the `AppView::ScriptList` anchor. \
             Layout has shifted; update this test."
        );
    };

    let arm_body: String = CANONICAL_SIMULATEKEY[anchor_idx..]
        .lines()
        .take(90)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        arm_body.contains("has_cmd")
            && arm_body.contains("key_lower == \"enter\"")
            && arm_body.contains("!has_shift")
            && arm_body.contains("!_has_alt")
            && arm_body.contains("!_has_ctrl"),
        "simulate_key_dispatch.rs `AppView::ScriptList` must gate the Cmd+Enter routing on modifiers"
    );

    assert!(
        arm_body.contains("view.try_route_global_cmd_enter_to_agent_chat_context_capture(ctx);"),
        "simulate_key_dispatch.rs `AppView::ScriptList` must call try_route_global_cmd_enter_to_agent_chat_context_capture"
    );

    // 2. Verify that the entry points delegate to the unified helper.
    for (name, source) in [
        ("src/main_entry/app_run_setup.rs", APP_RUN_SETUP),
        (
            "src/main_entry/runtime_stdin_match_simulate_key.rs",
            RUNTIME_MATCH_SIMULATEKEY,
        ),
    ] {
        assert!(
            source.contains("dispatch_simulate_key"),
            "{name} must delegate to dispatch_simulate_key helper"
        );
    }
}

#[test]
fn simulate_key_cmd_enter_arm_precedes_plain_enter_in_scriptlist() {
    // Ordering contract: the Cmd+Enter arm MUST appear before the plain-enter arm.
    let cmd_enter_idx = CANONICAL_SIMULATEKEY
        .find("SimulateKey: Cmd+Enter - route to Agent Chat context capture")
        .expect("missing Cmd+Enter log line in simulate_key_dispatch.rs");

    let plain_enter_idx = CANONICAL_SIMULATEKEY
        .find("SimulateKey: Enter - execute selected")
        .expect("missing plain-Enter log line in simulate_key_dispatch.rs");

    assert!(
        cmd_enter_idx < plain_enter_idx,
        "Cmd+Enter arm must appear BEFORE the plain-Enter arm inside the ScriptList match block"
    );
}

#[test]
fn try_route_global_cmd_enter_to_agent_chat_context_capture_still_defined() {
    assert!(
        TAB_AI_MODE
            .contains("pub(crate) fn try_route_global_cmd_enter_to_agent_chat_context_capture("),
        "try_route_global_cmd_enter_to_agent_chat_context_capture must still be defined"
    );
}

#[test]
fn script_list_shift_tab_runtime_and_simulate_key_share_profile_search_helper() {
    assert!(
        CANONICAL_SIMULATEKEY.contains("try_open_profile_search_from_script_list_shift_tab"),
        "simulateKey ScriptList Shift+Tab must use the shared Profile Search helper"
    );
    assert!(
        STARTUP_SOURCE.contains("try_open_profile_search_from_script_list_shift_tab"),
        "live ScriptList Shift+Tab must use the same Profile Search helper as simulateKey"
    );
    assert!(
        !STARTUP_SOURCE.contains("submit_to_current_or_new_tab_ai_harness_from_text"),
        "Shift+Tab must not reintroduce hidden quick-submit routing"
    );
}

#[test]
fn script_list_cmd_enter_spine_attempt_is_explicit_and_non_terminal() {
    let body = fn_body(
        TAB_AI_MODE,
        "pub(crate) fn try_route_global_cmd_enter_to_agent_chat_context_capture(",
    );

    assert!(
        !body.contains("return self.try_submit_spine_prompt_plan_from_enter(cx);"),
        "ScriptList Cmd+Enter must not let Spine return early for every non-empty filter"
    );
    assert!(
        body.contains("script_list_cmd_enter_has_explicit_spine_prompt_plan"),
        "ScriptList Cmd+Enter must gate Spine submission behind an explicit prompt-plan predicate"
    );
    assert!(
        body.contains("if self.try_submit_spine_prompt_plan_from_enter(cx)")
            && body.contains("return true;"),
        "Spine should consume Cmd+Enter only after a successful explicit prompt-plan submission"
    );
    assert_before(
        body,
        "script_list_cmd_enter_has_explicit_spine_prompt_plan",
        "self.try_route_cmd_enter_to_menu_syntax_ai(cx)",
    );
}

#[test]
fn script_list_cmd_enter_plain_prompt_uses_agent_chat_entry_intent() {
    let body = fn_body(
        TAB_AI_MODE,
        "pub(crate) fn try_route_global_cmd_enter_to_agent_chat_context_capture(",
    );

    assert!(
        body.contains("script_list_cmd_enter_plain_prompt_intent"),
        "plain natural ScriptList input should have a distinct Cmd+Enter prompt route"
    );
    assert!(
        body.contains("ask_agent_chat_suppressing_focused_part(intent, cx)"),
        "plain natural ScriptList input should seed Agent Chat as an entry intent so it can stream"
    );
}

#[test]
fn script_list_cmd_enter_plain_prompt_does_not_block_on_selected_rows() {
    let body = fn_body(TAB_AI_MODE, "fn script_list_cmd_enter_plain_prompt_intent(");

    assert!(
        !body.contains("search_result_for_grouped_item(self.selected_index)"),
        "plain prompt routing must not use selected grouped rows to override typed prose"
    );
    assert!(
        body.contains("Some(intent.to_string())"),
        "non-empty ScriptList prose must become the Agent Chat entry intent"
    );
}

#[test]
fn script_list_cmd_enter_spine_explicit_predicate_requires_prompt_builder_syntax() {
    let body = fn_body(
        TAB_AI_MODE,
        "fn script_list_cmd_enter_has_explicit_spine_prompt_plan(",
    );

    assert!(
        body.contains("parse_has_prompt_builder_segments(&self.spine_parse)"),
        "explicit prompt-builder syntax must still authorize the Spine prompt-plan path"
    );
    assert!(
        !body.contains("self.spine_cwd.is_some()"),
        "a cwd chip alone must not make plain launcher prose take the destructive Spine submit path"
    );
}
