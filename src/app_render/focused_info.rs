/// Theming context for focused-info rendering.
/// Pre-extracted from theme and design tokens to avoid borrow conflicts.
#[derive(Clone)]
struct FocusedInfoStyle {
    text_primary: u32,
    text_muted: u32,
    text_secondary: u32,
    ui_border: u32,
    accent_color: u32,
    accent_selected: u32,
    section_label_font_size: f32,
    body_text_line_height: f32,
    spacing: designs::DesignSpacing,
    typography: designs::DesignTypography,
    visual: designs::DesignVisual,
    badge_bg_rgba: u32,
    badge_text_hex: u32,
    badge_border_rgba: u32,
    accent_badge_bg_rgba: u32,
    accent_badge_text_hex: u32,
    accent_badge_border_rgba: u32,
    divider_rgba: u32,
}

impl FocusedInfoStyle {
    fn from_theme_and_design(theme: &crate::theme::Theme, design: designs::DesignVariant) -> Self {
        let tokens = get_tokens(design);
        let colors = tokens.colors();
        let spacing = tokens.spacing();
        let typography = tokens.typography();
        let visual = tokens.visual();
        let is_light_mode = !theme.is_dark_mode();
        let chrome = crate::theme::AppChromeColors::from_theme(theme);

        Self {
            text_primary: if is_light_mode {
                theme.colors.text.primary
            } else {
                colors.text_primary
            },
            text_muted: if is_light_mode {
                theme.colors.text.muted
            } else {
                colors.text_muted
            },
            text_secondary: if is_light_mode {
                theme.colors.text.secondary
            } else {
                colors.text_secondary
            },
            ui_border: colors.border,
            accent_color: colors.accent,
            accent_selected: theme.colors.accent.selected,
            section_label_font_size: preview_panel_typography_section_label_size(typography),
            body_text_line_height: preview_panel_typography_body_line_height(typography),
            spacing,
            typography,
            visual,
            badge_bg_rgba: chrome.badge_bg_rgba,
            badge_text_hex: chrome.badge_text_hex,
            badge_border_rgba: chrome.badge_border_rgba,
            accent_badge_bg_rgba: chrome.accent_badge_bg_rgba,
            accent_badge_text_hex: chrome.accent_badge_text_hex,
            accent_badge_border_rgba: chrome.accent_badge_border_rgba,
            divider_rgba: chrome.divider_rgba,
        }
    }
}

/// Render metadata content for an inline calculator result.
/// Returns a container div with expression, badges, result, and word representation.
fn render_focused_info_for_calculator(
    calculator: &crate::calculator::CalculatorInlineResult,
    style: &FocusedInfoStyle,
) -> Div {
    let s = &style.spacing;
    let t = &style.typography;

    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_xs()
                .font_family(t.font_family_mono)
                .text_color(rgba((style.text_muted << 8) | 0x99))
                .pb(px(s.padding_xs))
                .child("calculator: inline"),
        )
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(style.text_primary))
                .pb(px(s.padding_sm))
                .child(calculator.normalized_expr.clone()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(s.gap_sm))
                .pb(px(s.padding_md))
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .bg(rgba(style.badge_bg_rgba))
                        .border_1()
                        .border_color(rgba(style.badge_border_rgba))
                        .text_xs()
                        .text_color(rgb(style.badge_text_hex))
                        .child(calculator.operation_name.clone()),
                )
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .bg(rgba(style.accent_badge_bg_rgba))
                        .border_1()
                        .border_color(rgba(style.accent_badge_border_rgba))
                        .text_xs()
                        .text_color(rgb(style.accent_badge_text_hex))
                        .child("Enter copies result"),
                ),
        )
        .child(
            div()
                .w_full()
                .h(px(style.visual.border_thin))
                .bg(rgba(style.divider_rgba))
                .my(px(s.padding_sm)),
        )
        .child(
            div()
                .text_size(px(style.section_label_font_size))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgba((style.text_muted << 8) | 0xCC))
                .pb(px(s.padding_sm))
                .child("RESULT"),
        )
        .child(
            div()
                .text_size(px(t.font_size_xl))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(style.accent_selected))
                .pb(px(s.padding_xs))
                .child(calculator.formatted.clone()),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(style.text_secondary))
                .child(calculator.words.clone()),
        )
}

/// Render metadata content for a search result.
/// Returns a container div with type-specific info (name, badges, shortcuts, description, type indicator).
/// Does NOT include code preview — that is handled separately by the preview panel.
fn render_focused_info_for_result(
    result: &scripts::SearchResult,
    shortcut_display: &Option<String>,
    match_indices: &scripts::MatchIndices,
    style: &FocusedInfoStyle,
) -> Div {
    let s = &style.spacing;
    let t = &style.typography;

    let mut content = div().flex().flex_col();

    match result {
        scripts::SearchResult::Script(script_match) => {
            let script = &script_match.script;

            // Source indicator with match highlighting
            let filename = &script_match.filename;
            let filename_indices = &match_indices.filename_indices;
            let path_segments = render_path_with_highlights(filename, filename, filename_indices);

            let mut path_div = div()
                .flex()
                .flex_row()
                .text_xs()
                .font_family(t.font_family_mono)
                .pb(px(s.padding_xs))
                .overflow_x_hidden()
                .child(
                    div()
                        .text_color(rgba((style.text_muted << 8) | 0x99))
                        .child("script: "),
                );

            for (text, is_highlighted) in path_segments {
                let color = if is_highlighted {
                    rgb(style.accent_color)
                } else {
                    rgba((style.text_muted << 8) | 0x99)
                };
                path_div = path_div.child(div().text_color(color).child(text));
            }

            content = content.child(path_div);

            // Name header
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_md))
                    .child(format!("{}.{}", script.name, script.extension)),
            );

            // Metadata badges: kit name, language, alias, author, tags
            {
                let mut script_badges = div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(s.gap_sm))
                    .pb(px(s.padding_sm));
                let mut show_script_badges = false;
                if let Some(ref kit) = script.kit_name {
                    show_script_badges = true;
                    script_badges = script_badges.child(focused_info_badge(
                        &format!("kit: {}", kit),
                        style.badge_bg_rgba,
                        style.badge_border_rgba,
                        style.badge_text_hex,
                    ));
                }
                // Extension badge
                {
                    let ext_display = match script.extension.as_str() {
                        "ts" => "TypeScript",
                        "js" => "JavaScript",
                        "mjs" => "JavaScript",
                        "sh" => "Shell",
                        "bash" => "Bash",
                        "zsh" => "Zsh",
                        "py" => "Python",
                        "rb" => "Ruby",
                        _ => "",
                    };
                    if !ext_display.is_empty() {
                        show_script_badges = true;
                        script_badges = script_badges.child(focused_info_badge(
                            ext_display,
                            style.badge_bg_rgba,
                            style.badge_border_rgba,
                            style.badge_text_hex,
                        ));
                    }
                }
                if let Some(ref alias) = script.alias {
                    show_script_badges = true;
                    script_badges = script_badges.child(focused_info_badge(
                        &format!("alias: {}", alias),
                        style.accent_badge_bg_rgba,
                        style.accent_badge_border_rgba,
                        style.accent_badge_text_hex,
                    ));
                }
                // Author badge from typed metadata
                if let Some(ref typed_meta) = script.typed_metadata {
                    if let Some(ref author) = typed_meta.author {
                        show_script_badges = true;
                        script_badges = script_badges.child(focused_info_badge(
                            &format!("by {}", author),
                            style.badge_bg_rgba,
                            style.badge_border_rgba,
                            style.badge_text_hex,
                        ));
                    }
                    for tag in &typed_meta.tags {
                        show_script_badges = true;
                        script_badges = script_badges.child(focused_info_badge(
                            tag,
                            style.badge_bg_rgba,
                            style.badge_border_rgba,
                            style.badge_text_hex,
                        ));
                    }
                }
                if show_script_badges {
                    content = content.child(script_badges);
                }
            }

            // Keyboard shortcut
            let effective_shortcut = script.shortcut.clone().or_else(|| shortcut_display.clone());
            if let Some(shortcut_str) = effective_shortcut {
                content = content.child(focused_info_shortcut_section(&shortcut_str, style));
            }

            // Description
            if let Some(desc) = &script.description {
                content = content.child(
                    div()
                        .text_sm()
                        .line_height(px(style.body_text_line_height))
                        .text_color(rgb(style.text_secondary))
                        .pb(px(s.padding_lg))
                        .child(desc.clone()),
                );
            }
        }

        scripts::SearchResult::Scriptlet(scriptlet_match) => {
            let scriptlet = &scriptlet_match.scriptlet;

            // Source indicator with match highlighting
            if let Some(ref display_file_path) = scriptlet_match.display_file_path {
                let filename_indices = &match_indices.filename_indices;
                let path_segments = render_path_with_highlights(
                    display_file_path,
                    display_file_path,
                    filename_indices,
                );

                let mut path_div = div()
                    .flex()
                    .flex_row()
                    .text_xs()
                    .font_family(t.font_family_mono)
                    .pb(px(s.padding_xs))
                    .overflow_x_hidden()
                    .child(
                        div()
                            .text_color(rgba((style.text_muted << 8) | 0x99))
                            .child("scriptlet: "),
                    );

                for (text, is_highlighted) in path_segments {
                    let color = if is_highlighted {
                        rgb(style.accent_color)
                    } else {
                        rgba((style.text_muted << 8) | 0x99)
                    };
                    path_div = path_div.child(div().text_color(color).child(text));
                }

                content = content.child(path_div);
            }

            // Name header
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_md))
                    .child(scriptlet.name.clone()),
            );

            // Metadata badges: tool type, group, alias, keyword
            {
                let mut slet_badges = div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(s.gap_sm))
                    .pb(px(s.padding_sm));
                slet_badges = slet_badges.child(focused_info_badge(
                    scriptlet.tool_display_name(),
                    style.badge_bg_rgba,
                    style.badge_border_rgba,
                    style.badge_text_hex,
                ));
                if let Some(ref group) = scriptlet.group {
                    if !group.is_empty() {
                        slet_badges = slet_badges.child(focused_info_badge(
                            group,
                            style.badge_bg_rgba,
                            style.badge_border_rgba,
                            style.badge_text_hex,
                        ));
                    }
                }
                if let Some(ref alias) = scriptlet.alias {
                    slet_badges = slet_badges.child(focused_info_badge(
                        &format!("alias: {}", alias),
                        style.accent_badge_bg_rgba,
                        style.accent_badge_border_rgba,
                        style.accent_badge_text_hex,
                    ));
                }
                if let Some(ref keyword) = scriptlet.keyword {
                    slet_badges = slet_badges.child(focused_info_badge(
                        &format!("keyword: {}", keyword),
                        style.accent_badge_bg_rgba,
                        style.accent_badge_border_rgba,
                        style.accent_badge_text_hex,
                    ));
                }
                content = content.child(slet_badges);
            }

            // Description
            if let Some(desc) = &scriptlet.description {
                content = content.child(
                    div()
                        .text_sm()
                        .line_height(px(style.body_text_line_height))
                        .text_color(rgb(style.text_secondary))
                        .pb(px(s.padding_lg))
                        .child(desc.clone()),
                );
            }

            // Shortcut
            let effective_shortcut = scriptlet
                .shortcut
                .clone()
                .or_else(|| shortcut_display.clone());
            if let Some(shortcut) = effective_shortcut {
                content = content.child(focused_info_shortcut_section(&shortcut, style));
            }
        }

        scripts::SearchResult::BuiltIn(builtin_match) => {
            let builtin = &builtin_match.entry;

            // Name header
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_md))
                    .child(builtin.name.clone()),
            );

            // Keyboard shortcut
            if let Some(ref shortcut_str) = shortcut_display {
                content = content.child(focused_info_shortcut_section(shortcut_str, style));
            }

            // Description
            content = content.child(
                div()
                    .text_sm()
                    .line_height(px(style.body_text_line_height))
                    .text_color(rgb(style.text_secondary))
                    .pb(px(s.padding_lg))
                    .child(builtin.description.clone()),
            );

            // Keywords and feature type as subtle inline tags
            let mut metadata_tags = preview_keyword_tags(&builtin.keywords);
            let feature_tag = builtin_feature_annotation(&builtin.feature).to_lowercase();
            if !metadata_tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case(&feature_tag))
            {
                metadata_tags.push(feature_tag);
            }

            if !metadata_tags.is_empty() {
                let mut tags_row = div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(s.gap_sm))
                    .pb(px(s.padding_md));

                for tag in metadata_tags {
                    tags_row = tags_row.child(
                        div()
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(px(999.0))
                            .bg(rgba(style.badge_bg_rgba))
                            .border_1()
                            .border_color(rgba(style.badge_border_rgba))
                            .text_xs()
                            .text_color(rgb(style.badge_text_hex))
                            .child(tag),
                    );
                }

                content = content.child(tags_row);
            }
        }

        scripts::SearchResult::App(app_match) => {
            let app = &app_match.app;

            // Name header
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_md))
                    .child(app.name.clone()),
            );

            // Keyboard shortcut
            if let Some(ref shortcut_str) = shortcut_display {
                content = content.child(focused_info_shortcut_section(shortcut_str, style));
            }

            // Path
            content = content.child(focused_info_labeled_section(
                "PATH",
                &app.path.to_string_lossy(),
                style,
            ));

            // Bundle ID
            if let Some(bundle_id) = &app.bundle_id {
                content =
                    content.child(focused_info_labeled_section("BUNDLE ID", bundle_id, style));
            }

            // Divider + Type indicator
            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Application", style));
        }

        scripts::SearchResult::Window(window_match) => {
            let window = &window_match.window;

            // Title header
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(window.title.clone()),
            );

            // Application
            content = content.child(focused_info_labeled_section(
                "APPLICATION",
                &window.app,
                style,
            ));

            // Position & Size
            content = content.child(focused_info_labeled_section(
                "POSITION & SIZE",
                &format!(
                    "{}×{} at ({}, {})",
                    window.bounds.width, window.bounds.height, window.bounds.x, window.bounds.y
                ),
                style,
            ));

            // Divider + Type indicator
            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Window", style));
        }

        scripts::SearchResult::File(file_match) => {
            let file = &file_match.file;
            let path = std::path::Path::new(&file.path);
            let parent = path
                .parent()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();

            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(file.name.clone()),
            );

            if !parent.is_empty() {
                content = content.child(focused_info_labeled_section("LOCATION", &parent, style));
            }

            content = content.child(focused_info_labeled_section("PATH", &file.path, style));

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("File", style));
        }

        scripts::SearchResult::Note(note_match) => {
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(note_match.title.clone()),
            );

            content = content.child(focused_info_labeled_section(
                "UPDATED",
                &crate::formatting::format_absolute_datetime(note_match.hit.updated_at),
                style,
            ));
            content = content.child(focused_info_labeled_section(
                "LENGTH",
                &format!("{} chars", note_match.hit.char_count),
                style,
            ));

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Note", style));
        }

        scripts::SearchResult::BrainHit(brain_match) => {
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(brain_match.hit.title.clone()),
            );

            if !brain_match.hit.excerpt.is_empty() {
                content = content.child(focused_info_labeled_section(
                    "MEMORY",
                    &brain_match.hit.excerpt,
                    style,
                ));
            }
            content = content.child(focused_info_labeled_section(
                "SOURCE",
                brain_match.hit.source_label,
                style,
            ));

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("From Your Brain", style));
        }

        scripts::SearchResult::BrainInboxItem(inbox_match) => {
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(inbox_match.item.title.clone()),
            );

            if !inbox_match.item.detail.is_empty() {
                content = content.child(focused_info_labeled_section(
                    "INBOX",
                    &inbox_match.item.detail,
                    style,
                ));
            }
            content = content.child(focused_info_labeled_section(
                "KIND",
                inbox_match.item.kind.label(),
                style,
            ));
            content = content.child(focused_info_labeled_section(
                "SOURCE",
                &inbox_match.item.source,
                style,
            ));

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Brain Inbox", style));
        }

        scripts::SearchResult::Todo(todo_match) => {
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(todo_match.hit.title.clone()),
            );

            if !todo_match.hit.subtitle.is_empty() {
                content = content.child(focused_info_labeled_section(
                    "DETAILS",
                    &todo_match.hit.subtitle,
                    style,
                ));
            }

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Todo", style));
        }

        scripts::SearchResult::AgentChatHistory(agent_chat_history_match) => {
            let entry = &agent_chat_history_match.entry;

            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(entry.title_display().to_string()),
            );

            content = content.child(focused_info_labeled_section(
                "PREVIEW",
                entry.preview_display(),
                style,
            ));
            content = content.child(focused_info_labeled_section(
                "MESSAGES",
                &entry.message_count.to_string(),
                style,
            ));

            content =
                content
                    .child(focused_info_divider(style))
                    .child(focused_info_type_indicator(
                        "Agent Chat Conversation",
                        style,
                    ));
        }

        scripts::SearchResult::AiVault(ai_vault_match) => {
            let hit = &ai_vault_match.hit;

            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(hit.safe_title.clone()),
            );

            content = content.child(focused_info_labeled_section(
                "PROVIDER",
                &hit.provider_display_name,
                style,
            ));
            if let Some(workspace) = hit.workspace_path.as_deref() {
                content =
                    content.child(focused_info_labeled_section("WORKSPACE", workspace, style));
            }
            if let Some(modified) = hit.modified_at.as_deref() {
                content = content.child(focused_info_labeled_section("MODIFIED", modified, style));
            }

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("AI Vault", style));
        }

        scripts::SearchResult::ClipboardHistory(clipboard_match) => {
            let entry = &clipboard_match.entry;

            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(clipboard_match.title.clone()),
            );

            content = content.child(focused_info_labeled_section(
                "PREVIEW",
                &entry.display_preview(),
                style,
            ));
            content = content.child(focused_info_labeled_section(
                "TYPE",
                entry.content_type.as_str(),
                style,
            ));

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Clipboard", style));
        }

        scripts::SearchResult::DictationHistory(dictation_match) => {
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(dictation_match.preview.clone()),
            );

            content = content.child(focused_info_labeled_section(
                "TARGET",
                &dictation_match.target,
                style,
            ));
            content = content.child(focused_info_labeled_section(
                "DURATION",
                &crate::dictation::format_history_duration_ms(dictation_match.audio_duration_ms),
                style,
            ));
            content = content.child(focused_info_labeled_section(
                "TIME",
                &crate::dictation::format_history_timestamp(&dictation_match.timestamp),
                style,
            ));

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Dictation", style));
        }

        scripts::SearchResult::BrowserTab(browser_match) => {
            let hit = &browser_match.hit;

            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(hit.title.clone()),
            );

            content = content.child(focused_info_labeled_section("DOMAIN", &hit.domain, style));
            content = content.child(focused_info_labeled_section("URL", &hit.url, style));
            content = content.child(focused_info_labeled_section(
                "SOURCE",
                &hit.provider_label,
                style,
            ));

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Browser Tab", style));
        }

        scripts::SearchResult::BrowserHistory(browser_match) => {
            let hit = &browser_match.hit;

            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(hit.title.clone()),
            );

            content = content.child(focused_info_labeled_section("DOMAIN", &hit.domain, style));
            content = content.child(focused_info_labeled_section("URL", &hit.url, style));
            content = content.child(focused_info_labeled_section(
                "SOURCE",
                &format!("{} / {}", hit.provider_label, hit.profile_label),
                style,
            ));

            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Browser History", style));
        }

        scripts::SearchResult::Agent(agent_match) => {
            let agent = &agent_match.agent;

            // Source indicator
            let filename = agent
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "agent".to_string());

            let mut path_div = div()
                .flex()
                .flex_row()
                .text_xs()
                .font_family(t.font_family_mono)
                .pb(px(s.padding_xs))
                .overflow_x_hidden()
                .child(
                    div()
                        .text_color(rgba((style.text_muted << 8) | 0x99))
                        .child("agent: "),
                );

            path_div = path_div.child(
                div()
                    .text_color(rgba((style.text_muted << 8) | 0x99))
                    .child(filename),
            );

            content = content.child(path_div);

            // Name header
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(agent.name.clone()),
            );

            // Description
            if let Some(desc) = &agent.description {
                content = content.child(
                    div()
                        .text_sm()
                        .text_color(rgb(style.text_secondary))
                        .pb(px(s.padding_md))
                        .child(desc.clone()),
                );
            }

            // Backend
            content = content.child(focused_info_labeled_section(
                "BACKEND",
                &format!("{:?}", agent.backend),
                style,
            ));

            // Kit
            if let Some(kit) = &agent.kit {
                content = content.child(focused_info_labeled_section("KIT", kit, style));
            }

            // Divider + Type indicator
            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Agent", style));
        }

        scripts::SearchResult::Fallback(fallback_match) => {
            let fallback_name = fallback_match.display_name();
            let fallback_label = fallback_match.display_label();
            let fallback_description = fallback_match.display_description();

            // Source indicator
            let mut path_div = div()
                .flex()
                .flex_row()
                .text_xs()
                .font_family(t.font_family_mono)
                .pb(px(s.padding_xs))
                .overflow_x_hidden()
                .child(
                    div()
                        .text_color(rgba((style.text_muted << 8) | 0x99))
                        .child("fallback: "),
                );

            path_div = path_div.child(
                div()
                    .text_color(rgba((style.text_muted << 8) | 0x99))
                    .child(fallback_name),
            );

            content = content.child(path_div);

            // Name header
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(fallback_label),
            );

            // Description
            content = content.child(
                div()
                    .text_sm()
                    .text_color(rgb(style.text_secondary))
                    .pb(px(s.padding_md))
                    .child(fallback_description),
            );

            // Divider + Type indicator
            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Fallback", style));
        }

        scripts::SearchResult::Skill(skill_match) => {
            let skill = &skill_match.skill;

            // Source indicator
            let mut path_div = div()
                .flex()
                .flex_row()
                .text_xs()
                .font_family(t.font_family_mono)
                .pb(px(s.padding_xs))
                .overflow_x_hidden()
                .child(
                    div()
                        .text_color(rgba((style.text_muted << 8) | 0x99))
                        .child("skill: "),
                );

            path_div = path_div.child(
                div()
                    .text_color(rgba((style.text_muted << 8) | 0x99))
                    .child(format!("{}/{}", skill.plugin_id, skill.skill_id)),
            );

            content = content.child(path_div);

            // Name header
            content = content.child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(style.text_primary))
                    .pb(px(s.padding_sm))
                    .child(skill.title.clone()),
            );

            // Description
            if !skill.description.is_empty() {
                content = content.child(
                    div()
                        .text_sm()
                        .text_color(rgb(style.text_secondary))
                        .pb(px(s.padding_md))
                        .child(skill.description.clone()),
                );
            }

            // Plugin badge
            content = content.child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(s.gap_sm))
                    .pb(px(s.padding_sm))
                    .child(focused_info_badge(
                        &format!("plugin: {}", skill.plugin_title),
                        style.badge_bg_rgba,
                        style.badge_border_rgba,
                        style.badge_text_hex,
                    )),
            );

            // Divider + Type indicator
            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Skill", style));
        }
        scripts::SearchResult::ScriptIssue(issue) => {
            let mut summary_row = div()
                .flex()
                .flex_row()
                .gap(px(s.gap_sm))
                .pb(px(s.padding_sm));
            summary_row = summary_row.child(focused_info_badge(
                &format!("{} failed", issue.failed_count),
                style.badge_bg_rgba,
                style.badge_border_rgba,
                style.badge_text_hex,
            ));
            if issue.fatal_count > 0 {
                summary_row = summary_row.child(focused_info_badge(
                    &format!("{} fatal", issue.fatal_count),
                    style.badge_bg_rgba,
                    style.badge_border_rgba,
                    style.badge_text_hex,
                ));
            }
            if issue.warning_count > 0 {
                summary_row = summary_row.child(focused_info_badge(
                    &format!("{} warning", issue.warning_count),
                    style.badge_bg_rgba,
                    style.badge_border_rgba,
                    style.badge_text_hex,
                ));
            }
            content = content
                .child(summary_row)
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator("Issues", style));
            let _ = match_indices;
            let _ = shortcut_display;
        }
        scripts::SearchResult::SpineProjection(_) => {
            // Spine projection rows have no focused info panel
            let _ = match_indices;
            let _ = shortcut_display;
        }
        scripts::SearchResult::Flow(flow_match) => {
            // Flow rows keep the focused panel minimal: engine + origin.
            // Detached conversation rows (no descriptor) fall back to the
            // row subtitle, which already carries surface · context · state.
            match flow_match.flow.as_ref() {
                Some(flow) => {
                    content =
                        content.child(focused_info_labeled_section("ENGINE", &flow.engine, style));
                    content = content.child(focused_info_labeled_section(
                        "ORIGIN",
                        flow.origin_label(),
                        style,
                    ));
                }
                None => {
                    content = content.child(focused_info_labeled_section(
                        "CONTEXT",
                        &flow_match.subtitle,
                        style,
                    ));
                }
            }
            let type_label = match flow_match.target {
                scripts::ConversationRowTarget::Conversation(_) => "Conversation",
                scripts::ConversationRowTarget::FlowIdentity { .. } => "Flow",
            };
            content = content
                .child(focused_info_divider(style))
                .child(focused_info_type_indicator(type_label, style));
            let _ = match_indices;
            let _ = shortcut_display;
        }
    }

    content
}

// --- Shared rendering helpers ---

/// Render a small badge element with the given text and colors.
fn focused_info_badge(text: &str, bg_rgba: u32, border_rgba: u32, text_hex: u32) -> Div {
    div()
        .px(px(6.))
        .py(px(2.))
        .rounded(px(4.))
        .bg(rgba(bg_rgba))
        .border_1()
        .border_color(rgba(border_rgba))
        .text_xs()
        .text_color(rgb(text_hex))
        .child(text.to_string())
}

/// Render a keyboard shortcut section with label and value.
fn focused_info_shortcut_section(shortcut_str: &str, style: &FocusedInfoStyle) -> Div {
    div()
        .flex()
        .flex_col()
        .pb(px(style.spacing.padding_lg))
        .child(
            div()
                .text_size(px(style.section_label_font_size))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgba((style.text_muted << 8) | 0xCC))
                .pb(px(style.spacing.padding_xs))
                .child("KEYBOARD SHORTCUT"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(style.spacing.gap_sm))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(style.accent_badge_text_hex))
                        .child(shortcut_str.to_string()),
                ),
        )
}

/// Render a labeled metadata section (label + value).
fn focused_info_labeled_section(label: &str, value: &str, style: &FocusedInfoStyle) -> Div {
    div()
        .flex()
        .flex_col()
        .pb(px(style.spacing.padding_lg))
        .child(
            div()
                .text_size(px(style.section_label_font_size))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgba((style.text_muted << 8) | 0xCC))
                .pb(px(style.spacing.padding_xs))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_sm()
                .line_height(px(style.body_text_line_height))
                .text_color(rgb(style.text_secondary))
                .child(value.to_string()),
        )
}

/// Render a horizontal divider line.
fn focused_info_divider(style: &FocusedInfoStyle) -> Div {
    div()
        .w_full()
        .h(px(style.visual.border_thin))
        .bg(rgba((style.ui_border << 8) | 0x60))
        .my(px(style.spacing.padding_sm))
}

/// Render a type indicator section (e.g., "Application", "Window", "Agent").
fn focused_info_type_indicator(type_name: &str, style: &FocusedInfoStyle) -> Div {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(style.section_label_font_size))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgba((style.text_muted << 8) | 0xCC))
                .pb(px(style.spacing.padding_xs))
                .child("TYPE"),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(style.text_secondary))
                .child(type_name.to_string()),
        )
}

// --- Existing ScriptListApp methods ---

impl ScriptListApp {
    fn get_focused_script_info(&mut self) -> Option<ScriptInfo> {
        // Get grouped results to map from selected_index to actual result (cached)
        let (grouped_items, flat_results) = self.get_grouped_results_cached();
        // Clone to avoid borrow issues
        let grouped_items = grouped_items.clone();
        let flat_results = flat_results.clone();

        // Get the minimum score threshold for suggested items
        let min_score = self.config.get_suggested().min_score;

        // Get the result index from the grouped item
        let result_idx = match grouped_items.get(self.selected_index) {
            Some(GroupedListItem::Item(idx)) => Some(*idx),
            _ => None,
        };

        tracing::trace!(
            event = "get_focused_script_info",
            selected_index = self.selected_index,
            grouped_items_len = grouped_items.len(),
            result_idx = ?result_idx,
            "get_focused_script_info",
        );

        if let Some(idx) = result_idx {
            if let Some(result) = flat_results.get(idx) {
                // Compute frecency path for each result type (same logic as app_impl.rs)
                let frecency_path: Option<String> = match result {
                    scripts::SearchResult::Script(m) => {
                        Some(m.script.path.to_string_lossy().to_string())
                    }
                    scripts::SearchResult::Scriptlet(m) => {
                        Some(format!("scriptlet:{}", m.scriptlet.name))
                    }
                    scripts::SearchResult::BuiltIn(m) => {
                        // Check if excluded from frecency tracking
                        let excluded = &self.config.get_suggested().excluded_commands;
                        if m.entry.should_exclude_from_frecency(excluded) {
                            None
                        } else {
                            Some(format!("builtin:{}", m.entry.id))
                        }
                    }
                    scripts::SearchResult::App(m) => Some(m.app.path.to_string_lossy().to_string()),
                    scripts::SearchResult::Window(m) => {
                        Some(format!("window:{}:{}", m.window.app, m.window.title))
                    }
                    scripts::SearchResult::File(m) => Some(format!("file/{}", m.file.path)),
                    scripts::SearchResult::Note(_) => None,
                    scripts::SearchResult::BrainHit(_) => None,
                    scripts::SearchResult::BrainInboxItem(_) => None,
                    scripts::SearchResult::Todo(_) => None,
                    scripts::SearchResult::AgentChatHistory(_) => None,
                    scripts::SearchResult::AiVault(_) => None,
                    scripts::SearchResult::ClipboardHistory(_) => None,
                    scripts::SearchResult::DictationHistory(_) => None,
                    scripts::SearchResult::BrowserTab(_) => None,
                    scripts::SearchResult::BrowserHistory(_) => None,
                    scripts::SearchResult::Agent(m) => {
                        Some(format!("agent:{}", m.agent.path.to_string_lossy()))
                    }
                    scripts::SearchResult::Skill(_) => None, // Skills don't track frecency
                    scripts::SearchResult::Flow(m) => {
                        m.flow.as_ref().map(|flow| format!("flow:{}", flow.id))
                    }
                    scripts::SearchResult::Fallback(_) => None, // Fallbacks don't track frecency
                    scripts::SearchResult::ScriptIssue(_) => None, // Diagnostic row doesn't track frecency
                    scripts::SearchResult::SpineProjection(_) => None, // Spine projections don't track frecency
                };

                // Check if this item is "suggested" (has frecency data above min_score)
                let is_suggested = frecency_path
                    .as_ref()
                    .map(|path| self.frecency_store.get_score(path) >= min_score)
                    .unwrap_or(false);

                // Pre-compute launcher command ID and override lookups before the match
                // to avoid partial-move borrow issues within match arms.
                let preference_identity = result.command_preference_identity();
                let alias_overrides = crate::aliases::get_cached_alias_overrides();
                let override_shortcut = preference_identity
                    .as_ref()
                    .and_then(|identity| {
                        identity.preferred_value(|id| self.config.get_command_shortcut(id))
                    })
                    .map(|hotkey| hotkey.to_display_string());
                let override_alias = preference_identity.as_ref().and_then(|identity| {
                    identity.preferred_value(|id| alias_overrides.get(id).cloned())
                });
                let command_descriptor = result.command_descriptor().ok();

                let focused = match result {
                    scripts::SearchResult::Script(m) => {
                        // Launcher-managed overrides take precedence over inline metadata.
                        let shortcut = override_shortcut.or_else(|| m.script.shortcut.clone());
                        let alias = override_alias.or_else(|| m.script.alias.clone());
                        Some(
                            ScriptInfo::with_shortcut_and_alias(
                                &m.script.name,
                                m.script.path.to_string_lossy(),
                                shortcut,
                                alias,
                            )
                            .with_frecency(is_suggested, frecency_path),
                        )
                    }
                    scripts::SearchResult::Scriptlet(m) => {
                        // Scriptlets use the markdown file path for edit/reveal actions
                        // Extract the path without anchor for file operations
                        let markdown_path = m
                            .scriptlet
                            .file_path
                            .as_ref()
                            .map(|p| p.split('#').next().unwrap_or(p).to_string())
                            .unwrap_or_else(|| format!("scriptlet:{}", &m.scriptlet.name));
                        // Launcher-managed overrides take precedence over inline metadata.
                        let shortcut = override_shortcut.or_else(|| m.scriptlet.shortcut.clone());
                        let alias = override_alias.or_else(|| m.scriptlet.alias.clone());
                        Some(
                            ScriptInfo::scriptlet(
                                &m.scriptlet.name,
                                markdown_path,
                                shortcut,
                                alias,
                            )
                            .with_frecency(is_suggested, frecency_path),
                        )
                    }
                    scripts::SearchResult::BuiltIn(m) => {
                        // Built-ins use their id as identifier
                        // is_script=false: no editable file, hide "Edit Script" etc.
                        Some(
                            ScriptInfo::with_all(
                                &m.entry.name,
                                format!("builtin:{}", &m.entry.id),
                                false,
                                m.entry.default_action_text(),
                                override_shortcut,
                                override_alias,
                            )
                            .with_frecency(is_suggested, frecency_path),
                        )
                    }
                    scripts::SearchResult::App(m) => {
                        // Apps use their path as identifier
                        // is_app=true: enables app-specific actions (Finder, Process, Copy)
                        Some(
                            ScriptInfo::app(
                                &m.app.name,
                                m.app.path.to_string_lossy().to_string(),
                                m.app.bundle_id.clone(),
                                override_shortcut,
                                override_alias,
                            )
                            .with_frecency(is_suggested, frecency_path),
                        )
                    }
                    scripts::SearchResult::Window(m) => {
                        // Windows use their id as identifier
                        // is_script=false: windows aren't editable scripts
                        Some(
                            ScriptInfo::with_action_verb(
                                &m.window.title,
                                format!("window:{}", m.window.id),
                                false,
                                "Switch to",
                            )
                            .with_frecency(is_suggested, frecency_path),
                        )
                    }
                    scripts::SearchResult::File(m) => {
                        let action = if m.file.file_type == crate::file_search::FileType::Directory
                        {
                            "Open Folder"
                        } else {
                            "Open"
                        };
                        Some(
                            ScriptInfo::with_action_verb(
                                &m.file.name,
                                m.file.path.clone(),
                                false,
                                action,
                            )
                            .with_frecency(is_suggested, frecency_path),
                        )
                    }
                    scripts::SearchResult::AgentChatHistory(m) => {
                        Some(ScriptInfo::with_action_verb(
                            m.entry.title_display(),
                            format!("agent_chat-history:{}", m.entry.session_id),
                            false,
                            "Resume",
                        ))
                    }
                    scripts::SearchResult::AiVault(m) => Some(ScriptInfo::with_action_verb(
                        &m.hit.safe_title,
                        m.hit.stable_key.clone(),
                        false,
                        "Resume",
                    )),
                    scripts::SearchResult::Note(m) => Some(ScriptInfo::with_action_verb(
                        &m.title,
                        format!("note:{}", m.hit.id.as_str()),
                        false,
                        "Open",
                    )),
                    scripts::SearchResult::BrainHit(m) => Some(ScriptInfo::with_action_verb(
                        &m.hit.title,
                        format!("brain:{}/{}", m.hit.source.as_str(), m.hit.source_id),
                        false,
                        "Open",
                    )),
                    scripts::SearchResult::BrainInboxItem(m) => Some(ScriptInfo::with_action_verb(
                        &m.item.title,
                        format!("brain-inbox:{}", m.item.id),
                        false,
                        "Open",
                    )),
                    scripts::SearchResult::Todo(m) => Some(ScriptInfo::with_action_verb(
                        &m.hit.title,
                        m.hit.stable_key.clone(),
                        false,
                        "Copy",
                    )),
                    scripts::SearchResult::ClipboardHistory(m) => {
                        Some(ScriptInfo::with_action_verb(
                            &m.title,
                            format!("clipboard-history:{}", m.entry.id),
                            false,
                            "Paste",
                        ))
                    }
                    scripts::SearchResult::DictationHistory(m) => {
                        Some(ScriptInfo::with_action_verb(
                            &m.preview,
                            format!("dictation-history:{}", m.id),
                            false,
                            "Paste",
                        ))
                    }
                    scripts::SearchResult::BrowserTab(m) => Some(ScriptInfo::with_action_verb(
                        &m.hit.title,
                        m.hit.url.clone(),
                        false,
                        "Switch to",
                    )),
                    scripts::SearchResult::BrowserHistory(m) => Some(ScriptInfo::with_action_verb(
                        &m.hit.title,
                        m.hit.url.clone(),
                        false,
                        "Open Page",
                    )),
                    scripts::SearchResult::Agent(m) => {
                        // Agents use their path as identifier
                        Some(
                            ScriptInfo::agent(
                                &m.agent.name,
                                m.agent.path.to_string_lossy().to_string(),
                                override_shortcut,
                                override_alias,
                            )
                            .with_frecency(is_suggested, frecency_path),
                        )
                    }
                    scripts::SearchResult::Skill(m) => {
                        // Skills use plugin_id/skill_id as identifier
                        // is_script=false: skills aren't editable scripts
                        Some(
                            ScriptInfo::with_action_verb(
                                &m.skill.title,
                                format!("skill:{}:{}", m.skill.plugin_id, m.skill.skill_id),
                                false,
                                "Open",
                            )
                            .with_frecency(is_suggested, frecency_path),
                        )
                    }
                    scripts::SearchResult::Fallback(m) => {
                        let fallback_name = m.display_name();
                        // Fallbacks use their name as identifier
                        // is_script depends on whether it's a built-in fallback or script-based
                        // Fallbacks don't track frecency, so is_suggested is always false
                        Some(ScriptInfo::with_action_verb(
                            fallback_name,
                            format!("fallback:{}", m.fallback.name()),
                            !m.fallback.is_builtin(),
                            "Run",
                        ))
                    }
                    scripts::SearchResult::ScriptIssue(m) => Some(ScriptInfo::with_action_verb(
                        &m.title,
                        "script-issues".to_string(),
                        false,
                        "Inspect",
                    )),
                    scripts::SearchResult::SpineProjection(_) => None, // Spine projections have no info panel
                    scripts::SearchResult::Flow(m) => Some(ScriptInfo::with_action_verb(
                        &m.display_name,
                        m.flow
                            .as_ref()
                            .map(|flow| flow.path.clone())
                            .unwrap_or_else(|| {
                                m.conversation_id()
                                    .map(|id| id.automation_id())
                                    .unwrap_or_default()
                            }),
                        false,
                        match m.target {
                            scripts::ConversationRowTarget::Conversation(_) => "Resume",
                            scripts::ConversationRowTarget::FlowIdentity { .. } => "Converse",
                        },
                    )),
                };

                focused.map(|info| match command_descriptor {
                    Some(descriptor) => info.with_command_descriptor(descriptor),
                    None => info,
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get the full scriptlet with actions for the currently focused item
    ///
    /// This re-parses the markdown file to get the scriptlet's H3 actions
    /// and shared actions from the companion .actions.md file.
    /// Returns None if the focused item is not a scriptlet.
    pub fn get_focused_scriptlet_with_actions(&mut self) -> Option<crate::scriptlets::Scriptlet> {
        let (grouped_items, flat_results) = self.get_grouped_results_cached();
        let grouped_items = grouped_items.clone();
        let flat_results = flat_results.clone();

        let result_idx = match grouped_items.get(self.selected_index) {
            Some(GroupedListItem::Item(idx)) => Some(*idx),
            _ => None,
        };

        if let Some(idx) = result_idx {
            if let Some(scripts::SearchResult::Scriptlet(m)) = flat_results.get(idx) {
                // Get the file path from the UI scriptlet type
                let file_path = m.scriptlet.file_path.clone()?;
                let scriptlet_command = m.scriptlet.command.clone()?;

                // Extract just the file path (before #anchor)
                let file_only = file_path.split('#').next().unwrap_or(&file_path);

                // Read and parse the markdown file to get full scriptlet with actions
                if let Ok(content) = std::fs::read_to_string(file_only) {
                    let parsed_scriptlets =
                        crate::scriptlets::parse_markdown_as_scriptlets(&content, Some(file_only));

                    // Find the matching scriptlet by command
                    return parsed_scriptlets
                        .into_iter()
                        .find(|s| s.command == scriptlet_command);
                }
            }
        }

        None
    }
}
