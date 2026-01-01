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

    // NOTE: render_toasts() removed - now using gpui-component's NotificationList
    // via the Root wrapper. Toasts are flushed via flush_pending_toasts() in render().
    // See toast_manager.rs for the queue and main.rs for the flush logic.

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
                            builtins::BuiltInFeature::AiChat => "AI Assistant".to_string(),
                            builtins::BuiltInFeature::Notes => "Notes & Scratchpad".to_string(),
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
                            if self.filter_input.is_empty()
                                && self.scripts.is_empty()
                                && self.scriptlets.is_empty()
                            {
                                "No scripts or snippets found"
                            } else if !self.filter_input.is_empty() {
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
                // Global shortcuts (Cmd+W, ESC closes window from ActionsDialog too)
                // ActionsDialog has no other key handling, so we just call the global handler
                let _ = this.handle_global_shortcut_with_options(event, true, cx);
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
