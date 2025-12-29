//! DivPrompt - HTML content display
//!
//! Features:
//! - Display HTML content (text extraction for prototype)
//! - Optional Tailwind styling
//! - Simple keyboard: Enter or Escape to submit

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, Render, Window,
};
use std::sync::Arc;

use crate::logging;
use crate::theme;
use crate::utils::strip_html_tags;
use crate::designs::{DesignVariant, get_tokens};

use super::SubmitCallback;

/// DivPrompt - HTML content display
///
/// Features:
/// - Display HTML content (text extraction for prototype)
/// - Optional Tailwind styling
/// - Simple keyboard: Enter or Escape to submit
pub struct DivPrompt {
    pub id: String,
    pub html: String,
    pub tailwind: Option<String>,
    pub focus_handle: FocusHandle,
    pub on_submit: SubmitCallback,
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling (defaults to Default for theme-based styling)
    pub design_variant: DesignVariant,
}

impl DivPrompt {
    pub fn new(
        id: String,
        html: String,
        tailwind: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_design(id, html, tailwind, focus_handle, on_submit, theme, DesignVariant::Default)
    }
    
    pub fn with_design(
        id: String,
        html: String,
        tailwind: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
    ) -> Self {
        logging::log("PROMPTS", &format!("DivPrompt::new with theme colors: bg={:#x}, text={:#x}, design: {:?}", 
            theme.colors.background.main, theme.colors.text.primary, design_variant));
        DivPrompt {
            id,
            html,
            tailwind,
            focus_handle,
            on_submit,
            theme,
            design_variant,
        }
    }

    /// Submit - always with None value (just acknowledgment)
    fn submit(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }
}

impl Focusable for DivPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DivPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Get design tokens for the current design variant
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();
        let spacing = tokens.spacing();
        
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, _cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            
            match key_str.as_str() {
                "enter" | "escape" => this.submit(),
                _ => {}
            }
        });

        // Extract and render text content using shared utility
        let display_text = strip_html_tags(&self.html);

        // Use design tokens for colors (with theme fallback for Default variant)
        let (main_bg, text_color) = if self.design_variant == DesignVariant::Default {
            (
                rgb(self.theme.colors.background.main),
                rgb(self.theme.colors.text.secondary),
            )
        } else {
            (
                rgb(colors.background),
                rgb(colors.text_secondary),
            )
        };

        // Generate semantic IDs for div prompt elements
        let panel_semantic_id = format!("panel:content-{}", self.id);
        
        // Main container - fills entire window height with no bottom gap
        // Content area uses flex_1 to fill all remaining space
        div()
            .id(gpui::ElementId::Name("window:div".into()))
            .flex()
            .flex_col()
            .w_full()
            .h_full()            // Fill container height completely  
            .min_h(px(0.))       // Allow proper flex behavior
            .bg(main_bg)
            .text_color(text_color)
            .p(px(spacing.padding_lg))
            .key_context("div_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(
                div()
                    .id(gpui::ElementId::Name(panel_semantic_id.into()))
                    .flex_1()            // Grow to fill available space to bottom
                    .min_h(px(0.))       // Allow shrinking
                    .w_full()
                    .overflow_y_hidden() // Clip content at container boundary
                    .child(display_text),
            )
            // Footer removed - content now extends to bottom of container
    }
}
