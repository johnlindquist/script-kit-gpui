//! Source-level contracts for main-list ownership of menu-syntax trigger rows.

const TRIGGER_OWNER: &str = include_str!("../src/app_impl/menu_syntax_trigger_picker_main_list.rs");
const FILTER_INPUT_CHANGE: &str = include_str!("../src/app_impl/filter_input_change.rs");
const FILTERING_CACHE: &str = include_str!("../src/app_impl/filtering_cache.rs");
const RENDER_SCRIPT_LIST: &str = include_str!("../src/render_script_list/mod.rs");
const STARTUP: &str = include_str!("../src/app_impl/startup.rs");
const SIMULATE_KEY: &str = include_str!("../src/app_impl/simulate_key_dispatch.rs");
const UI_WINDOW: &str = include_str!("../src/app_impl/ui_window.rs");
const SELECTION_FALLBACK: &str = include_str!("../src/app_impl/selection_fallback.rs");
const APP_NAV_MOVEMENT: &str = include_str!("../src/app_navigation/impl_movement.rs");
const PROMPT_HANDLER: &str = include_str!("../src/prompt_handler/mod.rs");

#[test]
fn trigger_owner_no_longer_defines_detached_prompt_popup_window() {
    for stale in [
        "pub(crate) struct MenuSyntaxTriggerPickerWindow",
        "pub(crate) fn sync_menu_syntax_trigger_picker_main_list",
        "sync_menu_syntax_trigger_picker_main_list_for_filter",
        "MENU_SYNTAX_TRIGGER_POPUP_AUTOMATION_ID",
        "AutomationWindowKind::PromptPopup",
        "register_attached_popup",
        "menuSyntaxTriggerPicker",
        "open_window",
        "close_menu_syntax_trigger_picker_main_list",
        "is_menu_syntax_trigger_picker_main_list_open",
        "batch_select_menu_syntax_trigger_picker_row_by_value",
        "batch_select_menu_syntax_trigger_picker_row_by_semantic_id",
    ] {
        assert!(
            !TRIGGER_OWNER.contains(stale),
            "trigger picker must not keep detached popup window code: {stale}"
        );
    }
}

#[test]
fn filter_changes_update_trigger_state_without_window_sync() {
    assert!(
        FILTER_INPUT_CHANGE.contains("let mut trigger_state_changed = false;")
            && FILTER_INPUT_CHANGE.contains("trigger_state_changed = true;")
            && FILTER_INPUT_CHANGE.contains("} else if trigger_state_changed {")
            && FILTER_INPUT_CHANGE.contains("self.invalidate_grouped_cache();"),
        "filter changes should update trigger state and invalidate the main-list cache"
    );
    assert!(
        !FILTER_INPUT_CHANGE.contains("sync_menu_syntax_trigger_picker_main_list_for_filter"),
        "filter changes must not sync trigger rows into a detached popup"
    );
}

#[test]
fn script_list_keyboard_routes_trigger_picker_by_main_state() {
    assert!(
        TRIGGER_OWNER.contains("fn menu_syntax_trigger_picker_owns_main_keyboard")
            && TRIGGER_OWNER.contains("self.menu_syntax_trigger_picker_state.owns_main_list()"),
        "trigger picker keyboard ownership should be state-based"
    );
    for source in [RENDER_SCRIPT_LIST, SIMULATE_KEY] {
        assert!(
            source.contains("menu_syntax_trigger_picker_owns_main_keyboard()")
                && source.contains("apply_menu_syntax_trigger_picker_intent"),
            "ScriptList key path should route trigger picker intents by main-list state"
        );
    }
}

/// A real Enter can reach both the global key-down interceptor and the GPUI
/// input's PressEnter callback. Accepting a trigger row must consume the
/// follow-up PressEnter after the picker closes, or the first filtered script
/// row can be submitted by the same physical key.
#[test]
fn trigger_picker_enter_accept_arms_press_enter_echo_guard() {
    for needle in [
        "fn arm_menu_syntax_trigger_picker_enter_guard",
        "fn should_consume_menu_syntax_trigger_picker_press_enter",
        "menu_syntax_trigger_picker_enter_guard_armed",
        "menu_syntax_trigger_picker_press_enter_consumed",
    ] {
        assert!(
            TRIGGER_OWNER.contains(needle),
            "trigger picker owner must define/log Enter echo guard: {needle}"
        );
    }

    assert!(
        STARTUP.contains("this.arm_menu_syntax_trigger_picker_enter_guard(")
            && STARTUP.contains("this.should_consume_menu_syntax_trigger_picker_press_enter(")
            && STARTUP.find("this.should_consume_menu_syntax_trigger_picker_press_enter(")
                < STARTUP.find("this.should_consume_script_list_enter_after_submit("),
        "ScriptList PressEnter must check the trigger-picker echo guard before launcher submit guards"
    );
}

/// Accepting a type qualifier filters the main list while leaving the first
/// script row selected. The footer must describe the committed filter rather
/// than advertising that auto-selected row as immediately runnable.
#[test]
fn type_filter_accept_uses_temporary_showing_footer_label() {
    for needle in [
        "fn menu_syntax_type_filter_accept_label",
        "\"type:script\" => Some(\"Showing Scripts\")",
        "fn arm_menu_syntax_filter_accept_hint",
        "event = \"menu_syntax_filter_accept_hint_armed\"",
        "self.arm_menu_syntax_filter_accept_hint(&filter);",
        "self.rebuild_main_window_preflight_if_needed();",
    ] {
        assert!(
            TRIGGER_OWNER.contains(needle),
            "trigger picker accept path must arm a temporary type-filter footer hint: {needle}"
        );
    }

    assert!(
        UI_WINDOW.contains("self.menu_syntax_filter_accept_primary_label()")
            && UI_WINDOW.contains("return label.to_string();"),
        "main-window footer label must route the accepted-filter hint through the shared primary label path"
    );
}

/// While the accepted-filter hint is visible, Enter must not run the first
/// filtered script row. Explicit navigation clears the hint and restores the
/// normal Run Script label/behavior.
#[test]
fn type_filter_accept_hint_consumes_enter_until_selection_moves() {
    assert!(
        TRIGGER_OWNER.contains("fn should_consume_menu_syntax_filter_accept_enter")
            && TRIGGER_OWNER.contains("event = \"menu_syntax_filter_accept_enter_consumed\""),
        "trigger picker owner must expose/log Enter consumption for accepted-filter hint"
    );
    assert!(
        SELECTION_FALLBACK
            .contains("self.should_consume_menu_syntax_filter_accept_submit(\"execute_selected\")"),
        "normal launcher execute path must consume submit while accepted-filter hint is active"
    );
    assert!(
        SIMULATE_KEY.contains(
            "view.should_consume_menu_syntax_filter_accept_enter(\n                                    \"simulate_key_enter\","
        ),
        "simulateKey Enter path must consume the accepted-filter hint before execute_selected"
    );
    assert!(
        APP_NAV_MOVEMENT.contains("self.clear_menu_syntax_filter_accept_hint();")
            && RENDER_SCRIPT_LIST.contains("this.clear_menu_syntax_filter_accept_hint();"),
        "keyboard and mouse selection movement must clear the accepted-filter hint"
    );
}

#[test]
fn trigger_picker_pointer_path_has_proof_grade_logs() {
    for needle in [
        "menu_syntax_trigger_picker_accept_start",
        "menu_syntax_trigger_picker_accept_missing_snapshot",
        "menu_syntax_trigger_picker_accept_missing_row",
        "menu_syntax_trigger_picker_accept_outcome",
        "row_id = row_id.unwrap_or(\"\")",
    ] {
        assert!(
            TRIGGER_OWNER.contains(needle),
            "trigger picker accept path must log proof-grade accept events: {needle}"
        );
    }
}

#[test]
fn trigger_picker_rows_are_cached_as_main_list_rows() {
    for needle in [
        "fn build_menu_syntax_trigger_picker_main_list_results",
        "trigger_picker_row_to_main_list_row(row)",
        "SearchResult::SpineProjection",
    ] {
        assert!(
            FILTERING_CACHE.contains(needle),
            "trigger picker must build shared main-list rows through grouped cache: {needle}"
        );
    }
}

#[test]
fn trigger_picker_render_falls_through_to_shared_main_list() {
    assert!(
        RENDER_SCRIPT_LIST.contains("trigger_picker_owns_main_list_for_render")
            && RENDER_SCRIPT_LIST.contains("&& !trigger_picker_owns_main_list_for_render")
            && RENDER_SCRIPT_LIST.contains("&& !object_selector_owns_main_list_for_render"),
        "trigger picker ownership must bypass menu-syntax hint/card rendering and use the shared list"
    );
}

/// Committed filter heads such as `type:` still use trigger-picker rows for
/// their values. The active-head hint branch must not intercept those rows
/// before the shared list renderer can paint them.
#[test]
fn active_filter_head_render_branch_excludes_trigger_and_object_pickers() {
    let active_head_branch = RENDER_SCRIPT_LIST
        .split("} else if active_filter_head_owns_main_list_for_render")
        .nth(1)
        .and_then(|tail| {
            tail.split("} else if let Some(hint) = advanced_query_guide_hint")
                .next()
        })
        .expect("active filter head render branch");

    assert!(
        active_head_branch.contains("&& !trigger_picker_owns_main_list_for_render")
            && active_head_branch.contains("&& !object_selector_owns_main_list_for_render"),
        "active filter head hint rendering must not preempt trigger/object picker rows"
    );
}

#[test]
fn trigger_picker_arrows_are_not_popup_intents() {
    let arrow_block = STARTUP
        .split("AppView::ScriptList => {")
        .nth(1)
        .and_then(|tail| {
            tail.split("// CRITICAL: If actions picker is active")
                .next()
        })
        .expect("ScriptList arrow routing block");
    assert!(
        !arrow_block.contains("menu_syntax_trigger_picker_owns_main_keyboard()")
            && !arrow_block.contains("apply_menu_syntax_trigger_picker_intent"),
        "ArrowUp/ArrowDown must flow to shared main-list movement, not trigger picker movement"
    );
}

#[test]
fn prompt_popup_resolution_excludes_menu_syntax_trigger_picker() {
    assert!(
        !PROMPT_HANDLER.contains(
            "menu_syntax_trigger_picker_main_list::is_menu_syntax_trigger_picker_main_list_open"
        ),
        "PromptPopup target resolution must not include menu-syntax trigger picker"
    );
    assert!(
        !PROMPT_HANDLER.contains("batch_select_menu_syntax_trigger_picker_row_by_value")
            && !PROMPT_HANDLER
                .contains("batch_select_menu_syntax_trigger_picker_row_by_semantic_id"),
        "PromptPopup batch routing must not select menu-syntax trigger rows"
    );
}

#[test]
fn active_footer_does_not_treat_trigger_picker_as_popup() {
    let footer_block = PROMPT_HANDLER
        .split("pub(crate) fn active_footer_snapshot")
        .nth(1)
        .and_then(|tail| tail.split("let mut config").next())
        .expect("active footer block");

    assert!(
        !footer_block.contains("menu_syntax_trigger_picker_state")
            && !footer_block.contains("is_menu_syntax_trigger_picker_main_list_open")
            && !footer_block.contains("menu_syntax_object_selector_state")
            && !footer_block.contains("is_menu_syntax_object_selector_main_list_open"),
        "main-owned trigger picker must not make active footer owner popup"
    );
}
