//! Render implementation for UnifiedListItem.

// Allow dead_code - this is new code not yet integrated into the main app
#![allow(dead_code)]

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::tooltip::Tooltip;

use crate::components::button::TRANSPARENT;
use crate::designs::icon_variations::{icon_name_from_str, IconName};

use super::types::*;

// =============================================================================
// UnifiedListItem - The main component
// =============================================================================

/// A unified, presentational list item component.
#[derive(IntoElement)]
pub struct UnifiedListItem {
    id: ElementId,
    title: TextContent,
    subtitle: Option<TextContent>,
    leading: Option<LeadingContent>,
    trailing: Option<TrailingContent>,
    state: ItemState,
    density: Density,
    colors: UnifiedListItemColors,
    a11y_label: Option<SharedString>,
    a11y_hint: Option<SharedString>,
    direct_hover: bool,
}

impl UnifiedListItem {
    /// Create a new list item with required id and title.
    pub fn new(id: impl Into<ElementId>, title: TextContent) -> Self {
        Self {
            id: id.into(),
            title,
            subtitle: None,
            leading: None,
            trailing: None,
            state: ItemState::default(),
            density: Density::default(),
            colors: UnifiedListItemColors::default(),
            a11y_label: None,
            a11y_hint: None,
            direct_hover: true,
        }
    }

    pub fn subtitle(mut self, subtitle: TextContent) -> Self {
        self.subtitle = Some(subtitle);
        self
    }

    pub fn subtitle_opt(mut self, subtitle: Option<TextContent>) -> Self {
        self.subtitle = subtitle;
        self
    }

    pub fn leading(mut self, leading: LeadingContent) -> Self {
        self.leading = Some(leading);
        self
    }

    pub fn leading_opt(mut self, leading: Option<LeadingContent>) -> Self {
        self.leading = leading;
        self
    }

    pub fn trailing(mut self, trailing: TrailingContent) -> Self {
        self.trailing = Some(trailing);
        self
    }

    pub fn trailing_opt(mut self, trailing: Option<TrailingContent>) -> Self {
        self.trailing = trailing;
        self
    }

    pub fn state(mut self, state: ItemState) -> Self {
        self.state = state;
        self
    }

    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    pub fn colors(mut self, colors: UnifiedListItemColors) -> Self {
        self.colors = colors;
        self
    }

    pub fn a11y_label(mut self, label: impl Into<SharedString>) -> Self {
        self.a11y_label = Some(label.into());
        self
    }

    pub fn a11y_hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.a11y_hint = Some(hint.into());
        self
    }

    pub fn with_direct_hover(mut self, enabled: bool) -> Self {
        self.direct_hover = enabled;
        self
    }
}

impl RenderOnce for UnifiedListItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let layout = ListItemLayout::from_density(self.density);
        let colors = self.colors;
        let state = self.state;

        let selected_alpha = (colors.selected_opacity * 255.0) as u32;
        let hover_alpha = (colors.hover_opacity * 255.0) as u32;
        let selected_bg = rgba((colors.accent_subtle << 8) | selected_alpha);
        let hover_bg = rgba((colors.accent_subtle << 8) | hover_alpha);

        let bg_color = if state.is_selected {
            selected_bg
        } else if state.is_hovered {
            hover_bg
        } else {
            rgba(TRANSPARENT)
        };

        let title_color = if state.is_disabled {
            rgb(colors.text_dimmed)
        } else {
            rgb(colors.text_primary)
        };

        let subtitle_color = rgb(colors.text_muted);
        let highlight_color = rgb(colors.text_highlight);

        let leading_element = render_leading(&self.leading, &layout, &colors, state.is_selected);
        let title_element = render_text_content(&self.title, title_color, highlight_color, true);
        let subtitle_element = self
            .subtitle
            .as_ref()
            .map(|sub| render_text_content(sub, subtitle_color, highlight_color, false));
        let trailing_element = render_trailing(&self.trailing, &colors);

        let mut content_col = div()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .justify_center()
            .child(title_element);

        if let Some(sub_el) = subtitle_element {
            content_col = content_col.child(sub_el);
        }

        let mut inner = div()
            .w_full()
            .h_full()
            .pl(px(layout.padding_x))
            .pr(px(layout.padding_x))
            .py(px(layout.padding_y))
            .bg(bg_color)
            .rounded(px(layout.radius))
            .text_color(title_color)
            .font_family(crate::list_item::FONT_SYSTEM_UI)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(layout.gap));

        inner = if should_use_pointer_cursor(state.is_disabled) {
            inner.cursor_pointer()
        } else {
            inner.cursor_default()
        };

        if let Some(leading_el) = leading_element {
            inner = inner.child(leading_el);
        }

        inner = inner.child(content_col);

        if let Some(trailing_el) = trailing_element {
            inner = inner.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_shrink_0()
                    .child(trailing_el),
            );
        }

        let mut container = div()
            .w_full()
            .h(px(layout.height))
            .flex()
            .flex_row()
            .items_center()
            .id(self.id);

        if self.direct_hover && !state.is_selected && !state.is_disabled {
            container = container.hover(move |s| s.bg(hover_bg));
        }

        container.child(inner)
    }
}

// =============================================================================
// Render Helpers
// =============================================================================

fn should_use_pointer_cursor(is_disabled: bool) -> bool {
    !is_disabled
}

fn render_leading(
    leading: &Option<LeadingContent>,
    layout: &ListItemLayout,
    colors: &UnifiedListItemColors,
    is_selected: bool,
) -> Option<Div> {
    let icon_color = if is_selected {
        rgb(colors.text_primary)
    } else {
        rgb(colors.text_secondary)
    };

    match leading {
        Some(LeadingContent::Emoji(emoji)) => Some(
            div()
                .w(px(layout.leading_size))
                .h(px(layout.leading_size))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(icon_color)
                .flex_shrink_0()
                .child(emoji.clone()),
        ),
        Some(LeadingContent::Icon { name, color }) => {
            let icon_color_final = color.map(rgb).unwrap_or(icon_color);
            let svg_path = icon_name_from_str(name)
                .map(|i| i.asset_path())
                .unwrap_or_else(|| IconName::Code.asset_path());
            Some(
                div()
                    .w(px(layout.leading_size))
                    .h(px(layout.leading_size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        svg()
                            .path(svg_path)
                            .size(px(layout.leading_size - 4.0))
                            .text_color(icon_color_final),
                    ),
            )
        }
        Some(LeadingContent::AppIcon(render_image)) => {
            let image = render_image.clone();
            Some(
                div()
                    .w(px(layout.leading_size))
                    .h(px(layout.leading_size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        img(move |_w: &mut Window, _cx: &mut App| Some(Ok(image.clone())))
                            .w(px(layout.leading_size))
                            .h(px(layout.leading_size))
                            .object_fit(ObjectFit::Contain),
                    ),
            )
        }
        Some(LeadingContent::AppIconPlaceholder) => Some(
            div()
                .w(px(layout.leading_size))
                .h(px(layout.leading_size))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .bg(rgba((colors.accent_subtle << 8) | 0x40))
                .rounded(px(4.0)),
        ),
        Some(LeadingContent::Custom(_)) => None,
        None => None,
    }
}

fn render_trailing(
    trailing: &Option<TrailingContent>,
    colors: &UnifiedListItemColors,
) -> Option<Div> {
    let hint_color = rgb(colors.text_dimmed);

    match trailing {
        Some(TrailingContent::Shortcut(shortcut)) => Some(
            div()
                .text_xs()
                .font_family(crate::list_item::FONT_MONO)
                .text_color(hint_color)
                .child(shortcut.clone()),
        ),
        Some(TrailingContent::Hint(hint)) => {
            Some(div().text_xs().text_color(hint_color).child(hint.clone()))
        }
        Some(TrailingContent::Count(count)) => Some(
            div()
                .text_xs()
                .text_color(hint_color)
                .child(format!("{}", count)),
        ),
        Some(TrailingContent::Chevron) => Some(div().text_xs().text_color(hint_color).child("→")),
        Some(TrailingContent::Checkmark) => {
            Some(div().text_sm().text_color(rgb(colors.accent)).child("✓"))
        }
        Some(TrailingContent::Custom(_)) => None,
        None => None,
    }
}

fn render_text_content(
    content: &TextContent,
    base_color: Rgba,
    highlight_color: Rgba,
    is_title: bool,
) -> AnyElement {
    let font_weight = if is_title {
        FontWeight::MEDIUM
    } else {
        FontWeight::NORMAL
    };
    let line_height = if is_title { 18.0 } else { 14.0 };

    match content {
        TextContent::Plain(text) => {
            let full_label = text.clone();
            div()
                .id(ElementId::Name(SharedString::from(if is_title {
                    "unified-list-item-title-ellipsis"
                } else {
                    "unified-list-item-subtitle-ellipsis"
                })))
                .when(is_title, |d| d.text_sm())
                .when(!is_title, |d| d.text_xs())
                .font_weight(font_weight)
                .text_color(base_color)
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .line_height(px(line_height))
                .when(
                    crate::list_item::LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED,
                    |element| {
                        element.tooltip(move |window, cx| {
                            Tooltip::new(full_label.clone()).build(window, cx)
                        })
                    },
                )
                .child(text.clone())
                .into_any_element()
        }

        TextContent::Highlighted { text, .. } => {
            let full_label = text.clone();
            let spans = render_highlight_fragments(
                content.highlight_fragments().unwrap_or_default(),
                base_color,
                highlight_color,
            );
            div()
                .id(ElementId::Name(SharedString::from(if is_title {
                    "unified-list-item-title-ellipsis"
                } else {
                    "unified-list-item-subtitle-ellipsis"
                })))
                .when(is_title, |d| d.text_sm())
                .when(!is_title, |d| d.text_xs())
                .font_weight(font_weight)
                .text_color(base_color)
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .line_height(px(line_height))
                .when(
                    crate::list_item::LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED,
                    |element| {
                        element.tooltip(move |window, cx| {
                            Tooltip::new(full_label.clone()).build(window, cx)
                        })
                    },
                )
                .flex()
                .flex_row()
                .children(spans)
                .into_any_element()
        }

        TextContent::Custom(_) => div().into_any_element(),
    }
}

fn render_highlight_fragments(
    fragments: &[HighlightFragment],
    base_color: Rgba,
    highlight_color: Rgba,
) -> Vec<Div> {
    if fragments.is_empty() {
        return Vec::new();
    }

    let mut spans = Vec::with_capacity(fragments.len());
    for fragment in fragments {
        if fragment.is_highlighted {
            spans.push(
                div()
                    .text_color(highlight_color)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(fragment.text.clone()),
            );
        } else {
            spans.push(div().text_color(base_color).child(fragment.text.clone()));
        }
    }

    spans
}

// =============================================================================
// SectionHeader
// =============================================================================

/// A consistent section header for grouped lists.
#[derive(IntoElement)]
pub struct SectionHeader {
    label: SharedString,
    count: Option<usize>,
    colors: UnifiedListItemColors,
}

#[cfg(test)]
mod builder_tests {
    // No `use super::*` here: the parent's `use gpui::*` re-exports
    // `gpui_macros::test`, which would shadow the built-in `#[test]`
    // attribute and expand itself forever (recursion-limit compile error).
    use super::{TextContent, UnifiedListItem};
    use gpui::ElementId;

    #[test]
    fn unified_list_item_direct_hover_defaults_enabled() {
        let item = UnifiedListItem::new(
            ElementId::Name("choice:test".into()),
            TextContent::plain("Test"),
        );

        assert!(item.direct_hover);
    }

    #[test]
    fn unified_list_item_direct_hover_can_be_disabled() {
        let item = UnifiedListItem::new(
            ElementId::Name("choice:test".into()),
            TextContent::plain("Test"),
        )
        .with_direct_hover(false);

        assert!(!item.direct_hover);
    }
}

impl SectionHeader {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            count: None,
            colors: UnifiedListItemColors::default(),
        }
    }

    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    pub fn colors(mut self, colors: UnifiedListItemColors) -> Self {
        self.colors = colors;
        self
    }
}

impl RenderOnce for SectionHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let label_text = if let Some(count) = self.count {
            format!("{} ({})", self.label, count)
        } else {
            self.label.to_string()
        };

        div()
            .w_full()
            .h(px(SECTION_HEADER_HEIGHT))
            .px(px(16.))
            .pt(px(8.))
            .pb(px(4.))
            .flex()
            .flex_col()
            .justify_center()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(self.colors.text_dimmed))
                    .child(label_text),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::should_use_pointer_cursor;

    #[test]
    fn test_cursor_uses_pointer_when_item_is_enabled() {
        assert!(should_use_pointer_cursor(false));
    }

    #[test]
    fn test_cursor_uses_default_when_item_is_disabled() {
        assert!(!should_use_pointer_cursor(true));
    }
}
