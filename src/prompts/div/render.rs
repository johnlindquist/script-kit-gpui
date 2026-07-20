use super::*;
use crate::components::{FocusablePrompt, FocusablePromptInterceptedKey};
use gpui_component::scroll::ScrollableElement;

impl Focusable for DivPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DivPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Get design tokens for the current design variant
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();

        // Parse HTML into elements
        let elements = parse_html(&self.html);

        // Create link click callback using a weak entity handle
        // This allows us to call back into the DivPrompt to handle submit:value links
        let weak_handle = cx.entity().downgrade();
        let on_link_click: LinkClickCallback = Arc::new(move |href: &str, cx: &mut gpui::App| {
            let href_owned = href.to_string();
            if let Some(entity) = weak_handle.upgrade() {
                entity.update(cx, move |this, _cx| {
                    this.handle_link_click(&href_owned);
                });
            }
        });

        // Create render context using pre-extracted colors (avoids extraction on every render)
        let render_ctx = if self.design_variant == DesignVariant::Default {
            // Use pre-extracted prompt_colors instead of extracting from theme
            RenderContext {
                text_primary: self.prompt_colors.text_primary,
                text_secondary: self.prompt_colors.text_secondary,
                text_tertiary: self.prompt_colors.text_tertiary,
                accent_color: self.prompt_colors.accent_color,
                code_bg: self.prompt_colors.code_bg,
                quote_border: self.prompt_colors.quote_border,
                hr_color: self.prompt_colors.hr_color,
                on_link_click: Some(on_link_click),
            }
        } else {
            RenderContext {
                text_primary: colors.text_primary,
                text_secondary: colors.text_secondary,
                text_tertiary: colors.text_muted, // Use text_muted for tertiary
                accent_color: colors.accent,
                code_bg: colors.background_tertiary, // Use background_tertiary for code bg
                quote_border: colors.border,
                hr_color: colors.border,
                on_link_click: Some(on_link_click),
            }
        };

        // Determine container background:
        // 1. If container_options.background is set, use that
        // 2. If container_options.opacity is set, apply that to base color
        // 3. Otherwise use vibrancy foundation (None when vibrancy enabled)
        let container_bg: Option<Hsla> =
            if let Some(custom_bg) = self.container_options.parse_background() {
                // Custom background specified - always use it
                Some(custom_bg)
            } else if let Some(opacity) = self.container_options.opacity {
                // Opacity specified - apply to theme/design color
                let base_color = if self.design_variant == DesignVariant::Default {
                    self.theme.colors.background.main
                } else {
                    colors.background
                };
                Some(rgb_to_hsla(base_color, Some(opacity)))
            } else {
                // No custom background or opacity - use vibrancy foundation
                // Returns None when vibrancy enabled (let Root handle bg)
                get_vibrancy_background(&self.theme).map(Hsla::from)
            };

        // Determine container padding from design tokens for consistent prompt spacing.
        let container_padding = self
            .container_options
            .get_padding(default_container_padding(self.design_variant));

        // Generate semantic IDs for div prompt elements
        let panel_semantic_id = format!("panel:content-{}", self.id);

        // Render the HTML elements with any inline Tailwind classes
        let content = render_elements(&elements, render_ctx);

        // Apply root tailwind classes if provided (legacy support)
        let styled_content = if let Some(tw) = &self.tailwind {
            apply_tailwind_styles(content, tw)
        } else {
            content
        };

        // Build the content container with optional containerClasses
        // Apply containerClasses first (before .id() which makes it Stateful for overflow_y_scroll)
        let content_base = div()
            .flex_1() // Grow to fill available space to bottom
            .min_h(px(0.)) // Allow shrinking
            .w_full()
            .child(styled_content);

        let content_styled = if let Some(ref classes) = self.container_options.container_classes {
            apply_tailwind_styles(content_base, classes)
        } else {
            content_base
        };

        // Add ID to make it Stateful, then enable tracked scrolling and visible vertical scrollbar
        // overflow_y_scrollbar requires ScrollableElement trait (needs .id() first)
        let content_container = content_styled
            .id(gpui::ElementId::Name(panel_semantic_id.into()))
            .track_scroll(&self.scroll_handle)
            .overflow_y_scrollbar()
            .p(px(container_padding));

        // SDK container padding belongs to the authored inner content plane;
        // the shared main-menu inset remains non-removable outside it.
        let container = crate::components::render_inset_prompt_body(
            "window:div",
            content_container,
            crate::components::PromptBodyInsets::MainMenu(self.design_variant),
        )
        .relative()
        .min_h(px(0.))
        .when_some(container_bg, |d, bg| d.bg(bg));

        FocusablePrompt::new(container)
            .key_context("div_prompt")
            .focus_handle(self.focus_handle.clone())
            .build(
                window,
                cx,
                |this, intercepted_key, event, _window, _cx| match intercepted_key {
                    FocusablePromptInterceptedKey::Escape => {
                        let modifiers = &event.keystroke.modifiers;
                        if modifiers.platform || modifiers.control || modifiers.alt {
                            return false;
                        }
                        this.submit();
                        true
                    }
                    FocusablePromptInterceptedKey::CmdW | FocusablePromptInterceptedKey::CmdK => {
                        false
                    }
                },
                |this, event, _window, cx| {
                    let modifiers = &event.keystroke.modifiers;
                    if modifiers.platform || modifiers.control || modifiers.alt {
                        return;
                    }

                    if is_div_submit_key(event.keystroke.key.as_str()) {
                        this.submit();
                        cx.stop_propagation();
                    }
                },
            )
    }
}
