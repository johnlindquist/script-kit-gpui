#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppLauncherActivationAction {
    LaunchSelectedApp,
}

impl AppLauncherActivationAction {
    fn launch_log(self, app_name: &str) -> String {
        match self {
            Self::LaunchSelectedApp => format!("Launching app: {app_name}"),
        }
    }

    fn double_click_launch_log(self, app_name: &str) -> String {
        match self {
            Self::LaunchSelectedApp => format!("Double-click launching app: {app_name}"),
        }
    }

    fn success_log(self, app_name: &str) -> String {
        match self {
            Self::LaunchSelectedApp => format!("Launched: {app_name}"),
        }
    }

    fn failure_log(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::LaunchSelectedApp => format!("Failed to launch app: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppLauncherEmptyState {
    NoApplicationsFound,
    NoFilteredMatches,
}

impl AppLauncherEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoApplicationsFound
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoApplicationsFound => "No applications found",
            Self::NoFilteredMatches => "No apps match your filter",
        }
    }
}

impl ScriptListApp {
    fn app_launcher_filtered_entries<'a>(
        apps: &'a [app_launcher::AppInfo],
        filter: &str,
    ) -> Vec<(usize, &'a app_launcher::AppInfo)> {
        if filter.is_empty() {
            apps.iter().enumerate().collect()
        } else {
            let filter_lower = filter.to_lowercase();
            apps.iter()
                .enumerate()
                .filter(|(_, app)| app.name.to_lowercase().contains(&filter_lower))
                .collect()
        }
    }

    fn app_launcher_visible_row_names(&self, filter: &str) -> Vec<String> {
        Self::app_launcher_filtered_entries(&self.apps, filter)
            .into_iter()
            .map(|(_, app)| app.name.clone())
            .collect()
    }

    fn app_launcher_dataset_and_visible_counts(&self, filter: &str) -> (usize, usize) {
        (
            self.apps.len(),
            Self::app_launcher_filtered_entries(&self.apps, filter).len(),
        )
    }

    fn app_launcher_selected_visible_entry(
        &self,
        filter: &str,
        selected_index: usize,
    ) -> Option<(usize, &app_launcher::AppInfo)> {
        Self::app_launcher_filtered_entries(&self.apps, filter)
            .get(selected_index)
            .copied()
    }

    fn app_launcher_visible_target_rows(
        &self,
        filter: &str,
        limit: usize,
    ) -> Vec<(usize, usize, &app_launcher::AppInfo)> {
        Self::app_launcher_filtered_entries(&self.apps, filter)
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(display_index, (source_index, app))| (display_index, source_index, app))
            .collect()
    }

    /// Render app launcher view
    /// P0 FIX: Data comes from self.apps, view passes only state
    fn render_app_launcher(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for spacing/typography/visual, theme for colors
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let _design_typography = tokens.typography();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = self.theme.colors.background.main;
        let _bg_with_alpha = crate::ui_foundation::hex_to_rgba_with_opacity(bg_hex, opacity.main);
        // Removed: box_shadows - shadows on transparent elements block vibrancy
        let _box_shadows = self.create_box_shadows();

        let filtered_apps = Self::app_launcher_filtered_entries(&self.apps, &filter);
        let filtered_len = filtered_apps.len();

        // Key handler for app launcher
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

                logging::log("KEY", &format!("AppLauncher key: '{}'", key));

                // P0 FIX: View state only - data comes from this.apps
                if let AppView::AppLauncherView {
                    filter,
                    selected_index,
                } = &mut this.current_view
                {
                    let filtered_apps = Self::app_launcher_filtered_entries(&this.apps, filter);
                    let filtered_len = filtered_apps.len();

                    match key {
                        _ if is_key_up(key) => {
                            *selected_index = selected_index.saturating_sub(1);
                            this.list_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            cx.notify();
                        }
                        _ if is_key_down(key) => {
                            *selected_index =
                                (*selected_index + 1).min(filtered_len.saturating_sub(1));
                            this.list_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            cx.notify();
                        }
                        _ if is_key_enter(key) => {
                            // Launch selected app and hide window
                            if let Some((_, app)) = filtered_apps.get(*selected_index) {
                                let activation_action =
                                    AppLauncherActivationAction::LaunchSelectedApp;
                                logging::log("EXEC", &activation_action.launch_log(&app.name));
                                if let Err(e) = app_launcher::launch_application(app) {
                                    logging::log("ERROR", &activation_action.failure_log(e));
                                } else {
                                    logging::log("EXEC", &activation_action.success_log(&app.name));
                                    this.hide_main_and_reset(cx);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            },
        );

        let color_resolver =
            crate::theme::ColorResolver::new_for_shell(&self.theme, self.current_design);
        let typography_resolver =
            crate::theme::TypographyResolver::new_theme_first(&self.theme, self.current_design);
        let empty_text_color = color_resolver.empty_text_color();
        let empty_font_family = typography_resolver.primary_font().to_string();

        // Pre-compute colors
        let list_colors = ListItemColors::from_theme(&self.theme);
        let text_primary = self.theme.colors.text.primary;
        let main_menu_theme = self.current_main_menu_theme;

        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            let state = AppLauncherEmptyState::from_filter(&filter);
            crate::list_item::EmptyState::new(state.message(), empty_text_color, &empty_font_family)
                .icon(crate::designs::icon_variations::IconName::MagnifyingGlass)
                .into_element()
        } else {
            // Clone data for the closure
            let apps_for_closure: Vec<_> = filtered_apps
                .iter()
                .map(|(i, a)| (*i, (*a).clone()))
                .collect();
            let selected = selected_index;
            let hovered = self.hovered_index;
            let click_entity_handle = cx.entity().downgrade();
            let hover_entity_handle = cx.entity().downgrade();

            uniform_list(
                "app-launcher",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((_, app)) = apps_for_closure.get(ix) {
                                let is_selected = ix == selected;
                                let is_hovered = hovered == Some(ix);
                                let row_id = app
                                    .bundle_id
                                    .clone()
                                    .unwrap_or_else(|| app.path.to_string_lossy().into_owned());

                                // Format app path for description
                                let path_str = app.path.to_string_lossy();
                                let description = if path_str.starts_with("/Applications") {
                                    None // No need to show path for standard apps
                                } else {
                                    Some(path_str.to_string())
                                };

                                // Use pre-decoded icon if available, fallback to emoji
                                let icon = match &app.icon {
                                    Some(img) => list_item::IconKind::Image(img.clone()),
                                    None => list_item::IconKind::Emoji("📱".to_string()),
                                };

                                // Click handler: select on click, launch on double-click
                                let click_entity = click_entity_handle.clone();
                                let app_info = app.clone();
                                let activation_action =
                                    AppLauncherActivationAction::LaunchSelectedApp;
                                let click_handler =
                                    move |event: &gpui::ClickEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app_entity) = click_entity.upgrade() {
                                            let app_info = app_info.clone();
                                            app_entity.update(cx, |this, cx| {
                                                if let AppView::AppLauncherView {
                                                    selected_index,
                                                    ..
                                                } = &mut this.current_view
                                                {
                                                    *selected_index = ix;
                                                }
                                                this.list_scroll_handle
                                                    .scroll_to_item(ix, ScrollStrategy::Nearest);
                                                this.note_list_pointer_click(ix, cx);

                                                // Double-click: launch app
                                                if let gpui::ClickEvent::Mouse(mouse_event) = event
                                                {
                                                    if mouse_event.down.click_count == 2 {
                                                        logging::log(
                                                            "UI",
                                                            &activation_action
                                                                .double_click_launch_log(
                                                                    &app_info.name,
                                                                ),
                                                        );
                                                        if app_launcher::launch_application(
                                                            &app_info,
                                                        )
                                                        .is_ok()
                                                        {
                                                            this.hide_main_and_reset(cx);
                                                        }
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
                                    .id(row_id)
                                    .cursor_pointer()
                                    .when(
                                        crate::list_item::LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED,
                                        |row| {
                                            row.tooltip(|window, cx| {
                                                gpui_component::tooltip::Tooltip::new(
                                                    "Launch selected app",
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
                                        ListItem::new(app.name.clone(), list_colors)
                                            .icon_kind(icon)
                                            .description_opt(description)
                                            .selected(is_selected)
                                            .hovered(is_hovered)
                                            .main_menu_theme(main_menu_theme),
                                    )
                            } else {
                                div().id(ix).h(px(LIST_ITEM_HEIGHT))
                            }
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.list_scroll_handle)
            .into_any_element()
        };
        let list_scrollbar =
            self.builtin_uniform_list_scrollbar(&self.list_scroll_handle, filtered_len, 8);

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .py(px(design_spacing.padding_xs))
            .child(
                // Every list leads with a persistent section separator
                // (POLISH.md layout-stability bar; same rule as the main
                // menu's "Results" header, 4d76327b8): the label may swap but
                // the row never appears or disappears, so filtering can't
                // shift the rows below it.
                crate::components::builtin_leading_separator::render_builtin_leading_separator(
                    if filter.trim().is_empty() {
                        "Apps"
                    } else {
                        "Results"
                    },
                    None,
                    list_colors,
                ),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .flex_1()
                    .min_h(px(0.))
                    .on_scroll_wheel(cx.listener(
                        move |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                            this.observe_builtin_native_list_scroll(event, cx);
                        },
                    ))
                    .child(list_element)
                    .child(list_scrollbar),
            );

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list(
                "render_builtins::app_launcher",
                true,
            ),
        );

        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            vec![
                gpui::SharedString::from("↵ Launch"),
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
                .key_context("app_launcher")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(Vec::new(), cx),
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
mod app_launcher_chrome_audit {
    #[test]
    fn app_launcher_uses_minimal_shell_with_keyboard_hooks() {
        let source = include_str!("app_launcher.rs");
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];

        assert!(
            render_code.contains("render_main_view_chrome_footer_flush(")
                && render_code.contains("render_builtin_main_input_header("),
            "app_launcher should return the shared main-view chrome with built-in input header"
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
        let source = include_str!("app_launcher.rs");
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];

        let legacy = "Prompt".to_owned() + "Footer::new(";
        assert!(
            !render_code.contains(&legacy),
            "app_launcher should not construct PromptFooter after migration"
        );
        assert!(
            !render_code.contains("\u{1f680} Apps"),
            "app_launcher should not keep a redundant launcher title row"
        );
    }
}

#[cfg(test)]
mod app_launcher_chrome_tests {
    fn read_source() -> String {
        include_str!("app_launcher.rs").to_string()
    }

    #[test]
    fn app_launcher_declares_runtime_chrome_audit() {
        let source = read_source();
        assert!(
            source.contains("emit_prompt_chrome_audit(")
                && source.contains("PromptChromeAudit::minimal_list(")
                && source.contains("\"render_builtins::app_launcher\""),
            "app launcher should emit a minimal-list runtime audit"
        );
    }
}
