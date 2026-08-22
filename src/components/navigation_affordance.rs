//! Shared in-surface navigation affordances.
//!
//! Preview and drill-in surfaces use this row when Escape/click returns to a
//! parent context rather than closing the window. Keeping the arrow, spacing,
//! text hierarchy, and hover treatment here makes that distinction stable.

use gpui::{div, prelude::*, px, rgba, SharedString};

#[allow(
    dead_code,
    reason = "the separately compiled day-page renderer mounts this shared back affordance"
)]
pub(crate) fn render_back_affordance(
    id: SharedString,
    label: SharedString,
    theme: &crate::theme::Theme,
    on_mouse_down: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let chrome = crate::theme::AppChromeColors::from_theme(theme);
    let rest = rgba(chrome.text_muted_rgba);
    let hover = rgba(chrome.text_strong_rgba);

    div()
        .id(id)
        .w_full()
        .flex()
        .items_center()
        .gap(px(crate::components::INFO_SPACING.xs))
        .pb(px(crate::components::INFO_SPACING.xxs))
        .text_sm()
        .text_color(rest)
        .cursor_pointer()
        .hover(move |style| style.text_color(hover))
        .on_mouse_down(gpui::MouseButton::Left, on_mouse_down)
        .child("←")
        .child(label)
}
