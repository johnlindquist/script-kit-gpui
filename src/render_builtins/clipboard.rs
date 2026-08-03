#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardHistoryPasteAction {
    PasteSelectedEntry,
}

impl ClipboardHistoryPasteAction {
    fn copy_attempt_log(self, entry_id: &str) -> String {
        match self {
            Self::PasteSelectedEntry => format!("Copying clipboard entry: {entry_id}"),
        }
    }

    fn copy_failure_log(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::PasteSelectedEntry => format!("Failed to copy entry: {error}"),
        }
    }

    fn paste_failure_log(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::PasteSelectedEntry => format!("Failed to simulate paste: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardHistoryEmptyState {
    NoHistory,
    NoFilteredMatches,
}

impl ClipboardHistoryEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoHistory
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoHistory => "No clipboard history",
            Self::NoFilteredMatches => "No entries match your filter",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClipboardHistoryFilterQuery {
    content_types: Vec<clipboard_history::ContentType>,
    text: String,
    has_structured_filter: bool,
}

impl ClipboardHistoryFilterQuery {
    fn parse(raw: &str) -> Self {
        let raw_lower = raw.to_lowercase();
        let tokens: Vec<&str> = raw_lower.split_whitespace().collect();
        let mut content_types = Vec::new();
        let mut text_tokens = Vec::new();
        let mut has_structured_filter = false;
        let mut index = 0;

        while index < tokens.len() {
            let token = tokens[index];
            if let Some(value) = token
                .strip_prefix("type:")
                .or_else(|| token.strip_prefix("kind:"))
            {
                let (value, consumed_next) = if value.is_empty() {
                    match tokens.get(index + 1) {
                        Some(next) => (*next, true),
                        None => ("", false),
                    }
                } else {
                    (value, false)
                };

                if let Some(content_type) = Self::parse_content_type(value) {
                    if !content_types.contains(&content_type) {
                        content_types.push(content_type);
                    }
                    has_structured_filter = true;
                    index += if consumed_next { 2 } else { 1 };
                    continue;
                }
            }

            text_tokens.push(token);
            index += 1;
        }

        let text = if has_structured_filter {
            text_tokens.join(" ")
        } else {
            raw_lower
        };

        Self {
            content_types,
            text,
            has_structured_filter,
        }
    }

    fn parse_content_type(value: &str) -> Option<clipboard_history::ContentType> {
        match value {
            "text" | "texts" => Some(clipboard_history::ContentType::Text),
            "image" | "images" => Some(clipboard_history::ContentType::Image),
            "link" | "links" | "url" | "urls" => Some(clipboard_history::ContentType::Link),
            "file" | "files" => Some(clipboard_history::ContentType::File),
            "color" | "colors" => Some(clipboard_history::ContentType::Color),
            _ => None,
        }
    }

    fn is_empty(&self) -> bool {
        !self.has_structured_filter && self.text.is_empty()
    }

    fn matches(&self, entry: &clipboard_history::ClipboardEntryMeta) -> bool {
        let type_matches =
            self.content_types.is_empty() || self.content_types.contains(&entry.content_type);
        let text_matches =
            self.text.is_empty() || Self::entry_text_matches(entry, self.text.as_str());

        type_matches && text_matches
    }

    fn entry_text_matches(
        entry: &clipboard_history::ClipboardEntryMeta,
        filter_lower: &str,
    ) -> bool {
        entry.text_preview.to_lowercase().contains(filter_lower)
            || entry
                .ocr_text
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(filter_lower)
    }
}

impl ScriptListApp {
    pub(crate) fn clipboard_history_visible_rows_for_entries(
        entries: &[clipboard_history::ClipboardEntryMeta],
        filter: &str,
    ) -> Vec<(usize, clipboard_history::ClipboardEntryMeta)> {
        let query = ClipboardHistoryFilterQuery::parse(filter);
        if query.is_empty() {
            entries.iter().cloned().enumerate().collect()
        } else {
            entries
                .iter()
                .cloned()
                .enumerate()
                .filter(|(_, entry)| query.matches(entry))
                .collect()
        }
    }

    fn clipboard_history_visible_rows(
        &self,
        filter: &str,
    ) -> Vec<(usize, clipboard_history::ClipboardEntryMeta)> {
        Self::clipboard_history_visible_rows_for_entries(&self.cached_clipboard_entries, filter)
    }

    fn clipboard_history_selected_visible_row(
        &self,
        filter: &str,
        selected_index: usize,
    ) -> Option<(usize, clipboard_history::ClipboardEntryMeta)> {
        self.clipboard_history_visible_rows(filter)
            .get(selected_index)
            .cloned()
    }

    fn clipboard_history_dataset_and_visible_counts(&self, filter: &str) -> (usize, usize) {
        (
            self.cached_clipboard_entries.len(),
            self.clipboard_history_visible_rows(filter).len(),
        )
    }

    fn clipboard_history_visible_row_labels(&self, filter: &str) -> Vec<String> {
        self.clipboard_history_visible_rows(filter)
            .into_iter()
            .map(|(_, entry)| entry.text_preview)
            .collect()
    }

    /// Render clipboard history view
    /// P0 FIX: Data comes from self.cached_clipboard_entries, view passes only state
    fn render_clipboard_history(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("clipboard_history", false),
        );
        // Use theme for all colors - consistent with main menu
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        // Use theme colors for consistency with main menu
        let opacity = self.theme.get_opacity();
        let bg_hex = self.theme.colors.background.main;
        let _bg_with_alpha = crate::ui_foundation::hex_to_rgba_with_opacity(bg_hex, opacity.main);
        // Removed: box_shadows - shadows on transparent elements block vibrancy
        let _box_shadows = self.create_box_shadows();

        // P0 FIX: Reference data from self instead of taking ownership
        // P1 FIX: NEVER do synchronous SQLite queries or image decoding in render loop!
        // Only copy from global cache (populated async by background prewarm thread).
        // Images not yet cached will show placeholder with dimensions from metadata.
        for entry in &self.cached_clipboard_entries {
            if entry.content_type == clipboard_history::ContentType::Image {
                // Only use already-cached images - NO synchronous fetch/decode
                if !self.clipboard_image_cache.contains_key(&entry.id) {
                    if let Some(cached) = clipboard_history::get_cached_image(&entry.id) {
                        self.clipboard_image_cache.insert(entry.id.clone(), cached);
                    }
                    // If not in global cache yet, background thread will populate it.
                    // We'll show placeholder with dimensions until then.
                }
            }
        }

        // Clone the cache for use in closures
        let image_cache = self.clipboard_image_cache.clone();

        // Filter entries based on current filter
        let filtered_entries = self.clipboard_history_visible_rows(&filter);
        let filtered_len = filtered_entries.len();

        // Key handler for clipboard history
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                // Hide cursor while typing - automatically shows when mouse moves
                this.hide_mouse_cursor(cx);

                // If the shortcut recorder is active, don't process any key events.
                // The recorder has its own key handlers and should receive all key events.
                if this.shortcut_recorder_state.is_some() {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let key_char = event.keystroke.key_char.as_deref();
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;

                // Route keys to actions dialog first if it's open
                match this.route_key_to_actions_dialog(
                    key,
                    key_char,
                    modifiers,
                    ActionsDialogHost::ClipboardHistory,
                    window,
                    cx,
                ) {
                    ActionsRoute::NotHandled => {}
                    ActionsRoute::Handled => {
                        return;
                    }
                    ActionsRoute::Execute {
                        action_id,
                        should_close,
                    } => {
                        this.execute_actions_route_action(
                            ActionsDialogHost::ClipboardHistory,
                            action_id,
                            should_close,
                            window,
                            cx,
                        );
                        return;
                    }
                }

                // ESC: In portal mode, cancel and return to Agent Chat chat.
                // Otherwise, clear filter first; if empty, go back/close.
                if is_key_escape(key) && !this.show_actions_popup {
                    if this.is_in_attachment_portal() {
                        this.close_attachment_portal_cancel(cx);
                        return;
                    }
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    return;
                }

                // Cmd+W always closes window
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    logging::log("KEY", "Cmd+W - closing window");
                    this.close_and_reset_window(cx);
                    return;
                }

                logging::log("KEY", &format!("ClipboardHistory key: '{}'", key));

                let in_portal = this.is_in_attachment_portal();

                // P0 FIX: View state only - data comes from this.cached_clipboard_entries
                let view_state = if let AppView::ClipboardHistoryView {
                    filter,
                    selected_index,
                } = &this.current_view
                {
                    Some((filter.clone(), *selected_index))
                } else {
                    None
                };

                if let Some((current_filter, current_selected)) = view_state {
                    let filtered_entries = this.clipboard_history_visible_rows(&current_filter);
                    let filtered_len = filtered_entries.len();
                    let selected_entry = filtered_entries
                        .get(current_selected)
                        .map(|(_, entry)| entry.clone());
                    this.focused_clipboard_entry_id =
                        selected_entry.as_ref().map(|entry| entry.id.clone());

                    // Cmd+P toggles pin state for selected entry
                    if has_cmd && key.eq_ignore_ascii_case("p") {
                        if let Some(entry) = selected_entry {
                            drop(filtered_entries);
                            let action_id = if entry.pinned {
                                "clipboard_unpin"
                            } else {
                                "clipboard_pin"
                            };
                            this.handle_action(action_id.to_string(), window, cx);
                        }
                        return;
                    }

                    // Cmd+K opens clipboard actions dialog
                    if has_cmd && key.eq_ignore_ascii_case("k") {
                        if let Some(entry) = selected_entry {
                            drop(filtered_entries);
                            this.toggle_clipboard_actions(entry, window, cx);
                        }
                        return;
                    }

                    // Ctrl+Cmd+A attaches selected entry to AI chat
                    if modifiers.control && has_cmd && key.eq_ignore_ascii_case("a") {
                        if let Some(_entry) = selected_entry {
                            drop(filtered_entries);
                            this.handle_action("clipboard_attach_to_ai".to_string(), window, cx);
                        }
                        return;
                    }

                    // Space opens Quick Look (macOS Finder behavior)
                    if is_key_space(key)
                        && current_filter.is_empty()
                        && !modifiers.platform
                        && !modifiers.control
                        && !modifiers.alt
                        && !modifiers.shift
                    {
                        if let Some(entry) = selected_entry {
                            if let Err(e) = clipboard_history::quick_look_entry(&entry) {
                                logging::log("ERROR", &format!("Quick Look failed: {}", e));
                                this.show_error_toast(format!("Quick Look failed: {}", e), cx);
                            }
                        }
                        return;
                    }

                    match key {
                        _ if is_key_up(key) => {
                            if filtered_len > 0 {
                                let new_selected = current_selected.saturating_sub(1);
                                if let AppView::ClipboardHistoryView { selected_index, .. } =
                                    &mut this.current_view
                                {
                                    *selected_index = new_selected;
                                }
                                // Scroll to keep selection visible
                                this.clipboard_list_scroll_handle
                                    .scroll_to_item(new_selected, ScrollStrategy::Nearest);
                                this.focused_clipboard_entry_id = filtered_entries
                                    .get(new_selected)
                                    .map(|(_, entry)| entry.id.clone());
                                cx.notify();
                            }
                        }
                        _ if is_key_down(key) => {
                            if filtered_len > 0 {
                                let new_selected = (current_selected + 1)
                                    .min(filtered_len.saturating_sub(1));
                                if let AppView::ClipboardHistoryView { selected_index, .. } =
                                    &mut this.current_view
                                {
                                    *selected_index = new_selected;
                                }
                                // Scroll to keep selection visible
                                this.clipboard_list_scroll_handle
                                    .scroll_to_item(new_selected, ScrollStrategy::Nearest);
                                this.focused_clipboard_entry_id = filtered_entries
                                    .get(new_selected)
                                    .map(|(_, entry)| entry.id.clone());
                                cx.notify();
                            }
                        }
                        _ if has_cmd && is_key_enter(key) => {
                            if filtered_entries.get(current_selected).is_some() {
                                drop(filtered_entries);
                                this.handle_action(
                                    "clipboard_attach_to_ai".to_string(),
                                    window,
                                    cx,
                                );
                            }
                        }
                        _ if is_key_enter(key) => {
                            // Portal mode: attach the selected entry's content to Agent Chat chat.
                            if in_portal {
                                if let Some((_, entry)) = filtered_entries.get(current_selected) {
                                    // `cap_preview` floors the cut to a char
                                    // boundary; `&entry.text_preview[..40]` panicked
                                    // on multibyte clipboard content at byte 40.
                                    let label = format!(
                                        "Clipboard: {}",
                                        crate::context_snapshot::launcher_context::cap_preview(
                                            &entry.text_preview,
                                            40
                                        )
                                    );
                                    let part =
                                        crate::ai::message_parts::AiContextPart::ResourceUri {
                                            uri: format!("kit://clipboard-history?id={}", entry.id),
                                            label,
                                        };
                                    this.close_attachment_portal_with_part(part, cx);
                                }
                                cx.stop_propagation();
                                return;
                            }

                            // Copy selected entry to clipboard, hide window, then paste
                            if let Some((_, entry)) = filtered_entries.get(current_selected) {
                                let paste_action = ClipboardHistoryPasteAction::PasteSelectedEntry;
                                logging::log("EXEC", &paste_action.copy_attempt_log(&entry.id));
                                if let Err(e) =
                                    clipboard_history::copy_entry_to_clipboard(&entry.id)
                                {
                                    logging::log("ERROR", &paste_action.copy_failure_log(e));
                                } else {
                                    logging::log("EXEC", "Entry copied to clipboard");
                                    this.hide_main_and_reset(cx);

                                    // Simulate Cmd+V paste after a brief delay to let focus return
                                    std::thread::spawn(move || {
                                        std::thread::sleep(std::time::Duration::from_millis(100));
                                        if let Err(e) = selected_text::simulate_paste_with_cg() {
                                            logging::log(
                                                "ERROR",
                                                &paste_action.paste_failure_log(e),
                                            );
                                        } else {
                                            logging::log("EXEC", "Simulated Cmd+V paste");
                                        }
                                    });
                                }
                            }
                        }
                        // Note: "escape" is handled by handle_global_shortcut_with_options above
                        // Text input (backspace, characters) is handled by the shared Input component
                        // which syncs via handle_filter_input_change()
                        _ => {}
                    }
                }
            },
        );

        // Pre-compute colors - use theme for consistency with main menu
        let list_colors = ListItemColors::from_theme(&self.theme);
        let text_primary = self.theme.colors.text.primary;
        #[allow(unused_variables)]
        let text_muted = self.theme.colors.text.muted;

        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            let state = ClipboardHistoryEmptyState::from_filter(&filter);
            crate::components::render_simple_empty_state(
                "clipboard-history-empty",
                state.message(),
                "copy",
                None,
                &self.theme,
                cx,
            )
        } else {
            // Clone data for the closure
            let entries_for_closure: Vec<_> = filtered_entries
                .iter()
                .map(|(i, e)| (*i, (*e).clone()))
                .collect();
            let selected = selected_index;
            let hovered = self.hovered_index;
            let image_cache_for_list = image_cache.clone();
            let click_entity_handle = cx.entity().downgrade();
            let hover_entity_handle = cx.entity().downgrade();

            uniform_list(
                "clipboard-history",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((_, entry)) = entries_for_closure.get(ix) {
                                let is_selected = ix == selected;
                                let is_hovered = hovered == Some(ix);

                                // Get cached thumbnail for images
                                let cached_image = if entry.content_type
                                    == clipboard_history::ContentType::Image
                                {
                                    image_cache_for_list.get(&entry.id).cloned()
                                } else {
                                    None
                                };

                                // Use display_preview() from ClipboardEntryMeta
                                let display_content = entry.display_preview();

                                let relative_time =
                                    crate::formatting::format_relative_time_short_millis(
                                        entry.timestamp,
                                    );

                                // Add pin indicator
                                let name = if entry.pinned {
                                    let mut s = String::with_capacity("📌 ".len() + display_content.len());
                                    s.push_str("📌 ");
                                    s.push_str(&display_content);
                                    s
                                } else {
                                    display_content
                                };

                                // Build list item with optional thumbnail
                                let mut item = ListItem::new(name, list_colors)
                                    .description_opt(Some(relative_time))
                                    .selected(is_selected)
                                    .hovered(is_hovered);

                                // Add thumbnail for images, text icon for text entries
                                if let Some(render_image) = cached_image {
                                    item = item.icon_image(render_image);
                                } else if entry.content_type == clipboard_history::ContentType::Text
                                {
                                    item = item.icon("📄");
                                }

                                // Click handler: select on click, paste on double-click
                                let click_entity = click_entity_handle.clone();
                                let entry_id = entry.id.clone();
                                let click_handler = move |event: &gpui::ClickEvent,
                                                           _window: &mut Window,
                                                           cx: &mut gpui::App| {
                                    if let Some(app) = click_entity.upgrade() {
                                        let entry_id = entry_id.clone();
                                        app.update(cx, |this, cx| {
                                            if let AppView::ClipboardHistoryView {
                                                selected_index, ..
                                            } = &mut this.current_view
                                            {
                                                *selected_index = ix;
                                            }
                                            this.focused_clipboard_entry_id =
                                                Some(entry_id.clone());
                                            this.clipboard_list_scroll_handle
                                                .scroll_to_item(ix, ScrollStrategy::Nearest);
                                            this.note_list_pointer_click(ix, cx);

                                            // Double-click: copy and paste
                                            if let gpui::ClickEvent::Mouse(mouse_event) = event {
                                                if mouse_event.down.click_count == 2 {
                                                    logging::log(
                                                        "UI",
                                                        &format!(
                                                            "Double-click paste clipboard entry {}",
                                                            entry_id
                                                        ),
                                                    );
                                                    if clipboard_history::copy_entry_to_clipboard(
                                                        &entry_id,
                                                    )
                                                    .is_ok()
                                                    {
                                                        this.hide_main_and_reset(cx);
                                                        std::thread::spawn(|| {
                                                            std::thread::sleep(
                                                                std::time::Duration::from_millis(
                                                                    100,
                                                                ),
                                                            );
                                                            let _ = selected_text::simulate_paste_with_cg();
                                                        });
                                                    }
                                                }
                                            }
                                        });
                                    }
                                };

                                // Hover handler for mouse tracking
                                let hover_entity = hover_entity_handle.clone();
                                let move_entity = hover_entity_handle.clone();
                                let move_handler = move |_event: &gpui::MouseMoveEvent,
                                                         _window: &mut Window,
                                                         cx: &mut gpui::App| {
                                    if let Some(app) = move_entity.upgrade() {
                                        app.update(cx, |this, cx| {
                                            this.note_list_pointer_move(ix, cx);
                                        });
                                    }
                                };
                                let hover_handler = move |is_hovered: &bool, _window: &mut Window, cx: &mut gpui::App| {
                                    if let Some(app) = hover_entity.upgrade() {
                                        app.update(cx, |this, cx| {
                                            if !*is_hovered {
                                                this.note_list_pointer_leave(ix, cx);
                                            }
                                        });
                                    }
                                };

                                div()
                                    .id(format!("clipboard-history-row:{}", entry.id))
                                    .cursor_pointer()
                                    .when(
                                        crate::list_item::LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED,
                                        |row| {
                                            row.tooltip(|window, cx| {
                                                gpui_component::tooltip::Tooltip::new(
                                                    "Paste selected clipboard entry",
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
                                    .on_mouse_move(move_handler)
                                    .on_hover(hover_handler)
                                    .child(item.semantic_id(format!(
                                        "clipboard-history:{}",
                                        entry.id
                                    )))
                            } else {
                                div().id(ix).h(px(LIST_ITEM_HEIGHT))
                            }
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.clipboard_list_scroll_handle)
            .into_any_element()
        };
        let list_scrollbar = self.builtin_uniform_list_scrollbar(
            &self.clipboard_list_scroll_handle,
            filtered_len,
            8,
        );

        // Build preview panel for selected entry
        let selected_entry = filtered_entries
            .get(selected_index)
            .map(|(_, e)| (*e).clone());
        let preview_panel = self.render_clipboard_preview_panel(
            &selected_entry,
            &image_cache,
            &design_spacing,
            &design_typography,
            &design_visual,
        );

        // List pane with scrollbar overlay
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
                crate::components::builtin_leading_separator::render_builtin_leading_separator(
                    if filter.trim().is_empty() {
                        "Clipboard"
                    } else {
                        "Results"
                    },
                    None,
                    list_colors,
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

        let hints = if self.is_in_attachment_portal() {
            vec![
                "\u{21b5} Attach".into(),
                "Esc Cancel".into(),
                "Attaching to Agent Chat".into(),
            ]
        } else {
            crate::components::universal_prompt_hints_with_primary_label("Paste")
        };
        crate::components::emit_prompt_hint_audit("clipboard_history", &hints);
        let footer =
            self.main_window_footer_slot(crate::components::render_simple_hint_strip(hints, None));

        tracing::info!(
            target: "script_kit::prompt_chrome",
            surface = "clipboard_history",
            layout_mode = "main_view_chrome",
            "clipboard_history_chrome_checkpoint"
        );

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
            .id("clipboard-history-root")
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
                    .child(preview_panel),
            )
            .into_any_element();

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(self.theme_font_family())
                .key_context("clipboard_history")
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

#[cfg(test)]
mod clipboard_chrome_audit {
    fn production_source() -> &'static str {
        include_str!("clipboard.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source should exist")
    }

    #[test]
    fn clipboard_history_uses_shared_main_view_chrome() {
        let source = production_source();

        // Must route through the shared main-view chrome and native footer.
        assert!(
            source.contains("render_main_view_chrome_footer_flush("),
            "clipboard must use the shared main-view chrome"
        );
        assert!(
            source.contains("render_main_view_input_shell("),
            "clipboard must use the shared MainMenuInput shell"
        );
        assert!(
            source.contains("render_clickable_main_view_context_zone("),
            "clipboard must render the shared main-view context zone"
        );
        assert!(
            source.contains("main_window_footer_slot("),
            "clipboard footer must route through the persistent main-window footer"
        );
        assert!(
            !source.contains(&("render_expanded_view_scaffold".to_owned() + "_with_hints(")),
            "clipboard must not keep the old expanded scaffold"
        );

        // Must emit expanded layout audit
        assert!(
            source.contains("PromptChromeAudit::expanded(\"clipboard_history\""),
            "clipboard must declare expanded layout mode"
        );

        // Must NOT have hand-rolled divider chrome (split string to avoid self-match)
        let divider_call = "SectionDivider".to_owned() + "::new()";
        assert!(
            !source.contains(&divider_call),
            "clipboard must not use SectionDivider — expanded shell has no divider"
        );

        // Must NOT use legacy PromptFooter
        let legacy = "Prompt".to_owned() + "Footer::new(";
        assert_eq!(
            source.matches(&legacy).count(),
            0,
            "clipboard must not use PromptFooter"
        );

        // Must NOT hardcode paste-specific footer
        assert!(
            !source.contains("SharedString::from(\"↵ Paste\")"),
            "clipboard must not hardcode paste-specific footer"
        );

        // Must emit hint audit
        assert!(
            source.contains("emit_prompt_hint_audit(\"clipboard_history\""),
            "clipboard must emit hint audit for observability"
        );
        assert!(
            source.contains(".on_scroll_wheel(cx.listener("),
            "clipboard should observe wheel events on the list pane"
        );
        assert!(
            source.contains("observe_builtin_native_list_scroll(event, cx)"),
            "clipboard wheel scrolling should leave viewport movement to the native list"
        );
    }
}
