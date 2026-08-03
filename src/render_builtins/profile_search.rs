fn profile_search_row_description(result: &crate::profile_search::ProfileSearchResult) -> String {
    crate::profile_search::profile_search_result_description(&result.profile)
}

fn profile_search_row_status_accessory(
    result: &crate::profile_search::ProfileSearchResult,
    list_colors: &crate::list_item::ListItemColors,
    selected: bool,
) -> Option<AnyElement> {
    let label = match (result.selected, result.quick_ai) {
        (true, true) => "Current · Quick AI",
        (true, false) => "Current",
        (false, true) => "Quick AI",
        (false, false) => return None,
    };
    Some(
        div()
            .max_w(px(132.0))
            .overflow_hidden()
            .text_xs()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_color(rgba(crate::list_item::row_description_text_rgba(
                list_colors,
                selected,
            )))
            .child(label)
            .into_any_element(),
    )
}

fn profile_search_preview_meta_row(
    label: &'static str,
    value: String,
    text_primary: u32,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(text_primary))
                .child(label),
        )
        .child(div().text_sm().text_color(rgb(text_primary)).child(value))
        .into_any_element()
}

fn profile_search_preview_section(
    id: &'static str,
    title: &'static str,
    children: Vec<AnyElement>,
    text_primary: u32,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(text_primary))
                .child(title),
        )
        .children(children)
        .into_any_element()
}

impl ScriptListApp {
    fn render_profile_search(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("profile_search", false),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let list_colors = crate::list_item::ListItemColors::from_theme(&self.theme);
        let text_primary = self.theme.colors.text.primary;
        let text_dimmed = self.theme.colors.text.dimmed;
        let ui_border = self.theme.colors.ui.border;

        let results = self.profile_search_results_for_filter(&filter);
        let filtered_len = results.len();
        let selected_index = selected_index.min(filtered_len.saturating_sub(1));
        let selected = results.get(selected_index).cloned();

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
                if is_key_escape(key) {
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                if is_key_up(key) {
                    this.move_profile_search_selection(true, cx);
                    cx.stop_propagation();
                    return;
                }
                if is_key_down(key) {
                    this.move_profile_search_selection(false, cx);
                    cx.stop_propagation();
                    return;
                }
                if is_key_enter(key) {
                    cx.stop_propagation();
                    this.select_profile_search_result(cx);
                }
            },
        );

        let list_pane = if results.is_empty() {
            div()
                .id("profile-search-list")
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    crate::components::render_simple_empty_state(
                        "profile-search-empty",
                        if filter.trim().is_empty() {
                            "No profiles"
                        } else {
                            "No matching profiles"
                        },
                        "user",
                        None,
                        &self.theme,
                        cx,
                    ),
                )
                .into_any_element()
        } else {
            // Profile Search is a split-pane built-in, but its rows must use the
            // same shared ListItem chrome as Clipboard History and File Search.
            // This surface originally hand-built row padding/backgrounds while the
            // profile preview was introduced; ListItem owns selected/hover/theme
            // behavior so row metrics and opacity packing do not drift here.
            let profile_results_for_list = results.clone();
            let current_selected = selected_index;
            let profile_hovered = self.hovered_index;
            let main_menu_theme = self.current_main_menu_theme;
            let click_entity_handle = cx.entity().downgrade();
            let hover_entity_handle = cx.entity().downgrade();
            let list_element = uniform_list(
                "profile-search-results",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some(result) = profile_results_for_list.get(ix) {
                                let is_selected = ix == current_selected;
                                let is_hovered = profile_hovered == Some(ix);

                                let click_entity = click_entity_handle.clone();
                                let click_handler =
                                    move |_event: &gpui::ClickEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = click_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                if let AppView::ProfileSearchView {
                                                    selected_index,
                                                    ..
                                                } = &mut this.current_view
                                                {
                                                    *selected_index = ix;
                                                }
                                                this.list_scroll_handle
                                                    .scroll_to_item(ix, ScrollStrategy::Nearest);
                                                cx.notify();
                                            });
                                        }
                                        cx.stop_propagation();
                                    };

                                let hover_entity = hover_entity_handle.clone();
                                let hover_handler =
                                    move |hovered: &bool,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = hover_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                if *hovered {
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

                                let description = profile_search_row_description(result);

                                div()
                                    .id(ix)
                                    .cursor_pointer()
                                    .on_click(click_handler)
                                    .on_hover(hover_handler)
                                    .child(
                                        ListItem::new(result.profile.name.clone(), list_colors)
                                            .description(description)
                                            .highlight_indices_opt(
                                                result.name_highlight_indices.clone(),
                                            )
                                            .description_highlight_indices_opt(
                                                result.description_highlight_indices.clone(),
                                            )
                                            .selected(is_selected)
                                            .hovered(is_hovered)
                                            .main_menu_theme(main_menu_theme)
                                            .semantic_id(format!(
                                                "profile-search-row:{}",
                                                result.profile.id
                                            ))
                                            .trailing_accessory_opt(
                                                profile_search_row_status_accessory(
                                                    result,
                                                    &list_colors,
                                                    is_selected,
                                                ),
                                            ),
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
            .into_any_element();
            let list_scrollbar =
                self.builtin_uniform_list_scrollbar(&self.list_scroll_handle, filtered_len, 8);

            div()
                .id("profile-search-list")
                .relative()
                .w_full()
                .h_full()
                .min_h(px(0.))
                .overflow_hidden()
                .py(px(design_spacing.padding_xs))
                .child(list_element)
                .child(list_scrollbar)
                .into_any_element()
        };

        let preview_pane = div()
            .id("profile-search-preview")
            .w_full()
            .h_full()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p(px(16.0))
            .border_l_1()
            .border_color(rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
                ui_border,
                crate::theme::opacity::OPACITY_SUBTLE,
            )))
            .child(if let Some(result) = selected {
                let profile = result.profile;
                let cwd = profile
                    .cwd
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "Default".to_string());
                let badge_label = match (result.selected, result.quick_ai) {
                    (true, true) => Some("Current · Quick AI"),
                    (true, false) => Some("Current"),
                    (false, true) => Some("Quick AI"),
                    (false, false) => None,
                };
                let current_badge = badge_label.map(|label| {
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(text_primary))
                        .child(label)
                        .into_any_element()
                });
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .id("profile-search-preview-title")
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(text_primary))
                                            .child(profile.name.clone()),
                                    )
                                    .children(current_badge),
                            )
                            .child(
                                div()
                                    .id("profile-search-preview-explanation")
                                    .text_sm()
                                    .line_height(px(20.0))
                                    .text_color(rgb(text_dimmed))
                                    .child(crate::profile_search::profile_preview_explanation()),
                            ),
                    )
                    .child(profile_search_preview_section(
                        "profile-search-preview-overview",
                        "Overview",
                        vec![
                            profile_search_preview_meta_row(
                                "Source",
                                crate::profile_search::source_label(profile.source).to_string(),
                                text_primary,
                            ),
                            profile_search_preview_meta_row(
                                "Profile ID",
                                profile.id.clone(),
                                text_primary,
                            ),
                            profile_search_preview_meta_row(
                                "Backend",
                                crate::profile_search::backend_label(profile.backend).to_string(),
                                text_primary,
                            ),
                        ],
                        text_primary,
                    ))
                    .child(profile_search_preview_section(
                        "profile-search-preview-runtime",
                        "Runtime Setup",
                        vec![
                            div()
                                .id("profile-search-preview-model")
                                .child(profile_search_preview_meta_row(
                                    "Model",
                                    format!(
                                        "{} ({})",
                                        crate::profile_search::profile_model_label(&profile),
                                        crate::profile_search::backend_label(profile.backend)
                                    ),
                                    text_primary,
                                ))
                                .into_any_element(),
                            div()
                                .id("profile-search-preview-cwd")
                                .child(profile_search_preview_meta_row(
                                    "Working directory",
                                    cwd,
                                    text_primary,
                                ))
                                .into_any_element(),
                            div()
                                .id("profile-search-preview-tools")
                                .child(profile_search_preview_meta_row(
                                    "Tools",
                                    crate::profile_search::profile_tools_label(&profile),
                                    text_primary,
                                ))
                                .into_any_element(),
                        ],
                        text_primary,
                    ))
                    .child(
                        div()
                            .id("profile-search-preview-instructions")
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(text_primary))
                                    .child("Instructions"),
                            )
                            .child(
                                div()
                                    .id("profile-search-preview-prompt")
                                    .text_sm()
                                    .text_color(rgb(text_dimmed))
                                    .line_height(px(20.0))
                                    .child(crate::profile_search::profile_prompt_summary(&profile)),
                            ),
                    )
                    .into_any_element()
            } else {
                div()
                    .text_color(rgb(text_dimmed))
                    .child("Select a profile to preview its model, tools, cwd, and prompt.")
                    .into_any_element()
            });

        let hints: Vec<SharedString> = vec![
            "↵ Switch Profile".into(),
            "⇥ Use for Quick AI".into(),
            "Esc Back".into(),
        ];
        crate::components::emit_prompt_hint_audit("profile_search", &hints);
        let footer =
            self.main_window_footer_slot(crate::components::render_simple_hint_strip(hints, None));

        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let input = crate::components::main_view_chrome::render_main_view_input_shell(
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewInputChrome {
                body: self.render_search_input().into_any_element(),
                trailing: Vec::new(),
            },
        );
        let header = crate::components::main_view_chrome::MainViewHeaderChrome::canonical(
            menu_def,
            self.render_clickable_main_view_context_zone(menu_def, cx),
            input,
        );
        let divider = crate::components::main_view_chrome::MainViewDividerChrome {
            margin_x: shell.divider_margin_x,
            height: shell.divider_height,
            visible: false,
        };
        let main = div()
            .id("profile-search-root")
            .flex()
            .flex_row()
            .h_full()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .child(list_pane),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .child(preview_pane.into_any_element()),
            )
            .into_any_element();

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(self.theme_font_family())
                .key_context("ProfileSearchView")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header,
                divider,
                main,
                footer,
                overlays: Vec::new(),
            },
        )
    }
}
