//! Regression tests locking in GPUI vendor upgrade cleanups.
//!
//! These tests use source-string assertions to verify that already-landed
//! migrations (hover suppression removal, native follow-tail adoption) remain
//! in place and do not regress.

use script_kit_gpui::{
    theme::{contrast_ratio, Theme},
    warning_banner::WarningBannerColors,
};

const AI_ACTIONS_SOURCE: &str = include_str!("../src/ai/window/render_message_actions.rs");
const AI_MESSAGES_SOURCE: &str = include_str!("../src/ai/window/render_messages.rs");
const CHAT_STATE_SOURCE: &str = include_str!("../src/prompts/chat/state.rs");
const CHAT_RENDER_CORE_SOURCE: &str = include_str!("../src/prompts/chat/render_core.rs");
const ACTIONS_DIALOG_SOURCE: &str = include_str!("../src/actions/dialog.rs");
const ACTIONS_WINDOW_SOURCE: &str = include_str!("../src/actions/window.rs");
const MAIN_RENDER_SOURCE: &str = include_str!("../src/main_sections/render_impl.rs");
const LIST_ITEM_SOURCE: &str = include_str!("../src/list_item/mod.rs");
const FILE_SEARCH_SOURCE: &str = include_str!("../src/render_builtins/file_search.rs");
const THEME_CHOOSER_SOURCE: &str = include_str!("../src/render_builtins/theme_chooser.rs");
const KIT_STORE_SOURCE: &str = include_str!("../src/render_builtins/kit_store.rs");
const SELECT_PROMPT_SOURCE: &str = include_str!("../src/prompts/select/prompt.rs");
const SELECT_RENDER_SOURCE: &str = include_str!("../src/prompts/select/render.rs");
const SELECT_TYPES_SOURCE: &str = include_str!("../src/prompts/select/types.rs");
const UNIFIED_LIST_ITEM_RENDER_SOURCE: &str =
    include_str!("../src/components/unified_list_item/render.rs");
const WARNING_BANNER_SOURCE: &str = include_str!("../src/warning_banner.rs");

// ---------------------------------------------------------------------------
// AI action strip: unconditional GPUI hover
// ---------------------------------------------------------------------------

#[test]
fn ai_action_buttons_use_unconditional_gpui_hover() {
    assert!(
        AI_ACTIONS_SOURCE
            .contains(".hover(|s| s.bg(muted_bg.opacity(OPACITY_HOVER)).text_color(muted_fg))"),
        "action_btn_base should always apply GPUI hover styling"
    );
    assert!(
        !AI_ACTIONS_SOURCE.contains("mouse_mode"),
        "AI action buttons should not gate hover on a local mouse_mode flag"
    );
    assert!(
        !AI_ACTIONS_SOURCE.contains("InputMode::Mouse"),
        "AI action buttons should not gate hover on InputMode::Mouse"
    );
}

// ---------------------------------------------------------------------------
// ChatPrompt: native follow-tail via ListState::set_follow_tail
// ---------------------------------------------------------------------------

#[test]
fn chat_prompt_uses_native_follow_tail() {
    assert!(
        CHAT_STATE_SOURCE.contains("self.turns_list_state.set_follow_tail(false);"),
        "empty chat should explicitly disable follow-tail"
    );
    assert!(
        CHAT_STATE_SOURCE.contains(".set_follow_tail(!self.user_has_scrolled_up);"),
        "chat append path should drive follow-tail from manual-scroll state"
    );
    assert!(
        CHAT_STATE_SOURCE.contains("self.turns_list_state.set_follow_tail(item_count > 0);"),
        "force-scroll path should use follow-tail instead of manual last-item scrolling"
    );
    assert!(
        CHAT_RENDER_CORE_SOURCE
            .contains(".set_follow_tail(has_turns && !this.user_has_scrolled_up);"),
        "wheel reconciliation should write back to GPUI follow-tail"
    );
}

#[test]
fn chat_prompt_follow_paths_do_not_use_manual_item_scrolling() {
    assert!(
        !CHAT_STATE_SOURCE.contains("scroll_to_item(")
            && !CHAT_STATE_SOURCE.contains("scroll_to_reveal_item("),
        "state.rs follow-tail paths should not use manual item scrolling"
    );
    assert!(
        !CHAT_RENDER_CORE_SOURCE.contains("scroll_to_item(")
            && !CHAT_RENDER_CORE_SOURCE.contains("scroll_to_reveal_item("),
        "render_core.rs follow-tail paths should not use manual item scrolling"
    );
}

// ---------------------------------------------------------------------------
// AI window: native follow-tail via ListState::set_follow_tail
// ---------------------------------------------------------------------------

#[test]
fn ai_window_uses_native_follow_tail() {
    assert!(
        AI_MESSAGES_SOURCE
            .contains(".set_follow_tail(!self.user_has_scrolled_up && item_count > 0);"),
        "AI window append/stream path should drive native follow-tail"
    );
    assert!(
        AI_MESSAGES_SOURCE.contains("self.messages_list_state.set_follow_tail(item_count > 0);"),
        "AI window force-scroll path should use native follow-tail"
    );
    assert!(
        !AI_MESSAGES_SOURCE.contains("scroll_to_item(")
            && !AI_MESSAGES_SOURCE.contains("scroll_to_reveal_item("),
        "AI window follow-tail paths should not use manual item scrolling"
    );
}

// ---------------------------------------------------------------------------
// ActionsDialog: direct GPUI hover without local state machine
// ---------------------------------------------------------------------------

#[test]
fn actions_dialog_rows_use_direct_gpui_hover() {
    assert!(
        ACTIONS_DIALOG_SOURCE.contains("let hover_row_bg = if is_destructive {"),
        "actions dialog should compute a direct hover background"
    );
    assert!(
        ACTIONS_DIALOG_SOURCE.contains("row.hover("),
        "actions dialog rows should use GPUI hover styling directly"
    );
    assert!(
        ACTIONS_DIALOG_SOURCE.contains("AppChromeColors::from_theme(&this.theme)")
            && ACTIONS_DIALOG_SOURCE.contains("rgba(chrome.selection_rgba)")
            && ACTIONS_DIALOG_SOURCE.contains("rgba(chrome.hover_rgba)"),
        "default actions dialog rows should resolve selected/hover chrome through AppChromeColors"
    );
    assert!(
        ACTIONS_DIALOG_SOURCE.contains("show_container_border: false"),
        "default actions dialog chrome should stay borderless"
    );
    let default_branch_start = ACTIONS_DIALOG_SOURCE
        .find("if design_variant == DesignVariant::Default")
        .expect("actions dialog default design branch should exist");
    let default_branch = &ACTIONS_DIALOG_SOURCE[default_branch_start
        ..ACTIONS_DIALOG_SOURCE[default_branch_start..]
            .find("} else {")
            .map(|offset| default_branch_start + offset)
            .expect("actions dialog default branch else should exist")];
    assert!(
        !default_branch.contains("style.selection_opacity")
            && !default_branch.contains("style.hover_opacity"),
        "default actions dialog rows should not repack selected/hover opacity locally"
    );
}

#[test]
fn actions_dialog_does_not_gate_hover_through_local_state() {
    for needle in [
        "current_input_mode == InputMode::Mouse",
        "this.input_mode = InputMode::Mouse",
        "this.hovered_index = Some(ix)",
        "self.input_mode = InputMode::Keyboard;",
        "self.hovered_index = None;",
    ] {
        assert!(
            !ACTIONS_DIALOG_SOURCE.contains(needle),
            "unexpected legacy hover-state pattern still present: {needle}"
        );
    }
}

#[test]
fn actions_window_consumes_handled_popup_keys() {
    assert!(
        ACTIONS_WINDOW_SOURCE.contains("let handled = match actions_window_key_intent")
            && ACTIONS_WINDOW_SOURCE
                .contains("if handled {\n                cx.stop_propagation();\n            }"),
        "detached actions window should stop propagation after handling popup-owned keys"
    );
    assert!(
        ACTIONS_WINDOW_SOURCE.contains("actions_window_shortcut_matched")
            && ACTIONS_WINDOW_SOURCE
                .contains("true\n                    } else {\n                        false"),
        "matched action shortcuts should count as handled so they do not leak to the parent"
    );
}

// ---------------------------------------------------------------------------
// ListItem: GPUI hover without custom suppression toggle
// ---------------------------------------------------------------------------

#[test]
fn list_item_relies_on_gpui_hover_without_custom_toggle() {
    assert!(
        !LIST_ITEM_SOURCE.contains("enable_hover_effect"),
        "ListItem should not carry a custom hover suppression field"
    );
    assert!(
        !LIST_ITEM_SOURCE.contains("with_hover_effect"),
        "ListItem should not expose a custom hover suppression builder"
    );
    assert!(
        LIST_ITEM_SOURCE.contains("if !self.selected {"),
        "non-selected rows should keep hover styling"
    );
    assert!(
        LIST_ITEM_SOURCE.contains("inner_content = inner_content.hover(move |s| s.bg(hover_bg));"),
        "hover styling should be unconditional for non-selected rows"
    );
}

// ---------------------------------------------------------------------------
// File search: explicit hover state because AppKit drag-out can stale native hover
// ---------------------------------------------------------------------------

#[test]
fn file_search_rows_keep_drag_safe_explicit_hover_state() {
    assert!(
        FILE_SEARCH_SOURCE.contains("let file_hovered = self.hovered_index;"),
        "file search should keep explicit hover state for drag-safe row chrome"
    );
    assert!(
        FILE_SEARCH_SOURCE.contains("this.hovered_index = Some(ix);")
            && FILE_SEARCH_SOURCE.contains("this.hovered_index = None;"),
        "file search rows should set and clear explicit hover state"
    );
    assert!(
        !FILE_SEARCH_SOURCE.contains(".when(!is_selected, |d| d.hover(move |s| s.bg(hover_bg)))"),
        "file search deliberately avoids direct GPUI hover because drag-out can leave stale row hover"
    );
}

// ---------------------------------------------------------------------------
// Theme chooser: direct GPUI hover without manual hover bookkeeping
// ---------------------------------------------------------------------------

#[test]
fn theme_chooser_rows_use_direct_gpui_hover() {
    assert!(
        THEME_CHOOSER_SOURCE
            .contains("crate::list_item::ListItem::new(name.to_string(), list_colors)")
            && THEME_CHOOSER_SOURCE.contains(".selected(is_selected)")
            && THEME_CHOOSER_SOURCE.contains(".with_accent_bar(true)"),
        "theme chooser rows should use shared ListItem styling instead of manual hover bookkeeping"
    );
    for needle in [
        "current_input_mode == InputMode::Mouse",
        "let hover_entity_handle = entity_handle.clone();",
        "this.hovered_index = Some(ix);",
        ".on_hover(hover_handler)",
    ] {
        assert!(
            !THEME_CHOOSER_SOURCE.contains(needle),
            "unexpected legacy theme-chooser hover pattern still present: {needle}"
        );
    }
}

// ---------------------------------------------------------------------------
// Kit store: direct GPUI hover without manual hover bookkeeping
// ---------------------------------------------------------------------------

#[test]
fn kit_store_rows_use_direct_gpui_hover() {
    assert!(
        KIT_STORE_SOURCE.contains(".when(!is_selected,")
            && KIT_STORE_SOURCE.contains("row.hover(move |style| style.bg(hover_row_bg))"),
        "kit store rows should use direct GPUI hover styling"
    );
    assert!(
        KIT_STORE_SOURCE.contains("AppChromeColors::from_theme(&self.theme)"),
        "kit store rows should resolve selection/hover/badge chrome through AppChromeColors"
    );
    assert!(
        KIT_STORE_SOURCE.contains("rgba(chrome.selection_rgba)")
            && KIT_STORE_SOURCE.contains("rgba(chrome.hover_rgba)")
            && KIT_STORE_SOURCE.contains("rgba(chrome.accent_badge_bg_rgba)"),
        "kit store row state and action chips should consume shared chrome tokens"
    );
    for needle in [
        "hovered_index",
        "hovered_row == Some(ix) && input_mode == InputMode::Mouse",
        ".on_hover(move |is_hovered, _window, cx|",
        "opacity.selected * 255.0",
        "opacity.hover * 255.0",
    ] {
        assert!(
            !KIT_STORE_SOURCE.contains(needle),
            "unexpected legacy kit-store hover pattern still present: {needle}"
        );
    }
}

#[test]
fn kit_store_consumes_owned_keyboard_events() {
    for needle in [
        ".key_context(\"kit_store_browse\")",
        ".key_context(\"kit_store_installed\")",
        "is_key_escape(key)",
        "is_key_up(key)",
        "is_key_down(key)",
        "is_key_enter(key)",
        "\"backspace\"",
        "\"delete\"",
        "event.keystroke.key_char",
        "cx.stop_propagation();",
    ] {
        assert!(
            KIT_STORE_SOURCE.contains(needle),
            "Kit Store owned keyboard handling should consume handled keys and retain branch: {needle}"
        );
    }
}

#[test]
fn kit_store_uniform_lists_use_vendor_scrollbar() {
    assert!(
        KIT_STORE_SOURCE.contains("self.builtin_uniform_list_scrollbar(&self.list_scroll_handle"),
        "Kit Store uniform lists should attach the shared vendor scrollbar"
    );
}

#[test]
fn kit_store_uses_resolved_chrome_text_tokens() {
    for needle in [
        "rgba(chrome.text_muted_rgba)",
        "rgba(chrome.text_hint_rgba)",
        "rgba(chrome.placeholder_text_rgba)",
    ] {
        assert!(
            KIT_STORE_SOURCE.contains(needle),
            "Kit Store text colors should use resolved AppChromeColors token: {needle}"
        );
    }
    for needle in [
        "opacity.text_muted_alpha * 255.0",
        "opacity.text_hint * 255.0",
    ] {
        assert!(
            !KIT_STORE_SOURCE.contains(needle),
            "Kit Store should not repack text opacity locally: {needle}"
        );
    }
}

#[test]
fn kit_store_row_and_action_clicks_are_isolated() {
    for needle in [
        "\"kit-store-browse-row\"",
        "\"kit-store-installed-row\"",
        "\"kit-store-install-btn\"",
        "\"kit-store-update-btn\"",
        "\"kit-store-remove-btn\"",
        "cx.stop_propagation();",
        "this.kit_store_install_selected_result(",
        "this.kit_store_update_selected_kit(",
        "this.kit_store_remove_selected_kit(",
    ] {
        assert!(
            KIT_STORE_SOURCE.contains(needle),
            "Kit Store row/action click propagation contract should include: {needle}"
        );
    }
}

#[test]
fn kit_store_header_and_empty_states_are_bounded() {
    for needle in [
        ".h_full()\n                .flex()\n                .flex_col()\n                .items_center()\n                .justify_center()",
        ".flex_1()\n                            .min_w(px(0.0))\n                            .overflow_hidden()",
        ".flex_none()\n                            .whitespace_nowrap()",
    ] {
        assert!(
            KIT_STORE_SOURCE.contains(needle),
            "Kit Store surrounding chrome should keep bounded layout constraints: {needle}"
        );
    }
}

// ---------------------------------------------------------------------------
// ListItem: modality-aware hover disclosure via Window input modality
// ---------------------------------------------------------------------------

#[test]
fn list_item_masks_hover_driven_disclosure_with_window_input_modality() {
    assert!(
        LIST_ITEM_SOURCE
            .contains("let hover_visible = self.hovered && !window.last_input_was_keyboard();"),
        "ListItem should derive a modality-aware hover flag from Window"
    );
    for needle in [
        "should_show_search_description(self.selected, hover_visible, has_description_match)",
        "should_show_search_shortcut(is_filtering, self.selected, hover_visible)",
        "} else if hover_visible {",
        "self.selected || hover_visible || is_filtering",
    ] {
        assert!(
            LIST_ITEM_SOURCE.contains(needle),
            "ListItem should route hover-driven disclosure through hover_visible: {needle}"
        );
    }
}

// ---------------------------------------------------------------------------
// SelectPrompt: modality-aware hover via visual_row_state_for_input_modality
// ---------------------------------------------------------------------------

#[test]
fn select_prompt_masks_stateful_hover_with_window_input_modality() {
    assert!(
        SELECT_RENDER_SOURCE.contains("window.last_input_was_keyboard()"),
        "SelectPrompt should read GPUI input modality from Window"
    );
    assert!(
        SELECT_RENDER_SOURCE.contains("visual_row_state_for_input_modality("),
        "SelectPrompt should normalize row state before painting hover-driven visuals"
    );
    assert!(
        !SELECT_RENDER_SOURCE.contains("let is_hovered = row_state.is_hovered;"),
        "SelectPrompt should not paint hover from raw row_state anymore"
    );
    assert!(
        SELECT_RENDER_SOURCE.contains("let is_hovered = visual_row_state.is_hovered;"),
        "SelectPrompt should drive UnifiedListItem hover state from modality-adjusted row state"
    );
}

#[test]
fn select_prompt_uses_shared_semantic_ids_and_chrome_contract() {
    assert!(
        SELECT_TYPES_SOURCE.contains("pub(super) fn select_choice_semantic_id("),
        "SelectPrompt should centralize rendered and automation row semantic IDs"
    );
    assert!(
        SELECT_PROMPT_SOURCE.contains("select_choice_semantic_id(choice, choice_idx)"),
        "SelectPrompt getElements should use source-indexed shared semantic IDs"
    );
    assert!(
        SELECT_RENDER_SOURCE.contains("select_choice_semantic_id(choice, choice_idx)"),
        "SelectPrompt row render should use the same semantic ID helper as getElements"
    );
    assert!(
        SELECT_PROMPT_SOURCE.contains("PromptChromeAudit::minimal_list(\"prompts::select\", true)"),
        "SelectPrompt should emit the shared minimal-list chrome audit contract"
    );
    assert!(
        SELECT_RENDER_SOURCE.contains("render_minimal_list_prompt_shell_with_footer(")
            && SELECT_RENDER_SOURCE.contains("active_main_window_footer_surface()")
            && SELECT_RENDER_SOURCE.contains("render_native_main_window_footer_spacer()"),
        "SelectPrompt should use the footer-aware shell so native and GPUI footers cannot stack"
    );
}

#[test]
fn select_prompt_search_header_is_prompt_owned_and_shared_chrome() {
    let production = SELECT_RENDER_SOURCE
        .split("#[cfg(test)]")
        .next()
        .expect("select render production source should exist");
    assert!(
        production.contains("fn render_select_search_header("),
        "SelectPrompt should isolate its prompt-owned search header behind one helper"
    );
    assert!(
        production.contains("render_select_search_header("),
        "SelectPrompt render should use the shared prompt-owned search header helper"
    );
    assert!(
        production.contains("input:select-filter"),
        "SelectPrompt must preserve the automation-visible search input identity"
    );
    assert!(
        production.contains("render_minimal_list_prompt_shell_with_footer("),
        "SelectPrompt should keep shared minimal-list shell ownership for padding and footer layout"
    );
    assert!(
        !production.contains("render_search_input(")
            && !production.contains("gpui_input_state")
            && !production.contains("TextInputState"),
        "SelectPrompt must not borrow launcher or form text-input ownership"
    );
}

#[test]
fn select_prompt_keeps_selection_and_hover_ownership_explicit() {
    assert!(
        SELECT_RENDER_SOURCE.contains("pub(super) fn classify_select_key("),
        "SelectPrompt keyboard routing should be classified by a pure helper"
    );
    assert!(
        SELECT_RENDER_SOURCE.contains("SelectKeyIntent::LetGlobalHandle"),
        "SelectPrompt should yield platform shortcuts it does not own"
    );
    assert!(
        SELECT_RENDER_SOURCE.contains("visual_row_state_for_selection_mode("),
        "SelectPrompt should normalize visual selection by single/multi selection mode"
    );
    assert!(
        SELECT_RENDER_SOURCE.contains(".with_direct_hover(false)"),
        "SelectPrompt should disable UnifiedListItem direct hover so modality state owns hover paint"
    );
    assert!(
        UNIFIED_LIST_ITEM_RENDER_SOURCE.contains("direct_hover: true")
            && UNIFIED_LIST_ITEM_RENDER_SOURCE.contains("pub fn with_direct_hover(")
            && UNIFIED_LIST_ITEM_RENDER_SOURCE.contains("if self.direct_hover &&"),
        "UnifiedListItem should default to direct hover while allowing select to opt out"
    );
}

// ---------------------------------------------------------------------------
// WarningBanner: compact launcher chrome with isolated dismiss action
// ---------------------------------------------------------------------------

#[test]
fn warning_banner_uses_shared_chrome_tokens_and_safe_dismiss() {
    let dismiss_start = WARNING_BANNER_SOURCE
        .find("let dismiss_btn = {")
        .expect("warning banner dismiss button should exist");
    let dismiss_block = &WARNING_BANNER_SOURCE[dismiss_start
        ..WARNING_BANNER_SOURCE[dismiss_start..]
            .find("// Main banner container")
            .map(|offset| dismiss_start + offset)
            .expect("dismiss block should end before main banner")];

    assert!(
        dismiss_block.contains("cx.stop_propagation();"),
        "dismiss button clicks should not bubble into the parent banner action"
    );
    assert!(
        WARNING_BANNER_SOURCE.contains("best_readable_text_hex(colors.ui.warning)")
            && WARNING_BANNER_SOURCE.contains("let foreground ="),
        "warning banner foreground should be derived from the warning surface"
    );
    assert!(
        !WARNING_BANNER_SOURCE.contains("text: colors.background.main")
            && !WARNING_BANNER_SOURCE.contains("icon: colors.background.main")
            && !WARNING_BANNER_SOURCE.contains("dismiss: colors.background.main"),
        "warning banner should not use the window background as foreground on warning chrome"
    );
    assert!(
        WARNING_BANNER_SOURCE.contains("OPACITY_WARNING_BANNER_HOVER")
            && !WARNING_BANNER_SOURCE.contains("ALPHA_HOVER_WARNING"),
        "warning banner hover opacity should live in theme opacity tokens"
    );
    assert!(
        WARNING_BANNER_SOURCE.contains("TRANSPARENT")
            && WARNING_BANNER_SOURCE.contains("OPACITY_SUBTLE")
            && WARNING_BANNER_SOURCE
                .contains("hex_to_rgba_with_opacity(\n                    0x000000,\n                    OPACITY_SUBTLE"),
        "dismiss button colors should reuse shared transparent and subtle hover tokens"
    );
    assert!(
        !WARNING_BANNER_SOURCE.contains("0x00000026"),
        "warning banner should not hand-encode the subtle hover alpha"
    );
}

#[test]
fn warning_banner_text_is_contained_and_copy_is_mouse_neutral() {
    let message_start = WARNING_BANNER_SOURCE
        .find("let message_text = div()")
        .expect("warning banner message text should exist");
    let message_block = &WARNING_BANNER_SOURCE[message_start
        ..WARNING_BANNER_SOURCE[message_start..]
            .find("// Dismiss button")
            .map(|offset| message_start + offset)
            .expect("message block should end before dismiss button")];

    for needle in [
        ".flex_1()",
        ".min_w(px(0.0))",
        ".overflow_hidden()",
        ".text_ellipsis()",
    ] {
        assert!(
            message_block.contains(needle),
            "warning banner message should keep one-line bounded chrome: {needle}"
        );
    }
    assert!(
        MAIN_RENDER_SOURCE.contains("\"bun is not installed. Install from bun.sh\""),
        "bun warning copy should avoid presenting mouse click as the only path"
    );
    assert!(
        !MAIN_RENDER_SOURCE.contains("Click to download from bun.sh"),
        "bun warning copy should not use mouse-only wording"
    );
}

#[test]
fn warning_banner_theme_foreground_has_default_contrast() {
    for theme in [Theme::dark_default(), Theme::light_default()] {
        let colors = WarningBannerColors::from_theme(&theme);
        assert_eq!(colors.text, colors.icon);
        assert_eq!(colors.text, colors.dismiss);
        assert_eq!(colors.dismiss, colors.dismiss_hover);
        assert!(
            contrast_ratio(colors.text, colors.background) >= 4.5,
            "warning banner foreground should remain readable on warning background"
        );
    }
}
