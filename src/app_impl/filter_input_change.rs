use super::*;
use crate::filter_input_core::ScriptListSpecialEntry;

impl ScriptListApp {
    pub(crate) fn route_script_list_special_entry(
        &mut self,
        entry: ScriptListSpecialEntry,
        raw_filter_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let entry_kind = match &entry {
            ScriptListSpecialEntry::FileSearchMini { .. } => "file_search_mini",
            ScriptListSpecialEntry::QuickTerminal => "quick_terminal",
            ScriptListSpecialEntry::ActionsHelp => "actions_help",
            ScriptListSpecialEntry::DayPage => "day_page",
        };
        let raw_filter_text_safe = logging::log_user_value(raw_filter_text);

        tracing::info!(
            target: "script_kit::do_in_trace",
            event = "DO_IN_TRACE script_list_special_entry_routed",
            filter_text_preview = %raw_filter_text_safe,
            filter_text_bytes = raw_filter_text_safe.raw_bytes,
            filter_text_safe_bytes = raw_filter_text_safe.safe_bytes,
            filter_text_truncated = raw_filter_text_safe.truncated,
            entry_kind,
            current_view = ?self.current_view,
            "DO_IN_TRACE script_list_special_entry_routed"
        );
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "script_list_special_entry_routed",
            filter_text = %raw_filter_text,
            entry_kind,
            current_view = ?self.current_view,
        );

        match entry {
            ScriptListSpecialEntry::FileSearchMini { query } => {
                self.open_file_search_view(query, FileSearchPresentation::Mini, cx);
            }
            ScriptListSpecialEntry::QuickTerminal => {
                self.open_quick_terminal(None, cx);
            }
            ScriptListSpecialEntry::ActionsHelp => {
                if self.has_actions() {
                    self.toggle_actions(cx, window);
                }
            }
            ScriptListSpecialEntry::DayPage => {
                // A bare space is a launch trigger, not content: clear it before
                // the swap so the Day Page editor starts clean and ScriptList
                // has no stray space on return.
                self.set_filter_text_immediate(String::new(), window, cx);
                self.show_day_page_view(window, cx);
            }
        }

        true
    }

    pub(crate) fn handle_filter_input_change(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let handler_start = std::time::Instant::now();

        if self.suppress_filter_events {
            tracing::info!(
                target: "script_kit::do_in_trace",
                event = "DO_IN_TRACE filter_change.suppressed",
                current_view = ?self.current_view,
                canonical_filter = %self.filter_text,
                "DO_IN_TRACE filter_change.suppressed"
            );
            return;
        }

        let new_text = self.gpui_input_state.read(cx).value().to_string();
        let shared_filter_view = self.current_view_uses_shared_filter_input();

        if self
            .pending_programmatic_filter_echo
            .as_deref()
            .is_some_and(|pending| pending == new_text)
            && new_text == self.filter_text
        {
            self.pending_programmatic_filter_echo = None;
            tracing::info!(
                target: "script_kit::input_history",
                event = "programmatic_filter_echo_suppressed",
                filter_len = new_text.len(),
                current_view = ?self.current_view,
            );
            return;
        }

        if self.pending_programmatic_filter_echo.is_some() {
            self.pending_programmatic_filter_echo = None;
        }
        self.menu_syntax_form_input_active = false;
        self.menu_syntax_form_draft_field_id = None;
        self.menu_syntax_form_draft_value.clear();
        self.cancel_history_filter_render_pending_if_obsolete(&new_text);
        let new_text_safe = logging::log_user_value(&new_text);
        let canonical_filter_safe = logging::log_user_value(&self.filter_text);

        if logging::filter_perf_trace_enabled() {
            tracing::info!(
                target: "script_kit::do_in_trace",
                event = "DO_IN_TRACE filter_change.entry",
                current_view = ?self.current_view,
                new_text_preview = %new_text_safe,
                new_text_bytes = new_text_safe.raw_bytes,
                new_text_safe_bytes = new_text_safe.safe_bytes,
                new_text_truncated = new_text_safe.truncated,
                canonical_filter_preview = %canonical_filter_safe,
                canonical_filter_bytes = canonical_filter_safe.raw_bytes,
                canonical_filter_safe_bytes = canonical_filter_safe.safe_bytes,
                canonical_filter_truncated = canonical_filter_safe.truncated,
                shared_filter_view,
                show_actions_popup = self.show_actions_popup,
                pending_filter_sync = self.pending_filter_sync,
                input_mode = ?self.input_mode,
                "DO_IN_TRACE filter_change.entry"
            );
        }

        // Skip filter updates when actions picker is active
        // (text input should go to actions dialog search, not main filter)
        if self.show_actions_popup {
            if shared_filter_view && new_text != self.filter_text {
                self.pending_filter_sync = true;
            }
            tracing::info!(
                target: "script_kit::do_in_trace",
                event = "DO_IN_TRACE filter_change.actions_popup_return",
                current_view = ?self.current_view,
                new_text_preview = %new_text_safe,
                new_text_bytes = new_text_safe.raw_bytes,
                new_text_safe_bytes = new_text_safe.safe_bytes,
                new_text_truncated = new_text_safe.truncated,
                canonical_filter_preview = %canonical_filter_safe,
                shared_filter_view,
                pending_filter_sync = self.pending_filter_sync,
                "DO_IN_TRACE filter_change.actions_popup_return"
            );
            return;
        }

        if !shared_filter_view {
            tracing::info!(
                target: "script_kit::do_in_trace",
                event = "DO_IN_TRACE filter_change.not_shared_return",
                current_view = ?self.current_view,
                new_text_preview = %new_text_safe,
                new_text_bytes = new_text_safe.raw_bytes,
                new_text_safe_bytes = new_text_safe.safe_bytes,
                new_text_truncated = new_text_safe.truncated,
                canonical_filter_preview = %canonical_filter_safe,
                "DO_IN_TRACE filter_change.not_shared_return"
            );
            return;
        }

        if Self::single_line_filter_input_contains_newline(&new_text) {
            tracing::warn!(
                target: "script_kit::do_in_trace",
                event = "DO_IN_TRACE filter_change.newline_ignored",
                current_view = ?self.current_view,
                canonical_filter_preview = %canonical_filter_safe,
                raw_len = new_text.len(),
                new_text_preview = %new_text_safe,
                new_text_bytes = new_text_safe.raw_bytes,
                new_text_safe_bytes = new_text_safe.safe_bytes,
                new_text_truncated = new_text_safe.truncated,
                "DO_IN_TRACE filter_change.newline_ignored"
            );
            self.pending_filter_sync = true;
            self.sync_filter_input_if_needed(window, cx);
            cx.notify();
            return;
        }

        self.pending_filter_sync = false;

        if matches!(&self.current_view, AppView::CurrentAppCommandsView { .. }) {
            let previous_filter = self.filter_text.clone();
            self.filter_text = new_text.clone();
            if let Err(error) = self.refresh_current_app_commands_session_if_needed(&new_text, cx) {
                tracing::warn!(
                    error = %error,
                    query = %new_text,
                    "current_app_commands.refresh_failed_on_filter_change"
                );
            }
            if let AppView::CurrentAppCommandsView {
                filter,
                selected_index,
            } = &mut self.current_view
            {
                let selected_before = *selected_index;
                let filter_before = filter.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    let input_mode = if self.input_mode == InputMode::Keyboard {
                        "keyboard"
                    } else {
                        "mouse"
                    };

                    let (_filtered, receipt) = crate::builtins::filter_menu_bar_entries(
                        &self.cached_current_app_entries,
                        &new_text,
                    );

                    tracing::debug!(
                        query = %receipt.query,
                        normalized_query = %receipt.normalized_query,
                        total_entries = receipt.total_entries,
                        matched_entries = receipt.matched_entries,
                        input_mode = %input_mode,
                        "current_app_commands.filter_updated"
                    );
                    tracing::info!(
                        target: "script_kit::do_in_trace",
                        event = "DO_IN_TRACE current_app_filter.synced",
                        previous_canonical_filter = %previous_filter,
                        previous_view_filter = %filter_before,
                        new_text_preview = %new_text_safe,
                        new_text_bytes = new_text_safe.raw_bytes,
                        new_text_safe_bytes = new_text_safe.safe_bytes,
                        new_text_truncated = new_text_safe.truncated,
                        selected_before,
                        selected_after = *selected_index,
                        total_entries = receipt.total_entries,
                        matched_entries = receipt.matched_entries,
                        normalized_query = %receipt.normalized_query,
                        input_mode,
                        "DO_IN_TRACE current_app_filter.synced"
                    );

                    Self::scroll_builtin_to_top_with_log(
                        &self.current_app_commands_scroll_handle,
                        "current_app_commands",
                        self.cached_current_app_entries.len(),
                        &new_text,
                        input_mode,
                    );
                    cx.notify();
                } else {
                    tracing::info!(
                        target: "script_kit::do_in_trace",
                        event = "DO_IN_TRACE current_app_filter.unchanged",
                        previous_canonical_filter = %previous_filter,
                        view_filter = %filter_before,
                        new_text_preview = %new_text_safe,
                        new_text_bytes = new_text_safe.raw_bytes,
                        new_text_safe_bytes = new_text_safe.safe_bytes,
                        new_text_truncated = new_text_safe.truncated,
                        selected_index = *selected_index,
                        "DO_IN_TRACE current_app_filter.unchanged"
                    );
                }
            }
            return;
        }

        // Sync filter to builtin views that use the shared input
        match &mut self.current_view {
            AppView::ClipboardHistoryView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    let input_mode = if self.input_mode == InputMode::Keyboard {
                        "keyboard"
                    } else {
                        "mouse"
                    };
                    Self::scroll_builtin_to_top_with_log(
                        &self.clipboard_list_scroll_handle,
                        "clipboard_history",
                        self.cached_clipboard_entries.len(),
                        &new_text,
                        input_mode,
                    );
                }
                let filtered_entries: Vec<_> = if filter.is_empty() {
                    self.cached_clipboard_entries.iter().enumerate().collect()
                } else {
                    let filter_lower = filter.to_lowercase();
                    self.cached_clipboard_entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| {
                            e.text_preview.to_lowercase().contains(&filter_lower)
                                || e.ocr_text
                                    .as_deref()
                                    .unwrap_or("")
                                    .to_lowercase()
                                    .contains(&filter_lower)
                        })
                        .collect()
                };
                self.focused_clipboard_entry_id = filtered_entries
                    .get(*selected_index)
                    .map(|(_, entry)| entry.id.clone());
                cx.notify();
                return; // Don't run main menu filter logic
            }
            AppView::EmojiPickerView {
                filter,
                selected_index,
                selected_category,
            } => {
                // Skip entirely when the text hasn't changed — avoids a
                // spurious cx.notify() that can reset the scroll
                // position during trackpad/mouse scrolling.
                if new_text == *filter {
                    return;
                }

                self.filter_text = new_text.clone();
                *selected_index = 0;
                *filter = new_text.clone();

                let input_mode = if self.input_mode == InputMode::Keyboard {
                    "keyboard"
                } else {
                    "mouse"
                };
                let emoji_count =
                    crate::emoji::filtered_ordered_emojis(&new_text, *selected_category).len();

                Self::scroll_builtin_to_top_with_log(
                    &self.emoji_scroll_handle,
                    "emoji_picker",
                    emoji_count,
                    &new_text,
                    input_mode,
                );

                cx.notify();
                return; // Don't run main menu filter logic
            }
            AppView::AppLauncherView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    let input_mode = if self.input_mode == InputMode::Keyboard {
                        "keyboard"
                    } else {
                        "mouse"
                    };
                    Self::scroll_builtin_to_top_with_log(
                        &self.list_scroll_handle,
                        "app_launcher",
                        self.apps.len(),
                        &new_text,
                        input_mode,
                    );
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::WindowSwitcherView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    let input_mode = if self.input_mode == InputMode::Keyboard {
                        "keyboard"
                    } else {
                        "mouse"
                    };
                    Self::scroll_builtin_to_top_with_log(
                        &self.window_list_scroll_handle,
                        "window_switcher",
                        self.cached_windows.len(),
                        &new_text,
                        input_mode,
                    );
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::BrowserTabsView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    let input_mode = if self.input_mode == InputMode::Keyboard {
                        "keyboard"
                    } else {
                        "mouse"
                    };
                    Self::scroll_builtin_to_top_with_log(
                        &self.browser_tabs_scroll_handle,
                        "browser_tabs",
                        self.cached_browser_tabs.len(),
                        &new_text,
                        input_mode,
                    );
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::ProcessManagerView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    let input_mode = if self.input_mode == InputMode::Keyboard {
                        "keyboard"
                    } else {
                        "mouse"
                    };
                    Self::scroll_builtin_to_top_with_log(
                        &self.process_list_scroll_handle,
                        "process_manager",
                        self.cached_processes.len(),
                        &new_text,
                        input_mode,
                    );
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::FlowUxView {
                filter,
                selected_index,
                ..
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    let input_mode = if self.input_mode == InputMode::Keyboard {
                        "keyboard"
                    } else {
                        "mouse"
                    };
                    Self::scroll_builtin_to_top_with_log(
                        &self.flow_ux_scroll_handle,
                        "flow_ux",
                        0,
                        &new_text,
                        input_mode,
                    );
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::FlowSessionView { .. } => {
                // The main input is the session composer: the text is a
                // message draft, not a query. Hold it and repaint only.
                self.filter_text = new_text.clone();
                cx.notify();
                return; // Don't run main menu filter logic
            }
            AppView::ProfileSearchView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.list_scroll_handle
                        .scroll_to_item(0, ScrollStrategy::Top);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::SettingsView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.builtin_row_stack_scroll_handle.scroll_to_item(0);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::SearchAiPresetsView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.builtin_row_stack_scroll_handle.scroll_to_item(0);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::FavoritesBrowseView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.builtin_row_stack_scroll_handle.scroll_to_item(0);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::AgentChatHistoryView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.agent_chat_history_scroll_handle
                        .scroll_to_top_of_item(0);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::BrowserHistoryView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.browser_history_scroll_handle.scroll_to_top_of_item(0);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::DictationHistoryView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.dictation_history_scroll_handle
                        .scroll_to_top_of_item(0);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::TipsView {
                filter,
                selected_index,
                ..
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.tips_list_scroll_handle
                        .scroll_to_item(0, ScrollStrategy::Top);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::NotesBrowseView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.notes_browse_scroll_handle.scroll_to_top_of_item(0);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::DesignGalleryView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.design_gallery_scroll_handle
                        .scroll_to_item(0, ScrollStrategy::Top);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::FooterGalleryView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                if Self::sync_builtin_query_state(filter, selected_index, &new_text) {
                    self.footer_gallery_scroll_handle
                        .scroll_to_item(0, ScrollStrategy::Top);
                    cx.notify();
                }
                return; // Don't run main menu filter logic
            }
            AppView::ThemeChooserView {
                filter,
                selected_index,
            } => {
                self.filter_text = new_text.clone();
                let changed = Self::sync_builtin_query_state(filter, selected_index, &new_text);

                if changed {
                    // Filter against the same unified catalog (user themes +
                    // built-in presets) that the gallery renders, so typing
                    // never desyncs the list, sizing, and preview. Shared with
                    // the protocol setFilter path.
                    self.apply_theme_chooser_filter_change_effects(cx);
                }
                return; // Don't run main menu filter logic
            }
            AppView::FileSearchView {
                query,
                selected_index,
                presentation,
            } => {
                // Copy presentation early so we can use it after releasing
                // the mutable borrow on self.current_view.
                let presentation = *presentation;

                // ── Mini presentation: return to ScriptList when query
                //    no longer matches the ~ trigger ──────────────────
                if presentation == FileSearchPresentation::Mini
                    && !Self::should_enter_file_search_from_script_list(&new_text)
                {
                    let _ = self.begin_file_search_session();
                    self.reset_file_search_transient_state();
                    self.show_script_list_with_main_filter_focus();
                    self.filter_text = new_text.clone();
                    self.pending_placeholder = None;
                    self.queue_filter_compute(new_text.clone(), cx);
                    cx.notify();
                    return;
                }

                self.filter_text = new_text.clone();
                if *query != new_text {
                    logging::log(
                        "SEARCH",
                        &format!(
                            "Query changed: '{}' -> '{}' (cached_results={}, display_indices={})",
                            query,
                            new_text,
                            self.cached_file_results.len(),
                            self.file_search_display_indices.len()
                        ),
                    );

                    // Get old filter BEFORE updating query (for frozen filter during transitions)
                    let old_filter =
                        if let Some(old_parsed) = crate::file_search::parse_directory_path(query) {
                            old_parsed.filter
                        } else if !query.is_empty() {
                            Some(query.clone())
                        } else {
                            None
                        };

                    // Update query immediately for responsive UI
                    *query = new_text.clone();
                    *selected_index = 0;
                    self.reset_file_search_selection_mode();

                    // Check if this is a directory path with potential filter
                    if let Some(parsed) = crate::file_search::parse_directory_path(&new_text) {
                        let dir_changed =
                            self.file_search_current_dir.as_ref() != Some(&parsed.directory);
                        let hidden_visibility_changed =
                            self.file_search_current_dir_show_hidden != parsed.show_hidden;

                        if dir_changed || hidden_visibility_changed {
                            self.restart_file_search_stream_for_query(
                                new_text.clone(),
                                presentation,
                                Some(old_filter),
                                true,
                                cx,
                            );
                        } else {
                            // Same directory - just filter existing results (instant!)
                            self.file_search_frozen_filter = None;
                            self.file_search_loading = false;
                            self.recompute_file_search_display_indices();
                            Self::resize_file_search_window_after_results_change(
                                presentation,
                                self.file_search_display_indices.len(),
                                true,
                                true,
                            );
                            cx.notify();
                        }
                        return;
                    }

                    // Not a directory path - do regular file search with streaming
                    logging::log(
                        "SEARCH",
                        &format!("Starting mdfind search for query: '{}'", new_text),
                    );
                    self.restart_file_search_stream_for_query(
                        new_text.clone(),
                        presentation,
                        None,
                        false,
                        cx,
                    );
                }
                return; // Don't run main menu filter logic
            }
            _ => {} // Continue with main menu logic
        }

        // Token-atomic delete parity with the Agent Chat composer: a single
        // backspace inside an alias-registered `@file:` token removes the
        // whole token (and its trailing space) instead of leaving a damaged
        // mention that would re-trigger subsearch on a broken basename.
        if let Some(fixed) = self.spine_mention_atomic_delete_fixup(&self.filter_text, &new_text) {
            self.set_filter_text_immediate(fixed, window, cx);
            return;
        }

        // ── First-character ScriptList entry routes ──────────────────
        // Prompt-builder sigils (@, /, |, .) are handled by the Spine
        // projection and stay in the main list. Only mode-exit sigils and
        // file search trigger a view switch.
        if let Some(entry) = Self::special_entry_from_script_list_filter(&new_text) {
            if self.route_script_list_special_entry(entry, &new_text, window, cx) {
                return;
            }
        }

        self.set_menu_syntax_mode_from_filter(&new_text);

        // Spine: reparse alongside the existing menu syntax path.
        // Cursor position is approximated as end-of-input for now; full cursor
        // tracking from GPUI selection events will come in a later step.
        if self.spine_enabled {
            self.set_spine_parse_from_filter_and_cursor(&new_text, new_text.len());
            self.maybe_start_spine_file_subsearch_for_current_projection(cx);
        }

        // Iter 019 D1 / iter 020 D2a — run the pure picker state machine on
        // every filter update and mirror the resulting transition into the
        // cached `menu_syntax_trigger_picker_state` field. The GPUI window
        // consumer lands in a follow-up commit; today we only track state +
        // emit tracing so the transitions are observable in production.
        {
            // Honor post-Accept suppression. When the dispatcher just
            // committed a target selection (Enter on `+todo`), the
            // resulting filter text (`+todo `) would otherwise re-trigger
            // the state machine → `Open` with the handler snapshot,
            // flickering the popup back open immediately after dismissal.
            // We skip the state machine entirely for the exact suppressed
            // filter and clear the suppression the moment the filter
            // deviates (user types a body character or deletes back).
            let popup_suppressed_for_this_text = self
                .menu_syntax_trigger_picker_suppressed_filter
                .as_deref()
                .map(|s| s == new_text)
                .unwrap_or(false);
            if !popup_suppressed_for_this_text
                && self.menu_syntax_trigger_picker_suppressed_filter.is_some()
            {
                self.menu_syntax_trigger_picker_suppressed_filter = None;
            }

            let capture_composer_owns_input =
                self.menu_syntax_capture_form_owns_input_for(&new_text);
            let capture_form_field_owns_input =
                self.menu_syntax_form_input_active && capture_composer_owns_input;
            if capture_form_field_owns_input {
                self.menu_syntax_object_selector_state = Default::default();
                self.menu_syntax_trigger_picker_state = Default::default();
            } else {
                self.run_menu_syntax_object_selector_state_machine(&new_text, window, cx);
            }
            let object_selector_owns_input = !capture_form_field_owns_input
                && self.menu_syntax_object_selector_state.snapshot.is_some();

            let picker_ctx = self.menu_syntax_trigger_picker_context(&new_text);
            let transition = if popup_suppressed_for_this_text {
                // User just accepted; picker should stay closed for this
                // exact filter. Represent that as NoChange so the rest of
                // the block runs uniformly.
                crate::menu_syntax_trigger_picker::TriggerPickerTransition::NoChange
            } else if capture_form_field_owns_input {
                crate::menu_syntax_trigger_picker::TriggerPickerTransition::NoChange
            } else if object_selector_owns_input {
                if self.menu_syntax_trigger_picker_state.snapshot.is_some() {
                    crate::menu_syntax_trigger_picker::TriggerPickerTransition::Close
                } else {
                    crate::menu_syntax_trigger_picker::TriggerPickerTransition::NoChange
                }
            } else {
                crate::menu_syntax_trigger_picker::plan_trigger_picker_transition(
                    &self.menu_syntax_trigger_picker_state,
                    &new_text,
                    &picker_ctx,
                )
            };
            let mut trigger_state_changed = false;
            match &transition {
                crate::menu_syntax_trigger_picker::TriggerPickerTransition::NoChange => {}
                crate::menu_syntax_trigger_picker::TriggerPickerTransition::Close => {
                    if self.menu_syntax_trigger_picker_state.snapshot.is_some() {
                        tracing::info!(
                            target: "script_kit::menu_syntax_popup",
                            event = "menu_syntax_trigger_picker_close",
                            filter = %new_text,
                        );
                    }
                    self.menu_syntax_trigger_picker_state = Default::default();
                }
                crate::menu_syntax_trigger_picker::TriggerPickerTransition::Open {
                    snapshot,
                    selected_row_id,
                } => {
                    tracing::info!(
                        target: "script_kit::menu_syntax_popup",
                        event = "menu_syntax_trigger_picker_open",
                        filter = %new_text,
                        row_count = snapshot.rows.len(),
                        selected_row_id = ?selected_row_id,
                    );
                    self.menu_syntax_trigger_picker_state =
                        crate::menu_syntax_trigger_picker::MenuSyntaxTriggerPickerState {
                            snapshot: Some(snapshot.clone()),
                            selected_row_id: selected_row_id.clone(),
                            visible_start: 0,
                        };
                    trigger_state_changed = true;
                }
                crate::menu_syntax_trigger_picker::TriggerPickerTransition::Update {
                    snapshot,
                    selected_row_id,
                } => {
                    let selected_index = selected_row_id
                        .as_deref()
                        .and_then(|id| snapshot.rows.iter().position(|row| row.id == id))
                        .unwrap_or(0);
                    let visible_start =
                        crate::menu_syntax_trigger_picker::trigger_picker_visible_start_for_selection(
                            self.menu_syntax_trigger_picker_state.visible_start,
                            selected_index,
                            snapshot.rows.len(),
                        );
                    tracing::info!(
                        target: "script_kit::menu_syntax_popup",
                        event = "menu_syntax_trigger_picker_update",
                        filter = %new_text,
                        row_count = snapshot.rows.len(),
                        selected_row_id = ?selected_row_id,
                    );
                    self.menu_syntax_trigger_picker_state =
                        crate::menu_syntax_trigger_picker::MenuSyntaxTriggerPickerState {
                            snapshot: Some(snapshot.clone()),
                            selected_row_id: selected_row_id.clone(),
                            visible_start,
                        };
                    trigger_state_changed = true;
                }
            }
            if object_selector_owns_input {
                self.invalidate_grouped_cache();
            } else if trigger_state_changed {
                // Trigger snapshots now render in the main search area via
                // `menu_syntax_main_hint_snapshot`, not in the detached popup.
                // The popup window is still used by capture-form field
                // suggestions, so close any stale trigger-owned popup here
                // while preserving the snapshot that feeds the main area.
                // Ownership just flipped — invalidate the grouped results
                // cache so the main launcher list is rebuilt with the
                // trigger-aware gate in `get_grouped_results_cached`.
                // Without this, a cache computed before the trigger snapshot
                // appeared would stay visible behind the main hint surface.
                self.invalidate_grouped_cache();
            } else if matches!(
                &transition,
                crate::menu_syntax_trigger_picker::TriggerPickerTransition::Close
            ) || capture_composer_owns_input
                || crate::menu_syntax::active_filter_head_owns_main_list(&new_text)
            {
                // Popup just closed or the capture composer now owns the
                // input — rebuild the main list with the ownership gate.
                self.invalidate_grouped_cache();
            }
        }
        if crate::menu_syntax::active_filter_head_owns_main_list(&new_text) {
            self.main_menu_fallback_state.clear();
        }

        if new_text == self.filter_text {
            return;
        }

        self.filter_text = new_text.clone();
        self.sync_menu_syntax_form_inputs_from_filter(window, cx);
        // Echo the canonical input immediately. The grouped list remains keyed
        // off `computed_filter_text` until the coalesced compute below applies,
        // so this does not publish a partially recomputed result frame.
        cx.notify();

        // Reset input history navigation when user types (they're no longer navigating history)
        self.input_history.reset_navigation();

        let new_calc = crate::calculator::try_build(&new_text);
        if self.inline_calculator != new_calc {
            self.inline_calculator = new_calc;
            self.invalidate_grouped_cache();
            self.list_scroll_handle
                .scroll_to_item(0, ScrollStrategy::Top);
        }

        // FIX: Don't reset selected_index here - do it in queue_filter_compute() callback
        // AFTER computed_filter_text is updated. This prevents a race condition where:
        // 1. We set selected_index=0 immediately
        // 2. Render runs before async cache update
        // 3. Stale grouped_items has SectionHeader at index 0
        // 4. coerce_selection moves selection to index 1
        // Instead, we'll reset selection when the cache actually updates.
        self.last_scrolled_index = None;

        // A1 decision (2026-06-09): aliases no longer auto-execute on a
        // trailing space. An exact alias match instead pins the aliased
        // command at index 0 during grouping (see
        // `pin_alias_match_into_grouped_results`) so Enter runs it.

        if let Some(pred) = self.ghost_prediction.take() {
            self.ghost_prediction = crate::scripts::search::ghost::reconcile_typed_through(
                &self.filter_text,
                &new_text,
                &pred,
            );
        }

        self.queue_filter_compute(new_text.clone(), cx);

        // Log handler timing
        let handler_elapsed = handler_start.elapsed();
        if handler_elapsed >= std::time::Duration::from_millis(16) {
            logging::log(
                "FILTER_PERF",
                &format!(
                    "[HANDLER_SLOW] handle_filter_input_change took {:.2}ms for '{}'",
                    handler_elapsed.as_secs_f64() * 1000.0,
                    new_text
                ),
            );
        }
    }
}

/// Describes the source of a file search stream.
#[derive(Clone, Debug)]
enum FileSearchStreamSource {
    Directory { dir: String, show_hidden: bool },
    Spotlight { query: String },
}

impl ScriptListApp {
    fn reset_file_search_selection_mode(&mut self) {
        self.file_search_selection_mode = FileSearchSelectionMode::AutoFirst;
    }

    pub(crate) fn lock_file_search_selection_to_user_choice(&mut self) {
        self.file_search_selection_mode = FileSearchSelectionMode::UserLockedPath;
    }

    /// Return the path of the currently selected file-search row, if any.
    pub(crate) fn current_file_search_selected_path(&self) -> Option<String> {
        let AppView::FileSearchView { selected_index, .. } = &self.current_view else {
            return None;
        };
        let display_len = self.file_search_display_indices.len();
        if display_len == 0 {
            return None;
        }
        let clamped = (*selected_index).min(display_len.saturating_sub(1));
        let result_index = *self.file_search_display_indices.get(clamped)?;
        self.cached_file_results
            .get(result_index)
            .map(|entry| entry.path.clone())
    }

    /// After results change, restore selection to `preferred_path` if still
    /// visible, otherwise clamp to the nearest valid row.
    pub(crate) fn restore_file_search_selection_after_results_change(
        &mut self,
        preferred_path: Option<&str>,
    ) {
        let len = self.file_search_display_indices.len();
        let pin_to_first_row =
            self.file_search_selection_mode == FileSearchSelectionMode::AutoFirst;
        let fallback_index = match &self.current_view {
            AppView::FileSearchView { selected_index, .. } if len > 0 => {
                (*selected_index).min(len.saturating_sub(1))
            }
            _ => 0,
        };

        let next_index = if pin_to_first_row {
            0
        } else {
            preferred_path
                .and_then(|path| self.file_search_display_index_for_path(path))
                .unwrap_or(fallback_index)
        };

        if let AppView::FileSearchView { selected_index, .. } = &mut self.current_view {
            *selected_index = if len == 0 {
                0
            } else {
                next_index.min(len.saturating_sub(1))
            };
        }
    }

    /// Reset all transient file-search state so exit/reopen cycles start
    /// from a clean slate.
    fn reset_file_search_transient_state(&mut self) {
        self.file_search_current_dir = None;
        self.file_search_current_dir_show_hidden = false;
        self.file_search_loading = false;
        self.file_search_frozen_filter = None;
        self.file_search_actions_path = None;
        self.file_search_cancel = None;
        self.file_search_debounce_task = None;
        self.cached_file_results.clear();
        self.file_search_display_indices.clear();
        self.reset_file_search_selection_mode();
        self.file_search_scroll_handle
            .scroll_to_item(0, ScrollStrategy::Top);
    }

    /// Spawn a streaming file-search task that feeds batched results back
    /// into `apply_file_search_stream_batch`.  Directory streams defer UI
    /// updates until done so rows do not visibly reshuffle while loading;
    /// Spotlight streams can still reveal progressive matches.
    fn spawn_file_search_stream_task(
        &mut self,
        gen: u64,
        source: FileSearchStreamSource,
        presentation: FileSearchPresentation,
        debounce_ms: u64,
        query_guard: Option<String>,
        clear_on_first_batch: bool,
        sort_on_done: bool,
        defer_batches_until_done: bool,
        cx: &mut Context<Self>,
    ) {
        let cancel = crate::file_search::new_cancel_token();
        self.file_search_cancel = Some(cancel.clone());

        let task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(debounce_ms))
                .await;

            let (tx, rx) = std::sync::mpsc::channel();

            std::thread::spawn({
                let cancel = cancel.clone();
                let source = source.clone();
                move || match source {
                    FileSearchStreamSource::Directory { dir, show_hidden } => {
                        crate::file_search::list_directory_streaming_with_options(
                            &dir,
                            cancel,
                            false,
                            show_hidden,
                            |event| {
                                let _ = tx.send(event);
                            },
                        );
                    }
                    FileSearchStreamSource::Spotlight { query } => {
                        crate::file_search::search_files_streaming(
                            &query,
                            None,
                            crate::file_search::DEFAULT_SEARCH_LIMIT,
                            cancel,
                            false,
                            |event| {
                                let _ = tx.send(event);
                            },
                        );
                    }
                }
            });

            let mut pending: Vec<crate::file_search::FileResult> = Vec::new();
            let mut done = false;
            let mut first_batch = true;

            while !done {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;

                while let Ok(event) = rx.try_recv() {
                    match event {
                        crate::file_search::SearchEvent::Result(result) => {
                            pending.push(result);
                        }
                        crate::file_search::SearchEvent::Done => {
                            done = true;
                            break;
                        }
                    }
                }

                if defer_batches_until_done && !done {
                    continue;
                }

                if !pending.is_empty() || done {
                    let batch = std::mem::take(&mut pending);
                    let is_done = done;
                    let is_first_batch = first_batch;
                    let guard = query_guard.clone();
                    first_batch = false;

                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| {
                            app.apply_file_search_stream_batch(
                                gen,
                                presentation,
                                guard.as_deref(),
                                batch,
                                clear_on_first_batch,
                                sort_on_done,
                                is_first_batch,
                                is_done,
                                cx,
                            );
                        })
                    });
                }
            }
        });

        self.file_search_debounce_task = Some(task);
    }

    /// Apply one batch of streaming results.  Stale generations and
    /// query-guard mismatches are silently dropped.
    fn apply_file_search_stream_batch(
        &mut self,
        gen: u64,
        presentation: FileSearchPresentation,
        query_guard: Option<&str>,
        batch: Vec<crate::file_search::FileResult>,
        clear_on_first_batch: bool,
        sort_on_done: bool,
        is_first_batch: bool,
        is_done: bool,
        cx: &mut Context<Self>,
    ) {
        if self.file_search_gen != gen {
            return;
        }

        if let Some(expected_query) = query_guard {
            let AppView::FileSearchView { query, .. } = &self.current_view else {
                return;
            };
            if query != expected_query {
                return;
            }
        }

        // Capture selected path before mutating results so we can restore it.
        let preferred_selected_path = self.current_file_search_selected_path();

        let mut needs_recompute = false;

        if clear_on_first_batch && is_first_batch {
            self.cached_file_results.clear();
            needs_recompute = true;
        }

        if !batch.is_empty() {
            self.cached_file_results.extend(batch);
            needs_recompute = true;
        }

        if needs_recompute {
            let is_directory_query = matches!(
                &self.current_view,
                AppView::FileSearchView { query, .. }
                    if crate::file_search::parse_directory_path(query).is_some()
            );
            if is_directory_query {
                self.apply_file_search_sort_mode();
                tracing::debug!(
                    category = "FILE_SEARCH",
                    event = "stream_batch_reapplied_sort",
                    ?self.file_search_sort_mode,
                    cached_count = self.cached_file_results.len(),
                    "Reapplied active directory sort before recomputing streamed results"
                );
            }
            self.recompute_file_search_display_indices();
            self.restore_file_search_selection_after_results_change(
                preferred_selected_path.as_deref(),
            );
        }

        if is_done {
            self.file_search_loading = false;
            self.file_search_frozen_filter = None;

            // Always reapply the session sort mode when the active query
            // resolves to a directory, even if a caller passes
            // `sort_on_done = false`.  This keeps the user's chosen sort
            // order across directory navigations and refreshes.
            let is_directory_query = matches!(
                &self.current_view,
                AppView::FileSearchView { query, .. }
                    if crate::file_search::parse_directory_path(query).is_some()
            );
            if sort_on_done || is_directory_query {
                self.apply_file_search_sort_mode();
                self.recompute_file_search_display_indices();
                self.restore_file_search_selection_after_results_change(
                    preferred_selected_path.as_deref(),
                );
            }

            if !self.file_search_display_indices.is_empty() {
                if let AppView::FileSearchView { selected_index, .. } = &self.current_view {
                    self.file_search_scroll_handle
                        .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                }
            }
        }

        Self::resize_file_search_window_after_results_change(
            presentation,
            self.file_search_display_indices.len(),
            is_first_batch,
            is_done,
        );
        cx.notify();
    }

    /// Kick off a new streaming file-search session.  Cancels any
    /// in-flight work, advances the generation counter, and delegates
    /// to `spawn_file_search_stream_task`.
    pub(crate) fn restart_file_search_stream_for_query(
        &mut self,
        query: String,
        presentation: FileSearchPresentation,
        old_filter: Option<Option<String>>,
        preserve_old_results_until_first_batch: bool,
        cx: &mut Context<Self>,
    ) {
        let gen = self.begin_file_search_session();
        self.file_search_loading = true;
        self.reset_file_search_selection_mode();

        if let Some(parsed) = crate::file_search::parse_directory_path(&query) {
            self.file_search_current_dir = Some(parsed.directory.clone());
            self.file_search_current_dir_show_hidden = parsed.show_hidden;
            self.file_search_frozen_filter = if preserve_old_results_until_first_batch {
                old_filter
            } else {
                None
            };

            if !preserve_old_results_until_first_batch {
                self.cached_file_results.clear();
                self.file_search_display_indices.clear();
                Self::resize_file_search_window_after_results_change(presentation, 0, true, false);
            }

            self.spawn_file_search_stream_task(
                gen,
                FileSearchStreamSource::Directory {
                    dir: parsed.directory,
                    show_hidden: parsed.show_hidden,
                },
                presentation,
                30,
                None,
                preserve_old_results_until_first_batch,
                true,
                true,
                cx,
            );
            cx.notify();
            return;
        }

        // Spotlight search path
        self.file_search_current_dir = None;
        self.file_search_current_dir_show_hidden = false;
        self.file_search_frozen_filter = None;
        self.cached_file_results.clear();
        self.file_search_display_indices.clear();
        Self::resize_file_search_window_after_results_change(presentation, 0, true, false);

        self.spawn_file_search_stream_task(
            gen,
            FileSearchStreamSource::Spotlight {
                query: query.clone(),
            },
            presentation,
            75,
            Some(query),
            false,
            false,
            false,
            cx,
        );
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    fn read_filter_input_change_source() -> String {
        fs::read_to_string("src/app_impl/filter_input_change.rs")
            .expect("Failed to read src/app_impl/filter_input_change.rs")
    }

    #[test]
    fn test_handle_filter_input_change_marks_pending_sync_when_actions_popup_suppresses_updates() {
        let source = read_filter_input_change_source();
        let popup_guard_pos = source
            .find("if self.show_actions_popup")
            .expect("show_actions_popup guard not found");
        let popup_guard_end = (popup_guard_pos + 260).min(source.len());
        let popup_guard_section = &source[popup_guard_pos..popup_guard_end];

        assert!(
            popup_guard_section.contains("new_text != self.filter_text"),
            "show_actions_popup guard must detect divergence between input text and canonical filter_text"
        );
        assert!(
            popup_guard_section.contains("self.pending_filter_sync = true"),
            "show_actions_popup guard must queue pending_filter_sync to prevent persistent UI/filter desync"
        );
    }

    #[test]
    fn test_emoji_picker_filter_change_guards_scroll_behind_real_query_change() {
        let source = read_filter_input_change_source();
        let emoji_pos = source
            .find("AppView::EmojiPickerView")
            .expect("EmojiPickerView match arm not found");
        let emoji_section = &source[emoji_pos..(emoji_pos + 1200).min(source.len())];

        // Must have an early-return guard that skips cx.notify() when text is unchanged
        assert!(
            emoji_section.contains("new_text == *filter"),
            "emoji picker must early-return when the filter text has not changed"
        );

        // Must still scroll to top on real changes (via shared helper)
        assert!(
            emoji_section.contains("scroll_builtin_to_top_with_log"),
            "emoji picker should reset to top on real filter changes via shared helper"
        );
    }

    #[test]
    fn test_scrollable_builtin_views_use_shared_scroll_logging_helper() {
        let source = read_filter_input_change_source();
        for view in [
            "AppView::ClipboardHistoryView",
            "AppView::EmojiPickerView",
            "AppView::AppLauncherView",
            "AppView::WindowSwitcherView",
            "AppView::BrowserTabsView",
            "AppView::ProcessManagerView",
        ] {
            let view_pos = source
                .find(view)
                .unwrap_or_else(|| panic!("{} match arm not found", view));
            let view_section = &source[view_pos..(view_pos + 1200).min(source.len())];
            assert!(
                view_section.contains("scroll_builtin_to_top_with_log"),
                "{} should use the shared structured scroll helper",
                view,
            );
        }
    }

    #[test]
    fn atfile_regression_script_list_filter_changes_start_spine_file_subsearch_after_reparse() {
        let source = read_filter_input_change_source();
        let reparse_pos = source
            .find("self.set_spine_parse_from_filter_and_cursor(&new_text, new_text.len());")
            .expect("ScriptList live-input Spine reparse not found");
        let section = &source[reparse_pos..(reparse_pos + 220).min(source.len())];
        assert!(
            section.contains("self.maybe_start_spine_file_subsearch_for_current_projection(cx);"),
            "live ScriptList input changes must start @file: subsearch after reparsing Spine"
        );
    }
}
