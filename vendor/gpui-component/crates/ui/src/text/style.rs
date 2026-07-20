use std::sync::Arc;

use gpui::{Pixels, Rems, StyleRefinement, px, rems};

use crate::highlighter::HighlightTheme;

/// Controls how Markdown links are presented without changing their targets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkdownLinkLabelPolicy {
    /// Preserve the source-authored visible link text.
    #[default]
    Preserve,
    /// Compact long source-syntax bare HTTP(S) URLs to their authority.
    CompactLongBareHttp,
}

/// TextViewStyle used to customize the style for [`TextView`].
#[derive(Clone)]
pub struct TextViewStyle {
    /// Gap of each paragraphs, default is 1 rem.
    pub paragraph_gap: Rems,
    /// Base font size for headings, default is 14px.
    pub heading_base_font_size: Pixels,
    /// Function to calculate heading font size based on heading level (1-6).
    ///
    /// The first parameter is the heading level (1-6), the second parameter is the base font size.
    /// The second parameter is the base font size.
    pub heading_font_size: Option<Arc<dyn Fn(u8, Pixels) -> Pixels + Send + Sync + 'static>>,
    /// Highlight theme for code blocks. Default: [`HighlightTheme::default_light()`]
    pub highlight_theme: Arc<HighlightTheme>,
    /// The style refinement for code blocks.
    pub code_block: StyleRefinement,
    /// Whether code blocks render a hover-revealed copy button.
    pub code_block_copy_button: bool,
    /// The style refinement for blockquotes.
    pub blockquote: StyleRefinement,
    /// Display policy for Markdown link labels. Defaults to preserving source text.
    pub markdown_link_label_policy: MarkdownLinkLabelPolicy,
    pub is_dark: bool,
}

impl PartialEq for TextViewStyle {
    fn eq(&self, other: &Self) -> bool {
        self.paragraph_gap == other.paragraph_gap
            && self.heading_base_font_size == other.heading_base_font_size
            // Pointer fast path first: this comparison runs for every
            // TextView on every frame, and deep-comparing a HighlightTheme
            // walks the whole syntax style table.
            && (Arc::ptr_eq(&self.highlight_theme, &other.highlight_theme)
                || self.highlight_theme == other.highlight_theme)
            && self.code_block == other.code_block
            && self.code_block_copy_button == other.code_block_copy_button
            && self.blockquote == other.blockquote
            && self.markdown_link_label_policy == other.markdown_link_label_policy
            && self.is_dark == other.is_dark
    }
}

impl Default for TextViewStyle {
    fn default() -> Self {
        Self {
            paragraph_gap: rems(1.),
            heading_base_font_size: px(14.),
            heading_font_size: None,
            highlight_theme: HighlightTheme::default_light().clone(),
            code_block: StyleRefinement::default(),
            code_block_copy_button: false,
            blockquote: StyleRefinement::default(),
            markdown_link_label_policy: MarkdownLinkLabelPolicy::Preserve,
            is_dark: false,
        }
    }
}

impl TextViewStyle {
    /// Set paragraph gap, default is 1 rem.
    pub fn paragraph_gap(mut self, gap: Rems) -> Self {
        self.paragraph_gap = gap;
        self
    }

    pub fn heading_font_size<F>(mut self, f: F) -> Self
    where
        F: Fn(u8, Pixels) -> Pixels + Send + Sync + 'static,
    {
        self.heading_font_size = Some(Arc::new(f));
        self
    }

    /// Set style for code blocks.
    pub fn code_block(mut self, style: StyleRefinement) -> Self {
        self.code_block = style;
        self
    }

    /// Enable or disable the built-in copy button for code blocks.
    pub fn code_block_copy_button(mut self, enabled: bool) -> Self {
        self.code_block_copy_button = enabled;
        self
    }

    /// Set style for blockquotes.
    pub fn blockquote(mut self, style: StyleRefinement) -> Self {
        self.blockquote = style;
        self
    }

    /// Set the Markdown link-label presentation policy.
    pub fn markdown_link_label_policy(mut self, policy: MarkdownLinkLabelPolicy) -> Self {
        self.markdown_link_label_policy = policy;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_block_copy_button_round_trips_in_style() {
        let style = TextViewStyle::default().code_block_copy_button(true);

        assert!(style.code_block_copy_button);
        assert!(style != TextViewStyle::default());
    }
}
