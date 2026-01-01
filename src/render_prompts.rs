// Prompt render methods - extracted from app_render.rs
// This file is included via include!() macro in main.rs
// Contains: render_arg_prompt, render_div_prompt, render_form_prompt,
// render_term_prompt, render_editor_prompt, render_select_prompt,
// render_path_prompt, render_env_prompt, render_drop_prompt, render_template_prompt

impl ScriptListApp {
    fn render_arg_prompt(
        &mut self,
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        actions: Option<Vec<ProtocolAction>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let _theme = &self.theme;
        let _filtered = self.filtered_arg_choices();
        let has_actions = actions.is_some() && !actions.as_ref().unwrap().is_empty();
        let has_choices = !choices.is_empty();

        // Use design tokens for GLOBAL theming - all prompts use current design
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        // Key handler for arg prompt
        let prompt_id = id.clone();
        let has_actions_for_handler = has_actions;
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                let has_cmd = event.keystroke.modifiers.platform;
                logging::log(
                    "KEY",
                    &format!("ArgPrompt key: '{}' cmd={}", key_str, has_cmd),
                );

                // Check for Cmd+K to toggle actions popup (if actions are available)
                if has_cmd && key_str == "k" && has_actions_for_handler {
                    logging::log("KEY", "Cmd+K in ArgPrompt - calling toggle_arg_actions");
                    this.toggle_arg_actions(cx, window);
                    return;
                }

                // If actions popup is open, route keyboard events to it (same as main menu)
                if this.show_actions_popup {
                    if let Some(ref dialog) = this.actions_dialog {
                        match key_str.as_str() {
                            "up" | "arrowup" => {
                                dialog.update(cx, |d, cx| d.move_up(cx));
                                return;
                            }
                            "down" | "arrowdown" => {
                                dialog.update(cx, |d, cx| d.move_down(cx));
                                return;
                            }
                            "enter" => {
                                // Get the selected action and execute it
                                let action_id = dialog.read(cx).get_selected_action_id();
                                let should_close = dialog.read(cx).selected_action_should_close();
                                if let Some(action_id) = action_id {
                                    logging::log(
                                        "ACTIONS",
                                        &format!(
                                            "ArgPrompt executing action: {} (close={})",
                                            action_id, should_close
                                        ),
                                    );
                                    // Only close if action has close: true (default)
                                    if should_close {
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        this.focused_input = FocusedInput::ArgPrompt;
                                        window.focus(&this.focus_handle, cx);
                                    }
                                    // Trigger the SDK action by name
                                    this.trigger_action_by_name(&action_id, cx);
                                }
                                return;
                            }
                            "escape" => {
                                this.show_actions_popup = false;
                                this.actions_dialog = None;
                                this.focused_input = FocusedInput::ArgPrompt;
                                window.focus(&this.focus_handle, cx);
                                cx.notify();
                                return;
                            }
                            "backspace" => {
                                dialog.update(cx, |d, cx| d.handle_backspace(cx));
                                return;
                            }
                            _ => {
                                // Route character input to the dialog for search
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            dialog.update(cx, |d, cx| d.handle_char(ch, cx));
                                        }
                                    }
                                }
                                return;
                            }
                        }
                    }
                }

                // Check for SDK action shortcuts (only when actions popup is NOT open)
                let shortcut_key =
                    shortcuts::keystroke_to_shortcut(&key_str, &event.keystroke.modifiers);
                if let Some(action_name) = this.action_shortcuts.get(&shortcut_key).cloned() {
                    logging::log(
                        "KEY",
                        &format!("SDK action shortcut matched: {}", action_name),
                    );
                    this.trigger_action_by_name(&action_name, cx);
                    return;
                }

                match key_str.as_str() {
                    "up" | "arrowup" => {
                        if this.arg_selected_index > 0 {
                            this.arg_selected_index -= 1;
                            // P0: Scroll to keep selection visible
                            this.arg_list_scroll_handle
                                .scroll_to_item(this.arg_selected_index, ScrollStrategy::Nearest);
                            logging::log_debug(
                                "SCROLL",
                                &format!("P0: Arg up: selected_index={}", this.arg_selected_index),
                            );
                            cx.notify();
                        }
                    }
                    "down" | "arrowdown" => {
                        let filtered = this.filtered_arg_choices();
                        if this.arg_selected_index < filtered.len().saturating_sub(1) {
                            this.arg_selected_index += 1;
                            // P0: Scroll to keep selection visible
                            this.arg_list_scroll_handle
                                .scroll_to_item(this.arg_selected_index, ScrollStrategy::Nearest);
                            logging::log_debug(
                                "SCROLL",
                                &format!(
                                    "P0: Arg down: selected_index={}",
                                    this.arg_selected_index
                                ),
                            );
                            cx.notify();
                        }
                    }
                    "enter" => {
                        let filtered = this.filtered_arg_choices();
                        if let Some((_, choice)) = filtered.get(this.arg_selected_index) {
                            // Case 1: There are filtered choices - submit the selected one
                            let value = choice.value.clone();
                            this.submit_prompt_response(prompt_id.clone(), Some(value), cx);
                        } else if !this.arg_input_text.is_empty() {
                            // Case 2: No choices but user typed something - submit input_text
                            let value = this.arg_input_text.clone();
                            this.submit_prompt_response(prompt_id.clone(), Some(value), cx);
                        }
                        // Case 3: No choices and no input - do nothing (prevent empty submissions)
                    }
                    "escape" => {
                        logging::log("KEY", "ESC in ArgPrompt - canceling script");
                        // Send cancel response and clean up fully
                        this.submit_prompt_response(prompt_id.clone(), None, cx);
                        this.cancel_script_execution(cx);
                    }
                    "backspace" => {
                        if !this.arg_input_text.is_empty() {
                            this.arg_input_text.pop();
                            this.arg_selected_index = 0;
                            this.update_window_size();
                            cx.notify();
                        }
                    }
                    _ => {
                        if let Some(ref key_char) = event.keystroke.key_char {
                            if let Some(ch) = key_char.chars().next() {
                                if !ch.is_control() {
                                    this.arg_input_text.push(ch);
                                    this.arg_selected_index = 0;
                                    this.update_window_size();
                                    cx.notify();
                                }
                            }
                        }
                    }
                }
            },
        );

        let input_display = if self.arg_input_text.is_empty() {
            SharedString::from(placeholder.clone())
        } else {
            SharedString::from(self.arg_input_text.clone())
        };
        let input_is_empty = self.arg_input_text.is_empty();

        // P4: Pre-compute theme values for arg prompt using design tokens for GLOBAL theming
        let arg_list_colors = ListItemColors::from_design(&design_colors);
        let text_primary = design_colors.text_primary;
        let text_muted = design_colors.text_muted;

        // P0: Clone data needed for uniform_list closure
        let arg_selected_index = self.arg_selected_index;
        let filtered_choices = self.get_filtered_arg_choices_owned();
        let filtered_choices_len = filtered_choices.len();
        logging::log_debug(
            "UI",
            &format!(
                "P0: Arg prompt has {} filtered choices",
                filtered_choices_len
            ),
        );

        // P0: Build virtualized choice list using uniform_list
        let list_element: AnyElement = if filtered_choices_len == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(design_colors.text_muted))
                .font_family(design_typography.font_family)
                .child("No choices match your filter")
                .into_any_element()
        } else {
            // P0: Use uniform_list for virtualized scrolling of arg choices
            // Now uses shared ListItem component for consistent design with script list
            uniform_list(
                "arg-choices",
                filtered_choices_len,
                move |visible_range, _window, _cx| {
                    logging::log_debug(
                        "SCROLL",
                        &format!("P0: Arg choices visible range: {:?}", visible_range.clone()),
                    );
                    visible_range
                        .map(|ix| {
                            if let Some((_, choice)) = filtered_choices.get(ix) {
                                let is_selected = ix == arg_selected_index;

                                // Use shared ListItem component for consistent design
                                div().id(ix).child(
                                    ListItem::new(choice.name.clone(), arg_list_colors)
                                        .description_opt(choice.description.clone())
                                        .selected(is_selected)
                                        .with_accent_bar(true)
                                        .index(ix),
                                )
                            } else {
                                div().id(ix).h(px(LIST_ITEM_HEIGHT))
                            }
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.arg_list_scroll_handle)
            .into_any_element()
        };

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // P4: Pre-compute more theme values for the main container using design tokens
        let ui_border = design_colors.border;
        let text_dimmed = design_colors.text_dimmed;

        div()
            .relative() // Needed for absolute positioned actions dialog overlay
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("arg_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Search input with blinking cursor (same as main menu)
                    // Cursor appears at LEFT when input is empty (before placeholder text)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if input_is_empty {
                                rgb(text_muted)
                            } else {
                                rgb(text_primary)
                            })
                            // When empty: cursor FIRST (at left), then placeholder
                            // When typing: text, then cursor at end
                            //
                            // ALIGNMENT FIX: The left cursor (when empty) takes up space
                            // (CURSOR_WIDTH + CURSOR_GAP_X). We apply a negative margin to the
                            // placeholder text to pull it back by that amount.
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .mr(px(CURSOR_GAP_X))
                                        .when(
                                            self.focused_input == FocusedInput::ArgPrompt
                                                && self.cursor_visible,
                                            |d| d.bg(rgb(text_primary)),
                                        ),
                                )
                            })
                            // Display text - with negative margin for placeholder alignment
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .ml(px(-(CURSOR_WIDTH + CURSOR_GAP_X)))
                                        .child(input_display.clone()),
                                )
                            })
                            .when(!input_is_empty, |d| d.child(input_display.clone()))
                            .when(!input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .ml(px(CURSOR_GAP_X))
                                        .when(
                                            self.focused_input == FocusedInput::ArgPrompt
                                                && self.cursor_visible,
                                            |d| d.bg(rgb(text_primary)),
                                        ),
                                )
                            }),
                    )
                    // CLS-FREE ACTIONS AREA: Matches main menu pattern exactly
                    // Both states are always rendered at the same position, visibility toggled via opacity
                    .when(has_actions, |d| {
                        let button_colors = ButtonColors::from_theme(&self.theme);
                        let handle_actions = cx.entity().downgrade();
                        let show_actions = self.show_actions_popup;

                        // Get actions search text from the dialog
                        let search_text = self
                            .actions_dialog
                            .as_ref()
                            .map(|dialog| dialog.read(cx).search_text.clone())
                            .unwrap_or_default();
                        let search_is_empty = search_text.is_empty();
                        let search_display: SharedString = if search_is_empty {
                            "Search actions...".into()
                        } else {
                            search_text.into()
                        };
                        let accent_color = design_colors.accent;
                        let search_box_bg = self.theme.colors.background.search_box;
                        let cursor_visible_for_search = self.focused_input
                            == FocusedInput::ActionsSearch
                            && self.cursor_visible;

                        d.child(
                            div()
                                .relative()
                                .h(px(28.)) // Fixed height to prevent vertical CLS
                                .flex()
                                .items_center()
                                // Layer 1: Actions button - visible when NOT showing actions search
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .when(show_actions, |d| d.opacity(0.).invisible())
                                        .child(
                                            Button::new("Actions", button_colors)
                                                .variant(ButtonVariant::Ghost)
                                                .shortcut("⌘ K")
                                                .on_click(Box::new(move |_, window, cx| {
                                                    if let Some(app) = handle_actions.upgrade() {
                                                        app.update(cx, |this, cx| {
                                                            this.toggle_arg_actions(cx, window);
                                                        });
                                                    }
                                                })),
                                        )
                                        .child(
                                            div()
                                                .mx(px(4.))
                                                .text_color(rgba((text_dimmed << 8) | 0x60))
                                                .text_sm()
                                                .child("|"),
                                        ),
                                )
                                // Layer 2: Actions search input - visible when showing actions search
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(8.))
                                        .when(!show_actions, |d| d.opacity(0.).invisible())
                                        // ⌘K indicator
                                        .child(
                                            div()
                                                .text_color(rgb(text_dimmed))
                                                .text_xs()
                                                .child("⌘K"),
                                        )
                                        // Search input display - compact style matching buttons
                                        .child(
                                            div()
                                                .id("arg-actions-search")
                                                .flex_shrink_0()
                                                .w(px(130.))
                                                .min_w(px(130.))
                                                .max_w(px(130.))
                                                .h(px(24.))
                                                .min_h(px(24.))
                                                .max_h(px(24.))
                                                .overflow_hidden()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .px(px(8.))
                                                .rounded(px(4.))
                                                .bg(rgba(
                                                    (search_box_bg << 8)
                                                        | if search_is_empty { 0x40 } else { 0x80 },
                                                ))
                                                .border_1()
                                                .border_color(rgba(
                                                    (accent_color << 8)
                                                        | if search_is_empty { 0x20 } else { 0x40 },
                                                ))
                                                .text_sm()
                                                .text_color(if search_is_empty {
                                                    rgb(text_muted)
                                                } else {
                                                    rgb(text_primary)
                                                })
                                                // Cursor before placeholder when empty
                                                .when(search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .mr(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                })
                                                .child(search_display)
                                                // Cursor after text when not empty
                                                .when(!search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .ml(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                }),
                                        )
                                        .child(
                                            div()
                                                .mx(px(4.))
                                                .text_color(rgba((text_dimmed << 8) | 0x60))
                                                .text_sm()
                                                .child("|"),
                                        ),
                                ),
                        )
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(format!("{} choices", choices.len())),
                    ),
            )
            // Choices list (only when prompt has choices)
            .when(has_choices, |d| {
                d.child(
                    div()
                        .mx(px(design_spacing.padding_lg))
                        .h(px(design_visual.border_thin))
                        .bg(rgba((ui_border << 8) | 0x60)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.)) // P0: Allow flex container to shrink
                        .w_full()
                        .py(px(design_spacing.padding_xs))
                        .child(list_element),
                )
            })
            // Actions dialog overlay (when Cmd+K is pressed with SDK actions)
            // Uses same pattern as main menu: check BOTH show_actions_popup AND actions_dialog
            .when_some(
                if self.show_actions_popup {
                    self.actions_dialog.clone()
                } else {
                    None
                },
                |d, dialog| {
                    // Create click handler for backdrop to dismiss dialog
                    let backdrop_click = cx.listener(
                        |this: &mut Self,
                         _event: &gpui::ClickEvent,
                         window: &mut Window,
                         cx: &mut Context<Self>| {
                            logging::log(
                                "FOCUS",
                                "Arg actions backdrop clicked - dismissing dialog",
                            );
                            this.show_actions_popup = false;
                            this.actions_dialog = None;
                            this.focused_input = FocusedInput::ArgPrompt;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        },
                    );

                    d.child(
                        div()
                            .absolute()
                            .inset_0() // Cover entire arg prompt area
                            // Backdrop layer - captures clicks outside the dialog
                            .child(
                                div()
                                    .id("arg-actions-backdrop")
                                    .absolute()
                                    .inset_0()
                                    .on_click(backdrop_click),
                            )
                            // Dialog positioned at top-right
                            .child(
                                div()
                                    .absolute()
                                    .top(px(52.)) // Clear the header bar (~44px header + 8px margin)
                                    .right(px(8.))
                                    .child(dialog),
                            ),
                    )
                },
            )
            .into_any_element()
    }

    fn render_div_prompt(
        &mut self,
        entity: Entity<DivPrompt>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let has_actions =
            self.sdk_actions.is_some() && !self.sdk_actions.as_ref().unwrap().is_empty();

        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_visual = tokens.visual();

        // Key handler for Cmd+K actions toggle (at parent level to intercept before DivPrompt)
        let has_actions_for_handler = has_actions;
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                let has_cmd = event.keystroke.modifiers.platform;

                // Check for Cmd+K to toggle actions popup (if actions are available)
                if has_cmd && key_str == "k" && has_actions_for_handler {
                    logging::log("KEY", "Cmd+K in DivPrompt - calling toggle_arg_actions");
                    this.toggle_arg_actions(cx, window);
                    return;
                }

                // If actions popup is open, route keyboard events to it
                if this.show_actions_popup {
                    if let Some(ref dialog) = this.actions_dialog {
                        match key_str.as_str() {
                            "up" | "arrowup" => {
                                dialog.update(cx, |d, cx| d.move_up(cx));
                            }
                            "down" | "arrowdown" => {
                                dialog.update(cx, |d, cx| d.move_down(cx));
                            }
                            "enter" => {
                                let action_id = dialog.read(cx).get_selected_action_id();
                                let should_close = dialog.read(cx).selected_action_should_close();
                                if let Some(action_id) = action_id {
                                    logging::log(
                                        "ACTIONS",
                                        &format!(
                                            "DivPrompt executing action: {} (close={})",
                                            action_id, should_close
                                        ),
                                    );
                                    if should_close {
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        this.focused_input = FocusedInput::None;
                                        window.focus(&this.focus_handle, cx);
                                    }
                                    this.trigger_action_by_name(&action_id, cx);
                                }
                            }
                            "escape" => {
                                this.show_actions_popup = false;
                                this.actions_dialog = None;
                                this.focused_input = FocusedInput::None;
                                window.focus(&this.focus_handle, cx);
                                cx.notify();
                            }
                            "backspace" => {
                                dialog.update(cx, |d, cx| d.handle_backspace(cx));
                            }
                            _ => {
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            dialog.update(cx, |d, cx| d.handle_char(ch, cx));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // SDK action shortcuts are handled by DivPrompt's own key handler
            },
        );

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // Use explicit height from layout constants
        let content_height = window_resize::layout::STANDARD_HEIGHT;

        div()
            .relative() // Needed for absolute positioned actions dialog overlay
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h(content_height)
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .track_focus(&self.focus_handle) // Required to receive key events
            .on_key_down(handle_key)
            // CLS-FREE ACTIONS AREA: Matches main menu and ArgPrompt pattern
            .when(has_actions, |d| {
                let button_colors = ButtonColors::from_theme(&self.theme);
                let handle_actions = cx.entity().downgrade();
                let show_actions = self.show_actions_popup;

                // Get actions search text from the dialog
                let search_text = self
                    .actions_dialog
                    .as_ref()
                    .map(|dialog| dialog.read(cx).search_text.clone())
                    .unwrap_or_default();
                let search_is_empty = search_text.is_empty();
                let search_display: SharedString = if search_is_empty {
                    "Search actions...".into()
                } else {
                    search_text.into()
                };
                let accent_color = design_colors.accent;
                let text_primary = design_colors.text_primary;
                let text_muted = design_colors.text_muted;
                let text_dimmed = design_colors.text_dimmed;
                let search_box_bg = self.theme.colors.background.search_box;
                let cursor_visible_for_search =
                    self.focused_input == FocusedInput::ActionsSearch && self.cursor_visible;

                d.child(
                    div()
                        .w_full()
                        .px(px(design_spacing.padding_lg))
                        .py(px(design_spacing.padding_md))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_end()
                        .child(
                            div()
                                .relative()
                                .h(px(28.))
                                .flex()
                                .items_center()
                                // Layer 1: Actions button - visible when NOT showing actions search
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .when(show_actions, |d| d.opacity(0.).invisible())
                                        .child(
                                            Button::new("Actions", button_colors)
                                                .variant(ButtonVariant::Ghost)
                                                .shortcut("⌘ K")
                                                .on_click(Box::new(move |_, window, cx| {
                                                    if let Some(app) = handle_actions.upgrade() {
                                                        app.update(cx, |this, cx| {
                                                            this.toggle_arg_actions(cx, window);
                                                        });
                                                    }
                                                })),
                                        )
                                        .child(
                                            div()
                                                .mx(px(4.))
                                                .text_color(rgba((text_dimmed << 8) | 0x60))
                                                .text_sm()
                                                .child("|"),
                                        ),
                                )
                                // Layer 2: Actions search input - visible when showing actions search
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(8.))
                                        .when(!show_actions, |d| d.opacity(0.).invisible())
                                        .child(
                                            div()
                                                .text_color(rgb(text_dimmed))
                                                .text_xs()
                                                .child("⌘K"),
                                        )
                                        .child(
                                            div()
                                                .id("div-actions-search")
                                                .flex_shrink_0()
                                                .w(px(130.))
                                                .min_w(px(130.))
                                                .max_w(px(130.))
                                                .h(px(24.))
                                                .min_h(px(24.))
                                                .max_h(px(24.))
                                                .overflow_hidden()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .px(px(8.))
                                                .rounded(px(4.))
                                                .bg(rgba(
                                                    (search_box_bg << 8)
                                                        | if search_is_empty { 0x40 } else { 0x80 },
                                                ))
                                                .border_1()
                                                .border_color(rgba(
                                                    (accent_color << 8)
                                                        | if search_is_empty { 0x20 } else { 0x40 },
                                                ))
                                                .text_sm()
                                                .text_color(if search_is_empty {
                                                    rgb(text_muted)
                                                } else {
                                                    rgb(text_primary)
                                                })
                                                .when(search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .mr(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                })
                                                .child(search_display)
                                                .when(!search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .ml(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                }),
                                        )
                                        .child(
                                            div()
                                                .mx(px(4.))
                                                .text_color(rgba((text_dimmed << 8) | 0x60))
                                                .text_sm()
                                                .child("|"),
                                        ),
                                ),
                        ),
                )
            })
            // Content area - render the DivPrompt entity which handles HTML parsing and rendering
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.)) // Critical: allows flex children to size properly
                    .overflow_hidden()
                    .child(entity.clone()),
            )
            // Actions dialog overlay (when Cmd+K is pressed with SDK actions)
            .when_some(
                if self.show_actions_popup {
                    self.actions_dialog.clone()
                } else {
                    None
                },
                |d, dialog| {
                    let backdrop_click = cx.listener(
                        |this: &mut Self,
                         _event: &gpui::ClickEvent,
                         window: &mut Window,
                         cx: &mut Context<Self>| {
                            logging::log(
                                "FOCUS",
                                "Div actions backdrop clicked - dismissing dialog",
                            );
                            this.show_actions_popup = false;
                            this.actions_dialog = None;
                            this.focused_input = FocusedInput::None;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        },
                    );

                    d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .child(
                                div()
                                    .id("div-actions-backdrop")
                                    .absolute()
                                    .inset_0()
                                    .on_click(backdrop_click),
                            )
                            .child(div().absolute().top(px(52.)).right(px(8.)).child(dialog)),
                    )
                },
            )
            .into_any_element()
    }

    fn render_form_prompt(
        &mut self,
        entity: Entity<FormPromptState>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let has_actions =
            self.sdk_actions.is_some() && !self.sdk_actions.as_ref().unwrap().is_empty();

        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        // Get prompt ID and field count from entity
        let prompt_id = entity.read(cx).id.clone();
        let field_count = entity.read(cx).fields.len();

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
                let key_str = event.keystroke.key.to_lowercase();
                let has_shift = event.keystroke.modifiers.shift;
                let has_cmd = event.keystroke.modifiers.platform;

                logging::log(
                    "KEY",
                    &format!(
                        "FormPrompt key: '{}' (shift: {}, cmd: {}, key_char: {:?})",
                        key_str, has_shift, has_cmd, event.keystroke.key_char
                    ),
                );

                // Check for Cmd+K to toggle actions popup (if actions are available)
                if has_cmd && key_str == "k" && has_actions_for_handler {
                    logging::log("KEY", "Cmd+K in FormPrompt - calling toggle_arg_actions");
                    this.toggle_arg_actions(cx, window);
                    return;
                }

                // If actions popup is open, route keyboard events to it
                if this.show_actions_popup {
                    if let Some(ref dialog) = this.actions_dialog {
                        match key_str.as_str() {
                            "up" | "arrowup" => {
                                dialog.update(cx, |d, cx| d.move_up(cx));
                                return;
                            }
                            "down" | "arrowdown" => {
                                dialog.update(cx, |d, cx| d.move_down(cx));
                                return;
                            }
                            "enter" => {
                                let action_id = dialog.read(cx).get_selected_action_id();
                                let should_close = dialog.read(cx).selected_action_should_close();
                                if let Some(action_id) = action_id {
                                    logging::log(
                                        "ACTIONS",
                                        &format!(
                                            "FormPrompt executing action: {} (close={})",
                                            action_id, should_close
                                        ),
                                    );
                                    if should_close {
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        this.focused_input = FocusedInput::None;
                                        window.focus(&this.focus_handle, cx);
                                    }
                                    this.trigger_action_by_name(&action_id, cx);
                                }
                                return;
                            }
                            "escape" => {
                                this.show_actions_popup = false;
                                this.actions_dialog = None;
                                this.focused_input = FocusedInput::None;
                                window.focus(&this.focus_handle, cx);
                                cx.notify();
                                return;
                            }
                            "backspace" => {
                                dialog.update(cx, |d, cx| d.handle_backspace(cx));
                                return;
                            }
                            _ => {
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            dialog.update(cx, |d, cx| d.handle_char(ch, cx));
                                        }
                                    }
                                }
                                return;
                            }
                        }
                    }
                }

                // Check for SDK action shortcuts
                let shortcut_key =
                    shortcuts::keystroke_to_shortcut(&key_str, &event.keystroke.modifiers);
                if let Some(action_name) = this.action_shortcuts.get(&shortcut_key).cloned() {
                    logging::log(
                        "KEY",
                        &format!("SDK action shortcut matched: {}", action_name),
                    );
                    this.trigger_action_by_name(&action_name, cx);
                    return;
                }

                // Handle form-level keys (Enter, Escape, Tab) at this level
                // Forward all other keys to the focused form field for text input
                match key_str.as_str() {
                    "enter" => {
                        // Enter submits the form - collect all field values
                        logging::log("KEY", "Enter in FormPrompt - submitting form");
                        let values = entity_for_submit.read(cx).collect_values(cx);
                        logging::log("FORM", &format!("Form values: {}", values));
                        this.submit_prompt_response(prompt_id_for_key.clone(), Some(values), cx);
                    }
                    "escape" => {
                        // ESC cancels the script completely
                        logging::log("KEY", "ESC in FormPrompt - canceling script");
                        this.submit_prompt_response(prompt_id_for_key.clone(), None, cx);
                        this.cancel_script_execution(cx);
                    }
                    "tab" => {
                        // Tab navigation between fields
                        if has_shift {
                            entity_for_shift_tab.update(cx, |form, cx| {
                                form.focus_previous(cx);
                            });
                        } else {
                            entity_for_tab.update(cx, |form, cx| {
                                form.focus_next(cx);
                            });
                        }
                    }
                    _ => {
                        // Forward all other keys (characters, backspace, arrows, etc.) to the focused field
                        // This is necessary because GPUI requires track_focus() to receive key events,
                        // and we need the parent to have focus to handle Enter/Escape/Tab.
                        // The form fields' individual on_key_down handlers don't fire when parent has focus.
                        entity_for_input.update(cx, |form, cx| {
                            form.handle_key_input(event, cx);
                        });
                    }
                }
            },
        );

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // Dynamic height based on field count
        // Base height (150px) + per-field height (60px per field)
        // Minimum of calculated height and MAX_HEIGHT (700px)
        let base_height = 150.0;
        let field_height = 60.0;
        let calculated_height = base_height + (field_count as f32 * field_height);
        let max_height = 700.0; // Same as window_resize::layout::MAX_HEIGHT
        let content_height = px(calculated_height.min(max_height));

        // Button colors from theme
        let button_colors = ButtonColors::from_theme(&self.theme);
        let actions_button_colors = ButtonColors::from_theme(&self.theme);

        // Form fields have their own focus handles and on_key_down handlers.
        // We DO NOT track_focus on the container - the fields handle their own focus.
        // Enter/Escape/Tab are handled by the handle_key listener above.
        div()
            .relative() // Needed for absolute positioned actions dialog overlay
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h(content_height)
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(design_colors.text_primary))
            .font_family(design_typography.font_family)
            .key_context("form_prompt")
            .on_key_down(handle_key)
            // Content area with form fields
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.))
                    .overflow_y_hidden() // Clip content at container boundary
                    .p(px(design_spacing.padding_xl))
                    // Render the form entity (contains all fields)
                    .child(entity.clone()),
            )
            // Header with CLS-FREE Actions area and Submit button
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    // CLS-FREE ACTIONS AREA
                    .when(has_actions, |d| {
                        let handle_actions = cx.entity().downgrade();
                        let show_actions_state = self.show_actions_popup;

                        let search_text = self
                            .actions_dialog
                            .as_ref()
                            .map(|dialog| dialog.read(cx).search_text.clone())
                            .unwrap_or_default();
                        let search_is_empty = search_text.is_empty();
                        let search_display: SharedString = if search_is_empty {
                            "Search actions...".into()
                        } else {
                            search_text.into()
                        };
                        let accent_color = design_colors.accent;
                        let text_primary = design_colors.text_primary;
                        let text_muted = design_colors.text_muted;
                        let text_dimmed = design_colors.text_dimmed;
                        let search_box_bg = self.theme.colors.background.search_box;
                        let cursor_visible_for_search = self.focused_input
                            == FocusedInput::ActionsSearch
                            && self.cursor_visible;

                        d.child(
                            div()
                                .relative()
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .when(show_actions_state, |d| d.opacity(0.).invisible())
                                        .child(
                                            Button::new("Actions", actions_button_colors)
                                                .variant(ButtonVariant::Ghost)
                                                .shortcut("⌘ K")
                                                .on_click(Box::new(move |_, window, cx| {
                                                    if let Some(app) = handle_actions.upgrade() {
                                                        app.update(cx, |this, cx| {
                                                            this.toggle_arg_actions(cx, window);
                                                        });
                                                    }
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(8.))
                                        .when(!show_actions_state, |d| d.opacity(0.).invisible())
                                        .child(
                                            div()
                                                .text_color(rgb(text_dimmed))
                                                .text_xs()
                                                .child("⌘K"),
                                        )
                                        .child(
                                            div()
                                                .id("form-actions-search")
                                                .flex_shrink_0()
                                                .w(px(130.))
                                                .min_w(px(130.))
                                                .max_w(px(130.))
                                                .h(px(24.))
                                                .min_h(px(24.))
                                                .max_h(px(24.))
                                                .overflow_hidden()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .px(px(8.))
                                                .rounded(px(4.))
                                                .bg(rgba(
                                                    (search_box_bg << 8)
                                                        | if search_is_empty { 0x40 } else { 0x80 },
                                                ))
                                                .border_1()
                                                .border_color(rgba(
                                                    (accent_color << 8)
                                                        | if search_is_empty { 0x20 } else { 0x40 },
                                                ))
                                                .text_sm()
                                                .text_color(if search_is_empty {
                                                    rgb(text_muted)
                                                } else {
                                                    rgb(text_primary)
                                                })
                                                .when(search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .mr(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                })
                                                .child(search_display)
                                                .when(!search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .ml(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                }),
                                        ),
                                ),
                        )
                    })
                    // Submit button
                    .child(
                        Button::new("Submit", button_colors)
                            .variant(ButtonVariant::Primary)
                            .shortcut("↵"),
                    ),
            )
            // Actions dialog overlay
            .when_some(
                if self.show_actions_popup {
                    self.actions_dialog.clone()
                } else {
                    None
                },
                |d, dialog| {
                    let backdrop_click = cx.listener(
                        |this: &mut Self,
                         _event: &gpui::ClickEvent,
                         window: &mut Window,
                         cx: &mut Context<Self>| {
                            logging::log(
                                "FOCUS",
                                "Form actions backdrop clicked - dismissing dialog",
                            );
                            this.show_actions_popup = false;
                            this.actions_dialog = None;
                            this.focused_input = FocusedInput::None;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        },
                    );

                    d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .child(
                                div()
                                    .id("form-actions-backdrop")
                                    .absolute()
                                    .inset_0()
                                    .on_click(backdrop_click),
                            )
                            .child(div().absolute().top(px(52.)).right(px(8.)).child(dialog)),
                    )
                },
            )
            .into_any_element()
    }

    fn render_term_prompt(
        &mut self,
        entity: Entity<term_prompt::TermPrompt>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let has_actions =
            self.sdk_actions.is_some() && !self.sdk_actions.as_ref().unwrap().is_empty();

        // Sync suppress_keys with actions popup state so terminal ignores keys when popup is open
        let show_actions = self.show_actions_popup;
        entity.update(cx, |term, _| {
            term.suppress_keys = show_actions;
        });

        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // Use explicit height from layout constants instead of h_full()
        // h_full() doesn't work at the root level because there's no parent to fill
        let content_height = window_resize::layout::MAX_HEIGHT;

        // Key handler for Cmd+K actions toggle
        let has_actions_for_handler = has_actions;
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                let has_cmd = event.keystroke.modifiers.platform;

                // Check for Cmd+K to toggle actions popup (if actions are available)
                if has_cmd && key_str == "k" && has_actions_for_handler {
                    logging::log("KEY", "Cmd+K in TermPrompt - calling toggle_arg_actions");
                    this.toggle_arg_actions(cx, window);
                    return;
                }

                // If actions popup is open, route keyboard events to it
                if this.show_actions_popup {
                    if let Some(ref dialog) = this.actions_dialog {
                        match key_str.as_str() {
                            "up" | "arrowup" => {
                                dialog.update(cx, |d, cx| d.move_up(cx));
                                return;
                            }
                            "down" | "arrowdown" => {
                                dialog.update(cx, |d, cx| d.move_down(cx));
                                return;
                            }
                            "enter" => {
                                let action_id = dialog.read(cx).get_selected_action_id();
                                let should_close = dialog.read(cx).selected_action_should_close();
                                if let Some(action_id) = action_id {
                                    logging::log(
                                        "ACTIONS",
                                        &format!(
                                            "TermPrompt executing action: {} (close={})",
                                            action_id, should_close
                                        ),
                                    );
                                    if should_close {
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        this.focused_input = FocusedInput::None;
                                        window.focus(&this.focus_handle, cx);
                                    }
                                    this.trigger_action_by_name(&action_id, cx);
                                }
                                return;
                            }
                            "escape" => {
                                this.show_actions_popup = false;
                                this.actions_dialog = None;
                                this.focused_input = FocusedInput::None;
                                window.focus(&this.focus_handle, cx);
                                cx.notify();
                                return;
                            }
                            "backspace" => {
                                dialog.update(cx, |d, cx| d.handle_backspace(cx));
                                return;
                            }
                            _ => {
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            dialog.update(cx, |d, cx| d.handle_char(ch, cx));
                                        }
                                    }
                                }
                                return;
                            }
                        }
                    }
                }

                // Check for SDK action shortcuts
                let shortcut_key =
                    shortcuts::keystroke_to_shortcut(&key_str, &event.keystroke.modifiers);
                if let Some(action_name) = this.action_shortcuts.get(&shortcut_key).cloned() {
                    logging::log(
                        "KEY",
                        &format!("SDK action shortcut matched: {}", action_name),
                    );
                    this.trigger_action_by_name(&action_name, cx);
                }
                // Let other keys fall through to the terminal
            },
        );

        // Container with explicit height. We wrap the entity in a sized div because
        // GPUI entities don't automatically inherit parent flex sizing.
        // NOTE: No rounded corners for terminal - it should fill edge-to-edge
        div()
            .relative() // Needed for absolute positioned actions dialog overlay
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h(content_height)
            .overflow_hidden()
            .on_key_down(handle_key)
            .child(div().size_full().child(entity))
            // CLS-FREE ACTIONS AREA in top-right corner
            .when(has_actions, |d| {
                let button_colors = ButtonColors::from_theme(&self.theme);
                let handle_actions = cx.entity().downgrade();
                let show_actions_state = self.show_actions_popup;

                // Get actions search text from the dialog
                let search_text = self
                    .actions_dialog
                    .as_ref()
                    .map(|dialog| dialog.read(cx).search_text.clone())
                    .unwrap_or_default();
                let search_is_empty = search_text.is_empty();
                let search_display: SharedString = if search_is_empty {
                    "Search actions...".into()
                } else {
                    search_text.into()
                };
                let accent_color = design_colors.accent;
                let text_primary = design_colors.text_primary;
                let text_muted = design_colors.text_muted;
                let text_dimmed = design_colors.text_dimmed;
                let search_box_bg = self.theme.colors.background.search_box;
                let cursor_visible_for_search =
                    self.focused_input == FocusedInput::ActionsSearch && self.cursor_visible;

                d.child(
                    div()
                        .absolute()
                        .top(px(design_spacing.padding_md))
                        .right(px(design_spacing.padding_md))
                        .child(
                            div()
                                .relative()
                                .h(px(28.))
                                .flex()
                                .items_center()
                                // Layer 1: Actions button
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .when(show_actions_state, |d| d.opacity(0.).invisible())
                                        .child(
                                            Button::new("Actions", button_colors)
                                                .variant(ButtonVariant::Ghost)
                                                .shortcut("⌘ K")
                                                .on_click(Box::new(move |_, window, cx| {
                                                    if let Some(app) = handle_actions.upgrade() {
                                                        app.update(cx, |this, cx| {
                                                            this.toggle_arg_actions(cx, window);
                                                        });
                                                    }
                                                })),
                                        ),
                                )
                                // Layer 2: Actions search input
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(8.))
                                        .when(!show_actions_state, |d| d.opacity(0.).invisible())
                                        .child(
                                            div()
                                                .text_color(rgb(text_dimmed))
                                                .text_xs()
                                                .child("⌘K"),
                                        )
                                        .child(
                                            div()
                                                .id("term-actions-search")
                                                .flex_shrink_0()
                                                .w(px(130.))
                                                .min_w(px(130.))
                                                .max_w(px(130.))
                                                .h(px(24.))
                                                .min_h(px(24.))
                                                .max_h(px(24.))
                                                .overflow_hidden()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .px(px(8.))
                                                .rounded(px(4.))
                                                .bg(rgba(
                                                    (search_box_bg << 8)
                                                        | if search_is_empty { 0x40 } else { 0x80 },
                                                ))
                                                .border_1()
                                                .border_color(rgba(
                                                    (accent_color << 8)
                                                        | if search_is_empty { 0x20 } else { 0x40 },
                                                ))
                                                .text_sm()
                                                .text_color(if search_is_empty {
                                                    rgb(text_muted)
                                                } else {
                                                    rgb(text_primary)
                                                })
                                                .when(search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .mr(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                })
                                                .child(search_display)
                                                .when(!search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .ml(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                }),
                                        ),
                                ),
                        ),
                )
            })
            // Actions dialog overlay
            .when_some(
                if self.show_actions_popup {
                    self.actions_dialog.clone()
                } else {
                    None
                },
                |d, dialog| {
                    let backdrop_click = cx.listener(
                        |this: &mut Self,
                         _event: &gpui::ClickEvent,
                         window: &mut Window,
                         cx: &mut Context<Self>| {
                            logging::log(
                                "FOCUS",
                                "Term actions backdrop clicked - dismissing dialog",
                            );
                            this.show_actions_popup = false;
                            this.actions_dialog = None;
                            this.focused_input = FocusedInput::None;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        },
                    );

                    d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .child(
                                div()
                                    .id("term-actions-backdrop")
                                    .absolute()
                                    .inset_0()
                                    .on_click(backdrop_click),
                            )
                            .child(div().absolute().top(px(52.)).right(px(8.)).child(dialog)),
                    )
                },
            )
            .into_any_element()
    }

    fn render_editor_prompt(
        &mut self,
        entity: Entity<EditorPrompt>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let has_actions =
            self.sdk_actions.is_some() && !self.sdk_actions.as_ref().unwrap().is_empty();

        // Sync suppress_keys with actions popup state so editor ignores keys when popup is open
        let show_actions = self.show_actions_popup;
        entity.update(cx, |editor, _| {
            editor.suppress_keys = show_actions;
        });

        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // Use explicit height from layout constants instead of h_full()
        // h_full() doesn't work at the root level because there's no parent to fill
        let content_height = window_resize::layout::MAX_HEIGHT;

        // Key handler for Cmd+K actions toggle (at parent level to intercept before editor)
        let has_actions_for_handler = has_actions;
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                let has_cmd = event.keystroke.modifiers.platform;

                // Check for Cmd+K to toggle actions popup (if actions are available)
                if has_cmd && key_str == "k" && has_actions_for_handler {
                    logging::log("KEY", "Cmd+K in EditorPrompt - calling toggle_arg_actions");
                    this.toggle_arg_actions(cx, window);
                    return;
                }

                // If actions popup is open, route keyboard events to it
                if this.show_actions_popup {
                    if let Some(ref dialog) = this.actions_dialog {
                        match key_str.as_str() {
                            "up" | "arrowup" => {
                                dialog.update(cx, |d, cx| d.move_up(cx));
                                return;
                            }
                            "down" | "arrowdown" => {
                                dialog.update(cx, |d, cx| d.move_down(cx));
                                return;
                            }
                            "enter" => {
                                let action_id = dialog.read(cx).get_selected_action_id();
                                let should_close = dialog.read(cx).selected_action_should_close();
                                if let Some(action_id) = action_id {
                                    logging::log(
                                        "ACTIONS",
                                        &format!(
                                            "EditorPrompt executing action: {} (close={})",
                                            action_id, should_close
                                        ),
                                    );
                                    if should_close {
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        this.focused_input = FocusedInput::None;
                                        window.focus(&this.focus_handle, cx);
                                    }
                                    this.trigger_action_by_name(&action_id, cx);
                                }
                                return;
                            }
                            "escape" => {
                                this.show_actions_popup = false;
                                this.actions_dialog = None;
                                this.focused_input = FocusedInput::None;
                                window.focus(&this.focus_handle, cx);
                                cx.notify();
                                return;
                            }
                            "backspace" => {
                                dialog.update(cx, |d, cx| d.handle_backspace(cx));
                                return;
                            }
                            _ => {
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            dialog.update(cx, |d, cx| d.handle_char(ch, cx));
                                        }
                                    }
                                }
                                return;
                            }
                        }
                    }
                }

                // Check for SDK action shortcuts
                let shortcut_key =
                    shortcuts::keystroke_to_shortcut(&key_str, &event.keystroke.modifiers);
                if let Some(action_name) = this.action_shortcuts.get(&shortcut_key).cloned() {
                    logging::log(
                        "KEY",
                        &format!("SDK action shortcut matched: {}", action_name),
                    );
                    this.trigger_action_by_name(&action_name, cx);
                }
                // Let other keys fall through to the editor
            },
        );

        // NOTE: The EditorPrompt entity has its own track_focus and on_key_down in its render method.
        // We do NOT add track_focus here to avoid duplicate focus tracking on the same handle.
        //
        // Container with explicit height. We wrap the entity in a sized div because
        // GPUI entities don't automatically inherit parent flex sizing.
        div()
            .relative() // Needed for absolute positioned actions dialog overlay
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h(content_height)
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .on_key_down(handle_key)
            .child(div().size_full().child(entity))
            // CLS-FREE ACTIONS AREA in top-right corner
            .when(has_actions, |d| {
                let button_colors = ButtonColors::from_theme(&self.theme);
                let handle_actions = cx.entity().downgrade();
                let show_actions_state = self.show_actions_popup;

                // Get actions search text from the dialog
                let search_text = self
                    .actions_dialog
                    .as_ref()
                    .map(|dialog| dialog.read(cx).search_text.clone())
                    .unwrap_or_default();
                let search_is_empty = search_text.is_empty();
                let search_display: SharedString = if search_is_empty {
                    "Search actions...".into()
                } else {
                    search_text.into()
                };
                let accent_color = design_colors.accent;
                let text_primary = design_colors.text_primary;
                let text_muted = design_colors.text_muted;
                let text_dimmed = design_colors.text_dimmed;
                let search_box_bg = self.theme.colors.background.search_box;
                let cursor_visible_for_search =
                    self.focused_input == FocusedInput::ActionsSearch && self.cursor_visible;

                d.child(
                    div()
                        .absolute()
                        .top(px(design_spacing.padding_md))
                        .right(px(design_spacing.padding_md))
                        .child(
                            div()
                                .relative()
                                .h(px(28.))
                                .flex()
                                .items_center()
                                // Layer 1: Actions button
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .when(show_actions_state, |d| d.opacity(0.).invisible())
                                        .child(
                                            Button::new("Actions", button_colors)
                                                .variant(ButtonVariant::Ghost)
                                                .shortcut("⌘ K")
                                                .on_click(Box::new(move |_, window, cx| {
                                                    if let Some(app) = handle_actions.upgrade() {
                                                        app.update(cx, |this, cx| {
                                                            this.toggle_arg_actions(cx, window);
                                                        });
                                                    }
                                                })),
                                        ),
                                )
                                // Layer 2: Actions search input
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(8.))
                                        .when(!show_actions_state, |d| d.opacity(0.).invisible())
                                        .child(
                                            div()
                                                .text_color(rgb(text_dimmed))
                                                .text_xs()
                                                .child("⌘K"),
                                        )
                                        .child(
                                            div()
                                                .id("editor-actions-search")
                                                .flex_shrink_0()
                                                .w(px(130.))
                                                .min_w(px(130.))
                                                .max_w(px(130.))
                                                .h(px(24.))
                                                .min_h(px(24.))
                                                .max_h(px(24.))
                                                .overflow_hidden()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .px(px(8.))
                                                .rounded(px(4.))
                                                .bg(rgba(
                                                    (search_box_bg << 8)
                                                        | if search_is_empty { 0x40 } else { 0x80 },
                                                ))
                                                .border_1()
                                                .border_color(rgba(
                                                    (accent_color << 8)
                                                        | if search_is_empty { 0x20 } else { 0x40 },
                                                ))
                                                .text_sm()
                                                .text_color(if search_is_empty {
                                                    rgb(text_muted)
                                                } else {
                                                    rgb(text_primary)
                                                })
                                                .when(search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .mr(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                })
                                                .child(search_display)
                                                .when(!search_is_empty, |d| {
                                                    d.child(
                                                        div()
                                                            .w(px(2.))
                                                            .h(px(14.))
                                                            .ml(px(2.))
                                                            .rounded(px(1.))
                                                            .when(cursor_visible_for_search, |d| {
                                                                d.bg(rgb(accent_color))
                                                            }),
                                                    )
                                                }),
                                        ),
                                ),
                        ),
                )
            })
            // Actions dialog overlay
            .when_some(
                if self.show_actions_popup {
                    self.actions_dialog.clone()
                } else {
                    None
                },
                |d, dialog| {
                    let backdrop_click = cx.listener(
                        |this: &mut Self,
                         _event: &gpui::ClickEvent,
                         window: &mut Window,
                         cx: &mut Context<Self>| {
                            logging::log(
                                "FOCUS",
                                "Editor actions backdrop clicked - dismissing dialog",
                            );
                            this.show_actions_popup = false;
                            this.actions_dialog = None;
                            this.focused_input = FocusedInput::None;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        },
                    );

                    d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .child(
                                div()
                                    .id("editor-actions-backdrop")
                                    .absolute()
                                    .inset_0()
                                    .on_click(backdrop_click),
                            )
                            .child(div().absolute().top(px(52.)).right(px(8.)).child(dialog)),
                    )
                },
            )
            .into_any_element()
    }

    fn render_select_prompt(
        &mut self,
        entity: Entity<SelectPrompt>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // SelectPrompt entity has its own track_focus and on_key_down in its render method.
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .child(div().size_full().child(entity))
            .into_any_element()
    }

    fn render_path_prompt(
        &mut self,
        entity: Entity<PathPrompt>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // Check if we should close the actions dialog
        if let Ok(mut close_guard) = self.close_path_actions.lock() {
            if *close_guard {
                *close_guard = false;
                self.show_actions_popup = false;
                self.actions_dialog = None;
                // Update shared showing state for toggle behavior
                if let Ok(mut showing_guard) = self.path_actions_showing.lock() {
                    *showing_guard = false;
                }
                logging::log("UI", "Closed path actions dialog");
            }
        }

        // Check for pending path action result and execute it
        // Extract data first, then drop the lock, then execute
        let pending_action = self
            .pending_path_action_result
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());

        if let Some((action_id, path_info)) = pending_action {
            self.execute_path_action(&action_id, &path_info, &entity, cx);
        }

        // Check for pending path action and create ActionsDialog if needed
        let actions_dialog = if let Ok(mut guard) = self.pending_path_action.lock() {
            if let Some(path_info) = guard.take() {
                // Create ActionsDialog for this path
                let theme_arc = std::sync::Arc::new(self.theme.clone());
                let close_signal = self.close_path_actions.clone();
                let action_result_signal = self.pending_path_action_result.clone();
                let path_info_for_callback = path_info.clone();
                let action_callback: std::sync::Arc<dyn Fn(String) + Send + Sync> =
                    std::sync::Arc::new(move |action_id| {
                        logging::log(
                            "UI",
                            &format!(
                                "Path action selected: {} for path: {}",
                                action_id, path_info_for_callback.path
                            ),
                        );
                        // Store the action result for execution
                        if action_id != "__cancel__" {
                            if let Ok(mut guard) = action_result_signal.lock() {
                                *guard = Some((action_id.clone(), path_info_for_callback.clone()));
                            }
                        }
                        // Signal to close dialog on cancel or action selection
                        if let Ok(mut guard) = close_signal.lock() {
                            *guard = true;
                        }
                    });
                let dialog = cx.new(|cx| {
                    let focus_handle = cx.focus_handle();
                    let mut dialog = ActionsDialog::with_path(
                        focus_handle,
                        action_callback,
                        &path_info,
                        theme_arc,
                    );
                    // Hide search in the dialog - we show it in the header instead
                    dialog.set_hide_search(true);
                    dialog
                });
                self.actions_dialog = Some(dialog.clone());
                self.show_actions_popup = true;
                // Update shared showing state for toggle behavior
                if let Ok(mut showing_guard) = self.path_actions_showing.lock() {
                    *showing_guard = true;
                }
                Some(dialog)
            } else if self.show_actions_popup {
                self.actions_dialog.clone()
            } else {
                None
            }
        } else {
            None
        };

        // Sync the actions search text from the dialog to the shared state
        // This allows PathPrompt to display the search text in its header
        if let Some(ref dialog) = actions_dialog {
            let search_text = dialog.read(cx).search_text.clone();
            if let Ok(mut guard) = self.path_actions_search_text.lock() {
                *guard = search_text;
            }
        } else {
            // Clear search text when dialog is not showing
            if let Ok(mut guard) = self.path_actions_search_text.lock() {
                guard.clear();
            }
        }

        // Key handler for when actions dialog is showing
        // This intercepts keys and routes them to the dialog (like main menu does)
        let path_entity = entity.clone();
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                let has_cmd = event.keystroke.modifiers.platform;

                logging::log(
                    "KEY",
                    &format!(
                        "PathPrompt OUTER handler: key='{}', show_actions_popup={}",
                        key_str, this.show_actions_popup
                    ),
                );

                // Cmd+K toggles actions from anywhere
                if has_cmd && key_str == "k" {
                    // Toggle the actions dialog
                    if this.show_actions_popup {
                        // Close actions
                        this.show_actions_popup = false;
                        this.actions_dialog = None;
                        if let Ok(mut guard) = this.path_actions_showing.lock() {
                            *guard = false;
                        }
                        cx.notify();
                    } else {
                        // Open actions - trigger the callback in PathPrompt
                        path_entity.update(cx, |prompt, cx| {
                            prompt.toggle_actions(cx);
                        });
                    }
                    return;
                }

                // If actions popup is open, route keyboard events to it
                if this.show_actions_popup {
                    if let Some(ref dialog) = this.actions_dialog {
                        match key_str.as_str() {
                            "up" | "arrowup" => {
                                dialog.update(cx, |d, cx| d.move_up(cx));
                            }
                            "down" | "arrowdown" => {
                                dialog.update(cx, |d, cx| d.move_down(cx));
                            }
                            "enter" => {
                                // Get the selected action and execute it
                                let action_id = dialog.read(cx).get_selected_action_id();
                                let should_close = dialog.read(cx).selected_action_should_close();
                                if let Some(action_id) = action_id {
                                    logging::log(
                                        "ACTIONS",
                                        &format!(
                                            "Path action selected via Enter: {} (close={})",
                                            action_id, should_close
                                        ),
                                    );

                                    // Get path info from PathPrompt
                                    let path_info = path_entity.read(cx).get_selected_path_info();

                                    // Close dialog if action says so (built-in path actions always close)
                                    if should_close {
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        if let Ok(mut guard) = this.path_actions_showing.lock() {
                                            *guard = false;
                                        }

                                        // Focus back to PathPrompt
                                        if let AppView::PathPrompt { focus_handle, .. } =
                                            &this.current_view
                                        {
                                            window.focus(focus_handle, cx);
                                        }
                                    }

                                    // Execute the action if we have path info
                                    if let Some(info) = path_info {
                                        this.execute_path_action(
                                            &action_id,
                                            &info,
                                            &path_entity,
                                            cx,
                                        );
                                    }
                                }
                            }
                            "escape" => {
                                this.show_actions_popup = false;
                                this.actions_dialog = None;
                                if let Ok(mut guard) = this.path_actions_showing.lock() {
                                    *guard = false;
                                }
                                // Focus back to PathPrompt
                                if let AppView::PathPrompt { focus_handle, .. } = &this.current_view
                                {
                                    window.focus(focus_handle, cx);
                                }
                                cx.notify();
                            }
                            "backspace" => {
                                dialog.update(cx, |d, cx| d.handle_backspace(cx));
                            }
                            _ => {
                                // Route character input to the dialog for search
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            dialog.update(cx, |d, cx| d.handle_char(ch, cx));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // If actions not showing, let PathPrompt handle the keys via its own handler
            },
        );

        // PathPrompt entity has its own track_focus and on_key_down in its render method.
        // We add an outer key handler to intercept events when actions are showing.
        div()
            .relative()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .key_context("path_prompt_container")
            .on_key_down(handle_key)
            .child(div().size_full().child(entity))
            // Actions dialog overlays on top (upper-right corner, below the header bar)
            .when_some(actions_dialog, |d, dialog| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .justify_end()
                        .pt(px(52.)) // Clear the header bar (~44px header + 8px margin)
                        .pr(px(8.))
                        .child(dialog),
                )
            })
            .into_any_element()
    }

    fn render_env_prompt(
        &mut self,
        entity: Entity<EnvPrompt>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // EnvPrompt entity has its own track_focus and on_key_down in its render method.
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .child(div().size_full().child(entity))
            .into_any_element()
    }

    fn render_drop_prompt(
        &mut self,
        entity: Entity<DropPrompt>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // DropPrompt entity has its own track_focus and on_key_down in its render method.
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .child(div().size_full().child(entity))
            .into_any_element()
    }

    fn render_template_prompt(
        &mut self,
        entity: Entity<TemplatePrompt>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // TemplatePrompt entity has its own track_focus and on_key_down in its render method.
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .child(div().size_full().child(entity))
            .into_any_element()
    }
}
