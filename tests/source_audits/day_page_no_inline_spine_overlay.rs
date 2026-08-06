//! Day/Today must not render its own inline Spine overlay.
//!
//! `@` context in Day round-trips through the main menu/shared context surface;
//! the Day editor itself must not expose a Day-local prompt-builder or Agent
//! handoff surface.

use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn function_body<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(name)
        .unwrap_or_else(|| panic!("missing function marker: {name}"));
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing function body for: {name}"));
    let mut depth = 0usize;
    for (offset, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open..=open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated function body for: {name}");
}

#[test]
fn day_page_render_does_not_mount_inline_spine_overlay() {
    let view = source("src/main_sections/day_page_view.rs");
    let render = function_body(&view, "fn render(");

    assert!(
        !render.contains("spine"),
        "Day render must stay editor-only; Spine is direct handoff logic, not a mounted Day overlay"
    );
    // The inline day-switcher overlay was deleted (see "delete the dead
    // inline day-switcher machinery"), so the editor is the only mounted
    // surface this audit still requires.
    assert!(
        render.contains("editor_input"),
        "Day render should compose the shared editor"
    );
}

#[test]
fn day_page_keyboard_does_not_drive_spine_rows() {
    let view = source("src/main_sections/day_page_view.rs");
    let types = source("src/main_sections/day_page_types.rs");
    let handle_key = function_body(&view, "pub(crate) fn handle_key_parts(");

    for forbidden in [
        "SpineList",
        "selected_index",
        "hovered_index",
        "accept_",
        "move_",
    ] {
        assert!(
            !handle_key.contains(forbidden),
            "Day key handling must not drive inline Spine row/navigation state: {forbidden}"
        );
    }
    assert!(
        !handle_key.contains("submit_day_page_spine_prompt_from_current_line"),
        "Day Cmd+Enter must not open Agent Chat/prompt-builder surfaces"
    );
    let deprecated_submit_anchor = concat!("cwd_", "submit_", "anchor");
    assert!(
        !types.contains(deprecated_submit_anchor),
        "Day spine state must not keep submit anchors for deprecated prompt-builder handoff"
    );
}

#[test]
fn day_page_spine_adapter_exposes_no_overlay_rows() {
    let day_spine = source("src/main_sections/day_page_spine.rs");

    for forbidden in [
        "impl Render",
        "IntoElement",
        "GroupedListItem",
        "SpineListRow",
        "selected_index",
        "hovered_index",
        "MouseButton",
        "on_mouse_down",
    ] {
        assert!(
            !day_spine.contains(forbidden),
            "Day spine helper must not expose inline overlay/list machinery: {forbidden}"
        );
    }
}

#[test]
fn day_page_cannot_delegate_to_main_list_spine_projection() {
    let filter_input_core = source("src/app_impl/filter_input_core.rs");
    let owns_main_list = function_body(&filter_input_core, "fn spine_projection_owns_main_list(");

    assert!(
        owns_main_list.contains("AppView::DayPage { .. }")
            && owns_main_list.contains("return false"),
        "Day must not let the shared main-list Spine projection render prompt-builder rows"
    );
}

#[test]
fn free_text_spine_projection_does_not_own_a_list() {
    let list = source("src/spine/list.rs");
    let input_projection = source("src/spine/input_projection.rs");

    assert!(
        list.contains("SpineSegmentKind::FreeText => Vec::new()"),
        "Shared Spine list must not build rows for free-text tails"
    );

    let owns_list = function_body(&input_projection, "fn projection_owns_prompt_builder_list(");
    assert!(
        owns_list.contains("SpineSegmentKind::FreeText") && owns_list.contains("return false"),
        "Shared Spine ownership must reject free-text projections explicitly"
    );
}

#[test]
fn day_page_context_round_trip_still_uses_main_menu() {
    let round_trip = source("src/main_sections/day_page_context_round_trip.rs");

    for required in [
        "reset_to_script_list",
        "set_filter_text_immediate(segment_text, window, cx)",
        "request_script_list_main_filter_focus",
    ] {
        assert!(
            round_trip.contains(required),
            "Day @ context must keep main-menu round trip path: {required}"
        );
    }
}

#[test]
fn day_page_header_context_chips_are_inert() {
    let view = source("src/main_sections/day_page_view.rs");
    let render = function_body(&view, "fn render(");

    assert!(
        render.contains("render_inert_main_view_context_zone"),
        "Day header context labels should display state without clickable CWD/Agent chip exits"
    );
    assert!(
        !render.contains("render_clickable_main_view_context_zone"),
        "Day must not mount clickable header chips that can leave Day for prompt-builder surfaces"
    );
}

#[test]
fn day_page_actions_do_not_offer_agent_handoff() {
    let actions = source("src/main_sections/day_page_actions.rs");
    let agent_handoff = source("src/app_impl/agent_handoff/mod.rs");
    let actions_dialog = source("src/app_impl/actions_dialog.rs");
    let ai_mod = source("src/ai/mod.rs");

    for forbidden in [
        concat!("day_page:", "handoff_line"),
        concat!("Send Line", " to Agent Chat"),
        concat!("handoff_current_", "line_to_agent_chat"),
        concat!("day_page_", "handoff_plain_line"),
    ] {
        assert!(
            !actions.contains(forbidden),
            "Day actions must not expose Agent handoff path: {forbidden}"
        );
    }

    for forbidden in [
        concat!("submit_", "day_page_", "spine_prompt_plan_", "with_aliases"),
        concat!("submit_", "day_page_", "markdown_line_", "with_context"),
        concat!("day_page_", "markdown_reference_", "handoff"),
        concat!("day_page_", "line_", "handoff"),
    ] {
        assert!(
            !agent_handoff.contains(forbidden),
            "Agent handoff must not retain deprecated Day prompt-builder handoff path: {forbidden}"
        );
    }

    // Generic prompt export/target handoff rows are owned by the Agent Chat
    // composer (`get_agent_chat_actions`). `get_global_actions` must not
    // re-introduce them, or every host (Day Page included) would leak
    // prompt-builder rows that act on state the focused item does not own.
    let script_context = source("src/actions/builders/script_context.rs");
    let global_actions_body = function_body(&script_context, "pub fn get_global_actions(");
    assert!(
        !global_actions_body.contains("get_prompt_export_actions")
            && !global_actions_body.contains("get_prompt_target_actions")
            && !global_actions_body.contains("prompt-action/")
            && !global_actions_body.contains("prompt-target/"),
        "get_global_actions must not include generic prompt export/target handoff rows"
    );

    let routing = function_body(&actions_dialog, "fn route_key_to_actions_dialog(");
    assert!(
        routing.contains("tab_ai_actions_dialog_cmd_enter_ignored_day_page")
            && routing.contains("matches!(self.current_view, AppView::DayPage { .. })"),
        "Day-hosted Actions Cmd+Enter must not route into Agent Chat/prompt-builder target handoff"
    );

    let execute_action = function_body(&actions_dialog, "fn execute_action_for_actions_host(");
    assert!(
        execute_action.contains("AppView::DayPage { .. }")
            && execute_action.contains("is_prompt_action_id(&action_id)")
            && execute_action.contains("day_page_prompt_action_blocked"),
        "Day-hosted Actions execution must block stale prompt-action/prompt-target ids, not only hide rows"
    );

    let close_actions = function_body(&actions_dialog, "fn close_actions_popup(");
    assert!(
        close_actions.contains("take_pending_explicit_agent_chat_target()")
            && close_actions.contains("AppView::DayPage { .. }")
            && close_actions.contains("day_page_pending_agent_chat_target_dropped"),
        "closing Actions from Day must drop pending explicit Agent Chat targets instead of opening prompt-builder handoff UI"
    );

    let global_cmd_enter = function_body(&agent_handoff, "fn supports_global_cmd_enter_ai_entry(");
    assert!(
        !global_cmd_enter.contains("AppView::DayPage"),
        "global Cmd+Enter AI entry must continue to exclude Day Page"
    );

    assert!(
        !ai_mod.contains(concat!("inline", "_agent"))
            && !std::path::Path::new(concat!("src/ai/", "inline", "_agent")).exists(),
        "deleted inline assistant module/directory must not return"
    );
}

#[test]
fn day_page_footer_cannot_open_generic_agent_chat_popup() {
    let view = source("src/main_sections/day_page_view.rs");
    let footer = function_body(&view, "pub(crate) fn day_page_footer_buttons(");

    assert!(
        footer.contains("day_page_footer_buttons_for_preview"),
        "the ordinary Day footer must delegate preview-specific buttons to the preview owner"
    );
    assert!(
        !footer.contains("FooterAction::Ai"),
        "the ordinary Day footer must not expose a generic empty-context Agent button"
    );

    let ui_window = source("src/app_impl/ui_window.rs");
    let dispatcher = function_body(&ui_window, "fn dispatch_main_window_footer_action(");
    let ai_arm = dispatcher
        .split("crate::footer_popup::FooterAction::Ai =>")
        .nth(1)
        .and_then(|tail| {
            tail.split("crate::footer_popup::FooterAction::Stop =>")
                .next()
        })
        .expect("FooterAction::Ai arm should exist before Stop arm");
    assert!(
        ai_arm.contains("AppView::DayPage { .. }")
            && ai_arm.contains("main_window_footer_ai_ignored_day_page"),
        "stale Day footer AI events must be ignored before the generic Agent Chat open path"
    );
    assert!(
        ai_arm.contains("self.day_page_context_return.is_some()")
            && ai_arm.contains("main_window_footer_ai_ignored_day_page_context_return"),
        "stale AI footer events during the Day @ context main-menu round trip must not open Agent Chat"
    );

    let agent_model_arm = dispatcher
        .split("crate::footer_popup::FooterAction::AgentModel =>")
        .nth(1)
        .expect("FooterAction::AgentModel arm should exist");
    assert!(
        agent_model_arm.contains("AppView::DayPage { .. }")
            && agent_model_arm.contains("main_window_footer_agent_model_ignored_day_page"),
        "stale Day footer Agent/Model events must be ignored before profile/model picker paths"
    );
    assert!(
        agent_model_arm.contains("self.day_page_context_return.is_some()")
            && agent_model_arm
                .contains("main_window_footer_agent_model_ignored_day_page_context_return"),
        "stale Agent/Model footer events during the Day @ context main-menu round trip must not open profile/model picker paths"
    );
}
