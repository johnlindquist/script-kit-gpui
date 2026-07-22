impl ScriptListApp {
    /// Copy the selected tip's first example to the clipboard. Single action
    /// authority shared by the Enter key handler and the native footer's
    /// "Copy Example" button so the two can never drift.
    pub(crate) fn tips_copy_selected_example(&mut self, cx: &mut Context<Self>) {
        let AppView::TipsView {
            filter,
            selected_index,
            entries,
        } = &self.current_view
        else {
            return;
        };
        let visible = script_kit_gpui::tips::visible_tip_indices(entries, filter);
        let Some(example) = visible
            .get(*selected_index)
            .and_then(|index| entries.get(*index))
            .and_then(|tip| tip.examples.first())
        else {
            return;
        };
        if let Err(error) = crate::platform::copy_text_to_clipboard(&example.input) {
            tracing::warn!(%error, "tips copy-example failed");
        } else {
            self.show_hud(
                "Copied example — paste it in the main menu".to_string(),
                Some(2000),
                cx,
            );
        }
    }

    fn render_tips_view(
        &mut self,
        filter: &str,
        selected_index: usize,
        entries: Vec<script_kit_gpui::tips::Tip>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("tips", false),
        );
        let tokens = get_tokens(self.current_design);
        let spacing = tokens.spacing();
        let typography = tokens.typography();
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let primary = rgb(chrome.text_primary_hex);
        let muted = rgba(chrome.text_muted_rgba);
        let hint = rgba(chrome.text_hint_rgba);
        let list_colors = ListItemColors::from_theme(&self.theme);

        let visible = script_kit_gpui::tips::visible_tip_indices(&entries, filter);
        let filtered_len = visible.len();
        let selected = selected_index.min(filtered_len.saturating_sub(1));
        if selected != selected_index {
            if let AppView::TipsView { selected_index, .. } = &mut self.current_view {
                *selected_index = selected;
            }
        }
        let preview = visible
            .get(selected)
            .and_then(|index| entries.get(*index))
            .cloned();

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);
                let key = event.keystroke.key.as_str();
                if crate::ui_foundation::is_key_escape(key) {
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                let (filtered_len, current) = match &this.current_view {
                    AppView::TipsView {
                        filter,
                        selected_index,
                        entries,
                    } => (
                        script_kit_gpui::tips::visible_tip_indices(entries, filter).len(),
                        *selected_index,
                    ),
                    _ => return,
                };
                if crate::ui_foundation::is_key_up(key) {
                    let next = current.saturating_sub(1);
                    if let AppView::TipsView { selected_index, .. } = &mut this.current_view {
                        *selected_index = next;
                    }
                    this.tips_list_scroll_handle
                        .scroll_to_item(next, ScrollStrategy::Nearest);
                    cx.notify();
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_down(key) {
                    let next = (current + 1).min(filtered_len.saturating_sub(1));
                    if let AppView::TipsView { selected_index, .. } = &mut this.current_view {
                        *selected_index = next;
                    }
                    this.tips_list_scroll_handle
                        .scroll_to_item(next, ScrollStrategy::Nearest);
                    cx.notify();
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_enter(key) {
                    this.tips_copy_selected_example(cx);
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        let list_element: AnyElement =
            if filtered_len == 0 {
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(hint)
                    .child("No tips match your filter")
                    .into_any_element()
            } else {
                let entries_for_rows = entries.clone();
                let visible_for_rows = visible.clone();
                let current_selected = selected;
                let hovered = self.hovered_index;
                let click_entity_handle = cx.entity().downgrade();
                let hover_entity_handle = cx.entity().downgrade();
                uniform_list(
                    "tips-list-rows",
                    filtered_len,
                    move |visible_range, _window, _cx| {
                        visible_range
                            .map(|row| {
                                if let Some(tip) = visible_for_rows
                                    .get(row)
                                    .and_then(|index| entries_for_rows.get(*index))
                                {
                                    let underlying_index = visible_for_rows[row];
                                    let row_id = format!("tip-{underlying_index}-{}", tip.title);
                                    let is_selected = row == current_selected;
                                    let is_hovered = hovered == Some(row);

                                    let click_entity = click_entity_handle.clone();
                                    let click_handler = move |event: &gpui::ClickEvent,
                                                      _window: &mut Window,
                                                      cx: &mut gpui::App| {
                                if let Some(app) = click_entity.upgrade() {
                                    app.update(cx, |this, cx| {
                                        if let AppView::TipsView { selected_index, .. } =
                                            &mut this.current_view
                                        {
                                            *selected_index = row;
                                        }
                                        this.tips_list_scroll_handle
                                            .scroll_to_item(row, ScrollStrategy::Nearest);
                                        this.note_list_pointer_click(row, cx);
                                        if let gpui::ClickEvent::Mouse(mouse_event) = event {
                                            if mouse_event.down.click_count == 2 {
                                                this.tips_copy_selected_example(cx);
                                            }
                                        }
                                        cx.notify();
                                    });
                                }
                                cx.stop_propagation();
                            };

                                    let hover_entity = hover_entity_handle.clone();
                                    let move_entity = hover_entity_handle.clone();
                                    let move_handler = move |_event: &gpui::MouseMoveEvent,
                                                             _window: &mut Window,
                                                             cx: &mut gpui::App| {
                                        if let Some(app) = move_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                this.note_list_pointer_move(row, cx);
                                            });
                                        }
                                    };
                                    let hover_handler = move |is_hovered: &bool,
                                                      _window: &mut Window,
                                                      cx: &mut gpui::App| {
                                if let Some(app) = hover_entity.upgrade() {
                                    app.update(cx, |this, cx| {
                                        if !*is_hovered {
                                            this.note_list_pointer_leave(row, cx);
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
                                            ListItem::new(tip.title.clone(), list_colors)
                                                .description_opt(Some(tip.full_hint()))
                                                .selected(is_selected)
                                                .hovered(is_hovered),
                                        )
                                } else {
                                    div().id(row).h(px(LIST_ITEM_HEIGHT))
                                }
                            })
                            .collect()
                    },
                )
                .h_full()
                .track_scroll(&self.tips_list_scroll_handle)
                .into_any_element()
            };
        let list_scrollbar =
            self.builtin_uniform_list_scrollbar(&self.tips_list_scroll_handle, filtered_len, 8);

        let list = div()
            .id("tips-list")
            .w_full()
            .h_full()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .on_scroll_wheel(cx.listener(
                move |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                    this.observe_builtin_native_list_scroll(event, cx);
                },
            ))
            .child(
                // Every list leads with a persistent section separator
                // (POLISH.md layout-stability bar): the label may swap but the
                // row never appears or disappears, so filtering can't shift
                // the rows below it.
                crate::list_item::render_section_header(
                    if filter.trim().is_empty() {
                        "Tips"
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
                    .child(list_scrollbar),
            );

        let preview_panel = if let Some(tip) = preview {
            div()
                .w_full()
                .h_full()
                .min_h(px(0.))
                .overflow_y_scrollbar()
                .px(px(spacing.padding_lg))
                .py(px(spacing.padding_md))
                .font_family(typography.font_family)
                .flex()
                .flex_col()
                .gap(px(spacing.padding_md))
                .child(
                    div()
                        .text_size(px(typography.font_size_xl))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(primary)
                        .child(tip.title),
                )
                .child(
                    div()
                        .text_size(px(typography.font_size_md))
                        .text_color(muted)
                        .child(tip.description),
                )
                .children(tip.examples.into_iter().map(|example| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(spacing.padding_xs))
                        .child(
                            div()
                                .font_family(typography.font_family_mono)
                                .text_size(px(typography.font_size_sm))
                                .text_color(primary)
                                .child(example.input),
                        )
                        .child(
                            div()
                                .text_size(px(typography.font_size_xs))
                                .text_color(hint)
                                .child(example.note),
                        )
                }))
                .into_any_element()
        } else {
            div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(hint)
                .child("No tips match your filter")
                .into_any_element()
        };

        let hints: Vec<SharedString> = vec!["↵ Copy Example".into(), "Esc Back".into()];
        crate::components::emit_prompt_hint_audit("tips", &hints);
        let footer =
            self.main_window_footer_slot(crate::components::render_simple_hint_strip(hints, None));
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let main = self.render_builtin_split_main_content(list.into_any_element(), preview_panel);
        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(primary)
                .font_family(self.theme_font_family())
                .key_context("tips")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(
                    vec![self
                        .render_builtin_main_input_count_label(format!("{} tips", entries.len()))],
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

#[cfg(test)]
mod tips_consistency_audit {
    /// WHY: the Tips browser shipped once with a footer that bypassed the
    /// native main-window footer and a list that never scrolled its selection
    /// into view (2026-07-11 regression). These assertions pin the surface to
    /// the shared builtin-browser anatomy: tracked uniform list with
    /// scroll-into-view, vendor scrollbar, shared
    /// chrome, and the persistent native-footer slot.
    #[test]
    fn tips_list_scrolls_selection_into_view() {
        let source = include_str!("tips.rs");
        assert!(
            source.contains(".track_scroll(&self.tips_list_scroll_handle)"),
            "tips list must be a tracked uniform list"
        );
        assert!(
            source.contains("builtin_uniform_list_scrollbar("),
            "tips must attach the shared vendor scrollbar helper"
        );
    }

    #[test]
    fn tips_footer_routes_through_shared_chrome() {
        let source = include_str!("tips.rs");
        assert!(
            source.contains("render_main_view_chrome_footer_flush(")
                && source.contains("render_builtin_main_input_header("),
            "tips must use shared main-view chrome and built-in input header"
        );
        assert!(
            source.contains("main_window_footer_slot("),
            "tips footer must route through the persistent main-window footer"
        );
    }

    #[test]
    fn tips_uses_single_filter_authority() {
        let source = include_str!("tips.rs");
        // Composed so the needle does not match this assertion itself.
        let hand_rolled_filter = ["to_lowercase()", ".contains"].concat();
        assert!(
            !source.contains(&hand_rolled_filter),
            "tips must not hand-roll filtering; use script_kit_gpui::tips::visible_tip_indices"
        );
    }
}
