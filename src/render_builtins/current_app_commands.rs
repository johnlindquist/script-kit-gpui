#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurrentAppCommandsEmptyState {
    NoCommandsReady,
    NoFilteredMatches,
}

impl CurrentAppCommandsEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.trim().is_empty() {
            Self::NoCommandsReady
        } else {
            Self::NoFilteredMatches
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::NoCommandsReady => "No commands ready yet",
            Self::NoFilteredMatches => "No matching commands",
        }
    }

    fn detail(self, filter: &str) -> String {
        match self {
            Self::NoCommandsReady => {
                "Switch back to the app you want to control, then open Current App Commands again."
                    .to_string()
            }
            Self::NoFilteredMatches => {
                format!(
                    "Nothing matched \"{}\". Press Esc to clear the filter.",
                    filter.trim()
                )
            }
        }
    }
}

impl ScriptListApp {
    fn current_app_commands_filtered_entries<'a>(
        entries: &'a [builtins::BuiltInEntry],
        filter: &str,
    ) -> Vec<(usize, &'a builtins::BuiltInEntry)> {
        let (filtered, _) = builtins::filter_menu_bar_entries(entries, filter);
        filtered
    }

    fn current_app_commands_visible_row_names(&self, filter: &str) -> Vec<String> {
        Self::current_app_commands_filtered_entries(&self.cached_current_app_entries, filter)
            .into_iter()
            .map(|(_, entry)| entry.name.clone())
            .collect()
    }

    fn current_app_commands_dataset_and_visible_counts(&self, filter: &str) -> (usize, usize) {
        (
            self.cached_current_app_entries.len(),
            Self::current_app_commands_filtered_entries(&self.cached_current_app_entries, filter)
                .len(),
        )
    }

    fn current_app_commands_selected_visible_row_name(
        &self,
        filter: &str,
        selected_index: usize,
    ) -> Option<String> {
        Self::current_app_commands_filtered_entries(&self.cached_current_app_entries, filter)
            .get(selected_index)
            .map(|(_, entry)| entry.name.clone())
    }

    /// Render the current app commands view showing menu bar commands from the frontmost app.
    fn render_current_app_commands(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let _design_typography = tokens.typography();
        let color_resolver =
            crate::theme::ColorResolver::new_for_shell(&self.theme, self.current_design);
        let typography_resolver =
            crate::theme::TypographyResolver::new_theme_first(&self.theme, self.current_design);
        let empty_text_color = color_resolver.empty_text_color();
        let empty_font_family = typography_resolver.primary_font().to_string();

        let chrome = theme::AppChromeColors::from_theme(&self.theme);

        let filtered_entries =
            Self::current_app_commands_filtered_entries(&self.cached_current_app_entries, &filter);
        let filtered_len = filtered_entries.len();
        let filter_safe = crate::logging::log_user_value(&filter);
        tracing::info!(
            target: "script_kit::scroll_trace",
            event = "SCROLL_TRACE current_app.render_state",
            filter_preview = %filter_safe,
            filter_bytes = filter_safe.raw_bytes,
            filter_safe_bytes = filter_safe.safe_bytes,
            filter_truncated = filter_safe.truncated,
            selected_index,
            filtered_len,
            cached_entries = self.cached_current_app_entries.len(),
            wheel_owned_selected_index = ?self.builtin_wheel_owned_selected_index,
            "SCROLL_TRACE current_app.render_state"
        );

        // Key handler
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);

                if this.shortcut_recorder_state.is_some() {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let key_char = event.keystroke.key_char.as_deref();
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;

                match this.route_key_to_actions_dialog(
                    key,
                    key_char,
                    modifiers,
                    ActionsDialogHost::BuiltinList,
                    window,
                    cx,
                ) {
                    ActionsRoute::NotHandled => {}
                    ActionsRoute::Handled => {
                        tracing::debug!(
                            target: "script_kit::actions",
                            event = "builtin_view_actions_key_routed",
                            surface = "current_app_commands",
                            key = %key,
                        );
                        cx.stop_propagation();
                        return;
                    }
                    ActionsRoute::Execute {
                        action_id,
                        should_close,
                    } => {
                        this.execute_actions_route_action(
                            ActionsDialogHost::BuiltinList,
                            action_id,
                            should_close,
                            window,
                            cx,
                        );
                        cx.stop_propagation();
                        return;
                    }
                }

                // ESC: Clear filter first if present, otherwise go back/close
                if is_key_escape(key) {
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }

                // Cmd+W always closes window
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                // Extract current view state
                let view_state = if let AppView::CurrentAppCommandsView {
                    filter,
                    selected_index,
                } = &this.current_view
                {
                    Some((filter.clone(), *selected_index))
                } else {
                    None
                };

                let Some((current_filter, current_selected)) = view_state else {
                    return;
                };

                let filtered = Self::current_app_commands_filtered_entries(
                    &this.cached_current_app_entries,
                    &current_filter,
                );
                let current_filtered_len = filtered.len();

                if is_key_up(key) {
                    if current_selected > 0 {
                        if let AppView::CurrentAppCommandsView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected - 1;
                            this.current_app_commands_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_down(key) {
                    if current_selected < current_filtered_len.saturating_sub(1) {
                        if let AppView::CurrentAppCommandsView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected + 1;
                            this.current_app_commands_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_enter(key) {
                    // Execute selected menu bar action
                    if let Some((orig_idx, _)) = filtered.get(current_selected) {
                        this.execute_selected_current_app_command(*orig_idx, cx);
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        // Pre-compute colors
        let list_colors = ListItemColors::from_theme(&self.theme);

        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            let empty_state = CurrentAppCommandsEmptyState::from_filter(&filter);
            let empty_title = empty_state.title();
            let empty_detail = empty_state.detail(&filter);

            tracing::info!(
                filter = %filter,
                total_entries = self.cached_current_app_entries.len(),
                "current_app_commands.render_empty_state"
            );

            crate::list_item::EmptyState::new(empty_title, empty_text_color, &empty_font_family)
                .hint(empty_detail)
                .icon(crate::designs::icon_variations::IconName::Terminal)
                .into_element()
        } else {
            let entries_for_closure: Vec<(usize, builtins::BuiltInEntry)> = filtered_entries
                .iter()
                .map(|(i, e)| (*i, (*e).clone()))
                .collect();
            let selected = selected_index;
            let hovered = self.hovered_index;
            let click_entity_handle = cx.entity().downgrade();
            let hover_entity_handle = cx.entity().downgrade();
            uniform_list(
                "current-app-commands",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((orig_idx, entry)) = entries_for_closure.get(ix) {
                                let is_selected = ix == selected;
                                let is_hovered = hovered == Some(ix);

                                let name = entry.name.clone();
                                let description = entry.description.clone();
                                let original_entry_index = *orig_idx;

                                let click_entity = click_entity_handle.clone();
                                let click_handler =
                                    move |event: &gpui::ClickEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = click_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                let should_submit = if let AppView::CurrentAppCommandsView {
                                                    selected_index, ..
                                                } = &mut this.current_view {
                                                    let was_selected = *selected_index == ix;
                                                    *selected_index = ix;
                                                    crate::ui_foundation::should_submit_selected_row_click(
                                                        was_selected,
                                                        event.click_count(),
                                                    )
                                                } else {
                                                    false
                                                };
                                                if should_submit {
                                                    this.execute_selected_current_app_command(
                                                        original_entry_index,
                                                        cx,
                                                    );
                                                }
                                                cx.notify();
                                            });
                                        }
                                        cx.stop_propagation();
                                    };

                                let hover_entity = hover_entity_handle.clone();
                                let hover_handler =
                                    move |is_hovered: &bool,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = hover_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                if *is_hovered {
                                                    this.input_mode = InputMode::Mouse;
                                                    if this.hovered_index != Some(ix) {
                                                        this.hovered_index = Some(ix);
                                                        cx.notify();
                                                    }
                                                } else if this.hovered_index == Some(ix) {
                                                    this.hovered_index = None;
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    };

                                div()
                                    .id(ix)
                                    .cursor_pointer()
                                    .on_click(click_handler)
                                    .on_hover(hover_handler)
                                    .child(
                                        ListItem::new(name, list_colors)
                                            .description_opt(Some(description))
                                            .selected(is_selected)
                                            .hovered(is_hovered),
                                    )
                            } else {
                                div().id(ix).h(px(LIST_ITEM_HEIGHT))
                            }
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.current_app_commands_scroll_handle)
            .into_any_element()
        };

        let total_count = self.cached_current_app_entries.len();

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            .py(px(design_spacing.padding_xs))
            .relative()
            .on_scroll_wheel(cx.listener(
                move |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                    let view_state = if let AppView::CurrentAppCommandsView {
                        filter,
                        selected_index,
                    } = &this.current_view
                    {
                        Some((filter.clone(), *selected_index))
                    } else {
                        None
                    };

                    let Some((current_filter, current_selected)) = view_state else {
                        return;
                    };

                    let filtered = Self::current_app_commands_filtered_entries(
                        &this.cached_current_app_entries,
                        &current_filter,
                    );
                    let filtered_len = filtered.len();
                    let current_filter_safe = crate::logging::log_user_value(&current_filter);
                    tracing::info!(
                        target: "script_kit::scroll_trace",
                        event = "SCROLL_TRACE current_app.wheel_event",
                        current_selected,
                        filtered_len,
                        filter_preview = %current_filter_safe,
                        filter_bytes = current_filter_safe.raw_bytes,
                        filter_safe_bytes = current_filter_safe.safe_bytes,
                        filter_truncated = current_filter_safe.truncated,
                        "SCROLL_TRACE current_app.wheel_event"
                    );

                    let Some(new_selected) = this.builtin_scroll_target_from_wheel(
                        event,
                        current_selected,
                        filtered_len,
                    ) else {
                        tracing::info!(
                            target: "script_kit::scroll_trace",
                            event = "SCROLL_TRACE current_app.wheel_no_target",
                            current_selected,
                            filtered_len,
                            filter_preview = %current_filter_safe,
                            "SCROLL_TRACE current_app.wheel_no_target"
                        );
                        if filtered_len > 0 {
                            cx.stop_propagation();
                        }
                        return;
                    };

                    if let AppView::CurrentAppCommandsView { selected_index, .. } =
                        &mut this.current_view
                    {
                        *selected_index = new_selected;
                    }

                    this.current_app_commands_scroll_handle
                        .scroll_to_item(new_selected, ScrollStrategy::Nearest);
                    this.note_builtin_selection_owned_wheel_scroll(new_selected);
                    tracing::info!(
                        target: "script_kit::scroll_trace",
                        event = "SCROLL_TRACE current_app.wheel_selected",
                        selected_before = current_selected,
                        selected_after = new_selected,
                        filtered_len,
                        filter_preview = %current_filter_safe,
                        "SCROLL_TRACE current_app.wheel_selected"
                    );

                    Self::log_builtin_scroll_event(
                        "current_app_commands",
                        "scroll_to_item",
                        "wheel",
                        filtered_len,
                        Some(new_selected),
                        Some(new_selected),
                        Some(&current_filter),
                        "mouse",
                    );
                    cx.notify();
                    cx.stop_propagation();
                },
            ))
            .child(
                // Every list leads with a persistent section separator
                // (POLISH.md layout-stability bar; same rule as the main
                // menu's "Results" header, 4d76327b8): the label may swap but
                // the row never appears or disappears, so filtering can't
                // shift the rows below it.
                crate::list_item::render_section_header(
                    if filter.trim().is_empty() {
                        "Commands"
                    } else {
                        "Results"
                    },
                    None,
                    list_colors,
                    true,
                ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .child(list_element)
                    .child(self.builtin_uniform_list_scrollbar(
                        &self.current_app_commands_scroll_handle,
                        filtered_len,
                        8,
                    )),
            );

        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            vec![
                gpui::SharedString::from("↵ Run"),
                gpui::SharedString::from("Esc Back"),
            ],
            None,
        ));

        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(chrome.text_primary_hex))
                .font_family(self.theme_font_family())
                .key_context("current_app_commands")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(vec![
                    self.render_builtin_main_input_count_label(format!(
                        "{} command{}",
                        total_count,
                        if total_count == 1 { "" } else { "s" }
                    )),
                ], cx),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main: content.into_any_element(),
                footer,
                overlays: Vec::new(),
            },
        )
    }
}

#[cfg(test)]
mod current_app_commands_chrome_audit {
    fn production_source() -> &'static str {
        include_str!("current_app_commands.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source should exist")
    }

    #[test]
    fn current_app_commands_uses_minimal_chrome_footer() {
        let source = production_source();
        assert!(
            source.contains("render_simple_hint_strip(")
                && source.contains("main_window_footer_slot("),
            "current_app_commands should route minimal hint chrome through the shared footer slot"
        );
        let legacy = "Prompt".to_owned() + "Footer::new(";
        assert_eq!(
            source.matches(&legacy).count(),
            0,
            "current_app_commands should not use PromptFooter"
        );
    }

    #[test]
    fn current_app_commands_use_wheel_contract_and_vendor_scrollbar() {
        let source = production_source();
        assert!(
            source.contains(".on_scroll_wheel(cx.listener("),
            "current_app_commands should intercept wheel scrolling on the list pane"
        );
        assert!(
            source.contains("builtin_scroll_target_from_wheel("),
            "current_app_commands should use shared wheel delta conversion"
        );
        assert!(
            !source.contains("builtin_reanchor_selection_from_scroll("),
            "current_app_commands should keep selection owned by keyboard/wheel/click, matching the main menu"
        );
        assert!(
            source.contains("builtin_uniform_list_scrollbar("),
            "current_app_commands should attach the shared vendor scrollbar helper"
        );
    }
}
