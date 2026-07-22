// --- Expanded-view surfaces (preview IS the decision) ---
const CLIPBOARD_HISTORY_ENTRY_SOURCE: &str = include_str!("../../src/render_builtins/clipboard.rs");
const CLIPBOARD_HISTORY_LAYOUT_SOURCE: &str = CLIPBOARD_HISTORY_ENTRY_SOURCE;
const FILE_SEARCH_ENTRY_SOURCE: &str = include_str!("../../src/render_builtins/file_search.rs");

// File Search rendering is owned by file_search.rs.

// --- Minimal-list surfaces (name IS the content) ---
const WINDOW_SWITCHER_SOURCE: &str = include_str!("../../src/render_builtins/window_switcher.rs");
const APP_LAUNCHER_SOURCE: &str = include_str!("../../src/render_builtins/app_launcher.rs");
const CURRENT_APP_COMMANDS_SOURCE: &str =
    include_str!("../../src/render_builtins/current_app_commands.rs");
const PROCESS_MANAGER_SOURCE: &str = include_str!("../../src/render_builtins/process_manager.rs");
const SELECT_PROMPT_SOURCE: &str = include_str!("../../src/prompts/select/render.rs");
const PATH_PROMPT_SOURCE: &str = include_str!("../../src/prompts/path/render.rs");
const CHAT_PROMPT_SOURCE: &str = include_str!("../../src/prompts/chat/render_core.rs");
const APP_VIEW_STATE_SOURCE: &str = include_str!("../../src/main_sections/app_view_state.rs");

/// Assert that a minimal-list builtin surface uses shared chrome infrastructure.
///
/// Minimal surfaces render name-is-the-content lists and use scaffolds or
/// manual chrome tokens (hint strip, header padding, section dividers).
fn assert_minimal_builtin_surface(name: &str, source: &str) {
    let prompt_footer_needle = ["PromptFooter", "::new("].concat();

    assert!(
        source.contains("render_simple_hint_strip(")
            || source.contains("HintStrip::new(")
            || source.contains("render_minimal_list_prompt_scaffold(")
            || source.contains("render_minimal_list_prompt_shell("),
        "{name} should use a shared minimal chrome helper (hint strip or minimal scaffold)"
    );

    assert!(
        !source.contains(&prompt_footer_needle),
        "{name} should not use PromptFooter::new"
    );

    let uses_scaffold = source.contains("render_minimal_list_prompt_scaffold(")
        || source.contains("render_minimal_list_prompt_shell(");

    if !uses_scaffold {
        assert!(
            source.contains("HEADER_PADDING_X") && source.contains("HEADER_PADDING_Y"),
            "{name} should use shared chrome header padding tokens"
        );

        assert!(
            source.contains("SectionDivider::new()")
                || source.contains("border_t(px(DIVIDER_HEIGHT))")
                || source.contains("border_b(px(DIVIDER_HEIGHT))")
                || source.contains("main_window_footer_slot(")
                || source.contains("render_native_main_window_footer_spacer()"),
            "{name} should use shared minimal chrome structure tokens"
        );
    }
}

/// Assert the expanded-view contract for preview-driven builtins.
///
/// Expanded surfaces show a list + preview split where users cannot
/// confidently select without seeing the content. Per .impeccable.md:
/// - Must use the shared expanded-view scaffold or shell
/// - Must NOT use SectionDivider (spacing defines structure)
/// - Must NOT use PromptFooter (scaffold owns the footer)
/// - Entry file must emit PromptChromeAudit::expanded
fn assert_expanded_builtin_surface(name: &str, entry_source: &str, layout_source: &str) {
    let prompt_footer_needle = ["PromptFooter", "::new("].concat();
    let divider_needle = ["SectionDivider", "::new()"].concat();

    // Layout must route through the shared expanded-view scaffold or shell
    assert!(
        layout_source.contains("render_expanded_view_scaffold(")
            || layout_source.contains("render_expanded_view_scaffold_with_hints(")
            || layout_source.contains("render_expanded_view_scaffold_with_footer(")
            || layout_source.contains("render_expanded_view_prompt_shell("),
        "{name} layout should use shared expanded-view scaffold/shell"
    );

    // Layout should route footer rendering through the native footer path.
    assert!(
        layout_source.contains("main_window_footer_slot(")
            || layout_source.contains("render_expanded_view_scaffold_with_hints("),
        "{name} layout should route footer rendering through the native footer path"
    );

    // Must NOT use old PromptFooter
    assert!(
        !layout_source.contains(&prompt_footer_needle),
        "{name} layout should not use PromptFooter::new (scaffold/shell owns the footer)"
    );

    // Must NOT use SectionDivider — expanded view uses spacing, not dividers
    assert!(
        !layout_source.contains(&divider_needle),
        "{name} layout should not use SectionDivider — expanded shell uses spacing, not dividers"
    );

    // Entry file must declare expanded layout mode
    assert!(
        entry_source.contains(&format!("PromptChromeAudit::expanded(\"{name}\"")),
        "{name} entry should emit PromptChromeAudit::expanded(\"{name}\"...) for runtime observability"
    );

    // Entry file must NOT still claim minimal layout
    assert!(
        !entry_source.contains("PromptChromeAudit::minimal("),
        "{name} entry should not emit a minimal chrome audit (it is an expanded surface)"
    );

    eprintln!(
        "{{\"audit\":\"expanded_contract\",\"surface\":\"{name}\",\"scaffold_used\":{},\"footer_slot\":true,\"divider_absent\":true,\"footer_absent\":true,\"layout_mode\":\"expanded\"}}",
        layout_source.contains("render_expanded_view_scaffold(")
            || layout_source.contains("render_expanded_view_scaffold_with_hints(")
            || layout_source.contains("render_expanded_view_scaffold_with_footer(")
            || layout_source.contains("render_expanded_view_prompt_shell(")
    );
}

// ---- Expanded-view contract tests ----

#[test]
fn clipboard_history_enforces_expanded_view_contract() {
    assert_expanded_builtin_surface(
        "clipboard_history",
        CLIPBOARD_HISTORY_ENTRY_SOURCE,
        CLIPBOARD_HISTORY_LAYOUT_SOURCE,
    );
}

#[test]
fn file_search_enforces_expanded_view_contract() {
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("render_main_view_chrome_footer_flush("),
        "file_search should use shared main-view chrome"
    );
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("render_main_view_context_zone("),
        "file_search should keep the shared context zone"
    );
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("render_main_view_input_shell("),
        "file_search should keep the shared input shell"
    );
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("main_window_footer_slot("),
        "file_search should route footer content through the native footer slot"
    );
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("render_simple_hint_strip(file_search_hints, None)"),
        "file_search should build the shared hint strip"
    );
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("PromptChromeAudit::expanded(\"file_search\""),
        "file_search should keep expanded runtime audit classification"
    );
    assert!(
        !FILE_SEARCH_ENTRY_SOURCE.contains("PromptChromeAudit::minimal("),
        "file_search should not emit a minimal chrome audit"
    );
    let divider_needle = ["SectionDivider", "::new()"].concat();
    assert!(
        !FILE_SEARCH_ENTRY_SOURCE.contains(&divider_needle),
        "file_search should not use SectionDivider"
    );
}

// ---- File search mini footer contract ----

#[test]
fn file_search_mini_footer_uses_live_hint_contract() {
    // The live rendering is in file_search.rs (entry source).
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("emit_prompt_hint_audit(\"file_search\""),
        "file_search should emit a prompt hint audit"
    );
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("let file_search_hints = if")
            && (FILE_SEARCH_ENTRY_SOURCE.contains("render_minimal_list_prompt_scaffold(")
                || FILE_SEARCH_ENTRY_SOURCE
                    .contains("render_minimal_list_prompt_shell_with_footer(")),
        "file_search mini mode should compute live hints and route them through a minimal scaffold"
    );
}

// ---- Minimal-list contract tests ----

#[test]
fn window_switcher_uses_minimal_prompt_chrome() {
    assert_minimal_builtin_surface("window_switcher", WINDOW_SWITCHER_SOURCE);
}

#[test]
fn app_launcher_uses_minimal_prompt_chrome() {
    assert_minimal_builtin_surface("app_launcher", APP_LAUNCHER_SOURCE);
}

#[test]
fn current_app_commands_uses_minimal_prompt_chrome() {
    assert_minimal_builtin_surface("current_app_commands", CURRENT_APP_COMMANDS_SOURCE);
}

#[test]
fn process_manager_uses_minimal_prompt_chrome() {
    assert_minimal_builtin_surface("process_manager", PROCESS_MANAGER_SOURCE);
}

#[test]
fn launcher_family_uniform_lists_use_vendor_scrollbars() {
    for (label, source) in [
        ("app_launcher", APP_LAUNCHER_SOURCE),
        ("window_switcher", WINDOW_SWITCHER_SOURCE),
        (
            "browser_tabs",
            include_str!("../../src/render_builtins/browser_tabs.rs"),
        ),
        ("current_app_commands", CURRENT_APP_COMMANDS_SOURCE),
    ] {
        assert!(
            source.contains("builtin_uniform_list_scrollbar("),
            "{label} should attach the shared vendor scrollbar helper"
        );
    }
}

// ---- Actions parity contract tests ----
// Ensures footer hints are truthful: surfaces that advertise ⌘K Actions must
// actually wire the shared ActionsDialog, and surfaces without actions must not
// advertise them.

const EMOJI_PICKER_SOURCE: &str = include_str!("../../src/render_builtins/emoji_picker.rs");
const THEME_CHOOSER_SOURCE: &str = include_str!("../../src/render_builtins/theme_chooser.rs");
const ACTIONS_DIALOG_SOURCE: &str = include_str!("../../src/app_impl/actions_dialog.rs");

fn production_source(source: &str) -> &str {
    source.split("#[cfg(test)]").next().unwrap_or(source)
}

/// Surfaces that advertise universal_prompt_hints() must also wire the shared
/// actions dialog (either locally via route_key_to_actions_dialog or by being
/// listed as SharedDialog in actions_support_for_view).
#[test]
fn surfaces_that_advertise_universal_hints_must_have_actions_support() {
    for (source, label) in [(EMOJI_PICKER_SOURCE, "emoji_picker")] {
        let advertises_actions = source.contains("universal_prompt_hints()");
        // Either the surface locally wires the router, or the canonical resolver
        // maps it to SharedDialog (checked via the ActionsDialogHost variant name).
        let has_local_router = source.contains("route_key_to_actions_dialog(");
        let variant_name = match label {
            "app_launcher" => "AppLauncher",
            "window_switcher" => "WindowSwitcher",
            "emoji_picker" => "EmojiPicker",
            "current_app_commands" => "CurrentAppCommands",
            "process_manager" => "ProcessManager",
            _ => "",
        };
        let in_canonical_resolver =
            ACTIONS_DIALOG_SOURCE.contains(&format!("ActionsDialogHost::{variant_name}"));

        assert!(
            !advertises_actions || has_local_router || in_canonical_resolver,
            "{label} advertises universal hints but has no shared actions routing"
        );
    }
}

/// Surfaces explicitly excluded from shared actions must not use universal hints.
#[test]
fn no_actions_surfaces_must_not_advertise_universal_hints() {
    for (source, label) in [
        (APP_LAUNCHER_SOURCE, "app_launcher"),
        (WINDOW_SWITCHER_SOURCE, "window_switcher"),
        (CURRENT_APP_COMMANDS_SOURCE, "current_app_commands"),
        (PROCESS_MANAGER_SOURCE, "process_manager"),
    ] {
        let source = production_source(source);
        assert!(
            !source.contains("universal_prompt_hints()"),
            "{label} does not support shared actions but advertises universal hints"
        );
    }
}

/// ThemeChooser stays truthful: dedicated actions hints, custom footer only.
#[test]
fn theme_chooser_stays_truthful_and_advertises_dedicated_actions() {
    let source = production_source(THEME_CHOOSER_SOURCE);
    assert!(
        !source.contains("universal_prompt_hints()"),
        "theme_chooser should not use universal actions hints"
    );
    assert!(
        source.contains(r#"SharedString::from("⌘K Actions")"#),
        "theme_chooser should advertise its dedicated actions catalog"
    );
    assert!(
        source.contains("render_simple_hint_strip(")
            || source.contains("render_minimal_list_prompt_scaffold("),
        "theme_chooser should keep a truthful custom footer"
    );
}

/// AppLauncher must NOT be in the SharedDialog path — it has no Cmd+K interceptor arm.
#[test]
fn app_launcher_excluded_from_shared_actions_resolver() {
    let resolver_start = ACTIONS_DIALOG_SOURCE
        .find("fn actions_support_for_view")
        .expect("canonical resolver not found");
    let resolver_fn = &ACTIONS_DIALOG_SOURCE[resolver_start..];

    assert!(
        !resolver_fn.contains("ActionsSupport::SharedDialog(ActionsDialogHost::AppLauncher)"),
        "app_launcher should not be mapped to SharedDialog (no Cmd+K toggle exists)"
    );
}

/// ThemeChooser must be in the canonical host path through its dedicated host.
#[test]
fn theme_chooser_included_in_shared_actions_resolver() {
    let resolver_start = ACTIONS_DIALOG_SOURCE
        .find("fn actions_host_for_view")
        .expect("canonical host resolver not found");
    let resolver_fn = &ACTIONS_DIALOG_SOURCE[resolver_start..];

    assert!(
        resolver_fn
            .contains("AppView::ThemeChooserView { .. } => Some(ActionsDialogHost::ThemeChooser)"),
        "theme_chooser should be mapped to its dedicated host"
    );
}

/// Emoji picker must have both: universal hints AND actual actions wiring.
#[test]
fn emoji_picker_advertises_and_wires_shared_actions() {
    assert!(
        EMOJI_PICKER_SOURCE.contains("universal_prompt_hints()"),
        "emoji_picker should use universal hints (it supports shared actions)"
    );
    assert!(
        EMOJI_PICKER_SOURCE.contains("route_key_to_actions_dialog("),
        "emoji_picker must locally wire the shared actions router"
    );
}

// ---- Native footer contract tests ----

const UI_WINDOW_SOURCE: &str = include_str!("../../src/app_impl/ui_window.rs");
const FOOTER_POPUP_SOURCE: &str = include_str!("../../src/footer_popup.rs");
const ACTIONS_DIALOG_RENDER_SOURCE: &str = include_str!("../../src/actions/dialog.rs");
const PROMPT_LAYOUT_SHELL_SOURCE: &str =
    include_str!("../../src/components/prompt_layout_shell.rs");
const RENDER_SCRIPT_LIST_SOURCE: &str = include_str!("../../src/render_script_list/mod.rs");
const APP_NAVIGATION_SCROLL_SOURCE: &str = include_str!("../../src/app_navigation/impl_scroll.rs");
const WINDOW_RESIZE_SOURCE: &str = include_str!("../../src/window_resize/mod.rs");
const AI_PRESETS_SOURCE: &str = include_str!("../../src/render_builtins/ai_presets.rs");

#[test]
fn ui_window_resolves_native_footer_from_view() {
    assert!(
        UI_WINDOW_SOURCE.contains("fn main_window_footer_config("),
        "ui_window.rs must define a main-window footer config resolver"
    );
    assert!(
        UI_WINDOW_SOURCE.contains("MainWindowFooterConfig::new("),
        "main-window footer resolver must build MainWindowFooterConfig values"
    );
    assert!(
        UI_WINDOW_SOURCE.contains("main_window_footer_slot("),
        "ui_window.rs must expose a footer-slot helper for GPUI surfaces"
    );
    assert!(
        UI_WINDOW_SOURCE.contains("sync_main_footer_popup(window, config.as_ref(),"),
        "native footer sync must pass the resolved config into footer_popup.rs"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_contract\",\"resolver\":true,\"slot_helper\":true,\"config_sync\":true}}"
    );
}

#[test]
fn footer_popup_accepts_config_driven_refresh() {
    assert!(
        FOOTER_POPUP_SOURCE.contains("struct FooterButtonConfig"),
        "footer_popup.rs must define FooterButtonConfig"
    );
    assert!(
        FOOTER_POPUP_SOURCE.contains("struct MainWindowFooterConfig"),
        "footer_popup.rs must define MainWindowFooterConfig"
    );
    assert!(
        FOOTER_POPUP_SOURCE.contains("config: Option<&MainWindowFooterConfig>"),
        "sync_main_footer_popup must accept Option<&MainWindowFooterConfig>"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_popup_contract\",\"button_config\":true,\"footer_config\":true,\"config_param\":true}}"
    );
}

#[test]
fn native_footer_height_uses_shared_token() {
    assert!(
        WINDOW_RESIZE_SOURCE
            .contains("pub const NATIVE_MAIN_WINDOW_FOOTER_HEIGHT: f32 = HINT_STRIP_HEIGHT;"),
        "window_resize.rs must name the native main-window footer height contract"
    );
    assert!(
        FOOTER_POPUP_SOURCE
            .contains("crate::window_resize::mini_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT as f64"),
        "AppKit native footer host height must use the shared native footer token"
    );
    assert!(
        PROMPT_LAYOUT_SHELL_SOURCE.contains("render_native_main_window_footer_spacer()")
            && PROMPT_LAYOUT_SHELL_SOURCE
                .contains("crate::window_resize::mini_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT"),
        "GPUI native footer spacer must reserve the shared native footer height"
    );
    assert!(
        PROMPT_LAYOUT_SHELL_SOURCE.contains("render_native_main_window_footer_hover_blocker()")
            && PROMPT_LAYOUT_SHELL_SOURCE
                .contains("crate::window_resize::mini_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT")
            && RENDER_SCRIPT_LIST_SOURCE.contains("render_native_main_window_footer_hover_blocker()"),
        "main list footer hover blocker must use the shared native footer height through the shared helper"
    );
    assert!(
        APP_NAVIGATION_SCROLL_SOURCE
            .contains("crate::window_resize::mini_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT"),
        "footer-safe scroll reveal math must use the shared native footer height"
    );
}

#[test]
fn footer_popup_notify_none_removes_native_footer_host() {
    let notify_pos = FOOTER_POPUP_SOURCE
        .find("pub(crate) fn notify_main_footer_popup")
        .expect("footer_popup.rs must define notify_main_footer_popup");
    let notify_section =
        &FOOTER_POPUP_SOURCE[notify_pos..FOOTER_POPUP_SOURCE.len().min(notify_pos + 1200)];

    let some_pos = notify_section
        .find("if let Some(config) = config")
        .expect("notify_main_footer_popup must branch on config");
    let refresh_pos = notify_section
        .find("refresh_main_footer_host(ns_window, config);")
        .expect("notify_main_footer_popup(Some) must refresh the native footer host");
    let else_pos = notify_section
        .find("} else {")
        .expect("notify_main_footer_popup must handle None");
    let clear_pos = notify_section[else_pos..]
        .find("clear_main_window_footer_refresh_signature();")
        .map(|idx| else_pos + idx)
        .expect("notify_main_footer_popup(None) must clear the cached refresh signature");
    let remove_pos = notify_section[else_pos..]
        .find("remove_main_footer_host(ns_window);")
        .map(|idx| else_pos + idx)
        .expect("notify_main_footer_popup(None) must remove the stale native footer host");

    assert!(
        some_pos < refresh_pos
            && refresh_pos < else_pos
            && else_pos < clear_pos
            && clear_pos < remove_pos,
        "notify_main_footer_popup must refresh on Some and clear/remove the stale native footer host on None"
    );
}

#[test]
fn footer_popup_uses_subtle_hover_for_native_footer_buttons() {
    assert!(
        FOOTER_POPUP_SOURCE.contains("let hover_ns: id = ns_color_from_rgba(chrome.hover_rgba);"),
        "native footer hover should use chrome.hover_rgba so the blur stays subtle"
    );
    assert!(
        FOOTER_POPUP_SOURCE
            .contains("let selected_ns: id = ns_color_from_rgba(chrome.selection_rgba);"),
        "native footer selected state should still restore chrome.selection_rgba"
    );
}

#[test]
fn actions_dialog_vibrancy_render_path_leaves_background_to_native_material() {
    assert!(
        ACTIONS_DIALOG_RENDER_SOURCE
            .contains("let use_vibrancy = self.theme.is_vibrancy_enabled();"),
        "ActionsDialog render must resolve the current vibrancy mode"
    );
    assert!(
        ACTIONS_DIALOG_RENDER_SOURCE.contains(".when(!use_vibrancy, |d| d.bg(main_bg))"),
        "ActionsDialog must avoid painting a GPUI steady-state background when native vibrancy owns the material"
    );
}

#[test]
fn footer_popup_refresh_signature_covers_visible_theme_inputs() {
    assert!(
        FOOTER_POPUP_SOURCE.contains("selection_rgba: u32")
            && FOOTER_POPUP_SOURCE.contains("hover_rgba: u32")
            && FOOTER_POPUP_SOURCE.contains("left_dot_hex: Option<u32>"),
        "native footer refresh signature must include selected/hover/dot colors so theme-only changes cannot be skipped"
    );
    assert!(
        FOOTER_POPUP_SOURCE.contains("left_dot_hex = config.left_info.as_ref()")
            && FOOTER_POPUP_SOURCE.contains("footer_dot_hex("),
        "native footer refresh signature must key on the computed status dot color"
    );
    assert!(
        FOOTER_POPUP_SOURCE
            .contains("pub(crate) fn close_main_footer_popup(cx: &mut App) {\n    clear_main_window_footer_refresh_signature();"),
        "closing the native footer must clear the cached refresh signature unconditionally"
    );
}

#[test]
fn builtin_renderers_delegate_native_footer_policy_to_slot_helper() {
    let renderers: &[(&str, &str)] = &[
        (
            "app_launcher",
            include_str!("../../src/render_builtins/app_launcher.rs"),
        ),
        (
            "window_switcher",
            include_str!("../../src/render_builtins/window_switcher.rs"),
        ),
        (
            "browser_tabs",
            include_str!("../../src/render_builtins/browser_tabs.rs"),
        ),
        (
            "theme_chooser",
            include_str!("../../src/render_builtins/theme_chooser.rs"),
        ),
        (
            "process_manager",
            include_str!("../../src/render_builtins/process_manager.rs"),
        ),
        (
            "current_app_commands",
            include_str!("../../src/render_builtins/current_app_commands.rs"),
        ),
        (
            "search_ai_presets",
            include_str!("../../src/render_builtins/ai_presets.rs"),
        ),
        (
            "settings",
            include_str!("../../src/render_builtins/settings.rs"),
        ),
        (
            "favorites",
            include_str!("../../src/render_builtins/favorites.rs"),
        ),
        (
            "design_gallery",
            include_str!("../../src/render_builtins/design_gallery.rs"),
        ),
    ];

    for (name, source) in renderers {
        assert!(
            source.contains("main_window_footer_slot("),
            "{name} must route footer ownership through main_window_footer_slot"
        );
        assert!(
            !source.contains("active_main_window_footer_surface()"),
            "{name} must not duplicate native-footer surface checks in render code"
        );
    }
}

#[test]
fn prompt_renderers_delegate_native_footer_policy_to_prompt_slot_helper() {
    let renderers: &[(&str, &str, &str)] = &[
        ("select_prompt", SELECT_PROMPT_SOURCE, "select_prompt"),
        ("path_prompt", PATH_PROMPT_SOURCE, "path_prompt"),
        ("chat_prompt", CHAT_PROMPT_SOURCE, "chat_prompt"),
    ];

    for (name, source, surface) in renderers {
        let render_code = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(
            render_code.contains("main_window_footer_slot_for_prompt_surface(")
                || render_code.contains("render_main_window_footer_slot_for_prompt_surface("),
            "{name} must route footer ownership through the prompt surface slot helper"
        );
        assert!(
            render_code.contains(&format!("\"{surface}\"")),
            "{name} must pass its registered native footer surface string to the helper"
        );
        assert!(
            !render_code.contains("active_main_window_footer_surface()"),
            "{name} must not call active_main_window_footer_surface directly"
        );
    }
}

#[test]
fn prompt_entity_footer_surface_strings_match_ui_window_map() {
    let cases = [
        (
            "select_prompt",
            "AppView::SelectPrompt { .. } => Some(\"select_prompt\")",
            SELECT_PROMPT_SOURCE,
        ),
        (
            "path_prompt",
            "AppView::PathPrompt { .. } => Some(\"path_prompt\")",
            PATH_PROMPT_SOURCE,
        ),
        (
            "chat_prompt",
            "AppView::ChatPrompt { .. } => Some(\"chat_prompt\")",
            CHAT_PROMPT_SOURCE,
        ),
    ];

    for (surface, app_view_mapping, renderer_source) in cases {
        assert!(
            APP_VIEW_STATE_SOURCE.contains(app_view_mapping),
            "{surface} must be registered in AppView::native_footer_surface"
        );
        assert!(
            renderer_source.contains(&format!("\"{surface}\"")),
            "{surface} renderer must use the same native footer surface string"
        );
    }
}

#[test]
fn create_ai_preset_form_does_not_inherit_launcher_native_footer() {
    assert!(
        APP_VIEW_STATE_SOURCE
            .contains("AppView::SearchAiPresetsView { .. } => Some(\"search_ai_presets\")"),
        "search presets list keeps the native footer surface"
    );
    assert!(
        !APP_VIEW_STATE_SOURCE
            .contains("AppView::CreateAiPresetView { .. } => Some(\"create_ai_preset\")"),
        "create preset form must not inherit the generic launcher native footer"
    );
    assert!(
        !AI_PRESETS_SOURCE.contains("Some(\"create_ai_preset\")"),
        "create preset renderer must not reserve native footer space"
    );
}

#[test]
fn prompt_footer_exception_builtins_do_not_register_native_footer() {
    let surfaces: [&str; 0] = [];
    for surface in surfaces {
        assert!(
            !UI_WINDOW_SOURCE.contains(&format!("Some(\"{surface}\")")),
            "{surface} must not register a native footer while it owns PromptFooter actions"
        );
    }
}

#[test]
fn design_gallery_routes_footer_through_native_slot() {
    let design_gallery = include_str!("../../src/render_builtins/design_gallery.rs");
    assert!(
        !design_gallery.contains("PromptFooter::new("),
        "design gallery should not keep an in-content PromptFooter once it registers a native footer surface"
    );
    assert!(
        design_gallery.contains("main_window_footer_slot(")
            && design_gallery.contains("render_simple_hint_strip(")
            && design_gallery.contains("emit_surface_prompt_hint_audit(")
            && design_gallery.contains("\"↵ Select\"")
            && !design_gallery.contains("active_main_window_footer_surface()")
            && APP_VIEW_STATE_SOURCE.contains("Some(\"design_gallery\")"),
        "design gallery should register a native footer surface and route fallback hints through the shared slot"
    );
}

#[test]
fn kit_store_routes_footer_through_native_slot() {
    let kit_store = include_str!("../../src/render_builtins/kit_store.rs");
    assert!(
        !kit_store.contains("PromptFooter::new("),
        "kit store should not keep an in-content PromptFooter once it registers native footer surfaces"
    );
    assert!(
        kit_store.contains("main_window_footer_slot(")
            && kit_store.contains("render_simple_hint_strip(")
            && kit_store.contains("emit_surface_prompt_hint_audit(")
            && !kit_store.contains("PromptChromeAudit::exception(")
            && APP_VIEW_STATE_SOURCE.contains("Some(\"kit_store_browse\")")
            && APP_VIEW_STATE_SOURCE.contains("Some(\"kit_store_installed\")"),
        "kit store should register native footer surfaces and route fallback hints through the shared slot"
    );
}

#[test]
fn script_list_routes_footer_through_native_slot() {
    let source = include_str!("../../src/render_script_list/mod.rs");
    assert!(
        source.contains("main_window_footer_slot("),
        "render_script_list should route footer rendering through the main-window native footer slot"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_surface\",\"surface\":\"script_list\",\"footer_slot\":true}}"
    );
}

#[test]
fn prompt_wrapper_routes_footer_through_native_slot() {
    let source = include_str!("../../src/render_prompts/other.rs");
    assert!(
        source.contains("main_window_footer_slot("),
        "render_wrapped_prompt_entity should route footer rendering through the main-window native footer slot"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_surface\",\"surface\":\"prompt_wrapper\",\"footer_slot\":true}}"
    );
}

#[test]
fn clipboard_history_routes_footer_through_native_slot() {
    assert!(
        CLIPBOARD_HISTORY_LAYOUT_SOURCE.contains("render_expanded_view_scaffold_with_footer("),
        "clipboard_history should use the footer-aware expanded scaffold"
    );
    assert!(
        CLIPBOARD_HISTORY_LAYOUT_SOURCE.contains("main_window_footer_slot("),
        "clipboard_history should route footer rendering through the main-window native footer slot"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_surface\",\"surface\":\"clipboard_history\",\"footer_slot\":true}}"
    );
}

#[test]
fn file_search_routes_footer_through_native_slot() {
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("main_window_footer_slot("),
        "file_search should route footer rendering through the main-window native footer slot"
    );
    assert!(
        FILE_SEARCH_ENTRY_SOURCE.contains("render_expanded_view_scaffold_with_footer(")
            || FILE_SEARCH_ENTRY_SOURCE.contains("render_minimal_list_prompt_shell_with_footer("),
        "file_search should use footer-aware scaffold helpers"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_surface\",\"surface\":\"file_search\",\"footer_slot\":true}}"
    );
}

#[test]
fn mini_prompt_routes_footer_through_native_slot() {
    let source = include_str!("../../src/render_prompts/mini.rs");
    assert!(
        source.contains("main_window_footer_slot("),
        "mini_prompt should route footer rendering through the main-window native footer slot"
    );
    assert!(
        source.contains("render_minimal_list_prompt_shell_with_footer("),
        "mini_prompt should use the footer-aware minimal list prompt shell"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_surface\",\"surface\":\"mini_prompt\",\"footer_slot\":true}}"
    );
}

#[test]
fn div_prompt_routes_footer_through_native_slot() {
    let source = include_str!("../../src/render_prompts/div.rs");
    assert!(
        source.contains("main_window_footer_slot("),
        "div_prompt should route footer rendering through the main-window native footer slot"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_surface\",\"surface\":\"div_prompt\",\"footer_slot\":true}}"
    );
}

#[test]
fn form_prompt_routes_footer_through_native_slot() {
    let source = include_str!("../../src/render_prompts/form/render.rs");
    assert!(
        source.contains("main_window_footer_slot("),
        "form_prompt should route footer rendering through the main-window native footer slot"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_surface\",\"surface\":\"form_prompt\",\"footer_slot\":true}}"
    );
}

#[test]
fn editor_prompt_routes_footer_through_native_slot() {
    let source = include_str!("../../src/render_prompts/editor.rs");
    assert!(
        source.contains("main_window_footer_slot("),
        "editor_prompt should route footer rendering through the main-window native footer slot"
    );

    eprintln!(
        "{{\"audit\":\"native_footer_surface\",\"surface\":\"editor_prompt\",\"footer_slot\":true}}"
    );
}

/// Summary audit: verifies all migrated surfaces route through the native footer
/// slot, emitting a single JSON envelope listing all covered surfaces.
#[test]
fn native_footer_migration_coverage_summary() {
    let surfaces: &[(&str, &str)] = &[
        (
            "script_list",
            include_str!("../../src/render_script_list/mod.rs"),
        ),
        (
            "prompt_wrapper",
            include_str!("../../src/render_prompts/other.rs"),
        ),
        (
            "clipboard_history",
            include_str!("../../src/render_builtins/clipboard.rs"),
        ),
        (
            "file_search",
            include_str!("../../src/render_builtins/file_search.rs"),
        ),
        (
            "mini_prompt",
            include_str!("../../src/render_prompts/mini.rs"),
        ),
        (
            "div_prompt",
            include_str!("../../src/render_prompts/div.rs"),
        ),
        (
            "form_prompt",
            include_str!("../../src/render_prompts/form/render.rs"),
        ),
        (
            "editor_prompt",
            include_str!("../../src/render_prompts/editor.rs"),
        ),
    ];

    let mut covered = Vec::new();
    for (name, source) in surfaces {
        assert!(
            source.contains("main_window_footer_slot(")
                || source.contains("render_expanded_view_scaffold_with_hints("),
            "{name} must route footer through the native footer path"
        );
        covered.push(*name);
    }

    let covered_json: Vec<String> = covered.iter().map(|s| format!("\"{}\"", s)).collect();
    eprintln!(
        "{{\"audit\":\"native_footer_migration_summary\",\"covered_surfaces\":[{}],\"total\":{}}}",
        covered_json.join(","),
        covered.len(),
    );
}

/// The canonical resolver must list all ActionsDialogHost variants that have
/// focus-restore mappings, ensuring the two stay in sync.
#[test]
fn canonical_resolver_covers_all_focus_restore_hosts() {
    let resolver_start = ACTIONS_DIALOG_SOURCE
        .find("fn actions_support_for_view")
        .expect("canonical resolver not found");
    let resolver_fn = &ACTIONS_DIALOG_SOURCE[resolver_start..];

    for host in [
        "ActionsDialogHost::MainList",
        "ActionsDialogHost::ClipboardHistory",
        "ActionsDialogHost::EmojiPicker",
        "ActionsDialogHost::FileSearch",
        "ActionsDialogHost::ThemeChooser",
        "ActionsDialogHost::ChatPrompt",
        "ActionsDialogHost::ArgPrompt",
        "ActionsDialogHost::WebcamPrompt",
        "ActionsDialogHost::AgentChat",
    ] {
        assert!(
            resolver_fn.contains(host),
            "canonical actions resolver should map {host}"
        );
    }
}
