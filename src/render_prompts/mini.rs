mod __render_prompts_mini_docs {
    //! Mini prompt rendering: compact single-line input with reduced chrome.
    //! No list display, minimal padding, optional footer.
    //! This fragment is included via include!() macro in main.rs.
}

impl ScriptListApp {
    fn render_mini_prompt(
        &mut self,
        id: String,
        _placeholder: String,
        choices: Vec<Choice>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        tracing::info!(
            prompt_type = "mini",
            prompt_id = %id,
            choice_count = choices.len(),
            "render_mini_prompt: rendering compact single-line prompt"
        );

        let render_context = PromptRenderContext::new(self.theme.as_ref(), self.current_design);
        let theme = render_context.theme;
        let design_typography = render_context.design_typography;
        let menu_def = self.current_main_menu_theme.def();

        let text_primary = theme.colors.text.primary;
        let text_muted = theme.colors.text.muted;
        // Mini uses shared layout tokens from the resize contract
        let mini_padding_x: f32 = crate::window_resize::main_layout::HEADER_PADDING_X;

        // Navigation key handler — Escape, Enter, arrows, Tab, Cmd+W
        // Text editing is handled by the Input component.
        let prompt_id = id;
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);

                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;

                // Escape dismisses
                if ui_foundation::is_key_escape(key) {
                    this.go_back_or_close(window, cx);
                    cx.stop_propagation();
                    return;
                }

                // Enter submits the current prompt state
                if ui_foundation::is_key_enter(key) {
                    this.submit_arg_prompt_from_current_state(&prompt_id, cx);
                    cx.stop_propagation();
                    return;
                }

                // Arrow up/down for choice navigation. GEO-002: Mini shows a
                // five-row viewport, so selection must scroll the list
                // (ScrollStrategy::Nearest, same as the Full Arg prompt) or
                // choice six can be selected while rendered outside the
                // viewport.
                if ui_foundation::is_key_up(key) && !modifiers.shift {
                    if this.arg_selected_index > 0 {
                        this.arg_selected_index -= 1;
                        this.arg_list_scroll_handle.scroll_to_item(
                            this.arg_selected_index,
                            gpui::ScrollStrategy::Nearest,
                        );
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }

                if ui_foundation::is_key_down(key) && !modifiers.shift {
                    let filtered = this.filtered_arg_choices();
                    if this.arg_selected_index < filtered.len().saturating_sub(1) {
                        this.arg_selected_index += 1;
                        this.arg_list_scroll_handle.scroll_to_item(
                            this.arg_selected_index,
                            gpui::ScrollStrategy::Nearest,
                        );
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }

                // Tab completion
                if key.eq_ignore_ascii_case("tab") && !has_cmd && !modifiers.alt && !modifiers.shift
                {
                    this.apply_arg_tab_completion(window, cx);
                    cx.stop_propagation();
                    return;
                }

                // Cmd+W closes
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.go_back_or_close(window, cx);
                    cx.stop_propagation();
                    return;
                }

                // All other keys propagate to the Input component
                cx.propagate();
            },
        );

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list("render_prompts::mini", false),
        );

        let header = crate::components::main_view_chrome::render_prompt_search_input(
            theme,
            menu_def,
            crate::components::main_view_chrome::PromptSearchInputChrome::entity_backed(
                Input::new(&self.gpui_input_state),
            ),
        );

        // Compact choice list for mini prompt (e.g. mic picker)
        // GEO-002: resolve the Mini row slot once from the active renderer's
        // themed metrics (canonical 44px general row) — never the stale
        // LIST_ITEM_HEIGHT constant — and tag the viewport/rows with stable
        // measurement IDs so model and paint join by identity.
        let row_slot_height = crate::window_resize::arg_layout::arg_row_slot_height();
        let content = if choices.is_empty() {
            div()
        } else {
            let arg_list_colors = crate::list_item::ListItemColors::from_theme(theme);
            let arg_selected_index = self.arg_selected_index;
            let filtered_choices = self.get_filtered_arg_choices_owned();
            let filtered_choices_len = filtered_choices.len();

            if filtered_choices_len == 0 {
                div()
                    .w_full()
                    .h(px(row_slot_height))
                    .px(px(mini_padding_x))
                    .flex()
                    .items_center()
                    .text_color(rgb(text_muted))
                    .child("No matches")
            } else {
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .debug_selector(|| {
                        crate::window_resize::arg_layout::MINI_LIST_VIEWPORT_MEASUREMENT_ID
                            .to_string()
                    })
                    .child(
                        uniform_list(
                            "mini-choices",
                            filtered_choices_len,
                            move |visible_range, _window, _cx| {
                                visible_range
                                    .map(|ix| {
                                        if let Some((_, choice)) = filtered_choices.get(ix) {
                                            let is_selected = ix == arg_selected_index;
                                            div()
                                                .id(ix)
                                                .debug_selector(move || {
                                                    format!(
                                                        "{}:{ix}",
                                                        crate::window_resize::arg_layout::MINI_ROW_MEASUREMENT_ID_PREFIX
                                                    )
                                                })
                                                .child(
                                                    crate::list_item::ListItem::new(
                                                        choice.name.clone(),
                                                        arg_list_colors,
                                                    )
                                                    .selected(is_selected)
                                                    .index(ix),
                                                )
                                        } else {
                                            div().id(ix).h(px(row_slot_height))
                                        }
                                    })
                                    .collect()
                            },
                        )
                        .h_full()
                        .track_scroll(&self.arg_list_scroll_handle),
                    )
            }
        };
        // Build GPUI hint strip, then route through the native footer slot
        let gpui_footer = crate::components::render_simple_hint_strip(
            crate::components::universal_prompt_hints(),
            None,
        );
        let footer = self.main_window_footer_slot(gpui_footer);

        crate::components::render_minimal_list_prompt_shell_with_footer(
            0.0,
            crate::ui_foundation::get_vibrancy_background(&self.theme),
            header,
            content,
            footer,
        )
        .text_color(rgb(text_primary))
        .font_family(design_typography.font_family)
        .key_context("mini_prompt")
        .track_focus(&self.focus_handle)
        .capture_key_down(handle_key)
        .into_any_element()
    }
}

#[cfg(test)]
mod mini_prompt_render_tests {
    const MINI_SOURCE: &str = include_str!("mini.rs");

    #[test]
    fn mini_prompt_has_compact_padding() {
        assert!(
            MINI_SOURCE.contains("main_layout::HEADER_PADDING_X"),
            "mini prompt should use shared mini layout horizontal padding token"
        );
        assert!(
            MINI_SOURCE.contains("render_minimal_list_prompt_shell_with_footer("),
            "mini prompt should let the shared minimal-list shell own header padding"
        );
    }

    #[test]
    fn mini_prompt_header_does_not_double_pad_shared_shell() {
        let start = MINI_SOURCE.find("let header =").expect("header exists");
        let end = MINI_SOURCE.find("let content =").expect("content exists");
        let header = &MINI_SOURCE[start..end];

        assert!(
            !header.contains(".px(px(mini_padding_x))"),
            "mini prompt header should not apply local horizontal padding before the shared shell"
        );
        assert!(
            !header.contains(".py(px(mini_padding"),
            "mini prompt header should not apply local vertical padding before the shared shell"
        );
    }

    #[test]
    fn mini_prompt_renders_compact_choice_list() {
        assert!(
            MINI_SOURCE.contains("uniform_list("),
            "mini prompt should render a compact choice list when choices are present"
        );
        assert!(
            MINI_SOURCE.contains("ListItem::new("),
            "mini prompt should use ListItem for consistent choice rendering"
        );
        assert!(
            MINI_SOURCE.contains("min_h(px(0.))"),
            "mini prompt list container needs min_h to prevent overflow"
        );
    }

    #[test]
    fn mini_prompt_uses_shared_shell_with_universal_hints() {
        assert!(
            MINI_SOURCE.contains("render_minimal_list_prompt_shell(")
                || MINI_SOURCE.contains("render_minimal_list_prompt_shell_with_footer("),
            "mini prompt should use the shared prompt shell"
        );
        assert!(
            MINI_SOURCE.contains("universal_prompt_hints()"),
            "mini prompt should use the canonical three-key hint strip"
        );
        let render_fn_end = MINI_SOURCE
            .find("#[cfg(test)]")
            .unwrap_or(MINI_SOURCE.len());
        let render_code = &MINI_SOURCE[..render_fn_end];
        let needle = ["PromptFooter", "::new("].concat();
        assert!(
            !render_code.contains(&needle),
            "mini prompt should not use PromptFooter"
        );
    }

    #[test]
    fn mini_prompt_emits_chrome_audit() {
        assert!(
            MINI_SOURCE.contains("emit_prompt_chrome_audit("),
            "mini prompt should emit a chrome audit"
        );
        assert!(
            MINI_SOURCE.contains("\"render_prompts::mini\""),
            "mini prompt chrome audit should use correct surface name"
        );
    }

    #[test]
    fn mini_prompt_has_key_handlers() {
        let render_fn_end = MINI_SOURCE
            .find("#[cfg(test)]")
            .unwrap_or(MINI_SOURCE.len());
        let render_code = &MINI_SOURCE[..render_fn_end];

        assert!(
            render_code.contains("is_key_escape(key)"),
            "mini prompt should handle Escape"
        );
        assert!(
            render_code.contains("is_key_enter(key)"),
            "mini prompt should handle Enter"
        );
        assert!(
            render_code.contains("submit_arg_prompt_from_current_state(&prompt_id, cx)"),
            "mini prompt Enter should submit the current prompt state"
        );
        assert!(
            render_code.contains("stop_propagation()"),
            "mini prompt should stop propagation on handled keys"
        );
    }

    /// GEO-002: Mini renders a five-row viewport, so BOTH Up and Down
    /// selection changes must scroll the list (ScrollStrategy::Nearest, same
    /// path as the Full Arg prompt) — otherwise choice six can be selected
    /// while rendered outside the viewport (clipped under the footer).
    #[test]
    fn selection_scrolls_nearest_after_up_and_down() {
        let render_fn_end = MINI_SOURCE
            .find("#[cfg(test)]")
            .unwrap_or(MINI_SOURCE.len());
        let render_code = &MINI_SOURCE[..render_fn_end];

        let up_handler_start = render_code
            .find("is_key_up(key)")
            .expect("mini prompt handles Up");
        let down_handler_start = render_code
            .find("is_key_down(key)")
            .expect("mini prompt handles Down");
        let tab_handler_start = render_code
            .find("eq_ignore_ascii_case(\"tab\")")
            .expect("mini prompt handles Tab");

        let up_body = &render_code[up_handler_start..down_handler_start];
        let down_body = &render_code[down_handler_start..tab_handler_start];
        for (name, body) in [("Up", up_body), ("Down", down_body)] {
            assert!(
                body.contains("scroll_to_item")
                    && body.contains("ScrollStrategy::Nearest")
                    && body.contains("arg_list_scroll_handle"),
                "mini prompt {name} selection must scroll the tracked list \
                 with ScrollStrategy::Nearest"
            );
        }
    }

    /// GEO-002: the Mini row slot comes from the shared arg_layout resolver
    /// (themed renderer metrics), never the stale LIST_ITEM_HEIGHT constant.
    #[test]
    fn mini_rows_use_resolved_row_slot_height() {
        let render_fn_end = MINI_SOURCE
            .find("#[cfg(test)]")
            .unwrap_or(MINI_SOURCE.len());
        let render_code = &MINI_SOURCE[..render_fn_end];
        assert!(
            render_code.contains("arg_layout::arg_row_slot_height()"),
            "mini prompt must resolve its row slot from arg_layout"
        );
        assert!(
            !render_code.contains("px(crate::list_item::LIST_ITEM_HEIGHT)"),
            "mini prompt must not size rows from the legacy LIST_ITEM_HEIGHT constant"
        );
    }

    #[test]
    fn mini_prompt_emits_structured_logs() {
        assert!(
            MINI_SOURCE.contains("tracing::info!("),
            "mini prompt should emit structured log at info level"
        );
        assert!(
            MINI_SOURCE.contains("prompt_type = \"mini\""),
            "mini prompt log should include prompt_type field"
        );
    }
}
