const ACTION_HANDLER_SOURCE: &str = include_str!("../src/app_actions/handle_action/mod.rs");
const ACTIONS_TOGGLE_SOURCE: &str = include_str!("../src/app_impl/actions_toggle.rs");
const PROFILES_SOURCE: &str = include_str!("../src/ai/agent_chat/profiles.rs");
const PROFILE_SEARCH_SOURCE: &str = include_str!("../src/profile_search.rs");
const APP_IMPL_PROFILE_SEARCH_SOURCE: &str = include_str!("../src/app_impl/profile_search_view.rs");
const RENDER_PROFILE_SEARCH_SOURCE: &str = include_str!("../src/render_builtins/profile_search.rs");
const RENDER_SCRIPT_LIST_SOURCE: &str = include_str!("../src/render_script_list/mod.rs");
const COLLECT_ELEMENTS_SOURCE: &str = include_str!("../src/app_layout/collect_elements.rs");
const ACT_TS: &str = include_str!("../scripts/devtools/act.ts");
const APP_VIEW_STATE_SOURCE: &str = include_str!("../src/main_sections/app_view_state.rs");
const AGENT_CHAT_LAUNCH_SOURCE: &str =
    include_str!("../src/app_impl/agent_handoff/agent_chat_launch.rs");
const TAB_AI_MODE_SOURCE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");
const FILTER_INPUT_CORE_SOURCE: &str = include_str!("../src/app_impl/filter_input_core.rs");
const FILTER_INPUT_UPDATES_SOURCE: &str = include_str!("../src/app_impl/filter_input_updates.rs");
const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/view.rs");
const AGENT_CHAT_THREAD_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/thread.rs");
const AGENT_CHAT_MOD_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/mod.rs");
const CHAT_WINDOW_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/chat_window.rs");
const PROMPT_HANDLER_SOURCE: &str = include_str!("../src/prompt_handler/mod.rs");
const CONTEXT_SELECTOR_TYPES_SOURCE: &str = include_str!("../src/ai/context_selector/types.rs");
const CONTEXT_SELECTOR_SOURCE: &str = include_str!("../src/ai/context_selector/mod.rs");
const CONFIG_TYPES_SOURCE: &str = include_str!("../src/config/types.rs");
const AUTOMATION_COLLECTOR_SOURCE: &str =
    include_str!("../src/windows/automation_surface_collector.rs");
const DETACHED_TRANSACTION_PROVIDER_SOURCE: &str =
    include_str!("../src/windows/automation_transaction_provider.rs");
const FOOTER_CHROME_SOURCE: &str = include_str!("../src/components/footer_chrome.rs");
const FOOTER_POPUP_SOURCE: &str = include_str!("../src/footer_popup.rs");
const STDIN_COMMANDS_SOURCE: &str = include_str!("../src/stdin_commands/mod.rs");
const SPINE_PROFILE_SOURCE: &str = include_str!("../src/spine/catalog_profile.rs");
const STARTUP_SOURCE: &str = include_str!("../src/app_impl/startup.rs");
const STARTUP_NEW_TAB_SOURCE: &str = include_str!("../src/app_impl/startup_new_tab.rs");
const SIMULATE_KEY_DISPATCH_SOURCE: &str = include_str!("../src/app_impl/simulate_key_dispatch.rs");
const UI_WINDOW_SOURCE: &str = include_str!("../src/app_impl/ui_window.rs");

fn fn_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source.find(signature).expect("signature must exist");
    let rest = &source[start..];
    let body_start = signature.len()
        + rest[signature.len()..]
            .find('{')
            .expect("function or block body must start after its signature");
    let mut depth = 0usize;
    for (idx, ch) in rest[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &rest[..body_start + idx + 1];
                }
            }
            _ => {}
        }
    }
    panic!("function body must close");
}

#[test]
fn profile_picker_is_main_menu_spine_not_deprecated_popup() {
    assert!(SPINE_PROFILE_SOURCE.contains("agent_chat_profile_picker_entries"));
    assert!(SPINE_PROFILE_SOURCE.contains("selected_agent_chat_profile_picker_id"));
    assert!(SPINE_PROFILE_SOURCE.contains("SpineListRowKind::Profile"));
    assert!(SPINE_PROFILE_SOURCE.contains("resolution_source: ss(\"profile\")"));

    assert!(!AGENT_CHAT_MOD_SOURCE.contains("profile_selector_popup"));
    assert!(!AGENT_CHAT_VIEW_SOURCE.contains("profile_selector_open"));
    assert!(
        !AGENT_CHAT_VIEW_SOURCE.contains("sync_profile_selector_popup_window_from_cached_parent")
    );
    assert!(!PROMPT_HANDLER_SOURCE.contains("batch_select_profile_by_value"));
    assert!(!PROMPT_HANDLER_SOURCE.contains("batch_select_profile_by_semantic_id"));
    assert!(!PROMPT_HANDLER_SOURCE.contains("is_profile_selector_popup_window_open"));
    assert!(!AUTOMATION_COLLECTOR_SOURCE.contains("collect_profile_selector_snapshot"));
    assert!(!DETACHED_TRANSACTION_PROVIDER_SOURCE.contains("batch_select_profile_by_value"));
    assert!(!DETACHED_TRANSACTION_PROVIDER_SOURCE.contains("batch_select_profile_by_semantic_id"));
    assert!(!STDIN_COMMANDS_SOURCE.contains("OpenAgentChatProfilePicker"));
    assert!(!STDIN_COMMANDS_SOURCE.contains("\"openAgentChatProfilePicker\""));
}

#[test]
fn agent_chat_spine_context_attacher_uses_main_menu_list_chrome() {
    let body = fn_body(
        AGENT_CHAT_VIEW_SOURCE,
        "fn render_agent_chat_spine_projection_area(",
    );

    for required in [
        "crate::list_item::render_section_header(",
        "ListItemColors::from_theme(theme)",
        "ListItem::new(title, list_colors)",
        ".selected(selected)",
        ".main_menu_theme(main_menu_theme)",
        ".semantic_id(format!(\"agent_chat-spine-row-{row_id}\"",
        ".description_opt(subtitle)",
        ".source_hint_opt(source_hint)",
        ".type_accessory_opt(Some(TypeAccessory",
        "crate::list_item::effective_list_item_height_for_theme(main_menu_theme)",
    ] {
        assert!(
            body.contains(required),
            "Agent Chat context attacher must reuse main-menu list chrome: {required}"
        );
    }

    for forbidden in [
        "theme.colors.accent.selected << 8) | 0x22",
        ".px(px(10.0))",
        ".py(px(7.0))",
        ".rounded(px(6.0))",
        ".text_sm()",
        ".justify_between()",
    ] {
        assert!(
            !body.contains(forbidden),
            "Agent Chat context attacher must not keep bespoke item row styling: {forbidden}"
        );
    }
}

#[test]
fn bare_pipe_stays_in_main_menu_search_for_profile_rows() {
    assert!(!FILTER_INPUT_CORE_SOURCE.contains("AgentChatProfilePicker"));
    assert!(!FILTER_INPUT_UPDATES_SOURCE.contains("open_tab_ai_agent_chat_with_profile_picker"));
    assert!(!TAB_AI_MODE_SOURCE.contains("open_tab_ai_agent_chat_with_profile_picker"));
    assert!(!CHAT_WINDOW_SOURCE.contains("open_detached_profile_picker"));
}

#[test]
fn spine_profile_submission_persists_pi_profile_before_launch() {
    let body = fn_body(
        TAB_AI_MODE_SOURCE,
        "pub(crate) fn try_submit_spine_prompt_plan_from_parse_with_aliases(",
    );
    assert!(body.contains("persist_agent_chat_profile_selection"));
    assert!(body.contains("save_user_preferences(&prefs)"));
    assert!(body.contains("Agent Chat profile: {profile_name}"));
    assert!(PROFILES_SOURCE.contains("ai.selected_profile_id = Some(entry.id.clone());"));
    assert!(PROFILES_SOURCE.contains("ai.selected_backend = Some(AgentChatBackend::Pi);"));
}

#[test]
fn profile_selection_updates_live_agent_chat_footer_without_relaunch() {
    assert!(ACTION_HANDLER_SOURCE.contains("agent_chat_switch_profile_id_from_action"));
    assert!(ACTION_HANDLER_SOURCE.contains("persist_agent_chat_profile_selection"));
    assert!(AGENT_CHAT_VIEW_SOURCE.contains("set_on_profile_selected"));
    assert!(AGENT_CHAT_THREAD_SOURCE.contains("pub(crate) fn set_profile_display("));
    assert!(AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) fn set_profile_display("));
    assert!(TAB_AI_MODE_SOURCE.contains("view.set_profile_display("));
    assert!(TAB_AI_MODE_SOURCE.contains("select_agent_chat_profile_and_relaunch"));
    let body = fn_body(
        TAB_AI_MODE_SOURCE,
        "fn select_agent_chat_profile_and_relaunch(",
    );
    assert!(
        body.contains("refresh_agent_model_footer_labels")
            && body.find("refresh_agent_model_footer_labels")
                < body.find("view.set_profile_display("),
        "live Agent Chat profile selection must refresh the shared main-menu header labels before updating the embedded view"
    );
}

#[test]
fn agent_chat_launch_uses_effective_profile_for_agent_chat_agent_and_model() {
    assert!(AGENT_CHAT_LAUNCH_SOURCE.contains("resolve_selected_pi_launch_with_cwd_override"));
    assert!(AGENT_CHAT_LAUNCH_SOURCE.contains("resolve_focused_text_pi_launch"));
    assert!(AGENT_CHAT_LAUNCH_SOURCE
        .contains("profile_display_name: Some(pi_launch.profile.name.clone().into())"));
    assert!(
        AGENT_CHAT_LAUNCH_SOURCE.contains("profile_icon_name: pi_launch.profile.icon_name.clone()")
    );
}

#[test]
fn provider_scoped_pi_model_selection_is_split_before_launch() {
    assert!(PROFILES_SOURCE.contains("parse_provider_model_selection"));
    assert!(PROFILES_SOURCE.contains("profile.provider = Some(provider);"));
    assert!(PROFILES_SOURCE.contains("profile.model = Some(model);"));
}

#[test]
fn primary_pi_provider_catalog_is_codex_only_with_advanced_alternatives() {
    let primary = fn_body(PROFILES_SOURCE, "pub fn pi_provider_model_catalog()");
    assert!(primary.contains(r#"id: "openai-codex""#));
    assert!(primary.contains(r#"display_name: "Codex""#));
    assert!(
        !primary.contains(r#"id: "anthropic""#) && !primary.contains(r#"id: "google""#),
        "primary Agent Chat provider catalog must not surface alternate providers"
    );

    let advanced = fn_body(
        PROFILES_SOURCE,
        "pub fn advanced_pi_provider_model_catalog()",
    );
    assert!(advanced.contains(r#"id: "anthropic""#));
    assert!(advanced.contains(r#"id: "google""#));
}

#[test]
fn profile_display_flows_through_thread_and_footer() {
    assert!(AGENT_CHAT_THREAD_SOURCE.contains("profile_display_name"));
    assert!(AGENT_CHAT_THREAD_SOURCE.contains("pub(crate) fn profile_display"));
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("profile_display: thread.profile_display().to_string()")
    );
    assert!(AGENT_CHAT_VIEW_SOURCE.contains("profile_left_info"));
    assert!(AGENT_CHAT_VIEW_SOURCE.contains("profile_name: Some(self.profile_display.clone())"));
    assert!(AGENT_CHAT_VIEW_SOURCE.contains("agent-chat-profile-display"));
    assert!(AGENT_CHAT_VIEW_SOURCE.contains("snapshot.profile_display.clone()"));
    assert!(AGENT_CHAT_VIEW_SOURCE.contains(".id(\"agent_chat-profile-display\")"));
    assert!(AGENT_CHAT_VIEW_SOURCE.contains(".id(\"agent_chat-model-display\")"));
}

#[test]
fn shift_tab_routes_to_profile_switcher_copy() {
    assert!(APP_IMPL_PROFILE_SEARCH_SOURCE.contains("profile_switcher_open_shift_tab"));
    assert!(STARTUP_SOURCE.contains("agent_chat_shift_tab_profile_switcher"));
    assert!(STARTUP_NEW_TAB_SOURCE.contains("agent_chat_shift_tab_profile_switcher"));
    assert!(!STARTUP_SOURCE.contains("Shift+Tab → Agent & Model picker"));
    assert!(!STARTUP_NEW_TAB_SOURCE.contains("Shift+Tab → Agent & Model picker"));
}

#[test]
fn simulate_key_shift_tab_routes_to_profile_switcher_for_runtime_proof() {
    let script_list_arm = SIMULATE_KEY_DISPATCH_SOURCE
        .split("AppView::ScriptList =>")
        .nth(1)
        .expect("ScriptList simulateKey arm must exist")
        .split("AppView::")
        .next()
        .expect("ScriptList simulateKey arm must have a body");

    assert!(script_list_arm.contains("try_open_profile_search_from_script_list_shift_tab"));
    assert!(script_list_arm.contains("has_shift"));
    assert!(APP_IMPL_PROFILE_SEARCH_SOURCE.contains("profile_switcher_open_shift_tab"));
    assert!(APP_IMPL_PROFILE_SEARCH_SOURCE.contains("open_profile_search(cx)"));
    assert!(APP_IMPL_PROFILE_SEARCH_SOURCE.contains("menu_syntax_capture_form_owns_input"));
    assert!(!script_list_arm.contains("submit_to_current_or_new_tab_ai_harness_from_text"));
}

#[test]
fn shift_tab_profile_search_is_main_window_split_pane_not_actions_dialog() {
    assert!(APP_VIEW_STATE_SOURCE.contains("ProfileSearchView"));
    assert!(APP_VIEW_STATE_SOURCE.contains("SurfaceKind::ProfileSearch"));
    assert!(APP_VIEW_STATE_SOURCE.contains("RequiredSplitPreview"));
    assert!(STARTUP_SOURCE.contains("try_open_profile_search_from_script_list_shift_tab"));
    assert!(
        SIMULATE_KEY_DISPATCH_SOURCE.contains("try_open_profile_search_from_script_list_shift_tab")
    );
    assert!(APP_IMPL_PROFILE_SEARCH_SOURCE.contains("open_profile_search(cx)"));

    assert!(!ACTIONS_TOGGLE_SOURCE.contains("pub(crate) fn open_profile_switcher_window("));
    assert!(!SIMULATE_KEY_DISPATCH_SOURCE.contains("open_profile_switcher_window"));
    assert!(!STARTUP_SOURCE.contains("open_profile_switcher_window"));
    assert!(!ACTIONS_TOGGLE_SOURCE.contains("set_root_route(profile_route.clone())"));
    assert!(!ACTIONS_TOGGLE_SOURCE.contains("get_agent_chat_profile_picker_route"));
}

#[test]
fn profile_search_persists_profile_selection_without_actions_gate() {
    assert!(PROFILE_SEARCH_SOURCE.contains("persist_agent_chat_profile_selection"));
    assert!(PROFILE_SEARCH_SOURCE.contains("save_user_preferences(&prefs)"));
    assert!(PROFILE_SEARCH_SOURCE.contains("profile_search_profile_persisted"));
    assert!(APP_IMPL_PROFILE_SEARCH_SOURCE.contains("refresh_agent_model_footer_labels"));
    assert!(APP_IMPL_PROFILE_SEARCH_SOURCE.contains("reset_to_script_list(cx)"));

    assert!(!APP_IMPL_PROFILE_SEARCH_SOURCE.contains("execute_action_for_actions_host"));
    assert!(!APP_IMPL_PROFILE_SEARCH_SOURCE.contains("agent_chat_switch_profile_id_from_action"));
    assert!(!APP_IMPL_PROFILE_SEARCH_SOURCE.contains("agent_model_picker_active"));
}

#[test]
fn profile_search_renderer_has_right_pane_preview() {
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("profile-search-root"));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("profile-search-list"));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("profile-search-row"));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("profile-search-preview"));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("profile-search-preview-title"));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("profile-search-preview-model"));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("profile-search-preview-tools"));
}

/// Profile rows must inherit shared selection/hover/theme behavior, not pin a
/// particular decoration such as the shared component's optional accent bar.
#[test]
fn profile_search_renderer_uses_shared_list_item_contract() {
    let body = fn_body(RENDER_PROFILE_SEARCH_SOURCE, "fn render_profile_search(");
    for needle in [
        "ListItem::new(result.profile.name.clone(), list_colors)",
        "let description = profile_search_row_description(result);",
        ".description(description)",
        ".highlight_indices_opt(",
        ".description_highlight_indices_opt(",
        ".selected(is_selected)",
        ".hovered(is_hovered)",
        "let main_menu_theme = self.current_main_menu_theme;",
        ".main_menu_theme(main_menu_theme)",
        ".trailing_accessory_opt(",
        "profile_search_row_status_accessory(",
        "ListItemColors::from_theme(&self.theme)",
        "ListItem owns selected/hover/theme",
    ] {
        assert!(
            body.contains(needle),
            "ProfileSearch rows must use shared ListItem contract: {needle}"
        );
    }

    for forbidden in [
        "let row_bg = if is_selected",
        ".bg(rgba(row_bg))",
        "OPACITY_SELECTED",
        "OPACITY_HIDDEN",
        ".mx(px(4.0))",
        ".px(px(14.0))",
        ".py(px(4.0))",
        ".rounded(px(8.0))",
        "StyledText::new",
    ] {
        assert!(
            !body.contains(forbidden),
            "ProfileSearch must not reintroduce one-off row styling: {forbidden}"
        );
    }
}

#[test]
fn profile_search_renderer_tags_only_current_and_quick_ai_rows() {
    // Rows carry a status accessory only when they are the Agent Chat
    // default ("Current") and/or the Quick AI target ("Quick AI"); every
    // other row stays untagged — no per-row "Profile" noise.
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("(false, false) => return None,"));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("\"Current\""));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("\"Quick AI\""));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("\"Current · Quick AI\""));
    assert!(!RENDER_PROFILE_SEARCH_SOURCE.contains("\"Profile\""));
    assert!(COLLECT_ELEMENTS_SOURCE.contains("Some(\"current\".to_string())"));
    assert!(COLLECT_ELEMENTS_SOURCE.contains("Some(\"quick-ai\".to_string())"));
}

#[test]
fn profile_search_footer_uses_switch_profile_label() {
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("↵ Switch Profile"));
    assert!(RENDER_PROFILE_SEARCH_SOURCE.contains("⇥ Use for Quick AI"));
    assert!(!RENDER_PROFILE_SEARCH_SOURCE.contains("↵ Select Profile"));
    let body = fn_body(
        UI_WINDOW_SOURCE,
        "pub(crate) fn main_window_primary_action_label(",
    );
    assert!(body.contains("AppView::ProfileSearchView { .. }"));
    assert!(body.contains("\"Switch Profile\".to_string()"));
}

/// Profile queries must publish synchronously and return before the general
/// coalesced filter path; both caret-preserving and end-of-input callers share it.
#[test]
fn profile_search_filter_updates_bypass_coalescer_for_instant_search() {
    let wrapper = fn_body(
        FILTER_INPUT_UPDATES_SOURCE,
        "pub(crate) fn set_filter_text_immediate(",
    );
    assert!(wrapper.contains("self.set_filter_text_and_cursor_immediate("));
    let owner = fn_body(
        FILTER_INPUT_UPDATES_SOURCE,
        "pub(crate) fn set_filter_text_and_cursor_immediate(",
    );
    let profile_branch = fn_body(
        owner,
        "if handled_by_subview && matches!(self.current_view, AppView::ProfileSearchView { .. })",
    );
    for required in [
        "self.computed_filter_text = text.clone();",
        "self.filter_coalescer.reset();",
        "cx.notify();",
        "return;",
    ] {
        assert!(
            profile_branch.contains(required),
            "missing immediate profile update: {required}"
        );
    }
}

#[test]
fn profile_search_preview_explains_profiles_in_structured_sections() {
    for needle in [
        "profile-search-preview-explanation",
        "profile-search-preview-overview",
        "profile-search-preview-runtime",
        "profile-search-preview-instructions",
        "Working directory",
        "Instructions",
    ] {
        assert!(
            RENDER_PROFILE_SEARCH_SOURCE.contains(needle),
            "ProfileSearch preview should include structured profile explanation: {needle}"
        );
    }
    assert!(PROFILE_SEARCH_SOURCE.contains("Profiles define"));
}

#[test]
fn profile_search_renderer_keeps_hover_and_stable_profile_row_semantics() {
    for needle in [
        "let profile_hovered = self.hovered_index;",
        "this.input_mode = InputMode::Mouse;",
        "this.hovered_index = Some(ix);",
        "this.hovered_index = None;",
        ".on_hover(hover_handler)",
        ".semantic_id(format!(",
        "\"profile-search-row:{}\"",
        "result.profile.id",
        ".track_scroll(&self.list_scroll_handle)",
        "builtin_uniform_list_scrollbar(&self.list_scroll_handle",
    ] {
        assert!(
            RENDER_PROFILE_SEARCH_SOURCE.contains(needle),
            "ProfileSearch rows must preserve hover, scroll, and stable semantics: {needle}"
        );
    }
}

#[test]
fn profile_search_devtools_collector_exposes_rows_and_preview_not_current_view_fallback() {
    for needle in [
        "AppView::ProfileSearchView",
        "collect_profile_search_elements",
        "input:profile-search-input",
        "list:profile-search-results",
        "profile-search-row:",
        "status:profile-search-current",
        "profile-search-preview",
        "profile-search-preview-title",
        "profile-search-preview-model",
        "profile-search-preview-cwd",
        "profile-search-preview-tools",
        "profile-search-preview-prompt",
        "profile_search_elements_truncated_by_limit",
    ] {
        assert!(
            COLLECT_ELEMENTS_SOURCE.contains(needle),
            "ProfileSearch element collector must expose {needle}"
        );
    }

    let profile_arm = COLLECT_ELEMENTS_SOURCE
        .split("AppView::ProfileSearchView")
        .nth(1)
        .expect("ProfileSearch collect_elements arm must exist")
        .split("AppView::")
        .next()
        .expect("ProfileSearch collect_elements arm must have a body");
    assert!(profile_arm.contains("collect_profile_search_elements"));
    assert!(!profile_arm.contains("collector_used_current_view_fallback"));
}

/// Agents must address profiles by stable IDs and distinguish the current
/// chat profile from the Quick AI profile, including when they are the same row.
#[test]
fn profile_search_devtools_rows_use_stable_profile_id_semantic_ids() {
    let body = fn_body(
        COLLECT_ELEMENTS_SOURCE,
        "fn collect_profile_search_elements(",
    );
    for needle in [
        "format!(\"profile-search-row:{}\", result.profile.id)",
        "element_type: protocol::ElementType::Choice",
        "selected: Some(index == selected_index)",
        "value: Some(result.profile.id.clone())",
        "selectable: Some(true)",
    ] {
        assert!(
            body.contains(needle),
            "ProfileSearch rows must expose stable selectable row semantics: {needle}"
        );
    }
    let status = fn_body(
        body,
        "status_kind: match (result.selected, result.quick_ai)",
    );
    for arm in [
        "(true, true) => Some(\"current+quick-ai\".to_string())",
        "(true, false) => Some(\"current\".to_string())",
        "(false, true) => Some(\"quick-ai\".to_string())",
        "(false, false) => None",
    ] {
        assert!(
            status.contains(arm),
            "missing profile status mapping: {arm}"
        );
    }
}

#[test]
fn profile_search_devtools_submit_proof_is_main_surface_only() {
    for needle in [
        "function isProfileSearchTargetReceipt",
        "function isPlainEnter",
        "function isScopedProfileSearchSelect",
        "\"profile-search-select\"",
        "allowedBy: \"submitIntent:profile-search-select\"",
        "profile-search-row:",
        "submit.reason.required",
        "profile-search-select requires plain Enter on main ProfileSearch with a selected profile row",
        "resolved?.automationId === \"main\"",
        "resolved?.targetKind === \"Main\"",
        "resolved?.surfaceKind === \"ProfileSearch\"",
        "resolved?.semanticSurface === \"profileSearch\"",
        "args.modifiers.length === 0",
        "targetArgs: [\"--main\", \"--strict\", \"--surface\", \"ScriptList\"]",
        "expectedSurfaceKind: \"ScriptList\"",
    ] {
        assert!(
            ACT_TS.contains(needle),
            "act.ts ProfileSearch Enter allowlist must include {needle}"
        );
    }
}

#[test]
fn profile_search_enter_stops_propagation_before_selection_transition() {
    let enter_pos = RENDER_PROFILE_SEARCH_SOURCE
        .find("if is_key_enter(key)")
        .expect("ProfileSearch Enter handler must exist");
    let enter_section = &RENDER_PROFILE_SEARCH_SOURCE
        [enter_pos..(enter_pos + 300).min(RENDER_PROFILE_SEARCH_SOURCE.len())];
    let stop_pos = enter_section
        .find("cx.stop_propagation();")
        .expect("ProfileSearch Enter must stop propagation");
    let select_pos = enter_section
        .find("select_profile_search_result")
        .expect("ProfileSearch Enter must select a profile");
    assert!(
        stop_pos < select_pos,
        "ProfileSearch must stop propagation before resetting to ScriptList"
    );
}

#[test]
fn profile_search_selection_arms_enter_guard_before_reset_to_script_list() {
    let body = fn_body(
        APP_IMPL_PROFILE_SEARCH_SOURCE,
        "pub(crate) fn select_profile_search_result(",
    );
    let guard_pos = body
        .find("arm_return_to_script_list_enter_guard_from_profile_search")
        .expect("ProfileSearch selection must arm Enter transition guard");
    let reset_pos = body
        .find("reset_to_script_list_from_profile_search(window, cx)")
        .expect("ProfileSearch selection must return to ScriptList");
    assert!(
        guard_pos < reset_pos,
        "Enter transition guard must be armed before ScriptList is rendered"
    );
}

#[test]
fn profile_search_selection_refreshes_header_labels_after_reset() {
    let body = fn_body(
        APP_IMPL_PROFILE_SEARCH_SOURCE,
        "pub(crate) fn select_profile_search_result(",
    );
    let persist_pos = body
        .find("persist_profile_search_selection")
        .expect("ProfileSearch must persist before refresh");
    let reset_pos = body
        .find("reset_to_script_list_from_profile_search(window, cx)")
        .expect("ProfileSearch selection must return to ScriptList");
    let refresh_positions = body
        .match_indices("refresh_agent_model_footer_labels")
        .map(|(pos, _)| pos)
        .collect::<Vec<_>>();
    assert!(
        refresh_positions
            .iter()
            .any(|pos| *pos > persist_pos && *pos < reset_pos),
        "ProfileSearch must refresh active profile labels before returning to ScriptList"
    );
    assert!(
        refresh_positions.iter().any(|pos| *pos > reset_pos),
        "ProfileSearch should refresh labels after reset so shared header cannot render stale profile text"
    );
}

/// The Enter that selects a profile must not execute a launcher item after
/// returning. Check every live execution site inside the key handler, not an
/// obsolete fallback helper or a pointer-click handler elsewhere in the render.
#[test]
fn script_list_enter_guard_runs_before_execute_selected_paths() {
    let body = fn_body(RENDER_SCRIPT_LIST_SOURCE, "fn render_script_list(");
    let key_handler = fn_body(body, "let handle_key = cx.listener(");
    let guard_pos = key_handler
        .find("consume_return_to_script_list_enter_guard")
        .expect("ScriptList must consume return-to-list Enter guard");
    let guard = fn_body(key_handler, "consume_return_to_script_list_enter_guard");
    assert!(guard.contains("cx.stop_propagation();"));
    assert!(guard.contains("return;"));
    for needle in [
        "try_apply_pending_menu_syntax_ai_proposal",
        "try_handle_spine_enter",
        "execute_selected(",
    ] {
        assert!(
            key_handler.contains(needle),
            "ScriptList Enter path missing {needle}"
        );
        for (pos, _) in key_handler.match_indices(needle) {
            assert!(
                guard_pos < pos,
                "return-to-ScriptList guard must run before {needle}"
            );
        }
    }
}

#[test]
fn script_list_key_up_clears_return_to_script_list_enter_guard() {
    assert!(
        RENDER_SCRIPT_LIST_SOURCE.contains("clear_return_to_script_list_enter_guard_on_key_up"),
        "ScriptList key-up must clear the ProfileSearch Enter transition guard"
    );
}

/// Keep profile selection on the left status marker rather than duplicating
/// a trailing action, and dispatch GPUI/native clicks through the owning binding.
#[test]
fn footer_profile_affordance_is_merged_into_left_status_marker() {
    assert!(FOOTER_CHROME_SOURCE.contains("FOOTER_PROFILE_ICON_TOKEN"));
    assert!(FOOTER_CHROME_SOURCE.contains("FOOTER_PROFILE_ICON_PATH"));

    let footer_buttons = fn_body(AGENT_CHAT_VIEW_SOURCE, "fn footer_buttons_for_thread");
    assert!(!footer_buttons.contains("FooterAction::Ai"));
    assert!(!footer_buttons.contains("FOOTER_PROFILE_ICON_TOKEN"));

    assert!(AGENT_CHAT_VIEW_SOURCE.contains("profile_left_info"));
    assert!(AGENT_CHAT_VIEW_SOURCE.contains("render_profile_status_marker_from_snapshot"));
    assert!(AGENT_CHAT_VIEW_SOURCE
        .contains("FooterAction::Ai => self.open_profile_trigger_picker_in_window"));

    assert!(FOOTER_POPUP_SOURCE.contains("pub action: Option<FooterAction>"));
    assert!(FOOTER_POPUP_SOURCE.contains("pub icon_token: Option<String>"));
    let marker = fn_body(FOOTER_POPUP_SOURCE, "fn render_left_info(");
    let marker_action = fn_body(marker, "if let Some(action) = info.action");
    assert!(marker_action.contains("let binding = self.binding.clone();"));
    assert!(marker_action.contains("dispatch_bound_footer_action(&binding, action)"));
    let native_action = fn_body(FOOTER_POPUP_SOURCE, "fn send_footer_action_from_sender(");
    assert!(native_action.contains("dispatch_bound_footer_action(&binding, action)"));
}

#[test]
fn pipe_trigger_selects_agent_chat_profiles_without_context_attachment() {
    assert!(CONTEXT_SELECTOR_TYPES_SOURCE.contains("Profile"));
    assert!(CONTEXT_SELECTOR_TYPES_SOURCE.contains("PROFILE_TRIGGER_CHAR: char = '|'"));
    assert!(CONTEXT_SELECTOR_TYPES_SOURCE.contains("AgentChatProfile"));
    assert!(CONTEXT_SELECTOR_SOURCE.contains("b'|' => ContextSelectorTrigger::Profile"));
    let accept_body = fn_body(
        AGENT_CHAT_VIEW_SOURCE,
        "fn accept_composer_picker_selection_impl(",
    );
    assert!(accept_body.contains("AgentChatComposerPickerTrigger::Profile"));
    assert!(accept_body.contains("ContextSelectorRowKind::AgentChatProfile"));
    assert!(accept_body.contains("Self::replace_text_in_char_range("));
    assert!(accept_body.contains("session.trigger_range.clone()"));
    assert!(accept_body.contains("thread.input.set_text(next_text)"));
    assert!(accept_body.contains("self.select_profile_from_popup(&profile_id, cx);"));
}

fn config_profile_icon_name_flows_to_footer_marker() {
    assert!(CONFIG_TYPES_SOURCE.contains("pub icon_name: Option<String>"));
    assert!(PROFILES_SOURCE.contains("pub icon_name: Option<String>"));
    assert!(PROFILES_SOURCE.contains("icon_name: profile.icon_name"));
    assert!(AGENT_CHAT_THREAD_SOURCE.contains("profile_icon_name: Option<String>"));
    assert!(AGENT_CHAT_THREAD_SOURCE.contains("pub(crate) fn profile_icon_name(&self)"));
    assert!(AGENT_CHAT_VIEW_SOURCE
        .contains("profile_icon_name: thread.profile_icon_name().map(str::to_string)"));
    assert!(AGENT_CHAT_VIEW_SOURCE.contains("footer_icon_path_or_profile"));
    assert!(FOOTER_POPUP_SOURCE.contains("pub icon_token: Option<String>"));
}
