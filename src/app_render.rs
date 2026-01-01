impl ScriptListApp {
    /// Read the first N lines of a script file for preview
    #[allow(dead_code)]
    fn read_script_preview(path: &std::path::Path, max_lines: usize) -> String {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let preview: String = content
                    .lines()
                    .take(max_lines)
                    .collect::<Vec<_>>()
                    .join("\n");
                logging::log(
                    "UI",
                    &format!(
                        "Preview loaded: {} ({} lines read)",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        content.lines().count().min(max_lines)
                    ),
                );
                preview
            }
            Err(e) => {
                logging::log("UI", &format!("Preview error: {} - {}", path.display(), e));
                format!("Error reading file: {}", e)
            }
        }
    }

    /// Render toast notifications from the toast manager
    ///
    /// Toasts are positioned in the top-right corner and stack vertically.
    /// Each toast has its own dismiss callback that removes it from the manager.
    fn render_toasts(&mut self, _cx: &mut Context<Self>) -> Option<impl IntoElement> {
        // Tick the manager to handle auto-dismiss
        self.toast_manager.tick();

        // Clean up dismissed toasts
        self.toast_manager.cleanup();

        // Check if toasts need update (consume the flag to prevent repeated checks)
        // Note: We don't call cx.notify() here as it's an anti-pattern during render.
        // Toast updates are handled by timer-based refresh mechanisms.
        let _ = self.toast_manager.take_needs_notify();

        let visible = self.toast_manager.visible_toasts();
        if visible.is_empty() {
            return None;
        }

        // Use design tokens for consistent spacing
        let tokens = get_tokens(self.current_design);
        let spacing = tokens.spacing();

        // Build toast container (positioned in top-right via absolute positioning)
        let mut toast_container = div()
            .absolute()
            .top(px(spacing.padding_lg))
            .right(px(spacing.padding_lg))
            .flex()
            .flex_col()
            .gap(px(spacing.gap_sm))
            .w(px(380.0)); // Fixed width for toasts

        // Add each visible toast
        for notification in visible {
            // Clone the toast for rendering - unfortunately we need to recreate it
            // since Toast::render consumes self
            let toast_colors =
                ToastColors::from_theme(&self.theme, notification.toast.get_variant());
            let toast = Toast::new(notification.toast.get_message().clone(), toast_colors)
                .variant(notification.toast.get_variant())
                .duration_ms(notification.toast.get_duration_ms())
                .dismissible(true);

            // Add details if the toast has them
            let toast = toast.details_opt(notification.toast.get_details().cloned());

            toast_container = toast_container.child(toast);
        }

        Some(toast_container)
    }

    /// Render the preview panel showing details of the selected script/scriptlet
    fn render_preview_panel(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        // Get grouped results to map from selected_index to actual result (cached)
        // Clone to avoid borrow issues with self.selected_index access
        let selected_index = self.selected_index;
        let (grouped_items, flat_results) = self.get_grouped_results_cached();
        let grouped_items = grouped_items.clone();
        let flat_results = flat_results.clone();

        // Get the result index from the grouped item
        let selected_result = match grouped_items.get(selected_index) {
            Some(GroupedListItem::Item(idx)) => flat_results.get(*idx).cloned(),
            _ => None,
        };

        // Use design tokens for GLOBAL theming - design applies to ALL components
        let tokens = get_tokens(self.current_design);
        let colors = tokens.colors();
        let spacing = tokens.spacing();
        let typography = tokens.typography();
        let visual = tokens.visual();

        // Map design tokens to local variables (all designs use tokens now)
        let bg_main = colors.background;
        let ui_border = colors.border;
        let text_primary = colors.text_primary;
        let text_muted = colors.text_muted;
        let text_secondary = colors.text_secondary;
        let bg_search_box = colors.background_tertiary;
        let border_radius = visual.radius_md;
        let font_family = typography.font_family;

        // Preview panel container with left border separator
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
            .font_family(font_family);

        // P4: Compute match indices lazily for visible preview (only one result at a time)
        let computed_filter = self.computed_filter_text.clone();

        match selected_result {
            Some(ref result) => {
                // P4: Lazy match indices computation for preview panel
                let match_indices =
                    scripts::compute_match_indices_for_result(result, &computed_filter);

                match result {
                    scripts::SearchResult::Script(script_match) => {
                        let script = &script_match.script;

                        // Source indicator with match highlighting (e.g., "script: foo.ts")
                        let filename = &script_match.filename;
                        // P4: Use lazily computed indices instead of stored (empty) ones
                        let filename_indices = &match_indices.filename_indices;

                        // Render filename with highlighted matched characters
                        let path_segments =
                            render_path_with_highlights(filename, filename, filename_indices);
                        let accent_color = colors.accent;

                        let mut path_div = div()
                            .flex()
                            .flex_row()
                            .text_xs()
                            .font_family(typography.font_family_mono)
                            .pb(px(spacing.padding_xs))
                            .overflow_x_hidden()
                            .child(
                                div()
                                    .text_color(rgba((text_muted << 8) | 0x99))
                                    .child("script: "),
                            );

                        for (text, is_highlighted) in path_segments {
                            let color = if is_highlighted {
                                rgb(accent_color)
                            } else {
                                rgba((text_muted << 8) | 0x99)
                            };
                            path_div = path_div.child(div().text_color(color).child(text));
                        }

                        panel = panel.child(path_div);

                        // Script name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(text_primary))
                                .pb(px(spacing.padding_sm))
                                .child(format!("{}.{}", script.name, script.extension)),
                        );

                        // Description (if present)
                        if let Some(desc) = &script.description {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Description"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(desc.clone()),
                                    ),
                            );
                        }

                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm)),
                        );

                        // Code preview header
                        panel = panel.child(
                            div()
                                .text_xs()
                                .text_color(rgb(text_muted))
                                .pb(px(spacing.padding_sm))
                                .child("Code Preview"),
                        );

                        // Use cached syntax-highlighted lines (avoids file I/O and highlighting on every render)
                        let script_path = script.path.to_string_lossy().to_string();
                        let lang = script.extension.clone();
                        let lines = self
                            .get_or_update_preview_cache(&script_path, &lang)
                            .to_vec();

                        // Build code container - render line by line with monospace font
                        let mut code_container = div()
                            .w_full()
                            .min_w(px(280.))
                            .p(px(spacing.padding_md))
                            .rounded(px(border_radius))
                            .bg(rgba((bg_search_box << 8) | 0x80))
                            .overflow_hidden()
                            .flex()
                            .flex_col();

                        // Render each line as a row of spans with monospace font
                        for line in lines {
                            let mut line_div = div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .font_family(typography.font_family_mono)
                                .text_xs()
                                .min_h(px(spacing.padding_lg)); // Line height

                            if line.spans.is_empty() {
                                // Empty line - add a space to preserve height
                                line_div = line_div.child(" ");
                            } else {
                                for span in line.spans {
                                    line_div = line_div
                                        .child(div().text_color(rgb(span.color)).child(span.text));
                                }
                            }

                            code_container = code_container.child(line_div);
                        }

                        panel = panel.child(code_container);
                    }
                    scripts::SearchResult::Scriptlet(scriptlet_match) => {
                        let scriptlet = &scriptlet_match.scriptlet;

                        // Source indicator with match highlighting (e.g., "scriptlet: foo.md")
                        if let Some(ref display_file_path) = scriptlet_match.display_file_path {
                            // P4: Use lazily computed indices instead of stored (empty) ones
                            let filename_indices = &match_indices.filename_indices;

                            // Render filename with highlighted matched characters
                            let path_segments = render_path_with_highlights(
                                display_file_path,
                                display_file_path,
                                filename_indices,
                            );
                            let accent_color = colors.accent;

                            let mut path_div = div()
                                .flex()
                                .flex_row()
                                .text_xs()
                                .font_family(typography.font_family_mono)
                                .pb(px(spacing.padding_xs))
                                .overflow_x_hidden()
                                .child(
                                    div()
                                        .text_color(rgba((text_muted << 8) | 0x99))
                                        .child("scriptlet: "),
                                );

                            for (text, is_highlighted) in path_segments {
                                let color = if is_highlighted {
                                    rgb(accent_color)
                                } else {
                                    rgba((text_muted << 8) | 0x99)
                                };
                                path_div = path_div.child(div().text_color(color).child(text));
                            }

                            panel = panel.child(path_div);
                        }

                        // Scriptlet name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(text_primary))
                                .pb(px(spacing.padding_sm))
                                .child(scriptlet.name.clone()),
                        );

                        // Description (if present)
                        if let Some(desc) = &scriptlet.description {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Description"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(desc.clone()),
                                    ),
                            );
                        }

                        // Shortcut (if present)
                        if let Some(shortcut) = &scriptlet.shortcut {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Hotkey"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(shortcut.clone()),
                                    ),
                            );
                        }

                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm)),
                        );

                        // Content preview header
                        panel = panel.child(
                            div()
                                .text_xs()
                                .text_color(rgb(text_muted))
                                .pb(px(spacing.padding_sm))
                                .child("Content Preview"),
                        );

                        // Display scriptlet code with syntax highlighting (first 15 lines)
                        // Note: Scriptlets store code in memory, no file I/O needed (no cache benefit)
                        let code_preview: String = scriptlet
                            .code
                            .lines()
                            .take(15)
                            .collect::<Vec<_>>()
                            .join("\n");

                        // Determine language from tool (bash, js, etc.)
                        let lang = match scriptlet.tool.as_str() {
                            "bash" | "zsh" | "sh" => "bash",
                            "node" | "bun" => "js",
                            _ => &scriptlet.tool,
                        };
                        let lines = highlight_code_lines(&code_preview, lang);

                        // Build code container - render line by line with monospace font
                        let mut code_container = div()
                            .w_full()
                            .min_w(px(280.))
                            .p(px(spacing.padding_md))
                            .rounded(px(border_radius))
                            .bg(rgba((bg_search_box << 8) | 0x80))
                            .overflow_hidden()
                            .flex()
                            .flex_col();

                        // Render each line as a row of spans with monospace font
                        for line in lines {
                            let mut line_div = div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .font_family(typography.font_family_mono)
                                .text_xs()
                                .min_h(px(spacing.padding_lg)); // Line height

                            if line.spans.is_empty() {
                                // Empty line - add a space to preserve height
                                line_div = line_div.child(" ");
                            } else {
                                for span in line.spans {
                                    line_div = line_div
                                        .child(div().text_color(rgb(span.color)).child(span.text));
                                }
                            }

                            code_container = code_container.child(line_div);
                        }

                        panel = panel.child(code_container);
                    }
                    scripts::SearchResult::BuiltIn(builtin_match) => {
                        let builtin = &builtin_match.entry;

                        // Built-in name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(text_primary))
                                .pb(px(spacing.padding_sm))
                                .child(builtin.name.clone()),
                        );

                        // Description
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .pb(px(spacing.padding_md))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Description"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(builtin.description.clone()),
                                ),
                        );

                        // Keywords
                        if !builtin.keywords.is_empty() {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Keywords"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(builtin.keywords.join(", ")),
                                    ),
                            );
                        }

                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm)),
                        );

                        // Feature type indicator
                        let feature_type: String = match &builtin.feature {
                            builtins::BuiltInFeature::ClipboardHistory => {
                                "Clipboard History Manager".to_string()
                            }
                            builtins::BuiltInFeature::AppLauncher => {
                                "Application Launcher".to_string()
                            }
                            builtins::BuiltInFeature::App(name) => name.clone(),
                            builtins::BuiltInFeature::WindowSwitcher => {
                                "Window Manager".to_string()
                            }
                            builtins::BuiltInFeature::DesignGallery => "Design Gallery".to_string(),
                        };
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Feature Type"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(feature_type),
                                ),
                        );
                    }
                    scripts::SearchResult::App(app_match) => {
                        let app = &app_match.app;

                        // App name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(text_primary))
                                .pb(px(spacing.padding_sm))
                                .child(app.name.clone()),
                        );

                        // Path
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .pb(px(spacing.padding_md))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Path"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(app.path.to_string_lossy().to_string()),
                                ),
                        );

                        // Bundle ID (if available)
                        if let Some(bundle_id) = &app.bundle_id {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Bundle ID"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(bundle_id.clone()),
                                    ),
                            );
                        }

                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm)),
                        );

                        // Type indicator
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Type"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child("Application"),
                                ),
                        );
                    }
                    scripts::SearchResult::Window(window_match) => {
                        let window = &window_match.window;

                        // Window title header
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
                                .flex()
                                .flex_col()
                                .pb(px(spacing.padding_md))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Application"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(window.app.clone()),
                                ),
                        );

                        // Bounds
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .pb(px(spacing.padding_md))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Position & Size"),
                                )
                                .child(div().text_sm().text_color(rgb(text_secondary)).child(
                                    format!(
                                        "{}×{} at ({}, {})",
                                        window.bounds.width,
                                        window.bounds.height,
                                        window.bounds.x,
                                        window.bounds.y
                                    ),
                                )),
                        );

                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm)),
                        );

                        // Type indicator
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Type"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child("Window"),
                                ),
                        );
                    }
                }
            }
            None => {
                logging::log("UI", "Preview panel: No selection");
                // Empty state
                panel = panel.child(
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(text_muted))
                        .child(
                            if self.filter_text.is_empty()
                                && self.scripts.is_empty()
                                && self.scriptlets.is_empty()
                            {
                                "No scripts or snippets found"
                            } else if !self.filter_text.is_empty() {
                                "No matching scripts"
                            } else {
                                "Select a script to preview"
                            },
                        ),
                );
            }
        }

        panel
    }

    /// Get the ScriptInfo for the currently focused/selected script
    fn get_focused_script_info(&mut self) -> Option<ScriptInfo> {
        // Get grouped results to map from selected_index to actual result (cached)
        let (grouped_items, flat_results) = self.get_grouped_results_cached();
        // Clone to avoid borrow issues
        let grouped_items = grouped_items.clone();
        let flat_results = flat_results.clone();

        // Get the result index from the grouped item
        let result_idx = match grouped_items.get(self.selected_index) {
            Some(GroupedListItem::Item(idx)) => Some(*idx),
            _ => None,
        };

        if let Some(idx) = result_idx {
            if let Some(result) = flat_results.get(idx) {
                match result {
                    scripts::SearchResult::Script(m) => Some(ScriptInfo::new(
                        &m.script.name,
                        m.script.path.to_string_lossy(),
                    )),
                    scripts::SearchResult::Scriptlet(m) => {
                        // Scriptlets don't have a path, use name as identifier
                        Some(ScriptInfo::new(
                            &m.scriptlet.name,
                            format!("scriptlet:{}", &m.scriptlet.name),
                        ))
                    }
                    scripts::SearchResult::BuiltIn(m) => {
                        // Built-ins use their id as identifier
                        Some(ScriptInfo::new(
                            &m.entry.name,
                            format!("builtin:{}", &m.entry.id),
                        ))
                    }
                    scripts::SearchResult::App(m) => {
                        // Apps use their path as identifier
                        Some(ScriptInfo::new(
                            &m.app.name,
                            m.app.path.to_string_lossy().to_string(),
                        ))
                    }
                    scripts::SearchResult::Window(m) => {
                        // Windows use their id as identifier
                        Some(ScriptInfo::new(
                            &m.window.title,
                            format!("window:{}", m.window.id),
                        ))
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn render_script_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Get grouped or flat results based on filter state (cached) - MUST come first
        // to avoid borrow conflicts with theme access below
        // When filter is empty, use frecency-grouped results with RECENT/MAIN sections
        // When filtering, use flat fuzzy search results
        let (grouped_items, flat_results) = self.get_grouped_results_cached();
        // Clone for use in closures and to avoid borrow issues
        let grouped_items = grouped_items.clone();
        let flat_results = flat_results.clone();

        // Get design tokens for current design variant
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_visual = tokens.visual();
        let design_typography = tokens.typography();
        let theme = &self.theme;

        // For Default design, use theme.colors for backward compatibility
        // For other designs, use design tokens
        let is_default_design = self.current_design == DesignVariant::Default;

        // P4: Pre-compute theme values using ListItemColors
        let _list_colors = ListItemColors::from_theme(theme);
        logging::log_debug("PERF", "P4: Using ListItemColors for render closure");

        let item_count = grouped_items.len();
        let _total_len = self.scripts.len() + self.scriptlets.len();

        // Handle edge cases - keep selected_index in valid bounds
        // Also skip section headers when adjusting bounds
        if item_count > 0 {
            if self.selected_index >= item_count {
                self.selected_index = item_count.saturating_sub(1);
            }
            // If we land on a section header, move to first valid item
            if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(self.selected_index)
            {
                // Move down to find first Item
                for (i, item) in grouped_items.iter().enumerate().skip(self.selected_index) {
                    if matches!(item, GroupedListItem::Item(_)) {
                        self.selected_index = i;
                        break;
                    }
                }
            }
        }

        // Build script list using uniform_list for proper virtualized scrolling
        // Use design tokens for empty state styling
        let empty_text_color = if is_default_design {
            theme.colors.text.muted
        } else {
            design_colors.text_muted
        };
        let empty_font_family = if is_default_design {
            ".AppleSystemUIFont"
        } else {
            design_typography.font_family
        };

        let list_element: AnyElement = if item_count == 0 {
            div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(empty_text_color))
                .font_family(empty_font_family)
                .child(if self.filter_text.is_empty() {
                    "No scripts or snippets found".to_string()
                } else {
                    format!("No results match '{}'", self.filter_text)
                })
                .into_any_element()
        } else {
            // Use GPUI's list() component for variable-height items
            // Section headers render at 24px, regular items at 48px
            // This gives true visual compression for headers without the uniform_list hack

            // Clone grouped_items and flat_results for the closure
            let grouped_items_clone = grouped_items.clone();
            let flat_results_clone = flat_results.clone();

            // Calculate scrollbar parameters
            // Estimate visible items based on typical container height
            // Note: With variable heights, this is approximate
            let estimated_container_height = 400.0_f32; // Typical visible height
            let visible_items = (estimated_container_height / LIST_ITEM_HEIGHT) as usize;

            // Use selected_index as approximate scroll offset
            let scroll_offset = if self.selected_index > visible_items.saturating_sub(1) {
                self.selected_index.saturating_sub(visible_items / 2)
            } else {
                0
            };

            // Get scrollbar colors from theme or design
            let scrollbar_colors = if is_default_design {
                ScrollbarColors::from_theme(theme)
            } else {
                ScrollbarColors::from_design(&design_colors)
            };

            // Create scrollbar (only visible if content overflows and scrolling is active)
            let scrollbar =
                Scrollbar::new(item_count, visible_items, scroll_offset, scrollbar_colors)
                    .container_height(estimated_container_height)
                    .visible(self.is_scrolling);

            // Update list state if item count changed
            if self.main_list_state.item_count() != item_count {
                self.main_list_state.reset(item_count);
            }

            // Scroll to reveal selected item
            self.main_list_state
                .scroll_to_reveal_item(self.selected_index);

            // Capture entity handle for use in the render closure
            let entity = cx.entity();

            // Clone values needed in the closure (can't access self in FnMut)
            let theme_colors = ListItemColors::from_theme(&self.theme);
            let current_design = self.current_design;

            let variable_height_list =
                list(self.main_list_state.clone(), move |ix, _window, cx| {
                    // Access entity state inside the closure
                    entity.update(cx, |this, cx| {
                        let current_selected = this.selected_index;
                        let current_hovered = this.hovered_index;

                        if let Some(grouped_item) = grouped_items_clone.get(ix) {
                            match grouped_item {
                                GroupedListItem::SectionHeader(label) => {
                                    // Section header at 24px height (SECTION_HEADER_HEIGHT)
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "section-header".into(),
                                            ix as u64,
                                        ))
                                        .h(px(SECTION_HEADER_HEIGHT))
                                        .child(render_section_header(label, theme_colors))
                                        .into_any_element()
                                }
                                GroupedListItem::Item(result_idx) => {
                                    // Regular item at 48px height (LIST_ITEM_HEIGHT)
                                    if let Some(result) = flat_results_clone.get(*result_idx) {
                                        let is_selected = ix == current_selected;
                                        let is_hovered = current_hovered == Some(ix);

                                        // Create hover handler
                                        let hover_handler = cx.listener(
                                            move |this: &mut ScriptListApp,
                                                  hovered: &bool,
                                                  _window,
                                                  cx| {
                                                let now = std::time::Instant::now();
                                                const HOVER_DEBOUNCE_MS: u64 = 16;

                                                if *hovered {
                                                    // Mouse entered - set hovered_index with debounce
                                                    if this.hovered_index != Some(ix)
                                                        && now
                                                            .duration_since(this.last_hover_notify)
                                                            .as_millis()
                                                            >= HOVER_DEBOUNCE_MS as u128
                                                    {
                                                        this.hovered_index = Some(ix);
                                                        this.last_hover_notify = now;
                                                        cx.notify();
                                                    }
                                                } else if this.hovered_index == Some(ix) {
                                                    // Mouse left - clear hovered_index if it was this item
                                                    this.hovered_index = None;
                                                    this.last_hover_notify = now;
                                                    cx.notify();
                                                }
                                            },
                                        );

                                        // Create click handler
                                        let click_handler = cx.listener(
                                            move |this: &mut ScriptListApp,
                                                  _event: &gpui::ClickEvent,
                                                  _window,
                                                  cx| {
                                                if this.selected_index != ix {
                                                    this.selected_index = ix;
                                                    cx.notify();
                                                }
                                            },
                                        );

                                        // Dispatch to design-specific item renderer
                                        let item_element = render_design_item(
                                            current_design,
                                            result,
                                            ix,
                                            is_selected,
                                            is_hovered,
                                            theme_colors,
                                        );

                                        div()
                                            .id(ElementId::NamedInteger(
                                                "script-item".into(),
                                                ix as u64,
                                            ))
                                            .h(px(LIST_ITEM_HEIGHT)) // Explicit 48px height
                                            .on_hover(hover_handler)
                                            .on_click(click_handler)
                                            .child(item_element)
                                            .into_any_element()
                                    } else {
                                        // Fallback for missing result
                                        div().h(px(LIST_ITEM_HEIGHT)).into_any_element()
                                    }
                                }
                            }
                        } else {
                            // Fallback for out-of-bounds index
                            div().h(px(LIST_ITEM_HEIGHT)).into_any_element()
                        }
                    })
                })
                // Enable proper scroll handling for mouse wheel/trackpad
                // ListSizingBehavior::Infer sets overflow.y = Overflow::Scroll internally
                // which is required for the list's hitbox to capture scroll wheel events
                .with_sizing_behavior(ListSizingBehavior::Infer)
                .h_full();

            // Wrap list in a relative container with scrollbar overlay
            // The list() component with ListSizingBehavior::Infer handles scroll internally
            // No custom on_scroll_wheel handler needed - let GPUI handle it natively
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .w_full()
                .h_full()
                .child(variable_height_list)
                .child(scrollbar)
                .into_any_element()
        };

        // Log panel
        let log_panel = if self.show_logs {
            let logs = logging::get_last_logs(10);
            let mut log_container = div()
                .flex()
                .flex_col()
                .w_full()
                .bg(rgb(theme.colors.background.log_panel))
                .border_t_1()
                .border_color(rgb(theme.colors.ui.border))
                .p(px(design_spacing.padding_md))
                .max_h(px(120.))
                .font_family("SF Mono");

            for log_line in logs.iter().rev() {
                log_container = log_container.child(
                    div()
                        .text_color(rgb(theme.colors.ui.success))
                        .text_xs()
                        .child(log_line.clone()),
                );
            }
            Some(log_container)
        } else {
            None
        };

        let filter_display = if self.filter_text.is_empty() {
            SharedString::from(DEFAULT_PLACEHOLDER)
        } else {
            SharedString::from(self.filter_text.clone())
        };
        let filter_is_empty = self.filter_text.is_empty();

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                let has_cmd = event.keystroke.modifiers.platform;

                // Check SDK action shortcuts FIRST (before built-in shortcuts)
                // This allows scripts to override default shortcuts via setActions()
                if !this.action_shortcuts.is_empty() {
                    let key_combo =
                        shortcuts::keystroke_to_shortcut(&key_str, &event.keystroke.modifiers);
                    if let Some(action_name) = this.action_shortcuts.get(&key_combo).cloned() {
                        logging::log(
                            "ACTIONS",
                            &format!(
                                "SDK action shortcut matched: '{}' -> '{}'",
                                key_combo, action_name
                            ),
                        );
                        if this.trigger_action_by_name(&action_name, cx) {
                            return;
                        }
                    }
                }

                if has_cmd {
                    let has_shift = event.keystroke.modifiers.shift;

                    match key_str.as_str() {
                        "l" => {
                            this.toggle_logs(cx);
                            return;
                        }
                        "k" => {
                            this.toggle_actions(cx, window);
                            return;
                        }
                        // Cmd+1 cycles through all designs
                        "1" => {
                            this.cycle_design(cx);
                            return;
                        }
                        // Script context shortcuts (require a selected script)
                        "e" => {
                            // Cmd+E - Edit Script
                            this.handle_action("edit_script".to_string(), cx);
                            return;
                        }
                        "f" if has_shift => {
                            // Cmd+Shift+F - Reveal in Finder
                            this.handle_action("reveal_in_finder".to_string(), cx);
                            return;
                        }
                        "c" if has_shift => {
                            // Cmd+Shift+C - Copy Path
                            this.handle_action("copy_path".to_string(), cx);
                            return;
                        }
                        // Global shortcuts
                        "n" => {
                            // Cmd+N - Create Script
                            this.handle_action("create_script".to_string(), cx);
                            return;
                        }
                        "r" => {
                            // Cmd+R - Reload Scripts
                            this.handle_action("reload_scripts".to_string(), cx);
                            return;
                        }
                        "," => {
                            // Cmd+, - Settings
                            this.handle_action("settings".to_string(), cx);
                            return;
                        }
                        "q" => {
                            // Cmd+Q - Quit
                            this.handle_action("quit".to_string(), cx);
                            return;
                        }
                        _ => {}
                    }
                }

                // If actions popup is open, route keyboard events to it
                if this.show_actions_popup {
                    if let Some(ref dialog) = this.actions_dialog {
                        match key_str.as_str() {
                            "up" | "arrowup" => {
                                dialog.update(cx, |d, cx| d.move_up(cx));
                                return;
                            }
                            "down" | "arrowdown" => {
                                dialog.update(cx, |d, cx| d.move_down(cx));
                                return;
                            }
                            "enter" => {
                                // Get the selected action and execute it
                                let action_id = dialog.read(cx).get_selected_action_id();
                                let should_close = dialog.read(cx).selected_action_should_close();
                                if let Some(action_id) = action_id {
                                    logging::log(
                                        "ACTIONS",
                                        &format!(
                                            "Executing action: {} (close={})",
                                            action_id, should_close
                                        ),
                                    );
                                    // Only close if action has close: true (default)
                                    if should_close {
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        this.focused_input = FocusedInput::MainFilter;
                                        window.focus(&this.focus_handle, cx);
                                    }
                                    this.handle_action(action_id, cx);
                                }
                                return;
                            }
                            "escape" => {
                                this.show_actions_popup = false;
                                this.actions_dialog = None;
                                this.focused_input = FocusedInput::MainFilter;
                                window.focus(&this.focus_handle, cx);
                                cx.notify();
                                return;
                            }
                            "backspace" => {
                                dialog.update(cx, |d, cx| d.handle_backspace(cx));
                                return;
                            }
                            _ => {
                                // Route character input to the dialog for search
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            dialog.update(cx, |d, cx| d.handle_char(ch, cx));
                                        }
                                    }
                                }
                                return;
                            }
                        }
                    }
                }

                match key_str.as_str() {
                    "up" | "arrowup" => {
                        let _key_perf = crate::perf::KeyEventPerfGuard::new();
                        match this.nav_coalescer.record(NavDirection::Up) {
                            NavRecord::ApplyImmediate => this.move_selection_up(cx),
                            NavRecord::Coalesced => {}
                            NavRecord::FlushOld { dir, delta } => {
                                if delta != 0 {
                                    this.apply_nav_delta(dir, delta, cx);
                                }
                                this.move_selection_up(cx);
                            }
                        }
                        this.ensure_nav_flush_task(cx);
                    }
                    "down" | "arrowdown" => {
                        let _key_perf = crate::perf::KeyEventPerfGuard::new();
                        match this.nav_coalescer.record(NavDirection::Down) {
                            NavRecord::ApplyImmediate => this.move_selection_down(cx),
                            NavRecord::Coalesced => {}
                            NavRecord::FlushOld { dir, delta } => {
                                if delta != 0 {
                                    this.apply_nav_delta(dir, delta, cx);
                                }
                                this.move_selection_down(cx);
                            }
                        }
                        this.ensure_nav_flush_task(cx);
                    }
                    "enter" => this.execute_selected(cx),
                    "escape" => {
                        if !this.filter_text.is_empty() {
                            this.update_filter(None, false, true, cx);
                        } else {
                            // Update visibility state for hotkey toggle
                            WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                            // Reset UI state before hiding (clears selection, scroll position, filter)
                            logging::log("UI", "Resetting to script list before hiding via Escape");
                            this.reset_to_script_list(cx);
                            logging::log("HOTKEY", "Window hidden via Escape key");
                            // PERF: Measure window hide latency
                            let hide_start = std::time::Instant::now();
                            cx.hide();
                            let hide_elapsed = hide_start.elapsed();
                            logging::log(
                                "PERF",
                                &format!(
                                    "Window hide (Escape) took {:.2}ms",
                                    hide_elapsed.as_secs_f64() * 1000.0
                                ),
                            );
                        }
                    }
                    "backspace" => this.update_filter(None, true, false, cx),
                    "space" | " " => {
                        // Check if current filter text matches an alias
                        // If so, execute the matching script/scriptlet immediately
                        if !this.filter_text.is_empty() {
                            if let Some(alias_match) = this.find_alias_match(&this.filter_text) {
                                logging::log(
                                    "ALIAS",
                                    &format!("Alias '{}' triggered execution", this.filter_text),
                                );
                                match alias_match {
                                    AliasMatch::Script(script) => {
                                        this.execute_interactive(&script, cx);
                                    }
                                    AliasMatch::Scriptlet(scriptlet) => {
                                        this.execute_scriptlet(&scriptlet, cx);
                                    }
                                }
                                // Clear filter after alias execution
                                this.update_filter(None, false, true, cx);
                                return;
                            }
                        }
                        // No alias match - add space to filter as normal character
                        this.update_filter(Some(' '), false, false, cx);
                    }
                    _ => {
                        // Allow all printable characters (not control chars like Tab, Escape)
                        // This enables searching for filenames with special chars like ".ts", ".md"
                        if let Some(ref key_char) = event.keystroke.key_char {
                            if let Some(ch) = key_char.chars().next() {
                                if !ch.is_control() {
                                    this.update_filter(Some(ch), false, false, cx);
                                }
                            }
                        }
                    }
                }
            },
        );

        // Main container with system font and transparency
        // Use theme opacity settings for background transparency
        let opacity = self.theme.get_opacity();

        // Use design tokens for background color (or theme for Default design)
        let bg_hex = if is_default_design {
            theme.colors.background.main
        } else {
            design_colors.background
        };
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);

        // Create box shadows from theme
        let box_shadows = self.create_box_shadows();

        // Use design tokens for border radius
        let border_radius = if is_default_design {
            12.0 // Default radius
        } else {
            design_visual.radius_lg
        };

        // Use design tokens for text color
        let text_primary = if is_default_design {
            theme.colors.text.primary
        } else {
            design_colors.text_primary
        };

        // Use design tokens for font family
        let font_family = if is_default_design {
            ".AppleSystemUIFont"
        } else {
            design_typography.font_family
        };

        let mut main_div = div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(border_radius))
            .text_color(rgb(text_primary))
            .font_family(font_family)
            .key_context("script_list")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header: Search Input + Run + Actions + Logo
            // Use design tokens for spacing and colors
            .child({
                // Design token values for header
                let header_padding_x = if is_default_design {
                    16.0
                } else {
                    design_spacing.padding_lg
                };
                let header_padding_y = if is_default_design {
                    8.0
                } else {
                    design_spacing.padding_sm
                };
                let header_gap = if is_default_design {
                    12.0
                } else {
                    design_spacing.gap_md
                };
                let text_muted = if is_default_design {
                    theme.colors.text.muted
                } else {
                    design_colors.text_muted
                };
                let text_dimmed = if is_default_design {
                    theme.colors.text.dimmed
                } else {
                    design_colors.text_dimmed
                };
                let accent_color = if is_default_design {
                    theme.colors.accent.selected
                } else {
                    design_colors.accent
                };

                div()
                    .w_full()
                    .px(px(header_padding_x))
                    .py(px(header_padding_y))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(header_gap))
                    // Search input with blinking cursor
                    // Cursor appears at LEFT when input is empty (before placeholder text)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if filter_is_empty {
                                rgb(text_muted)
                            } else {
                                rgb(text_primary)
                            })
                            // When empty: cursor FIRST (at left), then placeholder
                            // When typing: text, then cursor at end
                            //
                            // ALIGNMENT FIX: The left cursor (when empty) takes up space
                            // (CURSOR_WIDTH + CURSOR_GAP_X). We apply a negative margin to the
                            // placeholder text to pull it back by that amount, so placeholder
                            // and typed text share the same starting x-position.
                            .when(filter_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .mr(px(CURSOR_GAP_X))
                                        .when(
                                            self.focused_input == FocusedInput::MainFilter
                                                && self.cursor_visible,
                                            |d| d.bg(rgb(text_primary)),
                                        ),
                                )
                            })
                            // Display text - with negative margin for placeholder alignment
                            .when(filter_is_empty, |d| {
                                d.child(
                                    div()
                                        .ml(px(-(CURSOR_WIDTH + CURSOR_GAP_X)))
                                        .child(filter_display.clone()),
                                )
                            })
                            .when(!filter_is_empty, |d| d.child(filter_display.clone()))
                            .when(!filter_is_empty, |d| {
                                d.child(
                                    div()
                                        .w(px(CURSOR_WIDTH))
                                        .h(px(CURSOR_HEIGHT_LG))
                                        .my(px(CURSOR_MARGIN_Y))
                                        .ml(px(CURSOR_GAP_X))
                                        .when(
                                            self.focused_input == FocusedInput::MainFilter
                                                && self.cursor_visible,
                                            |d| d.bg(rgb(text_primary)),
                                        ),
                                )
                            }),
                    )
                    // CLS-FREE ACTIONS AREA: Fixed-size relative container with stacked children
                    // Both states are always rendered at the same position, visibility toggled via opacity
                    // This prevents any layout shift when toggling between Run/Actions and search input
                    .child({
                        let button_colors = ButtonColors::from_theme(&self.theme);
                        let handle_run = cx.entity().downgrade();
                        let handle_actions = cx.entity().downgrade();
                        let show_actions = self.show_actions_popup;

                        // Get actions search text from the dialog
                        let search_text = self
                            .actions_dialog
                            .as_ref()
                            .map(|dialog| dialog.read(cx).search_text.clone())
                            .unwrap_or_default();
                        let search_is_empty = search_text.is_empty();
                        let search_display = if search_is_empty {
                            SharedString::from("Search actions...")
                        } else {
                            SharedString::from(search_text.clone())
                        };

                        // Outer container: relative positioned, fixed height to match header
                        div()
                            .relative()
                            .h(px(28.)) // Fixed height to prevent vertical CLS
                            .flex()
                            .items_center()
                            // Run + Actions buttons - absolute positioned, hidden when actions shown
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    // Visibility: hidden when actions popup is shown
                                    .when(show_actions, |d| d.opacity(0.).invisible())
                                    // Run button with click handler
                                    .child(
                                        Button::new("Run", button_colors)
                                            .variant(ButtonVariant::Ghost)
                                            .shortcut("↵")
                                            .on_click(Box::new(move |_, _window, cx| {
                                                if let Some(app) = handle_run.upgrade() {
                                                    app.update(cx, |this, cx| {
                                                        this.execute_selected(cx);
                                                    });
                                                }
                                            })),
                                    )
                                    .child(
                                        div()
                                            .mx(px(4.)) // Horizontal margin for spacing
                                            .text_color(rgba((text_dimmed << 8) | 0x60)) // Reduced opacity (60%)
                                            .text_sm() // Slightly smaller text
                                            .child("|"),
                                    )
                                    // Actions button with click handler
                                    .child(
                                        Button::new("Actions", button_colors)
                                            .variant(ButtonVariant::Ghost)
                                            .shortcut("⌘ K")
                                            .on_click(Box::new(move |_, window, cx| {
                                                if let Some(app) = handle_actions.upgrade() {
                                                    app.update(cx, |this, cx| {
                                                        this.toggle_actions(cx, window);
                                                    });
                                                }
                                            })),
                                    )
                                    .child(
                                        div()
                                            .mx(px(4.)) // Horizontal margin for spacing
                                            .text_color(rgba((text_dimmed << 8) | 0x60)) // Reduced opacity (60%)
                                            .text_sm() // Slightly smaller text
                                            .child("|"),
                                    ),
                            )
                            // Actions search input - absolute positioned, visible when actions shown
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    .gap(px(8.))
                                    // Visibility: hidden when actions popup is NOT shown
                                    .when(!show_actions, |d| d.opacity(0.).invisible())
                                    // ⌘K indicator
                                    .child(div().text_color(rgb(text_dimmed)).text_xs().child("⌘K"))
                                    // Search input display - compact style matching buttons
                                    // CRITICAL: Fixed width prevents resize when typing
                                    .child(
                                        div()
                                            .flex_shrink_0() // PREVENT flexbox from shrinking this
                                            .w(px(130.0)) // Compact width
                                            .min_w(px(130.0))
                                            .max_w(px(130.0))
                                            .h(px(24.0)) // Comfortable height with padding
                                            .min_h(px(24.0))
                                            .max_h(px(24.0))
                                            .overflow_hidden()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .px(px(8.)) // Comfortable horizontal padding
                                            .rounded(px(4.)) // Match button border radius
                                            // ALWAYS show background - just vary intensity
                                            .bg(rgba(
                                                (theme.colors.background.search_box << 8)
                                                    | if search_is_empty { 0x40 } else { 0x80 },
                                            ))
                                            .border_1()
                                            // ALWAYS show border - just vary intensity
                                            .border_color(rgba(
                                                (accent_color << 8)
                                                    | if search_is_empty { 0x20 } else { 0x40 },
                                            ))
                                            .text_sm()
                                            .text_color(if search_is_empty {
                                                rgb(text_muted)
                                            } else {
                                                rgb(text_primary)
                                            })
                                            // Cursor before placeholder when empty
                                            .when(search_is_empty, |d| {
                                                d.child(
                                                    div()
                                                        .w(px(2.))
                                                        .h(px(14.)) // Cursor height for comfortable input
                                                        .mr(px(2.))
                                                        .rounded(px(1.))
                                                        .when(
                                                            self.focused_input
                                                                == FocusedInput::ActionsSearch
                                                                && self.cursor_visible,
                                                            |d| d.bg(rgb(accent_color)),
                                                        ),
                                                )
                                            })
                                            .child(search_display)
                                            // Cursor after text when not empty
                                            .when(!search_is_empty, |d| {
                                                d.child(
                                                    div()
                                                        .w(px(2.))
                                                        .h(px(14.)) // Cursor height for comfortable input
                                                        .ml(px(2.))
                                                        .rounded(px(1.))
                                                        .when(
                                                            self.focused_input
                                                                == FocusedInput::ActionsSearch
                                                                && self.cursor_visible,
                                                            |d| d.bg(rgb(accent_color)),
                                                        ),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .mx(px(4.)) // Horizontal margin for spacing
                                            .text_color(rgba((text_dimmed << 8) | 0x60)) // Reduced opacity (60%)
                                            .text_sm() // Slightly smaller text
                                            .child("|"),
                                    ),
                            )
                    })
                    // Script Kit Logo - ALWAYS visible
                    // Size slightly larger than text for visual presence
                    .child(
                        svg()
                            .external_path(utils::get_logo_path())
                            .size(px(16.)) // Slightly larger than text_sm for visual presence
                            .text_color(rgb(accent_color)),
                    )
            })
            // Subtle divider - semi-transparent
            // Use design tokens for border color and spacing
            .child({
                let divider_margin = if is_default_design {
                    16.0
                } else {
                    design_spacing.margin_lg
                };
                let border_color = if is_default_design {
                    theme.colors.ui.border
                } else {
                    design_colors.border
                };
                let border_width = if is_default_design {
                    1.0
                } else {
                    design_visual.border_thin
                };

                div()
                    .mx(px(divider_margin))
                    .h(px(border_width))
                    .bg(rgba((border_color << 8) | 0x60))
            });

        // Main content area - 50/50 split: List on left, Preview on right
        main_div = main_div
            // Uses min_h(px(0.)) to prevent flex children from overflowing
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.)) // Critical: allows flex container to shrink properly
                    .w_full()
                    .overflow_hidden()
                    // Left side: Script list (50% width) - uses uniform_list for auto-scrolling
                    .child(
                        div()
                            .w_1_2() // 50% width
                            .h_full() // Take full height
                            .min_h(px(0.)) // Allow shrinking
                            .child(list_element),
                    )
                    // Right side: Preview panel (50% width) with actions overlay
                    // Preview ALWAYS renders, actions panel overlays on top when visible
                    .child(
                        div()
                            .relative() // Enable absolute positioning for overlay
                            .w_1_2() // 50% width
                            .h_full() // Take full height
                            .min_h(px(0.)) // Allow shrinking
                            .overflow_hidden()
                            // Preview panel ALWAYS renders (visible behind actions overlay)
                            .child(self.render_preview_panel(cx))
                            // Actions dialog overlays on top using absolute positioning
                            // Includes a backdrop to capture clicks outside the dialog
                            .when_some(
                                if self.show_actions_popup {
                                    self.actions_dialog.clone()
                                } else {
                                    None
                                },
                                |d, dialog| {
                                    // Create click handler for backdrop to dismiss dialog
                                    let backdrop_click = cx.listener(|this: &mut Self, _event: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>| {
                                        logging::log("FOCUS", "Actions backdrop clicked - dismissing dialog");
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        this.focused_input = FocusedInput::MainFilter;
                                        window.focus(&this.focus_handle, cx);
                                        cx.notify();
                                    });

                                    d.child(
                                        div()
                                            .absolute()
                                            .inset_0() // Cover entire preview area
                                            // Backdrop layer - captures clicks outside the dialog
                                            .child(
                                                div()
                                                    .id("actions-backdrop")
                                                    .absolute()
                                                    .inset_0()
                                                    .on_click(backdrop_click)
                                            )
                                            // Dialog container - positioned at top-right
                                            .child(
                                                div()
                                                    .absolute()
                                                    .inset_0()
                                                    .flex()
                                                    .justify_end()
                                                    .pr(px(8.)) // Small padding from right edge
                                                    .pt(px(8.)) // Small padding from top
                                                    .child(dialog),
                                            ),
                                    )
                                },
                            ),
                    ),
            );

        if let Some(panel) = log_panel {
            main_div = main_div.child(panel);
        }

        // Wrap in relative container for toast overlay positioning
        let mut container = div().relative().w_full().h_full().child(main_div);

        // Add toast notifications overlay (top-right)
        if let Some(toasts) = self.render_toasts(cx) {
            container = container.child(toasts);
        }

        // Note: HUD overlay is added at the top-level render() method for all views

        container.into_any_element()
    }


    fn render_actions_dialog(&mut self, cx: &mut Context<Self>) -> AnyElement {
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

        // Key handler for actions dialog
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  _window: &mut Window,
                  cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                logging::log("KEY", &format!("ActionsDialog key: '{}'", key_str));

                if key_str.as_str() == "escape" {
                    logging::log("KEY", "ESC in ActionsDialog - returning to script list");
                    this.current_view = AppView::ScriptList;
                    cx.notify();
                }
            },
        );

        // Simple actions dialog stub with design tokens
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .rounded(px(design_visual.radius_lg))
            .p(px(design_spacing.padding_xl))
            .text_color(rgb(design_colors.text_primary))
            .font_family(design_typography.font_family)
            .key_context("actions_dialog")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(div().text_lg().child("Actions (Cmd+K)"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(design_colors.text_muted))
                    .mt(px(design_spacing.margin_md))
                    .child("• Create script\n• Edit script\n• Reload\n• Settings\n• Quit"),
            )
            .child(
                div()
                    .mt(px(design_spacing.margin_lg))
                    .text_xs()
                    .text_color(rgb(design_colors.text_dimmed))
                    .child("Press Esc to close"),
            )
            .into_any_element()
    }
}

/// Helper function to render a group header style item with actual visual styling
fn render_group_header_item(
    ix: usize,
    is_selected: bool,
    style: &designs::group_header_variations::GroupHeaderStyle,
    spacing: &designs::DesignSpacing,
    typography: &designs::DesignTypography,
    visual: &designs::DesignVisual,
    colors: &designs::DesignColors,
) -> AnyElement {
    use designs::group_header_variations::GroupHeaderStyle;

    let name_owned = style.name().to_string();
    let desc_owned = style.description().to_string();

    let mut item_div = div()
        .id(ElementId::NamedInteger("gallery-header".into(), ix as u64))
        .w_full()
        .h(px(LIST_ITEM_HEIGHT))
        .px(px(spacing.padding_lg))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(spacing.gap_md));

    if is_selected {
        item_div = item_div.bg(rgb(colors.background_selected));
    }

    // Create the preview element based on the style
    let preview = match style {
        // Text Only styles - vary font weight and style
        GroupHeaderStyle::UppercaseLeft => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(rgb(colors.text_secondary))
            .child("MAIN"),
        GroupHeaderStyle::UppercaseCenter => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(rgb(colors.text_secondary))
            .child("MAIN"),
        GroupHeaderStyle::SmallCapsLeft => {
            div()
                .w(px(140.0))
                .h(px(28.0))
                .rounded(px(visual.radius_sm))
                .bg(rgba((colors.background_secondary << 8) | 0x60))
                .flex()
                .items_center()
                .px(px(8.0))
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(rgb(colors.text_secondary))
                .child("MAIN") // Would use font-variant: small-caps if available
        }
        GroupHeaderStyle::BoldLeft => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(rgb(colors.text_primary))
            .child("MAIN"),
        GroupHeaderStyle::LightLeft => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .font_weight(gpui::FontWeight::LIGHT)
            .text_color(rgb(colors.text_muted))
            .child("MAIN"),
        GroupHeaderStyle::MonospaceLeft => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .font_family(typography.font_family_mono)
            .text_color(rgb(colors.text_secondary))
            .child("MAIN"),

        // With Lines styles
        GroupHeaderStyle::LineLeft => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .child(div().w(px(24.0)).h(px(1.0)).bg(rgb(colors.border)))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            ),
        GroupHeaderStyle::LineRight => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            )
            .child(div().flex_1().h(px(1.0)).bg(rgb(colors.border))),
        GroupHeaderStyle::LineBothSides => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .child(div().flex_1().h(px(1.0)).bg(rgb(colors.border)))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            )
            .child(div().flex_1().h(px(1.0)).bg(rgb(colors.border))),
        GroupHeaderStyle::LineBelow => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_col()
            .justify_center()
            .px(px(8.0))
            .gap(px(2.0))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            )
            .child(div().w(px(40.0)).h(px(1.0)).bg(rgb(colors.border))),
        GroupHeaderStyle::LineAbove => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_col()
            .justify_center()
            .px(px(8.0))
            .gap(px(2.0))
            .child(div().w(px(40.0)).h(px(1.0)).bg(rgb(colors.border)))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            ),
        GroupHeaderStyle::DoubleLine => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap(px(1.0))
            .child(div().w(px(100.0)).h(px(1.0)).bg(rgb(colors.border)))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            )
            .child(div().w(px(100.0)).h(px(1.0)).bg(rgb(colors.border))),

        // With Background styles
        GroupHeaderStyle::PillBackground => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .child(
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(10.0))
                    .bg(rgba((colors.accent << 8) | 0x30))
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.accent))
                    .child("MAIN"),
            ),
        GroupHeaderStyle::FullWidthBackground => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.accent << 8) | 0x20))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(rgb(colors.text_primary))
            .child("MAIN"),
        GroupHeaderStyle::SubtleBackground => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x90))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(rgb(colors.text_secondary))
            .child("MAIN"),
        GroupHeaderStyle::GradientFade => {
            // Simulated with opacity fade
            div()
                .w(px(140.0))
                .h(px(28.0))
                .rounded(px(visual.radius_sm))
                .bg(rgba((colors.background_secondary << 8) | 0x60))
                .flex()
                .items_center()
                .px(px(8.0))
                .child(
                    div()
                        .px(px(16.0))
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(rgb(colors.text_secondary))
                        .child("~  MAIN  ~"),
                )
        }
        GroupHeaderStyle::BorderedBox => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .child(
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .border_1()
                    .border_color(rgb(colors.border))
                    .rounded(px(2.0))
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            ),

        // Minimal styles
        GroupHeaderStyle::DotPrefix => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .child(
                div()
                    .w(px(4.0))
                    .h(px(4.0))
                    .rounded(px(2.0))
                    .bg(rgb(colors.text_muted)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            ),
        GroupHeaderStyle::DashPrefix => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .text_color(rgb(colors.text_secondary))
            .child("- MAIN"),
        GroupHeaderStyle::BulletPrefix => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .child(
                div()
                    .w(px(6.0))
                    .h(px(6.0))
                    .rounded(px(3.0))
                    .bg(rgb(colors.accent)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            ),
        GroupHeaderStyle::ArrowPrefix => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .text_color(rgb(colors.text_secondary))
            .child("\u{25B8} MAIN"),
        GroupHeaderStyle::ChevronPrefix => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .text_color(rgb(colors.text_secondary))
            .child("\u{203A} MAIN"),
        GroupHeaderStyle::Dimmed => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .opacity(0.5)
            .text_color(rgb(colors.text_muted))
            .child("MAIN"),

        // Decorative styles
        GroupHeaderStyle::Bracketed => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .text_color(rgb(colors.text_secondary))
            .child("[MAIN]"),
        GroupHeaderStyle::Quoted => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .text_xs()
            .text_color(rgb(colors.text_secondary))
            .child("\"MAIN\""),
        GroupHeaderStyle::Tagged => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .items_center()
            .px(px(8.0))
            .child(
                div()
                    .px(px(6.0))
                    .py(px(1.0))
                    .bg(rgba((colors.accent << 8) | 0x40))
                    .rounded(px(2.0))
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.accent))
                    .child("MAIN"),
            ),
        GroupHeaderStyle::Numbered => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(colors.accent))
                    .child("01."),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            ),
        GroupHeaderStyle::IconPrefix => div()
            .w(px(140.0))
            .h(px(28.0))
            .rounded(px(visual.radius_sm))
            .bg(rgba((colors.background_secondary << 8) | 0x60))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .bg(rgb(colors.accent))
                    .rounded(px(1.0)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors.text_secondary))
                    .child("MAIN"),
            ),
    };

    item_div
        // Preview element
        .child(preview)
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
                        .text_color(rgb(colors.text_primary))
                        .child(name_owned),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors.text_muted))
                        .child(desc_owned),
                ),
        )
        .into_any_element()
}
