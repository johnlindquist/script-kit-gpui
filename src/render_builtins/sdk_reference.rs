#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SdkReferenceCatalogAction {
    CopyMarkdownCard,
}

impl SdkReferenceCatalogAction {
    fn copied_hud(self, entry_name: &str) -> String {
        match self {
            Self::CopyMarkdownCard => format!("Copied {entry_name} reference"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SdkReferenceEmptyState {
    NoFunctionsAvailable,
    NoFilteredMatches,
}

impl SdkReferenceEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.trim().is_empty() {
            Self::NoFunctionsAvailable
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoFunctionsAvailable => "No SDK functions available",
            Self::NoFilteredMatches => "No SDK functions match your filter",
        }
    }
}

impl ScriptListApp {
    fn sdk_reference_row_description(entry: &crate::mcp_resources::SdkFunctionRef) -> String {
        if entry.signature.is_empty() {
            entry.category.clone()
        } else {
            format!("{}  ·  {}", entry.category, entry.signature)
        }
    }

    /// Render the in-product SDK Reference browser (list + preview).
    ///
    /// Data source is [`crate::mcp_resources::sdk_reference_entries_for_ui`] —
    /// the same Rust objects that power the `kit://sdk-reference` MCP resource,
    /// so the in-app docs never drift from the agent-facing contract.
    fn render_sdk_reference_view(
        &mut self,
        filter: &str,
        selected_index: usize,
        entries: std::sync::Arc<[crate::mcp_resources::SdkFunctionRef]>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::expanded("sdk_reference", false),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();

        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let text_primary = rgb(chrome.text_primary_hex);
        let text_secondary = rgba(chrome.text_muted_rgba);
        let text_hint = rgba(chrome.text_hint_rgba);

        let visible_rows = crate::mcp_resources::sdk_reference_visible_rows(&entries, filter);
        let filtered_len = visible_rows.len();

        // Clamp selection if the filter shrank the visible set.
        let effective_selected = if filtered_len == 0 {
            0
        } else {
            selected_index.min(filtered_len.saturating_sub(1))
        };
        if effective_selected != selected_index {
            if let AppView::SdkReferenceView {
                selected_index: stored,
                ..
            } = &mut self.current_view
            {
                *stored = effective_selected;
            }
        }

        let preview_entry = visible_rows
            .get(effective_selected)
            .map(|row| row.entry.clone());

        // Key handler: ↑/↓ nav, Esc back, Cmd+C copy, Enter copy-and-back.
        let entries_for_keys = entries.clone();
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

                let (current_filter, current_selected) = if let AppView::SdkReferenceView {
                    filter,
                    selected_index,
                    ..
                } = &this.current_view
                {
                    (filter.clone(), *selected_index)
                } else {
                    return;
                };

                let visible = crate::mcp_resources::sdk_reference_visible_rows(
                    &entries_for_keys,
                    &current_filter,
                );
                let visible_len = visible.len();

                if crate::ui_foundation::is_key_up(key) {
                    if current_selected > 0 {
                        if let AppView::SdkReferenceView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected - 1;
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_down(key) {
                    if current_selected + 1 < visible_len {
                        if let AppView::SdkReferenceView { selected_index, .. } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected + 1;
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if has_cmd && key.eq_ignore_ascii_case("c") {
                    if let Some(entry) = visible.get(current_selected).map(|row| row.entry) {
                        let catalog_action = SdkReferenceCatalogAction::CopyMarkdownCard;
                        let markdown =
                            crate::mcp_resources::format_sdk_reference_entry_markdown(entry);
                        match crate::platform::copy_text(&markdown) {
                            Ok(receipt) => {
                                this.show_hud(
                                    receipt.feedback(catalog_action.copied_hud(&entry.name)),
                                    Some(2000),
                                    cx,
                                );
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "sdk_reference copy failed");
                                this.show_error_toast(format!("Failed to copy SDK reference: {e}"), cx);
                            }
                        }
                    }
                    cx.stop_propagation();
                } else if crate::ui_foundation::is_key_enter(key) {
                    if let Some(entry) = visible.get(current_selected).map(|row| row.entry) {
                        let catalog_action = SdkReferenceCatalogAction::CopyMarkdownCard;
                        let markdown =
                            crate::mcp_resources::format_sdk_reference_entry_markdown(entry);
                        match crate::platform::copy_text(&markdown) {
                            Ok(receipt) => {
                                this.show_hud(
                                    receipt.feedback(catalog_action.copied_hud(&entry.name)),
                                    Some(2000),
                                    cx,
                                );
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "sdk_reference enter-copy failed");
                                this.show_error_toast(format!("Failed to copy SDK reference: {e}"), cx);
                            }
                        }
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        // Build list rows.
        let list_colors = ListItemColors::from_theme(&self.theme);
        let list_element: AnyElement = if filtered_len == 0 {
            let state = SdkReferenceEmptyState::from_filter(filter);
            crate::components::render_simple_empty_state(
                "sdk-reference-empty",
                state.message(),
                "file-code",
                None,
                &self.theme,
                cx,
            )
        } else {
            let entries_for_list = entries.clone();
            let visible_for_list: Vec<usize> =
                visible_rows.iter().map(|row| row.source_index).collect();
            let selected = effective_selected;

            div()
                .id("sdk-reference-list")
                .w_full()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .overflow_y_scrollbar()
                .children(visible_for_list.into_iter().enumerate().filter_map(
                    move |(display_ix, original_idx)| {
                        let entry = entries_for_list.get(original_idx)?;
                        let is_selected = display_ix == selected;
                        let is_unsupported =
                            entry.support == crate::mcp_resources::SdkSupport::Unsupported;

                        let description = Self::sdk_reference_row_description(entry);

                        let item = ListItem::new(entry.name.clone(), list_colors)
                            .description_opt(Some(description))
                            .selected(is_selected);

                        // Row wrapper: when the entry is unsupported, overlay a
                        // right-aligned "Unsupported in GPUI" pill on the item so
                        // the label is visible without changing filter ranking.
                        let row = div()
                            .id(gpui::ElementId::Integer(display_ix as u64))
                            .relative()
                            .w_full()
                            .child(item);

                        Some(if is_unsupported {
                            row.child(
                                div()
                                    .absolute()
                                    .top(px(design_spacing.padding_sm))
                                    .right(px(design_spacing.padding_md))
                                    .px(px(design_spacing.padding_sm))
                                    .py(px(2.))
                                    .text_size(px(design_typography.font_size_xs))
                                    .text_color(text_hint)
                                    .child("Unsupported in GPUI"),
                            )
                        } else {
                            row
                        })
                    },
                ))
                .into_any_element()
        };

        // Preview panel.
        let preview_panel: AnyElement = match &preview_entry {
            Some(entry) => {
                let is_unsupported = entry.support == crate::mcp_resources::SdkSupport::Unsupported;
                let unsupported_note = entry.unsupported_note.clone();

                let base = div()
                    .w_full()
                    .h_full()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .font_family(design_typography.font_family)
                    .flex()
                    .flex_col()
                    .gap(px(design_spacing.padding_sm));

                let base = if is_unsupported {
                    let banner_title = div()
                        .text_size(px(design_typography.font_size_sm))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(text_secondary)
                        .child("⚠ Unsupported in GPUI");
                    let banner_body = div()
                        .text_size(px(design_typography.font_size_xs))
                        .text_color(text_hint)
                        .child(
                            unsupported_note.clone().unwrap_or_else(|| {
                                "This API is defined in scripts/kit-sdk.ts, but the GPUI app does not currently handle it. It will log a warning and may no-op or throw at runtime.".to_string()
                            }),
                        );
                    let banner = div()
                        .w_full()
                        .px(px(design_spacing.padding_md))
                        .py(px(design_spacing.padding_sm))
                        .flex()
                        .flex_col()
                        .gap(px(design_spacing.padding_xs))
                        .child(banner_title)
                        .child(banner_body);
                    base.child(banner)
                } else {
                    base
                };

                base.child(
                    div()
                        .text_size(px(design_typography.font_size_xl))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(text_primary)
                        .child(entry.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(design_typography.font_size_sm))
                        .text_color(text_secondary)
                        .child(entry.signature.clone()),
                )
                .child(
                    div()
                        .text_size(px(design_typography.font_size_xs))
                        .text_color(text_hint)
                        .child(entry.category.clone()),
                )
                .child(
                    div()
                        .text_size(px(design_typography.font_size_md))
                        .text_color(text_primary)
                        .child(entry.description.clone()),
                )
                .into_any_element()
            }
            None => div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(text_hint)
                .font_family(design_typography.font_family)
                .child("Select an SDK function")
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
            "↵ Copy Markdown".into(),
            "⌘C Copy".into(),
            "Esc Back".into(),
        ];
        crate::components::emit_surface_prompt_hint_audit(
            "sdk_reference",
            &hints,
            "sdk_reference_copy_browser",
        );

        let gpui_footer = crate::components::render_simple_hint_strip(hints, None);
        let footer = self.main_window_footer_slot(gpui_footer);
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let count_label = format!(
            "{} function{}",
            entries.len(),
            if entries.len() == 1 { "" } else { "s" },
        );
        let main =
            self.render_builtin_split_main_content(list_pane.into_any_element(), preview_panel);

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(text_primary)
                .font_family(self.theme_font_family())
                .key_context("sdk_reference")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(
                    vec![self.render_builtin_main_input_count_label(count_label)],
                    cx,
                ),
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
