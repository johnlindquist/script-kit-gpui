#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DictationHistoryEmptyState {
    NoSavedDictation,
    NoFilteredMatches,
}

const DICTATION_HISTORY_ROW_SELECTOR_PREFIX: &str = "dictation-history-row-";

fn dictation_history_row_selector(entry_id: &str) -> String {
    format!("{DICTATION_HISTORY_ROW_SELECTOR_PREFIX}{entry_id}")
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DictationHistoryRowLayout {
    viewport_paint_offset_x: f32,
    row_paint_offset_x: f32,
    requested_width: f32,
}

fn dictation_history_row_layout(
    slot_width: f32,
    row_outer_padding_x: f32,
    chrome_edge_inset: f32,
) -> DictationHistoryRowLayout {
    DictationHistoryRowLayout {
        viewport_paint_offset_x: (row_outer_padding_x - chrome_edge_inset).max(0.0),
        row_paint_offset_x: 0.0,
        requested_width: slot_width,
    }
}

fn render_dictation_history_row(
    display_index: usize,
    entry: &crate::dictation::DictationHistoryEntry,
    list_colors: ListItemColors,
    main_menu_theme: crate::designs::MainMenuThemeVariant,
    layout: DictationHistoryRowLayout,
    selected: bool,
    hovered: bool,
) -> impl IntoElement {
    let selector = dictation_history_row_selector(&entry.id);
    let metrics = crate::list_item::ListItemMetricsOverride::from_main_menu_theme(main_menu_theme);
    let item = ListItem::new(entry.preview.clone(), list_colors)
        .description_opt(Some(ScriptListApp::dictation_history_meta(entry)))
        .selected(selected)
        .hovered(hovered)
        .semantic_id(format!("dictation-history:{}", entry.id))
        .main_menu_theme(main_menu_theme);

    div()
        .id(gpui::ElementId::Integer(display_index as u64))
        .debug_selector(move || selector)
        .relative()
        .left(px(layout.row_paint_offset_x))
        .w(px(layout.requested_width))
        .h(px(metrics.item_height))
        .child(item)
}

fn render_dictation_history_results_list<I, E>(
    id: &'static str,
    scroll_handle: &gpui::ScrollHandle,
    rows: I,
    layout: DictationHistoryRowLayout,
) -> gpui::AnyElement
where
    I: IntoIterator<Item = E>,
    E: gpui::IntoElement,
{
    div()
        .relative()
        .left(px(layout.viewport_paint_offset_x))
        .w(px(layout.requested_width))
        .h_full()
        .min_h(px(0.0))
        .child(crate::components::scrollbar::render_tracked_scroll_column(
            id,
            scroll_handle,
            rows,
        ))
        .into_any_element()
}

fn render_dictation_history_leading_slot(
    label: &str,
    list_colors: ListItemColors,
    main_menu_theme: crate::designs::MainMenuThemeVariant,
    top_inset: f32,
    chrome_edge_inset: f32,
) -> impl IntoElement {
    let metrics = crate::list_item::ListItemMetricsOverride::from_main_menu_theme(main_menu_theme);

    div()
        .w_full()
        .h(px(metrics.item_height))
        .mt(px(-chrome_edge_inset))
        .pt(px(top_inset + chrome_edge_inset))
        .child(
            crate::components::builtin_leading_separator::render_builtin_leading_separator(
                label,
                None,
                list_colors,
            ),
        )
}

impl DictationHistoryEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoSavedDictation
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoSavedDictation => "No saved dictation yet",
            Self::NoFilteredMatches => "No dictations match your filter",
        }
    }
}

impl ScriptListApp {
    pub(crate) fn dictation_history_visible_rows(
        filter: &str,
    ) -> Vec<crate::dictation::DictationHistoryEntry> {
        crate::dictation::search_history(filter, 100)
            .into_iter()
            .map(|hit| hit.entry)
            .collect()
    }

    fn dictation_history_selected_visible_row(
        filter: &str,
        selected_index: usize,
    ) -> Option<crate::dictation::DictationHistoryEntry> {
        Self::dictation_history_visible_rows(filter)
            .get(selected_index)
            .cloned()
    }

    fn dictation_history_dataset_and_visible_counts(filter: &str) -> (usize, usize) {
        (
            crate::dictation::load_history().len(),
            Self::dictation_history_visible_rows(filter).len(),
        )
    }

    pub(crate) fn dictation_history_visible_row_labels(filter: &str) -> Vec<String> {
        Self::dictation_history_visible_rows(filter)
            .into_iter()
            .map(|entry| entry.preview)
            .collect()
    }

    fn dictation_history_meta(entry: &crate::dictation::DictationHistoryEntry) -> String {
        format!(
            "{} · {} · {}",
            entry.target,
            crate::dictation::format_history_duration_ms(entry.audio_duration_ms),
            crate::dictation::format_history_timestamp(&entry.timestamp)
        )
    }

    fn dictation_history_attachment_part(
        entry: &crate::dictation::DictationHistoryEntry,
    ) -> crate::ai::message_parts::AiContextPart {
        crate::ai::message_parts::AiContextPart::ResourceUri {
            uri: format!("kit://dictation-history?id={}", entry.id),
            label: format!("Dictation: {}", entry.preview),
        }
    }

    /// Render the saved dictation history browser (list + preview).
    fn render_dictation_history(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use gpui_component::scroll::ScrollableElement as _;

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("dictation_history", false),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let color_resolver =
            crate::theme::ColorResolver::new_for_shell(&self.theme, self.current_design);
        let typography_resolver =
            crate::theme::TypographyResolver::new_theme_first(&self.theme, self.current_design);
        let empty_text_color = color_resolver.empty_text_color();
        let empty_font_family = typography_resolver.primary_font().to_string();

        let all_entries = crate::dictation::load_history();
        let text_primary = self.theme.colors.text.primary;
        let text_muted = self.theme.colors.text.muted;

        let hits = crate::dictation::search_history(&filter, 100);
        let filtered_entries: Vec<crate::dictation::DictationHistoryEntry> =
            hits.into_iter().map(|hit| hit.entry).collect();
        let filtered_len = filtered_entries.len();
        let row_keys: Vec<String> = filtered_entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect();
        let selected_index = crate::reconcile_dynamic_tracked_list_on_render(
            self.tracked_builtin_list_states
                .entry("dictation_history")
                .or_default(),
            &filter,
            &row_keys,
            selected_index,
            &self.dictation_history_scroll_handle,
        );
        if let AppView::DictationHistoryView {
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

                let key = event.keystroke.key.as_str();
                let key_char = event.keystroke.key_char.as_deref();
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;

                match this.route_key_to_actions_dialog(
                    key,
                    key_char,
                    modifiers,
                    ActionsDialogHost::DictationHistory,
                    window,
                    cx,
                ) {
                    ActionsRoute::NotHandled => {}
                    ActionsRoute::Handled => {
                        cx.stop_propagation();
                        return;
                    }
                    ActionsRoute::Execute {
                        action_id,
                        should_close,
                    } => {
                        if should_close {
                            this.close_actions_popup(
                                ActionsDialogHost::DictationHistory,
                                window,
                                cx,
                            );
                        }
                        this.handle_action(action_id, window, cx);
                        cx.stop_propagation();
                        return;
                    }
                }

                if is_key_escape(key) {
                    if this.is_in_attachment_portal() {
                        this.close_attachment_portal_cancel(cx);
                        cx.stop_propagation();
                        return;
                    }
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

                let view_state = if let AppView::DictationHistoryView {
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

                let hits = crate::dictation::search_history(&current_filter, 100);
                let filtered: Vec<crate::dictation::DictationHistoryEntry> =
                    hits.into_iter().map(|hit| hit.entry).collect();
                let current_filtered_len = filtered.len();
                let selected_entry = filtered.get(current_selected).cloned();

                if is_key_up(key) {
                    if current_filtered_len > 0 {
                        let next = current_selected.saturating_sub(1);
                        if let AppView::DictationHistoryView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = next;
                        }
                        this.dictation_history_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_down(key) {
                    if current_filtered_len > 0 {
                        let next = current_selected
                            .saturating_add(1)
                            .min(current_filtered_len.saturating_sub(1));
                        if let AppView::DictationHistoryView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = next;
                        }
                        this.dictation_history_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_enter(key) {
                    if has_cmd {
                        if let Some(entry) = selected_entry {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                entry.transcript,
                            ));
                            this.show_hud(
                                "Copied dictation to clipboard".to_string(),
                                Some(HUD_MEDIUM_MS),
                                cx,
                            );
                        }
                    } else if this.is_in_attachment_portal() {
                        if let Some(entry) = selected_entry {
                            let part = Self::dictation_history_attachment_part(&entry);
                            this.close_attachment_portal_with_part(part, cx);
                        }
                    } else if selected_entry.is_some() {
                        this.handle_action("dictation_history_paste".to_string(), window, cx);
                    }
                    cx.stop_propagation();
                } else if has_cmd && key.eq_ignore_ascii_case("k") {
                    if let Some(entry) = selected_entry {
                        this.toggle_dictation_history_actions(entry, window, cx);
                    }
                    cx.stop_propagation();
                } else if modifiers.control && has_cmd && key.eq_ignore_ascii_case("a") {
                    if selected_entry.is_some() {
                        this.handle_action(
                            "dictation_history_attach_to_ai".to_string(),
                            window,
                            cx,
                        );
                    }
                    cx.stop_propagation();
                } else if key.eq_ignore_ascii_case("backspace") && has_cmd {
                    if selected_entry.is_some() {
                        this.handle_action("dictation_history_delete".to_string(), window, cx);
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        let list_colors = ListItemColors::from_theme(&self.theme);
        let main_menu_theme = self.current_main_menu_theme;
        let slot_width = crate::window_resize::MAIN_WINDOW_WIDTH * 0.5;
        let chrome_edge_inset = crate::window_resize::layout::WINDOW_BORDER_Y * 0.5;
        let row_metrics =
            crate::list_item::ListItemMetricsOverride::from_main_menu_theme(main_menu_theme);
        let row_layout = dictation_history_row_layout(
            slot_width,
            row_metrics.row_outer_padding_x,
            chrome_edge_inset,
        );
        let list_element: AnyElement = if filtered_len == 0 {
            let state = DictationHistoryEmptyState::from_filter(&filter);
            crate::list_item::EmptyState::new(state.message(), empty_text_color, &empty_font_family)
                .icon(crate::designs::icon_variations::IconName::MessageCircle)
                .into_element()
        } else {
            let selected = selected_index;
            let hovered = self.hovered_index;
            let entity = cx.entity().downgrade();
            render_dictation_history_results_list(
                "dictation-history-list",
                &self.dictation_history_scroll_handle,
                filtered_entries
                    .iter()
                    .enumerate()
                    .map(move |(display_ix, entry)| {
                        let entry_id = entry.id.clone();
                        let click_entity = entity.clone();
                        let move_entity = entity.clone();
                        let hover_entity = entity.clone();
                        div()
                            .id(format!("dictation-history-row:{entry_id}"))
                            .w_full()
                            .cursor_pointer()
                            .on_click(move |_event, _window, cx| {
                                if let Some(app) = click_entity.upgrade() {
                                    app.update(cx, |this, cx| {
                                        if let AppView::DictationHistoryView {
                                            selected_index,
                                            ..
                                        } = &mut this.current_view
                                        {
                                            *selected_index = display_ix;
                                        }
                                        this.dictation_history_scroll_handle
                                            .scroll_to_item(display_ix);
                                        this.note_list_pointer_click(display_ix, cx);
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
                            .child(render_dictation_history_row(
                                display_ix,
                                entry,
                                list_colors,
                                main_menu_theme,
                                row_layout,
                                display_ix == selected,
                                hovered == Some(display_ix),
                            ))
                    }),
                row_layout,
            )
        };

        let preview_panel: AnyElement = match selected_entry {
            Some(entry) => div()
                .w_full()
                .h_full()
                .min_w_0()
                .min_h(px(0.))
                .overflow_y_scrollbar()
                .px(px(design_spacing.padding_lg))
                .py(px(design_spacing.padding_md))
                .font_family(design_typography.font_family)
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .child(Self::dictation_history_meta(&entry)),
                )
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .pt(px(design_spacing.padding_md))
                        .text_sm()
                        .text_color(rgb(text_primary))
                        .child(entry.transcript),
                )
                .into_any_element(),
            None => div()
                .w_full()
                .h_full()
                .min_h(px(0.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(text_muted))
                .font_family(design_typography.font_family)
                .child("Select a dictation to preview it")
                .into_any_element(),
        };

        let list_pane = div()
            .relative()
            .w_full()
            .h_full()
            .min_h(px(0.))
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
                render_dictation_history_leading_slot(
                    if filter.trim().is_empty() {
                        "Transcripts"
                    } else {
                        "Results"
                    },
                    list_colors,
                    main_menu_theme,
                    design_spacing.padding_xs,
                    chrome_edge_inset,
                ),
            )
            .child(div().relative().flex_1().min_h(px(0.)).child(list_element));

        let hints = if in_portal {
            vec![
                "↵ Attach".into(),
                "⌘↵ Copy".into(),
                "Esc Cancel".into(),
                "Attaching to Agent Chat".into(),
            ]
        } else {
            vec![
                "↵ Paste".into(),
                "⌘↵ Copy".into(),
                "⌃⌘A AI".into(),
                "⌘K Actions".into(),
                "⌘⌫ Delete".into(),
                "Esc Back".into(),
            ]
        };
        crate::components::emit_prompt_hint_audit("dictation_history", &hints);

        let gpui_footer = crate::components::render_simple_hint_strip(hints, None);
        let footer = self.main_window_footer_slot(gpui_footer);
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let count_label = format!(
            "{} dictation{}",
            all_entries.len(),
            if all_entries.len() == 1 { "" } else { "s" }
        );
        let main =
            self.render_builtin_split_main_content(list_pane.into_any_element(), preview_panel);

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(self.theme_font_family())
                .key_context("dictation_history")
                .on_key_down(handle_key)
                .track_focus(&self.focus_handle),
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

#[cfg(test)]
mod dictation_history_scroll_contract {
    fn production_source() -> &'static str {
        include_str!("dictation_history.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source should exist")
    }

    #[test]
    fn dictation_history_tracks_scroll_and_copy_shortcuts() {
        let source = production_source();

        assert!(
            source.contains("render_builtin_main_input_header("),
            "dictation history should expose the shared built-in main input header"
        );
        assert!(
            !source.contains(&["Input::new(&self.", "gpui_input_state)"].concat()),
            "dictation history should delegate GPUI input construction to render_search_input"
        );
        assert!(
            source.contains("render_main_view_chrome_footer_flush("),
            "dictation history should use the shared main-view chrome"
        );
        assert!(
            source.contains("\"dictation_history_paste\""),
            "dictation history should route Enter through the paste action"
        );
        assert!(
            source.contains("\"dictation_history_delete\""),
            "dictation history should surface the delete action"
        );
        assert!(
            source.contains("toggle_dictation_history_actions"),
            "dictation history should expose a dedicated actions menu"
        );
    }
}

#[cfg(test)]
mod dictation_history_paint_tests {
    use super::*;

    const TEST_ENTRY_ID: &str = "paint-probe-entry";

    struct TestDictationHistoryRow;

    impl gpui::Render for TestDictationHistoryRow {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let entry = crate::dictation::DictationHistoryEntry {
                id: TEST_ENTRY_ID.to_string(),
                timestamp: "2026-07-18T12:00:00Z".to_string(),
                transcript: "A rendered dictation history row".to_string(),
                preview: "A rendered dictation history row".to_string(),
                target: "Agent Chat".to_string(),
                audio_duration_ms: 1_250,
            };
            let colors = ListItemColors::from_theme(&crate::theme::Theme::default());
            let main_menu_theme = crate::designs::MainMenuThemeVariant::default();
            let metrics =
                crate::list_item::ListItemMetricsOverride::from_main_menu_theme(main_menu_theme);
            let layout = dictation_history_row_layout(
                crate::window_resize::MAIN_WINDOW_WIDTH * 0.5,
                metrics.row_outer_padding_x,
                0.0,
            );

            render_dictation_history_row(0, &entry, colors, main_menu_theme, layout, true, false)
        }
    }

    struct TestDictationHistorySlotAnatomy {
        scroll_handle: gpui::ScrollHandle,
    }

    impl gpui::Render for TestDictationHistorySlotAnatomy {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let entry = crate::dictation::DictationHistoryEntry {
                id: TEST_ENTRY_ID.to_string(),
                timestamp: "2026-07-18T12:00:00Z".to_string(),
                transcript: "A rendered dictation history row".to_string(),
                preview: "A rendered dictation history row".to_string(),
                target: "Agent Chat".to_string(),
                audio_duration_ms: 1_250,
            };
            let colors = ListItemColors::from_theme(&crate::theme::Theme::default());
            let main_menu_theme = crate::designs::MainMenuThemeVariant::default();
            let spacing = get_tokens(crate::designs::DesignVariant::default()).spacing();
            let chrome_edge_inset = crate::window_resize::layout::WINDOW_BORDER_Y * 0.5;
            let split_width = crate::window_resize::MAIN_WINDOW_WIDTH * 0.5;
            let ratified_offset_x = 4.0;
            let layout = dictation_history_row_layout(
                split_width,
                chrome_edge_inset + ratified_offset_x,
                chrome_edge_inset,
            );
            let results = render_dictation_history_results_list(
                "dictation-history-test-list",
                &self.scroll_handle,
                std::iter::once(render_dictation_history_row(
                    0,
                    &entry,
                    colors,
                    main_menu_theme,
                    layout,
                    true,
                    false,
                )),
                layout,
            );
            let list_pane = div()
                .relative()
                .w_full()
                .h_full()
                .min_h(px(0.0))
                .flex()
                .flex_col()
                .child(render_dictation_history_leading_slot(
                    "Transcripts",
                    colors,
                    main_menu_theme,
                    spacing.padding_xs,
                    chrome_edge_inset,
                ))
                .child(div().relative().flex_1().min_h(px(0.0)).child(results));

            crate::render_builtin_split_main_content_layout(
                list_pane.into_any_element(),
                div().w_full().h_full().into_any_element(),
            )
        }
    }

    #[gpui::test]
    fn rendered_dictation_row_records_non_zero_paint_bounds(cx: &mut gpui::TestAppContext) {
        use gpui::px;

        let window = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(480.0), px(120.0)),
            )));
            cx.open_window(options, |_, cx| cx.new(|_| TestDictationHistoryRow))
                .expect("dictation paint test window should open")
        });

        cx.run_until_parked();

        let expected_selector = dictation_history_row_selector(TEST_ENTRY_ID);
        window
            .update(cx, |_, window, _| {
                let measurement = window
                    .debug_bounds_entries()
                    .iter()
                    .find(|entry| entry.selector == expected_selector)
                    .expect("dictation history row wrapper should record paint bounds");

                assert!(measurement.bounds.size.width > px(0.0));
                assert!(measurement.bounds.size.height > px(0.0));
            })
            .expect("dictation paint test window should remain available");
    }

    #[gpui::test]
    fn dictation_split_portal_row_visible_bounds_match_shared_slot_geometry(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::px;

        let split_width = crate::window_resize::MAIN_WINDOW_WIDTH * 0.5;
        let ratified_offset_x = px(4.0);

        let window = cx.update(|cx| {
            gpui_component::init(cx);
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(crate::window_resize::MAIN_WINDOW_WIDTH), px(160.0)),
            )));
            cx.open_window(options, |_, cx| {
                cx.new(|_| TestDictationHistorySlotAnatomy {
                    scroll_handle: gpui::ScrollHandle::default(),
                })
            })
            .expect("dictation slot geometry test window should open")
        });

        cx.run_until_parked();

        let expected_row_selector = dictation_history_row_selector(TEST_ENTRY_ID);
        window
            .update(cx, |_, window, _| {
                let row = window
                    .debug_bounds_entries()
                    .iter()
                    .find(|entry| entry.selector == expected_row_selector)
                    .expect("dictation first row wrapper should record paint bounds");

                assert_eq!(row.bounds.origin.x, ratified_offset_x);
                assert_eq!(row.bounds.size.width, px(split_width));
                assert_eq!(row.visible_bounds.origin.x, ratified_offset_x);
                assert_eq!(
                    row.visible_bounds.size.width,
                    px(split_width),
                    "the production portal/scroll masks must preserve the ratified 379px right edge",
                );
                assert_eq!(row.visible_bounds, row.bounds);
                assert_eq!(row.bounds.right(), ratified_offset_x + px(split_width));
            })
            .expect("dictation slot geometry test window should remain available");
    }
}
