//! Shared read-only `kit://` resource preview surface.
//!
//! The Day Page (main window) and the Notes window both open deeplinked
//! `kit://` resources in a read-only inspector. This component owns the
//! preview chrome — a "Preview" mode badge, header, scrollable body, and
//! optional in-body action/hint rows — so padding, typography, and visual
//! language cannot drift between windows. Hosts with a native footer (the
//! Day Page) pass empty `actions`/`footer_hints` and surface those actions
//! as native footer buttons instead; hosts without one (the Notes window)
//! keep the in-body rows.
//!
//! Horizontal inset contract: callers pass the host editor's own text inset
//! (`NotesEditorLayout.padding_x`) as `inset_x` so preview text aligns with
//! prose rendered in the same window.

use gpui::{div, prelude::*, px, AnyElement, App, FontWeight, SharedString};
use gpui_component::theme::ActiveTheme;

use crate::components::hint_strip::{render_hint_icons_clickable, ClickableHint, HintClickHandler};
use crate::list_item::FONT_MONO;
use crate::ui::chrome::{alpha_from_opacity, HINT_TEXT_OPACITY};

/// Muted opacity for secondary metadata rows (mime line).
/// Matches the Notes window `OPACITY_MUTED` token.
const MUTED_OPACITY: f32 = 0.7;
/// Border opacity for the "Preview" mode badge chip.
const BADGE_BORDER_OPACITY: f32 = 0.35;
/// Corner radius for the "Preview" mode badge chip.
const BADGE_RADIUS: f32 = 4.0;
/// Border opacity for the scrollable body frame.
/// Matches the Notes window `OPACITY_SECTION_BORDER` token.
const BORDER_OPACITY: f32 = 0.2;
/// Corner radius for the scrollable body frame.
const BODY_RADIUS: f32 = 6.0;
/// Vertical padding around the whole preview surface.
const INSET_Y: f32 = 8.0;
/// Internal padding of the scrollable body frame.
const BODY_PADDING: f32 = 12.0;
/// Distance of the hover hint chip from the editor viewport edges.
const HOVER_HINT_INSET: f32 = 8.0;
/// Horizontal padding inside the hover hint chip.
const HOVER_HINT_PADDING_X: f32 = 8.0;
/// Vertical padding inside the hover hint chip.
const HOVER_HINT_PADDING_Y: f32 = 4.0;
/// Background opacity of the hover hint chip — high enough to stay legible
/// over prose, low enough to read as a transient overlay.
const HOVER_HINT_BG_OPACITY: f32 = 0.92;
/// Longest href shown in the hover hint chip before middle truncation.
const HOVER_HINT_HREF_MAX_CHARS: usize = 44;
/// One header action link ("Copy URI", "Close Preview", …).
pub(crate) struct ResourcePreviewAction {
    /// Full element id, e.g. "day-page-kit-resource-preview-copy-uri".
    /// Runtime probes address these ids — keep them stable per surface.
    pub id: SharedString,
    pub label: SharedString,
    /// Muted actions (Close) render in muted foreground; the rest in accent.
    pub muted: bool,
    pub on_click: HintClickHandler,
}

pub(crate) struct ResourcePreviewSurface {
    /// Element id prefix, e.g. "day-page-kit-resource-preview".
    pub id_prefix: &'static str,
    pub title: SharedString,
    pub uri: SharedString,
    pub mime_type: SharedString,
    pub text: SharedString,
    pub truncated: bool,
    /// Horizontal content inset; pass the host editor's text inset so the
    /// preview aligns with prose in the same window.
    pub inset_x: f32,
    pub actions: Vec<ResourcePreviewAction>,
    /// Clickable keycap hints for the footer line, in hint-strip syntax
    /// (e.g. "Esc Close", "⌘C Copy URI", "↵ Open Source"). These teach the
    /// preview keyboard contract and double as buttons.
    pub footer_hints: Vec<ClickableHint>,
}

fn preview_id(prefix: &str, suffix: &str) -> SharedString {
    SharedString::from(format!("{prefix}-{suffix}"))
}

#[allow(dead_code)] // Used by the binary target's Day Page clipboard shelf.
pub(crate) struct CompactResourceRow {
    pub id: SharedString,
    pub meta: SharedString,
    pub preview: SharedString,
}

/// What `render_compact_resource_row` actually paints — the shared owner the
/// design-contract exporter reads so the Day Page shelf mockup consumes the
/// row's real metrics instead of value-coincident aliases.
///
/// Framework values mirrored here (gpui `Styled` helpers are rem-relative
/// against the default 16px window rem; there is no public accessor):
/// - `.gap_2()` = 0.5rem → 8px between the mono meta and the preview text.
/// - `.text_xs()` = 0.75rem → 12px row font size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedCompactResourceRowStyle {
    /// `INFO_SPACING.xs` — app-authored source leaf.
    pub padding_x: f32,
    /// `INFO_SPACING.xxs` — app-authored source leaf.
    pub padding_y: f32,
    /// Framework `.gap_2()` (see struct docs).
    pub gap: f32,
    /// Framework `.text_xs()` (see struct docs).
    pub font_size: f32,
    /// Rest text color: gpui-component `theme.muted_foreground` via the
    /// Script Kit theme bridge.
    pub rest_color: gpui::Hsla,
    /// Hover text color: gpui-component `theme.foreground` via the bridge.
    pub hover_color: gpui::Hsla,
}

/// Framework `.gap_2()` = 0.5rem × 16px rem (mirror; tripwire in tests).
const COMPACT_ROW_GAP_PX: f32 = 8.0;
/// Framework `.text_xs()` = 0.75rem × 16px rem (mirror; tripwire in tests).
const COMPACT_ROW_FONT_SIZE_PX: f32 = 12.0;

/// Resolve the compact row's painted style for a Script Kit theme through
/// the SAME theme bridge (`map_scriptkit_to_gpui_theme`) the renderer's
/// `cx.theme()` values come from. Pure: safe for the checked-in exporter.
pub(crate) fn resolved_compact_resource_row_style(
    sk_theme: &crate::theme::Theme,
) -> ResolvedCompactResourceRowStyle {
    let bridge = crate::theme::gpui_integration::map_scriptkit_to_gpui_theme(
        sk_theme,
        sk_theme.is_dark_mode(),
    );
    ResolvedCompactResourceRowStyle {
        padding_x: crate::components::INFO_SPACING.xs,
        padding_y: crate::components::INFO_SPACING.xxs,
        gap: COMPACT_ROW_GAP_PX,
        font_size: COMPACT_ROW_FONT_SIZE_PX,
        rest_color: bridge.muted_foreground,
        hover_color: bridge.foreground,
    }
}

#[allow(dead_code)] // Used by the binary target's Day Page clipboard shelf.
pub(crate) fn render_compact_resource_row(
    row: CompactResourceRow,
    cx: &App,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let muted = cx.theme().muted_foreground;
    let foreground = cx.theme().foreground;

    div()
        .id(row.id)
        .w_full()
        .flex()
        .items_center()
        .gap_2()
        .px(px(crate::components::INFO_SPACING.xs))
        .py(px(crate::components::INFO_SPACING.xxs))
        .text_xs()
        .text_color(muted)
        .cursor_pointer()
        .hover(move |style| style.text_color(foreground))
        .on_click(on_click)
        .child(div().font_family(FONT_MONO).child(row.meta))
        .child(div().flex_1().min_w(px(0.)).truncate().child(row.preview))
}

/// Transient chip shown while the mouse hovers a deeplink in a markdown
/// editor, teaching what a click will do before the user commits to it
/// ("Click · Preview" + the destination). Display-only by design: moving the
/// mouse toward the chip leaves the link, which hides the chip, so click
/// handlers here could never be reached reliably.
///
/// Hosts overlay it inside the editor's relative container; it anchors to the
/// bottom-right corner and never reflows the editor.
pub(crate) fn render_deeplink_hover_hint(
    id: &'static str,
    verb: &'static str,
    href: &str,
    cx: &App,
) -> AnyElement {
    div()
        .id(SharedString::from(id))
        .absolute()
        .bottom(px(HOVER_HINT_INSET))
        .right(px(HOVER_HINT_INSET))
        .flex()
        .items_center()
        .gap_2()
        .px(px(HOVER_HINT_PADDING_X))
        .py(px(HOVER_HINT_PADDING_Y))
        .rounded(px(BADGE_RADIUS))
        .bg(cx.theme().background.opacity(HOVER_HINT_BG_OPACITY))
        .border_1()
        .border_color(cx.theme().accent.opacity(BADGE_BORDER_OPACITY))
        .text_xs()
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().accent)
                .child(format!("Click · {verb}")),
        )
        .child(
            div()
                .text_color(cx.theme().muted_foreground)
                .child(truncate_href_for_hover_hint(href)),
        )
        .into_any_element()
}

/// Middle-truncate long hrefs so the chip stays a single compact line.
fn truncate_href_for_hover_hint(href: &str) -> String {
    let chars: Vec<char> = href.chars().collect();
    if chars.len() <= HOVER_HINT_HREF_MAX_CHARS {
        return href.to_string();
    }
    let keep = HOVER_HINT_HREF_MAX_CHARS - 1;
    let head = keep / 2 + keep % 2;
    let tail = keep / 2;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(chars[chars.len() - tail..].iter());
    out
}

pub(crate) fn render_resource_preview(surface: ResourcePreviewSurface, cx: &App) -> AnyElement {
    let ResourcePreviewSurface {
        id_prefix,
        title,
        uri,
        mime_type,
        text,
        truncated,
        inset_x,
        actions,
        footer_hints,
    } = surface;

    let action_row = if actions.is_empty() {
        None
    } else {
        let mut row = div().flex().items_center().gap_2();
        for action in actions {
            let color = if action.muted {
                cx.theme().muted_foreground
            } else {
                cx.theme().accent
            };
            let on_click = action.on_click;
            row = row.child(
                div()
                    .id(action.id)
                    .text_xs()
                    .text_color(color)
                    .cursor_pointer()
                    .hover(|s| s.text_color(cx.theme().foreground))
                    .on_click(move |event, window, cx| on_click(event, window, cx))
                    .child(action.label),
            );
        }
        Some(row)
    };

    // Mode badge: the preview replaces the host editor, so the surface must
    // say it is a read-only preview (Escape returns, it does not close the
    // window). Rendered as a small accent chip next to the title.
    let mode_badge = div()
        .id(preview_id(id_prefix, "mode-badge"))
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().accent)
        .border_1()
        .border_color(cx.theme().accent.opacity(BADGE_BORDER_OPACITY))
        .rounded(px(BADGE_RADIUS))
        .px(px(6.))
        .child("Preview");

    let hint_text_rgba = {
        let theme = crate::theme::get_cached_theme();
        ((theme.colors.text.primary & 0x00FF_FFFF) << 8) | alpha_from_opacity(HINT_TEXT_OPACITY)
    };

    div()
        .id(SharedString::from(id_prefix))
        .flex_1()
        .h_full()
        .min_h(px(0.))
        .flex()
        .flex_col()
        .gap_3()
        .px(px(inset_x))
        .py(px(INSET_Y))
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div().flex().items_center().gap_2().child(mode_badge).child(
                                div()
                                    .id(preview_id(id_prefix, "title"))
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            ),
                        )
                        .child(
                            div()
                                .id(preview_id(id_prefix, "uri"))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(uri),
                        )
                        .child(
                            div()
                                .id(preview_id(id_prefix, "meta"))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground.opacity(MUTED_OPACITY))
                                .child(format!(
                                    "{mime_type} · read-only{}",
                                    if truncated { " · truncated" } else { "" }
                                )),
                        ),
                )
                .when_some(action_row, |parent, row| parent.child(row)),
        )
        .child(
            div()
                .id(preview_id(id_prefix, "body"))
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .rounded(px(BODY_RADIUS))
                .border_1()
                .border_color(cx.theme().border.opacity(BORDER_OPACITY))
                .p(px(BODY_PADDING))
                .text_xs()
                .font_family(FONT_MONO)
                .text_color(cx.theme().foreground)
                .child(text),
        )
        .when(!footer_hints.is_empty(), |parent| {
            parent.child(
                div()
                    .id(preview_id(id_prefix, "hints"))
                    .flex()
                    .items_center()
                    .child(render_hint_icons_clickable(footer_hints, hint_text_rgba)),
            )
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_href_is_untouched() {
        assert_eq!(
            truncate_href_for_hover_hint("kit://notes"),
            "kit://notes".to_string()
        );
    }

    #[test]
    fn long_href_is_middle_truncated_to_budget() {
        let href = format!("kit://notes/{}", "a".repeat(80));
        let shown = truncate_href_for_hover_hint(&href);
        assert_eq!(shown.chars().count(), HOVER_HINT_HREF_MAX_CHARS);
        assert!(shown.starts_with("kit://notes/"));
        assert!(shown.contains('…'));
        assert!(shown.ends_with('a'));
    }

    #[test]
    fn truncation_respects_multibyte_chars() {
        let href = format!("https://example.com/{}", "é".repeat(60));
        let shown = truncate_href_for_hover_hint(&href);
        assert_eq!(shown.chars().count(), HOVER_HINT_HREF_MAX_CHARS);
    }

    #[test]
    fn stock_theme_resolves_compact_row_paint_style() {
        let theme = crate::theme::presets::all_presets()
            .into_iter()
            .find(|preset| preset.id == "script-kit-dark")
            .expect("script-kit-dark preset")
            .create_theme();
        let style = resolved_compact_resource_row_style(&theme);

        assert_eq!(style.padding_x, crate::components::INFO_SPACING.xs);
        assert_eq!(style.padding_y, crate::components::INFO_SPACING.xxs);
        // Framework mirror tripwires (.gap_2 / .text_xs at the 16px rem).
        assert_eq!(style.gap, 8.0);
        assert_eq!(style.font_size, 12.0);
        // muted_foreground routes through the semantic placeholder ladder:
        // text.primary at opacity.text_placeholder (0.40 in the stock theme).
        assert_eq!(style.rest_color.a, theme.get_opacity().text_placeholder);
        assert_eq!(style.hover_color.a, 1.0);
    }
}
