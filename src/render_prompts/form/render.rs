impl ScriptListApp {
    fn render_form_prompt(
        &mut self,
        entity: Entity<FormPromptState>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list(
                "render_prompts::form",
                self.has_nonempty_sdk_actions(),
            ),
        );
        let render_context = PromptRenderContext::new(self.theme.as_ref(), self.current_design);
        let theme = render_context.theme;
        let design_colors = render_context.design_colors;
        let design_typography = render_context.design_typography;
        let actions_dialog_top = render_context.actions_dialog_top;
        let actions_dialog_right = render_context.actions_dialog_right;
        let has_actions = self.has_nonempty_sdk_actions();

        // Get prompt ID and field count from entity for tracing
        let (prompt_id, prompt_type, field_count) = {
            let form_state = entity.read(cx);
            (
                form_state.id.clone(),
                form_state.prompt_type(),
                form_state.fields.len(),
            )
        };

        tracing::info!(
            surface = "render_prompts::form",
            prompt_id = %prompt_id,
            prompt_type,
            field_count,
            shell = "render_simple_prompt_shell",
            body_insets = "main_menu",
            "prompt_surface_rendered"
        );

        // Clone entity for closures
        let entity_for_submit = entity.clone();
        let entity_for_tab = entity.clone();
        let entity_for_shift_tab = entity.clone();
        let entity_for_input = entity.clone();

        let prompt_id_for_key = prompt_id.clone();
        // Key handler for form navigation (Enter/Tab/Escape) and Cmd+K actions
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
                        stop_propagation_on_global_shortcut: false,
                        stop_propagation_when_handled: false,
                        host: ActionsDialogHost::FormPrompt,
                    },
                    has_actions_for_handler,
                    "FormPrompt",
                ) {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let has_shift = event.keystroke.modifiers.shift;
                let has_cmd = event.keystroke.modifiers.platform;

                // Handle form-level keys (Enter, Escape, Tab) at this level
                // Forward all other keys to the focused form field for text input
                let focused_field_is_textarea = {
                    let form = entity_for_submit.read(cx);
                    focused_form_field_is_textarea(form)
                };
                match form_enter_behavior(key, has_cmd, focused_field_is_textarea) {
                    FormEnterBehavior::Submit => {
                        let validation_message = {
                            let form = entity_for_submit.read(cx);
                            form.submit_validation_message(cx)
                        };
                        if let Some(message) = validation_message {
                            this.show_hud(message, Some(HUD_LONG_MS), cx);
                            return;
                        }

                        logging::log("KEY", "Enter in FormPrompt - submitting form");
                        let values = entity_for_submit.read(cx).collect_values(cx);
                        this.submit_prompt_response(prompt_id_for_key.clone(), Some(values), cx);
                        return;
                    }
                    FormEnterBehavior::ForwardToField => {
                        entity_for_input.update(cx, |form, cx| {
                            form.handle_key_input(event, cx);
                        });
                        return;
                    }
                    FormEnterBehavior::Ignore => {}
                }

                // Note: "escape" is handled by handle_global_shortcut_with_options above
                if key.eq_ignore_ascii_case("tab") {
                    // Tab navigation between fields
                    if has_shift {
                        entity_for_shift_tab.update(cx, |form, cx| {
                            form.focus_previous(window, cx);
                        });
                    } else {
                        entity_for_tab.update(cx, |form, cx| {
                            form.focus_next(window, cx);
                        });
                    }
                    return;
                }

                // Forward all other keys (characters, backspace, arrows, etc.) to the focused field
                // This is necessary because GPUI requires track_focus() to receive key events,
                // and we need the parent to have focus to handle Enter/Escape/Tab.
                // The form fields' individual on_key_down handlers don't fire when parent has focus.
                entity_for_input.update(cx, |form, cx| {
                    form.handle_key_input(event, cx);
                });
            },
        );

        // VIBRANCY: Use foundation helper - returns None when vibrancy enabled (let Root handle bg)
        let vibrancy_bg = get_vibrancy_background(theme);
        let content_height = window_resize::layout::STANDARD_HEIGHT;

        // Form fields have their own focus handles and on_key_down handlers.
        // We DO NOT track_focus on the container - the fields handle their own focus.
        // Enter/Escape/Tab are handled by the handle_key listener above.
        crate::components::render_simple_prompt_shell(
            0.0,
            vibrancy_bg,
            crate::components::render_inset_prompt_body(
                "form-prompt-inset-body",
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .child(entity.clone()),
                crate::components::PromptBodyInsets::MainMenu(self.current_design),
            ),
            self.main_window_footer_slot(self.clickable_universal_footer_action_rail(cx)),
        )
        .h(content_height)
        .text_color(rgb(design_colors.text_primary))
        .font_family(design_typography.font_family)
        .key_context("form_prompt")
        .on_key_down(handle_key)
        .when_some(
            render_actions_backdrop(
                self.show_actions_popup,
                self.actions_dialog.clone(),
                actions_dialog_top,
                actions_dialog_right,
                ActionsBackdropConfig {
                    backdrop_id: "form-actions-backdrop",
                    close_host: ActionsDialogHost::FormPrompt,
                    backdrop_log_message: "Form actions backdrop clicked - dismissing dialog",
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
mod form_prompt_render_backdrop_tests {
    const FORM_RENDER_SOURCE: &str = include_str!("render.rs");

    #[test]
    fn test_form_actions_backdrop_uses_shared_helper_with_clickable_cursor() {
        assert!(
            FORM_RENDER_SOURCE.contains("render_actions_backdrop("),
            "form render should delegate backdrop overlay creation to shared helper"
        );
        assert!(
            FORM_RENDER_SOURCE.contains("\"form-actions-backdrop\""),
            "form render should pass its backdrop id to shared helper"
        );
        assert!(
            FORM_RENDER_SOURCE.contains("ActionsDialogHost::FormPrompt"),
            "form render should preserve actions host routing when helper is used"
        );
        assert!(
            FORM_RENDER_SOURCE.contains("show_pointer_cursor: true"),
            "form render should keep backdrop cursor pointer enabled"
        );
    }

    #[test]
    fn test_form_key_handler_uses_shared_preamble_helper() {
        assert!(
            FORM_RENDER_SOURCE.contains("handle_prompt_key_preamble("),
            "form key handling should delegate preamble logic to shared helper"
        );
        assert!(
            FORM_RENDER_SOURCE.contains("PromptKeyPreambleCfg"),
            "form key handling should configure the shared helper via PromptKeyPreambleCfg"
        );
    }
}
