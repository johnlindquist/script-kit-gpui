mod __render_prompts_div_docs {
    //! Div prompt rendering integration for `ScriptListApp::render_div_prompt`.
    //! The key surface is the single render method that wires keyboard handling and action popups.
    //! It depends on prompt-shell/components/theme tokens and is included into the main app module.
}

// Div prompt render method - extracted from render_prompts.rs
// This file is included via include!() macro in main.rs

impl ScriptListApp {
    fn render_div_prompt(
        &mut self,
        _id: String,
        entity: Entity<DivPrompt>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let render_context = PromptRenderContext::new(self.theme.as_ref(), self.current_design);
        let theme = render_context.theme;
        let actions_dialog_top = render_context.actions_dialog_top;
        let actions_dialog_right = render_context.actions_dialog_right;
        let has_actions = self.has_nonempty_sdk_actions();

        tracing::info!(surface = "render_prompts::div", "prompt_surface_rendered");

        // Key handler for Cmd+K actions toggle (at parent level to intercept before DivPrompt)
        let has_actions_for_handler = has_actions;
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                if handle_prompt_key_preamble_default(
                    this,
                    event,
                    window,
                    cx,
                    PromptKeyPreambleCfg {
                        stop_propagation_on_global_shortcut: true,
                        stop_propagation_when_handled: true,
                        host: ActionsDialogHost::DivPrompt,
                    },
                    has_actions_for_handler,
                    "DivPrompt",
                ) {}
                // Fall through to DivPrompt entity key handling.
            },
        );

        // VIBRANCY: Use foundation helper - returns None when vibrancy enabled (let Root handle bg)
        let vibrancy_bg = get_vibrancy_background(theme);

        // Use explicit height from layout constants
        let content_height = window_resize::layout::STANDARD_HEIGHT;

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list(
                "render_prompts::div",
                has_actions,
            ),
        );

        crate::components::prompt_shell_container(0.0, vibrancy_bg)
            .h(content_height)
            .track_focus(&self.focus_handle) // Required to receive key events
            .on_key_down(handle_key)
            // Content shell — no titled header, let the output speak for itself
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.)) // Critical: allows flex children to size properly
                    .overflow_hidden()
                    .child(crate::components::prompt_shell_content(entity.clone())),
            )
            // Universal three-key hint strip footer (native or GPUI)
            .when_some(
                self.main_window_footer_slot(self.clickable_universal_footer_action_rail(cx)),
                |d, footer| d.child(footer),
            )
            // Actions dialog overlay (when Cmd+K is pressed with SDK actions)
            .when_some(
                render_actions_backdrop(
                    self.show_actions_popup,
                    self.actions_dialog.clone(),
                    actions_dialog_top,
                    actions_dialog_right,
                    ActionsBackdropConfig {
                        backdrop_id: "div-actions-backdrop",
                        close_host: ActionsDialogHost::DivPrompt,
                        backdrop_log_message: "Div actions backdrop clicked - dismissing dialog",
                        show_pointer_cursor: true,
                    },
                    cx,
                ),
                |d, backdrop_overlay| d.child(backdrop_overlay),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod div_prompt_render_tests {
    const DIV_RENDER_SOURCE: &str = include_str!("div.rs");

    #[test]
    fn test_div_actions_backdrop_uses_shared_helper_with_clickable_cursor() {
        assert!(
            DIV_RENDER_SOURCE.contains("render_actions_backdrop("),
            "div render should delegate backdrop overlay creation to shared helper"
        );
        assert!(
            DIV_RENDER_SOURCE.contains("\"div-actions-backdrop\""),
            "div render should pass its backdrop id to shared helper"
        );
        assert!(
            DIV_RENDER_SOURCE.contains("ActionsDialogHost::DivPrompt"),
            "div render should preserve actions host routing when helper is used"
        );
        assert!(
            DIV_RENDER_SOURCE.contains("show_pointer_cursor: true"),
            "div render should keep backdrop cursor pointer enabled"
        );
    }

    #[test]
    fn test_div_key_handling_uses_preamble_helper() {
        assert!(
            DIV_RENDER_SOURCE.contains("handle_prompt_key_preamble_default("),
            "div key handling should delegate shared preamble behavior to helper"
        );
        assert!(
            DIV_RENDER_SOURCE.contains("stop_propagation_on_global_shortcut: true"),
            "div key preamble should stop propagation when global shortcut consumes the key"
        );
        assert!(
            DIV_RENDER_SOURCE.contains("stop_propagation_when_handled: true"),
            "div key preamble should stop propagation when helper branches handle the key"
        );
    }

    #[test]
    fn div_prompt_uses_universal_footer_action_rail() {
        assert!(
            DIV_RENDER_SOURCE.contains("clickable_universal_footer_action_rail("),
            "div prompt should render the shared footer-button rail"
        );
        // Split string to avoid self-match in source audit
        let needle = ["PromptFooter", "::new("].concat();
        let render_fn_end = DIV_RENDER_SOURCE
            .find("#[cfg(test)]")
            .unwrap_or(DIV_RENDER_SOURCE.len());
        let render_code = &DIV_RENDER_SOURCE[..render_fn_end];
        assert!(
            !render_code.contains(&needle),
            "div prompt render code should not use PromptFooter"
        );
        assert!(
            !render_code.contains("\"Script Output\""),
            "div prompt should not have a titled Script Output header"
        );
    }

    #[test]
    fn div_prompt_emits_chrome_audit() {
        assert!(
            DIV_RENDER_SOURCE.contains("emit_prompt_chrome_audit("),
            "div prompt should emit a chrome audit"
        );
        assert!(
            DIV_RENDER_SOURCE.contains("\"render_prompts::div\""),
            "div prompt chrome audit should use correct surface name"
        );
    }
}
