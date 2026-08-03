#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserHistoryEmptyState {
    NoHistoryFound,
    NoFilteredMatches,
}

impl BrowserHistoryEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoHistoryFound
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoHistoryFound => "No browser history found",
            Self::NoFilteredMatches => "No browser history entries match your filter",
        }
    }
}

impl ScriptListApp {
    fn browser_history_attachment_part(
        &self,
        index: usize,
        entry: &crate::browser_history::BrowserHistoryEntry,
    ) -> crate::ai::message_parts::AiContextPart {
        let title = entry.display_title().to_string();
        let target = crate::ai::TabAiTargetContext {
            source: "BrowserHistory".to_string(),
            kind: "browser_history_entry".to_string(),
            semantic_id: crate::protocol::generate_semantic_id(
                "browser-history",
                index,
                &entry.history_key(),
            ),
            label: title.clone(),
            metadata: Some(serde_json::json!({
                "browserName": entry.browser_name,
                "browserBundleId": entry.browser_bundle_id,
                "title": entry.title,
                "url": entry.url,
                "host": entry.host,
                "lastVisitedAtMs": entry.last_visited_at_ms,
                "lastVisitedAt": crate::browser_history::format_history_timestamp(entry.last_visited_at_ms),
                "visitCount": entry.visit_count,
                "profile": entry.profile,
            })),
        };
        let _needing = script_kit_gpui::favicons::domains_needing_favicons(&[entry.url.clone()]);
        let label = crate::ai::format_explicit_target_chip_label(&target);
        crate::ai::message_parts::AiContextPart::FocusedTarget { target, label }
    }

    fn browser_history_meta(entry: &crate::browser_history::BrowserHistoryEntry) -> String {
        crate::browser_history::format_browser_history_meta(entry)
    }

    fn render_browser_history(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use gpui_component::scroll::ScrollableElement as _;

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("browser_history", false),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();

        let text_primary = self.theme.colors.text.primary;
        let text_muted = self.theme.colors.text.muted;

        let filtered_entries: Vec<crate::browser_history::BrowserHistoryEntry> =
            crate::browser_history::fuzzy_search_browser_history(
                &self.cached_browser_history,
                &filter,
            )
            .into_iter()
            .map(|hit| hit.entry)
            .collect();
        let filtered_len = filtered_entries.len();
        let row_keys: Vec<String> = filtered_entries
            .iter()
            .map(|entry| entry.history_key())
            .collect();
        let selected_index = crate::reconcile_dynamic_tracked_list_on_render(
            self.tracked_builtin_list_states
                .entry("browser_history")
                .or_default(),
            &filter,
            &row_keys,
            selected_index,
            &self.browser_history_scroll_handle,
        );
        if let AppView::BrowserHistoryView {
            selected_index: current_selected,
            ..
        } = &mut self.current_view
        {
            *current_selected = selected_index;
        }
        let selected_entry = filtered_entries.get(selected_index).cloned();
        let in_portal = self.is_in_attachment_portal();

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

                if crate::ui_foundation::is_key_escape(key) && !this.show_actions_popup {
                    if !this.clear_builtin_view_filter(cx) {
                        if this.is_in_attachment_portal() {
                            this.close_attachment_portal_cancel(cx);
                        } else {
                            this.go_back_or_close(window, cx);
                        }
                    }
                    cx.stop_propagation();
                    return;
                }

                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                let Some((current_filter, current_selected)) = (match &this.current_view {
                    AppView::BrowserHistoryView {
                        filter,
                        selected_index,
                    } => Some((filter.clone(), *selected_index)),
                    _ => None,
                }) else {
                    return;
                };

                let filtered_entries: Vec<crate::browser_history::BrowserHistoryEntry> =
                    crate::browser_history::fuzzy_search_browser_history(
                        &this.cached_browser_history,
                        &current_filter,
                    )
                    .into_iter()
                    .map(|hit| hit.entry)
                    .collect();
                let filtered_len = filtered_entries.len();

                if crate::ui_foundation::is_key_up(key) {
                    if filtered_len > 0 {
                        let next = current_selected.saturating_sub(1);
                        if let AppView::BrowserHistoryView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = next;
                        }
                        this.browser_history_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_down(key) {
                    if filtered_len > 0 {
                        let next = current_selected
                            .saturating_add(1)
                            .min(filtered_len.saturating_sub(1));
                        if let AppView::BrowserHistoryView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = next;
                        }
                        this.browser_history_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_enter(key) {
                    if this.is_in_attachment_portal() {
                        if let Some(entry) = filtered_entries.get(current_selected) {
                            let part =
                                this.browser_history_attachment_part(current_selected, entry);
                            this.close_attachment_portal_with_part(part, cx);
                        }
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        let list_colors = ListItemColors::from_theme(&self.theme);
        let list_element: AnyElement = if self.browser_history_loading {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(text_muted))
                .font_family(design_typography.font_family)
                .child("Loading browser history...")
                .into_any_element()
        } else if filtered_len == 0 {
            let state = BrowserHistoryEmptyState::from_filter(&filter);
            crate::components::render_simple_empty_state(
                "browser-history-empty",
                state.message(),
                "search",
                None,
                &self.theme,
                cx,
            )
        } else {
            let selected = selected_index;
            let hovered = self.hovered_index;
            let entity = cx.entity().downgrade();
            let app_icons: std::collections::HashMap<String, crate::app_launcher::DecodedIcon> =
                self.apps
                    .iter()
                    .filter_map(|app| {
                        app.bundle_id
                            .clone()
                            .and_then(|bundle_id| app.icon.clone().map(|icon| (bundle_id, icon)))
                    })
                    .collect();

            crate::components::scrollbar::render_tracked_scroll_column(
                "browser-history-list",
                &self.browser_history_scroll_handle,
                filtered_entries.iter().enumerate().map(move |(display_ix, entry)| {
                    let history_key = entry.history_key();
                    let icon = browser_history_icon_for_render(entry, &app_icons);
                    let item = ListItem::new(entry.display_title().to_string(), list_colors)
                        .icon_kind(icon)
                        .description_opt(Some(Self::browser_history_meta(entry)))
                        .selected(display_ix == selected)
                        .hovered(hovered == Some(display_ix))
                        .semantic_id(format!("browser-history:{history_key}"));

                    let click_entity = entity.clone();
                    let move_entity = entity.clone();
                    let hover_entity = entity.clone();
                    let app_icons = app_icons.clone();
                    div()
                        .id(format!("browser-history-row:{history_key}"))
                        .cursor_pointer()
                        .on_click(move |event, _window, cx| {
                            if let Some(app) = click_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    if let AppView::BrowserHistoryView { selected_index, .. } =
                                        &mut this.current_view
                                    {
                                        *selected_index = display_ix;
                                    }
                                    this.browser_history_scroll_handle
                                        .scroll_to_item(display_ix);
                                    this.note_list_pointer_click(display_ix, cx);
                                    if let gpui::ClickEvent::Mouse(mouse_event) = event {
                                        if mouse_event.down.click_count == 2
                                            && this.is_in_attachment_portal()
                                        {
                                            let filtered_entries: Vec<
                                                crate::browser_history::BrowserHistoryEntry,
                                            > = crate::browser_history::fuzzy_search_browser_history(
                                                &this.cached_browser_history,
                                                this.filter_text(),
                                            )
                                            .into_iter()
                                            .map(|hit| hit.entry)
                                            .collect();
                                            if let Some(entry) =
                                                filtered_entries.get(display_ix)
                                            {
                                                let _icon = browser_history_icon_for_render(entry, &app_icons);
                                                let part = this.browser_history_attachment_part(
                                                    display_ix,
                                                    entry,
                                                );
                                                this.close_attachment_portal_with_part(part, cx);
                                            }
                                        }
                                    }
                                });
                            }
                        })
                        .on_mouse_move(move |_event, _window, cx| {
                            if let Some(app) = move_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.note_list_pointer_move(display_ix, cx);
                                });
                            }
                        })
                        .on_hover(move |is_hovered, _window, cx| {
                            if !*is_hovered {
                                if let Some(app) = hover_entity.upgrade() {
                                    app.update(cx, |this, cx| {
                                        this.note_list_pointer_leave(display_ix, cx);
                                    });
                                }
                            }
                        })
                        .child(item)
                }),
            )
        };

        let preview_panel: AnyElement = match selected_entry {
            Some(entry) => div()
                .w_full()
                .h_full()
                .min_h(px(0.))
                .overflow_y_scrollbar()
                .px(px(design_spacing.padding_lg))
                .py(px(design_spacing.padding_md))
                .font_family(design_typography.font_family)
                .child(
                    div()
                        .w_full()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .child(Self::browser_history_meta(&entry)),
                )
                .child(
                    div()
                        .w_full()
                        .pt(px(design_spacing.padding_md))
                        .text_sm()
                        .text_color(rgb(text_primary))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(entry.display_title().to_string()),
                )
                .child(
                    div()
                        .w_full()
                        .pt(px(design_spacing.padding_sm))
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .child(entry.url.to_string()),
                )
                .into_any_element(),
            None => div()
                .w_full()
                .h_full()
                .min_h(px(0.))
                .px(px(design_spacing.padding_lg))
                .py(px(design_spacing.padding_md))
                .font_family(design_typography.font_family)
                .text_xs()
                .text_color(rgb(text_muted))
                .child("Select a browser history entry to preview it")
                .into_any_element(),
        };

        let list_pane = div()
            .relative()
            .w_full()
            .h_full()
            .min_h(px(0.))
            .py(px(design_spacing.padding_xs))
            .on_scroll_wheel(cx.listener(
                move |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                    this.observe_builtin_native_list_scroll(event, cx);
                },
            ))
            .flex()
            .flex_col()
            .child(
                // Every list leads with a persistent section separator
                // (POLISH.md layout-stability bar; same rule as the main
                // menu's "Results" header, 4d76327b8): the label may swap but
                // the row never appears or disappears, so filtering can't
                // shift the rows below it.
                crate::list_item::render_section_header(
                    if filter.trim().is_empty() {
                        "History"
                    } else {
                        "Results"
                    },
                    None,
                    list_colors,
                    true,
                ),
            )
            .child(div().relative().flex_1().min_h(px(0.)).child(list_element));

        let hints = if in_portal {
            vec!["↵ Attach".into(), "Esc Cancel".into()]
        } else {
            vec!["Esc Back".into()]
        };
        crate::components::emit_prompt_hint_audit("browser_history", &hints);

        let gpui_footer = crate::components::render_simple_hint_strip(hints, None);
        let footer = self.main_window_footer_slot(gpui_footer);
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let count_label = format!(
            "{} entr{}",
            self.cached_browser_history.len(),
            if self.cached_browser_history.len() == 1 {
                "y"
            } else {
                "ies"
            }
        );
        let main =
            self.render_builtin_split_main_content(list_pane.into_any_element(), preview_panel);

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(self.theme_font_family())
                .key_context("browser_history")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(
                    vec![self.render_builtin_main_input_count_label(count_label)],
                    cx,
                ),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main,
                footer,
                overlays: Vec::new(),
            },
        )
    }
}

fn browser_history_icon_for_render(
    entry: &crate::browser_history::BrowserHistoryEntry,
    app_icons: &std::collections::HashMap<String, crate::app_launcher::DecodedIcon>,
) -> list_item::IconKind {
    // Prefer per-site favicon
    if let Some(favicon) = script_kit_gpui::favicons::cached_favicon(&entry.url) {
        return list_item::IconKind::Image(favicon);
    }

    // Fall back to browser app icon if available
    if let Some(icon) = app_icons.get(entry.browser_bundle_id.as_ref()) {
        return list_item::IconKind::Image(icon.clone());
    }

    // Last resort: browser-specific emoji
    match entry.browser_name.as_ref() {
        "Safari" => list_item::IconKind::Emoji("🧭".to_string()),
        "Google Chrome" => list_item::IconKind::Emoji("🌐".to_string()),
        "Arc" => list_item::IconKind::Emoji("🅰️".to_string()),
        "Brave Browser" => list_item::IconKind::Emoji("🦁".to_string()),
        "Microsoft Edge" => list_item::IconKind::Emoji("🌊".to_string()),
        _ => list_item::IconKind::Emoji("🔗".to_string()),
    }
}
