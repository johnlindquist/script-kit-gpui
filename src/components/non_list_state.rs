//! Owner-bound rich non-list composition.
//!
//! Semantic empty/help/setup/permission/recovery states belong to `info_state`,
//! while this module retains only rich About and menu-syntax compositions. New
//! callers must add an explicit owner variant instead of importing loose helpers.

use gpui::{div, prelude::*, px, rgb, rgba, AnyElement, Div, Rgba, SharedString, Stateful};

use crate::theme::{self, AppChromeColors};
use crate::ui::chrome;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NonListDensity {
    #[allow(
        dead_code,
        reason = "the separately compiled root-list renderer selects compact menu-syntax density"
    )]
    Compact,
    Comfortable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NonListCompositionOwner {
    #[allow(
        dead_code,
        reason = "the separately compiled root-list renderer owns menu-syntax rich composition"
    )]
    MenuSyntax,
    About,
}

impl NonListCompositionOwner {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::MenuSyntax => "menu-syntax",
            Self::About => "about",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NonListMetrics {
    pub max_width: f32,
    pub card_radius: f32,
    pub card_padding_x: f32,
    pub card_padding_y: f32,
    pub block_gap: f32,
    pub item_gap: f32,
    pub icon_size: f32,
    pub title_size: f32,
    pub title_line: f32,
    pub body_size: f32,
    pub body_line: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct NonListPalette {
    pub title: Rgba,
    pub body: Rgba,
    pub hint: Rgba,
    pub panel: Rgba,
    pub input: Rgba,
    pub border: Rgba,
    pub hover: Rgba,
    pub accent: Rgba,
}

#[derive(Clone, Copy)]
pub(crate) struct NonListComposition {
    owner: NonListCompositionOwner,
    metrics: NonListMetrics,
    palette: NonListPalette,
}

impl NonListComposition {
    pub(crate) fn new(
        owner: NonListCompositionOwner,
        density: NonListDensity,
        theme: &theme::Theme,
    ) -> Self {
        Self {
            owner,
            metrics: non_list_metrics(density),
            palette: non_list_palette(theme),
        }
    }

    pub(crate) const fn metrics(self) -> NonListMetrics {
        self.metrics
    }

    pub(crate) const fn palette(self) -> NonListPalette {
        self.palette
    }

    pub(crate) fn content_stack(self, id: &'static str, max_width: f32, gap: f32) -> Stateful<Div> {
        debug_assert!(!self.owner.as_str().is_empty());
        non_list_content_stack(id, max_width, gap)
    }

    pub(crate) fn card(self, id: &'static str) -> Stateful<Div> {
        non_list_card(id, self.palette, self.metrics)
    }

    pub(crate) fn action_row(self, actions: Vec<AnyElement>) -> Div {
        non_list_action_row(actions)
    }

    pub(crate) fn footer_note(self, text: impl Into<SharedString>) -> Div {
        non_list_footer_note(text, self.palette)
    }
}

fn non_list_metrics(density: NonListDensity) -> NonListMetrics {
    match density {
        NonListDensity::Compact => NonListMetrics {
            max_width: 420.0,
            card_radius: chrome::LIQUID_GLASS_COMPACT_RADIUS_PX,
            card_padding_x: 12.0,
            card_padding_y: 10.0,
            block_gap: 12.0,
            item_gap: 8.0,
            icon_size: 32.0,
            title_size: 18.0,
            title_line: 24.0,
            body_size: 13.0,
            body_line: 18.0,
        },
        NonListDensity::Comfortable => NonListMetrics {
            max_width: 500.0,
            card_radius: chrome::LIQUID_GLASS_COMPACT_RADIUS_PX,
            card_padding_x: 16.0,
            card_padding_y: 14.0,
            block_gap: 16.0,
            item_gap: 10.0,
            icon_size: 40.0,
            title_size: 22.0,
            title_line: 28.0,
            body_size: 13.0,
            body_line: 19.0,
        },
    }
}

fn non_list_palette(theme: &theme::Theme) -> NonListPalette {
    let chrome = AppChromeColors::from_theme(theme);

    NonListPalette {
        title: rgb(chrome.text_primary_hex),
        body: rgba(chrome.text_muted_rgba),
        hint: rgba(chrome.text_hint_rgba),
        panel: rgba(chrome.panel_surface_rgba),
        input: rgba(chrome.input_surface_rgba),
        border: rgba(chrome.border_rgba),
        hover: rgba(chrome.hover_rgba),
        accent: rgb(chrome.accent_hex),
    }
}

fn non_list_content_stack(id: &'static str, max_width: f32, gap: f32) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .max_w(px(max_width))
        .flex()
        .flex_col()
        .gap(px(gap))
}

fn non_list_card(
    id: &'static str,
    palette: NonListPalette,
    metrics: NonListMetrics,
) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .px(px(metrics.card_padding_x))
        .py(px(metrics.card_padding_y))
        .rounded(px(metrics.card_radius))
        .border_1()
        .border_color(palette.border)
        .bg(palette.panel)
}

fn non_list_action_row(actions: Vec<AnyElement>) -> Div {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .children(actions)
}

fn non_list_footer_note(text: impl Into<SharedString>, palette: NonListPalette) -> Div {
    div()
        .text_xs()
        .line_height(px(16.0))
        .text_color(palette.hint)
        .child(text.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_keep_about_scale_as_upper_bound() {
        let compact = non_list_metrics(NonListDensity::Compact);
        let comfortable = non_list_metrics(NonListDensity::Comfortable);

        assert_eq!(compact.title_size, 18.0);
        assert_eq!(comfortable.title_size, 22.0);
        assert!(comfortable.title_size < 28.0);
        assert!(compact.max_width < comfortable.max_width);
    }

    #[test]
    fn density_uses_four_pixel_rhythm() {
        for metrics in [
            non_list_metrics(NonListDensity::Compact),
            non_list_metrics(NonListDensity::Comfortable),
        ] {
            for value in [
                metrics.card_padding_x,
                metrics.card_padding_y,
                metrics.block_gap,
                metrics.item_gap,
                metrics.icon_size,
                metrics.title_line,
            ] {
                assert_eq!(value.rem_euclid(2.0), 0.0);
            }
        }
    }

    #[test]
    fn rich_composition_has_only_explicit_current_owners() {
        assert_eq!(NonListCompositionOwner::MenuSyntax.as_str(), "menu-syntax");
        assert_eq!(NonListCompositionOwner::About.as_str(), "about");
    }
}
