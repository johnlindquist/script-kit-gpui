fn preview_panel_typography_section_label_size(typography: designs::DesignTypography) -> f32 {
    typography.font_size_xs
}

fn preview_panel_typography_body_line_height(typography: designs::DesignTypography) -> f32 {
    typography.font_size_sm * typography.line_height_relaxed
}

fn preview_panel_code_surface_rgba(chrome: crate::theme::AppChromeColors) -> u32 {
    chrome.whisper_surface_rgba
}

fn preview_panel_code_radius_px() -> f32 {
    crate::ui::chrome::LIQUID_GLASS_COMPACT_RADIUS_PX
}

fn truncate_preview_line_for_display(line: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return "...".to_string();
    }

    if let Some((cutoff, _)) = line.char_indices().nth(max_chars) {
        let mut truncated = String::with_capacity(cutoff + 3);
        truncated.push_str(&line[..cutoff]);
        truncated.push_str("...");
        truncated
    } else {
        line.to_string()
    }
}

fn preview_scriptlet_cache_key(scriptlet: &scripts::Scriptlet, is_dark: bool) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    scriptlet.code.hash(&mut hasher);
    let code_hash = hasher.finish();

    let source_path = scriptlet.file_path.as_deref().unwrap_or("<inline>");
    let command = scriptlet.command.as_deref().unwrap_or("<none>");
    let theme = if is_dark { "dark" } else { "light" };

    format!(
        "{source_path}|{command}|{}|{}|{theme}|{code_hash:016x}",
        scriptlet.name, scriptlet.tool
    )
}

impl ScriptListApp {
    #[allow(dead_code)]
    fn read_script_preview(path: &std::path::Path, max_lines: usize) -> String {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let preview: String = content.lines().take(max_lines).join("\n");
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

    /// Get the command ID for a search result, used for config lookups (shortcuts, etc.)
    ///
    /// Source-owned identity agrees with newly written shortcut preferences;
    /// the render path still falls back to historical display-name bindings.
    fn get_command_id_for_result(result: &scripts::SearchResult) -> Option<String> {
        result.external_command_id()
    }

    /// Render the preview panel showing details of the selected script/scriptlet.
    /// Delegates metadata rendering to `render_focused_info_for_result` / `render_focused_info_for_calculator`,
    /// then appends code preview for Script/Scriptlet types.
    fn render_preview_panel(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let preview_start = std::time::Instant::now();
        let filter_for_log = self.filter_text.clone();

        // Only log when meaningful state changed (flag set by render_script_list)
        if self.main_menu_render_diagnostics.log_this_render {
            logging::log(
                "PREVIEW_PERF",
                &format!(
                    "[PREVIEW_START] filter='{}' selected_idx={}",
                    filter_for_log, self.selected_index
                ),
            );
        }
        let _ = preview_start; // Used in PREVIEW_PANEL_DONE below

        // Get grouped results to map from selected_index to actual result (cached)
        let selected_index = self.selected_index;
        self.get_grouped_results_cached();

        let selected_result_idx = self
            .main_menu_result_caches
            .flat_result_index_for_grouped_item(selected_index);
        let selected_result = selected_result_idx.and_then(|result_idx| {
            self.main_menu_result_caches
                .cloned_search_result_for_flat_index(result_idx)
        });
        let selected_calculator = selected_result_idx
            .and_then(|result_idx| self.inline_calculator_for_result_index(result_idx))
            .cloned();

        // Build shared focused-info style from current theme/design
        let style = FocusedInfoStyle::from_theme_and_design(&self.theme, self.current_design);

        // Get shortcut display string for the selected item (if any)
        let shortcut_display: Option<String> = if selected_calculator.is_some() {
            None
        } else {
            selected_result.as_ref().and_then(|result| {
                Self::get_command_id_for_result(result).and_then(|command_id| {
                    self.config
                        .get_command_shortcut(&command_id)
                        .or_else(|| {
                            result
                                .launcher_command_id()
                                .as_deref()
                                .and_then(|legacy_id| self.config.get_command_shortcut(legacy_id))
                        })
                        .map(|hotkey| hotkey.to_display_string())
                })
            })
        };

        // Use design tokens for panel container chrome
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let code_surface_rgba = preview_panel_code_surface_rgba(chrome);
        let code_radius = preview_panel_code_radius_px();
        let code_match_bg_rgba = chrome.accent_badge_bg_rgba;
        let code_match_radius = get_tokens(self.current_design).visual().radius_sm;

        // Preview panel container with left border separator. The main shell
        // supplies vibrancy; avoid painting an opaque panel over it.
        let mut panel = div()
            .w_full()
            .h_full()
            .border_l_1()
            .border_color(rgba(chrome.divider_rgba))
            .p(px(style.spacing.padding_lg))
            .flex()
            .flex_col()
            .overflow_y_scrollbar()
            .font_family(style.typography.font_family);

        // Handle calculator result via shared focused-info renderer
        if let Some(calculator) = selected_calculator {
            panel = panel.child(render_focused_info_for_calculator(&calculator, &style));
            return panel;
        }

        // Lazy match indices computation for visible preview
        let computed_filter = self.computed_filter_text.clone();

        match selected_result {
            Some(ref result) => {
                // Compute match indices for source path highlighting
                let match_start = std::time::Instant::now();
                let match_indices =
                    scripts::compute_match_indices_for_result(result, &computed_filter);
                let match_elapsed = match_start.elapsed();
                if match_elapsed.as_micros() > 500 {
                    logging::log(
                        "FILTER_PERF",
                        &format!(
                            "[PREVIEW] match_indices for '{}' took {:.2}ms (filter='{}')",
                            result.name(),
                            match_elapsed.as_secs_f64() * 1000.0,
                            filter_for_log
                        ),
                    );
                }

                // Render metadata via shared focused-info renderer
                panel = panel.child(render_focused_info_for_result(
                    result,
                    &shortcut_display,
                    &match_indices,
                    &style,
                ));

                // Append code preview for Script and Scriptlet types
                match result {
                    scripts::SearchResult::Script(script_match) => {
                        let script = &script_match.script;

                        // Divider before code preview
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(style.visual.border_thin))
                                .bg(rgba(chrome.divider_rgba))
                                .my(px(style.spacing.padding_sm)),
                        );

                        // Code preview header
                        panel = panel.child(
                            div()
                                .text_size(px(style.section_label_font_size))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgba((style.text_muted << 8) | 0xCC))
                                .pb(px(style.spacing.padding_sm))
                                .child("CODE PREVIEW"),
                        );

                        // Syntax-highlighted code from cache
                        let script_path = script.path.to_string_lossy().to_string();
                        let lang = script.extension.clone();
                        let is_dark = self.theme.is_dark_mode();
                        logging::log(
                            "FILTER_PERF",
                            &format!(
                                "[PREVIEW_CONTEXT] script='{}' content_match={} match_line={:?}",
                                script.name,
                                script_match.content_match.is_some(),
                                script_match.content_match.as_ref().map(|cm| cm.line_number)
                            ),
                        );
                        let cache_start = std::time::Instant::now();
                        let lines = self
                            .get_or_update_preview_cache_with_match(
                                &script_path,
                                &lang,
                                is_dark,
                                script_match.content_match.as_ref(),
                            )
                            .to_vec();
                        let cache_elapsed = cache_start.elapsed();
                        if cache_elapsed.as_micros() > 500 {
                            logging::log(
                                "FILTER_PERF",
                                &format!(
                                    "[PREVIEW] preview_cache for '{}' took {:.2}ms ({} lines, filter='{}')",
                                    script.name,
                                    cache_elapsed.as_secs_f64() * 1000.0,
                                    lines.len(),
                                    filter_for_log
                                ),
                            );
                        }

                        // Build code container
                        let mut code_container = div()
                            .w_full()
                            .min_w(px(280.))
                            .p(px(style.spacing.padding_md))
                            .rounded(px(code_radius))
                            .bg(rgba(code_surface_rgba))
                            .overflow_hidden()
                            .flex()
                            .flex_col();

                        for line in lines {
                            let mut line_div = div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .font_family(style.typography.font_family_mono)
                                .text_xs()
                                .min_h(px(style.spacing.padding_lg));

                            if line.spans.is_empty() {
                                line_div = line_div.child(" ");
                            } else {
                                for span in line.spans {
                                    let mut span_div =
                                        div().text_color(rgb(span.color)).child(span.text);
                                    if span.is_match_emphasis {
                                        span_div = span_div
                                            .bg(rgba(code_match_bg_rgba))
                                            .rounded(px(code_match_radius));
                                    }
                                    line_div = line_div.child(span_div);
                                }
                            }

                            code_container = code_container.child(line_div);
                        }

                        panel = panel.child(code_container);
                    }

                    scripts::SearchResult::Scriptlet(scriptlet_match) => {
                        let scriptlet = &scriptlet_match.scriptlet;

                        // Divider before content preview
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(style.visual.border_thin))
                                .bg(rgba(chrome.divider_rgba))
                                .my(px(style.spacing.padding_sm)),
                        );

                        // Content preview header
                        panel = panel.child(
                            div()
                                .text_size(px(style.section_label_font_size))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgba((style.text_muted << 8) | 0xCC))
                                .pb(px(style.spacing.padding_sm))
                                .child("CONTENT PREVIEW"),
                        );

                        // Syntax-highlighted scriptlet code with cache
                        let total_start = std::time::Instant::now();
                        let is_dark = self.theme.is_dark_mode();
                        let cache_key = preview_scriptlet_cache_key(scriptlet, is_dark);
                        let is_cache_hit =
                            self.scriptlet_preview_cache_key.as_ref() == Some(&cache_key);

                        let step2_start = std::time::Instant::now();
                        let lines = if is_cache_hit {
                            self.scriptlet_preview_cache_lines.clone()
                        } else {
                            const MAX_LINE_LENGTH: usize = 120;
                            let code_preview: String = scriptlet
                                .code
                                .lines()
                                .take(15)
                                .map(|line| {
                                    truncate_preview_line_for_display(line, MAX_LINE_LENGTH)
                                })
                                .join("\n");

                            let lang = match scriptlet.tool.as_str() {
                                "bash" | "zsh" | "sh" => "bash",
                                "node" | "bun" => "js",
                                _ => &scriptlet.tool,
                            };
                            let highlighted = highlight_code_lines(&code_preview, lang, is_dark);
                            self.scriptlet_preview_cache_key = Some(cache_key);
                            self.scriptlet_preview_cache_lines = highlighted.clone();
                            highlighted
                        };
                        let step2_ms = step2_start.elapsed().as_secs_f64() * 1000.0;

                        let step3_start = std::time::Instant::now();
                        let mut code_container = div()
                            .w_full()
                            .min_w(px(280.))
                            .p(px(style.spacing.padding_md))
                            .rounded(px(code_radius))
                            .bg(rgba(code_surface_rgba))
                            .overflow_hidden()
                            .flex()
                            .flex_col();
                        let step3_ms = step3_start.elapsed().as_secs_f64() * 1000.0;

                        let step4_start = std::time::Instant::now();
                        let line_count = lines.len();
                        let mut span_count = 0usize;

                        for line in lines {
                            let mut line_div = div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .font_family(style.typography.font_family_mono)
                                .text_xs()
                                .min_h(px(style.spacing.padding_lg));

                            if line.spans.is_empty() {
                                line_div = line_div.child(" ");
                            } else {
                                span_count += line.spans.len();
                                for span in line.spans {
                                    line_div = line_div
                                        .child(div().text_color(rgb(span.color)).child(span.text));
                                }
                            }

                            code_container = code_container.child(line_div);
                        }
                        let step4_ms = step4_start.elapsed().as_secs_f64() * 1000.0;

                        let step5_start = std::time::Instant::now();
                        panel = panel.child(code_container);
                        let step5_ms = step5_start.elapsed().as_secs_f64() * 1000.0;

                        let total_ms = total_start.elapsed().as_secs_f64() * 1000.0;

                        logging::log(
                            "CODE_PERF",
                            &format!(
                                "[SYNTAX] {} lines={} spans={} | cache={} get={:.2}ms container={:.2}ms lines={:.2}ms add={:.2}ms TOTAL={:.2}ms",
                                if is_cache_hit { "HIT" } else { "MISS" },
                                line_count,
                                span_count,
                                if is_cache_hit { "hit" } else { "miss" },
                                step2_ms,
                                step3_ms,
                                step4_ms,
                                step5_ms,
                                total_ms
                            ),
                        );
                    }

                    scripts::SearchResult::Skill(skill_match) => {
                        let skill = &skill_match.skill;

                        // Divider before SKILL.md preview
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(style.visual.border_thin))
                                .bg(rgba(chrome.divider_rgba))
                                .my(px(style.spacing.padding_sm)),
                        );

                        // SKILL.md header
                        panel = panel.child(
                            div()
                                .text_size(px(style.section_label_font_size))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgba((style.text_muted << 8) | 0xCC))
                                .pb(px(style.spacing.padding_sm))
                                .child("SKILL.md"),
                        );

                        let is_dark = self.theme.is_dark_mode();
                        let preview = {
                            const MAX_LINES: usize = 40;
                            const MAX_LINE_LENGTH: usize = 120;
                            match std::fs::read_to_string(&skill.path) {
                                Ok(content) => content
                                    .lines()
                                    .take(MAX_LINES)
                                    .map(|line| {
                                        truncate_preview_line_for_display(line, MAX_LINE_LENGTH)
                                    })
                                    .join("\n"),
                                Err(error) => {
                                    tracing::warn!(
                                        event = "skill_info_panel_preview_read_failed",
                                        path = %skill.path.display(),
                                        %error,
                                        "Failed to read SKILL.md for preview"
                                    );
                                    format!("Failed to read {}: {}", skill.path.display(), error)
                                }
                            }
                        };
                        let lines = highlight_code_lines(&preview, "md", is_dark);

                        let mut doc_container = div()
                            .w_full()
                            .min_w(px(280.))
                            .p(px(style.spacing.padding_md))
                            .rounded(px(code_radius))
                            .bg(rgba(code_surface_rgba))
                            .overflow_hidden()
                            .flex()
                            .flex_col();

                        for line in lines {
                            let mut line_div = div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .font_family(style.typography.font_family_mono)
                                .text_xs()
                                .min_h(px(style.spacing.padding_lg));

                            if line.spans.is_empty() {
                                line_div = line_div.child(" ");
                            } else {
                                for span in line.spans {
                                    line_div = line_div
                                        .child(div().text_color(rgb(span.color)).child(span.text));
                                }
                            }

                            doc_container = doc_container.child(line_div);
                        }

                        panel = panel.child(doc_container);
                    }

                    // No code preview for other result types
                    _ => {}
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
                        .text_color(rgb(style.text_muted))
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
}
