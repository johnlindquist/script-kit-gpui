impl ScriptListApp {
    fn notes_browse_display_title(note: &crate::notes::Note) -> String {
        if note.title.trim().is_empty() {
            "Untitled Note".to_string()
        } else {
            note.title.clone()
        }
    }

    fn notes_browse_filtered_notes(filter: &str) -> Vec<crate::notes::Note> {
        let result = if filter.trim().is_empty() {
            crate::notes::get_all_notes()
        } else {
            crate::notes::search_notes(filter)
        };

        match result {
            Ok(notes) => notes,
            Err(error) => {
                tracing::warn!(
                    event = "notes_browse_portal_load_failed",
                    filter = %filter,
                    error = %error,
                );
                Vec::new()
            }
        }
    }

    pub(crate) fn notes_browse_visible_rows(filter: &str) -> Vec<crate::notes::Note> {
        Self::notes_browse_filtered_notes(filter)
    }

    fn notes_browse_selected_visible_row(
        filter: &str,
        selected_index: usize,
    ) -> Option<crate::notes::Note> {
        Self::notes_browse_visible_rows(filter)
            .get(selected_index)
            .cloned()
    }

    fn notes_browse_dataset_and_visible_counts(filter: &str) -> (usize, usize) {
        (
            Self::notes_browse_filtered_notes("").len(),
            Self::notes_browse_visible_rows(filter).len(),
        )
    }

    pub(crate) fn notes_browse_visible_row_labels(filter: &str) -> Vec<String> {
        Self::notes_browse_visible_rows(filter)
            .into_iter()
            .map(|note| Self::notes_browse_display_title(&note))
            .collect()
    }

    fn notes_browse_preview(content: &str) -> String {
        const LIMIT: usize = 280;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return "Empty note".to_string();
        }
        let mut preview: String = trimmed.chars().take(LIMIT).collect();
        if trimmed.chars().count() > LIMIT {
            preview.push('…');
        }
        preview
    }

    fn build_notes_browse_portal_part(
        &self,
        note: &crate::notes::Note,
    ) -> crate::ai::message_parts::AiContextPart {
        let title = Self::notes_browse_display_title(note);
        let note_id = note.id.as_str();
        let target = crate::ai::TabAiTargetContext {
            source: "NotesBrowse".to_string(),
            kind: "note".to_string(),
            semantic_id: crate::protocol::generate_semantic_id_named("note", &note_id),
            label: title.clone(),
            metadata: Some(serde_json::json!({
                "noteId": note_id,
                "title": title,
                "content": note.content,
                "preview": Self::notes_browse_preview(&note.content),
                "isPinned": note.is_pinned,
                "createdAt": note.created_at.to_rfc3339(),
                "updatedAt": note.updated_at.to_rfc3339(),
            })),
        };
        let label = crate::ai::format_explicit_target_chip_label(&target);
        crate::ai::message_parts::AiContextPart::FocusedTarget { target, label }
    }

    fn render_notes_browse_portal(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use gpui_component::scroll::ScrollableElement as _;

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("notes_browse", false),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();

        let chrome = theme::AppChromeColors::from_theme(&self.theme);

        let filtered_notes = Self::notes_browse_filtered_notes(&filter);
        let filtered_len = filtered_notes.len();
        let selected_index = if let Some(reanchored) =
            Self::builtin_reanchor_selection_from_scroll_handle(
                selected_index,
                &self.notes_browse_scroll_handle,
                filtered_len,
            ) {
            tracing::info!(
                target: "script_kit::scroll",
                event = "builtin_selection_resynced_from_scrollbar",
                view = "notes_browse",
                reason = "render",
                selected_before = selected_index,
                selected_after = reanchored,
            );
            if let AppView::NotesBrowseView { selected_index, .. } = &mut self.current_view {
                *selected_index = reanchored;
            }
            reanchored
        } else {
            selected_index
        };
        let total_notes = filtered_len;
        let preview_note = filtered_notes.get(selected_index).cloned();
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
                    if this.is_in_attachment_portal() {
                        this.close_attachment_portal_cancel(cx);
                    } else if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }

                if has_cmd && key.eq_ignore_ascii_case("w") {
                    if this.is_in_attachment_portal() {
                        this.close_attachment_portal_cancel(cx);
                    }
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                let Some((current_filter, current_selected)) = (match &this.current_view {
                    AppView::NotesBrowseView {
                        filter,
                        selected_index,
                    } => Some((filter.clone(), *selected_index)),
                    _ => None,
                }) else {
                    return;
                };

                let notes = Self::notes_browse_filtered_notes(&current_filter);
                let note_count = notes.len();

                if crate::ui_foundation::is_key_up(key) {
                    if current_selected > 0 {
                        if let AppView::NotesBrowseView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected - 1;
                            this.notes_browse_scroll_handle
                                .scroll_to_item(*selected_index);
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_down(key) {
                    if current_selected < note_count.saturating_sub(1) {
                        if let AppView::NotesBrowseView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected + 1;
                            this.notes_browse_scroll_handle
                                .scroll_to_item(*selected_index);
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_enter(key) {
                    if let Some(note) = notes.get(current_selected) {
                        if this.is_in_attachment_portal() {
                            let part = this.build_notes_browse_portal_part(note);
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
        let list_element: AnyElement = if filtered_notes.is_empty() {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .px(px(design_spacing.padding_md))
                .font_family(design_typography.font_family)
                .child(crate::components::render_shared_empty_state(
                    crate::components::InfoEmptySurface::NotesBrowse,
                    &filter,
                    &self.theme,
                    cx,
                ))
                .into_any_element()
        } else {
            let notes_for_closure = filtered_notes.clone();
            let selected = selected_index;
            let entity = cx.entity().downgrade();

            crate::components::scrollbar::render_tracked_scroll_column(
                "notes-browse-list",
                &self.notes_browse_scroll_handle,
                notes_for_closure.into_iter().enumerate().map(
                    move |(display_ix, note)| {
                        let is_selected = display_ix == selected;
                        let title = Self::notes_browse_display_title(&note);
                        let description = format!(
                            "{} · {} chars{}",
                            crate::formatting::format_absolute_datetime(note.updated_at),
                            note.content.chars().count(),
                            if note.is_pinned { " · pinned" } else { "" }
                        );

                        let item = ListItem::new(title, list_colors)
                            .description_opt(Some(description))
                            .selected(is_selected);

                        let entity = entity.clone();
                        div()
                            .id(gpui::ElementId::Integer(display_ix as u64))
                            .cursor_pointer()
                            .on_click(move |event, _window, cx| {
                                if let Some(app) = entity.upgrade() {
                                    app.update(cx, |this, cx| {
                                        let should_submit = if let AppView::NotesBrowseView {
                                            selected_index,
                                            ..
                                        } = &mut this.current_view
                                        {
                                            let was_selected = *selected_index == display_ix;
                                            *selected_index = display_ix;
                                            crate::ui_foundation::should_submit_selected_row_click(
                                                was_selected,
                                                event.click_count(),
                                            )
                                        } else {
                                            false
                                        };

                                        this.notes_browse_scroll_handle.scroll_to_item(display_ix);

                                        if should_submit && this.is_in_attachment_portal() {
                                            let notes = Self::notes_browse_filtered_notes(
                                                this.filter_text(),
                                            );
                                            if let Some(note) = notes.get(display_ix) {
                                                let part =
                                                    this.build_notes_browse_portal_part(note);
                                                this.close_attachment_portal_with_part(part, cx);
                                            }
                                        }

                                        cx.notify();
                                    });
                                }
                                cx.stop_propagation();
                            })
                            .child(item)
                    },
                ),
            )
        };

        let preview_panel: AnyElement = match preview_note {
            Some(note) => {
                let title = Self::notes_browse_display_title(&note);

                div()
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
                            .pb(px(design_spacing.padding_md))
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(rgba(chrome.text_hint_rgba))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(crate::formatting::format_absolute_datetime(
                                        note.updated_at,
                                    )),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_sm()
                                    .text_color(rgba(chrome.text_strong_rgba))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .text_sm()
                            .text_color(rgba(chrome.text_muted_rgba))
                            .child(if note.content.trim().is_empty() {
                                "Empty note".to_string()
                            } else {
                                note.content
                            }),
                    )
                    .into_any_element()
            }
            None => div()
                .w_full()
                .h_full()
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgba(chrome.text_hint_rgba))
                .font_family(design_typography.font_family)
                .child("No note selected")
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
                    let view_state = if let AppView::NotesBrowseView {
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

                    let filtered_notes = Self::notes_browse_filtered_notes(&current_filter);
                    let filtered_len = filtered_notes.len();

                    let Some(new_selected) = this.builtin_scroll_target_from_wheel(
                        event,
                        current_selected,
                        filtered_len,
                    ) else {
                        if filtered_len > 0 {
                            cx.stop_propagation();
                        }
                        return;
                    };

                    if let AppView::NotesBrowseView { selected_index, .. } = &mut this.current_view
                    {
                        *selected_index = new_selected;
                    }

                    this.notes_browse_scroll_handle.scroll_to_item(new_selected);
                    Self::log_builtin_scroll_event(
                        "notes_browse",
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
                        "Notes"
                    } else {
                        "Results"
                    },
                    None,
                    list_colors,
                    true,
                ),
            )
            .child(div().relative().flex_1().min_h(px(0.)).child(list_element));

        let hints: Vec<SharedString> = if in_portal {
            vec!["↵ Attach Note".into(), "Esc Cancel".into()]
        } else {
            vec!["Esc Back".into()]
        };
        crate::components::emit_prompt_hint_audit("notes_browse", &hints);

        let gpui_footer = crate::components::render_simple_hint_strip(hints, None);
        let footer = self.main_window_footer_slot(gpui_footer);
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let count_label = format!(
            "{} note{}",
            total_notes,
            if total_notes == 1 { "" } else { "s" }
        );
        let main = self.render_builtin_split_main_content(
            list_pane.into_any_element(),
            preview_panel,
        );

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(chrome.text_primary_hex))
                .font_family(self.theme_font_family())
                .key_context("notes_browse")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(vec![
                    self.render_builtin_main_input_count_label(count_label),
                ], cx),
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
