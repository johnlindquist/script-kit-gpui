use super::*;
use crate::components::{FocusablePrompt, FocusablePromptInterceptedKey};
use crate::ui_foundation::{is_key_backspace, is_key_enter};
use gpui::FontWeight;
use gpui_component::scroll::ScrollableElement;

const TEMPLATE_PROMPT_BODY_SELECTOR: &str = "template-prompt-body-content";

fn template_prompt_body_insets(
    design_variant: DesignVariant,
) -> crate::components::PromptBodyInsets {
    crate::components::PromptBodyInsets::MainMenu(design_variant)
}

impl Focusable for TemplatePrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TemplatePrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = get_tokens(self.design_variant);
        let spacing = tokens.spacing();

        let text = crate::components::prompt_text_palette(&self.theme);
        let text_primary = text.primary;
        let text_secondary = text.label;
        let text_muted = text.help;
        let error_color = rgb(self.theme.colors.ui.error);

        let description = if self.inputs.is_empty() {
            "This template has no editable placeholders. Review the preview and press Enter to submit."
                .to_string()
        } else {
            format!(
                "Fill {} field(s). The preview updates as you type.",
                self.inputs.len()
            )
        };

        let preview = self.preview_template();
        let preview_style = crate::components::prompt_field_style(
            &self.theme,
            crate::components::PromptFieldState::ReadOnly,
            false,
        );

        let mut content = div()
            .id(gpui::ElementId::Name("window:template".into()))
            .debug_selector(|| TEMPLATE_PROMPT_BODY_SELECTOR.to_string())
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .text_color(text_primary)
            .gap(px(spacing.gap_lg))
            .child(crate::components::prompt_form_intro(
                "Complete the template",
                description,
                text_primary,
                text_muted,
                spacing.gap_sm,
            ))
            .child(crate::components::prompt_form_section(
                "Preview",
                text_secondary,
                spacing.gap_sm,
                crate::components::prompt_text_field(preview, preview_style),
            ));

        if self.inputs.is_empty() {
            content = content.child(crate::components::prompt_form_help(
                "No {{placeholders}} found in template.",
                text_secondary,
            ));
        } else {
            let mut fields = div()
                .id("template-fields-scroll")
                .w_full()
                .flex()
                .flex_1()
                .min_h(px(0.))
                .flex_col()
                .gap(px(spacing.gap_lg))
                .overflow_y_scrollbar();
            let mut previous_group: Option<String> = None;

            for (idx, input) in self.inputs.iter().enumerate() {
                if !input.group.is_empty()
                    && previous_group.as_deref() != Some(input.group.as_str())
                {
                    previous_group = Some(input.group.clone());
                    fields = fields.child(
                        div()
                            .w_full()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text_muted)
                            .child(input.group.clone()),
                    );
                }

                let is_current = idx == self.current_input;
                let value = self.values.get(idx).cloned().unwrap_or_default();
                let label = if input.required {
                    format!("{} *", input.label)
                } else {
                    input.label.clone()
                };
                let display = if value.is_empty() {
                    SharedString::from(input.placeholder.clone())
                } else {
                    SharedString::from(value.clone())
                };
                let validation_message = self.validation_errors.get(idx).and_then(|m| m.as_ref());

                let field_state = if validation_message.is_some() {
                    crate::components::PromptFieldState::Error
                } else if is_current {
                    crate::components::PromptFieldState::Active
                } else {
                    crate::components::PromptFieldState::Default
                };
                let field_style = crate::components::prompt_field_style(
                    &self.theme,
                    field_state,
                    value.is_empty(),
                );

                let handle_select = cx.entity().downgrade();
                let field_section = crate::components::prompt_form_section(
                    label,
                    text_secondary,
                    spacing.gap_sm,
                    crate::components::prompt_text_field(display, field_style),
                )
                .when_some(validation_message, |d, message| {
                    d.child(crate::components::prompt_form_help(
                        message.clone(),
                        error_color,
                    ))
                })
                .id(SharedString::from(format!("template-field-{idx}")))
                .cursor_pointer()
                .on_click(move |_event, _window, cx| {
                    if let Some(entity) = handle_select.upgrade() {
                        entity.update(cx, |this, cx| {
                            if this.current_input != idx {
                                this.set_current_input(idx);
                                cx.notify();
                            }
                        });
                    }
                });

                fields = fields.child(field_section);
            }

            if self
                .inputs
                .iter()
                .any(|input| Self::is_name_field(&input.name))
            {
                fields = fields.child(crate::components::prompt_form_help(
                    "Naming tip: use lowercase letters, numbers, and hyphens.",
                    text_muted,
                ));
            }

            content = content.child(fields);
        }

        let content = crate::components::render_inset_prompt_body(
            "template-prompt-inset-body",
            content,
            template_prompt_body_insets(self.design_variant),
        );

        FocusablePrompt::new(content)
            .key_context("template_prompt")
            .focus_handle(self.focus_handle.clone())
            .build(
                window,
                cx,
                |this, intercepted_key, _event, _window, _cx| match intercepted_key {
                    FocusablePromptInterceptedKey::Escape => {
                        this.submit_cancel();
                        true
                    }
                    _ => false,
                },
                |this, event, _window, cx| {
                    let modifiers = &event.keystroke.modifiers;
                    if modifiers.platform || modifiers.control || modifiers.function {
                        return;
                    }

                    if is_key_backspace(&event.keystroke.key) && !modifiers.alt {
                        this.handle_backspace(cx);
                        cx.stop_propagation();
                    } else if let Some(text) = event.keystroke.key_char.as_deref() {
                        // Use committed text, including Option-produced Unicode, never key names.
                        if this.handle_text(text, cx) {
                            cx.stop_propagation();
                        }
                    }
                },
            )
            // Keymap actions run before raw key capture; consume traversal here
            // before the ancestor Root can move focus outside the template.
            .on_action(cx.listener(|this, _: &gpui_component::Tab, _window, cx| {
                this.next_input(cx);
                cx.stop_propagation();
            }))
            .on_action(
                cx.listener(|this, _: &gpui_component::TabPrev, _window, cx| {
                    this.prev_input(cx);
                    cx.stop_propagation();
                }),
            )
            .capture_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                    let modifiers = &event.keystroke.modifiers;
                    if modifiers.platform
                        || modifiers.control
                        || modifiers.alt
                        || modifiers.function
                    {
                        return;
                    }

                    let key = event.keystroke.key.as_str();
                    if is_key_enter(key) {
                        this.submit(cx);
                    } else {
                        return;
                    }
                    cx.stop_propagation();
                }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = include_str!("render.rs");

    #[test]
    fn of58ab_template_inset_policy_matches_main_menu_content_frame() {
        let resolved = template_prompt_body_insets(DesignVariant::Default).resolve();
        let def = crate::designs::current_main_menu_theme().def();
        let spacing = get_tokens(DesignVariant::Default).spacing();
        assert_eq!(resolved.x_px, def.shell.content_inset_x);
        assert_eq!(resolved.y_px, spacing.padding_sm);
    }

    #[gpui::test]
    fn of58ab_layout_lock_template_body_uses_main_menu_content_insets(
        cx: &mut gpui::TestAppContext,
    ) {
        let def = crate::designs::current_main_menu_theme().def();
        let spacing = get_tokens(DesignVariant::Default).spacing();
        cx.update(gpui_component::init);
        let window = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(480.0), px(320.0)),
            )));
            cx.open_window(options, |_, cx| {
                let focus_handle = cx.focus_handle();
                cx.new(|_| {
                    TemplatePrompt::new(
                        "layout-lock".to_string(),
                        "Hello {{name}}".to_string(),
                        focus_handle,
                        Arc::new(|_, _| {}),
                        Arc::new(theme::Theme::default()),
                    )
                })
            })
            .expect("template layout test window should open")
        });

        cx.run_until_parked();

        window
            .update(cx, |_, window, _| {
                let body = window
                    .debug_bounds_entries()
                    .iter()
                    .find(|entry| entry.selector == TEMPLATE_PROMPT_BODY_SELECTOR)
                    .expect("template body should publish debug bounds");
                assert_eq!(body.bounds.origin.x, px(def.shell.content_inset_x));
                assert_eq!(body.bounds.origin.y, px(spacing.padding_sm));
            })
            .expect("template layout test window should update");
    }

    #[test]
    fn template_render_uses_shared_create_flow_helpers() {
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
        assert!(
            SOURCE.contains("prompt_text_field("),
            "render.rs should use prompt_text_field"
        );
        assert!(
            SOURCE.contains("prompt_field_style("),
            "render.rs should use prompt_field_style"
        );
        assert!(
            SOURCE.contains("prompt_text_palette("),
            "render.rs should use the shared text palette"
        );
    }

    #[test]
    fn template_render_exposes_scrollable_clickable_field_region() {
        assert!(
            SOURCE.contains("template-fields-scroll"),
            "template fields should have a stable scroll region id"
        );
        assert!(
            SOURCE.contains(".overflow_y_scrollbar()"),
            "template fields should scroll when placeholders exceed available height"
        );
        assert!(
            SOURCE.contains(".on_click(") && SOURCE.contains("set_current_input(idx)"),
            "template fields should be clickable and move the current input"
        );
    }

    #[test]
    fn template_render_no_longer_renders_inline_shortcut_footer_text() {
        let production_code = SOURCE.split("#[cfg(test)]").next().unwrap_or(SOURCE);

        assert!(
            !production_code
                .contains("Tab: next field | Shift+Tab: previous | Enter: submit | Escape: cancel"),
            "render.rs production code should not contain inline shortcut footer text"
        );
    }
}
