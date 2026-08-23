impl ScriptListApp {
    pub(crate) fn open_standalone_notes_browse(
        &mut self,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        crate::notes::init_notes_db()?;
        let query = String::new();
        self.open_builtin_filterable_view_with_filter(
            AppView::NotesBrowseView {
                search: crate::notes::search_model::NoteSearchHostState::load(
                    query.clone(),
                    crate::notes::search_model::NoteSearchDestination::OpenInNotesWindow,
                    &crate::notes::notes_brain_days_dir(),
                ),
            },
            &query,
            "Search notes...",
            true,
            cx,
        );
        Ok(())
    }

    pub(crate) fn notes_browse_visible_rows(
        search: &crate::notes::search_model::NoteSearchHostState,
    ) -> Vec<crate::notes::search_model::NoteSearchRow> {
        search.state.rows().to_vec()
    }

    fn notes_browse_selected_visible_row(
        search: &crate::notes::search_model::NoteSearchHostState,
    ) -> Option<crate::notes::search_model::NoteSearchRow> {
        search.selected_row().cloned()
    }

    fn notes_browse_dataset_and_visible_counts(
        search: &crate::notes::search_model::NoteSearchHostState,
    ) -> (usize, usize) {
        let visible_count = search.state.rows().len();
        let dataset_count = search
            .state
            .snapshot()
            .map(|snapshot| snapshot.total_count)
            .unwrap_or(visible_count);
        (dataset_count, visible_count)
    }

    pub(crate) fn notes_browse_visible_row_labels(
        search: &crate::notes::search_model::NoteSearchHostState,
    ) -> Vec<String> {
        search
            .state
            .rows()
            .iter()
            .map(|row| row.title.clone())
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
        row: &crate::notes::search_model::NoteSearchRow,
    ) -> Result<
        crate::ai::message_parts::AiContextPart,
        Box<crate::ai::reliability::AppFailureRecord>,
    > {
        let document = crate::notes::search_model::load_note_search_document(
            row.id,
            &crate::notes::notes_brain_days_dir(),
        )?;
        let stable_id = row.stable_id();
        let target = crate::ai::TabAiTargetContext {
            source: "NotesBrowse".to_string(),
            kind: row.kind.as_str().to_string(),
            semantic_id: crate::protocol::generate_semantic_id_named(row.kind.as_str(), &stable_id),
            label: document.title.clone(),
            metadata: Some(serde_json::json!({
                "documentId": stable_id,
                "documentKind": row.kind.as_str(),
                "title": document.title,
                "content": document.content,
                "preview": Self::notes_browse_preview(&document.content),
                "isPinned": document.pinned,
                "updatedAt": document.updated_at.to_rfc3339(),
            })),
        };
        let label = crate::ai::format_explicit_target_chip_label(&target);
        Ok(crate::ai::message_parts::AiContextPart::FocusedTarget { target, label })
    }

    fn activate_notes_browse_row(
        &mut self,
        row: &crate::notes::search_model::NoteSearchRow,
        cx: &mut Context<Self>,
    ) -> bool {
        let destination = match &self.current_view {
            AppView::NotesBrowseView { search } => search.destination,
            _ => return false,
        };
        match destination {
            crate::notes::search_model::NoteSearchDestination::AttachNote => {
                match self.build_notes_browse_portal_part(row) {
                    Ok(part) => {
                        self.close_attachment_portal_with_part(part, cx);
                        true
                    }
                    Err(failure) => {
                        self.show_error_toast(failure.primary_message().to_string(), cx);
                        false
                    }
                }
            }
            crate::notes::search_model::NoteSearchDestination::OpenInNotesWindow => {
                let result = match row.id {
                    crate::notes::search_model::NoteSearchDocumentId::Note(note_id) => {
                        crate::notes::open_note_in_notes_window(cx, note_id)
                    }
                    crate::notes::search_model::NoteSearchDocumentId::Day(date) => {
                        crate::notes::open_day_note_in_notes_window(cx, date)
                    }
                };
                match result {
                    Ok(()) => {
                        self.close_and_reset_window(cx);
                        true
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "script_kit::notes",
                            error = %error,
                            destination = destination.as_str(),
                            "notes_browse_open_in_notes_window_failed"
                        );
                        self.show_error_toast(
                            "The selected note couldn’t be opened.".to_string(),
                            cx,
                        );
                        false
                    }
                }
            }
            crate::notes::search_model::NoteSearchDestination::OpenInNotes
            | crate::notes::search_model::NoteSearchDestination::OpenHere => false,
        }
    }

    fn render_notes_browse_portal(
        &mut self,
        search: crate::notes::search_model::NoteSearchHostState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filter = search.query.clone();
        let selected_index = search.selected_index();
        use gpui_component::scroll::ScrollableElement as _;

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("notes_browse", false),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();

        let chrome = theme::AppChromeColors::from_theme(&self.theme);

        let filtered_notes = search.state.rows().to_vec();
        let filtered_len = filtered_notes.len();
        let row_keys: Vec<String> = filtered_notes
            .iter()
            .map(crate::notes::search_model::NoteSearchRow::semantic_id)
            .collect();
        let selected_index = crate::reconcile_dynamic_tracked_list_on_render(
            self.tracked_builtin_list_states
                .entry("notes_browse")
                .or_default(),
            &filter,
            &row_keys,
            selected_index,
            &self.notes_browse_scroll_handle,
        );
        if let AppView::NotesBrowseView { search } = &mut self.current_view {
            search.select_index(selected_index);
        }
        let total_notes = search
            .state
            .snapshot()
            .map(|snapshot| snapshot.total_count)
            .unwrap_or(filtered_len);
        let preview_note = filtered_notes.get(selected_index).cloned();
        let destination = search.destination;
        let in_portal =
            destination == crate::notes::search_model::NoteSearchDestination::AttachNote;

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

                let Some((current_selected, notes)) = (match &this.current_view {
                    AppView::NotesBrowseView { search } => {
                        Some((search.selected_index(), search.state.rows().to_vec()))
                    }
                    _ => None,
                }) else {
                    return;
                };
                let note_count = notes.len();

                if crate::ui_foundation::is_key_up(key) {
                    if note_count > 0 {
                        let next = current_selected.saturating_sub(1);
                        if let AppView::NotesBrowseView { search } = &mut this.current_view {
                            search.select_index(next);
                        }
                        this.notes_browse_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_down(key) {
                    if note_count > 0 {
                        let next = current_selected
                            .saturating_add(1)
                            .min(note_count.saturating_sub(1));
                        if let AppView::NotesBrowseView { search } = &mut this.current_view {
                            search.select_index(next);
                        }
                        this.notes_browse_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_enter(key) {
                    if let Some(note) = notes.get(current_selected) {
                        this.activate_notes_browse_row(note, cx);
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        let list_colors = ListItemColors::from_theme(&self.theme);
        let list_element: AnyElement = if filtered_notes.is_empty() {
            let state = match &search.state {
                crate::notes::search_model::NoteSearchState::Loading { .. } => {
                    crate::components::InfoStateSpec::new("notes-browse-loading")
                        .layout(crate::components::InfoStateLayout::InlineRow)
                        .density(crate::components::InfoStateDensity::Compact)
                        .tone(crate::components::InfoStateTone::Help)
                        .body("Loading notes…")
                }
                crate::notes::search_model::NoteSearchState::Failed { .. } => {
                    crate::components::InfoStateSpec::new("notes-browse-failed")
                        .layout(crate::components::InfoStateLayout::InlineRow)
                        .density(crate::components::InfoStateDensity::Compact)
                        .tone(crate::components::InfoStateTone::Recovery)
                        .title("Notes couldn’t be loaded")
                        .body("Notes search is unavailable. Try again.")
                        .footer_note("This is an error, not an empty result.")
                }
                _ => crate::components::shared_empty_state_spec(
                    crate::components::InfoEmptySurface::NotesBrowse,
                    &filter,
                ),
            };
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .px(px(design_spacing.padding_md))
                .font_family(design_typography.font_family)
                .child(crate::components::render_info_state(state, &self.theme, cx))
                .into_any_element()
        } else {
            let notes_for_closure = filtered_notes.clone();
            let selected = selected_index;
            let hovered = self.hovered_index;
            let entity = cx.entity().downgrade();

            crate::components::scrollbar::render_tracked_scroll_column(
                "notes-browse-list",
                &self.notes_browse_scroll_handle,
                notes_for_closure
                    .into_iter()
                    .enumerate()
                    .map(move |(display_ix, note)| {
                        let is_selected = display_ix == selected;
                        let note_id = note.stable_id();
                        let title = note.title.clone();
                        let description = note.search_description();
                        let semantic_id = note.semantic_id();

                        let item = ListItem::new(title, list_colors)
                            .description_opt(Some(description))
                            .selected(is_selected)
                            .hovered(hovered == Some(display_ix))
                            .semantic_id(semantic_id);

                        let click_entity = entity.clone();
                        let move_entity = entity.clone();
                        let hover_entity = entity.clone();
                        let activation_row = note.clone();
                        div()
                            .id(format!("notes-browse-row:{note_id}"))
                            .cursor_pointer()
                            .on_click(move |event, _window, cx| {
                                if let Some(app) = click_entity.upgrade() {
                                    app.update(cx, |this, cx| {
                                        let should_submit = if let AppView::NotesBrowseView {
                                            search,
                                        } = &mut this.current_view
                                        {
                                            let was_selected =
                                                search.selected_id == Some(activation_row.id);
                                            search.select_index(display_ix);
                                            crate::ui_foundation::should_submit_selected_row_click(
                                                was_selected,
                                                event.click_count(),
                                            )
                                        } else {
                                            false
                                        };

                                        this.notes_browse_scroll_handle.scroll_to_item(display_ix);
                                        this.note_list_pointer_click(display_ix, cx);

                                        if should_submit {
                                            this.activate_notes_browse_row(&activation_row, cx);
                                        }
                                    });
                                }
                                cx.stop_propagation();
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

        let preview_panel: AnyElement = match preview_note {
            Some(note) => {
                let title = note.title.clone();

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
                            .child(if note.preview.trim().is_empty() {
                                "Empty note".to_string()
                            } else {
                                note.preview
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

        let failure_notice = search
            .state
            .failure()
            .filter(|_| !filtered_notes.is_empty())
            .map(|_failure| {
                crate::components::render_info_state(
                    crate::components::InfoStateSpec::new("notes-browse-stale-results")
                        .layout(crate::components::InfoStateLayout::InlineRow)
                        .density(crate::components::InfoStateDensity::Compact)
                        .tone(crate::components::InfoStateTone::Recovery)
                        .body("Notes couldn’t be refreshed. Showing previous results."),
                    &self.theme,
                    cx,
                )
            });

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
            .when_some(failure_notice, |pane, notice| pane.child(notice))
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

        let hints: Vec<SharedString> = vec![
            format!("↵ {}", destination.primary_verb()).into(),
            if in_portal {
                "Esc Cancel".into()
            } else {
                "Esc Back".into()
            },
        ];
        crate::components::emit_prompt_hint_audit("notes_browse", &hints);

        let gpui_footer = crate::components::render_simple_hint_strip(hints, None);
        let footer = self.main_window_footer_slot(gpui_footer);
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let count_label = if filtered_len == total_notes {
            format!(
                "{} note{}",
                total_notes,
                if total_notes == 1 { "" } else { "s" }
            )
        } else {
            format!("{} of {} notes", filtered_len, total_notes)
        };
        let main =
            self.render_builtin_split_main_content(list_pane.into_any_element(), preview_panel);

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
