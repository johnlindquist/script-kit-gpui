#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserTabsActivationAction {
    ActivateSelectedTab,
}

impl BrowserTabsActivationAction {
    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::ActivateSelectedTab => format!("Failed to activate tab: {error}"),
        }
    }

    fn generic_failure_message(self) -> &'static str {
        match self {
            Self::ActivateSelectedTab => "Failed to activate tab",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserTabsEmptyState {
    NoOpenTabs,
    NoFilteredMatches,
}

impl BrowserTabsEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoOpenTabs
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoOpenTabs => "No open browser tabs",
            Self::NoFilteredMatches => "No browser tabs match your filter",
        }
    }
}

impl ScriptListApp {
    fn browser_tabs_visible_rows(&self, filter: &str) -> Vec<crate::browser_tabs::BrowserTabInfo> {
        crate::browser_tabs::fuzzy_search_browser_tabs(&self.cached_browser_tabs, filter)
            .into_iter()
            .map(|entry| entry.tab)
            .collect()
    }

    fn browser_tabs_selected_visible_row(
        &self,
        filter: &str,
        selected_index: usize,
    ) -> Option<crate::browser_tabs::BrowserTabInfo> {
        self.browser_tabs_visible_rows(filter)
            .get(selected_index)
            .cloned()
    }

    fn browser_tabs_dataset_and_visible_counts(&self, filter: &str) -> (usize, usize) {
        (
            self.cached_browser_tabs.len(),
            self.browser_tabs_visible_rows(filter).len(),
        )
    }

    fn browser_tabs_visible_row_labels(&self, filter: &str) -> Vec<String> {
        self.browser_tabs_visible_rows(filter)
            .into_iter()
            .map(|tab| tab.display_title().to_string())
            .collect()
    }

    fn browser_tab_attachment_part(
        index: usize,
        tab: &crate::browser_tabs::BrowserTabInfo,
    ) -> crate::ai::message_parts::AiContextPart {
        let title = tab.display_title().to_string();
        let stable_key = crate::browser_tabs::browser_tab_stable_key(tab);
        let host = crate::browser_tabs::browser_tab_host(tab);
        let target = crate::ai::TabAiTargetContext {
            source: "BrowserTabs".to_string(),
            kind: "browser_tab".to_string(),
            semantic_id: crate::protocol::generate_semantic_id("browser-tab", index, &stable_key),
            label: title.clone(),
            metadata: Some(serde_json::json!({
                "browserName": tab.browser_name,
                "browserBundleId": tab.browser_bundle_id,
                "windowIndex": tab.window_index,
                "tabIndex": tab.tab_index,
                "title": tab.title,
                "url": tab.url,
                "host": host,
                "stableKey": stable_key,
            })),
        };
        let label = crate::ai::format_explicit_target_chip_label(&target);
        crate::ai::message_parts::AiContextPart::FocusedTarget { target, label }
    }

    fn browser_tabs_count_label(total_count: usize) -> String {
        let suffix = if total_count == 1 { "" } else { "s" };
        format!("{} tab{}", total_count, suffix)
    }

    fn render_browser_tabs(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list(
                "render_builtins::browser_tabs",
                true,
            ),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();

        let text_primary = self.theme.colors.text.primary;
        let _text_muted = self.theme.colors.text.muted;

        let filtered_tabs =
            crate::browser_tabs::fuzzy_search_browser_tabs(&self.cached_browser_tabs, &filter);
        let filtered_len = filtered_tabs.len();
        let total_count = self.cached_browser_tabs.len();

        // Key handler — only navigation keys; character input flows through the shared GPUI Input.
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
                let has_cmd = event.keystroke.modifiers.platform;

                if is_key_escape(key) && !this.show_actions_popup {
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }

                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                let is_attachment_portal = this.is_in_attachment_portal();

                if let AppView::BrowserTabsView {
                    filter,
                    selected_index,
                } = &mut this.current_view
                {
                    let filtered_tabs = crate::browser_tabs::fuzzy_search_browser_tabs(
                        &this.cached_browser_tabs,
                        filter,
                    );
                    let filtered_len = filtered_tabs.len();

                    if is_key_up(key) {
                        *selected_index = selected_index.saturating_sub(1);
                        this.browser_tabs_scroll_handle
                            .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                        cx.notify();
                        cx.stop_propagation();
                    } else if is_key_down(key) {
                        *selected_index = (*selected_index + 1).min(filtered_len.saturating_sub(1));
                        this.browser_tabs_scroll_handle
                            .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                        cx.notify();
                        cx.stop_propagation();
                    } else if is_key_enter(key) {
                        if let Some(tab) = filtered_tabs.get(*selected_index).map(|m| m.tab.clone())
                        {
                            if is_attachment_portal {
                                let part = Self::browser_tab_attachment_part(*selected_index, &tab);
                                this.close_attachment_portal_with_part(part, cx);
                            } else {
                                let activation_action =
                                    BrowserTabsActivationAction::ActivateSelectedTab;
                                match crate::browser_tabs::activate_tab(&tab) {
                                    Ok(()) => this.hide_main_and_reset(cx),
                                    Err(error) => {
                                        this.toast_manager.push(
                                            components::toast::Toast::error(
                                                activation_action.failure_message(error),
                                                &this.theme,
                                            )
                                            .duration_ms(Some(TOAST_ERROR_MS)),
                                        );
                                        cx.notify();
                                    }
                                }
                            }
                        }
                        cx.stop_propagation();
                    }
                }
            },
        );

        let list_colors = ListItemColors::from_theme(&self.theme);
        let list_element: AnyElement = if filtered_len == 0 {
            let state = BrowserTabsEmptyState::from_filter(&filter);
            crate::components::render_simple_empty_state(
                "browser-tabs-empty",
                state.message(),
                "search",
                None,
                &self.theme,
                cx,
            )
        } else {
            let app_icons: std::collections::HashMap<String, crate::app_launcher::DecodedIcon> =
                self.apps
                    .iter()
                    .filter_map(|app| {
                        app.bundle_id
                            .clone()
                            .and_then(|bundle_id| app.icon.clone().map(|icon| (bundle_id, icon)))
                    })
                    .collect();
            let tabs_for_closure: Vec<(crate::browser_tabs::BrowserTabInfo, list_item::IconKind)> =
                filtered_tabs
                    .iter()
                    .map(|tab_match| {
                        let tab = tab_match.tab.clone();
                        let icon = browser_tab_icon_for_render(&tab, &app_icons);
                        (tab, icon)
                    })
                    .collect();
            let selected = selected_index;
            let hovered = self.hovered_index;
            let click_entity_handle = cx.entity().downgrade();
            let hover_entity_handle = cx.entity().downgrade();

            uniform_list(
                "browser-tabs",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((tab, icon)) = tabs_for_closure.get(ix) {
                                let tab = tab.clone();
                                let is_selected = ix == selected;
                                let is_hovered = hovered == Some(ix);

                                let description = if !tab.title.trim().is_empty() {
                                    Some(tab.url.to_string())
                                } else if !tab.browser_name.is_empty() {
                                    Some(tab.browser_name.to_string())
                                } else {
                                    None
                                };
                                let display_title = tab.display_title().to_string();
                                let row_id = crate::browser_tabs::browser_tab_stable_key(&tab);

                                let click_entity = click_entity_handle.clone();
                                let hover_entity = hover_entity_handle.clone();
                                let move_entity = hover_entity_handle.clone();
                                let tab_for_click = tab.clone();
                                let activation_action =
                                    BrowserTabsActivationAction::ActivateSelectedTab;
                                let click_handler =
                                    move |event: &gpui::ClickEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app_entity) = click_entity.upgrade() {
                                            let tab = tab_for_click.clone();
                                            app_entity.update(cx, |this, cx| {
                                                if let AppView::BrowserTabsView {
                                                    selected_index,
                                                    ..
                                                } = &mut this.current_view
                                                {
                                                    *selected_index = ix;
                                                }
                                                this.browser_tabs_scroll_handle
                                                    .scroll_to_item(ix, ScrollStrategy::Nearest);
                                                this.note_list_pointer_click(ix, cx);

                                                if let gpui::ClickEvent::Mouse(mouse_event) = event
                                                {
                                                    if mouse_event.down.click_count == 2 {
                                                        if this.is_in_attachment_portal() {
                                                            let part =
                                                                Self::browser_tab_attachment_part(
                                                                    ix, &tab,
                                                                );
                                                            this.close_attachment_portal_with_part(
                                                                part, cx,
                                                            );
                                                        } else {
                                                            match crate::browser_tabs::activate_tab(
                                                                &tab,
                                                            ) {
                                                                Ok(()) => {
                                                                    this.hide_main_and_reset(cx)
                                                                }
                                                                Err(_) => {
                                                                    this.toast_manager.push(
                                                                        components::toast::Toast::error(
                                                                            activation_action
                                                                                .generic_failure_message(),
                                                                            &this.theme,
                                                                        )
                                                                        .duration_ms(Some(
                                                                            TOAST_ERROR_MS,
                                                                        )),
                                                                    );
                                                                    cx.notify();
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                    };

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
                                    .on_click(click_handler)
                                    .on_mouse_move(move_handler)
                                    .on_hover(hover_handler)
                                    .child(
                                        ListItem::new(display_title, list_colors)
                                            .icon_kind(icon.clone())
                                            .description_opt(description)
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
            .track_scroll(&self.browser_tabs_scroll_handle)
            .into_any_element()
        };

        let content = div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            .py(px(design_spacing.padding_xs))
            .on_scroll_wheel(cx.listener(
                move |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                    this.observe_builtin_native_list_scroll(event, cx);
                },
            ))
            .child(list_element)
            .child(self.builtin_uniform_list_scrollbar(
                &self.browser_tabs_scroll_handle,
                filtered_len,
                8,
            ));

        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            if self.is_in_attachment_portal() {
                vec![
                    gpui::SharedString::from("↵ Attach"),
                    gpui::SharedString::from("Esc Cancel"),
                ]
            } else {
                vec![
                    gpui::SharedString::from("↵ Open Tab"),
                    gpui::SharedString::from("Esc Back"),
                ]
            },
            None,
        ));
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(self.theme_font_family())
                .key_context("browser_tabs")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(
                    vec![self.render_builtin_main_input_count_label(
                        Self::browser_tabs_count_label(total_count),
                    )],
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

fn browser_tab_icon_for_render(
    tab: &crate::browser_tabs::BrowserTabInfo,
    app_icons: &std::collections::HashMap<String, crate::app_launcher::DecodedIcon>,
) -> list_item::IconKind {
    // Prefer per-site favicon (fetched from Google's favicon service)
    if let Some(favicon) = script_kit_gpui::favicons::cached_favicon(&tab.url) {
        return list_item::IconKind::Image(favicon);
    }

    // Fall back to browser app icon
    if let Some(icon) = app_icons.get(tab.browser_bundle_id.as_ref()) {
        return list_item::IconKind::Image(icon.clone());
    }

    // Last resort: browser-specific emoji
    match tab.browser_name.as_ref() {
        "Safari" => list_item::IconKind::Emoji("🧭".to_string()),
        "Google Chrome" => list_item::IconKind::Emoji("🌐".to_string()),
        "Arc" => list_item::IconKind::Emoji("🅰️".to_string()),
        "Brave Browser" => list_item::IconKind::Emoji("🦁".to_string()),
        "Microsoft Edge" => list_item::IconKind::Emoji("🌊".to_string()),
        _ => list_item::IconKind::Emoji("🔗".to_string()),
    }
}

#[cfg(test)]
mod browser_tabs_chrome_tests {
    fn production_source() -> &'static str {
        include_str!("browser_tabs.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source should exist")
    }

    #[test]
    fn browser_tabs_uses_shared_input_and_chrome() {
        let source = production_source();

        assert!(
            source.contains("render_builtin_main_input_header(")
                && source.contains("render_builtin_main_input_count_label("),
            "browser tabs must use the shared built-in main input header"
        );
        assert!(
            !source.contains(&["Input::new(&self.", "gpui_input_state)"].concat()),
            "browser tabs should delegate GPUI input construction to render_search_input"
        );
        assert!(
            !source.contains(&["HEADER_PADDING", "_X"].concat())
                && !source.contains(&["HEADER_PADDING", "_Y"].concat()),
            "browser tabs should not hardcode local main input header padding"
        );
        assert!(
            !source.contains(&["SectionDivider", "::new()"].concat()),
            "browser tabs should use the shared main-view divider contract"
        );
        assert!(
            source.contains("render_simple_hint_strip("),
            "browser tabs must use render_simple_hint_strip for footer"
        );
        assert!(
            source.contains(".key_context(\"browser_tabs\")"),
            "browser tabs must keep a dedicated key context"
        );
        assert!(
            !source.contains("CURSOR_WIDTH") && !source.contains("CURSOR_HEIGHT_LG"),
            "browser tabs must not build a custom fake cursor — use the shared Input"
        );
    }
}
