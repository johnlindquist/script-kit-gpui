#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowSwitcherFocusAction {
    FocusSelectedWindow,
}

impl WindowSwitcherFocusAction {
    fn attempt_log(self, window_title: &str) -> String {
        match self {
            Self::FocusSelectedWindow => format!("Focusing window: {window_title}"),
        }
    }

    fn success_log(self, window_title: &str) -> String {
        match self {
            Self::FocusSelectedWindow => format!("Focused window: {window_title}"),
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::FocusSelectedWindow => format!("Failed to focus window: {error}"),
        }
    }
}

/// What the UI does after an async focus attempt completes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum WindowSwitcherFocusCompletion {
    /// Focus verified: hide Script Kit and reset.
    HideMainWindow,
    /// Focus failed: keep the switcher open and show the error toast.
    ShowErrorToast(String),
}

/// Pure reducer: focus result -> UI completion. The main window is hidden
/// ONLY from a successful completion — never before the result is known.
fn reduce_window_switcher_focus_result(
    action: WindowSwitcherFocusAction,
    result: Result<(), String>,
) -> WindowSwitcherFocusCompletion {
    match result {
        Ok(()) => WindowSwitcherFocusCompletion::HideMainWindow,
        Err(error) => WindowSwitcherFocusCompletion::ShowErrorToast(action.failure_message(error)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowSwitcherEmptyState {
    NoWindowsFound,
    NoFilteredMatches,
}

impl WindowSwitcherEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoWindowsFound
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoWindowsFound => "No windows found",
            Self::NoFilteredMatches => "No windows match your filter",
        }
    }
}

impl ScriptListApp {
    /// Render window switcher view with 50/50 split layout
    /// P0 FIX: Data comes from self.cached_windows, view passes only state
    fn render_window_switcher(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_window_switcher_inner(filter, selected_index, cx)
    }

    /// Execute the switcher's focus action OFF the GPUI thread.
    ///
    /// The main window hides ONLY from the successful completion callback;
    /// a failed focus keeps the switcher open with an error toast. Does
    /// nothing when the entity no longer exists.
    fn focus_window_switcher_target_async(
        &mut self,
        window_id: u32,
        window_title: String,
        cx: &mut Context<Self>,
    ) {
        let focus_action = WindowSwitcherFocusAction::FocusSelectedWindow;
        logging::log("EXEC", &focus_action.attempt_log(&window_title));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    window_control::focus_window(window_id).map_err(|error| error.to_string())
                })
                .await;
            cx.update(|cx| {
                let Some(entity) = this.upgrade() else {
                    return;
                };
                entity.update(cx, |this, cx| {
                    match reduce_window_switcher_focus_result(focus_action, result) {
                        WindowSwitcherFocusCompletion::HideMainWindow => {
                            logging::log("EXEC", &focus_action.success_log(&window_title));
                            this.hide_main_and_reset(cx);
                        }
                        WindowSwitcherFocusCompletion::ShowErrorToast(failure_message) => {
                            logging::log("ERROR", &failure_message);
                            this.toast_manager.push(
                                components::toast::Toast::error(failure_message, &this.theme)
                                    .duration_ms(Some(TOAST_ERROR_MS)),
                            );
                            cx.notify();
                        }
                    }
                });
            });
        })
        .detach();
    }

    fn render_window_switcher_inner(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = self.theme.colors.background.main;
        let _bg_with_alpha = crate::ui_foundation::hex_to_rgba_with_opacity(bg_hex, opacity.main);
        // Removed: box_shadows - shadows on transparent elements block vibrancy
        let _box_shadows = self.create_box_shadows();

        // P0 FIX: Filter windows from self.cached_windows instead of taking ownership
        let filtered_windows: Vec<_> = if filter.is_empty() {
            self.cached_windows.iter().enumerate().collect()
        } else {
            let filter_lower = filter.to_lowercase();
            self.cached_windows
                .iter()
                .enumerate()
                .filter(|(_, w)| {
                    w.title.to_lowercase().contains(&filter_lower)
                        || w.app.to_lowercase().contains(&filter_lower)
                })
                .collect()
        };
        let filtered_len = filtered_windows.len();

        // Key handler for window switcher
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                // Hide cursor while typing - automatically shows when mouse moves
                this.hide_mouse_cursor(cx);

                // If the shortcut recorder is active, don't process any key events.
                // The recorder has its own key handlers and should receive all key events.
                if this.shortcut_recorder_state.is_some() {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;

                // ESC: Clear filter first if present, otherwise go back/close
                if is_key_escape(key) && !this.show_actions_popup {
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    return;
                }

                // Cmd+W always closes window
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    logging::log("KEY", "Cmd+W - closing window");
                    this.close_and_reset_window(cx);
                    return;
                }

                logging::log("KEY", &format!("WindowSwitcher key: '{}'", key));

                // P0 FIX: View state only - data comes from this.cached_windows
                if let AppView::WindowSwitcherView {
                    filter,
                    selected_index,
                } = &mut this.current_view
                {
                    // Apply filter to get current filtered list
                    // P0 FIX: Reference cached_windows from self
                    let filtered_windows: Vec<_> = if filter.is_empty() {
                        this.cached_windows.iter().enumerate().collect()
                    } else {
                        let filter_lower = filter.to_lowercase();
                        this.cached_windows
                            .iter()
                            .enumerate()
                            .filter(|(_, w)| {
                                w.title.to_lowercase().contains(&filter_lower)
                                    || w.app.to_lowercase().contains(&filter_lower)
                            })
                            .collect()
                    };
                    let filtered_len = filtered_windows.len();

                    match key {
                        _ if is_key_up(key) => {
                            *selected_index = selected_index.saturating_sub(1);
                            this.window_list_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            cx.notify();
                        }
                        _ if is_key_down(key) => {
                            *selected_index =
                                (*selected_index + 1).min(filtered_len.saturating_sub(1));
                            this.window_list_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            cx.notify();
                        }
                        _ if is_key_enter(key) => {
                            // Focus selected window (async, off the GPUI
                            // thread); Script Kit hides only after verified
                            // focus success.
                            if let Some((_, window_info)) = filtered_windows.get(*selected_index) {
                                let window_id = window_info.id;
                                let window_title = window_info.title.clone();
                                this.focus_window_switcher_target_async(
                                    window_id,
                                    window_title,
                                    cx,
                                );
                            }
                        }
                        // Note: "escape" is handled by handle_global_shortcut_with_options above
                        // Text input (backspace, characters) is handled by the shared Input component
                        // which syncs via handle_filter_input_change()
                        _ => {}
                    }
                }
            },
        );

        // Pre-compute colors
        let list_colors = ListItemColors::from_theme(&self.theme);
        let text_primary = self.theme.colors.text.primary;
        #[allow(unused_variables)]
        let text_muted = self.theme.colors.text.muted;

        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            let state = WindowSwitcherEmptyState::from_filter(&filter);
            crate::components::render_simple_empty_state(
                "window-switcher-empty",
                state.message(),
                "panel-left",
                None,
                &self.theme,
                cx,
            )
        } else {
            // Clone data for the closure
            let windows_for_closure: Vec<_> = filtered_windows
                .iter()
                .map(|(i, w)| (*i, (*w).clone()))
                .collect();
            let selected = selected_index;
            let hovered = self.hovered_index;
            let click_entity_handle = cx.entity().downgrade();
            let hover_entity_handle = cx.entity().downgrade();

            uniform_list(
                "window-switcher",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((_, window_info)) = windows_for_closure.get(ix) {
                                let is_selected = ix == selected;
                                let is_hovered = hovered == Some(ix);

                                // Format: "AppName: Window Title"
                                let name = format!("{}: {}", window_info.app, window_info.title);

                                // Format bounds as description
                                let description = format!(
                                    "{}×{} at ({}, {})",
                                    window_info.bounds.width,
                                    window_info.bounds.height,
                                    window_info.bounds.x,
                                    window_info.bounds.y
                                );

                                // Click handler: select on click, focus window on double-click
                                let click_entity = click_entity_handle.clone();
                                let win_id = window_info.id;
                                let win_title = window_info.title.clone();
                                let click_handler =
                                    move |event: &gpui::ClickEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = click_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                if let AppView::WindowSwitcherView {
                                                    selected_index,
                                                    ..
                                                } = &mut this.current_view
                                                {
                                                    *selected_index = ix;
                                                }
                                                this.window_list_scroll_handle
                                                    .scroll_to_item(ix, ScrollStrategy::Nearest);
                                                this.note_list_pointer_click(ix, cx);

                                                // Double-click: focus window
                                                // through the same async
                                                // helper as Enter.
                                                if let gpui::ClickEvent::Mouse(mouse_event) = event
                                                {
                                                    if mouse_event.down.click_count == 2 {
                                                        logging::log(
                                                            "UI",
                                                            &format!(
                                                                "Double-click focusing window {}",
                                                                win_id
                                                            ),
                                                        );
                                                        this.focus_window_switcher_target_async(
                                                            win_id,
                                                            win_title.clone(),
                                                            cx,
                                                        );
                                                    }
                                                }
                                            });
                                        }
                                    };

                                // Hover handler for mouse tracking
                                let hover_entity = hover_entity_handle.clone();
                                let move_entity = hover_entity_handle.clone();
                                let move_handler =
                                    move |_event: &gpui::MouseMoveEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = move_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                this.note_list_pointer_move(ix, cx);
                                            });
                                        }
                                    };
                                let hover_handler =
                                    move |is_hovered: &bool,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = hover_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                if !*is_hovered {
                                                    this.note_list_pointer_leave(ix, cx);
                                                }
                                            });
                                        }
                                    };

                                div()
                                    .id(("window-switcher-row", win_id))
                                    .cursor_pointer()
                                    .when(
                                        crate::list_item::LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED,
                                        |row| {
                                            row.tooltip(|window, cx| {
                                                gpui_component::tooltip::Tooltip::new(
                                                    "Switch to selected window",
                                                )
                                                .key_binding(
                                                    gpui::Keystroke::parse("enter")
                                                        .ok()
                                                        .map(gpui_component::kbd::Kbd::new),
                                                )
                                                .build(window, cx)
                                            })
                                        },
                                    )
                                    .on_click(click_handler)
                                    .on_mouse_move(move_handler)
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
            .track_scroll(&self.window_list_scroll_handle)
            .into_any_element()
        };

        // Build actions panel for selected window
        let selected_window = filtered_windows
            .get(selected_index)
            .map(|(_, w)| (*w).clone());
        let actions_panel = self.render_window_actions_panel(
            &selected_window,
            &design_colors,
            &design_spacing,
            &design_typography,
            &design_visual,
            cx,
        );

        // Main content area - 50/50 split: Window list on left, Actions on right
        let content = div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .h_full()
            // Left side: Window list (50% width)
            .child(
                div()
                    .relative()
                    .w_1_2()
                    .h_full()
                    .min_h(px(0.))
                    .py(px(design_spacing.padding_xs))
                    .on_scroll_wheel(cx.listener(
                        move |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                            this.observe_builtin_native_list_scroll(event, cx);
                        },
                    ))
                    .child(list_element)
                    .child(self.builtin_uniform_list_scrollbar(
                        &self.window_list_scroll_handle,
                        filtered_len,
                        8,
                    )),
            )
            // Right side: Actions panel (50% width)
            .child(
                div()
                    .w_1_2()
                    .h_full()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .child(actions_panel),
            );

        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            vec![
                gpui::SharedString::from("↵ Switch"),
                gpui::SharedString::from("Esc Back"),
            ],
            None,
        ));

        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(self.theme_font_family())
                .key_context("window_switcher")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(
                    vec![self.render_builtin_main_input_count_label(format!(
                        "{} windows",
                        self.cached_windows.len()
                    ))],
                    cx,
                ),
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
mod window_switcher_chrome_audit {
    #[test]
    fn window_switcher_uses_minimal_chrome_footer() {
        let source = include_str!("window_switcher.rs");
        assert!(
            source.contains("render_main_view_chrome_footer_flush(")
                && source.contains("render_builtin_main_input_header("),
            "window_switcher should use shared main-view chrome and built-in input header"
        );
        let legacy = "Prompt".to_owned() + "Footer::new(";
        assert_eq!(
            source.matches(&legacy).count(),
            0,
            "window_switcher should not use PromptFooter"
        );
    }
}

#[cfg(test)]
mod focus_reducer_tests {
    // NOTE: no `use super::*` here — this file is include!()d beneath
    // `use gpui::*`, whose `test` macro would shadow `#[test]` and silently
    // unregister these tests (see gpui-test-macro-shadowing).

    #[test]
    fn successful_focus_hides_the_main_window() {
        let completion = super::reduce_window_switcher_focus_result(
            super::WindowSwitcherFocusAction::FocusSelectedWindow,
            Ok(()),
        );
        assert_eq!(
            completion,
            super::WindowSwitcherFocusCompletion::HideMainWindow
        );
    }

    #[test]
    fn failed_focus_keeps_the_switcher_open_with_an_error_toast() {
        let completion = super::reduce_window_switcher_focus_result(
            super::WindowSwitcherFocusAction::FocusSelectedWindow,
            Err("window_engine:Failed:gone".to_string()),
        );
        let super::WindowSwitcherFocusCompletion::ShowErrorToast(message) = completion else {
            panic!("failed focus must show a toast, never hide the window");
        };
        assert!(message.contains("Failed to focus window"));
        assert!(message.contains("window_engine:Failed:gone"));
    }
}
