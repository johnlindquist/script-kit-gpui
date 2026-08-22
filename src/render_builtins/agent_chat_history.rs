#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatHistoryEmptyState {
    NoConversationHistory,
    NoFilteredMatches,
}

impl AgentChatHistoryEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoConversationHistory
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoConversationHistory => "No conversation history",
            Self::NoFilteredMatches => "No conversations match your filter",
        }
    }
}

impl ScriptListApp {
    fn agent_chat_history_visible_rows(
        filter: &str,
    ) -> Vec<crate::ai::agent_chat::ui::history::AgentChatHistoryEntry> {
        crate::ai::agent_chat::ui::history::search_history(filter, 100)
            .into_iter()
            .map(|hit| hit.entry)
            .collect()
    }

    fn agent_chat_history_selected_visible_row(
        filter: &str,
        selected_index: usize,
    ) -> Option<crate::ai::agent_chat::ui::history::AgentChatHistoryEntry> {
        Self::agent_chat_history_visible_rows(filter)
            .get(selected_index)
            .cloned()
    }

    fn agent_chat_history_dataset_and_visible_counts(filter: &str) -> (usize, usize) {
        (
            crate::ai::agent_chat::ui::history::load_history().len(),
            Self::agent_chat_history_visible_rows(filter).len(),
        )
    }

    fn agent_chat_history_visible_row_labels(filter: &str) -> Vec<String> {
        Self::agent_chat_history_visible_rows(filter)
            .into_iter()
            .map(|entry| entry.title_display().to_string())
            .collect()
    }

    /// Render the Agent Chat conversation history browser (list + preview).
    fn render_agent_chat_history(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use gpui_component::scroll::ScrollableElement as _;

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("agent_chat_history", false),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let _design_visual = tokens.visual();

        let text_primary = self.theme.colors.text.primary;
        let text_dimmed = self.theme.colors.text.dimmed;
        let text_muted = self.theme.colors.text.muted;

        // Load history entries via the shared search model (small JSONL, fast).
        let all_entries = crate::ai::agent_chat::ui::history::load_history();
        let hits = crate::ai::agent_chat::ui::history::search_history(&filter, 100);
        let filtered_entries: Vec<crate::ai::agent_chat::ui::history::AgentChatHistoryEntry> =
            hits.into_iter().map(|h| h.entry).collect();
        let filtered_len = filtered_entries.len();
        let row_keys: Vec<String> = filtered_entries
            .iter()
            .map(|entry| entry.session_id.clone())
            .collect();
        let selected_index = crate::reconcile_dynamic_tracked_list_on_render(
            self.tracked_builtin_list_states
                .entry("agent_chat_history")
                .or_default(),
            &filter,
            &row_keys,
            selected_index,
            &self.agent_chat_history_scroll_handle,
        );
        if let AppView::AgentChatHistoryView {
            selected_index: current_selected,
            ..
        } = &mut self.current_view
        {
            *current_selected = selected_index;
        }
        // Load preview for selected entry
        let selected_session_id = filtered_entries
            .get(selected_index)
            .map(|e| e.session_id.clone());
        let preview_conversation = selected_session_id
            .as_deref()
            .and_then(crate::ai::agent_chat::ui::history::load_conversation);
        let in_portal = self.is_in_attachment_portal();

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
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;

                // Route keys to actions dialog first if open
                match this.route_key_to_actions_dialog(
                    key,
                    event.keystroke.key_char.as_deref(),
                    modifiers,
                    ActionsDialogHost::AgentChatHistory,
                    window,
                    cx,
                ) {
                    ActionsRoute::NotHandled => {}
                    ActionsRoute::Handled => return,
                    ActionsRoute::Execute {
                        action_id,
                        should_close,
                    } => {
                        if should_close {
                            this.close_actions_popup(ActionsDialogHost::AgentChatHistory, window, cx);
                        }
                        this.handle_action(action_id, window, cx);
                        return;
                    }
                }

                // ESC: Clear filter first if present, otherwise go back/close
                if is_key_escape(key) && !this.show_actions_popup {
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

                // Cmd+W always closes window
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                // Extract current view state
                let view_state = if let AppView::AgentChatHistoryView {
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

                // Recompute filtered list via the shared search model.
                let hits = crate::ai::agent_chat::ui::history::search_history(&current_filter, 100);
                let filtered: Vec<crate::ai::agent_chat::ui::history::AgentChatHistoryEntry> =
                    hits.into_iter().map(|h| h.entry).collect();
                let current_filtered_len = filtered.len();

                if is_key_up(key) {
                    if current_filtered_len > 0 {
                        let next = current_selected.saturating_sub(1);
                        if let AppView::AgentChatHistoryView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = next;
                        }
                        this.agent_chat_history_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_down(key) {
                    if current_filtered_len > 0 {
                        let next = current_selected
                            .saturating_add(1)
                            .min(current_filtered_len.saturating_sub(1));
                        if let AppView::AgentChatHistoryView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = next;
                        }
                        this.agent_chat_history_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if has_cmd && is_key_enter(key) {
                    if let Some(entry) = filtered.get(current_selected) {
                        if this.is_in_attachment_portal() {
                            match crate::ai::agent_chat::ui::history_attachment::write_history_attachment(
                                &entry.session_id,
                                crate::ai::agent_chat::ui::history_attachment::AgentChatHistoryAttachMode::Summary,
                            ) {
                                Ok((path, label)) => {
                                    this.close_attachment_portal_with_part(
                                        crate::ai::message_parts::AiContextPart::FilePath {
                                            path: path.to_string_lossy().to_string(),
                                            label,
                                        },
                                        cx,
                                    );
                                }
                                Err(error) => {
                                    tracing::warn!(
                                        event = "agent_chat_history_portal_summary_attach_failed",
                                        session_id = %entry.session_id,
                                        error = %error,
                                    );
                                }
                            }
                        } else if let Err(error) = this.attach_agent_chat_history_to_chat_from_browser(
                            &entry.session_id,
                            window,
                            cx,
                        ) {
                            tracing::warn!(
                                event = "agent_chat_history_browser_attach_failed",
                                session_id = %entry.session_id,
                                error = %error,
                            );
                        }
                    }
                    cx.stop_propagation();
                } else if is_key_enter(key) {
                    if let Some(entry) = filtered.get(current_selected) {
                        if this.is_in_attachment_portal() {
                            match crate::ai::agent_chat::ui::history_attachment::write_history_attachment(
                                &entry.session_id,
                                crate::ai::agent_chat::ui::history_attachment::AgentChatHistoryAttachMode::Transcript,
                            ) {
                                Ok((path, label)) => {
                                    this.close_attachment_portal_with_part(
                                        crate::ai::message_parts::AiContextPart::FilePath {
                                            path: path.to_string_lossy().to_string(),
                                            label,
                                        },
                                        cx,
                                    );
                                }
                                Err(error) => {
                                    tracing::warn!(
                                        event = "agent_chat_history_portal_transcript_attach_failed",
                                        session_id = %entry.session_id,
                                        error = %error,
                                    );
                                }
                            }
                        } else {
                            let session_id = entry.session_id.clone();
                            let title = entry.title_display().to_string();
                            tracing::info!(
                                event = "agent_chat_history_browser_transcript_loaded",
                                session_id = %session_id,
                            );
                            this.resume_agent_chat_conversation_from_history(
                                &session_id,
                                &title,
                                cx,
                            );
                        }
                    }
                    cx.stop_propagation();
                } else if key.eq_ignore_ascii_case("backspace") && has_cmd {
                    // Cmd+Backspace is irreversible: capture the exact
                    // conversation, confirm in the owning window, then
                    // revalidate the still-selected session before mutation.
                    if let Some(entry) = filtered.get(current_selected) {
                        let session_id = entry.session_id.clone();
                        let conversation_title = entry.title_display().to_string();
                        let owner = cx.entity().downgrade();
                        let confirm_owner = owner.clone();
                        this.was_window_focused = true;

                        crate::confirm::open_parent_confirm_dialog_for_entity(
                            window,
                            cx,
                            owner,
                            crate::confirm::ParentConfirmOptions::destructive(
                                "Delete Conversation",
                                format!(
                                    "Permanently delete \"{conversation_title}\"? This cannot be undone."
                                ),
                                "Delete",
                            ),
                            move |_window, cx| {
                                let Some(entity) = confirm_owner.upgrade() else {
                                    return;
                                };

                                entity.update(cx, |this, cx| {
                                    let (filter, selected_index) = match &this.current_view {
                                        AppView::AgentChatHistoryView {
                                            filter,
                                            selected_index,
                                        } => (filter.clone(), *selected_index),
                                        _ => return,
                                    };
                                    let current_entries =
                                        crate::ai::agent_chat::ui::history::search_history(
                                            &filter,
                                            100,
                                        );
                                    let current_session = current_entries
                                        .get(selected_index)
                                        .map(|hit| hit.entry.session_id.clone());
                                    let decision = crate::window_orchestrator::interaction::
                                        plan_confirmed_destructive_action(
                                            true,
                                            &session_id,
                                            current_session.as_ref(),
                                        );
                                    if decision
                                        != crate::window_orchestrator::interaction::
                                            ConfirmedDestructiveActionDecision::ExecuteCapturedTarget
                                    {
                                        tracing::warn!(
                                            event = "agent_chat_history_delete_target_rejected",
                                            ?decision,
                                        );
                                        return;
                                    }

                                    if let Err(error) =
                                        crate::ai::agent_chat::ui::history::delete_conversation(
                                            &session_id,
                                        )
                                    {
                                        let safe_error = crate::logging::log_private_user_value(
                                            &error.to_string(),
                                        );
                                        tracing::warn!(
                                            event = "agent_chat_history_delete_failed",
                                            error_bytes = safe_error.raw_bytes,
                                            error_sha256 = %safe_error.sha256,
                                        );
                                        return;
                                    }

                                    let new_len = current_entries.len().saturating_sub(1);
                                    if let AppView::AgentChatHistoryView { selected_index, .. } =
                                        &mut this.current_view
                                    {
                                        if *selected_index >= new_len && new_len > 0 {
                                            *selected_index = new_len - 1;
                                        } else if new_len == 0 {
                                            *selected_index = 0;
                                        }
                                        this.agent_chat_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                    }
                                    cx.notify();
                                });
                            },
                            |_window, _cx| {
                                tracing::info!(event = "agent_chat_history_delete_cancelled");
                            },
                        );
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        // Build list
        let list_colors = ListItemColors::from_theme(&self.theme);

        let list_element: AnyElement = if filtered_len == 0 {
            let state = AgentChatHistoryEmptyState::from_filter(&filter);
            crate::components::render_simple_empty_state(
                "agent-chat-history-empty",
                state.message(),
                "message-circle",
                None,
                &self.theme,
                cx,
            )
        } else {
            let entries_for_closure: Vec<
                crate::ai::agent_chat::ui::history::AgentChatHistoryEntry,
            > = filtered_entries.clone();
            let selected = selected_index;
            let hovered = self.hovered_index;
            let entity = cx.entity().downgrade();

            crate::components::scrollbar::render_tracked_scroll_column(
                "agent_chat-history-list",
                &self.agent_chat_history_scroll_handle,
                entries_for_closure
                    .into_iter()
                    .enumerate()
                    .map(move |(display_ix, entry)| {
                        let is_selected = display_ix == selected;
                        let session_id = entry.session_id.clone();

                        let name = entry.title_display().to_string();
                        let description = format!(
                            "{} \u{00b7} {} messages \u{00b7} {}",
                            entry.preview_display(),
                            entry.message_count,
                            entry.timestamp,
                        );

                        let item = ListItem::new(name, list_colors)
                            .description_opt(Some(description))
                            .selected(is_selected)
                            .hovered(hovered == Some(display_ix))
                            .semantic_id(format!("agent-chat-history:{session_id}"));

                        let click_entity = entity.clone();
                        let move_entity = entity.clone();
                        let hover_entity = entity.clone();
                        div()
                            .id(format!("agent-chat-history-row:{session_id}"))
                            .cursor_pointer()
                            .on_click(move |_event, _window, cx| {
                                if let Some(app) = click_entity.upgrade() {
                                    app.update(cx, |this, cx| {
                                        if let AppView::AgentChatHistoryView {
                                            selected_index,
                                            ..
                                        } = &mut this.current_view
                                        {
                                            *selected_index = display_ix;
                                        }
                                        this.agent_chat_history_scroll_handle
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
                            .child(item)
                    }),
            )
        };

        // Build preview panel
        let preview_panel: AnyElement = match &preview_conversation {
            Some(conv) => {
                let mut blocks: Vec<AnyElement> = Vec::new();
                for msg in &conv.messages {
                    let role_color = if msg.role == "user" {
                        text_primary
                    } else {
                        text_dimmed
                    };
                    blocks.push(
                        div()
                            .w_full()
                            .pb(px(design_spacing.padding_md))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(text_muted))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(msg.role.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(role_color))
                                    .child(msg.body.clone()),
                            )
                            .into_any_element(),
                    );
                }
                div()
                    .w_full()
                    .h_full()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .font_family(design_typography.font_family)
                    .children(blocks)
                    .into_any_element()
            }
            None => div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(text_muted))
                .font_family(design_typography.font_family)
                .child("No conversation selected")
                .into_any_element(),
        };

        // List pane
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
                        "Chats"
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
            vec![
                "↵ Attach Transcript".into(),
                "⌘↵ Attach Summary".into(),
                "⌘⌫ Delete".into(),
                "Esc Cancel".into(),
            ]
        } else {
            vec![
                "↵ Load Transcript".into(),
                "⌘↵ Attach Summary".into(),
                "⌘⌫ Delete".into(),
                "Esc Back".into(),
            ]
        };
        crate::components::emit_prompt_hint_audit("agent_chat_history", &hints);

        let gpui_footer = crate::components::render_simple_hint_strip(hints, None);
        let footer = self.main_window_footer_slot(gpui_footer);
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let count_label = format!(
            "{} conversation{}",
            all_entries.len(),
            if all_entries.len() == 1 { "" } else { "s" }
        );
        let main =
            self.render_builtin_split_main_content(list_pane.into_any_element(), preview_panel);

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(self.theme_font_family())
                .key_context("agent_chat_history")
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

    /// Resume an Agent Chat conversation from history by opening Agent Chat chat with
    /// the saved messages loaded.
    pub(crate) fn resume_agent_chat_conversation_from_history(
        &mut self,
        session_id: &str,
        first_message: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(chat_entity) =
            crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()
        {
            let resumed = chat_entity.update(cx, |chat_view, cx| {
                chat_view.resume_from_history(session_id, cx)
            });
            if !resumed {
                let fallback_input = first_message.to_string();
                chat_entity.update(cx, |chat_view, cx| {
                    chat_view.set_input(fallback_input, cx);
                });
            }

            self.reset_to_script_list(cx);
            return;
        }

        self.open_tab_ai_agent_chat_with_entry_intent(None, cx);

        if let AppView::AgentChatView { entity } = &self.current_view {
            let resumed = entity.update(cx, |chat_view, cx| {
                chat_view.resume_from_history(session_id, cx)
            });
            if !resumed {
                entity.update(cx, |chat_view, cx| {
                    chat_view.set_input(first_message.to_string(), cx);
                });
            }
        }
    }

    /// Enter on a chat_turn-sourced Brain Inbox row: resume the saved
    /// conversation AND auto-submit the inbox follow-up prompt as the next
    /// turn (an empty prompt only loads the conversation).
    ///
    /// Differs from [`Self::resume_agent_chat_conversation_from_history`]
    /// (history-browser semantics, where `first_message` is only a fallback
    /// composer draft) in that the embedded chat is entered through the
    /// suppressing entry, so the focused launcher row is not staged as an
    /// `@cmd:` mention in the composer.
    pub(crate) fn resume_agent_chat_conversation_with_followup(
        &mut self,
        session_id: &str,
        followup_prompt: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(chat_entity) =
            crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()
        {
            chat_entity.update(cx, |chat_view, cx| {
                chat_view.resume_from_history_with_followup(session_id, followup_prompt, cx);
            });
            self.reset_to_script_list(cx);
            return;
        }

        self.open_clean_agent_chat(cx);

        if let AppView::AgentChatView { entity } = &self.current_view {
            let entity = entity.clone();
            entity.update(cx, |chat_view, cx| {
                chat_view.resume_from_history_with_followup(session_id, followup_prompt, cx);
            });
        }
    }

    /// Attach a history conversation summary as a context chip to the Agent Chat chat
    /// (Cmd+Enter in the browser). Opens Agent Chat if not already open.
    fn attach_agent_chat_history_to_chat_from_browser(
        &mut self,
        session_id: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.open_tab_ai_agent_chat_with_entry_intent(None, cx);

        if let AppView::AgentChatView { entity } = &self.current_view {
            let sid = session_id.to_string();
            entity.update(cx, |chat_view, cx| {
                if let Err(error) = chat_view.attach_history_session(
                    &sid,
                    crate::ai::agent_chat::ui::history_attachment::AgentChatHistoryAttachMode::Summary,
                    cx,
                ) {
                    tracing::warn!(
                        event = "agent_chat_history_browser_attach_write_failed",
                        session_id = %sid,
                        error = %error,
                    );
                }
            });
        }

        tracing::info!(
            event = "agent_chat_history_browser_attach_selected",
            session_id = %session_id,
        );
        Ok(())
    }
}

#[cfg(test)]
mod agent_chat_history_scroll_contract {
    const SOURCE: &str = include_str!("agent_chat_history.rs");

    #[test]
    fn agent_chat_history_tracks_scroll_and_keeps_selection_visible() {
        assert!(
            SOURCE.contains("render_tracked_scroll_column(")
                && SOURCE.contains("&self.agent_chat_history_scroll_handle"),
            "Agent Chat history list should use the shared tracked-scroll viewport"
        );
        assert!(
            SOURCE.contains("this.agent_chat_history_scroll_handle"),
            "Agent Chat history keyboard navigation should scroll the selected row into view"
        );
    }

    #[test]
    fn agent_chat_history_uses_shared_search_and_explicit_verbs() {
        assert!(
            SOURCE.contains("search_history(&filter, 100)")
                || SOURCE.contains("search_history(&current_filter, 100)"),
            "Agent Chat history browser must use the shared search_history function"
        );
        assert!(
            SOURCE.contains("\"↵ Load Transcript\""),
            "Agent Chat history Enter hint must say 'Load Transcript'"
        );
        assert!(
            SOURCE.contains("\"⌘↵ Attach Summary\""),
            "Agent Chat history Cmd+Enter hint must say 'Attach Summary'"
        );
    }
}
