use super::*;

const NAMING_PROMPT_BODY_SELECTOR: &str = "naming-prompt-body-content";

impl Focusable for NamingPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for NamingPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = get_tokens(self.design_variant);
        let spacing = tokens.spacing();

        let text_primary = rgb(self.theme.colors.text.primary);
        let text_secondary = rgb(self.theme.colors.text.secondary);
        let text_muted = rgb(self.theme.colors.text.muted);
        let error_color = rgb(self.theme.colors.ui.error);

        let (title, destination_label, preview_label, preview_kind) = match self.target {
            NamingTarget::Script => (
                "Name your script",
                "Destination",
                "Filename",
                "TypeScript script",
            ),
            NamingTarget::Extension => (
                "Name your scriptlet bundle",
                "Destination",
                "Bundle Filename",
                "Markdown bundle",
            ),
        };

        let placeholder = self
            .placeholder
            .clone()
            .unwrap_or_else(|| match self.target {
                NamingTarget::Script => "My Cool Script".to_string(),
                NamingTarget::Extension => "My Cool Scriptlet Bundle".to_string(),
            });

        let validation_message = self
            .validation_error
            .as_ref()
            .map(NamingValidationError::message);

        let input_value = if self.friendly_name.is_empty() {
            SharedString::from(placeholder)
        } else {
            SharedString::from(self.friendly_name.clone())
        };

        let input_state = if self.validation_error.is_some() {
            crate::components::PromptFieldState::Error
        } else {
            crate::components::PromptFieldState::Active
        };
        let input_style = crate::components::prompt_field_style(
            &self.theme,
            input_state,
            self.friendly_name.is_empty(),
        );
        let preview_style = crate::components::prompt_field_style(
            &self.theme,
            crate::components::PromptFieldState::ReadOnly,
            false,
        );

        let preview_filename = self.filename_preview();
        let destination_text = SharedString::from(self.target_directory.display().to_string());
        let preview_path_text = SharedString::from(
            self.target_directory
                .join(&preview_filename)
                .display()
                .to_string(),
        );

        let content = div()
            .id(gpui::ElementId::Name("window:naming".into()))
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .text_color(text_primary)
            .child(
                div()
                    .w_full()
                    .flex_1()
                    .debug_selector(|| NAMING_PROMPT_BODY_SELECTOR.to_string())
                    .flex()
                    .flex_col()
                    .gap(px(spacing.gap_lg))
                    .child(crate::components::prompt_form_intro(
                        title,
                        "Choose a display name. Script Kit generates the filename automatically.",
                        text_primary,
                        text_muted,
                        spacing.gap_sm,
                    ))
                    .child(
                        crate::components::prompt_form_section(
                            "Friendly Name",
                            text_secondary,
                            spacing.gap_sm,
                            crate::components::prompt_text_field(input_value, input_style),
                        )
                        .when_some(validation_message, |d, message| {
                            d.child(crate::components::prompt_form_help(message, error_color))
                        }),
                    )
                    .when_some(
                        self.template.as_ref().map(|t| t.label.clone()),
                        |d, label| {
                            d.child(crate::components::prompt_form_section(
                                "Template",
                                text_secondary,
                                spacing.gap_sm,
                                crate::components::prompt_surface(
                                    preview_style.background,
                                    preview_style.border,
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .text_sm()
                                        .text_color(text_primary)
                                        .child(SharedString::from(label)),
                                ),
                            ))
                        },
                    )
                    .child(crate::components::prompt_form_section(
                        destination_label,
                        text_secondary,
                        spacing.gap_sm,
                        crate::components::prompt_surface(
                            preview_style.background,
                            preview_style.border,
                        )
                        .child(
                            crate::components::prompt_scroll_value_with_id(
                                "naming-destination-path",
                                destination_text,
                                text_muted,
                            ),
                        ),
                    ))
                    .child(crate::components::prompt_form_section(
                        preview_label,
                        text_secondary,
                        spacing.gap_sm,
                        crate::components::prompt_surface(
                            preview_style.background,
                            preview_style.border,
                        )
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap(px(spacing.gap_sm))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(text_primary)
                                        .child(SharedString::from(preview_filename)),
                                )
                                .child(crate::components::prompt_scroll_value_with_id(
                                    "naming-preview-path",
                                    preview_path_text,
                                    text_muted,
                                ))
                                .child(crate::components::prompt_form_help(
                                    format!("{preview_kind} ({})", self.extension_label()),
                                    text_secondary,
                                )),
                        ),
                    )),
            );

        let content = crate::components::render_inset_prompt_body(
            "naming-prompt-inset-body",
            content,
            crate::components::PromptBodyInsets::MainMenu(self.design_variant),
        );

        FocusablePrompt::new(content)
            .key_context("naming_prompt")
            .focus_handle(self.focus_handle.clone())
            .build(
                window,
                cx,
                |this, intercepted_key, _event, _window, _cx| match intercepted_key {
                    FocusablePromptInterceptedKey::Escape => {
                        this.submit_cancel();
                        true
                    }
                    FocusablePromptInterceptedKey::CmdW | FocusablePromptInterceptedKey::CmdK => {
                        false
                    }
                },
                |this, event, _window, cx| {
                    let key = event.keystroke.key.as_str();

                    if is_key_enter(key) {
                        this.submit(cx);
                        return;
                    }

                    if is_key_backspace(key) {
                        this.handle_backspace(cx);
                        return;
                    }

                    if let Some(ch) = printable_char(event.keystroke.key_char.as_deref()) {
                        this.handle_char(ch, cx);
                    }
                },
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = include_str!("render.rs");

    #[gpui::test]
    fn of58ab_layout_lock_naming_body_uses_main_menu_content_insets(cx: &mut gpui::TestAppContext) {
        let def = crate::designs::current_main_menu_theme().def();
        let spacing = get_tokens(DesignVariant::Default).spacing();
        let window = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(480.0), px(360.0)),
            )));
            cx.open_window(options, |_, cx| {
                let focus_handle = cx.focus_handle();
                cx.new(|_| {
                    NamingPrompt::new(
                        "layout-lock".to_string(),
                        NamingPromptConfig::new(NamingTarget::Script, PathBuf::from("/tmp"), "ts"),
                        focus_handle,
                        Arc::new(|_, _| {}),
                        Arc::new(theme::Theme::default()),
                    )
                })
            })
            .expect("naming layout test window should open")
        });

        cx.run_until_parked();

        window
            .update(cx, |_, window, _| {
                let body = window
                    .debug_bounds_entries()
                    .iter()
                    .find(|entry| entry.selector == NAMING_PROMPT_BODY_SELECTOR)
                    .expect("naming body should publish debug bounds");
                assert_eq!(body.bounds.origin.x, px(def.shell.content_inset_x));
                assert_eq!(body.bounds.origin.y, px(spacing.padding_sm));
            })
            .expect("naming layout test window should update");
    }

    #[test]
    fn naming_render_uses_shared_create_flow_helpers() {
        assert!(
            SOURCE.contains("prompt_form_intro("),
            "render.rs should use prompt_form_intro"
        );
        assert!(
            SOURCE.contains("prompt_form_section("),
            "render.rs should use prompt_form_section"
        );
        assert!(
            SOURCE.contains("prompt_form_help("),
            "render.rs should use prompt_form_help"
        );
    }

    #[test]
    fn naming_render_leaves_submit_hint_to_footer() {
        let production_code = SOURCE.split("#[cfg(test)]").next().unwrap_or(SOURCE);
        assert!(
            !production_code.contains("Press Enter to create"),
            "render.rs production code should not contain submit hint — the footer handles that"
        );
    }

    #[test]
    fn naming_render_no_longer_duplicates_footer_cancel_hint() {
        // Check that the non-test portion of the file doesn't contain the hint.
        // Split at the test module boundary to avoid matching our own assertion.
        let production_code = SOURCE.split("#[cfg(test)]").next().unwrap_or(SOURCE);
        let needle = ["Esc", " to ", "cancel"].concat();
        assert!(
            !production_code.contains(&needle),
            "render.rs production code should not contain cancel hint — the footer handles that"
        );
    }
}
