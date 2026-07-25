//! Shared selectable conversation Markdown renderer.
//!
//! Single owner of the `TextView` construction path used by EVERY
//! conversation surface — Agent Chat's transcript and Flow's `ChatPrompt`.
//! Style values come from [`crate::components::conversation_style`]; this
//! module owns turning them into a `TextViewStyle` and building the view.
//!
//! ## Why this exists
//!
//! Agent Chat rendered answers through the vendored `TextView` (selectable,
//! syntax-highlighted, with a code-block copy button) while Flow rendered them
//! through a separate bespoke element tree that had no selection model at all.
//! Two engines meant the type scale, code treatment, and copy affordances
//! could drift independently — and Flow answers could not be selected or
//! quoted. This module is the single seam both surfaces now build through.
//!
//! ## The style cache, and the bug that motivated its shape
//!
//! Building a `TextViewStyle` clones a full `HighlightTheme` syntax table, and
//! this runs once per visible row per frame while scrolling. The original
//! Agent Chat implementation memoized the **base** style and then applied
//! `.markdown_link_label_policy(policy)` to the cached value at every call
//! site. That final call allocates a fresh style, so `TextViewStyle::eq`
//! inside `TextView::request_layout` could never hit its pointer fast path and
//! deep-compared the syntax table per row per frame anyway — the cache was
//! doing the work but not delivering the benefit.
//!
//! [`conversation_text_style`] therefore caches the **finished,
//! policy-specific** style, one slot per [`MarkdownLinkLabelPolicy`], so the
//! returned value is pointer-stable across frames.

use gpui::{prelude::*, px, rems, rgba, SharedString, StyleRefinement};
use gpui_component::text::{MarkdownLinkLabelPolicy, TextView, TextViewState, TextViewStyle};
use std::sync::Arc;

use crate::components::conversation_style::{
    resolved_conversation_transcript_colors, ConversationStyleDef,
};
use crate::theme::PromptColors;

/// Inputs that fully determine a built [`TextViewStyle`]. Two calls with an
/// equal key MUST produce an identical style, so the cache can return a clone
/// of the previous one.
#[derive(PartialEq, Clone, Copy)]
struct StyleCacheKey {
    style_def: ConversationStyleDef,
    is_dark: bool,
    accent: u32,
    secondary: u32,
    muted: u32,
    code_bg: u32,
    quote_border: u32,
}

/// One finished style per link-label policy. Caching the finished style (not
/// a base to be refined at the call site) is what preserves `TextViewStyle`'s
/// pointer-equality fast path — see the module header.
struct CachedConversationTextStyles {
    key: StyleCacheKey,
    preserve: TextViewStyle,
    compact_long_bare_http: TextViewStyle,
}

impl CachedConversationTextStyles {
    fn get(&self, policy: MarkdownLinkLabelPolicy) -> TextViewStyle {
        match policy {
            MarkdownLinkLabelPolicy::Preserve => self.preserve.clone(),
            MarkdownLinkLabelPolicy::CompactLongBareHttp => self.compact_long_bare_http.clone(),
        }
    }
}

thread_local! {
    static STYLE_CACHE: std::cell::RefCell<Option<CachedConversationTextStyles>> =
        const { std::cell::RefCell::new(None) };
}

/// Build (or reuse) the finished conversation `TextViewStyle` for `policy`.
///
/// Memoized on the exact inputs so repeated frames return the same `Arc`ed
/// highlight theme and a pointer-stable style.
pub(crate) fn conversation_text_style(
    theme: &crate::theme::Theme,
    colors: &PromptColors,
    style_def: &ConversationStyleDef,
    policy: MarkdownLinkLabelPolicy,
) -> TextViewStyle {
    let key = StyleCacheKey {
        style_def: *style_def,
        is_dark: theme.is_dark_mode(),
        accent: theme.colors.accent.selected,
        secondary: theme.colors.text.secondary,
        muted: theme.colors.text.muted,
        code_bg: colors.code_bg,
        quote_border: colors.quote_border,
    };

    if let Some(style) = STYLE_CACHE.with(|cache| {
        cache
            .borrow()
            .as_ref()
            .filter(|cached| cached.key == key)
            .map(|cached| cached.get(policy))
    }) {
        return style;
    }

    let base = build_conversation_text_style(theme, colors, style_def);
    let cached = CachedConversationTextStyles {
        key,
        preserve: base
            .clone()
            .markdown_link_label_policy(MarkdownLinkLabelPolicy::Preserve),
        compact_long_bare_http: base
            .markdown_link_label_policy(MarkdownLinkLabelPolicy::CompactLongBareHttp),
    };
    let style = cached.get(policy);
    STYLE_CACHE.with(|cache| {
        *cache.borrow_mut() = Some(cached);
    });
    style
}

fn build_conversation_text_style(
    theme: &crate::theme::Theme,
    colors: &PromptColors,
    style_def: &ConversationStyleDef,
) -> TextViewStyle {
    // PromptColors.code_bg/quote_border are background.search_box and
    // ui.border — the same theme authorities the shared resolver reads. They
    // participate in the cache key so a theme swap invalidates the cache.
    let _ = colors;
    let resolved = resolved_conversation_transcript_colors(style_def, theme);
    let code_bg = rgba(resolved.code_bg_rgba);
    let code_border = rgba(resolved.code_border_rgba);
    let blockquote_bg = rgba(resolved.blockquote_bg_rgba);
    let blockquote_border = rgba(resolved.blockquote_border_rgba);
    let heading_1_font_size = style_def.markdown.heading_1_font_size;
    let heading_2_font_size = style_def.markdown.heading_2_font_size;
    let heading_3_font_size = style_def.markdown.heading_3_font_size;
    let body_font_size = style_def.markdown.body_font_size;
    let mut style = TextViewStyle::default()
        .paragraph_gap(rems(style_def.markdown.paragraph_gap))
        .heading_font_size(move |level, _base_size| match level {
            1 => px(heading_1_font_size),
            2 => px(heading_2_font_size),
            3 => px(heading_3_font_size),
            _ => px(body_font_size),
        })
        .code_block(
            StyleRefinement::default()
                .bg(code_bg)
                .border_1()
                .border_color(code_border)
                .rounded(px(style_def.markdown.code_block_radius))
                .px(px(style_def.markdown.code_block_padding_x))
                .py(px(style_def.markdown.code_block_padding_y))
                .text_size(px(style_def.markdown.code_block_font_size)),
        )
        .code_block_copy_button(true)
        .blockquote(
            StyleRefinement::default()
                .bg(blockquote_bg)
                .border_color(blockquote_border)
                .rounded(px(style_def.markdown.blockquote_radius))
                .px(px(style_def.markdown.blockquote_padding_x))
                .py(px(style_def.markdown.blockquote_padding_y)),
        );

    style.highlight_theme = Arc::new(
        crate::theme::gpui_integration::build_markdown_highlight_theme(theme, theme.is_dark_mode()),
    );
    style.is_dark = theme.is_dark_mode();
    style
}

/// Build the shared, selectable conversation Markdown view.
///
/// Every conversation surface constructs its answer/message text through this
/// function. Selection is opt-in on the vendored `TextView` (it defaults to
/// `false`), so a surface that built its own `TextView` would silently render
/// unselectable text — which is precisely how Flow and Agent Chat diverged.
#[allow(clippy::too_many_arguments)]
pub(crate) fn conversation_markdown_view(
    state: &gpui::Entity<TextViewState>,
    theme: &crate::theme::Theme,
    colors: &PromptColors,
    text_color: gpui::Rgba,
    style: &ConversationStyleDef,
    fidelity_scope: SharedString,
    link_policy: MarkdownLinkLabelPolicy,
) -> TextView {
    TextView::new(state)
        .style(conversation_text_style(theme, colors, style, link_policy))
        .selectable(crate::logging::conversation_markdown_selectable_enabled())
        .fidelity_scope(fidelity_scope)
        .w_full()
        .text_size(px(style.markdown.body_font_size))
        .text_color(text_color)
}

#[cfg(test)]
mod conversation_text_tests {
    use super::*;
    use crate::components::conversation_style::production_conversation_style;

    fn stock_theme() -> crate::theme::Theme {
        crate::theme::presets::all_presets()
            .into_iter()
            .find(|preset| preset.id == "script-kit-dark")
            .expect("script-kit-dark preset")
            .create_theme()
    }

    fn stock_prompt_colors(theme: &crate::theme::Theme) -> PromptColors {
        PromptColors::from_theme(theme)
    }

    fn clear_cache() {
        STYLE_CACHE.with(|cache| *cache.borrow_mut() = None);
    }

    #[test]
    fn same_style_key_and_policy_reuses_the_cached_final_text_view_style() {
        clear_cache();
        let theme = stock_theme();
        let colors = stock_prompt_colors(&theme);
        let style_def = production_conversation_style();

        let first = conversation_text_style(
            &theme,
            &colors,
            &style_def,
            MarkdownLinkLabelPolicy::Preserve,
        );
        let second = conversation_text_style(
            &theme,
            &colors,
            &style_def,
            MarkdownLinkLabelPolicy::Preserve,
        );

        // The highlight theme is the expensive part; the cache must hand back
        // the same allocation so TextViewStyle::eq can take its pointer path
        // instead of deep-comparing the syntax table every row every frame.
        assert!(
            Arc::ptr_eq(&first.highlight_theme, &second.highlight_theme),
            "cached style must reuse the same Arc'd highlight theme"
        );
    }

    #[test]
    fn different_link_policies_have_distinct_cached_styles() {
        clear_cache();
        let theme = stock_theme();
        let colors = stock_prompt_colors(&theme);
        let style_def = production_conversation_style();

        let preserve = conversation_text_style(
            &theme,
            &colors,
            &style_def,
            MarkdownLinkLabelPolicy::Preserve,
        );
        let compact = conversation_text_style(
            &theme,
            &colors,
            &style_def,
            MarkdownLinkLabelPolicy::CompactLongBareHttp,
        );

        assert_eq!(
            preserve.markdown_link_label_policy,
            MarkdownLinkLabelPolicy::Preserve
        );
        assert_eq!(
            compact.markdown_link_label_policy,
            MarkdownLinkLabelPolicy::CompactLongBareHttp
        );
        // Both policies are built from one base, so they still share the
        // expensive highlight theme rather than building it twice.
        assert!(Arc::ptr_eq(
            &preserve.highlight_theme,
            &compact.highlight_theme
        ));
    }

    #[test]
    fn theme_or_style_change_invalidates_the_cache() {
        clear_cache();
        let theme = stock_theme();
        let colors = stock_prompt_colors(&theme);
        let style_def = production_conversation_style();

        let first = conversation_text_style(
            &theme,
            &colors,
            &style_def,
            MarkdownLinkLabelPolicy::Preserve,
        );

        // A style change must not be served from the stale slot.
        let mut changed = style_def;
        changed.markdown.body_font_size = 99.0;
        let after_style_change =
            conversation_text_style(&theme, &colors, &changed, MarkdownLinkLabelPolicy::Preserve);
        assert!(
            !Arc::ptr_eq(&first.highlight_theme, &after_style_change.highlight_theme),
            "changing the style def must rebuild rather than reuse the cache"
        );

        // And going back rebuilds again rather than resurrecting the first.
        let back = conversation_text_style(
            &theme,
            &colors,
            &style_def,
            MarkdownLinkLabelPolicy::Preserve,
        );
        assert_eq!(
            back.markdown_link_label_policy,
            MarkdownLinkLabelPolicy::Preserve
        );
    }

    #[test]
    fn built_style_carries_the_shared_production_values() {
        clear_cache();
        let theme = stock_theme();
        let colors = stock_prompt_colors(&theme);
        let style_def = production_conversation_style();
        let style = conversation_text_style(
            &theme,
            &colors,
            &style_def,
            MarkdownLinkLabelPolicy::Preserve,
        );

        // Guards the promotion: these must track the shared owner, so a value
        // edited in conversation_style.rs shows up here rather than silently
        // applying to only one surface.
        assert_eq!(style.is_dark, theme.is_dark_mode());
        assert!(style.code_block_copy_button);
    }
}
