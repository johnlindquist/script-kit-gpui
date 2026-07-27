//! Source-level contract for the shared main-window automation surface re-key owner.

const AUTOMATION_SURFACE_SOURCE: &str = include_str!("../src/app_impl/automation_surface.rs");
const ABOUT_ROUTE_SOURCE: &str = include_str!("../src/app_impl/about_route.rs");
const TRIGGER_DISPATCH_SOURCE: &str = include_str!("../src/app_impl/trigger_builtin_dispatch.rs");
const AGENT_CHAT_SETUP_SOURCE: &str =
    include_str!("../src/app_impl/agent_handoff/agent_chat_setup.rs");
const AGENT_CHAT_LAUNCH_SOURCE: &str =
    include_str!("../src/app_impl/agent_handoff/agent_chat_launch.rs");
const TAB_AI_MODE_SOURCE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");
const AGENT_CHAT_SURFACE_TRANSITIONS_SOURCE: &str =
    include_str!("../src/app_impl/agent_chat_surface_transitions.rs");

fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing function signature: {signature}"));
    let after_start = &source[start..];
    let open = after_start
        .find('{')
        .unwrap_or_else(|| panic!("missing function body for: {signature}"));
    let mut depth = 0usize;
    for (offset, ch) in after_start[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &after_start[..open + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated function body: {signature}");
}

#[test]
fn shared_owner_derives_main_surface_from_current_view_contract() {
    assert!(
        AUTOMATION_SURFACE_SOURCE.contains(
            "pub(crate) fn rekey_main_automation_surface_from_current_view(&self) -> bool"
        ),
        "ScriptListApp must expose a named owner for current-view main automation re-keying"
    );
    assert!(
        AUTOMATION_SURFACE_SOURCE.contains("crate::semantic_surface_for_main_view(&self.current_view)")
            && AUTOMATION_SURFACE_SOURCE
                .contains("crate::windows::update_automation_semantic_surface(\"main\", semantic_surface)"),
        "the owner must derive from the active AppView contract and perform the narrow in-place registry update"
    );
}

#[test]
fn transition_owner_sets_view_then_rekeys_from_current_view_contract() {
    let body = function_body(
        AUTOMATION_SURFACE_SOURCE,
        "pub(crate) fn transition_current_view_and_rekey_main_automation_surface",
    );
    assert!(
        body.contains("self.current_view = next_view;")
            && body.contains("self.rekey_main_automation_surface_from_current_view()"),
        "transition owner must assign the requested AppView and then re-key from the active view"
    );
    assert!(
        !body.contains("update_automation_semantic_surface("),
        "transition owner must delegate semantic-surface writes to the shared re-key owner"
    );
}

#[test]
fn restore_owner_sets_view_and_focus_without_hidden_side_effects_contract() {
    let body = function_body(
        AUTOMATION_SURFACE_SOURCE,
        "pub(crate) fn restore_current_view_with_focus",
    );
    assert!(
        body.contains("self.current_view = next_view;")
            && body.contains("self.pending_focus = Some(focus_target);")
            && body.contains("FocusTarget::MainFilter => FocusedInput::MainFilter")
            && body.contains("FocusTarget::ActionsDialog => FocusedInput::ActionsSearch"),
        "restore owner must restore the AppView and translate the requested focus target"
    );
    assert!(
        !body.contains("rekey_main_automation_surface_from_current_view")
            && !body.contains("update_automation_semantic_surface(")
            && !body.contains("cx.notify"),
        "restore owner must not hide re-keying or notification side effects from route-specific callers"
    );
}

#[test]
fn script_list_main_filter_owner_delegates_to_restore_owner_contract() {
    let body = function_body(
        AUTOMATION_SURFACE_SOURCE,
        "pub(crate) fn show_script_list_with_main_filter_focus",
    );
    assert!(
        body.contains(
            "self.restore_current_view_with_focus(AppView::ScriptList, FocusTarget::MainFilter);"
        ),
        "ScriptList main-filter route owner must delegate AppView + focus restoration to restore_current_view_with_focus"
    );
    assert!(
        body.contains("self.rekey_main_automation_surface_from_current_view()"),
        "ScriptList main-filter route owner must re-key the main automation surface after restoring ScriptList"
    );
}

#[test]
fn return_view_restore_paths_delegate_to_shared_owner() {
    for (name, source, signature) in [
        (
            "attachment_portal.rs",
            include_str!("../src/app_impl/attachment_portal.rs"),
            "fn restore_attachment_portal_return_view",
        ),
        (
            "agent_handoff/mod.rs",
            TAB_AI_MODE_SOURCE,
            "fn close_tab_ai_harness_terminal_impl",
        ),
    ] {
        let body = function_body(source, signature);
        assert!(
            body.contains("self.restore_current_view_with_focus("),
            "{name}::{signature} must restore AppView + focus through restore_current_view_with_focus"
        );
        assert!(
            !body.contains("self.current_view = return_view")
                && !body.contains("self.pending_focus = Some(return_focus_target)"),
            "{name}::{signature} must not split return-view restoration into raw field writes"
        );
    }
}

#[test]
fn script_list_entry_paths_delegate_to_main_filter_owner() {
    for (name, source, signature) in [
        (
            "attachment_portal.rs",
            include_str!("../src/app_impl/attachment_portal.rs"),
            "fn open_script_list_attachment_portal",
        ),
        (
            "filter_input_change.rs",
            include_str!("../src/app_impl/filter_input_change.rs"),
            "pub(crate) fn handle_filter_input_change",
        ),
        (
            "builtin_execution.rs",
            include_str!("../src/app_execute/builtin_execution.rs"),
            "fn open_main_window",
        ),
    ] {
        let body = function_body(source, signature);
        assert!(
            body.contains("self.show_script_list_with_main_filter_focus();"),
            "{name}::{signature} must use show_script_list_with_main_filter_focus"
        );
    }
}

#[test]
fn trigger_builtin_helper_delegates_to_shared_owner() {
    let body = function_body(
        TRIGGER_DISPATCH_SOURCE,
        "pub(crate) fn rekey_main_automation_surface_after_trigger_builtin_dispatch",
    );
    assert!(
        body.contains("self.rekey_main_automation_surface_from_current_view()"),
        "triggerBuiltin post-dispatch re-keying must reuse the shared current-view owner"
    );
    assert!(
        !body.contains("update_automation_semantic_surface(")
            && !body.contains("semantic_surface_for_main_view(&self.current_view)"),
        "triggerBuiltin helper must not duplicate raw semantic-surface lookup or registry writes"
    );
}

#[test]
fn about_and_confirm_routes_rekey_from_current_view_contract() {
    assert_eq!(
        ABOUT_ROUTE_SOURCE
            .matches("self.transition_current_view_and_rekey_main_automation_surface(")
            .count(),
        3,
        "About open, About dismiss, and parent confirm open must transition through the shared current-view owner"
    );
    assert!(
        !ABOUT_ROUTE_SOURCE.contains("self.rekey_main_automation_surface_from_current_view();"),
        "About/confirm routes should not split current_view mutation from the paired semantic re-key"
    );
    for forbidden in ["Some(\"about\"", "Some(\"confirmPrompt\""] {
        assert!(
            !ABOUT_ROUTE_SOURCE.contains(forbidden),
            "About/confirm routes must not hand-code semanticSurface literals outside the surface registry: {forbidden}"
        );
    }
}

#[test]
fn embedded_agent_chat_entry_and_return_paths_rekey_from_current_view_contract() {
    for (name, source) in [
        ("agent_chat_setup.rs", AGENT_CHAT_SETUP_SOURCE),
        ("agent_chat_launch.rs", AGENT_CHAT_LAUNCH_SOURCE),
        ("agent_handoff/mod.rs", TAB_AI_MODE_SOURCE),
    ] {
        assert!(
            !source.contains("semantic_surface_for_main_view(&self.current_view)"),
            "{name} must not duplicate current-view semantic-surface lookup"
        );
    }

    assert!(
        !TAB_AI_MODE_SOURCE.contains("semantic_surface_for_main_view(&self.current_view)"),
        "agent_handoff/mod.rs must not hand-roll main semantic-surface re-keying"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("self.exit_embedded_agent_chat_surface("),
        "agent_handoff/mod.rs must delegate embedded Agent Chat return-origin close to the lifecycle actor"
    );
    assert!(
        !AGENT_CHAT_SETUP_SOURCE.contains("self.rekey_main_automation_surface_from_current_view();")
            && !AGENT_CHAT_LAUNCH_SOURCE
                .contains("self.rekey_main_automation_surface_from_current_view();"),
        "Agent Chat setup and launch entry paths must not split re-keying from the shared embedded entry owner"
    );
    assert!(
        AGENT_CHAT_SURFACE_TRANSITIONS_SOURCE
            .contains("pub(crate) fn enter_embedded_agent_chat_surface")
            && AGENT_CHAT_SURFACE_TRANSITIONS_SOURCE
                .contains("pub(crate) fn exit_embedded_agent_chat_surface")
            && AGENT_CHAT_SURFACE_TRANSITIONS_SOURCE
                .matches("self.rekey_main_automation_surface_from_current_view()")
                .count()
                >= 2,
        "embedded Agent Chat entry and exit must re-key main through the lifecycle actors"
    );
    let entry_delegate_count: usize = [
        ("agent_chat_setup.rs", AGENT_CHAT_SETUP_SOURCE),
        ("agent_chat_launch.rs", AGENT_CHAT_LAUNCH_SOURCE),
        ("agent_handoff/mod.rs", TAB_AI_MODE_SOURCE),
    ]
    .iter()
    .map(|(_, source)| {
        source
            .matches("self.enter_embedded_agent_chat_surface(")
            .count()
    })
    .sum();
    // Enumerated delegation sites (do not grow this list casually):
    // 1. agent_chat_setup.rs — setup view entry
    // 2. agent_chat_launch.rs — standard mock fixture entry
    // 3. agent_chat_launch.rs — Quick AI codex-exec entry
    // 4. agent_chat_launch.rs — Pi launch entry
    // 5. agent_handoff/mod.rs — reattach-from-detached entry
    // 6. agent_handoff/mod.rs — cached-view reuse entry
    assert_eq!(
        entry_delegate_count, 6,
        "setup, launch (fixture/quick-ai/pi), reattach, and reuse Agent Chat entry paths must delegate to enter_embedded_agent_chat_surface"
    );
}
