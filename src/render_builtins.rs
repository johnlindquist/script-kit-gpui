// Builtin view render methods - extracted from app_render.rs
// This file is included via include!() macro in main.rs
// Contains: render_clipboard_history, render_app_launcher, render_window_switcher, render_design_gallery

impl ScriptListApp {
    /// Render clipboard history view
    fn render_clipboard_history(
        &mut self,
        entries: Vec<clipboard_history::ClipboardEntry>,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // Use global image cache from clipboard_history module
        // Images are pre-decoded in the background monitor thread, so this is fast
        // Only decode if not already in the global cache (fallback)
        for entry in &entries {
            if entry.content_type == clipboard_history::ContentType::Image {
                // Check global cache first, then local cache
                if clipboard_history::get_cached_image(&entry.id).is_none()
                    && !self.clipboard_image_cache.contains_key(&entry.id)
                {
                    // Fallback: decode now if not pre-cached
                    if let Some(render_image) =
                        clipboard_history::decode_to_render_image(&entry.content)
                    {
                        // Store in global cache for future use
                        clipboard_history::cache_image(&entry.id, render_image.clone());
                        self.clipboard_image_cache
                            .insert(entry.id.clone(), render_image);
                    }
                } else if let Some(cached) = clipboard_history::get_cached_image(&entry.id) {
                    // Copy from global cache to local cache for this render
                    if !self.clipboard_image_cache.contains_key(&entry.id) {
                        self.clipboard_image_cache.insert(entry.id.clone(), cached);
                    }
                }
            }
        }

        // Clone the cache for use in closures
        let image_cache = self.clipboard_image_cache.clone();

        // Filter entries based on current filter
        let filtered_entries: Vec<_> = if filter.is_empty() {
            entries.iter().enumerate().collect()
        } else {
            let filter_lower = filter.to_lowercase();
            entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.content.to_lowercase().contains(&filter_lower))
                .collect()
        };
        let filtered_len = filtered_entries.len();

        // Key handler for clipboard history
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  _window: &mut Window,
                  cx: &mut Context<Self>| {
                // Global shortcuts (Cmd+W, ESC for dismissable views)
                if this.handle_global_shortcut_with_options(event, true, cx) {
                    return;
                }

                let key_str = event.keystroke.key.to_lowercase();
                logging::log("KEY", &format!("ClipboardHistory key: '{}'", key_str));

                if let AppView::ClipboardHistoryView {
                    entries,
                    filter,
                    selected_index,
                } = &mut this.current_view
                {
                    // Apply filter to get current filtered list
                    let filtered_entries: Vec<_> = if filter.is_empty() {
                        entries.iter().enumerate().collect()
                    } else {
                        let filter_lower = filter.to_lowercase();
                        entries
                            .iter()
                            .enumerate()
                            .filter(|(_, e)| e.content.to_lowercase().contains(&filter_lower))
                            .collect()
                    };
                    let filtered_len = filtered_entries.len();

                    match key_str.as_str() {
                        "up" | "arrowup" => {
                            if *selected_index > 0 {
                                *selected_index -= 1;
                                // Scroll to keep selection visible
                                this.clipboard_list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        "down" | "arrowdown" => {
                            if *selected_index < filtered_len.saturating_sub(1) {
                                *selected_index += 1;
                                // Scroll to keep selection visible
                                this.clipboard_list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        "enter" => {
                            // Copy selected entry to clipboard, hide window, then paste
                            if let Some((_, entry)) = filtered_entries.get(*selected_index) {
                                logging::log(
                                    "EXEC",
                                    &format!("Copying clipboard entry: {}", entry.id),
                                );
                                if let Err(e) =
                                    clipboard_history::copy_entry_to_clipboard(&entry.id)
                                {
                                    logging::log("ERROR", &format!("Failed to copy entry: {}", e));
                                } else {
                                    logging::log("EXEC", "Entry copied to clipboard");
                                    // Hide window first
                                    script_kit_gpui::set_main_window_visible(false);
                                    cx.hide();
                                    NEEDS_RESET.store(true, Ordering::SeqCst);

                                    // Simulate Cmd+V paste after a brief delay to let focus return
                                    std::thread::spawn(|| {
                                        std::thread::sleep(std::time::Duration::from_millis(100));
                                        if let Err(e) = selected_text::simulate_paste_with_cg() {
                                            logging::log(
                                                "ERROR",
                                                &format!("Failed to simulate paste: {}", e),
                                            );
                                        } else {
                                            logging::log("EXEC", "Simulated Cmd+V paste");
                                        }
                                    });
                                }
                            }
                        }
                        // Note: "escape" is handled by handle_global_shortcut_with_options above
                        "backspace" => {
                            if !filter.is_empty() {
                                filter.pop();
                                *selected_index = 0;
                                // Reset scroll to top when filter changes
                                this.clipboard_list_scroll_handle
                                    .scroll_to_item(0, ScrollStrategy::Top);
                                cx.notify();
                            }
                        }
                        _ => {
                            if let Some(ref key_char) = event.keystroke.key_char {
                                if let Some(ch) = key_char.chars().next() {
                                    if !ch.is_control() {
                                        filter.push(ch);
                                        *selected_index = 0;
                                        // Reset scroll to top when filter changes
                                        this.clipboard_list_scroll_handle
                                            .scroll_to_item(0, ScrollStrategy::Top);
                                        cx.notify();
                                    }
                                }
                            }
                        }
                    }
                }
            },
        );

        let input_display = if filter.is_empty() {
            SharedString::from("Search clipboard history...")
        } else {
            SharedString::from(filter.clone())
        };
        let input_is_empty = filter.is_empty();

        // Pre-compute colors
        let list_colors = ListItemColors::from_design(&design_colors);
        let text_primary = design_colors.text_primary;
        let text_muted = design_colors.text_muted;
        let text_dimmed = design_colors.text_dimmed;
        let ui_border = design_colors.border;

        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(design_colors.text_muted))
                .font_family(design_typography.font_family)
                .child(if filter.is_empty() {
                    "No clipboard history"
                } else {
                    "No entries match your filter"
                })
                .into_any_element()
        } else {
            // Clone data for the closure
            let entries_for_closure: Vec<_> = filtered_entries
                .iter()
                .map(|(i, e)| (*i, (*e).clone()))
                .collect();
            let selected = selected_index;
            let image_cache_for_list = image_cache.clone();

            uniform_list(
                "clipboard-history",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((_, entry)) = entries_for_closure.get(ix) {
                                let is_selected = ix == selected;

                                // Get cached thumbnail for images
                                let cached_image = if entry.content_type
                                    == clipboard_history::ContentType::Image
                                {
                                    image_cache_for_list.get(&entry.id).cloned()
                                } else {
                                    None
                                };

                                // Truncate content for display (show dimensions for images)
                                let display_content = match entry.content_type {
                                    clipboard_history::ContentType::Image => {
                                        // Show image dimensions instead of "[Image]"
                                        if let Some((w, h)) =
                                            clipboard_history::get_image_dimensions(&entry.content)
                                        {
                                            format!("{}×{} image", w, h)
                                        } else {
                                            "Image".to_string()
                                        }
                                    }
                                    clipboard_history::ContentType::Text => {
                                        // Replace newlines with spaces to prevent multi-line list items
                                        let sanitized = entry.content.replace(['\n', '\r'], " ");
                                        let truncated: String =
                                            sanitized.chars().take(50).collect();
                                        if sanitized.len() > 50 {
                                            format!("{}...", truncated)
                                        } else {
                                            truncated
                                        }
                                    }
                                };

                                // Format relative time
                                let now = chrono::Utc::now().timestamp();
                                let age_secs = now - entry.timestamp;
                                let relative_time = if age_secs < 60 {
                                    "just now".to_string()
                                } else if age_secs < 3600 {
                                    format!("{}m ago", age_secs / 60)
                                } else if age_secs < 86400 {
                                    format!("{}h ago", age_secs / 3600)
                                } else {
                                    format!("{}d ago", age_secs / 86400)
                                };

                                // Add pin indicator
                                let name = if entry.pinned {
                                    format!("📌 {}", display_content)
                                } else {
                                    display_content
                                };

                                // Build list item with optional thumbnail
                                let mut item = ListItem::new(name, list_colors)
                                    .description_opt(Some(relative_time))
                                    .selected(is_selected)
                                    .with_accent_bar(true);

                                // Add thumbnail for images, text icon for text entries
                                if let Some(render_image) = cached_image {
                                    item = item.icon_image(render_image);
                                } else if entry.content_type == clipboard_history::ContentType::Text
                                {
                                    item = item.icon("📄");
                                }

                                div().id(ix).child(item)
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

        // Build preview panel for selected entry
        let selected_entry = filtered_entries
            .get(selected_index)
            .map(|(_, e)| (*e).clone());
        let preview_panel = self.render_clipboard_preview_panel(
            &selected_entry,
            &image_cache,
            &design_colors,
            &design_spacing,
            &design_typography,
            &design_visual,
        );

        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("clipboard_history")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Search input with blinking cursor
                    // ALIGNMENT FIX: Uses canonical cursor constants and negative margin for placeholder
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if input_is_empty {
                                rgb(text_muted)
                            } else {
                                rgb(text_primary)
                            })
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .mr(px(CURSOR_GAP_X))
                                        .when(self.cursor_visible, |d| d.bg(rgb(text_primary))),
                                )
                            })
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .ml(px(-(CURSOR_WIDTH + CURSOR_GAP_X)))
                                        .child(input_display.clone()),
                                )
                            })
                            .when(!input_is_empty, |d| d.child(input_display.clone()))
                            .when(!input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .ml(px(CURSOR_GAP_X))
                                        .when(self.cursor_visible, |d| d.bg(rgb(text_primary))),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(format!("{} entries", entries.len())),
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(design_spacing.padding_lg))
                    .h(px(design_visual.border_thin))
                    .bg(rgba((ui_border << 8) | 0x60)),
            )
            // Main content area - 50/50 split: List on left, Preview on right
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .overflow_hidden()
                    // Left side: Clipboard list (50% width)
                    .child(
                        div()
                            .w_1_2()
                            .h_full()
                            .min_h(px(0.))
                            .py(px(design_spacing.padding_xs))
                            .child(list_element),
                    )
                    // Right side: Preview panel (50% width)
                    .child(
                        div()
                            .w_1_2()
                            .h_full()
                            .min_h(px(0.))
                            .overflow_hidden()
                            .child(preview_panel),
                    ),
            )
            .into_any_element()
    }

    /// Render the preview panel for clipboard history
    fn render_clipboard_preview_panel(
        &self,
        selected_entry: &Option<clipboard_history::ClipboardEntry>,
        image_cache: &std::collections::HashMap<String, Arc<gpui::RenderImage>>,
        colors: &designs::DesignColors,
        spacing: &designs::DesignSpacing,
        typography: &designs::DesignTypography,
        visual: &designs::DesignVisual,
    ) -> impl IntoElement {
        let bg_main = colors.background;
        let ui_border = colors.border;
        let text_primary = colors.text_primary;
        let text_muted = colors.text_muted;
        let text_secondary = colors.text_secondary;
        let bg_search_box = colors.background_tertiary;

        let mut panel = div()
            .w_full()
            .h_full()
            .bg(rgb(bg_main))
            .border_l_1()
            .border_color(rgba((ui_border << 8) | 0x80))
            .p(px(spacing.padding_lg))
            .flex()
            .flex_col()
            .overflow_y_hidden()
            .font_family(typography.font_family);

        match selected_entry {
            Some(entry) => {
                // Header with content type
                let content_type_label = match entry.content_type {
                    clipboard_history::ContentType::Text => "Text",
                    clipboard_history::ContentType::Image => "Image",
                };

                panel = panel.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .pb(px(spacing.padding_sm))
                        // Content type badge
                        .child(
                            div()
                                .px(px(spacing.padding_sm))
                                .py(px(spacing.padding_xs / 2.0))
                                .rounded(px(visual.radius_sm))
                                .bg(rgba((colors.accent << 8) | 0x30))
                                .text_xs()
                                .text_color(rgb(colors.accent))
                                .child(content_type_label),
                        )
                        // Pin indicator
                        .when(entry.pinned, |d| {
                            d.child(
                                div()
                                    .px(px(spacing.padding_sm))
                                    .py(px(spacing.padding_xs / 2.0))
                                    .rounded(px(visual.radius_sm))
                                    .bg(rgba((colors.accent << 8) | 0x20))
                                    .text_xs()
                                    .text_color(rgb(colors.accent))
                                    .child("📌 Pinned"),
                            )
                        }),
                );

                // Timestamp
                let now = chrono::Utc::now().timestamp();
                let age_secs = now - entry.timestamp;
                let relative_time = if age_secs < 60 {
                    "just now".to_string()
                } else if age_secs < 3600 {
                    format!("{} minutes ago", age_secs / 60)
                } else if age_secs < 86400 {
                    format!("{} hours ago", age_secs / 3600)
                } else {
                    format!("{} days ago", age_secs / 86400)
                };

                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .pb(px(spacing.padding_md))
                        .child(relative_time),
                );

                // Divider
                panel = panel.child(
                    div()
                        .w_full()
                        .h(px(visual.border_thin))
                        .bg(rgba((ui_border << 8) | 0x60))
                        .my(px(spacing.padding_sm)),
                );

                // Content preview
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .pb(px(spacing.padding_sm))
                        .child("Content Preview"),
                );

                match entry.content_type {
                    clipboard_history::ContentType::Text => {
                        // Show full text content in a code-like container
                        let content = entry.content.clone();
                        let char_count = content.chars().count();
                        let line_count = content.lines().count();

                        panel = panel
                            .child(
                                div()
                                    .w_full()
                                    .flex_1()
                                    .p(px(spacing.padding_md))
                                    .rounded(px(visual.radius_md))
                                    .bg(rgba((bg_search_box << 8) | 0x80))
                                    .overflow_hidden()
                                    .font_family(typography.font_family_mono)
                                    .text_sm()
                                    .text_color(rgb(text_primary))
                                    .child(content),
                            )
                            // Stats footer
                            .child(
                                div()
                                    .pt(px(spacing.padding_sm))
                                    .text_xs()
                                    .text_color(rgb(text_secondary))
                                    .child(format!(
                                        "{} characters • {} lines",
                                        char_count, line_count
                                    )),
                            );
                    }
                    clipboard_history::ContentType::Image => {
                        // Get image dimensions
                        let (width, height) =
                            clipboard_history::get_image_dimensions(&entry.content)
                                .unwrap_or((0, 0));

                        // Try to get cached render image
                        let cached_image = image_cache.get(&entry.id).cloned();

                        let image_container = if let Some(render_image) = cached_image {
                            // Calculate display size that fits in the preview panel
                            // Max size is 300x300, maintain aspect ratio
                            let max_size: f32 = 300.0;
                            let (display_w, display_h) = if width > 0 && height > 0 {
                                let w = width as f32;
                                let h = height as f32;
                                let scale = (max_size / w).min(max_size / h).min(1.0);
                                (w * scale, h * scale)
                            } else {
                                (max_size, max_size)
                            };

                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                // Actual image thumbnail
                                .child(
                                    gpui::img(move |_window: &mut Window, _cx: &mut App| {
                                        Some(Ok(render_image.clone()))
                                    })
                                    .w(px(display_w))
                                    .h(px(display_h))
                                    .object_fit(gpui::ObjectFit::Contain)
                                    .rounded(px(visual.radius_sm)),
                                )
                                // Dimensions label below image
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(format!("{}×{} pixels", width, height)),
                                )
                        } else {
                            // Fallback if image not in cache (shouldn't happen)
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .child(div().text_2xl().child("🖼️"))
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(text_primary))
                                        .child(format!("{}×{}", width, height)),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_muted))
                                        .child("Loading image..."),
                                )
                        };

                        panel = panel.child(
                            div()
                                .w_full()
                                .flex_1()
                                .p(px(spacing.padding_lg))
                                .rounded(px(visual.radius_md))
                                .bg(rgba((bg_search_box << 8) | 0x80))
                                .flex()
                                .items_center()
                                .justify_center()
                                .overflow_hidden()
                                .child(image_container),
                        );
                    }
                }
            }
            None => {
                // Empty state
                panel = panel.child(
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(text_muted))
                        .child("No entry selected"),
                );
            }
        }

        panel
    }

    /// Render app launcher view
    fn render_app_launcher(
        &mut self,
        apps: Vec<app_launcher::AppInfo>,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // Filter apps based on current filter
        let filtered_apps: Vec<_> = if filter.is_empty() {
            apps.iter().enumerate().collect()
        } else {
            let filter_lower = filter.to_lowercase();
            apps.iter()
                .enumerate()
                .filter(|(_, a)| a.name.to_lowercase().contains(&filter_lower))
                .collect()
        };
        let filtered_len = filtered_apps.len();

        // Key handler for app launcher
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  _window: &mut Window,
                  cx: &mut Context<Self>| {
                // Global shortcuts (Cmd+W) - handled first regardless of view state
                // Global shortcuts (Cmd+W, ESC for dismissable views)
                if this.handle_global_shortcut_with_options(event, true, cx) {
                    return;
                }

                let key_str = event.keystroke.key.to_lowercase();
                logging::log("KEY", &format!("AppLauncher key: '{}'", key_str));

                if let AppView::AppLauncherView {
                    apps,
                    filter,
                    selected_index,
                } = &mut this.current_view
                {
                    // Apply filter to get current filtered list
                    let filtered_apps: Vec<_> = if filter.is_empty() {
                        apps.iter().enumerate().collect()
                    } else {
                        let filter_lower = filter.to_lowercase();
                        apps.iter()
                            .enumerate()
                            .filter(|(_, a)| a.name.to_lowercase().contains(&filter_lower))
                            .collect()
                    };
                    let filtered_len = filtered_apps.len();

                    match key_str.as_str() {
                        "up" | "arrowup" => {
                            if *selected_index > 0 {
                                *selected_index -= 1;
                                cx.notify();
                            }
                        }
                        "down" | "arrowdown" => {
                            if *selected_index < filtered_len.saturating_sub(1) {
                                *selected_index += 1;
                                cx.notify();
                            }
                        }
                        "enter" => {
                            // Launch selected app and hide window
                            if let Some((_, app)) = filtered_apps.get(*selected_index) {
                                logging::log("EXEC", &format!("Launching app: {}", app.name));
                                if let Err(e) = app_launcher::launch_application(app) {
                                    logging::log("ERROR", &format!("Failed to launch app: {}", e));
                                } else {
                                    logging::log("EXEC", &format!("Launched: {}", app.name));
                                    // Hide window after launching
                                    script_kit_gpui::set_main_window_visible(false);
                                    cx.hide();
                                    NEEDS_RESET.store(true, Ordering::SeqCst);
                                }
                            }
                        }
                        // Note: "escape" is handled by handle_global_shortcut_with_options above
                        "backspace" => {
                            if !filter.is_empty() {
                                filter.pop();
                                *selected_index = 0;
                                cx.notify();
                            }
                        }
                        _ => {
                            if let Some(ref key_char) = event.keystroke.key_char {
                                if let Some(ch) = key_char.chars().next() {
                                    if !ch.is_control() {
                                        filter.push(ch);
                                        *selected_index = 0;
                                        cx.notify();
                                    }
                                }
                            }
                        }
                    }
                }
            },
        );

        let input_display = if filter.is_empty() {
            SharedString::from("Search applications...")
        } else {
            SharedString::from(filter.clone())
        };
        let input_is_empty = filter.is_empty();

        // Pre-compute colors
        let list_colors = ListItemColors::from_design(&design_colors);
        let text_primary = design_colors.text_primary;
        let text_muted = design_colors.text_muted;
        let text_dimmed = design_colors.text_dimmed;
        let ui_border = design_colors.border;

        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(design_colors.text_muted))
                .font_family(design_typography.font_family)
                .child(if filter.is_empty() {
                    "No applications found"
                } else {
                    "No apps match your filter"
                })
                .into_any_element()
        } else {
            // Clone data for the closure
            let apps_for_closure: Vec<_> = filtered_apps
                .iter()
                .map(|(i, a)| (*i, (*a).clone()))
                .collect();
            let selected = selected_index;

            uniform_list(
                "app-launcher",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((_, app)) = apps_for_closure.get(ix) {
                                let is_selected = ix == selected;

                                // Format app path for description
                                let path_str = app.path.to_string_lossy();
                                let description = if path_str.starts_with("/Applications") {
                                    None // No need to show path for standard apps
                                } else {
                                    Some(path_str.to_string())
                                };

                                // Use pre-decoded icon if available, fallback to emoji
                                let icon = match &app.icon {
                                    Some(img) => list_item::IconKind::Image(img.clone()),
                                    None => list_item::IconKind::Emoji("📱".to_string()),
                                };

                                div().id(ix).child(
                                    ListItem::new(app.name.clone(), list_colors)
                                        .icon_kind(icon)
                                        .description_opt(description)
                                        .selected(is_selected)
                                        .with_accent_bar(true),
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
            .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("app_launcher")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Title
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child("🚀 Apps"),
                    )
                    // Search input with blinking cursor
                    // ALIGNMENT FIX: Uses canonical cursor constants and negative margin for placeholder
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if input_is_empty {
                                rgb(text_muted)
                            } else {
                                rgb(text_primary)
                            })
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .mr(px(CURSOR_GAP_X))
                                        .when(self.cursor_visible, |d| d.bg(rgb(text_primary))),
                                )
                            })
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .ml(px(-(CURSOR_WIDTH + CURSOR_GAP_X)))
                                        .child(input_display.clone()),
                                )
                            })
                            .when(!input_is_empty, |d| d.child(input_display.clone()))
                            .when(!input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .ml(px(CURSOR_GAP_X))
                                        .when(self.cursor_visible, |d| d.bg(rgb(text_primary))),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(format!("{} apps", apps.len())),
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(design_spacing.padding_lg))
                    .h(px(design_visual.border_thin))
                    .bg(rgba((ui_border << 8) | 0x60)),
            )
            // App list
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .py(px(design_spacing.padding_xs))
                    .child(list_element),
            )
            .into_any_element()
    }

    /// Render window switcher view with 50/50 split layout
    fn render_window_switcher(
        &mut self,
        windows: Vec<window_control::WindowInfo>,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // Filter windows based on current filter
        let filtered_windows: Vec<_> = if filter.is_empty() {
            windows.iter().enumerate().collect()
        } else {
            let filter_lower = filter.to_lowercase();
            windows
                .iter()
                .enumerate()
                .filter(|(_, w)| {
                    w.title.to_lowercase().contains(&filter_lower)
                        || w.app.to_lowercase().contains(&filter_lower)
                })
                .collect()
        };
        let filtered_len = filtered_windows.len();

        // Key handler for window switcher
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  _window: &mut Window,
                  cx: &mut Context<Self>| {
                // Global shortcuts (Cmd+W, ESC for dismissable views)
                if this.handle_global_shortcut_with_options(event, true, cx) {
                    return;
                }

                let key_str = event.keystroke.key.to_lowercase();
                logging::log("KEY", &format!("WindowSwitcher key: '{}'", key_str));

                if let AppView::WindowSwitcherView {
                    windows,
                    filter,
                    selected_index,
                } = &mut this.current_view
                {
                    // Apply filter to get current filtered list
                    let filtered_windows: Vec<_> = if filter.is_empty() {
                        windows.iter().enumerate().collect()
                    } else {
                        let filter_lower = filter.to_lowercase();
                        windows
                            .iter()
                            .enumerate()
                            .filter(|(_, w)| {
                                w.title.to_lowercase().contains(&filter_lower)
                                    || w.app.to_lowercase().contains(&filter_lower)
                            })
                            .collect()
                    };
                    let filtered_len = filtered_windows.len();

                    match key_str.as_str() {
                        "up" | "arrowup" => {
                            if *selected_index > 0 {
                                *selected_index -= 1;
                                this.window_list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        "down" | "arrowdown" => {
                            if *selected_index < filtered_len.saturating_sub(1) {
                                *selected_index += 1;
                                this.window_list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        "enter" => {
                            // Focus selected window and hide Script Kit
                            if let Some((_, window_info)) = filtered_windows.get(*selected_index) {
                                logging::log(
                                    "EXEC",
                                    &format!("Focusing window: {}", window_info.title),
                                );
                                if let Err(e) = window_control::focus_window(window_info.id) {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to focus window: {}", e),
                                    );
                                    this.toast_manager.push(
                                        components::toast::Toast::error(
                                            format!("Failed to focus window: {}", e),
                                            &this.theme,
                                        )
                                        .duration_ms(Some(5000)),
                                    );
                                    cx.notify();
                                } else {
                                    logging::log(
                                        "EXEC",
                                        &format!("Focused window: {}", window_info.title),
                                    );
                                    script_kit_gpui::set_main_window_visible(false);
                                    cx.hide();
                                    NEEDS_RESET.store(true, Ordering::SeqCst);
                                }
                            }
                        }
                        // Note: "escape" is handled by handle_global_shortcut_with_options above
                        "backspace" => {
                            if !filter.is_empty() {
                                filter.pop();
                                *selected_index = 0;
                                this.window_list_scroll_handle
                                    .scroll_to_item(0, ScrollStrategy::Top);
                                cx.notify();
                            }
                        }
                        // Number keys for quick window actions - extract window_id to avoid borrow issues
                        "1" | "2" | "3" | "4" | "m" | "n" | "c" => {
                            if let Some((_, window_info)) = filtered_windows.get(*selected_index) {
                                let window_id = window_info.id;
                                let action = match key_str.as_str() {
                                    "1" => "tile_left",
                                    "2" => "tile_right",
                                    "3" => "tile_top",
                                    "4" => "tile_bottom",
                                    "m" => "maximize",
                                    "n" => "minimize",
                                    "c" => "close",
                                    _ => unreachable!(),
                                };
                                // Drop the borrow before calling execute_window_action
                                drop(filtered_windows);
                                this.execute_window_action(window_id, action, cx);
                            }
                        }
                        _ => {
                            // Allow all printable characters for window search
                            if let Some(ref key_char) = event.keystroke.key_char {
                                if let Some(ch) = key_char.chars().next() {
                                    if !ch.is_control() {
                                        filter.push(ch);
                                        *selected_index = 0;
                                        this.window_list_scroll_handle
                                            .scroll_to_item(0, ScrollStrategy::Top);
                                        cx.notify();
                                    }
                                }
                            }
                        }
                    }
                }
            },
        );

        let input_display = if filter.is_empty() {
            SharedString::from("Search windows...")
        } else {
            SharedString::from(filter.clone())
        };
        let input_is_empty = filter.is_empty();

        // Pre-compute colors
        let list_colors = ListItemColors::from_design(&design_colors);
        let text_primary = design_colors.text_primary;
        let text_muted = design_colors.text_muted;
        let text_dimmed = design_colors.text_dimmed;
        let ui_border = design_colors.border;

        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(design_colors.text_muted))
                .font_family(design_typography.font_family)
                .child(if filter.is_empty() {
                    "No windows found"
                } else {
                    "No windows match your filter"
                })
                .into_any_element()
        } else {
            // Clone data for the closure
            let windows_for_closure: Vec<_> = filtered_windows
                .iter()
                .map(|(i, w)| (*i, (*w).clone()))
                .collect();
            let selected = selected_index;

            uniform_list(
                "window-switcher",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((_, window_info)) = windows_for_closure.get(ix) {
                                let is_selected = ix == selected;

                                // Format: "AppName: Window Title"
                                let name = format!("{}: {}", window_info.app, window_info.title);

                                // Format bounds as description
                                let description = format!(
                                    "{}×{} at ({}, {})",
                                    window_info.bounds.width,
                                    window_info.bounds.height,
                                    window_info.bounds.x,
                                    window_info.bounds.y
                                );

                                div().id(ix).child(
                                    ListItem::new(name, list_colors)
                                        .description_opt(Some(description))
                                        .selected(is_selected)
                                        .with_accent_bar(true),
                                )
                            } else {
                                div().id(ix).h(px(LIST_ITEM_HEIGHT))
                            }
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.window_list_scroll_handle)
            .into_any_element()
        };

        // Build actions panel for selected window
        let selected_window = filtered_windows
            .get(selected_index)
            .map(|(_, w)| (*w).clone());
        let actions_panel = self.render_window_actions_panel(
            &selected_window,
            &design_colors,
            &design_spacing,
            &design_typography,
            &design_visual,
            cx,
        );

        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("window_switcher")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Title
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child("🪟 Windows"),
                    )
                    // Search input with blinking cursor
                    // ALIGNMENT FIX: Uses canonical cursor constants and negative margin for placeholder
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if input_is_empty {
                                rgb(text_muted)
                            } else {
                                rgb(text_primary)
                            })
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .mr(px(CURSOR_GAP_X))
                                        .when(self.cursor_visible, |d| d.bg(rgb(text_primary))),
                                )
                            })
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .ml(px(-(CURSOR_WIDTH + CURSOR_GAP_X)))
                                        .child(input_display.clone()),
                                )
                            })
                            .when(!input_is_empty, |d| d.child(input_display.clone()))
                            .when(!input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .ml(px(CURSOR_GAP_X))
                                        .when(self.cursor_visible, |d| d.bg(rgb(text_primary))),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(format!("{} windows", windows.len())),
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(design_spacing.padding_lg))
                    .h(px(design_visual.border_thin))
                    .bg(rgba((ui_border << 8) | 0x60)),
            )
            // Main content area - 50/50 split: Window list on left, Actions on right
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .overflow_hidden()
                    // Left side: Window list (50% width)
                    .child(
                        div()
                            .w_1_2()
                            .h_full()
                            .min_h(px(0.))
                            .py(px(design_spacing.padding_xs))
                            .child(list_element),
                    )
                    // Right side: Actions panel (50% width)
                    .child(
                        div()
                            .w_1_2()
                            .h_full()
                            .min_h(px(0.))
                            .overflow_hidden()
                            .child(actions_panel),
                    ),
            )
            .into_any_element()
    }

    /// Render the actions panel for window switcher
    fn render_window_actions_panel(
        &self,
        selected_window: &Option<window_control::WindowInfo>,
        colors: &designs::DesignColors,
        spacing: &designs::DesignSpacing,
        typography: &designs::DesignTypography,
        visual: &designs::DesignVisual,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bg_main = colors.background;
        let ui_border = colors.border;
        let text_primary = colors.text_primary;
        let text_muted = colors.text_muted;
        let text_secondary = colors.text_secondary;

        let mut panel = div()
            .w_full()
            .h_full()
            .bg(rgb(bg_main))
            .border_l_1()
            .border_color(rgba((ui_border << 8) | 0x80))
            .p(px(spacing.padding_lg))
            .flex()
            .flex_col()
            .overflow_y_hidden()
            .font_family(typography.font_family);

        match selected_window {
            Some(window) => {
                // Window info header
                panel = panel.child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(text_primary))
                        .pb(px(spacing.padding_sm))
                        .child(window.title.clone()),
                );

                // App name
                panel = panel.child(
                    div()
                        .text_sm()
                        .text_color(rgb(text_secondary))
                        .pb(px(spacing.padding_md))
                        .child(window.app.clone()),
                );

                // Bounds info
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .pb(px(spacing.padding_lg))
                        .child(format!(
                            "{}×{} at ({}, {})",
                            window.bounds.width,
                            window.bounds.height,
                            window.bounds.x,
                            window.bounds.y
                        )),
                );

                // Divider
                panel = panel.child(
                    div()
                        .w_full()
                        .h(px(visual.border_thin))
                        .bg(rgba((ui_border << 8) | 0x60))
                        .mb(px(spacing.padding_lg)),
                );

                // Actions header
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .pb(px(spacing.padding_md))
                        .child("Actions (keyboard shortcuts)"),
                );

                // Action buttons grid - using text labels with shortcuts
                let action_items = [
                    ("1", "← Tile Left Half"),
                    ("2", "→ Tile Right Half"),
                    ("3", "↑ Tile Top Half"),
                    ("4", "↓ Tile Bottom Half"),
                    ("M", "□ Maximize"),
                    ("N", "_ Minimize"),
                    ("⏎", "◉ Focus"),
                    ("C", "✕ Close"),
                ];

                for (key, label) in action_items {
                    panel = panel.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .py(px(spacing.padding_xs))
                            // Key badge
                            .child(
                                div()
                                    .w(px(24.0))
                                    .h(px(20.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(visual.radius_sm))
                                    .bg(rgba((colors.background_tertiary << 8) | 0x80))
                                    .text_xs()
                                    .text_color(rgb(text_secondary))
                                    .child(key),
                            )
                            // Label
                            .child(div().text_sm().text_color(rgb(text_primary)).child(label)),
                    );
                }
            }
            None => {
                // Empty state
                panel = panel.child(
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(text_muted))
                        .child("No window selected"),
                );
            }
        }

        panel
    }

    /// Execute a window action (tile, maximize, minimize, close)
    fn execute_window_action(&mut self, window_id: u32, action: &str, cx: &mut Context<Self>) {
        logging::log(
            "EXEC",
            &format!("Window action: {} on window {}", action, window_id),
        );

        let result = match action {
            "tile_left" => {
                window_control::tile_window(window_id, window_control::TilePosition::LeftHalf)
            }
            "tile_right" => {
                window_control::tile_window(window_id, window_control::TilePosition::RightHalf)
            }
            "tile_top" => {
                window_control::tile_window(window_id, window_control::TilePosition::TopHalf)
            }
            "tile_bottom" => {
                window_control::tile_window(window_id, window_control::TilePosition::BottomHalf)
            }
            "maximize" => window_control::maximize_window(window_id),
            "minimize" => window_control::minimize_window(window_id),
            "close" => window_control::close_window(window_id),
            "focus" => window_control::focus_window(window_id),
            _ => {
                logging::log("ERROR", &format!("Unknown window action: {}", action));
                return;
            }
        };

        match result {
            Ok(()) => {
                logging::log("EXEC", &format!("Window action {} succeeded", action));

                // Show success toast
                self.toast_manager.push(
                    components::toast::Toast::success(
                        format!("Window {}", action.replace("_", " ")),
                        &self.theme,
                    )
                    .duration_ms(Some(2000)),
                );

                // Refresh window list after action
                if let AppView::WindowSwitcherView {
                    windows,
                    selected_index,
                    ..
                } = &mut self.current_view
                {
                    match window_control::list_windows() {
                        Ok(new_windows) => {
                            *windows = new_windows;
                            // Adjust selected index if needed
                            if *selected_index >= windows.len() && !windows.is_empty() {
                                *selected_index = windows.len() - 1;
                            }
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to refresh windows: {}", e));
                        }
                    }
                }
            }
            Err(e) => {
                logging::log("ERROR", &format!("Window action {} failed: {}", action, e));

                // Show error toast
                self.toast_manager.push(
                    components::toast::Toast::error(
                        format!("Failed to {}: {}", action.replace("_", " "), e),
                        &self.theme,
                    )
                    .duration_ms(Some(5000)),
                );
            }
        }

        cx.notify();
    }

    /// Render design gallery view with group header and icon variations
    fn render_design_gallery(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use designs::group_header_variations::{GroupHeaderCategory, GroupHeaderStyle};
        use designs::icon_variations::{IconCategory, IconName, IconStyle};

        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();

        // Build gallery items: group headers grouped by category, then icons grouped by category
        #[derive(Clone)]
        enum GalleryItem {
            GroupHeaderCategory(GroupHeaderCategory),
            GroupHeader(GroupHeaderStyle),
            IconCategoryHeader(IconCategory),
            Icon(IconName, IconStyle),
        }

        let mut gallery_items: Vec<GalleryItem> = Vec::new();

        // Add group headers by category
        for category in GroupHeaderCategory::all() {
            gallery_items.push(GalleryItem::GroupHeaderCategory(*category));
            for style in category.styles() {
                gallery_items.push(GalleryItem::GroupHeader(*style));
            }
        }

        // Add icons by category, showing each icon with default style
        for category in IconCategory::all() {
            gallery_items.push(GalleryItem::IconCategoryHeader(*category));
            for icon in category.icons() {
                gallery_items.push(GalleryItem::Icon(icon, IconStyle::Default));
            }
        }

        // Filter items based on current filter
        let filtered_items: Vec<(usize, GalleryItem)> = if filter.is_empty() {
            gallery_items
                .iter()
                .enumerate()
                .map(|(i, item)| (i, item.clone()))
                .collect()
        } else {
            let filter_lower = filter.to_lowercase();
            gallery_items
                .iter()
                .enumerate()
                .filter(|(_, item)| match item {
                    GalleryItem::GroupHeaderCategory(cat) => {
                        cat.name().to_lowercase().contains(&filter_lower)
                    }
                    GalleryItem::GroupHeader(style) => {
                        style.name().to_lowercase().contains(&filter_lower)
                            || style.description().to_lowercase().contains(&filter_lower)
                    }
                    GalleryItem::IconCategoryHeader(cat) => {
                        cat.name().to_lowercase().contains(&filter_lower)
                    }
                    GalleryItem::Icon(icon, _) => {
                        icon.name().to_lowercase().contains(&filter_lower)
                            || icon.description().to_lowercase().contains(&filter_lower)
                    }
                })
                .map(|(i, item)| (i, item.clone()))
                .collect()
        };
        let filtered_len = filtered_items.len();

        // Key handler for design gallery
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  _window: &mut Window,
                  cx: &mut Context<Self>| {
                // Global shortcuts (Cmd+W) - handled first regardless of view state
                // Global shortcuts (Cmd+W, ESC for dismissable views)
                if this.handle_global_shortcut_with_options(event, true, cx) {
                    return;
                }

                let key_str = event.keystroke.key.to_lowercase();
                logging::log("KEY", &format!("DesignGallery key: '{}'", key_str));

                if let AppView::DesignGalleryView {
                    filter,
                    selected_index,
                } = &mut this.current_view
                {
                    // Re-compute filtered_len for this scope
                    let total_items = GroupHeaderStyle::count()
                        + IconName::count()
                        + GroupHeaderCategory::all().len()
                        + IconCategory::all().len();
                    let current_filtered_len = total_items;

                    match key_str.as_str() {
                        "up" | "arrowup" => {
                            if *selected_index > 0 {
                                *selected_index -= 1;
                                this.design_gallery_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        "down" | "arrowdown" => {
                            if *selected_index < current_filtered_len.saturating_sub(1) {
                                *selected_index += 1;
                                this.design_gallery_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        // Note: "escape" is handled by handle_global_shortcut_with_options above
                        "backspace" => {
                            if !filter.is_empty() {
                                filter.pop();
                                *selected_index = 0;
                                this.design_gallery_scroll_handle
                                    .scroll_to_item(0, ScrollStrategy::Top);
                                cx.notify();
                            }
                        }
                        _ => {
                            if let Some(ref key_char) = event.keystroke.key_char {
                                if let Some(ch) = key_char.chars().next() {
                                    if !ch.is_control() {
                                        filter.push(ch);
                                        *selected_index = 0;
                                        this.design_gallery_scroll_handle
                                            .scroll_to_item(0, ScrollStrategy::Top);
                                        cx.notify();
                                    }
                                }
                            }
                        }
                    }
                }
            },
        );

        let input_display = if filter.is_empty() {
            SharedString::from("Search design variations...")
        } else {
            SharedString::from(filter.clone())
        };
        let input_is_empty = filter.is_empty();

        // Pre-compute colors
        let list_colors = ListItemColors::from_design(&design_colors);
        let text_primary = design_colors.text_primary;
        let text_muted = design_colors.text_muted;
        let text_dimmed = design_colors.text_dimmed;
        let ui_border = design_colors.border;
        let _accent = design_colors.accent;

        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(design_colors.text_muted))
                .font_family(design_typography.font_family)
                .child("No items match your filter")
                .into_any_element()
        } else {
            // Clone data for the closure
            let items_for_closure = filtered_items.clone();
            let selected = selected_index;
            let _list_colors_clone = list_colors; // Kept for future use
            let design_spacing_clone = design_spacing;
            let design_typography_clone = design_typography;
            let design_visual_clone = design_visual;
            let design_colors_clone = design_colors;

            uniform_list(
                "design-gallery",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((_, item)) = items_for_closure.get(ix) {
                                let is_selected = ix == selected;

                                let element: AnyElement = match item {
                                    GalleryItem::GroupHeaderCategory(category) => {
                                        // Category header - styled as section header
                                        div()
                                            .id(ElementId::NamedInteger(
                                                "gallery-header-cat".into(),
                                                ix as u64,
                                            ))
                                            .w_full()
                                            .h(px(32.0))
                                            .px(px(design_spacing_clone.padding_lg))
                                            .flex()
                                            .items_center()
                                            .bg(rgba(
                                                (design_colors_clone.background_secondary << 8)
                                                    | 0x80,
                                            ))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(design_colors_clone.accent))
                                                    .child(format!(
                                                        "── Group Headers: {} ──",
                                                        category.name()
                                                    )),
                                            )
                                            .into_any_element()
                                    }
                                    GalleryItem::GroupHeader(style) => render_group_header_item(
                                        ix,
                                        is_selected,
                                        style,
                                        &design_spacing_clone,
                                        &design_typography_clone,
                                        &design_visual_clone,
                                        &design_colors_clone,
                                    ),
                                    GalleryItem::IconCategoryHeader(category) => {
                                        // Icon category header
                                        div()
                                            .id(ElementId::NamedInteger(
                                                "gallery-icon-cat".into(),
                                                ix as u64,
                                            ))
                                            .w_full()
                                            .h(px(32.0))
                                            .px(px(design_spacing_clone.padding_lg))
                                            .flex()
                                            .items_center()
                                            .bg(rgba(
                                                (design_colors_clone.background_secondary << 8)
                                                    | 0x80,
                                            ))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(design_colors_clone.accent))
                                                    .child(format!(
                                                        "── Icons: {} ──",
                                                        category.name()
                                                    )),
                                            )
                                            .into_any_element()
                                    }
                                    GalleryItem::Icon(icon, _style) => {
                                        // Render icon item with SVG
                                        let icon_path = icon.external_path();
                                        let name_owned = icon.name().to_string();
                                        let desc_owned = icon.description().to_string();

                                        let mut item_div = div()
                                            .id(ElementId::NamedInteger(
                                                "gallery-icon".into(),
                                                ix as u64,
                                            ))
                                            .w_full()
                                            .h(px(LIST_ITEM_HEIGHT))
                                            .px(px(design_spacing_clone.padding_lg))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(design_spacing_clone.gap_md));

                                        if is_selected {
                                            item_div = item_div
                                                .bg(rgb(design_colors_clone.background_selected));
                                        }

                                        item_div
                                            // Icon preview with SVG
                                            .child(
                                                div()
                                                    .w(px(32.0))
                                                    .h(px(32.0))
                                                    .rounded(px(4.0))
                                                    .bg(rgba(
                                                        (design_colors_clone.background_secondary
                                                            << 8)
                                                            | 0x60,
                                                    ))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        svg()
                                                            .external_path(icon_path)
                                                            .size(px(16.0))
                                                            .text_color(rgb(
                                                                design_colors_clone.text_primary
                                                            )),
                                                    ),
                                            )
                                            // Name and description
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(gpui::FontWeight::MEDIUM)
                                                            .text_color(rgb(
                                                                design_colors_clone.text_primary
                                                            ))
                                                            .child(name_owned),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(
                                                                design_colors_clone.text_muted
                                                            ))
                                                            .overflow_x_hidden()
                                                            .child(desc_owned),
                                                    ),
                                            )
                                            .into_any_element()
                                    }
                                };
                                element
                            } else {
                                div()
                                    .id(ElementId::NamedInteger("gallery-empty".into(), ix as u64))
                                    .h(px(LIST_ITEM_HEIGHT))
                                    .into_any_element()
                            }
                        })
                        .collect()
                },
            )
            .w_full()
            .h_full()
            .track_scroll(&self.design_gallery_scroll_handle)
            .into_any_element()
        };

        // Build the full view
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("design_gallery")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Gallery icon
                    .child(div().text_xl().child("🎨"))
                    // Search input with blinking cursor
                    // ALIGNMENT FIX: Uses canonical cursor constants and negative margin for placeholder
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if input_is_empty {
                                rgb(text_muted)
                            } else {
                                rgb(text_primary)
                            })
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .mr(px(CURSOR_GAP_X))
                                        .when(self.cursor_visible, |d| d.bg(rgb(text_primary))),
                                )
                            })
                            .when(input_is_empty, |d| {
                                d.child(
                                    div()
                                        .ml(px(-(CURSOR_WIDTH + CURSOR_GAP_X)))
                                        .child(input_display.clone()),
                                )
                            })
                            .when(!input_is_empty, |d| d.child(input_display.clone()))
                            .when(!input_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .ml(px(CURSOR_GAP_X))
                                        .when(self.cursor_visible, |d| d.bg(rgb(text_primary))),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(format!("{} items", filtered_len)),
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(design_spacing.padding_lg))
                    .h(px(design_visual.border_thin))
                    .bg(rgba((ui_border << 8) | 0x60)),
            )
            // Main content area - just the list (no preview panel for gallery)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .h_full()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .py(px(design_spacing.padding_xs))
                    .child(list_element),
            )
            .into_any_element()
    }
}
