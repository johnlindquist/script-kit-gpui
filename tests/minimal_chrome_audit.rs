//! Source-audit tests for the minimal chrome migration.
//!
//! These tests verify that migrated prompt surfaces use the correct shared
//! layout contract:
//! - Minimal-list surfaces: shared scaffold/shell with hint strip footer
//! - Persistent main-window search surfaces: shared main-view chrome with stable
//!   header, input, main, and footer slots
//!
//! Clipboard history and file search are split-preview surfaces per
//! `.impeccable.md` (preview IS the decision — users can't pick without
//! seeing the content), but their persistent Chrome belongs to MainViewChrome.

fn production_source(source: &'static str) -> &'static str {
    source
        .split("#[cfg(test)]")
        .next()
        .expect("production source should exist")
}

#[test]
fn arg_prompt_uses_shared_minimal_list_shell() {
    let source = include_str!("../src/render_prompts/arg/render.rs");
    assert!(
        source.contains("render_minimal_list_prompt_shell_with_footer("),
        "arg prompt should delegate layout to the footer-aware shared minimal list prompt shell"
    );
    assert!(
        source.contains("main_window_footer_slot("),
        "arg prompt should route its GPUI footer through main_window_footer_slot"
    );
    assert!(
        !source.contains("ALPHA_DIVIDER"),
        "arg prompt should not use inline ALPHA_DIVIDER constant for its list divider"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"arg\",\"shell\":\"minimal_list_with_footer\",\"status\":\"pass\"}}");
}

#[test]
fn clipboard_history_uses_shared_main_view_chrome_contract() {
    let entry_source = production_source(include_str!("../src/render_builtins/clipboard.rs"));
    let layout_source = entry_source;

    // Entry file must declare expanded layout mode
    assert!(
        entry_source.contains("PromptChromeAudit::expanded(\"clipboard_history\""),
        "clipboard history entry should emit PromptChromeAudit::expanded for runtime audit"
    );
    assert!(
        !entry_source.contains("PromptChromeAudit::minimal("),
        "clipboard history entry should not emit a minimal chrome audit"
    );

    // Layout must not use old PromptFooter
    assert!(
        !layout_source.contains("PromptFooter::new("),
        "clipboard history layout should not use PromptFooter"
    );

    assert!(
        layout_source.contains("render_main_view_chrome_footer_flush("),
        "clipboard history layout should route through shared main-view chrome"
    );
    assert!(
        layout_source.contains("render_main_view_input_shell("),
        "clipboard history should use the shared MainMenuInput shell"
    );
    assert!(
        layout_source.contains("render_clickable_main_view_context_zone("),
        "clipboard history should keep the shared context zone"
    );
    assert!(
        layout_source.contains("main_window_footer_slot("),
        "clipboard history footer should route through the native footer slot"
    );
    assert!(
        layout_source.contains("render_simple_hint_strip(hints, None)"),
        "clipboard history should build the shared hint strip before native footer ownership"
    );
    let expanded_scaffold_with_hints = "render_expanded_view_scaffold".to_owned() + "_with_hints(";
    assert!(
        !layout_source.contains(&expanded_scaffold_with_hints),
        "clipboard history should not use the stale expanded scaffold"
    );

    // No SectionDivider — expanded view uses spacing, not dividers
    let divider_call = "SectionDivider".to_owned() + "::new()";
    assert!(
        !layout_source.contains(&divider_call),
        "clipboard history layout should not use SectionDivider — expanded view uses spacing"
    );

    eprintln!("{{\"audit\":\"main_view_chrome_contract\",\"surface\":\"clipboard_history\",\"layout_mode\":\"main_view_chrome\",\"divider_absent\":true,\"footer_shared\":true,\"status\":\"pass\"}}");
}

#[test]
fn emoji_picker_no_longer_uses_prompt_footer() {
    let source = include_str!("../src/render_builtins/emoji_picker.rs");
    assert!(
        !source.contains("PromptFooter::new("),
        "emoji picker should use hint strip, not PromptFooter"
    );
    assert!(
        source.contains("main_window_footer_slot("),
        "emoji picker should route its GPUI footer through main_window_footer_slot"
    );
    assert!(
        source.contains("render_simple_hint_strip("),
        "emoji picker should build its hint strip via render_simple_hint_strip"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"emoji_picker\",\"shell\":\"native_footer_slot\",\"status\":\"pass\"}}");
}

#[test]
fn builtin_main_input_surfaces_use_shared_input_chrome() {
    let common = production_source(include_str!("../src/render_builtins/common.rs"));
    assert!(
        common.contains("fn render_builtin_main_input_shell(")
            && common.contains("render_main_view_input_shell(")
            && common.contains("fn render_builtin_main_input_header(")
            && common.contains("fn render_builtin_main_input_surface("),
        "common built-in renderer should own the shared main input shell adapters"
    );
    assert!(
        common.contains("render_generic_filterable_search_surface(")
            && common.contains("render_builtin_main_input_surface("),
        "generic filterable built-ins should inherit the shared main input surface"
    );

    for (surface, source) in [
        (
            "app_launcher",
            production_source(include_str!("../src/render_builtins/app_launcher.rs")),
        ),
        (
            "browser_tabs",
            production_source(include_str!("../src/render_builtins/browser_tabs.rs")),
        ),
        (
            "current_app_commands",
            production_source(include_str!(
                "../src/render_builtins/current_app_commands.rs"
            )),
        ),
        (
            "design_gallery",
            production_source(include_str!("../src/render_builtins/design_gallery.rs")),
        ),
        (
            "emoji_picker",
            production_source(include_str!("../src/render_builtins/emoji_picker.rs")),
        ),
        (
            "footer_gallery",
            production_source(include_str!("../src/render_builtins/footer_gallery.rs")),
        ),
        (
            "kit_store",
            production_source(include_str!("../src/render_builtins/kit_store.rs")),
        ),
        (
            "process_manager",
            production_source(include_str!("../src/render_builtins/process_manager.rs")),
        ),
        (
            "settings",
            production_source(include_str!("../src/render_builtins/settings.rs")),
        ),
        (
            "theme_chooser",
            production_source(include_str!("../src/render_builtins/theme_chooser.rs")),
        ),
        (
            "window_switcher",
            production_source(include_str!("../src/render_builtins/window_switcher.rs")),
        ),
    ] {
        assert!(
            source.contains("render_builtin_main_input_header(")
                && source.contains("render_main_view_chrome_footer_flush("),
            "{surface} should route its main input header through shared main-view chrome"
        );
        assert!(
            !source.contains("HEADER_PADDING_X") && !source.contains("HEADER_PADDING_Y"),
            "{surface} should not hardcode local header padding around the main input"
        );
    }

    eprintln!("{{\"audit\":\"main_input_style_parity\",\"surfaces\":[\"emoji_picker\",\"settings\",\"theme_chooser\",\"generic_filterable\"],\"status\":\"pass\"}}");
}

#[test]
fn file_search_uses_shared_main_view_chrome_contract() {
    let entry_source = production_source(include_str!("../src/render_builtins/file_search.rs"));

    // Entry file must declare expanded layout mode
    assert!(
        entry_source.contains("PromptChromeAudit::expanded(\"file_search\""),
        "file search entry should emit PromptChromeAudit::expanded for runtime audit"
    );
    assert!(
        !entry_source.contains("PromptChromeAudit::minimal("),
        "file search entry should not emit a minimal chrome audit"
    );

    assert!(
        entry_source.contains("render_main_view_chrome_footer_flush("),
        "file search should use shared main-view chrome"
    );
    assert!(
        entry_source.contains("render_clickable_main_view_context_zone("),
        "file search should keep the shared context zone"
    );
    assert!(
        entry_source.contains("render_main_view_input_shell("),
        "file search should keep the shared input shell"
    );
    assert!(
        entry_source.contains("main_window_footer_slot("),
        "file search should route footer content through the native footer slot"
    );

    // Must use universal hints and emit hint audit
    assert!(
        entry_source.contains("emit_prompt_hint_audit(\"file_search\""),
        "file search should emit prompt hint audit"
    );

    // No old PromptFooter
    assert!(
        !entry_source.contains("PromptFooter::new("),
        "file search should not use PromptFooter"
    );

    // No SectionDivider — expanded view uses spacing, not dividers
    let divider_call = "SectionDivider".to_owned() + "::new()";
    assert!(
        !entry_source.contains(&divider_call),
        "file search should not use SectionDivider — expanded view uses spacing"
    );

    eprintln!("{{\"audit\":\"main_view_chrome_contract\",\"surface\":\"file_search\",\"footer_slot\":true,\"divider_absent\":true,\"status\":\"pass\"}}");
}

#[test]
fn editor_choice_popup_has_no_box_card_chrome() {
    let source = include_str!("../src/editor/mod.rs");
    let start = source
        .find("fn render_choices_popup")
        .expect("render_choices_popup exists");
    let popup_source = &source[start..];
    assert!(
        !popup_source.contains(".rounded_md()"),
        "editor choice popup should not use rounded card chrome"
    );
    assert!(
        !popup_source.contains(".border_1()"),
        "editor choice popup should not use bordered card chrome"
    );
    assert!(
        popup_source.contains("AppChromeColors::from_theme")
            && popup_source.contains("inline_dropdown_surface_rgba")
            && popup_source.contains("selection_rgba")
            && popup_source.contains("text_primary_hex"),
        "editor choice popup should resolve inline dropdown chrome through AppChromeColors"
    );
    assert!(
        !popup_source.contains("colors.background.main.to_rgb()")
            && !popup_source.contains("colors.accent.selected")
            && !popup_source.contains("colors.text.on_accent"),
        "editor choice popup should avoid raw background/accent/on-accent color paths"
    );
    assert!(
        !popup_source.contains(".cursor_pointer()") || popup_source.contains(".on_mouse_down("),
        "pointer cursor should only be used when choice rows are actually clickable"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"editor_choice_popup\",\"border_absent\":true,\"rounded_absent\":true,\"chrome_tokens\":true,\"status\":\"pass\"}}");
}

#[test]
fn editor_footer_contract_is_single_source() {
    let editor_source = include_str!("../src/editor/mod.rs");
    let wrapper_source = include_str!("../src/render_prompts/editor.rs");
    assert!(
        !editor_source.contains("PromptFooter"),
        "EditorPrompt should not reference PromptFooter; footer ownership belongs to the wrapper"
    );
    assert!(
        !wrapper_source.contains("PromptFooter::new("),
        "editor wrapper should not instantiate PromptFooter"
    );
    assert!(
        wrapper_source.contains("main_window_footer_slot(self.clickable_universal_hint_strip(cx))"),
        "editor wrapper should route universal hints through main_window_footer_slot"
    );
    assert!(
        !wrapper_source.contains("40px fixed height"),
        "editor wrapper comments should not hard-code stale footer height"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"editor_footer\",\"single_source\":true,\"status\":\"pass\"}}");
}

#[test]
fn editor_layout_info_has_editor_content_branch() {
    let source = include_str!("../src/app_layout/build_layout_info.rs");
    assert!(
        source.contains("AppView::EditorPrompt")
            && source.contains("EditorContent")
            && source.contains("LayoutComponentType::Prompt"),
        "build_layout_info should expose an EditorContent prompt branch"
    );
    assert!(
        source.contains("return LayoutInfo"),
        "editor layout branch should return before adding main-menu ScriptList/PreviewPanel components"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"editor_layout_info\",\"editor_content_branch\":true,\"status\":\"pass\"}}");
}

#[test]
fn div_layout_info_has_div_content_branch() {
    let source = include_str!("../src/app_layout/build_layout_info.rs");
    assert!(
        source.contains("AppView::DivPrompt")
            && source.contains("DivContent")
            && source.contains("LayoutComponentType::Prompt"),
        "build_layout_info should expose a DivContent prompt branch"
    );
    let branch_start = source
        .find("AppView::DivPrompt { .. } => (\n                    \"DivContent\"")
        .expect("DivPrompt layout branch exists");
    let prompt_branch = &source[branch_start..];
    let return_idx = prompt_branch
        .find("return LayoutInfo")
        .expect("DivPrompt branch should return layout info before launcher components");
    assert!(
        prompt_branch[..return_idx].contains("content.promptBody")
            && prompt_branch[..return_idx].contains("MATERIAL_SOLID_THEME_TOKEN")
            && prompt_branch[..return_idx].contains("CHROME_LAYER_CONTENT"),
        "DivPrompt content must carry Liquid Glass content-layer visual metadata before returning"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"div_layout_info\",\"div_content_branch\":true,\"status\":\"pass\"}}");
}

#[test]
fn terminal_layout_info_has_terminal_content_branch() {
    let source = include_str!("../src/app_layout/build_layout_info.rs");
    let branch_start = source
        .find("AppView::EditorPrompt")
        .expect("prompt layout branch exists");
    let prompt_branch = &source[branch_start..];
    assert!(
        prompt_branch.contains("AppView::TermPrompt")
            && prompt_branch.contains("AppView::QuickTerminalView")
            && prompt_branch.contains("AppView::ScratchPadView")
            && prompt_branch.contains("TerminalContent")
            && prompt_branch.contains("ScratchPadContent")
            && prompt_branch.contains("LayoutComponentType::Prompt"),
        "terminal/editor utility layout info should expose prompt-owned content for SDK, scratch pad, and quick terminal views"
    );
    assert!(
        source.contains(
            "AppView::ScratchPadView { .. } => crate::window_resize::ViewType::EditorPrompt"
        ) && source.contains(
            "AppView::QuickTerminalView { .. } => crate::window_resize::ViewType::TermPrompt"
        ),
        "utility child content layout receipts should use the same sizing as runtime resize paths"
    );
    assert!(
        prompt_branch.contains("return LayoutInfo"),
        "terminal layout branch should return before adding launcher list/preview components"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"terminal_layout_info\",\"terminal_content_branch\":true,\"status\":\"pass\"}}");
}

#[test]
fn webcam_layout_info_has_webcam_content_branch() {
    let source = include_str!("../src/app_layout/build_layout_info.rs");
    let branch_start = source
        .find("AppView::WebcamView { .. } => (\n                    \"WebcamContent\"")
        .expect("webcam layout branch exists");
    let prompt_branch = &source[branch_start..];
    let return_idx = prompt_branch
        .find("return LayoutInfo")
        .expect("webcam layout branch should return layout info");
    assert!(
        source.contains("AppView::WebcamView { .. } => crate::window_resize::ViewType::DivPrompt"),
        "Webcam layout receipts should use the same DivPrompt sizing as the runtime resize path"
    );
    assert!(
        prompt_branch[..return_idx].contains("WebcamContent")
            && prompt_branch[..return_idx].contains("content.webcamPreview")
            && prompt_branch[..return_idx].contains("LayoutComponentType::Prompt")
            && prompt_branch[..return_idx].contains("MATERIAL_SOLID_THEME_TOKEN")
            && prompt_branch[..return_idx].contains("CHROME_LAYER_CONTENT"),
        "Webcam content must carry content-layer Liquid Glass visual metadata"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"webcam_layout_info\",\"webcam_content_branch\":true,\"status\":\"pass\"}}");
}

#[test]
fn select_drop_layout_info_has_prompt_owned_branches() {
    let source = include_str!("../src/app_layout/build_layout_info.rs");
    let branch_start = source
        .find("AppView::SelectPrompt { .. } | AppView::DropPrompt { .. }")
        .expect("select/drop prompt layout branch exists");
    let prompt_branch = &source[branch_start..];
    let return_idx = prompt_branch
        .find("return LayoutInfo")
        .expect("select/drop layout branch should return layout info");
    assert!(
        prompt_branch[..return_idx].contains("AppView::SelectPrompt")
            && prompt_branch[..return_idx].contains("SelectChoices")
            && prompt_branch[..return_idx].contains("LayoutComponentType::List"),
        "select layout info should report its prompt-owned list instead of launcher list/preview"
    );
    assert!(
        prompt_branch[..return_idx].contains("AppView::DropPrompt")
            && prompt_branch[..return_idx].contains("DropContent")
            && prompt_branch[..return_idx].contains("LayoutComponentType::Prompt"),
        "drop layout info should report prompt-owned drop content instead of launcher list/preview"
    );
    assert!(
        prompt_branch[..return_idx].contains("content.promptChoices")
            && prompt_branch[..return_idx].contains("content.promptDrop")
            && prompt_branch[..return_idx].contains("MATERIAL_SOLID_THEME_TOKEN")
            && prompt_branch[..return_idx].contains("CHROME_LAYER_CONTENT"),
        "select/drop child content must carry content-layer Liquid Glass visual metadata"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"select_drop_layout_info\",\"prompt_owned_branches\":true,\"status\":\"pass\"}}");
}

#[test]
fn select_prompt_search_header_lets_shared_shell_own_padding() {
    let source = production_source(include_str!("../src/prompts/select/render.rs"));
    assert!(
        source.contains("render_minimal_list_prompt_shell_with_footer("),
        "select prompt should keep the footer-aware shared minimal-list shell"
    );
    assert!(
        source.contains("render_select_search_header("),
        "select prompt should render search through the prompt-owned shared-header helper"
    );
    assert!(
        !source.contains("render_search_input(")
            && !source.contains("gpui_input_state")
            && !source.contains("TextInputState"),
        "select prompt must not borrow launcher input ownership"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"select_prompt\",\"search_header\":\"prompt_owned_shared_shell\",\"status\":\"pass\"}}");
}

#[test]
fn prompt_header_search_chrome_is_retired() {
    let components_mod = include_str!("../src/components/mod.rs");
    assert!(
        !components_mod.contains("- [`PromptHeader`]"),
        "components docs should not advertise retired PromptHeader search chrome"
    );
    assert!(
        !components_mod.contains("pub mod prompt_header;"),
        "components should not compile/export the retired PromptHeader module"
    );
    assert!(
        !components_mod.contains("pub use prompt_header::{"),
        "components should not publicly re-export retired PromptHeader types"
    );
    assert!(
        !std::path::Path::new("src/components/prompt_header.rs").exists(),
        "retired PromptHeader root module should be removed"
    );
    assert!(
        !std::path::Path::new("src/components/prompt_header").exists(),
        "retired PromptHeader implementation directory should be removed"
    );
}

#[test]
fn mini_layout_info_reports_single_column_without_preview() {
    let source = include_str!("../src/app_layout/build_layout_info.rs");
    assert!(
        source.contains("let uses_split_preview = matches!(")
            && source.contains("crate::window_resize::ViewType::MainWindow")
            && source.contains("crate::window_resize::ViewType::ScriptList"),
        "layout info must derive split-preview receipts from the view type"
    );
    assert!(
        source.contains("let list_width = if uses_split_preview")
            && source.contains("} else {\n            window_width"),
        "mini layout info must give ScriptList the full window width"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"mini_layout_info\",\"single_column\":true,\"status\":\"pass\"}}");
}

#[test]
fn mini_component_bounds_do_not_emit_preview_panel() {
    let source = include_str!("../src/app_layout/build_component_bounds.rs");
    assert!(
        source.contains(
            "let uses_split_preview = matches!(self.main_window_mode, MainWindowMode::Full);"
        ) && source.contains("let list_width = if uses_split_preview"),
        "debug component bounds must keep mini ScriptList full-width"
    );
    assert!(
        source.contains("if uses_split_preview {") && source.contains("PreviewPanel"),
        "debug component bounds must gate preview panel bounds behind full-mode split preview"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"mini_component_bounds\",\"single_column\":true,\"status\":\"pass\"}}");
}

#[test]
fn header_layout_measurement_uses_shared_main_view_input_width() {
    let layout_info = include_str!("../src/app_layout/build_layout_info.rs");
    assert!(
        layout_info.contains(
            "let input_width = (window_width - (shell_horizontal_padding * 2.0)).max(0.0);"
        ) && !layout_info.contains("LayoutComponentInfo::new(\"RunButton\""),
        "layout info must derive MainViewInput width from shared header padding, not stale RunButton geometry"
    );

    let component_bounds = include_str!("../src/app_layout/build_component_bounds.rs");
    assert!(
        component_bounds.contains("let input_x = px(menu_def.shell.header_padding_x);")
            && component_bounds.contains("let input_width = (width - (input_x * 2.)).max(px(0.));")
            && component_bounds.contains("\"MainViewInput\"")
            && !component_bounds.contains("ComponentBounds::new(\n                    \"Run\""),
        "debug component bounds must derive MainViewInput width from shared header padding"
    );

    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"header_layout\",\"shared_main_view_input\":true,\"status\":\"pass\"}}");
}

#[test]
fn form_prompt_wrapper_has_no_prompt_footer_or_hardcoded_hex() {
    let source = include_str!("../src/render_prompts/form/render.rs");
    assert!(
        !source.contains("PromptFooter::new("),
        "form prompt wrapper should not use PromptFooter"
    );
    assert!(
        !source.contains("rgb(0x"),
        "form prompt wrapper should not contain hardcoded hex rgb colors"
    );
    assert!(
        !source.contains("rgba(0x"),
        "form prompt wrapper should not contain hardcoded hex rgba colors"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"form_prompt\",\"footer_absent\":true,\"hardcoded_hex_absent\":true,\"status\":\"pass\"}}");
}

#[test]
fn form_prompt_fields_use_shared_chrome_and_focus_contracts() {
    let form_source = include_str!("../src/form_prompt.rs");
    let colors_source = include_str!("../src/components/form_fields/colors.rs");
    let text_field_source = include_str!("../src/components/form_fields/text_field/render.rs");
    let text_area_source = include_str!("../src/components/form_fields/text_area/render.rs");
    let text_field_model = include_str!("../src/components/form_fields/text_field/mod.rs");

    assert!(
        colors_source.contains("AppChromeColors::from_theme")
            && colors_source.contains("placeholder_text_rgba"),
        "form field colors should resolve text through shared chrome tokens"
    );
    assert!(
        !colors_source.contains("theme.colors.text.muted")
            && !colors_source.contains("theme.colors.text.secondary"),
        "form fields should not double-dim from muted/secondary base text colors"
    );
    assert!(
        !text_field_source.contains("rgb(colors.text")
            && !text_field_source.contains("rgb(colors.placeholder")
            && !text_field_source.contains("rgb(colors.label")
            && !text_area_source.contains("rgb(colors.text")
            && !text_area_source.contains("rgb(colors.placeholder")
            && !text_area_source.contains("rgb(colors.label"),
        "form field renderers should consume resolved Rgba text colors directly"
    );
    assert!(
        text_area_source.contains("cursor_element")
            && text_area_source.contains("self.cursor_position")
            && text_area_source.contains("slice_by_char_range"),
        "textarea should render a visible char-safe cursor"
    );
    assert!(
        form_source.contains("to_ascii_lowercase()")
            && text_field_model.contains("eq_ignore_ascii_case(\"password\")"),
        "form field type dispatch and password masking should be case-insensitive"
    );
    assert!(
        form_source.contains("focus_field_at")
            && form_source.contains("form-field-slot-")
            && form_source.contains("self.focused_index = index.min"),
        "form prompt field clicks should synchronize parent focused_index"
    );
    assert!(
        !text_field_model.contains("self.value.insert_str(self.cursor_position")
            && !text_field_model.contains("self.value.remove(self.cursor_position)"),
        "text field code must not treat char indices as byte indices"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"form_fields\",\"chrome_tokens\":true,\"cursor_visible\":true,\"focus_sync\":true,\"status\":\"pass\"}}");
}

#[test]
fn path_prompt_render_has_tracing_and_no_hardcoded_hex() {
    let outer_source = include_str!("../src/render_prompts/path.rs");
    let inner_source = include_str!("../src/prompts/path/render.rs");

    assert!(
        outer_source.contains("tracing::info!("),
        "path prompt outer render should emit tracing::info"
    );
    assert!(
        !outer_source.contains("rgb(0x"),
        "path prompt outer render should not contain hardcoded hex rgb colors"
    );
    assert!(
        !inner_source.contains("PromptFooter::new("),
        "path prompt inner render should not use PromptFooter"
    );
    assert!(
        inner_source.contains("prompt_text_palette("),
        "path prompt inner render should use shared prompt text palette"
    );
    assert!(
        !inner_source.contains("<< 8")
            && !inner_source.contains("0x99")
            && !inner_source.contains("0xCC"),
        "path prompt inner render should not build local packed-alpha text colors"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"path\",\"tracing_present\":true,\"hardcoded_hex_absent\":true,\"status\":\"pass\"}}");
}

#[test]
fn path_native_footer_submits_path_prompt_without_launcher_ai() {
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let path_render = include_str!("../src/prompts/path/render.rs");

    let run_start = ui_window
        .find("FooterAction::Run =>")
        .expect("footer Run branch exists");
    let run_branch = &ui_window[run_start..run_start + 2600];
    let footer_branch_start = ui_window
        .find("Resolved PathPrompt footer buttons")
        .expect("PathPrompt native footer branch exists");
    let footer_branch = &ui_window[footer_branch_start.saturating_sub(700)..footer_branch_start];

    assert!(
        run_branch.contains("AppView::PathPrompt")
            && run_branch.contains("prompt.handle_enter(cx)")
            && run_branch.find("prompt.handle_enter(cx)") < run_branch.find("execute_selected(cx)"),
        "PathPrompt native footer Run should submit the path prompt before launcher execute_selected fallback"
    );
    assert!(
        footer_branch.contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Select\")")
            && footer_branch
                .contains("FooterButtonConfig::new(FooterAction::Actions, \"⌘K\", \"Actions\")")
            && !footer_branch.contains("FooterAction::Ai"),
        "PathPrompt native footer should expose Select and Actions, not launcher AI"
    );
    assert!(
        path_render.contains("main_window_footer_slot_for_prompt_surface(")
            && path_render.contains("\"path_prompt\"")
            && !path_render.contains("PromptFooter::new("),
        "PathPrompt entity should route fallback/native footer ownership through the prompt surface slot helper"
    );
    assert!(
        path_render.contains("\"↵ Select\"")
            && path_render.contains("\"⌘K Actions\"")
            && !path_render.contains("universal_prompt_hints()"),
        "PathPrompt GPUI fallback should match native Select/Actions semantics and omit launcher AI"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"path_footer\",\"run_routes_to_path\":true,\"ai_absent\":true,\"status\":\"pass\"}}");
}

#[test]
fn env_footer_submits_env_prompt_without_launcher_ai() {
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let layout_info = include_str!("../src/app_layout/build_layout_info.rs");
    let outer_source = include_str!("../src/render_prompts/other.rs");
    let render_source = include_str!("../src/prompts/env/render.rs");
    let render_code = &render_source[..render_source
        .find("#[cfg(test)]")
        .unwrap_or(render_source.len())];
    let prompt_source = include_str!("../src/prompts/env/prompt.rs");

    let run_start = ui_window
        .find("FooterAction::Run =>")
        .expect("footer Run branch exists");
    let run_end = ui_window[run_start..]
        .find("crate::footer_popup::FooterAction::Actions =>")
        .map(|offset| run_start + offset)
        .expect("footer Actions branch follows Run branch");
    let run_branch = &ui_window[run_start..run_end];
    let footer_branch_start = ui_window
        .find("Resolved EnvPrompt footer buttons")
        .expect("EnvPrompt native footer branch exists");
    let footer_branch = &ui_window[footer_branch_start.saturating_sub(500)..footer_branch_start];
    let sizing_start = ui_window
        .find("pub(crate) fn calculate_window_size_params")
        .expect("calculate_window_size_params exists");
    let sizing = &ui_window[sizing_start..];

    assert!(
        prompt_source.contains("pub(crate) fn submit("),
        "EnvPrompt submit should be callable by wrapper/native footer routing"
    );
    assert!(
        run_branch.contains("AppView::EnvPrompt")
            && run_branch.contains("prompt.submit(cx)")
            && run_branch.find("prompt.submit(cx)") < run_branch.find("execute_selected(cx)"),
        "EnvPrompt native footer Run should submit env before launcher execute_selected fallback"
    );
    assert!(
        footer_branch.contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Submit\")")
            && !footer_branch.contains("FooterAction::Ai"),
        "EnvPrompt native footer should expose Submit without launcher AI"
    );
    assert!(
        outer_source.contains("clickable_env_hint_strip(")
            && outer_source.contains("emit_surface_prompt_hint_audit(")
            && outer_source.contains("env_submit_footer")
            && !outer_source
                .split("fn render_env_prompt(")
                .nth(1)
                .and_then(|section| section.split("fn render_drop_prompt(").next())
                .unwrap_or_default()
                .contains("render_wrapped_prompt_entity("),
        "EnvPrompt wrapper should use a surface-specific Submit footer instead of the universal AI footer"
    );
    assert!(
        sizing.contains("AppView::EnvPrompt { .. } => Some((ViewType::DivPrompt, 0))"),
        "EnvPrompt should size as a form-like DivPrompt surface"
    );
    assert!(
        layout_info
            .contains("AppView::EnvPrompt { .. } => crate::window_resize::ViewType::DivPrompt")
            && layout_info.contains("EnvPromptContent")
            && layout_info.contains("content.explicitEnvPrompt")
            && layout_info.contains("CHROME_LAYER_CONTENT")
            && layout_info.contains("MATERIAL_SOLID_THEME_TOKEN"),
        "EnvPrompt layout info should report explicit prompt content, not launcher list regions"
    );
    assert!(
        render_code.contains("prompt_text_palette(")
            && render_code.contains("prompt_field_style(")
            && !render_code.contains("PromptFooter::new("),
        "EnvPrompt render should keep shared create-flow chrome and avoid PromptFooter"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"env_footer\",\"run_routes_to_env\":true,\"ai_absent\":true,\"status\":\"pass\"}}");
}

#[test]
fn drop_footer_submits_drop_prompt_without_launcher_ai() {
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let outer_source = include_str!("../src/render_prompts/other.rs");
    let prompt_source = include_str!("../src/prompts/drop.rs");

    let run_start = ui_window
        .find("FooterAction::Run =>")
        .expect("footer Run branch exists");
    let run_branch = &ui_window[run_start..run_start + 2600];
    let footer_branch_start = ui_window
        .find("Resolved DropPrompt footer buttons")
        .expect("DropPrompt native footer branch exists");
    let footer_branch = &ui_window[footer_branch_start.saturating_sub(700)..footer_branch_start];
    let sizing_start = ui_window
        .find("pub(crate) fn calculate_window_size_params")
        .expect("calculate_window_size_params exists");
    let sizing = &ui_window[sizing_start..];
    let render_body = outer_source
        .split("fn render_drop_prompt(")
        .nth(1)
        .and_then(|section| section.split("fn render_template_prompt(").next())
        .unwrap_or_default();

    assert!(
        prompt_source.contains("pub(crate) fn submit("),
        "DropPrompt submit should be callable by wrapper/native footer routing"
    );
    assert!(
        run_branch.contains("AppView::DropPrompt")
            && run_branch.contains("prompt.submit()")
            && run_branch.find("prompt.submit()") < run_branch.find("execute_selected(cx)"),
        "DropPrompt native/footer Run should submit dropped files before launcher execute_selected fallback"
    );
    assert!(
        footer_branch.contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Submit\")")
            && footer_branch
                .contains("FooterButtonConfig::new(FooterAction::Actions, \"⌘K\", \"Actions\")")
            && !footer_branch.contains("FooterAction::Ai"),
        "DropPrompt native footer should expose Submit and Actions, not launcher AI"
    );
    assert!(
        outer_source.contains("clickable_drop_hint_strip(")
            && outer_source.contains("emit_surface_prompt_hint_audit(")
            && outer_source.contains("drop_submit_footer")
            && !render_body.contains("universal_prompt_hints()"),
        "DropPrompt should use truthful surface-specific footer hints, not launcher AI"
    );
    assert!(
        render_body.contains("render_wrapped_prompt_entity_with_footer(")
            && outer_source.contains("main_window_footer_slot(")
            && !render_body.contains("PromptFooter::new("),
        "DropPrompt should keep the shared footer slot and avoid prompt-local footers"
    );
    assert!(
        sizing.contains("AppView::DropPrompt { .. } => Some((ViewType::DivPrompt, 0))"),
        "DropPrompt should remain a prompt-owned DivPrompt surface"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"drop_footer\",\"run_routes_to_drop\":true,\"ai_absent\":true,\"status\":\"pass\"}}");
}

#[test]
fn template_prompt_render_has_tracing_and_uses_shared_helpers() {
    let source = include_str!("../src/prompts/template/render.rs");
    let outer_source = include_str!("../src/render_prompts/other.rs");

    assert!(
        outer_source.contains("surface = \"render_prompts::template\""),
        "template prompt outer render should emit tracing with surface tag"
    );
    assert!(
        source.contains("prompt_field_style("),
        "template prompt should use shared prompt_field_style"
    );
    assert!(
        source.contains("prompt_text_palette("),
        "template prompt should use shared prompt_text_palette"
    );
    assert!(
        source.contains("template-fields-scroll") && source.contains(".overflow_y_scrollbar()"),
        "template prompt should keep placeholder fields in a scrollable region"
    );
    assert!(
        outer_source.contains("template_prompt_hints()")
            && outer_source.contains("emit_surface_prompt_hint_audit("),
        "template prompt should use truthful surface-specific footer hints"
    );
    let ui_window_source = include_str!("../src/app_impl/ui_window.rs");
    assert!(
        ui_window_source.contains("AppView::TemplatePrompt")
            && ui_window_source
                .contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Submit\")")
            && ui_window_source
                .contains("FooterButtonConfig::new(FooterAction::Ai, \"⇥\", \"Next Field\")"),
        "native template footer should advertise Submit and Next Field instead of Run/AI"
    );
    assert!(
        !source.contains("rgb(0x"),
        "template prompt should not contain hardcoded hex rgb colors"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"template\",\"tracing_present\":true,\"shared_helpers\":true,\"status\":\"pass\"}}");
}

#[test]
fn drop_prompt_uses_whisper_chrome_not_heavy_borders() {
    let source = include_str!("../src/prompts/drop.rs");

    assert!(
        !source.contains(".border_2()"),
        "drop prompt should not use heavy border_2 chrome"
    );
    let rounded_needle = ".rounded".to_owned() + "(px(8.";
    assert!(
        !source.contains(&rounded_needle),
        "drop prompt should not use rounded corners - whisper chrome means sharp edges"
    );
    assert!(
        !source.contains(".rounded_md()"),
        "drop prompt should not use rounded_md chrome"
    );
    assert!(
        source.contains("AppChromeColors::from_theme(&self.theme)")
            && source.contains("drop_target_bg_rgba")
            && source.contains("drop_target_border_rgba"),
        "drop prompt should use shared AppChromeColors drop-target whisper chrome"
    );
    assert!(
        !source.contains("rgba(0x") && !source.contains("rgb(0x"),
        "drop prompt should not contain hardcoded hex color literals"
    );
    let outer_source = include_str!("../src/render_prompts/other.rs");
    assert!(
        outer_source.contains("surface = \"render_prompts::drop\""),
        "drop prompt outer render should emit tracing with surface tag"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"drop\",\"whisper_chrome\":true,\"tracing_present\":true,\"status\":\"pass\"}}");
}

#[test]
fn term_prompt_uses_chrome_audit_and_no_hardcoded_wrapper_colors() {
    let source = include_str!("../src/render_prompts/term.rs");

    assert!(
        source.contains("PromptChromeAudit::editor("),
        "term prompt should emit the shared chrome audit contract"
    );
    assert!(
        !source.contains("rgb(0x"),
        "term prompt wrapper should not contain hardcoded hex rgb colors"
    );
    assert!(
        !source.contains("PromptFooter::new("),
        "term prompt should not use PromptFooter"
    );
    assert!(
        source.contains("render_terminal_prompt_hint_strip("),
        "term prompt should use terminal-specific hint strip"
    );
    assert!(
        source.contains("\"native_footer_spacer\"")
            && source.contains("\"custom_hint_strip\"")
            && source.contains("\"quick_terminal_uses_native_footer\"")
            && source.contains("\"terminal_owns_contextual_footer\""),
        "term prompt chrome audit should distinguish SDK terminal and Quick Terminal footer ownership"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"term\",\"chrome_audit_present\":true,\"hardcoded_hex_absent\":true,\"status\":\"pass\"}}");
}

#[test]
fn chat_prompt_has_chrome_audit_and_custom_footer_exception() {
    let source = include_str!("../src/render_prompts/other.rs");

    assert!(
        source.contains("surface: \"render_prompts::chat\""),
        "chat prompt should declare the shared chrome audit surface tag"
    );
    // Chat owns its footer — verify no duplicate universal footer is forced in
    let chat_fn_start = source
        .find("fn render_chat_prompt(")
        .expect("chat fn exists");
    let chat_fn_end = source[chat_fn_start..]
        .find("\n    fn ")
        .map(|ix| chat_fn_start + ix)
        .unwrap_or(source.len());
    let chat_fn = &source[chat_fn_start..chat_fn_end];
    assert!(
        chat_fn.contains(", None)"),
        "chat prompt should pass None for footer to avoid duplicate"
    );
    assert!(
        chat_fn.contains("footer_mode: \"custom\""),
        "chat prompt should declare custom footer mode in chrome audit"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"chat\",\"chrome_audit_present\":true,\"custom_footer\":true,\"status\":\"pass\"}}");
}

#[test]
fn webcam_prompt_has_chrome_audit_and_no_redundant_chrome() {
    let source = include_str!("../src/render_prompts/other.rs");

    assert!(
        source.contains("PromptChromeAudit::exception("),
        "webcam prompt should emit the shared chrome audit exception"
    );
    let webcam_fn_start = source
        .find("fn render_webcam_prompt(")
        .expect("webcam fn exists");
    let webcam_fn_end = source[webcam_fn_start..]
        .find("\n    fn ")
        .map(|ix| webcam_fn_start + ix)
        .unwrap_or(source.len());
    let webcam_fn = &source[webcam_fn_start..webcam_fn_end];
    assert!(
        !webcam_fn.contains("PromptFooter::new("),
        "webcam prompt should not use PromptFooter"
    );
    assert!(
        webcam_fn.contains("clickable_webcam_hint_strip("),
        "webcam prompt should use the capture-specific clickable hint strip"
    );
    assert!(
        webcam_fn.contains("universal_prompt_hints_with_primary_label(\"Capture Photo\")")
            && webcam_fn.contains("emit_surface_prompt_hint_audit("),
        "webcam prompt should advertise Capture Photo through surface-specific hint auditing"
    );
    assert!(
        !webcam_fn.contains("rgb(0x"),
        "webcam prompt wrapper should not contain hardcoded hex rgb colors"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"webcam\",\"chrome_audit_present\":true,\"hardcoded_hex_absent\":true,\"status\":\"pass\"}}");
}

#[test]
fn mic_protocol_stays_stubbed_until_media_surface_exists() {
    let execute_script = include_str!("../src/execute_script/mod.rs");
    let prompt_messages = include_str!("../src/main_sections/prompt_messages.rs");
    let prompt_handler = include_str!("../src/prompt_handler/mod.rs");
    let app_view_state = include_str!("../src/main_sections/app_view_state.rs");
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let micro_render = include_str!("../src/render_prompts/micro.rs");
    let micro_fn_start = micro_render
        .find("fn render_micro_prompt(")
        .expect("micro render fn exists");
    let micro_fn_end = micro_render[micro_fn_start..]
        .find("\n    fn ")
        .map(|ix| micro_fn_start + ix)
        .unwrap_or_else(|| {
            micro_render[micro_fn_start..]
                .find("\n#[cfg(test)]")
                .map(|ix| micro_fn_start + ix)
                .unwrap_or(micro_render.len())
        });
    let micro_fn = &micro_render[micro_fn_start..micro_fn_end];

    assert!(
        execute_script.contains("Message::Mic { id }")
            && execute_script.contains("Some(PromptMessage::MicComingSoon { id })"),
        "SDK mic() should stay routed to the explicit coming-soon prompt message until a real media surface exists"
    );
    assert!(
        prompt_messages.contains("MicComingSoon"),
        "mic() should keep a named stub message instead of borrowing the micro prompt"
    );
    assert!(
        prompt_handler.contains("self.show_prompt_coming_soon_toast(\"mic()\", cx)"),
        "mic() should surface as coming-soon feedback rather than launcher footer chrome"
    );
    assert!(
        !app_view_state.contains("MicView") && !ui_window.contains("mic_prompt"),
        "mic() must not expose native-footer surface metadata until a real MicView is implemented"
    );
    assert!(
        micro_fn.contains("\"render_prompts::micro\"")
            && micro_fn.contains("\"ultra_compact_no_footer\"")
            && !micro_fn.contains("PromptFooter::new("),
        "micro() is the ultra-compact text prompt and should not be repurposed as microphone UI"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"mic_stub\",\"mic_coming_soon\":true,\"micro_prompt_distinct\":true,\"status\":\"pass\"}}");
}

#[test]
fn mini_prompt_submits_enter_and_avoids_double_header_padding() {
    let source = include_str!("../src/render_prompts/mini.rs");
    let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
    let render_code = &source[..render_fn_end];
    let header_start = render_code
        .find("let header =")
        .expect("mini header exists");
    let header_end = render_code
        .find("let content =")
        .expect("mini content exists");
    let header = &render_code[header_start..header_end];

    assert!(
        render_code.contains("is_key_enter(key)")
            && render_code.contains("submit_arg_prompt_from_current_state(&prompt_id, cx)"),
        "mini prompt Enter should submit current prompt state like other arg-like prompts"
    );
    assert!(
        !header.contains(".px(px(mini_padding_x))") && !header.contains(".py(px(mini_padding"),
        "mini prompt should let the shared minimal-list shell own header padding"
    );
    assert!(
        render_code.contains("main_window_footer_slot(")
            && render_code.contains("universal_prompt_hints()"),
        "mini prompt should keep shared footer ownership through the native footer slot"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"mini\",\"enter_submit\":true,\"shared_header_padding\":true,\"status\":\"pass\"}}");
}

#[test]
fn micro_prompt_stays_footerless_and_off_native_footer_surface_map() {
    let micro = include_str!("../src/render_prompts/micro.rs");
    let app_view_state = include_str!("../src/main_sections/app_view_state.rs");
    let surface_fn_start = app_view_state
        .find("fn native_footer_surface")
        .expect("native_footer_surface exists");
    let surface_fn_end = app_view_state[surface_fn_start..]
        .find("\n    ///")
        .map(|ix| surface_fn_start + ix)
        .unwrap_or(app_view_state.len());
    let surface_fn = &app_view_state[surface_fn_start..surface_fn_end];

    assert!(
        micro.contains("PromptChromeAudit::exception(")
            && micro.contains("\"ultra_compact_no_footer\"")
            && !micro.contains("main_window_footer_slot("),
        "micro prompt should remain an explicitly footerless ultra-compact surface"
    );
    assert!(
        !surface_fn.contains("AppView::MicroPrompt { .. } => Some("),
        "micro prompt must stay off native footer routing because it does not reserve footer space"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"micro\",\"footerless\":true,\"native_footer_absent\":true,\"status\":\"pass\"}}");
}

#[test]
fn kit_store_footer_uses_single_native_slot_owner() {
    let source = include_str!("../src/render_builtins/kit_store.rs");
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let app_view_state = include_str!("../src/main_sections/app_view_state.rs");
    let app_run_setup = include_str!("../src/main_entry/app_run_setup.rs");
    let runtime_stdin = include_str!("../src/main_entry/runtime_stdin.rs");

    assert!(
        !source.contains("PromptFooter::new("),
        "Kit Store should not render an in-content PromptFooter that can stack with the native footer"
    );
    assert!(
        source.contains("main_window_footer_slot(")
            && source.contains("\"↵ Install\"")
            && source.contains("\"⌫ Remove\""),
        "Kit Store should route fallback hints through the native footer slot"
    );
    assert!(
        !source.contains("PromptChromeAudit::exception(")
            && source.contains("PromptChromeAudit::minimal(")
            && source.contains("emit_surface_prompt_hint_audit("),
        "Kit Store should audit as a native-slot list with surface-specific footer hints"
    );
    assert!(
        app_view_state.contains("AppView::BrowseKitsView { .. } => Some(\"kit_store_browse\")")
            && app_view_state
                .contains("AppView::InstalledKitsView { .. } => Some(\"kit_store_installed\")"),
        "Kit Store views should register native footer surfaces so the fallback strip becomes a spacer"
    );
    assert!(
        ui_window.contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Install\")")
            && ui_window.contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Update\")")
            && ui_window.contains("FooterButtonConfig::new(FooterAction::Close, \"Esc\", secondary_label)")
            && ui_window.contains("\"Back\"")
            && ui_window.contains("\"Clear Search\"")
            && ui_window.contains("FooterButtonConfig::new(FooterAction::Apply, \"⌫\", \"Remove\")"),
        "Kit Store native footer buttons should preserve Install, Back/Clear Search, Update, and Remove semantics"
    );
    assert!(
        ui_window.contains("dispatch_kit_store_primary_footer_action(cx)")
            && ui_window.contains("dispatch_kit_store_browse_back_footer_action(window, cx)")
            && ui_window.contains("dispatch_kit_store_remove_footer_action(cx)"),
        "Kit Store native footer dispatch should call the same selected-row operations as keyboard activation"
    );
    assert!(
        app_run_setup.contains("view.sync_main_footer_popup(window, ctx);\n                    ctx.notify();")
            && runtime_stdin
                .contains("view.sync_main_footer_popup(window, ctx);\n                    ctx.notify();"),
        "stdin-driven view transitions should immediately refresh the native footer before notifying"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"kit_store\",\"single_footer_owner\":true,\"status\":\"pass\"}}");
}

#[test]
fn design_gallery_footer_uses_single_native_slot_owner() {
    let source = include_str!("../src/render_builtins/design_gallery.rs");
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let app_view_state = include_str!("../src/main_sections/app_view_state.rs");

    assert!(
        !source.contains("PromptFooter::new("),
        "Design Gallery should not render an in-content PromptFooter that can stack with the native footer"
    );
    assert!(
        source.contains("main_window_footer_slot(")
            && source.contains("render_simple_hint_strip(")
            && source.contains("\"↵ Select\""),
        "Design Gallery should route its fallback Select hint through the native footer slot"
    );
    assert!(
        !source.contains("active_main_window_footer_surface()"),
        "Design Gallery renderer should delegate native-footer policy to main_window_footer_slot"
    );
    assert!(
        app_view_state.contains("AppView::DesignGalleryView { .. } => Some(\"design_gallery\")"),
        "Design Gallery should register a native footer surface"
    );
    assert!(
        ui_window.contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Select\")"),
        "Design Gallery native footer should preserve the visible Select shortcut"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"design_gallery\",\"single_footer_owner\":true,\"status\":\"pass\"}}");
}

#[test]
fn design_gallery_footer_run_does_not_execute_launcher_selection() {
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let run_start = ui_window
        .find("FooterAction::Run =>")
        .expect("footer Run branch exists");
    let run_branch = &ui_window[run_start..run_start + 2600];
    assert!(
        run_branch.contains("dispatch_design_gallery_select_footer_action(cx)")
            && run_branch.contains("execute_selected(cx)")
            && run_branch.find("dispatch_design_gallery_select_footer_action(cx)")
                < run_branch.find("execute_selected(cx)"),
        "Design Gallery native footer Select must be handled before launcher execute_selected fallback"
    );
}

#[test]
fn design_gallery_native_footer_is_select_only() {
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let branch_end = ui_window
        .find("Resolved Design Gallery footer buttons")
        .expect("Design Gallery footer branch exists");
    let branch = &ui_window[branch_end.saturating_sub(700)..branch_end];
    assert!(
        branch.contains("FooterButtonConfig::new(FooterAction::Run, \"↵\", \"Select\")"),
        "Design Gallery footer should advertise Select"
    );
    assert!(
        !branch.contains("FooterAction::Ai")
            && !branch.contains("FooterAction::Actions")
            && !branch.contains("\"Run\""),
        "Design Gallery footer should not inherit launcher Run/AI/Actions chrome"
    );
}

#[test]
fn design_gallery_sizing_uses_render_projection() {
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let sizing_start = ui_window
        .find("pub(crate) fn calculate_window_size_params")
        .expect("calculate_window_size_params exists");
    let sizing = &ui_window[sizing_start..];
    assert!(
        sizing.contains("AppView::DesignGalleryView { filter, .. }")
            && sizing.contains("design_gallery_filtered_len(filter)"),
        "Design Gallery sizing should use the same filtered row projection as render"
    );
    assert!(
        !sizing.contains("separator_variations::SeparatorStyle::count()"),
        "Design Gallery sizing should not use stale separator/icon-only counts"
    );
}

#[test]
fn footer_gallery_footer_uses_single_native_slot_owner_while_preserving_previews() {
    let source = include_str!("../src/render_builtins/footer_gallery.rs");
    let ui_window = include_str!("../src/app_impl/ui_window.rs");
    let app_view_state = include_str!("../src/main_sections/app_view_state.rs");

    let footer_hints_start = source
        .find("let footer_hints")
        .expect("Footer Gallery fallback footer hints should exist");
    let preview_section = &source[..footer_hints_start];
    let footer_section = &source[footer_hints_start..];

    assert!(
        preview_section.contains("PromptFooter::new(config, footer_colors)")
            && preview_section.contains(".h(px(80.0))"),
        "Footer Gallery must preserve its live 80px PromptFooter preview rows"
    );
    assert!(
        !footer_section.contains("PromptFooter::new("),
        "Footer Gallery should not render a second in-content PromptFooter after footer hints"
    );
    assert!(
        footer_section.contains("main_window_footer_slot(")
            && footer_section.contains("render_simple_hint_strip("),
        "Footer Gallery should route fallback hints through the native footer slot"
    );
    assert!(
        !source.contains("active_main_window_footer_surface()"),
        "Footer Gallery renderer should delegate native-footer policy to main_window_footer_slot"
    );
    assert!(
        app_view_state.contains("AppView::FooterGalleryView { .. } => Some(\"footer_gallery\")"),
        "Footer Gallery should register a native footer surface"
    );
    assert!(
        ui_window.contains("dispatch_footer_gallery_select_footer_action(cx)")
            && ui_window.contains(
                "Footer Gallery native footer Select preserves current no-op selection behavior"
            ),
        "Footer Gallery native footer Select should preserve current no-op selection behavior"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"footer_gallery\",\"single_footer_owner\":true,\"preview_rows_preserved\":true,\"status\":\"pass\"}}");
}

#[test]
fn naming_prompt_render_has_tracing_and_uses_shared_helpers() {
    let source = include_str!("../src/prompts/naming/render.rs");
    let outer_source = include_str!("../src/render_prompts/other.rs");

    assert!(
        outer_source.contains("surface = \"render_prompts::naming\""),
        "naming prompt outer render should emit tracing with surface tag"
    );
    assert!(
        source.contains("prompt_form_intro("),
        "naming prompt should use shared prompt_form_intro"
    );
    assert!(
        source.contains("prompt_form_section("),
        "naming prompt should use shared prompt_form_section"
    );
    assert!(
        !source.contains("rgb(0x"),
        "naming prompt should not contain hardcoded hex rgb colors"
    );
    assert!(
        !source.contains("PromptFooter::new("),
        "naming prompt should not use PromptFooter"
    );
    eprintln!("{{\"audit\":\"minimal_chrome\",\"surface\":\"naming\",\"tracing_present\":true,\"shared_helpers\":true,\"status\":\"pass\"}}");
}
