//! Source-level contract for the native main-window footer surface owner.

const APP_VIEW_STATE_SOURCE: &str = include_str!("../src/main_sections/app_view_state.rs");
const DICTATION_WINDOW_SOURCE: &str = include_str!("../src/dictation/window.rs");
const FOOTER_POPUP_SOURCE: &str = include_str!("../src/footer_popup.rs");
const UI_WINDOW_SOURCE: &str = include_str!("../src/app_impl/ui_window.rs");
const RENDER_PROMPTS_OTHER_SOURCE: &str = include_str!("../src/render_prompts/other.rs");
const STARTUP_SOURCE: &str = include_str!("../src/app_impl/startup.rs");
const VIBRANCY_CONFIG_SOURCE: &str = include_str!("../src/platform/vibrancy_config.rs");
const SIMULATE_KEY_DISPATCH_SOURCE: &str = include_str!("../src/app_impl/simulate_key_dispatch.rs");

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
fn app_view_owns_native_footer_surface_map() {
    let body = function_body(APP_VIEW_STATE_SOURCE, "pub(crate) fn native_footer_surface");
    for expected in [
        "AppView::ScriptList => Some(\"script_list\")",
        "AppView::QuickTerminalView { .. } => Some(\"quick_terminal\")",
        "AppView::AgentChatView { .. } => Some(\"agent_chat\")",
        "AppView::ScriptIssuesView { .. } => Some(\"script_issues\")",
        "AppView::ConfirmPrompt { .. } => Some(\"confirm_prompt\")",
        "AppView::TermPrompt { .. } => Some(\"term_prompt\")",
        "AppView::About { .. } => Some(\"about\")",
        "AppView::MicroPrompt { .. } => Some(\"micro_prompt\")",
        "AppView::SdkReferenceView { .. } => Some(\"sdk_reference\")",
        "AppView::ScriptTemplateCatalogView { .. } => Some(\"script_template_catalog\")",
        "AppView::CreateAiPresetView { .. } => Some(\"create_ai_preset\")",
        "AppView::NotesBrowseView { .. } => Some(\"notes_browse\")",
    ] {
        assert!(
            body.contains(expected),
            "AppView::native_footer_surface must declare footer ownership for `{expected}`"
        );
    }
    assert!(
        !body.contains("_ => None"),
        "native footer ownership must remain explicit so new AppView variants cannot inherit footer behavior silently"
    );
}

/// Shrink-only ratchet: main-window views that still opt OUT of the native
/// footer. The Tips browser shipped 2026-07-11 with a hand-rolled hint-strip
/// footer because `TipsView` silently joined this `None` group; every NEW
/// view must own a native footer surface (shared FooterButtonConfig buttons)
/// and may only appear here after an explicit, documented decision. Fixing a
/// legacy view means DELETING its entry — never add one.
#[test]
fn main_window_views_without_native_footer_are_ratcheted() {
    const LEGACY_GPUI_FOOTER_VIEWS: &[&str] = &["ActionsDialog"];

    let body = function_body(APP_VIEW_STATE_SOURCE, "pub(crate) fn native_footer_surface");
    let segments: Vec<&str> = body.split("=>").collect();
    let mut none_views: Vec<String> = Vec::new();
    for index in 1..segments.len() {
        if !segments[index].trim_start().starts_with("None") {
            continue;
        }
        let mut pattern = segments[index - 1];
        while let Some(position) = pattern.find("AppView::") {
            let after = &pattern[position + "AppView::".len()..];
            let name: String = after
                .chars()
                .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                .collect();
            if !name.is_empty() {
                none_views.push(name);
            }
            pattern = after;
        }
    }

    assert!(
        !none_views.is_empty(),
        "expected to parse the legacy `=> None` arms of native_footer_surface"
    );
    for view in &none_views {
        assert!(
            LEGACY_GPUI_FOOTER_VIEWS.contains(&view.as_str()),
            "AppView::{view} maps to None in native_footer_surface but is not a \
             grandfathered legacy surface. New main-window views MUST return \
             Some(surface) and declare native footer buttons in \
             main_window_footer_buttons_for_current_view — do not extend the \
             legacy list; it is shrink-only."
        );
    }
    for legacy in LEGACY_GPUI_FOOTER_VIEWS {
        assert!(
            none_views.iter().any(|view| view == legacy),
            "AppView::{legacy} no longer opts out of the native footer — delete it \
             from LEGACY_GPUI_FOOTER_VIEWS (shrink-only ratchet)"
        );
    }
}

/// The six OF-57 migrations use the same action authorities as their physical
/// keyboard handlers; native footer labels must never become decorative.
#[test]
fn migrated_footer_buttons_route_to_real_surface_actions() {
    let buttons = function_body(
        UI_WINDOW_SOURCE,
        "fn main_window_footer_buttons_for_current_view",
    );
    for expected in [
        "about_footer_buttons(enabled)",
        "micro_prompt_footer_buttons(enabled)",
        "sdk_reference_footer_buttons(enabled, has_selection)",
        "script_template_catalog_footer_buttons(enabled, has_selection)",
        "create_ai_preset_footer_buttons(enabled, !name.trim().is_empty())",
        "notes_browse_footer_buttons(",
    ] {
        assert!(
            buttons.contains(expected),
            "migrated footer config must use its truthful button grammar: {expected}"
        );
    }

    let dispatch = function_body(
        UI_WINDOW_SOURCE,
        "pub(crate) fn dispatch_main_window_footer_action",
    );
    for expected in [
        "self.dismiss_about(cx);",
        "self.submit_arg_prompt_from_current_state(&prompt_id, cx);",
        "self.copy_selected_sdk_reference_markdown(cx)",
        "self.show_naming_dialog_for_script_template(template, window, cx);",
        "self.copy_selected_script_template_markdown(cx)",
        "self.handle_create_ai_preset_key(\"enter\", window, cx);",
        "self.handle_create_ai_preset_key(\"tab\", window, cx);",
        "self.attach_selected_note_from_footer(cx)",
        "self.close_attachment_portal_cancel(cx);",
    ] {
        assert!(
            dispatch.contains(expected),
            "migrated footer action must dispatch real surface behavior: {expected}"
        );
    }
}

#[test]
fn tips_view_keeps_native_footer_with_copy_example_primary_action() {
    let footer_map = function_body(APP_VIEW_STATE_SOURCE, "pub(crate) fn native_footer_surface");
    assert!(
        footer_map.contains("AppView::TipsView { .. } => Some(\"tips\")"),
        "TipsView must keep the main-window native footer active"
    );

    let footer_buttons = function_body(
        UI_WINDOW_SOURCE,
        "fn main_window_footer_buttons_for_current_view",
    );
    assert!(
        footer_buttons.contains("AppView::TipsView")
            && footer_buttons
                .contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Copy Example\")")
            && footer_buttons.contains("FooterButtonConfig::new(FooterAction::Close, \"Esc\", secondary_label)"),
        "TipsView footer must expose Copy Example and Back/Clear Search through the shared FooterButtonConfig components"
    );

    let footer_dispatch = function_body(
        UI_WINDOW_SOURCE,
        "pub(crate) fn dispatch_main_window_footer_action",
    );
    assert!(
        footer_dispatch.contains("AppView::TipsView { .. }")
            && footer_dispatch.contains("self.tips_copy_selected_example(cx);"),
        "TipsView footer Run must copy the selected example via the shared action authority"
    );
}

#[test]
fn script_issues_view_keeps_native_footer_and_fix_in_agent_primary_action() {
    let footer_map = function_body(APP_VIEW_STATE_SOURCE, "pub(crate) fn native_footer_surface");
    assert!(
        footer_map.contains("AppView::ScriptIssuesView { .. } => Some(\"script_issues\")"),
        "ScriptIssuesView must keep the main-window native footer active"
    );

    let footer_buttons = function_body(
        UI_WINDOW_SOURCE,
        "fn main_window_footer_buttons_for_current_view",
    );
    assert!(
        footer_buttons.contains("AppView::ScriptIssuesView { .. }")
            && footer_buttons
                .contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Fix in Agent\")")
            && footer_buttons
                .contains("FooterButtonConfig::new(FooterAction::Apply, \"⌘C\", \"Copy Issues\")"),
        "ScriptIssuesView footer must expose Fix in Agent and Copy Issues"
    );

    let footer_dispatch = function_body(
        UI_WINDOW_SOURCE,
        "pub(crate) fn dispatch_main_window_footer_action",
    );
    assert!(
        footer_dispatch.contains("AppView::ScriptIssuesView { report }")
            && footer_dispatch.contains("self.fix_script_issues_in_agent(&report, cx);"),
        "ScriptIssuesView footer Run must submit diagnostics to Agent Chat"
    );

    assert!(
        footer_dispatch.contains("AppView::ScriptIssuesView { report }")
            && footer_dispatch.contains("self.copy_script_issues_to_clipboard(&report, cx);"),
        "ScriptIssuesView footer Copy Issues must copy diagnostics"
    );
}

#[test]
fn script_issues_enter_routes_to_agent_chat_prompt_submission() {
    assert!(
        RENDER_PROMPTS_OTHER_SOURCE.contains("pub(crate) fn format_script_issues_agent_prompt")
            && RENDER_PROMPTS_OTHER_SOURCE.contains("Self::format_script_issues_diagnostics(report)")
            && RENDER_PROMPTS_OTHER_SOURCE.contains("ask_agent_chat_suppressing_focused_part(prompt, cx)"),
        "Script issues Agent handoff must include the formatted diagnostics and suppress focused context"
    );

    assert!(
        STARTUP_SOURCE.contains("if let AppView::ScriptIssuesView { report } = &this.current_view")
            && STARTUP_SOURCE.contains("this.fix_script_issues_in_agent(&report, cx);")
            && STARTUP_SOURCE.contains("cx.stop_propagation();"),
        "physical Enter must route ScriptIssuesView to Fix in Agent before generic key handling"
    );

    assert!(
        SIMULATE_KEY_DISPATCH_SOURCE.contains("AppView::ScriptIssuesView { report }")
            && SIMULATE_KEY_DISPATCH_SOURCE
                .contains("SimulateKey: Enter - fix script issues in Agent Chat")
            && SIMULATE_KEY_DISPATCH_SOURCE
                .contains("view.fix_script_issues_in_agent(&report, ctx);"),
        "simulateKey Enter must keep parity with physical Enter for ScriptIssuesView"
    );

    let render_script_issues = function_body(
        RENDER_PROMPTS_OTHER_SOURCE,
        "pub(crate) fn render_script_issues_view",
    );
    assert!(
        render_script_issues.contains("has_cmd && key.eq_ignore_ascii_case(\"w\")")
            && render_script_issues.contains("this.go_back_or_close(window, cx);"),
        "ScriptIssuesView must handle Cmd+W as a close/back shortcut"
    );
}

#[test]
fn native_footer_visual_effect_refresh_invalidates_without_synchronous_redisplay() {
    // The shared host-refresh body lives in `refresh_footer_host_impl`;
    // `refresh_main_footer_host` / `refresh_window_footer_host` are thin
    // wrappers that only choose glyph ownership.
    let refresh = function_body(FOOTER_POPUP_SOURCE, "unsafe fn refresh_footer_host_impl");
    let invalidate = function_body(
        FOOTER_POPUP_SOURCE,
        "unsafe fn invalidate_footer_effect_view_theme",
    );

    assert!(
        refresh.contains("footer_content_changed")
            && refresh.contains("footer_visuals_changed")
            && refresh.contains("effect_theme_changed")
            && refresh.contains("invalidate_footer_effect_view_theme(")
            && refresh.contains("effect_theme_changed\n            || footer_geometry_changed"),
        "native footer refresh must invalidate the AppKit visual-effect background after theme/material/geometry changes"
    );
    assert!(
        refresh.contains("if footer_content_changed")
            && refresh.contains("} else if footer_visuals_changed {")
            && refresh.contains("recolor_footer_hint_subviews(hints_view, &theme);")
            && refresh.contains("layout_footer_hints(hints_view, text_color, &config.buttons, &theme);"),
        "theme-only native footer refreshes must recolor existing AppKit hint subviews instead of rebuilding buttons"
    );
    assert!(
        invalidate.contains("effect_theme_changed")
            && invalidate.contains("setNeedsLayout: YES")
            && invalidate.contains("setNeedsDisplay: YES")
            && invalidate.contains("footer_layer")
            && invalidate.contains("setNeedsDisplay")
            && !invalidate.contains("layoutSubtreeIfNeeded")
            && !invalidate.contains("displayIfNeeded"),
        "native footer visual-effect refresh should invalidate layout/display without forcing synchronous AppKit display"
    );
}

#[test]
fn native_vibrancy_config_skips_redundant_same_window_reapply() {
    let body = function_body(
        VIBRANCY_CONFIG_SOURCE,
        "pub fn configure_window_vibrancy_material_for_appearance",
    );

    assert!(
        VIBRANCY_CONFIG_SOURCE.contains("static LAST_MAIN_WINDOW_VIBRANCY_SIGNATURE")
            && body.contains("let signature = (")
            && body.contains("window as usize,")
            && body.contains("background_tint,")
            && body.contains("glass_bits,")
            && body.contains("guard.as_ref() == Some(&signature)")
            && body.contains("return;")
            && body.find("guard.as_ref() == Some(&signature)")
                < body.find("configure_visual_effect_views_recursive("),
        "theme preview must not recursively reconfigure native vibrancy when window, appearance, and material are unchanged"
    );
}

#[test]
fn ui_window_delegates_footer_surface_to_app_view_contract() {
    let body = function_body(UI_WINDOW_SOURCE, "fn main_window_footer_surface");
    assert!(
        body.contains("self.current_view.native_footer_surface()"),
        "ui_window must delegate footer surface identity to AppView::native_footer_surface"
    );
    assert!(
        !body.contains("match &self.current_view"),
        "ui_window must not duplicate the AppView footer surface map"
    );
}

#[test]
fn main_window_footer_config_exposes_slot_model_contract() {
    for needle in [
        "pub(crate) const MAIN_WINDOW_FOOTER_MAX_ACTION_SLOTS: usize = 3",
        "pub(crate) enum FooterSlotRole",
        "ActionSlot",
        "ContextChip",
        "pub(crate) struct MainWindowFooterSlotModel",
        "pub action_slot_count: usize",
        "pub context_chip_count: usize",
        "pub duplicate_shortcut_keys: Vec<String>",
        "pub violation: Option<&'static str>",
        "pub(crate) fn footer_button_slot_role",
        "pub(crate) fn slot_model(&self) -> MainWindowFooterSlotModel",
        "pub(crate) fn slot_contract_violation(&self) -> Option<&'static str>",
    ] {
        assert!(
            FOOTER_POPUP_SOURCE.contains(needle),
            "footer config must expose semantic slot-model contract: {needle}"
        );
    }

    assert!(
        FOOTER_POPUP_SOURCE.contains("let model = config.slot_model();")
            && FOOTER_POPUP_SOURCE.contains("debug_assert!(")
            && FOOTER_POPUP_SOURCE.contains("tracing::warn!")
            && FOOTER_POPUP_SOURCE.contains("duplicate_shortcut_keys"),
        "MainWindowFooterConfig::new must audit the slot model without mutating button availability"
    );
}

#[test]
fn footer_context_chips_do_not_count_as_action_slots() {
    let role_body = function_body(FOOTER_POPUP_SOURCE, "pub(crate) fn footer_button_slot_role");
    assert!(
        role_body.contains("FooterAction::Cwd | FooterAction::AgentModel")
            && role_body.contains("FooterSlotRole::ContextChip")
            && role_body.contains("matches!(button.action, FooterAction::Ai)")
            && role_body.contains("crate::components::footer_chrome::FOOTER_MIC_ICON_TOKEN")
            && role_body.contains("FooterSlotRole::ActionSlot"),
        "cwd, agent/model, and mic-key AI footer entries must be context chips, not action slots"
    );

    let slot_model_body = function_body(FOOTER_POPUP_SOURCE, "pub(crate) fn slot_model(");
    assert!(
        slot_model_body.contains("FooterSlotRole::ActionSlot")
            && slot_model_body.contains("action_slot_count += 1;")
            && slot_model_body.contains("FooterSlotRole::ContextChip")
            && slot_model_body.contains("context_chip_count += 1;")
            && slot_model_body.contains("too_many_action_slots")
            && slot_model_body.contains("duplicate_shortcut_keys"),
        "slot_model must count action slots separately from context chips and report violations"
    );
}

#[test]
fn cwd_and_agent_model_are_not_rendered_as_main_window_footer_chips() {
    assert!(
        !UI_WINDOW_SOURCE.contains("FooterButtonConfig::new(FooterAction::Cwd")
            && !UI_WINDOW_SOURCE.contains("FooterButtonConfig::new(FooterAction::AgentModel"),
        "cwd and Agent/model are shared-header controls and must not be duplicated as native footer buttons"
    );
    assert!(
        !UI_WINDOW_SOURCE.contains("prepend_global_main_window_left_chips")
            && !UI_WINDOW_SOURCE.contains("global_main_window_left_chip_buttons")
            && !UI_WINDOW_SOURCE.contains("current_view_shows_global_left_chips"),
        "main-window footer config must not have a global cwd/model prepending path"
    );
    assert!(
        function_body(
            UI_WINDOW_SOURCE,
            "pub(crate) fn enrich_footer_config_with_agent_chat_info"
        )
        .contains("config.left_info = None;"),
        "Agent Chat footer enrichment must suppress legacy left-info cwd/model markers"
    );
}

#[test]
fn live_dictation_overlay_does_not_join_main_window_footer_ownership() {
    let footer_map = function_body(APP_VIEW_STATE_SOURCE, "pub(crate) fn native_footer_surface");
    assert!(
        footer_map.contains("AppView::DictationHistoryView { .. } => Some(\"dictation_history\")"),
        "DictationHistoryView is the main-window dictation history footer surface"
    );
    assert!(
        !footer_map.contains("dictation_overlay"),
        "Live DictationOverlay must not be owned by AppView::native_footer_surface"
    );
    assert!(
        DICTATION_WINDOW_SOURCE.contains("DICTATION_OVERLAY_FOOTER_SURFACE")
            && DICTATION_WINDOW_SOURCE.contains("\"dictation_overlay\""),
        "Dictation overlay may keep a local footer identity for tests/logging"
    );
    assert!(
        DICTATION_WINDOW_SOURCE.contains("dictation_footer_action_channel")
            && DICTATION_WINDOW_SOURCE.contains("MainWindowFooterConfig")
            && !DICTATION_WINDOW_SOURCE.contains("active_main_window_footer_surface")
            && DICTATION_WINDOW_SOURCE.contains("FooterAction::Stop")
            && DICTATION_WINDOW_SOURCE.contains("FooterAction::Close"),
        "Dictation overlay must reuse the native footer renderer without joining main-window surface ownership"
    );
}

#[test]
fn gpui_footer_overlay_is_default_and_keeps_native_material_as_background_only() {
    assert!(
        FOOTER_POPUP_SOURCE.contains("struct GpuiFooterOverlay")
            && FOOTER_POPUP_SOURCE.contains("render_footer_hint_content_flex(")
            && FOOTER_POPUP_SOURCE.contains("left_pinned_buttons")
            && FOOTER_POPUP_SOURCE.contains("trailing_buttons")
            && FOOTER_POPUP_SOURCE.contains("render_left_info")
            && FOOTER_POPUP_SOURCE.contains("SCRIPT_KIT_GPUI_FOOTER_OVERLAY"),
        "footer overlay must be explicit, config-driven, and reuse the GPUI footer chrome renderer"
    );
    // Default-on promotion: the overlay renders unless explicitly disabled.
    let enabled_fn = function_body(FOOTER_POPUP_SOURCE, "fn gpui_footer_overlay_enabled");
    assert!(
        enabled_fn.contains("SCRIPT_KIT_GPUI_FOOTER_OVERLAY")
            && enabled_fn.contains(".unwrap_or(true)"),
        "GPUI footer overlay must be the default main-window footer renderer with an explicit opt-out"
    );
    assert!(
        FOOTER_POPUP_SOURCE.contains("WindowBackgroundAppearance::Transparent")
            && FOOTER_POPUP_SOURCE.contains("attach_inline_popup_to_parent_window")
            && FOOTER_POPUP_SOURCE.contains("setIgnoresMouseEvents: NO")
            && FOOTER_POPUP_SOURCE.contains("setBecomesKeyOnlyIfNeeded: YES")
            && FOOTER_POPUP_SOURCE.contains("send_footer_action_to_channel(action, false)"),
        "GPUI footer overlay must be a transparent child surface that owns hover/click without stealing key focus"
    );
    assert!(
        FOOTER_POPUP_SOURCE.contains("layout_footer_hints(hints_view, text_color, &[], &theme)")
            && FOOTER_POPUP_SOURCE
                .contains("layout_footer_hints(hints_view, text_color, &config.buttons, &theme)"),
        "AppKit footer text must be bypassed only while the GPUI overlay owns the glyphs"
    );
    assert!(
        FOOTER_POPUP_SOURCE.contains("layout_footer_left_info(left_info_view, None, text_color)")
            && FOOTER_POPUP_SOURCE.contains(
                "layout_footer_left_info(left_info_view, config.left_info.as_ref(), text_color)"
            ),
        "AppKit left-info text must also be bypassed only while GPUI overlay owns footer visuals"
    );
    // Non-main footer hosts (detached Agent Chat, dictation overlay) have no
    // GPUI overlay child window, so they must keep native glyph rendering.
    let window_sync = function_body(
        FOOTER_POPUP_SOURCE,
        "pub(crate) fn sync_window_footer_popup",
    );
    assert!(
        window_sync.contains("refresh_window_footer_host(ns_window, config)"),
        "reusable window footer hosts must render glyphs natively, not defer to the main-window GPUI overlay"
    );
    assert!(
        function_body(FOOTER_POPUP_SOURCE, "unsafe fn refresh_window_footer_host")
            .contains("reusable_window_footer_search_root(ns_window)")
            && function_body(FOOTER_POPUP_SOURCE, "unsafe fn refresh_window_footer_host")
                .contains("config,\n        false,"),
        "refresh_window_footer_host must never blank AppKit glyphs"
    );
    // Flexbox sizing contract: buttons take intrinsic (text-measured) width
    // with a per-action slot minimum; only the Run slot is capped; the left
    // group absorbs shrink pressure (flex_1 + min_w 0 + overflow_hidden) so
    // groups can never overlap. No estimated character widths.
    assert!(
        FOOTER_POPUP_SOURCE.contains(".min_w(px(min_width))")
            && FOOTER_POPUP_SOURCE.contains("FOOTER_RUN_SLOT_MAX_WIDTH_PX")
            && FOOTER_POPUP_SOURCE.contains(".min_w(px(0.0))")
            && FOOTER_POPUP_SOURCE.contains(".overflow_hidden()")
            && FOOTER_POPUP_SOURCE.contains(".flex_1()")
            && FOOTER_POPUP_SOURCE.contains(".flex_none()")
            && !FOOTER_POPUP_SOURCE.contains("footer_hint_content_estimated_width_px"),
        "GPUI footer overlay buttons must size intrinsically via flexbox with slot minimums, never via estimated text widths"
    );
}
