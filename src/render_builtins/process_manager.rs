#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessManagerTerminateAction {
    StopSelectedProcess,
}

impl ProcessManagerTerminateAction {
    fn success_hud(self, script_name: &str) -> String {
        match self {
            Self::StopSelectedProcess => format!("Stopped {script_name}"),
        }
    }

    fn failure_message(self, pid: u32, error: &str) -> String {
        match self {
            Self::StopSelectedProcess => format!("Failed to stop PID {pid}: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessManagerEmptyState {
    NoRunningScripts,
    NoFilteredMatches,
}

impl ProcessManagerEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoRunningScripts
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoRunningScripts => "No running scripts",
            Self::NoFilteredMatches => "No processes match your filter",
        }
    }
}

impl ScriptListApp {
    /// Refresh interval for the process manager list (2 seconds).
    const PROCESS_MANAGER_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

    fn process_manager_stop_all_entry() -> crate::builtins::BuiltInEntry {
        crate::builtins::BuiltInEntry {
            id: crate::config::canonical_builtin_command_id("builtin/stop-all-processes"),
            name: "Stop All Running Scripts".to_string(),
            description: "Terminate every active Script Kit child process".to_string(),
            keywords: vec![
                "process".to_string(),
                "running".to_string(),
                "scripts".to_string(),
                "stop".to_string(),
                "kill".to_string(),
                "terminate".to_string(),
                "jobs".to_string(),
            ],
            feature: crate::builtins::BuiltInFeature::UtilityCommand(
                crate::builtins::UtilityCommandType::StopAllProcesses,
            ),
            icon: Some("square-stop".to_string()),
            group: crate::builtins::BuiltInGroup::Core,
        }
    }

    fn trigger_process_manager_stop_all(&mut self, cx: &mut Context<Self>) {
        let entry = Self::process_manager_stop_all_entry();
        self.execute_builtin(&entry, cx);
    }

    fn process_manager_filter_matches(
        process: &crate::process_manager::ProcessInfo,
        filter_lower: &str,
    ) -> bool {
        process.script_path.to_lowercase().contains(filter_lower)
            || process.pid.to_string().contains(filter_lower)
    }

    fn process_manager_filtered_entries<'a>(
        processes: &'a [crate::process_manager::ProcessInfo],
        filter: &str,
    ) -> Vec<(usize, &'a crate::process_manager::ProcessInfo)> {
        if filter.is_empty() {
            processes.iter().enumerate().collect()
        } else {
            let filter_lower = filter.to_lowercase();
            processes
                .iter()
                .enumerate()
                .filter(|(_, process)| Self::process_manager_filter_matches(process, &filter_lower))
                .collect()
        }
    }

    fn process_manager_visible_row_names(&self, filter: &str) -> Vec<String> {
        Self::process_manager_filtered_entries(&self.cached_processes, filter)
            .into_iter()
            .map(|(_, process)| process.script_path.clone())
            .collect()
    }

    fn process_manager_dataset_and_visible_counts(&self, filter: &str) -> (usize, usize) {
        (
            self.cached_processes.len(),
            Self::process_manager_filtered_entries(&self.cached_processes, filter).len(),
        )
    }

    fn process_manager_selected_visible_entry(
        &self,
        filter: &str,
        selected_index: usize,
    ) -> Option<(usize, &crate::process_manager::ProcessInfo)> {
        Self::process_manager_filtered_entries(&self.cached_processes, filter)
            .get(selected_index)
            .copied()
    }

    fn process_manager_selected_visible_row_name(
        &self,
        filter: &str,
        selected_index: usize,
    ) -> Option<String> {
        self.process_manager_selected_visible_entry(filter, selected_index)
            .map(|(_, process)| process.script_path.clone())
    }

    fn process_manager_visible_target_rows(
        &self,
        filter: &str,
        limit: usize,
    ) -> Vec<(usize, usize, &crate::process_manager::ProcessInfo)> {
        Self::process_manager_filtered_entries(&self.cached_processes, filter)
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(display_index, (source_index, process))| (display_index, source_index, process))
            .collect()
    }

    fn process_manager_count_label(total_count: usize) -> String {
        let suffix = if total_count == 1 { "" } else { "es" };
        format!("{} process{}", total_count, suffix)
    }

    /// Start the periodic refresh task for the ProcessManagerView.
    ///
    /// Spawns a background timer that polls the process manager every 2 seconds
    /// and updates `cached_processes` only when the list has changed.
    /// The task is stored in `process_manager_refresh_task` and is automatically
    /// cancelled when the field is overwritten or set to None.
    fn start_process_manager_refresh(&mut self, cx: &mut Context<Self>) {
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(ScriptListApp::PROCESS_MANAGER_REFRESH_INTERVAL)
                    .await;

                let should_continue = cx
                    .update(|cx| {
                        this.update(cx, |app, cx| {
                            // Bail if we've navigated away
                            if !matches!(app.current_view, AppView::ProcessManagerView { .. }) {
                                tracing::info!(
                                    correlation_id = "process-manager-refresh",
                                    "process_manager.refresh_stopped.view_changed"
                                );
                                app.process_manager_refresh_task = None;
                                return false;
                            }

                            let new_processes = crate::process_manager::PROCESS_MANAGER
                                .get_active_processes_sorted();

                            // Compare by PIDs to avoid unnecessary rerenders
                            let old_pids: Vec<u32> =
                                app.cached_processes.iter().map(|p| p.pid).collect();
                            let new_pids: Vec<u32> = new_processes.iter().map(|p| p.pid).collect();

                            if old_pids != new_pids {
                                tracing::info!(
                                    correlation_id = "process-manager-refresh",
                                    old_count = old_pids.len(),
                                    new_count = new_pids.len(),
                                    "process_manager.refresh.list_changed"
                                );

                                app.cached_processes = new_processes;

                                // Clamp selection index against the visible filtered rows.
                                if let AppView::ProcessManagerView {
                                    filter,
                                    selected_index,
                                } = &mut app.current_view
                                {
                                    let visible_len = Self::process_manager_filtered_entries(
                                        &app.cached_processes,
                                        filter,
                                    )
                                    .len();
                                    if visible_len == 0 {
                                        *selected_index = 0;
                                    } else if *selected_index >= visible_len {
                                        *selected_index = visible_len - 1;
                                    }
                                    app.process_list_scroll_handle
                                        .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                }

                                cx.notify();
                            }

                            true
                        })
                    })
                    .unwrap_or(false);

                if !should_continue {
                    break;
                }
            }
        });

        self.process_manager_refresh_task = Some(task);
        tracing::info!(
            correlation_id = "process-manager-refresh",
            interval_secs = 2,
            "process_manager.refresh_started"
        );
    }

    /// Stop the periodic refresh task for ProcessManagerView.
    fn stop_process_manager_refresh(&mut self) {
        if self.process_manager_refresh_task.take().is_some() {
            tracing::info!(
                correlation_id = "process-manager-refresh",
                "process_manager.refresh_stopped"
            );
        }
    }

    /// Render process manager view showing running background scripts
    fn render_process_manager(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal(
                "process_manager",
                2,     // hints: "↵ Stop", "Esc Back"
                false, // no leading status text
                false, // no actions hint
            ),
        );
        let tokens = get_tokens(self.current_design);
        let _design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let _design_typography = tokens.typography();
        let color_resolver =
            crate::theme::ColorResolver::new_for_shell(&self.theme, self.current_design);
        let typography_resolver =
            crate::theme::TypographyResolver::new_theme_first(&self.theme, self.current_design);
        let empty_text_color = color_resolver.empty_text_color();
        let empty_font_family = typography_resolver.primary_font().to_string();

        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let text_primary = chrome.text_primary_hex;

        // Filter processes from cached data
        let filtered_processes =
            Self::process_manager_filtered_entries(&self.cached_processes, &filter);
        let filtered_len = filtered_processes.len();

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

                // ESC: Clear filter first if present, otherwise go back/close
                if is_key_escape(key) && !this.show_actions_popup {
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
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

                if has_cmd && is_key_enter(key) {
                    this.trigger_process_manager_stop_all(cx);
                    cx.stop_propagation();
                    return;
                }

                // Extract current view state without holding mutable borrow
                let view_state = if let AppView::ProcessManagerView {
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

                // Compute filtered list from cached data
                let filtered =
                    Self::process_manager_filtered_entries(&this.cached_processes, &current_filter);
                let current_filtered_len = filtered.len();

                if is_key_up(key) {
                    if current_selected > 0 {
                        if let AppView::ProcessManagerView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected - 1;
                            this.process_list_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_down(key) {
                    if current_selected < current_filtered_len.saturating_sub(1) {
                        if let AppView::ProcessManagerView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected + 1;
                            this.process_list_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_enter(key) {
                    // Kill selected process
                    if let Some((_, process_info)) = filtered.get(current_selected) {
                        let pid = process_info.pid;
                        let script_path = process_info.script_path.clone();
                        let terminate_action = ProcessManagerTerminateAction::StopSelectedProcess;

                        tracing::info!(
                            correlation_id = "process-manager-terminate",
                            pid,
                            script_path = script_path.as_str(),
                            "process_manager.terminate_selected"
                        );

                        match crate::process_manager::PROCESS_MANAGER.terminate_process(pid) {
                            Ok(()) => {
                                // Refresh the cached processes
                                this.cached_processes = crate::process_manager::PROCESS_MANAGER
                                    .get_active_processes_sorted();

                                let script_name = std::path::Path::new(&script_path)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("process");

                                this.show_hud(
                                    terminate_action.success_hud(script_name),
                                    Some(HUD_SHORT_MS),
                                    cx,
                                );

                                // Clamp selection if the visible filtered list got shorter.
                                let new_len = this.cached_processes.len();
                                if let AppView::ProcessManagerView { selected_index, .. } =
                                    &mut this.current_view
                                {
                                    if *selected_index >= new_len && new_len > 0 {
                                        *selected_index = new_len - 1;
                                    }
                                    let visible_len = Self::process_manager_filtered_entries(
                                        &this.cached_processes,
                                        &current_filter,
                                    )
                                    .len();
                                    if visible_len == 0 {
                                        *selected_index = 0;
                                    } else if *selected_index >= visible_len {
                                        *selected_index = visible_len - 1;
                                    }
                                    this.process_list_scroll_handle
                                        .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                }

                                if new_len == 0 {
                                    this.go_back_or_close(window, cx);
                                    return;
                                }

                                cx.notify();
                            }
                            Err(err_msg) => {
                                tracing::warn!(
                                    pid,
                                    error = err_msg.as_str(),
                                    "process_manager.terminate_failed"
                                );
                                this.show_error_toast(
                                    terminate_action.failure_message(pid, &err_msg),
                                    cx,
                                );
                            }
                        }
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        // Pre-compute colors
        let list_colors = ListItemColors::from_theme(&self.theme);

        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            let state = ProcessManagerEmptyState::from_filter(&filter);
            crate::list_item::EmptyState::new(state.message(), empty_text_color, &empty_font_family)
                .icon(crate::designs::icon_variations::IconName::Terminal)
                .into_element()
        } else {
            let processes_for_closure: Vec<_> = filtered_processes
                .iter()
                .map(|(i, p)| (*i, (*p).clone()))
                .collect();
            let selected = selected_index;
            let hovered = self.hovered_index;
            let click_entity_handle = cx.entity().downgrade();
            let hover_entity_handle = cx.entity().downgrade();

            uniform_list(
                "process-manager",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((_, process_info)) = processes_for_closure.get(ix) {
                                let is_selected = ix == selected;
                                let is_hovered = hovered == Some(ix);

                                // Show script name as primary, PID and path as description
                                let script_name = std::path::Path::new(&process_info.script_path)
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or(&process_info.script_path);
                                let name = script_name.to_string();

                                let duration_str = crate::formatting::format_running_duration(
                                    chrono::Utc::now(),
                                    process_info.started_at,
                                );

                                let description =
                                    format!("PID {} • running {}", process_info.pid, duration_str);

                                let click_entity = click_entity_handle.clone();
                                let click_handler =
                                    move |_event: &gpui::ClickEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        cx.stop_propagation();
                                        if let Some(app) = click_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                if let AppView::ProcessManagerView {
                                                    selected_index,
                                                    ..
                                                } = &mut this.current_view
                                                {
                                                    *selected_index = ix;
                                                }
                                                cx.notify();
                                            });
                                        }
                                    };

                                let hover_entity = hover_entity_handle.clone();
                                let hover_handler =
                                    move |is_hovered: &bool,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = hover_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                if *is_hovered {
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

                                div()
                                    .id(ix)
                                    .cursor_pointer()
                                    .when(
                                        crate::list_item::LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED,
                                        |row| {
                                            row.tooltip(|window, cx| {
                                                gpui_component::tooltip::Tooltip::new(
                                                    "Stop this process",
                                                )
                                                .key_binding(
                                                    gpui::Keystroke::parse("enter")
                                                        .ok()
                                                        .map(gpui_component::kbd::Kbd::new),
                                                )
                                                .build(window, cx)
                                            })
                                        },
                                    )
                                    .on_click(click_handler)
                                    .on_hover(hover_handler)
                                    .child(
                                        ListItem::new(name, list_colors)
                                            .description_opt(Some(description))
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
            .track_scroll(&self.process_list_scroll_handle)
            .into_any_element()
        };

        let total_count = self.cached_processes.len();
        let list_scrollbar =
            self.builtin_uniform_list_scrollbar(&self.process_list_scroll_handle, filtered_len, 8);
        let stop_all_button_colors = crate::components::ButtonColors::from_theme(&self.theme);
        let stop_all_button_entity = cx.entity().downgrade();

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            .py(px(design_spacing.padding_xs))
            .child(
                // Every list leads with a persistent section separator
                // (POLISH.md layout-stability bar; same rule as the main
                // menu's "Results" header, 4d76327b8): the label may swap but
                // the row never appears or disappears, so filtering can't
                // shift the rows below it.
                crate::list_item::render_section_header(
                    if filter.trim().is_empty() {
                        "Processes"
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
                    .w_full()
                    .flex_1()
                    .min_h(px(0.))
                    .on_scroll_wheel(cx.listener(
                        move |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                                    let view_state = if let AppView::ProcessManagerView {
                                        filter,
                                        selected_index,
                                    } = &this.current_view
                                    {
                                        Some((filter.clone(), *selected_index))
                                    } else {
                                        None
                                    };
                                    let Some((current_filter, current_selected)) = view_state
                                    else {
                                        return;
                                    };
                                    let filtered_len = Self::process_manager_filtered_entries(
                                        &this.cached_processes,
                                        &current_filter,
                                    )
                                    .len();
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
                                    if let AppView::ProcessManagerView { selected_index, .. } =
                                        &mut this.current_view
                                    {
                                        *selected_index = new_selected;
                                    }
                                    this.process_list_scroll_handle
                                        .scroll_to_item(new_selected, ScrollStrategy::Nearest);
                                    this.note_builtin_selection_owned_wheel_scroll(new_selected);
                                    cx.notify();
                                    cx.stop_propagation();
                        },
                    ))
                    .child(list_element)
                    .child(list_scrollbar),
            );

        let mut trailing = vec![self
            .render_builtin_main_input_count_label(Self::process_manager_count_label(total_count))];
        if total_count > 0 {
            let stop_all_button_entity = stop_all_button_entity.clone();
            trailing.push(
                crate::components::Button::new("Stop All", stop_all_button_colors)
                    .variant(crate::components::ButtonVariant::Ghost)
                    .shortcut("⌘↵")
                    .on_click(Box::new(move |_event, _window, cx| {
                        cx.stop_propagation();
                        if let Some(app) = stop_all_button_entity.upgrade() {
                            app.update(cx, |this, cx| {
                                this.trigger_process_manager_stop_all(cx);
                            });
                        }
                    }))
                    .into_any_element(),
            );
        }
        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            if total_count > 0 {
                vec![
                    gpui::SharedString::from("↵ Stop"),
                    gpui::SharedString::from("⌘↵ Stop All"),
                    gpui::SharedString::from("Esc Back"),
                ]
            } else {
                vec![gpui::SharedString::from("Esc Back")]
            },
            None,
        ));
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(self.theme_font_family())
                .key_context("process_manager")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(trailing, cx),
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

#[cfg(test)]
mod process_manager_chrome_audit {
    #[test]
    fn process_manager_uses_minimal_chrome_footer() {
        let source = include_str!("process_manager.rs");
        assert!(
            source.contains("render_simple_hint_strip("),
            "process_manager should use render_simple_hint_strip"
        );
        assert!(
            source.contains("render_builtin_main_input_header(")
                && source.contains("render_builtin_main_input_count_label("),
            "process_manager should use the shared built-in main input header"
        );
        assert!(
            !source.contains(&["SectionDivider", "::new()"].concat()),
            "process_manager should use the shared main-view divider contract"
        );
        assert!(
            !source.contains(&["HEADER_PADDING", "_X"].concat())
                && !source.contains(&["HEADER_PADDING", "_Y"].concat()),
            "process_manager should not hardcode local main input header padding"
        );
        assert!(
            source.contains(".shortcut(\"⌘↵\")")
                && source.contains("trigger_process_manager_stop_all(cx)"),
            "process_manager should preserve the Stop All header action and keyboard route"
        );
        let legacy = "Prompt".to_owned() + "Footer::new(";
        assert_eq!(
            source.matches(&legacy).count(),
            0,
            "process_manager should not use PromptFooter"
        );
    }
}
