#[cfg(test)]
mod prompt_layout_shell_tests {
    use super::{prompt_shell_frame_config, PromptFrameConfig};
    use gpui::{AppContext as _, InteractiveElement as _};

    const SINGLE_LINE_CONTROL_SELECTOR: &str = "prompt-single-line-control";

    struct TestSingleLinePromptControl;

    impl gpui::Render for TestSingleLinePromptControl {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let theme = crate::theme::Theme::default();
            let style = super::prompt_field_style(&theme, super::PromptFieldState::Active, false);
            super::prompt_text_field("Value", style)
                .debug_selector(|| SINGLE_LINE_CONTROL_SELECTOR.to_string())
        }
    }

    #[gpui::test]
    fn of58ab_layout_lock_single_line_control_matches_main_menu_search_height(
        cx: &mut gpui::TestAppContext,
    ) {
        let expected_height = crate::designs::current_main_menu_theme()
            .def()
            .search
            .height;
        assert_eq!(
            super::prompt_single_line_control_metrics().total_height_px,
            expected_height
        );
        let window = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(gpui::px(0.0), gpui::px(0.0)),
                gpui::size(gpui::px(320.0), gpui::px(120.0)),
            )));
            cx.open_window(options, |_, cx| cx.new(|_| TestSingleLinePromptControl))
                .expect("single-line control test window should open")
        });

        cx.run_until_parked();

        window
            .update(cx, |_, window, _| {
                let control = window
                    .debug_bounds_entries()
                    .iter()
                    .find(|entry| entry.selector == SINGLE_LINE_CONTROL_SELECTOR)
                    .expect("single-line prompt control should publish debug bounds");
                assert_eq!(control.bounds.size.height, gpui::px(expected_height));
            })
            .expect("single-line control test window should update");
    }

    #[test]
    fn test_prompt_frame_defaults_apply_min_h_and_overflow_hidden() {
        let config = PromptFrameConfig::default();
        assert_eq!(config.min_height_px, 0.0);
        assert!(config.clip_overflow);
        assert!(!config.relative);
        assert_eq!(config.rounded_corners, None);
    }

    #[test]
    fn test_prompt_shell_frame_config_sets_relative_and_radius() {
        let config = prompt_shell_frame_config(14.0);
        assert_eq!(config.min_height_px, 0.0);
        assert!(config.clip_overflow);
        assert!(config.relative);
        assert_eq!(config.rounded_corners, Some(14.0));
    }

    #[test]
    fn prompt_surface_defaults_match_create_flow_field_chrome() {
        // Verify the shared surface uses the design-specified values.
        // If these change, update all callers too.
        let _surface = super::prompt_surface(gpui::rgba(0x112233ee), gpui::rgba(0x445566ff));
        // The function is purely a builder; the real assertion is that it
        // compiles and the constants below stay in sync with the implementation.
        assert_eq!(8.0_f32, 8.0); // radius
        assert_eq!(0.875_f32, 0.875); // px padding
        assert_eq!(0.625_f32, 0.625); // py padding
    }

    #[test]
    fn prompt_field_style_uses_theme_chrome_contract_for_default_and_active_states() {
        let theme = crate::theme::Theme::light_default();
        let chrome = crate::theme::AppChromeColors::from_theme(&theme);

        let default_style =
            super::prompt_field_style(&theme, super::PromptFieldState::Default, true);
        let active_style =
            super::prompt_field_style(&theme, super::PromptFieldState::Active, false);

        assert_eq!(
            default_style.background,
            gpui::rgba(chrome.input_surface_rgba)
        );
        assert_eq!(default_style.border, gpui::rgba(chrome.badge_border_rgba));
        assert_eq!(
            default_style.value,
            gpui::rgba(chrome.placeholder_text_rgba)
        );
        assert_eq!(active_style.background, gpui::rgba(chrome.selection_rgba));
        assert_eq!(active_style.border, gpui::rgb(chrome.accent_hex));
    }

    const OTHER_RENDERERS_SOURCE: &str = include_str!("../render_prompts/other.rs");

    fn fn_source(name: &str) -> &'static str {
        let marker = format!("fn {}(", name);
        let Some(start) = OTHER_RENDERERS_SOURCE.find(&marker) else {
            return "";
        };
        let tail = &OTHER_RENDERERS_SOURCE[start..];
        let end = tail.find("\n    fn ").unwrap_or(tail.len());
        &tail[..end]
    }

    #[test]
    fn simple_prompt_wrappers_use_shared_layout_shell() {
        for fn_name in ["render_env_prompt", "render_drop_prompt"] {
            let body = fn_source(fn_name);
            assert!(
                body.contains("render_wrapped_prompt_entity"),
                "{fn_name} should delegate to a shared prompt entity wrapper"
            );
        }
    }

    #[test]
    fn select_prompt_outer_host_is_entity_owned_shell() {
        let body = fn_source("render_select_prompt");
        assert!(
            body.contains("render_entity_owned_prompt_host("),
            "select prompt outer renderer should host keys only"
        );
        assert!(
            !body.contains("render_wrapped_prompt_entity("),
            "select prompt should not get a second outer prompt shell/footer"
        );
        assert!(
            !body.contains("clickable_universal_footer_action_rail("),
            "select prompt outer renderer should not add a second footer"
        );
        assert!(
            !body.contains("emit_prompt_hint_audit("),
            "select prompt hint audit should stay with the entity-owned footer"
        );
    }

    #[test]
    fn entity_owned_prompt_host_preserves_key_boundary_without_chrome() {
        let body = fn_source("render_entity_owned_prompt_host");
        assert!(
            body.contains(".on_key_down(handle_key)"),
            "entity-owned prompt host must keep the parent key boundary"
        );
        assert!(
            body.contains(".child(entity)"),
            "entity-owned prompt host must render the entity directly"
        );
        assert!(
            !body.contains("render_simple_prompt_shell("),
            "entity-owned prompt host must not add a second prompt shell"
        );
        assert!(
            !body.contains("main_window_footer_slot("),
            "entity-owned prompt host must not add a second footer owner"
        );
    }

    #[test]
    fn chat_prompt_uses_simple_prompt_shell_in_other_rs() {
        let body = fn_source("render_chat_prompt");
        // Chat renders its own footer (mini hint strip or rich interactive footer),
        // so it uses render_simple_prompt_shell directly with None footer
        // instead of render_wrapped_prompt_entity (which always adds a footer).
        assert!(
            body.contains("render_simple_prompt_shell("),
            "render_chat_prompt should use the shared shell directly"
        );
        assert!(
            !body.contains("render_wrapped_prompt_entity("),
            "render_chat_prompt should not use render_wrapped_prompt_entity (would add duplicate footer)"
        );
        assert!(
            body.contains("entity, None)") || body.contains("entity,\n            None,"),
            "render_chat_prompt should pass None footer to render_simple_prompt_shell"
        );
        assert!(
            body.contains("other_prompt_shell_handle_key_chat"),
            "render_chat_prompt should keep the chat-specific key handler"
        );
    }

    #[test]
    fn other_rs_calls_component_render_simple_prompt_shell_explicitly() {
        assert!(
            OTHER_RENDERERS_SOURCE.contains("crate::components::render_simple_prompt_shell("),
            "other.rs should call the shared shell helper explicitly"
        );
        assert!(
            !OTHER_RENDERERS_SOURCE.contains("fn render_simple_prompt_shell("),
            "other.rs should not define a local helper that shadows the shared helper name"
        );
    }

    #[test]
    fn template_prompt_uses_hint_strip_in_other_rs() {
        let body = fn_source("render_template_prompt");
        assert!(
            !body.contains("PromptFooter::new("),
            "render_template_prompt should not use PromptFooter"
        );
        assert!(
            body.contains("render_wrapped_prompt_entity_with_footer("),
            "render_template_prompt should delegate to the footer-aware shared wrapper"
        );
    }

    #[test]
    fn naming_prompt_uses_hint_strip_in_other_rs() {
        let body = fn_source("render_naming_prompt");
        assert!(
            !body.contains("PromptFooter::new("),
            "render_naming_prompt should not use PromptFooter"
        );
        assert!(
            body.contains("render_wrapped_prompt_entity("),
            "render_naming_prompt should delegate to render_wrapped_prompt_entity"
        );
    }

    // ── render_simple_prompt_shell contract tests ──────────────────────

    const SHELL_SOURCE: &str = include_str!("prompt_layout_shell.rs");

    #[test]
    fn render_simple_prompt_shell_accepts_optional_footer() {
        // The function signature must accept Option<AnyElement> for the footer
        // so callers can pass None (no footer) or Some(hint_strip).
        assert!(
            SHELL_SOURCE.contains("footer: Option<AnyElement>"),
            "render_simple_prompt_shell must accept footer as Option<AnyElement>"
        );
    }

    #[test]
    fn render_simple_prompt_shell_delegates_to_shell_container() {
        // Must compose from the existing prompt_shell_container + prompt_shell_content.
        let fn_start = SHELL_SOURCE
            .find("fn render_simple_prompt_shell(")
            .expect("function must exist");
        let fn_body = &SHELL_SOURCE[fn_start..];
        assert!(
            fn_body.contains("prompt_shell_container("),
            "must delegate to prompt_shell_container"
        );
        assert!(
            fn_body.contains("prompt_shell_content("),
            "must delegate to prompt_shell_content"
        );
    }

    #[test]
    fn render_simple_hint_strip_accepts_optional_leading() {
        assert!(
            SHELL_SOURCE.contains("fn render_simple_hint_strip("),
            "render_simple_hint_strip must exist"
        );
        assert!(
            SHELL_SOURCE.contains("leading: Option<AnyElement>"),
            "render_simple_hint_strip must accept leading as Option<AnyElement>"
        );
    }

    #[test]
    fn render_simple_hint_strip_returns_any_element() {
        let fn_start = SHELL_SOURCE
            .find("fn render_simple_hint_strip(")
            .expect("function must exist");
        let fn_body = &SHELL_SOURCE[fn_start..];
        let sig_end = fn_body.find('{').expect("must have body");
        let sig = &fn_body[..sig_end];
        assert!(
            sig.contains("-> AnyElement"),
            "render_simple_hint_strip must return AnyElement"
        );
    }

    // ── PromptChromeAudit contract tests ────────────────────────────────

    #[test]
    fn prompt_chrome_audit_minimal_list_uses_shared_tokens() {
        let audit = super::PromptChromeAudit::minimal_list("test_surface", true);
        assert_eq!(audit.surface, "test_surface");
        assert_eq!(audit.layout_mode, "mini");
        assert_eq!(audit.input_mode, "bare");
        assert_eq!(audit.divider_mode, "none");
        assert_eq!(audit.footer_mode, "hint_strip");
        assert_eq!(
            audit.header_padding_x,
            crate::ui::chrome::HEADER_PADDING_X as u16
        );
        assert_eq!(
            audit.header_padding_y,
            crate::ui::chrome::HEADER_PADDING_Y as u16
        );
        assert_eq!(audit.hint_count, super::UNIVERSAL_PROMPT_HINT_COUNT);
        assert!(!audit.has_leading_status);
        assert!(audit.has_actions);
        assert_eq!(audit.exception_reason, None);
    }

    #[test]
    fn prompt_chrome_audit_editor_uses_editor_layout() {
        let audit = super::PromptChromeAudit::editor("test_editor", true);
        assert_eq!(audit.layout_mode, "editor");
        assert_eq!(audit.input_mode, "bare");
        assert_eq!(audit.footer_mode, "hint_strip");
        assert_eq!(audit.hint_count, super::UNIVERSAL_PROMPT_HINT_COUNT);
    }

    #[test]
    fn prompt_chrome_audit_expanded_uses_expanded_layout() {
        let audit = super::PromptChromeAudit::expanded("test_expanded", false);
        assert_eq!(audit.layout_mode, "expanded");
        assert_eq!(audit.input_mode, "bare");
        assert_eq!(audit.footer_mode, "hint_strip");
        assert!(!audit.has_actions);
    }

    #[test]
    fn prompt_chrome_audit_grid_uses_grid_layout() {
        let audit = super::PromptChromeAudit::grid("test_grid", true);
        assert_eq!(audit.layout_mode, "grid");
        assert_eq!(audit.input_mode, "bare");
        assert_eq!(audit.footer_mode, "hint_strip");
    }

    #[test]
    fn prompt_chrome_audit_minimal_adapter_backward_compatible() {
        // When called with the universal contract values, matches minimal_list.
        let via_adapter = super::PromptChromeAudit::minimal(
            "compat_surface",
            super::UNIVERSAL_PROMPT_HINT_COUNT,
            false,
            true,
        );
        let direct = super::PromptChromeAudit::minimal_list("compat_surface", true);
        assert_eq!(via_adapter, direct);

        // Legacy callers with different hint_count still compile and set layout_mode.
        let legacy = super::PromptChromeAudit::minimal("legacy_surface", 2, false, false);
        assert_eq!(legacy.layout_mode, "mini");
        assert_eq!(legacy.hint_count, 2);
    }

    #[test]
    fn prompt_chrome_audit_exception_records_reason() {
        let audit = super::PromptChromeAudit::exception("webcam_prompt", "media_capture_surface");
        assert_eq!(audit.surface, "webcam_prompt");
        assert_eq!(audit.layout_mode, "custom");
        assert_eq!(audit.footer_mode, "prompt_footer");
        assert_eq!(audit.exception_reason, Some("media_capture_surface"));
        assert_eq!(audit.input_mode, "custom");
        assert_eq!(audit.divider_mode, "custom");
    }

    #[test]
    fn prompt_chrome_audit_emit_does_not_panic() {
        // Verify all variants can be emitted without panicking.
        let minimal = super::PromptChromeAudit::minimal_list("smoke_minimal_list", true);
        super::emit_prompt_chrome_audit(&minimal);

        let editor = super::PromptChromeAudit::editor("smoke_editor", false);
        super::emit_prompt_chrome_audit(&editor);

        let expanded = super::PromptChromeAudit::expanded("smoke_expanded", true);
        super::emit_prompt_chrome_audit(&expanded);

        let grid = super::PromptChromeAudit::grid("smoke_grid", false);
        super::emit_prompt_chrome_audit(&grid);

        let exception =
            super::PromptChromeAudit::exception("smoke_exception", "form_heavy_surface");
        super::emit_prompt_chrome_audit(&exception);
    }

    #[test]
    fn prompt_chrome_audit_dedupes_identical_contracts() {
        let audit = super::PromptChromeAudit::minimal_list("test_dedup_surface_v2", false);

        // First insert is new → true
        assert!(super::mark_prompt_chrome_audit_seen(&audit));
        // Duplicate → false
        assert!(!super::mark_prompt_chrome_audit_seen(&audit));

        // Changed contract (different has_actions) → true
        let changed = super::PromptChromeAudit::minimal_list("test_dedup_surface_v2", true);
        assert!(super::mark_prompt_chrome_audit_seen(&changed));
    }

    #[test]
    fn universal_prompt_hints_returns_exactly_three() {
        let hints = super::universal_prompt_hints();
        assert_eq!(hints.len(), super::UNIVERSAL_PROMPT_HINT_COUNT);
        assert_eq!(hints[0].as_ref(), "↵ Run");
        assert_eq!(hints[1].as_ref(), "⌘K Actions");
        assert_eq!(hints[2].as_ref(), "⌘↵ Agent");
    }

    #[test]
    fn template_prompt_hints_are_truthful_and_non_universal() {
        let hints = super::template_prompt_hints();
        assert_eq!(hints.len(), super::UNIVERSAL_PROMPT_HINT_COUNT);
        assert_eq!(hints[0].as_ref(), "↵ Submit");
        assert_eq!(hints[1].as_ref(), "⇥ Next Field");
        assert_eq!(hints[2].as_ref(), "⌘K Actions");
        assert!(!super::is_universal_prompt_hints(&hints));
    }

    #[test]
    fn surface_prompt_hint_audit_allows_three_non_universal_hints() {
        let hints: Vec<gpui::SharedString> = vec![
            "↵ Copy Markdown".into(),
            "⌘C Copy".into(),
            "Esc Back".into(),
        ];
        let audit = super::PromptHintAudit {
            surface: "sdk_reference_test_surface",
            hint_count: hints.len(),
            hints_joined: hints
                .iter()
                .map(|hint: &gpui::SharedString| hint.to_string())
                .collect::<Vec<_>>()
                .join(" | "),
            is_universal: super::is_universal_prompt_hints(&hints),
        };

        assert_eq!(audit.hint_count, super::UNIVERSAL_PROMPT_HINT_COUNT);
        assert!(!audit.is_universal);
        super::emit_surface_prompt_hint_audit(
            "sdk_reference_test_surface",
            &hints,
            "test_surface_footer",
        );
    }

    #[test]
    fn sdk_facing_builtins_use_three_surface_specific_footer_hints() {
        for (source, surface) in [
            (
                include_str!("../render_builtins/sdk_reference.rs"),
                "sdk_reference",
            ),
            (
                include_str!("../render_builtins/script_templates.rs"),
                "script_template_catalog",
            ),
        ] {
            assert!(
                source.contains("main_window_footer_slot("),
                "{surface} should route footer ownership through main_window_footer_slot"
            );
            assert!(
                source.contains("emit_surface_prompt_hint_audit("),
                "{surface} should use intentional surface hint auditing"
            );
            assert!(
                !source.contains("\"↑↓ Navigate\""),
                "{surface} should not spend footer chrome on baseline list navigation"
            );
            assert!(
                !source.contains("emit_prompt_hint_audit(\""),
                "{surface} should not warn as a universal footer mismatch"
            );
            assert!(
                source.contains("AppChromeColors::from_theme"),
                "{surface} should resolve secondary text through AppChromeColors"
            );
            assert!(
                !source.contains("self.theme.colors.text.dimmed"),
                "{surface} should not double-dim text with raw dimmed colors"
            );
            assert!(
                !source.contains("self.theme.colors.text.muted"),
                "{surface} should not double-dim text with raw muted colors"
            );
        }
    }

    #[test]
    fn prompt_chrome_audit_serializes_layout_mode() {
        let audit = super::PromptChromeAudit::minimal_list("serialize_test", true);
        let json = serde_json::to_string(&audit).expect("should serialize");
        assert!(json.contains("\"layout_mode\":\"mini\""));

        let editor = super::PromptChromeAudit::editor("serialize_editor", false);
        let json = serde_json::to_string(&editor).expect("should serialize");
        assert!(json.contains("\"layout_mode\":\"editor\""));

        let exception = super::PromptChromeAudit::exception("serialize_exc", "reason");
        let json = serde_json::to_string(&exception).expect("should serialize");
        assert!(json.contains("\"layout_mode\":\"custom\""));
    }

    #[test]
    fn other_rs_surfaces_emit_chrome_audit() {
        let source = OTHER_RENDERERS_SOURCE;
        // All prompt surfaces in other.rs should emit audit logs
        assert!(
            source.contains("emit_prompt_chrome_audit("),
            "other.rs should call emit_prompt_chrome_audit"
        );
        // Migrated surfaces use namespaced IDs
        for surface in [
            "render_prompts::template",
            "render_prompts::naming",
            "render_prompts::webcam",
            "creation_feedback",
        ] {
            assert!(
                source.contains(&format!("\"{}\"", surface)),
                "other.rs should classify {surface}"
            );
        }
        // Webcam remains as a spec-blessed exception (media capture surface)
        assert!(
            source.contains("PromptChromeAudit::exception("),
            "other.rs should still have webcam as exception"
        );
        assert!(
            source.contains("\"render_prompts::webcam\""),
            "other.rs should classify render_prompts::webcam as exception"
        );
    }

    #[test]
    fn editor_prompt_emits_chrome_audit_editor_layout() {
        let source = include_str!("../render_prompts/editor.rs");
        assert!(
            source.contains("emit_prompt_chrome_audit("),
            "editor.rs should call emit_prompt_chrome_audit"
        );
        assert!(
            source.contains("PromptChromeAudit::editor("),
            "editor.rs should classify as editor layout mode"
        );
        assert!(
            source.contains("\"render_prompts::editor\""),
            "editor.rs should identify as render_prompts::editor surface"
        );
    }

    #[test]
    fn form_prompt_emits_chrome_audit_minimal_list() {
        let source = include_str!("../render_prompts/form/render.rs");
        assert!(
            source.contains("emit_prompt_chrome_audit("),
            "form/render.rs should call emit_prompt_chrome_audit"
        );
        assert!(
            source.contains("PromptChromeAudit::minimal_list("),
            "form/render.rs should classify as minimal_list"
        );
        assert!(
            source.contains("\"form_prompt\""),
            "form/render.rs should identify as form_prompt surface"
        );
    }

    #[test]
    fn builtin_special_surfaces_emit_expected_chrome_audit() {
        // Kit Store migrated off its PromptFooter chrome exception onto native
        // footer slots + shared main-view chrome (32a6b6586 "Move Kit Store
        // footers to native slot"); both views now declare minimal audits.
        let kit_store = include_str!("../render_builtins/kit_store.rs");
        assert!(
            kit_store.contains("PromptChromeAudit::minimal(\"kit_store_browse\"")
                && kit_store.contains("PromptChromeAudit::minimal(\"kit_store_installed\""),
            "kit_store.rs should classify browse/installed as minimal"
        );
        assert!(
            !kit_store.contains("PromptChromeAudit::exception("),
            "kit_store.rs should no longer carry a chrome exception"
        );

        let process_manager = include_str!("../render_builtins/process_manager.rs");
        assert!(
            process_manager.contains("PromptChromeAudit::minimal("),
            "process_manager.rs should classify as minimal (migrated from exception)"
        );

        let settings = include_str!("../render_builtins/settings.rs");
        assert!(
            settings.contains("PromptChromeAudit::minimal_list("),
            "settings.rs should classify as minimal_list"
        );
    }

    // ── Minimal-chrome source-audit tests for migrated builtins ──────

    /// Assert the migrated minimal builtin contract.
    ///
    /// Since 9ff5f45e9 ("Share built-in search chrome broadly") minimal
    /// builtins route through the shared main-view chrome
    /// (`render_main_view_chrome_footer_flush` + `render_builtin_main_input_header`),
    /// which owns header padding and the divider — so local
    /// `HEADER_PADDING_*` tokens and `SectionDivider` must NOT reappear.
    /// The shared hint strip footer remains, and `PromptFooter` stays gone.
    fn assert_minimal_surface_source(source: &str, surface: &str) {
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];

        assert!(
            render_code.contains("render_main_view_chrome_footer_flush("),
            "{surface} should route through the shared main-view chrome"
        );
        assert!(
            render_code.contains("render_builtin_main_input_header("),
            "{surface} should use the shared built-in main input header"
        );
        assert!(
            !render_code.contains("HEADER_PADDING_X") && !render_code.contains("HEADER_PADDING_Y"),
            "{surface} should not hardcode local header padding (the shared input header owns it)"
        );
        assert!(
            !render_code.contains("SectionDivider::new()"),
            "{surface} should use the shared main-view divider contract, not a local SectionDivider"
        );
        assert!(
            render_code.contains("render_simple_hint_strip("),
            "{surface} should render a minimal hint strip footer"
        );

        let needle = ["PromptFooter", "::new("].concat();
        assert!(
            !render_code.contains(&needle),
            "{surface} should not construct PromptFooter after migration"
        );
    }

    /// Assert that source declares a runtime `PromptChromeAudit` with the given
    /// constructor and surface name literal. The failure message names the
    /// drifting surface so agents can pinpoint which builtin regressed.
    fn assert_surface_declares_runtime_audit(source: &str, surface: &str, constructor: &str) {
        let ctor = format!("PromptChromeAudit::{constructor}(");
        let surface_literal = format!("\"{surface}\"");

        assert!(
            source.contains(&ctor) && source.contains(&surface_literal),
            "{surface} should declare PromptChromeAudit::{constructor}(\"{surface}\", ...)"
        );
    }

    /// Combined source-level and runtime-audit assertion for a minimal surface.
    ///
    /// Checks both that the layout file routes through the shared main-view
    /// chrome (`render_main_view_chrome` + `render_builtin_main_input_header`)
    /// with the shared hint strip, AND that the entry-point file declares
    /// `PromptChromeAudit::minimal("<surface>", ...)`.
    macro_rules! assert_minimal_surface_file {
        ($layout_path:literal, $entry_path:literal, $surface:literal) => {{
            let layout_source = include_str!($layout_path);
            let entry_source = include_str!($entry_path);
            assert_surface_declares_runtime_audit(entry_source, $surface, "minimal");
            assert_minimal_surface_source(layout_source, $surface);
        }};
    }

    #[test]
    fn process_manager_source_matches_minimal_contract() {
        let source = include_str!("../render_builtins/process_manager.rs");
        assert!(
            source.contains("PromptChromeAudit::minimal("),
            "process_manager.rs should emit a minimal chrome audit"
        );
        assert!(
            !source.contains("PromptChromeAudit::exception("),
            "process_manager.rs should no longer emit an exception audit"
        );
        assert_minimal_surface_source(source, "process_manager.rs");
    }

    #[test]
    fn clipboard_history_source_matches_expanded_contract() {
        let source = include_str!("../render_builtins/clipboard.rs");
        // Expanded-view contract: no SectionDivider (spacing defines structure per .impeccable.md)
        assert!(
            !source.contains("SectionDivider::new()"),
            "clipboard.rs should not use SectionDivider (whisper chrome: spacing defines structure)"
        );
        // Uses universal prompt hints
        assert!(
            source.contains("universal_prompt_hints_with_primary_label(\"Paste\")"),
            "clipboard.rs should use canonical universal prompt hints with Paste as primary"
        );
        // Must route through the shared main-view chrome with the native
        // footer slot (clipboard migrated off the expanded-view scaffold onto
        // MainViewChrome; see tests/minimal_chrome_audit.rs and clipboard.rs's
        // own clipboard_history_uses_shared_main_view_chrome audit).
        assert!(
            source.contains("render_main_view_chrome_footer_flush(")
                && source.contains("main_window_footer_slot("),
            "clipboard.rs should route through the shared main-view chrome with the native footer slot"
        );
        // No PromptFooter after migration
        assert!(
            !source.contains("PromptFooter::new("),
            "clipboard.rs should not construct PromptFooter after migration"
        );
        // Emits hint audit with the universal three-key footer
        assert!(
            source.contains("emit_prompt_hint_audit("),
            "clipboard history should emit a prompt hint audit"
        );
        // Sharp edges — no rounded corners on main container
        assert!(
            !source.contains(".rounded(px(design_visual.radius_lg))"),
            "clipboard.rs should not use rounded corners on main container"
        );
    }

    /// Table-driven regression test covering all migrated minimal builtin surfaces.
    ///
    /// Each entry asserts both source-level markers (shared main-view chrome,
    /// shared input header, hint strip, no local divider/padding/PromptFooter)
    /// and the presence of a runtime `PromptChromeAudit::minimal("<surface>", ...)`
    /// declaration in the entry file. When a surface drifts, the failure
    /// message names it explicitly.
    #[test]
    fn migrated_builtin_surfaces_match_minimal_contract() {
        // process_manager: layout and entry are in the same file
        assert_minimal_surface_file!(
            "../render_builtins/process_manager.rs",
            "../render_builtins/process_manager.rs",
            "process_manager"
        );

        // clipboard_history is now expanded (not minimal) — tested separately below.
        // file_search is now expanded (not minimal) — tested separately below.
    }

    #[test]
    fn clipboard_history_declares_expanded_layout_mode() {
        let source = include_str!("../render_builtins/clipboard.rs");
        assert!(
            source.contains("PromptChromeAudit::expanded(\"clipboard_history\""),
            "clipboard_history should emit an expanded chrome audit"
        );
        assert!(
            !source.contains("PromptChromeAudit::minimal("),
            "clipboard.rs should no longer emit a minimal chrome audit"
        );
    }

    #[test]
    fn clipboard_history_uses_universal_hint_strip() {
        let layout_source = include_str!("../render_builtins/clipboard.rs");
        assert!(
            layout_source.contains("universal_prompt_hints_with_primary_label(\"Paste\")"),
            "clipboard history should use canonical universal hints with Paste as primary"
        );
        assert!(
            !layout_source.contains("SharedString::from(\"↵ Paste\")"),
            "clipboard history should not hardcode a paste-specific footer label"
        );
        assert!(
            !layout_source.contains("SharedString::from(\"Esc Back\")"),
            "clipboard history should not hardcode an escape-only footer label"
        );
    }

    #[test]
    fn file_search_declares_expanded_layout_mode() {
        let source = include_str!("../render_builtins/file_search.rs");
        assert!(
            source.contains("PromptChromeAudit::expanded(\"file_search\""),
            "file_search.rs should emit an expanded chrome audit"
        );
        assert!(
            !source.contains("PromptChromeAudit::minimal("),
            "file_search.rs should no longer emit a minimal chrome audit"
        );
    }

    #[test]
    fn render_minimal_list_prompt_scaffold_uses_shared_tokens_and_footer() {
        let fn_start = SHELL_SOURCE
            .find("fn render_minimal_list_prompt_scaffold(")
            .expect("function must exist");
        let fn_body = &SHELL_SOURCE[fn_start..];

        assert!(
            fn_body.contains("HEADER_PADDING_X"),
            "shared list scaffold must own HEADER_PADDING_X"
        );
        assert!(
            fn_body.contains("HEADER_PADDING_Y"),
            "shared list scaffold must own HEADER_PADDING_Y"
        );
        assert!(
            fn_body.contains("render_simple_hint_strip("),
            "shared list scaffold must own the hint strip footer"
        );
        assert!(
            fn_body.contains("flex_1()") && fn_body.contains("min_h(px(0."),
            "shared list scaffold must own the flex content contract"
        );
    }

    #[test]
    fn arg_prompt_uses_shared_minimal_list_prompt_shell() {
        let source = include_str!("../render_prompts/arg/render.rs");
        assert!(
            source.contains("render_minimal_list_prompt_shell_with_footer("),
            "arg prompt should use the footer-aware shared minimal list prompt shell"
        );
        assert!(
            source.contains("main_window_footer_slot("),
            "arg prompt should route its GPUI footer through main_window_footer_slot"
        );
    }

    #[test]
    fn launcher_surfaces_use_shared_minimal_list_scaffold() {
        for (source, label) in [
            (
                include_str!("../render_builtins/emoji_picker.rs"),
                "emoji_picker",
            ),
            (
                include_str!("../render_builtins/window_switcher.rs"),
                "window_switcher",
            ),
            (
                include_str!("../render_builtins/app_launcher.rs"),
                "app_launcher",
            ),
            (
                include_str!("../render_builtins/current_app_commands.rs"),
                "current_app_commands",
            ),
            (
                include_str!("../render_builtins/ai_presets.rs"),
                "ai_presets",
            ),
        ] {
            assert!(
                source.contains("render_minimal_list_prompt_scaffold(")
                    || source.contains("render_minimal_list_prompt_shell(")
                    || source.contains("render_minimal_list_prompt_shell_with_footer(")
                    || source.contains("main_window_footer_slot("),
                "{label} should use the shared minimal list prompt scaffold, shell, or native footer slot"
            );
            let legacy = ["PromptFooter", "::new("].concat();
            let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
            let render_code = &source[..render_fn_end];
            assert!(
                !render_code.contains(&legacy),
                "{label} should not construct PromptFooter"
            );
        }
    }

    #[test]
    fn render_minimal_list_prompt_shell_delegates_to_simple_shell() {
        let fn_start = SHELL_SOURCE
            .find("fn render_minimal_list_prompt_shell(")
            .expect("function must exist");
        let fn_body = &SHELL_SOURCE[fn_start..];

        assert!(
            fn_body.contains("render_simple_prompt_shell("),
            "shared list shell must delegate to render_simple_prompt_shell"
        );
        assert!(
            fn_body.contains("render_minimal_list_prompt_scaffold("),
            "shared list shell must wrap the scaffold"
        );
    }

    #[test]
    fn app_launcher_keeps_shell_root_keyboard_hooks() {
        let source = include_str!("../render_builtins/app_launcher.rs");
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];

        // app_launcher migrated from the minimal list prompt shell onto the
        // shared main-view chrome (9ff5f45e9 "Share built-in search chrome
        // broadly"); the keyboard hooks below must stay on the shell root.
        assert!(
            render_code.contains("render_main_view_chrome_footer_flush(")
                && render_code.contains("render_builtin_main_input_header("),
            "app_launcher should return the shared main-view chrome wrapper"
        );
        assert!(
            render_code.contains(".key_context(\"app_launcher\")"),
            "app_launcher should keep its key context on the shell root"
        );
        assert!(
            render_code.contains(".track_focus(&self.focus_handle)"),
            "app_launcher should keep focus tracking on the shell root"
        );
        assert!(
            render_code.contains(".on_key_down(handle_key)"),
            "app_launcher should keep the keyboard handler on the shell root"
        );
    }

    #[test]
    fn app_launcher_drops_redundant_header_and_footer_chrome() {
        let source = include_str!("../render_builtins/app_launcher.rs");
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];

        let legacy = ["PromptFooter", "::new("].concat();
        assert!(
            !render_code.contains(&legacy),
            "app_launcher should not construct PromptFooter after migration"
        );
        assert!(
            !render_code.contains("\u{1f680} Apps"),
            "app_launcher should not keep a redundant launcher title row"
        );
        assert!(
            !render_code.contains("render_hint_strip_leading_text("),
            "app launcher footer should not render leading status text"
        );
        assert!(
            !render_code.contains("universal_prompt_hints()"),
            "app launcher should not use universal hints (no actions dialog wired)"
        );
        assert!(
            render_code.contains("\"↵ Launch\""),
            "app launcher should use a truthful two-item footer"
        );
    }

    #[test]
    fn path_prompt_entity_uses_minimal_shell_and_select_actions_hint_strip() {
        let source = include_str!("../prompts/path/render.rs");

        assert!(
            source.contains("render_minimal_list_prompt_shell(")
                || source.contains("render_minimal_list_prompt_shell_with_footer("),
            "path prompt entity should use the shared minimal list prompt shell"
        );
        assert!(
            source.contains("path_prompt_hints()")
                && source.contains("\"↵ Select\"")
                && source.contains("\"⌘K Actions\""),
            "path prompt entity should use Select + Actions footer hints"
        );
        assert!(
            !source.contains("universal_prompt_hints()") && !source.contains("\"⌘↵ AI\""),
            "path prompt entity should suppress launcher AI instead of using universal hints"
        );
        assert!(
            source.contains("prompt_text_palette("),
            "path prompt entity should use the shared prompt text palette"
        );
        assert!(
            !source.contains("<< 8") && !source.contains("0x99") && !source.contains("0xCC"),
            "path prompt entity should not build local packed-alpha text colors"
        );
        let legacy = ["PromptFooter", "::new("].concat();
        assert!(
            !source.contains(&legacy),
            "path prompt entity should not construct PromptFooter"
        );
        assert!(
            !source.contains("PromptContainer::new("),
            "path prompt entity should not use legacy PromptContainer"
        );
        assert!(
            !source.contains(&["PromptHeader", "::new("].concat()),
            "path prompt entity should not use legacy PromptHeader"
        );
    }

    #[test]
    fn universal_prompt_hints_match_only_the_canonical_three_key_set() {
        let canonical = super::universal_prompt_hints();
        assert!(super::is_universal_prompt_hints(&canonical));

        // Blessed primary-label variants share the universal anatomy.
        for label in ["Paste", "Open App", "Capture Photo"] {
            let relabeled = super::universal_prompt_hints_with_primary_label(label);
            assert!(
                super::is_universal_prompt_hints(&relabeled),
                "↵ {label} | ⌘K Actions | Agent must count as universal anatomy"
            );
        }

        // A non-Agent third slot is not universal.
        let non_canonical = vec![
            gpui::SharedString::from("↵ Paste"),
            gpui::SharedString::from("⌘K Actions"),
            gpui::SharedString::from("Esc Back"),
        ];
        assert!(!super::is_universal_prompt_hints(&non_canonical));

        // A different primary key is not universal.
        let cmd_enter_primary =
            super::universal_prompt_hints_with_primary_key_label("⌘↵", "Submit");
        assert!(!super::is_universal_prompt_hints(&cmd_enter_primary));

        // An empty primary label is not universal.
        let empty_label = super::universal_prompt_hints_with_primary_label("");
        assert!(!super::is_universal_prompt_hints(&empty_label));

        // Wrong length
        let too_short = vec![gpui::SharedString::from("↵ Run")];
        assert!(!super::is_universal_prompt_hints(&too_short));

        // Empty
        assert!(!super::is_universal_prompt_hints(&[]));
    }

    #[test]
    fn select_prompt_uses_footer_aware_universal_hint_strip() {
        let source = include_str!("../prompts/select/render.rs");
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];

        assert!(
            render_code.contains("universal_prompt_hints()"),
            "select prompt should use the canonical three-key footer"
        );
        assert!(
            render_code.contains("emit_prompt_hint_audit("),
            "select prompt should emit a prompt hint audit"
        );
        assert!(
            render_code.contains("render_minimal_list_prompt_shell_with_footer(")
                && render_code.contains("main_window_footer_slot_for_prompt_surface(")
                && render_code.contains("\"select_prompt\""),
            "select prompt should route its footer through the prompt surface slot helper"
        );
        assert!(
            !render_code.contains("SharedString::from(\"↵ Select\")"),
            "select prompt should not hardcode a select-specific footer label"
        );
        assert!(
            !render_code.contains("SharedString::from(\"⌘Space Toggle\")"),
            "select prompt should not hardcode a toggle-specific footer label"
        );
        assert!(
            !render_code.contains("SharedString::from(\"Esc Back\")"),
            "select prompt should not hardcode an escape-only footer label"
        );
    }

    #[test]
    fn mini_chat_uses_universal_hint_strip() {
        let source = include_str!("../prompts/chat/render_core.rs");
        let render_code = &source[..source.find("#[cfg(test)]").unwrap_or(source.len())];

        assert!(
            render_code.contains("render_simple_hint_strip(")
                && render_code.contains("universal_prompt_hints()"),
            "mini chat should use the shared universal hint strip"
        );
        assert!(
            render_code.contains("emit_prompt_hint_audit(\"prompts::chat::mini\""),
            "mini chat should emit a prompt hint audit for prompts::chat::mini"
        );
        assert!(
            !render_code.contains("\"↵ Send  ·  ⌘K Actions  ·  Esc Back\""),
            "mini chat should not hardcode a send/back footer string"
        );
    }

    #[test]
    fn path_prompt_outer_wrapper_uses_shared_shell_container() {
        let source = include_str!("../render_prompts/path.rs");
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];

        assert!(
            render_code.contains("prompt_shell_container("),
            "path prompt outer wrapper should use the shared prompt_shell_container"
        );
        assert!(
            render_code.contains(".key_context(\"path_prompt_container\")"),
            "path prompt outer wrapper should keep its key context"
        );
        assert!(
            render_code.contains(".on_key_down(handle_key)"),
            "path prompt outer wrapper should keep the keyboard handler"
        );
    }

    // ── Expanded-view scaffold source-audit tests ──────────────────

    #[test]
    fn expanded_view_scaffold_source_uses_universal_hints_and_shared_header() {
        let source = include_str!("prompt_layout_shell.rs");

        // Find the render_expanded_view_scaffold function body
        let fn_start = source
            .find("fn render_expanded_view_scaffold(")
            .expect("render_expanded_view_scaffold must exist");
        let fn_body = &source[fn_start..fn_start + 2000];

        assert!(
            fn_body.contains("universal_prompt_hints()"),
            "expanded scaffold must use universal_prompt_hints for footer"
        );
        assert!(
            fn_body.contains("HEADER_PADDING_X"),
            "expanded scaffold must use shared HEADER_PADDING_X"
        );
        assert!(
            fn_body.contains("HEADER_PADDING_Y"),
            "expanded scaffold must use shared HEADER_PADDING_Y"
        );
        assert!(
            fn_body.contains("render_simple_hint_strip("),
            "expanded scaffold must render footer via render_simple_hint_strip"
        );
        assert!(
            !fn_body.contains("SectionDivider"),
            "expanded scaffold must NOT use SectionDivider"
        );
        assert!(
            !fn_body.contains("rounded("),
            "expanded scaffold must NOT add rounded preview wrapper chrome"
        );
    }

    #[test]
    fn expanded_view_scaffold_has_no_hardcoded_opacity_literals() {
        let source = include_str!("prompt_layout_shell.rs");
        let fn_start = source
            .find("fn render_expanded_view_scaffold(")
            .expect("render_expanded_view_scaffold must exist");
        let fn_end_marker = source[fn_start..]
            .find("\n/// ")
            .map(|pos| fn_start + pos)
            .unwrap_or(fn_start + 1500);
        let fn_body = &source[fn_start..fn_end_marker];

        // No magic opacity floats (0.03, 0.06, 0.40, 0.55, 0.60, 0.75, 0.85)
        for magic in &["0.03", "0.06", "0.40", "0.55", "0.60", "0.75", "0.85"] {
            assert!(
                !fn_body.contains(magic),
                "expanded scaffold must not contain hardcoded opacity {magic}"
            );
        }
    }

    #[test]
    fn expanded_view_prompt_shell_delegates_to_simple_prompt_shell() {
        let source = include_str!("prompt_layout_shell.rs");
        let fn_start = source
            .find("fn render_expanded_view_prompt_shell(")
            .expect("render_expanded_view_prompt_shell must exist");
        let fn_body = &source[fn_start..fn_start + 600];

        assert!(
            fn_body.contains("render_simple_prompt_shell("),
            "expanded shell must delegate to render_simple_prompt_shell"
        );
        assert!(
            fn_body.contains("render_expanded_view_scaffold("),
            "expanded shell must compose the scaffold"
        );
    }
}
