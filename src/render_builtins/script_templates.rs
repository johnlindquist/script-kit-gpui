#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptTemplateCatalogAction {
    CopyMarkdownCard,
}

impl ScriptTemplateCatalogAction {
    fn copied_hud(self, template_title: &str) -> String {
        match self {
            Self::CopyMarkdownCard => format!("Copied {template_title} template"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptTemplateCatalogEmptyState {
    NoTemplatesAvailable,
    NoFilteredMatches,
}

impl ScriptTemplateCatalogEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.trim().is_empty() {
            Self::NoTemplatesAvailable
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoTemplatesAvailable => "No starter templates available",
            Self::NoFilteredMatches => "No templates match your filter",
        }
    }
}

fn script_template_catalog_primary_hint() -> SharedString {
    "↵ Create Local Script".into()
}

impl ScriptListApp {
    fn script_template_catalog_row_description(
        template: &crate::mcp_resources::ScriptTemplateRef,
    ) -> String {
        if template.description.is_empty() {
            template.category.clone()
        } else {
            format!("{}  ·  {}", template.category, template.description)
        }
    }

    /// Render the in-product starter-template catalog (list + preview).
    ///
    /// Data source is [`crate::mcp_resources::script_template_entries_for_ui`] —
    /// the same Rust objects that power the `kit://script-templates` MCP
    /// resource, so a script created from the launcher and a script created
    /// from an MCP client are byte-identical.
    ///
    /// Enter transitions to the naming prompt via
    /// [`ScriptListApp::show_naming_dialog_for_script_template`]; after the
    /// user picks a filename, [`ScriptListApp::handle_naming_dialog_completion`]
    /// overwrites the freshly-created file with
    /// [`crate::mcp_resources::render_script_template_file`] before opening the
    /// editor. Cmd+C copies the selected template's markdown card so authors
    /// can paste it into a note or commit message.
    fn render_script_template_catalog_view(
        &mut self,
        filter: &str,
        selected_index: usize,
        templates: std::sync::Arc<[crate::mcp_resources::ScriptTemplateRef]>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("script_template_catalog", false),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();

        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let text_primary = rgb(chrome.text_primary_hex);
        let text_secondary = rgba(chrome.text_muted_rgba);
        let text_hint = rgba(chrome.text_hint_rgba);

        let visible_rows =
            crate::mcp_resources::script_template_catalog_visible_rows(&templates, filter);
        let filtered_len = visible_rows.len();

        let effective_selected = if filtered_len == 0 {
            0
        } else {
            selected_index.min(filtered_len.saturating_sub(1))
        };
        if effective_selected != selected_index {
            if let AppView::ScriptTemplateCatalogView {
                selected_index: stored,
                ..
            } = &mut self.current_view
            {
                *stored = effective_selected;
            }
        }

        let preview_template = visible_rows
            .get(effective_selected)
            .map(|row| row.template.clone());

        let templates_for_keys = templates.clone();
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);

                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;

                if crate::ui_foundation::is_key_escape(key) {
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }

                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                let (current_filter, current_selected) =
                    if let AppView::ScriptTemplateCatalogView {
                        filter,
                        selected_index,
                        ..
                    } = &this.current_view
                    {
                        (filter.clone(), *selected_index)
                    } else {
                        return;
                    };

                let visible = crate::mcp_resources::script_template_catalog_visible_rows(
                    &templates_for_keys,
                    &current_filter,
                );
                let visible_len = visible.len();

                if crate::ui_foundation::is_key_up(key) {
                    if current_selected > 0 {
                        if let AppView::ScriptTemplateCatalogView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected - 1;
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_down(key) {
                    if current_selected + 1 < visible_len {
                        if let AppView::ScriptTemplateCatalogView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected + 1;
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if has_cmd && key.eq_ignore_ascii_case("c") {
                    if let Some(template) = visible.get(current_selected).map(|row| row.template) {
                        let catalog_action = ScriptTemplateCatalogAction::CopyMarkdownCard;
                        let markdown =
                            crate::mcp_resources::format_script_template_markdown(template);
                        match crate::platform::copy_text_to_clipboard(&markdown) {
                            Ok(()) => {
                                this.show_hud(
                                    catalog_action.copied_hud(&template.title),
                                    Some(2000),
                                    cx,
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "script_template_catalog copy_text_to_clipboard failed"
                                );
                            }
                        }
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_enter(key) {
                    if let Some(template) = visible.get(current_selected).map(|row| row.template) {
                        this.show_naming_dialog_for_script_template(template.clone(), window, cx);
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        let list_colors = ListItemColors::from_theme(&self.theme);
        let list_element: AnyElement = if filtered_len == 0 {
            let state = ScriptTemplateCatalogEmptyState::from_filter(filter);
            crate::components::render_simple_empty_state(
                "script-templates-empty",
                state.message(),
                "file",
                None,
                &self.theme,
                cx,
            )
        } else {
            let templates_for_list = templates.clone();
            let visible_for_list: Vec<usize> =
                visible_rows.iter().map(|row| row.source_index).collect();
            let selected = effective_selected;

            div()
                .id("script-template-catalog-list")
                .w_full()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .overflow_y_scrollbar()
                .children(visible_for_list.into_iter().enumerate().map(
                    move |(display_ix, original_idx)| {
                        let template = templates_for_list
                            .get(original_idx)
                            .expect("visible index within bounds");
                        let is_selected = display_ix == selected;

                        let description = Self::script_template_catalog_row_description(template);

                        let item = ListItem::new(template.title.clone(), list_colors)
                            .description_opt(Some(description))
                            .selected(is_selected);

                        div()
                            .id(gpui::ElementId::Integer(display_ix as u64))
                            .child(item)
                    },
                ))
                .into_any_element()
        };

        let preview_panel: AnyElement = match &preview_template {
            Some(template) => div()
                .w_full()
                .h_full()
                .min_h(px(0.))
                .overflow_y_scrollbar()
                .px(px(design_spacing.padding_lg))
                .py(px(design_spacing.padding_md))
                .font_family(design_typography.font_family)
                .flex()
                .flex_col()
                .gap(px(design_spacing.padding_sm))
                .child(
                    div()
                        .text_size(px(design_typography.font_size_xl))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(text_primary)
                        .child(template.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(design_typography.font_size_xs))
                        .text_color(text_hint)
                        .child(template.category.clone()),
                )
                .child(
                    div()
                        .text_size(px(design_typography.font_size_md))
                        .text_color(text_primary)
                        .child(template.description.clone()),
                )
                .child(
                    div()
                        .text_size(px(design_typography.font_size_sm))
                        .text_color(text_secondary)
                        .child(template.body_template.clone()),
                )
                .into_any_element(),
            None => div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(text_hint)
                .font_family(design_typography.font_family)
                .child("Select a template")
                .into_any_element(),
        };

        let list_pane = div()
            .relative()
            .w_full()
            .h_full()
            .min_h(px(0.))
            .py(px(design_spacing.padding_xs))
            .child(list_element);

        let hints: Vec<SharedString> = vec![
            script_template_catalog_primary_hint(),
            "⌘C Copy".into(),
            "Esc Back".into(),
        ];
        crate::components::emit_surface_prompt_hint_audit(
            "script_template_catalog",
            &hints,
            "script_template_create_browser",
        );

        let gpui_footer = crate::components::render_simple_hint_strip(hints, None);
        let footer = self.main_window_footer_slot(gpui_footer);
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let count_label = format!(
            "{} template{}",
            templates.len(),
            if templates.len() == 1 { "" } else { "s" },
        );
        let main = self.render_builtin_split_main_content(
            list_pane.into_any_element(),
            preview_panel,
        );

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(text_primary)
                .font_family(self.theme_font_family())
                .key_context("script_template_catalog")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(vec![
                    self.render_builtin_main_input_count_label(count_label),
                ], cx),
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
}
