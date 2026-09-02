mod __render_prompts_micro_docs {
    //! Micro prompt rendering: ultra-compact inline widget for quick numeric or short text entry.
    //! No list, no footer — just input. Submit on Enter, dismiss on Escape.
    //! This fragment is included via include!() macro in main.rs.
}

impl ScriptListApp {
    fn render_micro_prompt(
        &mut self,
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        tracing::info!(
            prompt_type = "micro",
            prompt_id = %id,
            choice_count = choices.len(),
            "render_micro_prompt: rendering ultra-compact inline prompt"
        );

        let render_context = PromptRenderContext::new(self.theme.as_ref(), self.current_design);
        let theme = render_context.theme;
        let design_typography = render_context.design_typography;
        let design_visual = render_context.design_visual;

        let text_primary = theme.colors.text.primary;
        let text_muted = theme.colors.text.muted;
        let accent_color = theme.colors.accent.selected;
        let input_is_empty = self.arg_input.is_empty();

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::exception(
                "render_prompts::micro",
                "ultra_compact_no_footer",
            ),
        );

        // Micro uses minimal padding — tightest possible chrome
        let micro_padding_x: f32 = HEADER_PADDING_X * 0.5;
        let micro_padding_y: f32 = HEADER_PADDING_Y * 0.5;

        // Key handler — Enter submits, Escape dismisses, minimal navigation
        let prompt_id = id;
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                // Hide cursor while typing
                this.hide_mouse_cursor(cx);

                let key = event.keystroke.key.as_str();
                let key_char = event.keystroke.key_char.as_deref();
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;
                let key_lower = key.to_lowercase();

                // Escape dismisses
                if ui_foundation::is_key_escape(key) {
                    this.go_back_or_close(window, cx);
                    cx.stop_propagation();
                    return;
                }

                // Enter submits
                if ui_foundation::is_key_enter(key) {
                    this.submit_arg_prompt_from_current_state(&prompt_id, cx);
                    cx.stop_propagation();
                    return;
                }

                // Arrow up/down for hidden selection navigation
                if ui_foundation::is_key_up(key) && !modifiers.shift {
                    if this.arg_selected_index > 0 {
                        this.set_arg_selected_index(this.arg_selected_index - 1);
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }

                if ui_foundation::is_key_down(key) && !modifiers.shift {
                    let filtered = this.filtered_arg_choices();
                    if this.arg_selected_index < filtered.len().saturating_sub(1) {
                        this.set_arg_selected_index(this.arg_selected_index + 1);
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }

                // Cmd+W closes
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.go_back_or_close(window, cx);
                    cx.stop_propagation();
                    return;
                }

                // Delegate text editing to TextInputState
                let old_text = this.arg_input.text().to_string();
                let prev_original_idx = this
                    .filtered_arg_choices()
                    .get(this.arg_selected_index)
                    .map(|(orig_idx, _)| *orig_idx);

                let handled = this.arg_input.handle_key(
                    &key_lower,
                    key_char,
                    modifiers.platform,
                    modifiers.alt,
                    modifiers.shift,
                    cx,
                );

                if handled {
                    if this.arg_input.text() != old_text {
                        // Update selection tracking (no resize — micro is fixed height)
                        let new_selected_idx = {
                            let filtered = this.filtered_arg_choices();
                            if let Some(prev_idx) = prev_original_idx {
                                filtered
                                    .iter()
                                    .position(|(orig_idx, _)| *orig_idx == prev_idx)
                                    .unwrap_or(0)
                            } else {
                                0
                            }
                        };
                        this.set_arg_selected_index(new_selected_idx);
                    }
                    cx.notify();
                } else {
                    cx.propagate();
                }
            },
        );

        // Use slightly smaller font for micro to reinforce compact feel
        let micro_font_size = design_typography.font_size_lg - 1.0;

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("micro_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Ultra-compact input — no footer, minimal padding
            .child(
                div()
                    .w_full()
                    .px(px(micro_padding_x))
                    .py(px(micro_padding_y))
                    .flex()
                    .flex_row()
                    .items_center()
                    .child({
                        let input_height = CURSOR_HEIGHT_LG + (CURSOR_MARGIN_Y * 2.0);
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .h(px(input_height))
                            .text_size(px(micro_font_size))
                            .text_color(if input_is_empty {
                                rgb(text_muted)
                            } else {
                                rgb(text_primary)
                            })
                            .when(input_is_empty, |d: gpui::Div| {
                                let is_cursor_visible = self.focused_input
                                    == FocusedInput::ArgPrompt
                                    && self.cursor_visible;
                                d.child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .when(is_cursor_visible, |d: gpui::Div| {
                                            d.child(
                                                crate::components::text_input::pulse_cursor_bar(
                                                    div()
                                                        .w(px(CURSOR_WIDTH))
                                                        .h(px(CURSOR_HEIGHT_LG))
                                                        .bg(rgb(text_primary)),
                                                    "micro-input-cursor-pulse",
                                                ),
                                            )
                                        })
                                        .child(
                                            div()
                                                .ml(px(-(CURSOR_WIDTH)))
                                                .text_color(rgb(text_muted))
                                                .child(placeholder),
                                        ),
                                )
                            })
                            .when(!input_is_empty, |d: gpui::Div| {
                                d.child(self.render_arg_input_text(text_primary, accent_color, cx))
                            })
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod micro_prompt_render_tests {
    const MICRO_SOURCE: &str = include_str!("micro.rs");

    #[test]
    fn micro_prompt_has_ultra_compact_padding() {
        assert!(
            MICRO_SOURCE.contains("HEADER_PADDING_X * 0.5"),
            "micro prompt should use half horizontal padding"
        );
        assert!(
            MICRO_SOURCE.contains("HEADER_PADDING_Y * 0.5"),
            "micro prompt should use half vertical padding"
        );
    }

    #[test]
    fn micro_prompt_has_key_handlers() {
        assert!(
            MICRO_SOURCE.contains("is_key_escape(key)"),
            "micro prompt should handle Escape"
        );
        assert!(
            MICRO_SOURCE.contains("is_key_enter(key)"),
            "micro prompt should handle Enter"
        );
        assert!(
            MICRO_SOURCE.contains("stop_propagation()"),
            "micro prompt should stop propagation on handled keys"
        );
    }

    #[test]
    fn micro_prompt_emits_structured_logs() {
        assert!(
            MICRO_SOURCE.contains("tracing::info!("),
            "micro prompt should emit structured log at info level"
        );
        assert!(
            MICRO_SOURCE.contains("prompt_type = \"micro\""),
            "micro prompt log should include prompt_type field"
        );
    }

    #[test]
    fn micro_prompt_uses_smaller_font() {
        assert!(
            MICRO_SOURCE.contains("font_size_lg - 1.0"),
            "micro prompt should use slightly smaller font size"
        );
    }
}
