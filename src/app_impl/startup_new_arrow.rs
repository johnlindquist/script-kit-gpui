        // Add arrow key interceptor for builtin views with Input components
        // This fires BEFORE Input component handles arrow keys, allowing list navigation
        let app_entity_for_arrows = cx.entity().downgrade();
        let arrow_interceptor = cx.intercept_keystrokes({
            let app_entity = app_entity_for_arrows;
            move |event, window, cx| {
                // When the main window is hidden (e.g. Notes/AI open), main-menu
                // key interceptors must not consume keystrokes from secondary windows.
                if !script_kit_gpui::is_main_window_visible() {
                    return;
                }

                // intercept_keystrokes is GLOBAL and fires for ALL windows in the app.
                // Keep main list arrow routing scoped to the main window so notes/AI/actions
                // windows receive their own navigation key events.
                let is_notes = crate::notes::is_notes_window(window);
                let is_detached_agent_chat = crate::ai::agent_chat::ui::chat_window::is_chat_window(window);
                let is_actions = crate::actions::is_actions_window(window);
                let skip_secondary = is_notes || is_detached_agent_chat || is_actions;
                if skip_secondary {
                    tracing::debug!(
                        target: "script_kit::keyboard",
                        event = "arrow_interceptor_skipped_secondary_window",
                        is_notes,
                        is_detached_agent_chat,
                        is_actions,
                        skip_secondary,
                        detached_agent_chat_explicit_skip = is_detached_agent_chat,
                    );
                    return;
                }

                let key = event.keystroke.key.as_str();
                let is_up = crate::ui_foundation::is_key_up(key);
                let is_down = crate::ui_foundation::is_key_down(key);
                let is_left = crate::ui_foundation::is_key_left(key);
                let is_right = crate::ui_foundation::is_key_right(key);
                if confirm::consume_main_window_key_while_confirm_open(
                    key,
                    &event.keystroke.modifiers,
                    cx,
                ) {
                    cx.stop_propagation();
                    return;
                }
                // Check for arrow keys (no modifiers except shift for selection)
                // Left/right included for EmojiPickerView grid navigation;
                // other views fall through to _ => {} so Input handles them normally.
                if (is_up || is_down || is_left || is_right)
                    && !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.control
                {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            // Universal: Route arrow keys to actions dialog when popup is open
                            // This ensures ALL views (ChatPrompt, ArgPrompt, etc.) route
                            // arrows to the dialog, not just the few views with explicit cases below.
                            if this.show_actions_popup {
                                if let Some(ref dialog) = this.actions_dialog {
                                    if is_up {
                                        dialog.update(cx, |d, cx| d.move_up(cx));
                                    } else if is_down {
                                        dialog.update(cx, |d, cx| d.move_down(cx));
                                    }
                                    crate::actions::notify_actions_window(cx);
                                }
                                cx.stop_propagation();
                                return;
                            }

                            // Up/Down in a live flow session walks the prompt
                            // history, matching Agent Chat. Flow had no recall
                            // at all: the match below has an arm for the desk
                            // but none for a session, so arrows fell through
                            // and tweaking one word of a long prompt meant
                            // retyping the whole thing.
                            //
                            // This sits BEFORE the match on purpose. That match
                            // holds `&mut this.current_view` for the whole
                            // block, so an arm cannot call a `&mut self` method
                            // like `set_filter_text_immediate` — and putting
                            // the composer write behind a deferred call instead
                            // would let it land after the next keystroke.
                            // `is_up || is_down` is load-bearing: the guard
                            // above also admits Left/Right, and branching on
                            // `is_up` alone would make Left behave as Down and
                            // steal caret movement inside the composer.
                            if (is_up || is_down)
                                && let AppView::FlowSessionView { session_id } = this.current_view
                            {
                                // The history IS the session's turns; there is
                                // no second copy to drift out of step.
                                let history: Vec<String> = this
                                    .conversations.flow_sessions
                                    .iter()
                                    .find(|(meta, _)| meta.id == session_id)
                                    .map(|(meta, _)| {
                                        meta.turns
                                            .iter()
                                            .map(|turn| turn.user.clone())
                                            .filter(|prompt| !prompt.trim().is_empty())
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                match crate::flow_prompt_history_move(
                                    history.len(),
                                    this.flow_session_prompt_history_index,
                                    is_up,
                                ) {
                                    crate::FlowPromptHistoryMove::Ignore => {}
                                    crate::FlowPromptHistoryMove::Recall(index) => {
                                        if let Some(prompt) = history.get(index).cloned() {
                                            // Park the live draft on the way in
                                            // so arrowing back down restores
                                            // what was typed instead of an
                                            // empty composer.
                                            if this.flow_session_prompt_history_index.is_none() {
                                                this.flow_session_prompt_draft =
                                                    Some(this.filter_text.clone());
                                            }
                                            this.flow_session_prompt_history_index = Some(index);
                                            this.set_filter_text_immediate(prompt, window, cx);
                                            cx.stop_propagation();
                                            return;
                                        }
                                    }
                                    crate::FlowPromptHistoryMove::RestoreDraft => {
                                        let draft =
                                            this.flow_session_prompt_draft.take().unwrap_or_default();
                                        this.flow_session_prompt_history_index = None;
                                        this.set_filter_text_immediate(draft, window, cx);
                                        cx.stop_propagation();
                                        return;
                                    }
                                }
                            }

                            // Only intercept in views that use Input + list navigation
                            match &mut this.current_view {
                                AppView::FileSearchView {
                                    selected_index,
                                    ..
                                } => {
                                    // Actions popup routing is handled by the
                                    // universal guard above; this arm only runs
                                    // when the popup is closed.

                                    // Use precomputed display list length —
                                    // never re-filter ad hoc.
                                    let filtered_len =
                                        this.file_search_display_indices.len();

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    let mut moved_selection = false;
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        moved_selection = true;
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        moved_selection = true;
                                    }

                                    if moved_selection {
                                        this.lock_file_search_selection_to_user_choice();
                                    }

                                    this.file_search_scroll_handle.scroll_to_item(
                                        *selected_index,
                                        gpui::ScrollStrategy::Nearest,
                                    );
                                    cx.notify();
                                    cx.stop_propagation();
                                }
                                AppView::ClipboardHistoryView {
                                    selected_index,
                                    filter,
                                } => {
                                    // CRITICAL: If actions popup is open, route to actions dialog instead
                                    if this.show_actions_popup {
                                        if let Some(ref dialog) = this.actions_dialog {
                                            if is_up {
                                                dialog.update(cx, |d, cx| d.move_up(cx));
                                            } else if is_down {
                                                dialog.update(cx, |d, cx| d.move_down(cx));
                                            }
                                            // Notify the actions window to re-render
                                            crate::actions::notify_actions_window(cx);
                                        }
                                        cx.stop_propagation();
                                        return;
                                    }

                                    let filtered_entries: Vec<_> = if filter.is_empty() {
                                        this.cached_clipboard_entries.iter().enumerate().collect()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        this.cached_clipboard_entries
                                            .iter()
                                            .enumerate()
                                            .filter(|(_, e)| {
                                                e.text_preview
                                                    .to_lowercase()
                                                    .contains(&filter_lower)
                                            })
                                            .collect()
                                    };
                                    let filtered_len = filtered_entries.len();
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.clipboard_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.clipboard_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                    }
                                    this.focused_clipboard_entry_id = filtered_entries
                                        .get(*selected_index)
                                        .map(|(_, entry)| entry.id.clone());
                                    cx.notify();
                                    cx.stop_propagation();
                                }
                                AppView::AppLauncherView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len =
                                        Self::app_launcher_filtered_entries(&this.apps, filter)
                                            .len();
                                    let old_index = *selected_index;

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                    }

                                    if *selected_index != old_index {
                                        tracing::debug!(
                                            target: "script_kit::scroll",
                                            event = "builtin_selection_nav",
                                            view = "app_launcher",
                                            old_index,
                                            new_index = *selected_index,
                                            total_items = filtered_len,
                                            strategy = "nearest",
                                        );

                                        this.list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        this.input_mode = InputMode::Keyboard;
                                        this.hovered_index = None;
                                        cx.notify();
                                    }

                                    cx.stop_propagation();
                                }
                                AppView::WindowSwitcherView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len = if filter.is_empty() {
                                        this.cached_windows.len()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        this.cached_windows
                                            .iter()
                                            .filter(|w| {
                                                w.title.to_lowercase().contains(&filter_lower)
                                                    || w.app.to_lowercase().contains(&filter_lower)
                                            })
                                            .count()
                                    };

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.window_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.window_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::ProcessManagerView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len = if filter.is_empty() {
                                        this.cached_processes.len()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        this.cached_processes
                                            .iter()
                                            .filter(|p| {
                                                p.script_path.to_lowercase().contains(&filter_lower)
                                                    || p.pid.to_string().contains(&filter_lower)
                                            })
                                            .count()
                                    };

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.process_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.process_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::FlowUxView {
                                    selected_index,
                                    filter,
                                    ..
                                } => {
                                    let cwd = this.flow_ux_cwd();
                                    let roster =
                                        crate::flows::catalog::flow_catalog().roster_for(&cwd);
                                    let filtered_len =
                                        crate::flows::catalog::filter_flows(&roster.flows, filter)
                                            .len();

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.flow_ux_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.flow_ux_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::CurrentAppCommandsView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len = if filter.is_empty() {
                                        this.cached_current_app_entries.len()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        this.cached_current_app_entries
                                            .iter()
                                            .filter(|e| {
                                                e.name.to_lowercase().contains(&filter_lower)
                                                    || e.keywords.iter().any(|k| k.contains(&filter_lower))
                                            })
                                            .count()
                                    };

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.current_app_commands_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.current_app_commands_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::AgentChatHistoryView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len = if filter.is_empty() {
                                        crate::ai::agent_chat::ui::history::load_history().len()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        crate::ai::agent_chat::ui::history::load_history()
                                            .into_iter()
                                            .filter(|entry| {
                                                entry.first_message
                                                    .to_lowercase()
                                                    .contains(&filter_lower)
                                                    || entry
                                                        .timestamp
                                                        .to_lowercase()
                                                        .contains(&filter_lower)
                                            })
                                            .count()
                                    };

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.agent_chat_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.agent_chat_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::BrowserHistoryView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len =
                                        crate::browser_history::fuzzy_search_browser_history(
                                            &this.cached_browser_history,
                                            filter,
                                        )
                                        .len();

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.browser_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.browser_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::DictationHistoryView {
                                    selected_index,
                                    filter,
                                    visible_limit,
                                } => {
                                    let retained_len = this
                                        .dictation_history_previous_page
                                        .as_ref()
                                        .map(|page| page.rows.len())
                                        .unwrap_or(0);
                                    let filtered_len = crate::dictation::search_history_page(
                                        filter,
                                        0,
                                        *visible_limit,
                                    )
                                    .map(|page| page.rows.len())
                                    .unwrap_or(retained_len);

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.dictation_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.dictation_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::SearchAiPresetsView {
                                    selected_index,
                                    filter,
                                } => {
                                    // Replicate render-time filtering from ai_presets.rs
                                    let default_presets: Vec<(&str, &str, &str)> = vec![
                                        ("general", "General Assistant", "Helpful AI assistant for any task"),
                                        ("coder", "Code Assistant", "Expert programmer and debugger"),
                                        ("writer", "Writing Assistant", "Help with writing and editing"),
                                        ("researcher", "Research Assistant", "Deep analysis and research"),
                                        ("creative", "Creative Partner", "Brainstorming and creative ideas"),
                                    ];
                                    let all_presets = crate::ai::presets::load_presets().unwrap_or_default();
                                    let mut items: Vec<(String, String, String)> = Vec::new();
                                    for (id, name, desc) in &default_presets {
                                        items.push((id.to_string(), name.to_string(), desc.to_string()));
                                    }
                                    for preset in &all_presets {
                                        if !default_presets.iter().any(|(did, _, _)| *did == preset.id) {
                                            items.push((preset.id.clone(), preset.name.clone(), preset.description.clone()));
                                        }
                                    }
                                    let filtered_len = if filter.is_empty() {
                                        items.len()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        items.iter()
                                            .filter(|(id, name, desc)| {
                                                name.to_lowercase().contains(&filter_lower)
                                                    || desc.to_lowercase().contains(&filter_lower)
                                                    || id.to_lowercase().contains(&filter_lower)
                                            })
                                            .count()
                                    };

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.builtin_row_stack_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.builtin_row_stack_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::FavoritesBrowseView {
                                    selected_index,
                                    filter,
                                } => {
                                    // Replicate render-time filtering from favorites.rs
                                    let favorites = script_kit_gpui::favorites::load_favorites()
                                        .unwrap_or_default();
                                    let resolved: Vec<(String, String)> = favorites
                                        .script_ids
                                        .iter()
                                        .map(|id| {
                                            let display_name = this
                                                .scripts
                                                .iter()
                                                .find(|s| s.name == *id)
                                                .map(|s| s.name.clone())
                                                .or_else(|| {
                                                    this.scriptlets
                                                        .iter()
                                                        .find(|sl| sl.name == *id)
                                                        .map(|sl| sl.name.clone())
                                                })
                                                .unwrap_or_else(|| id.clone());
                                            let description = this
                                                .scripts
                                                .iter()
                                                .find(|s| s.name == *id)
                                                .and_then(|s| s.description.clone())
                                                .or_else(|| {
                                                    this.scriptlets
                                                        .iter()
                                                        .find(|sl| sl.name == *id)
                                                        .and_then(|sl| sl.description.clone())
                                                })
                                                .unwrap_or_default();
                                            (display_name, description)
                                        })
                                        .collect();
                                    let filtered_len = if filter.is_empty() {
                                        resolved.len()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        resolved
                                            .iter()
                                            .filter(|(name, desc)| {
                                                name.to_lowercase().contains(&filter_lower)
                                                    || desc.to_lowercase().contains(&filter_lower)
                                            })
                                            .count()
                                    };

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.builtin_row_stack_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.builtin_row_stack_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::ScriptList => {
                                    // CRITICAL: If actions popup is open, route to actions dialog instead
                                    if this.show_actions_popup {
                                        if let Some(ref dialog) = this.actions_dialog {
                                            if is_up {
                                                dialog.update(cx, |d, cx| d.move_up(cx));
                                            } else if is_down {
                                                dialog.update(cx, |d, cx| d.move_down(cx));
                                            }
                                            // Notify the actions window to re-render
                                            crate::actions::notify_actions_window(cx);
                                        }
                                        cx.stop_propagation();
                                        return;
                                    }
                                    if is_up || is_down { this.flush_pending_main_menu_query(cx); }

                                    let menu_syntax_owns_main_list =
                                        this.menu_syntax_mode
                                            .capture_composer_owns_input_for(&this.filter_text)
                                            || this
                                                .menu_syntax_mode
                                                .command_owns_input_for(&this.filter_text);
                                    if menu_syntax_owns_main_list && (is_up || is_down) {
                                        this.scroll_menu_syntax_main_hint(if is_down { 1.0 } else { -1.0 });
                                        cx.notify();
                                        cx.stop_propagation();
                                        return;
                                    }

                                    // Main menu: handle list navigation + input history
                                    const HISTORY: &str = "HISTORY";
                                    if is_up {
                                        // Ensure grouped cache is populated before reading cached boundaries.
                                        let _ = this.get_grouped_results_cached();
                                        let first_selectable_index =
                                            this.main_menu_result_caches.first_selectable_index();
                                        let at_top_of_list = first_selectable_index
                                            .map(|position| this.selected_index <= position)
                                            .unwrap_or(true);
                                        let in_history = this.input_history.current_index().is_some();

                                        if !this.source_filter_mode_blocks_input_history_recall()
                                            && (in_history || at_top_of_list)
                                        {
                                            if let Some(text) = this.input_history.navigate_up() {
                                                logging::log(
                                                    HISTORY,
                                                    &format!("Recalled: {}", text),
                                                );
                                                this.set_filter_text_immediate(text, window, cx);
                                            }
                                            cx.stop_propagation();
                                            return;
                                        }

                                        this.move_selection_up(cx);
                                    } else if is_down {
                                        if !this.source_filter_mode_blocks_input_history_recall()
                                            && this.input_history.current_index().is_some()
                                        {
                                            if let Some(text) = this.input_history.navigate_down() {
                                                logging::log(
                                                    HISTORY,
                                                    &format!("Recalled: {}", text),
                                                );
                                                this.set_filter_text_immediate(text, window, cx);
                                            } else {
                                                this.input_history.reset_navigation();
                                                this.clear_filter(window, cx);
                                            }
                                            cx.stop_propagation();
                                            return;
                                        }

                                        this.move_selection_down(cx);
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::EmojiPickerView { .. } => {
                                    let direction = if is_up {
                                        Some(crate::emoji::EmojiNavDirection::Up)
                                    } else if is_down {
                                        Some(crate::emoji::EmojiNavDirection::Down)
                                    } else if is_left {
                                        Some(crate::emoji::EmojiNavDirection::Left)
                                    } else if is_right {
                                        Some(crate::emoji::EmojiNavDirection::Right)
                                    } else {
                                        None
                                    };

                                    if let Some(direction) = direction {
                                        this.navigate_emoji_picker(direction, cx);
                                        cx.stop_propagation();
                                    }
                                }
                                _ => {
                                    // Don't intercept arrows for other views (let normal handling work)
                                }
                            }
                        });
                    }
                }
            }
        });
        app.gpui_input_subscriptions.push(arrow_interceptor);

        // Add Home/End/PageUp/PageDown key interceptor for jump navigation
        let app_entity_for_home_end = cx.entity().downgrade();
        let home_end_interceptor = cx.intercept_keystrokes({
            let app_entity = app_entity_for_home_end;
            move |event, window, cx| {
                // When the main window is hidden (e.g. Notes/AI open), main-menu
                // key interceptors must not consume keystrokes from secondary windows.
                if !script_kit_gpui::is_main_window_visible() {
                    return;
                }

                // Skip processing if this keystroke is from a secondary window.
                let is_notes = crate::notes::is_notes_window(window);
                let is_detached_agent_chat = crate::ai::agent_chat::ui::chat_window::is_chat_window(window);
                let is_actions = crate::actions::is_actions_window(window);
                let skip_secondary = is_notes || is_detached_agent_chat || is_actions;
                if skip_secondary {
                    tracing::debug!(
                        target: "script_kit::keyboard",
                        event = "home_end_interceptor_skipped_secondary_window",
                        is_notes,
                        is_detached_agent_chat,
                        is_actions,
                        skip_secondary,
                        detached_agent_chat_explicit_skip = is_detached_agent_chat,
                    );
                    return;
                }

                let key = event.keystroke.key.as_str();
                let has_platform_mod = event.keystroke.modifiers.platform; // Cmd on macOS

                if confirm::consume_main_window_key_while_confirm_open(
                    key,
                    &event.keystroke.modifiers,
                    cx,
                ) {
                    cx.stop_propagation();
                    return;
                }

                // Home key or Cmd+Up → jump to first item
                // End key or Cmd+Down → jump to last item
                let is_home = key.eq_ignore_ascii_case("home")
                    || (has_platform_mod && crate::ui_foundation::is_key_up(key));
                let is_end = key.eq_ignore_ascii_case("end")
                    || (has_platform_mod && crate::ui_foundation::is_key_down(key));
                // Page Up/Down → move by ~10 selectable items
                let is_page_up = key.eq_ignore_ascii_case("pageup");
                let is_page_down = key.eq_ignore_ascii_case("pagedown");

                if !is_home && !is_end && !is_page_up && !is_page_down {
                    return;
                }

                if let Some(app) = app_entity.upgrade() {
                    app.update(cx, |this, cx| {
                        // Only handle in ScriptList view
                        if !matches!(this.current_view, AppView::ScriptList) {
                            return;
                        }

                        // Don't handle if actions popup is open
                        if this.show_actions_popup {
                            return;
                        }

                        if is_home {
                            this.move_selection_to_first(cx);
                        } else if is_end {
                            this.move_selection_to_last(cx);
                        } else if is_page_up {
                            this.move_selection_page_up(cx);
                        } else if is_page_down {
                            this.move_selection_page_down(cx);
                        }

                        cx.stop_propagation();
                    });
                }
            }
        });
        app.gpui_input_subscriptions.push(home_end_interceptor);
