impl ScriptListApp {
    /// Render the arg input text with cursor and selection highlight
    fn render_arg_input_text(
        &self,
        text_primary: u32,
        accent_color: u32,
        cx: &gpui::App,
    ) -> gpui::Div {
        let text = self.arg_input.text();
        let text_muted = self.theme.colors.text.muted;
        let max_visible_chars = self.arg_input_max_visible_chars(cx);
        let (window_start, window_end) = self.arg_input.visible_window_range(max_visible_chars);
        let is_window_truncated_left = window_start > 0;
        let is_window_truncated_right = window_end < text.chars().count();
        // Separate focus state from blink state to avoid layout shift
        let is_focused = self.focused_input == FocusedInput::ArgPrompt;
        let is_cursor_visible = is_focused && self.cursor_visible;

        crate::components::text_input::render_text_input_cursor_selection(
            crate::components::text_input::TextInputRenderConfig {
                cursor: self.arg_input.cursor(),
                selection: Some(self.arg_input.selection()),
                window: Some((window_start, window_end)),
                cursor_visible: is_cursor_visible,
                cursor_color: text_primary,
                text_color: text_primary,
                selection_color: accent_color,
                selection_text_color: text_primary,
                container_height: Some(CURSOR_HEIGHT_LG + (CURSOR_MARGIN_Y * 2.0)),
                overflow_x_hidden: true,
                leading_indicator: is_window_truncated_left.then_some(
                    crate::components::text_input::TextInputRenderIndicator {
                        text: "...",
                        color: text_muted,
                    },
                ),
                trailing_indicator: is_window_truncated_right.then_some(
                    crate::components::text_input::TextInputRenderIndicator {
                        text: "...",
                        color: text_muted,
                    },
                ),
                ..crate::components::text_input::TextInputRenderConfig::default_for_prompt(text)
            },
        )
    }

    fn arg_input_max_visible_chars(&self, cx: &gpui::App) -> usize {
        const DEFAULT_WINDOW_WIDTH: f64 = crate::window_resize::MAIN_WINDOW_WIDTH as f64;
        const ARG_INPUT_WIDTH_PADDING_PX: f64 = (HEADER_PADDING_X as f64 * 2.0) + 12.0;
        // Fallback only for fonts whose `0` glyph cannot be measured.
        const ARG_INPUT_FALLBACK_CHAR_WIDTH_PX: f64 = 8.5;

        let window_width = crate::platform::get_main_window_bounds()
            .map(|(_, _, width, _)| width)
            .filter(|width| width.is_finite() && *width > 0.0)
            .unwrap_or(DEFAULT_WINDOW_WIDTH);

        // Measure the `ch` advance (digit-zero width) of the font the input
        // actually renders with, instead of assuming a flat 8.5px per char.
        let render_context = PromptRenderContext::new(self.theme.as_ref(), self.current_design);
        let typography = render_context.design_typography;
        let micro_font_size = typography.font_size_lg - 1.0;
        let text_system = cx.text_system();
        let font_id = text_system.resolve_font(&gpui::font(typography.font_family));
        let char_width = text_system
            .ch_advance(font_id, gpui::px(micro_font_size))
            .map(|advance| f64::from(f32::from(advance)))
            .ok()
            .filter(|width| width.is_finite() && *width > 0.0)
            .unwrap_or(ARG_INPUT_FALLBACK_CHAR_WIDTH_PX);

        arg_input_visible_chars_for_width(window_width - ARG_INPUT_WIDTH_PADDING_PX, char_width)
    }
    fn render_arg_prompt(
        &mut self,
        _id: String,
        _placeholder: String,
        choices: Vec<Choice>,
        actions: Option<Vec<ProtocolAction>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let render_context = PromptRenderContext::new(self.theme.as_ref(), self.current_design);
        let theme = render_context.theme;
        let design_spacing = render_context.design_spacing;
        let design_typography = render_context.design_typography;
        let actions_dialog_top = render_context.actions_dialog_top;
        let actions_dialog_right = render_context.actions_dialog_right;
        let menu_def = self.current_main_menu_theme.def();
        let _filtered = self.filtered_arg_choices();
        #[allow(clippy::unnecessary_map_or)]
        let has_actions = actions
            .as_ref()
            .map_or(false, |action_list| !action_list.is_empty());
        let has_choices = !choices.is_empty();

        // Navigation key handler — Escape, arrows, Tab, Cmd+K, actions
        // Text editing is handled by the Input component; Enter by the subscription
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
                        host: ActionsDialogHost::ArgPrompt,
                    },
                    has_actions_for_handler,
                    "ArgPrompt",
                ) {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;

                // Arrow up/down: list navigation
                if ui_foundation::is_key_up(key) && !modifiers.shift {
                    if this.arg_selected_index > 0 {
                        this.set_arg_selected_index(this.arg_selected_index - 1);
                        this.arg_list_scroll_handle
                            .scroll_to_item(this.arg_selected_index, ScrollStrategy::Nearest);
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }

                if ui_foundation::is_key_down(key) && !modifiers.shift {
                    let filtered = this.filtered_arg_choices();
                    if this.arg_selected_index < filtered.len().saturating_sub(1) {
                        this.set_arg_selected_index(this.arg_selected_index + 1);
                        this.arg_list_scroll_handle
                            .scroll_to_item(this.arg_selected_index, ScrollStrategy::Nearest);
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }

                if key.eq_ignore_ascii_case("tab") && !has_cmd && !modifiers.alt && !modifiers.shift
                {
                    this.apply_arg_tab_completion(window, cx);
                    cx.stop_propagation();
                    return;
                }

                // All other keys propagate to the Input component
                cx.propagate();
            },
        );

        // P4: Pre-compute theme values for arg prompt - use theme for consistent styling
        let arg_list_colors = ListItemColors::from_theme(theme);
        let text_primary = theme.colors.text.primary;

        // GEO-002: resolve the Full Arg row slot once from the active
        // renderer's themed metrics (canonical 44px general row). Rows and
        // the list viewport carry stable measurement IDs so the layout model
        // and paint measurements join by identity, not by name inference.
        let row_slot_height = crate::window_resize::arg_layout::arg_row_slot_height();

        // P0: Clone data needed for uniform_list closure
        let arg_selected_index = self.arg_selected_index;
        let filtered_choices = self.get_filtered_arg_choices_owned();
        let filtered_choices_len = filtered_choices.len();
        // NOTE: Removed per-render log - fires every render frame during cursor blink

        // P0: Build virtualized choice list using uniform_list
        let list_element: AnyElement = if filtered_choices_len == 0 {
            div()
                .w_full()
                .px(px(design_spacing.padding_md))
                .py(px(design_spacing.padding_sm))
                .font_family(design_typography.font_family)
                .child(crate::components::render_shared_empty_state(
                    crate::components::InfoEmptySurface::ArgChoices,
                    self.filter_text(),
                    self.theme.as_ref(),
                    cx,
                ))
                .into_any_element()
        } else {
            // P0: Use uniform_list for virtualized scrolling of arg choices
            // Now uses shared ListItem component for consistent design with script list
            uniform_list(
                "arg-choices",
                filtered_choices_len,
                move |visible_range, _window, _cx| {
                    // NOTE: Removed visible range log - fires per render frame
                    visible_range
                        .map(|ix| {
                            if let Some((_, choice)) = filtered_choices.get(ix) {
                                let is_selected = ix == arg_selected_index;

                                // Use shared ListItem component for consistent design
                                div()
                                    .id(ix)
                                    .debug_selector(move || {
                                        format!(
                                            "{}:{ix}",
                                            crate::window_resize::arg_layout::ARG_ROW_MEASUREMENT_ID_PREFIX
                                        )
                                    })
                                    .child(
                                        ListItem::new(choice.name.clone(), arg_list_colors)
                                            .description_opt(choice.description.clone())
                                            .selected(is_selected)
                                            .index(ix),
                                    )
                            } else {
                                // Fallback rows use the resolved row slot, not
                                // the stale LIST_ITEM_HEIGHT constant.
                                div().id(ix).h(px(row_slot_height))
                            }
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.arg_list_scroll_handle)
            .into_any_element()
        };

        let header = crate::components::main_view_chrome::render_prompt_search_input(
            theme,
            menu_def,
            crate::components::main_view_chrome::PromptSearchInputChrome::entity_backed(
                Input::new(&self.gpui_input_state),
            ),
        );

        let content = if has_choices {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .debug_selector(|| {
                    crate::window_resize::arg_layout::ARG_LIST_VIEWPORT_MEASUREMENT_ID.to_string()
                })
                .child(list_element)
        } else {
            div()
        };

        let filtered_choices_len = self.filtered_arg_choices().len();
        tracing::info!(
            surface = "render_prompts::arg",
            filtered_choices = filtered_choices_len,
            selected_index = self.arg_selected_index,
            "prompt_surface_rendered"
        );

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list("render_prompts::arg", has_actions),
        );

        let gpui_footer = crate::components::render_simple_hint_strip(
            crate::components::universal_prompt_hints(),
            None,
        );
        let footer = self.main_window_footer_slot(gpui_footer);

        let header = crate::components::main_view_chrome::MainViewHeaderChrome::canonical(
            menu_def,
            self.render_clickable_main_view_context_zone(menu_def, cx),
            header,
        );
        let mut overlays = Vec::new();
        if let Some(backdrop) = render_actions_backdrop(
            self.show_actions_popup,
            self.actions_dialog.clone(),
            actions_dialog_top,
            actions_dialog_right,
            ActionsBackdropConfig {
                backdrop_id: "arg-actions-backdrop",
                close_host: ActionsDialogHost::ArgPrompt,
                backdrop_log_message: "Arg actions backdrop clicked - dismissing dialog",
                show_pointer_cursor: true,
            },
            cx,
        ) {
            overlays.push(backdrop.into_any_element());
        }
        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(design_typography.font_family)
                .key_context("arg_prompt")
                .track_focus(&self.focus_handle)
                .capture_key_down(handle_key),
            theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header,
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: menu_def.shell.divider_margin_x,
                    height: menu_def.shell.divider_height,
                    visible: false,
                },
                main: content.into_any_element(),
                footer,
                overlays,
            },
        )
    }
}

#[cfg(test)]
mod arg_prompt_render_backdrop_tests {
    const ARG_RENDER_SOURCE: &str = include_str!("render.rs");

    #[test]
    fn test_arg_actions_backdrop_uses_shared_helper_with_clickable_cursor() {
        assert!(
            ARG_RENDER_SOURCE.contains("render_actions_backdrop("),
            "arg render should delegate backdrop overlay creation to shared helper"
        );
        assert!(
            ARG_RENDER_SOURCE.contains("\"arg-actions-backdrop\""),
            "arg render should pass its backdrop id to shared helper"
        );
        assert!(
            ARG_RENDER_SOURCE.contains("ActionsDialogHost::ArgPrompt"),
            "arg render should preserve actions host routing when helper is used"
        );
        assert!(
            ARG_RENDER_SOURCE.contains("show_pointer_cursor: true"),
            "arg render should keep backdrop cursor pointer enabled"
        );
    }

    #[test]
    fn test_arg_key_handler_uses_shared_preamble_helper() {
        assert!(
            ARG_RENDER_SOURCE.contains("handle_prompt_key_preamble("),
            "arg key handling should delegate preamble logic to shared helper"
        );
        assert!(
            ARG_RENDER_SOURCE.contains("PromptKeyPreambleCfg"),
            "arg key handling should configure the shared helper via PromptKeyPreambleCfg"
        );
    }
}
