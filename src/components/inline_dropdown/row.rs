//! Shared dense-monoline picker row for InlineDropdown surfaces.
//!
//! All inline dropdown consumers (Agent Chat popups, context selectors, model
//! selectors, presets dropdown) render through these helpers so the chrome
//! stays consistent: launcher-aligned row height, theme-aware selection background,
//! fuzzy-match highlights, and optional leading visual / accessory.

use gpui::{
    div, prelude::FluentBuilder, px, AnyElement, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, Styled,
};
use std::collections::HashSet;

use crate::list_item::{FONT_MONO, LIST_ITEM_HEIGHT, NAME_FONT_SIZE, NAME_LINE_HEIGHT};

/// Impeccable opacity tiers.
pub(crate) const GHOST: f32 = 0.06;
pub(crate) const HINT: f32 = 0.45;
pub(crate) const MUTED_OP: f32 = 0.65;
pub(crate) const SELECTED_ROW_OPACITY: f32 = 0.23;
pub(crate) const SOFT_COMPACT_SELECTED_ROW_OPACITY: f32 = 0.18;

/// Right-side command/meta opacity.
pub(crate) const COMMAND_OPACITY: f32 = 0.30;

/// Picker rows should align with the main launcher row rhythm.
pub(crate) const CONTEXT_SELECTOR_ROW_HEIGHT: f32 = LIST_ITEM_HEIGHT;
pub(crate) const SOFT_COMPACT_PICKER_ROW_HEIGHT: f32 = 36.0;
pub(crate) const CONTEXT_SELECTOR_SYNOPSIS_HEIGHT: f32 = 64.0;
const CONTEXT_SELECTOR_SYNOPSIS_MAX_LINES: usize = 4;
const CONTEXT_SELECTOR_SYNOPSIS_LINE_HEIGHT: f32 = 16.0;

fn hsla_rgba(color: Hsla) -> u32 {
    let rgba = gpui::Rgba::from(color);
    let color_channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    let alpha_channel = (rgba.a.clamp(0.0, 1.0) * 255.0) as u32;
    (color_channel(rgba.r) << 24)
        | (color_channel(rgba.g) << 16)
        | (color_channel(rgba.b) << 8)
        | alpha_channel
}

pub(super) fn picker_row_state_palette(
    foreground: Hsla,
    secondary_foreground: Hsla,
    selected_opacity: f32,
) -> crate::theme::RowStatePalette {
    use crate::theme::{resolve_row_state_palette, RowForegroundColors, RowStateColorInputs};

    let rest = RowForegroundColors {
        primary_rgba: hsla_rgba(foreground.opacity(MUTED_OP)),
        secondary_rgba: hsla_rgba(secondary_foreground),
        icon_rgba: hsla_rgba(foreground.opacity(MUTED_OP)),
        accessory_rgba: hsla_rgba(secondary_foreground),
    };
    let selected = RowForegroundColors {
        primary_rgba: hsla_rgba(foreground),
        secondary_rgba: hsla_rgba(secondary_foreground),
        icon_rgba: hsla_rgba(foreground),
        accessory_rgba: hsla_rgba(secondary_foreground),
    };

    resolve_row_state_palette(RowStateColorInputs {
        rest_background_rgba: None,
        hover_background_rgba: Some(hsla_rgba(foreground.opacity(GHOST))),
        selected_background_rgba: Some(hsla_rgba(foreground.opacity(selected_opacity))),
        active_background_rgba: Some(hsla_rgba(foreground.opacity(selected_opacity))),
        rest_foregrounds: rest,
        selected_foregrounds: selected,
        // Inline pickers currently expose only enabled options. Supplying the
        // muted tier keeps future disabled rows from brightening on hover.
        disabled_foregrounds: rest,
        disabled_selected_foregrounds: rest,
    })
}

/// Render a single dense-monoline picker row.
///
/// Shared by Agent Chat popups, model selectors, presets dropdowns, and any
/// future smart-input dropdown that wants the same launcher-aligned chrome.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_dense_monoline_picker_row(
    id: SharedString,
    label: SharedString,
    meta: SharedString,
    label_highlight_indices: &[usize],
    meta_highlight_indices: &[usize],
    is_selected: bool,
    foreground: Hsla,
    muted_foreground: Hsla,
    accent: Hsla,
) -> gpui::Stateful<gpui::Div> {
    render_dense_monoline_picker_row_full(
        id,
        label,
        meta,
        label_highlight_indices,
        meta_highlight_indices,
        is_selected,
        foreground,
        muted_foreground,
        accent,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_dense_monoline_picker_row_with_accessory(
    id: SharedString,
    label: SharedString,
    meta: SharedString,
    label_highlight_indices: &[usize],
    meta_highlight_indices: &[usize],
    is_selected: bool,
    foreground: Hsla,
    muted_foreground: Hsla,
    accent: Hsla,
    accessory: Option<AnyElement>,
) -> gpui::Stateful<gpui::Div> {
    render_dense_monoline_picker_row_full(
        id,
        label,
        meta,
        label_highlight_indices,
        meta_highlight_indices,
        is_selected,
        foreground,
        muted_foreground,
        accent,
        None,
        accessory,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_dense_monoline_picker_row_with_leading_visual(
    id: SharedString,
    label: SharedString,
    meta: SharedString,
    label_highlight_indices: &[usize],
    meta_highlight_indices: &[usize],
    is_selected: bool,
    foreground: Hsla,
    muted_foreground: Hsla,
    accent: Hsla,
    leading_visual: AnyElement,
) -> gpui::Stateful<gpui::Div> {
    render_dense_monoline_picker_row_full(
        id,
        label,
        meta,
        label_highlight_indices,
        meta_highlight_indices,
        is_selected,
        foreground,
        muted_foreground,
        accent,
        Some(leading_visual),
        None,
    )
}

pub(crate) fn render_soft_compact_picker_row(
    id: SharedString,
    label: SharedString,
    meta: Option<SharedString>,
    label_highlight_indices: &[usize],
    meta_highlight_indices: &[usize],
    is_selected: bool,
    colors: crate::components::inline_dropdown::InlineDropdownColors,
) -> gpui::Stateful<gpui::Div> {
    let label_hits: HashSet<usize> = label_highlight_indices.iter().copied().collect();
    let meta_hits: HashSet<usize> = meta_highlight_indices.iter().copied().collect();
    let row_palette = picker_row_state_palette(
        colors.foreground,
        colors.muted_foreground,
        SOFT_COMPACT_SELECTED_ROW_OPACITY,
    );
    let row_colors = row_palette.for_flags(crate::theme::RowStateFlags {
        selected: is_selected,
        hovered: false,
        active: false,
        disabled: false,
    });
    let foreground: Hsla = gpui::rgba(row_colors.primary_foreground_rgba).into();
    let selected_row_bg: Hsla = gpui::rgba(
        row_palette
            .selected
            .background_rgba
            .expect("selected compact rows always have a background"),
    )
    .into();
    let hover_row_bg: Hsla = gpui::rgba(
        row_palette
            .hovered
            .background_rgba
            .expect("hovered compact rows always have a background"),
    )
    .into();

    div()
        .id(id)
        .w_full()
        .h(px(SOFT_COMPACT_PICKER_ROW_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .pl(px(10.0))
        .pr(px(14.0))
        .py(px(4.0))
        .bg(if is_selected {
            selected_row_bg
        } else {
            gpui::transparent_black()
        })
        .when(!is_selected, |d| d.hover(|el| el.bg(hover_row_bg)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(render_soft_compact_label(
                    &label,
                    &label_hits,
                    foreground,
                    colors.accent,
                )),
        )
        .when_some(meta.filter(|value| !value.is_empty()), |d, meta| {
            d.child(render_soft_compact_meta_badge(
                meta,
                &meta_hits,
                is_selected,
                colors,
            ))
        })
}

#[allow(clippy::too_many_arguments)]
fn render_dense_monoline_picker_row_full(
    id: SharedString,
    label: SharedString,
    meta: SharedString,
    label_highlight_indices: &[usize],
    meta_highlight_indices: &[usize],
    is_selected: bool,
    foreground: Hsla,
    muted_foreground: Hsla,
    accent: Hsla,
    leading_visual: Option<AnyElement>,
    accessory: Option<AnyElement>,
) -> gpui::Stateful<gpui::Div> {
    let label_hits: HashSet<usize> = label_highlight_indices.iter().copied().collect();
    let meta_hits: HashSet<usize> = meta_highlight_indices.iter().copied().collect();

    let show_meta = accessory.is_none() && !meta.is_empty();
    let row_palette = picker_row_state_palette(
        foreground,
        muted_foreground.opacity(COMMAND_OPACITY),
        SELECTED_ROW_OPACITY,
    );
    let row_colors = row_palette.for_flags(crate::theme::RowStateFlags {
        selected: is_selected,
        hovered: false,
        active: false,
        disabled: false,
    });
    let label_foreground: Hsla = gpui::rgba(row_colors.primary_foreground_rgba).into();
    let selected_row_bg: Hsla = gpui::rgba(
        row_palette
            .selected
            .background_rgba
            .expect("selected dense rows always have a background"),
    )
    .into();
    let hover_row_bg: Hsla = gpui::rgba(
        row_palette
            .hovered
            .background_rgba
            .expect("hovered dense rows always have a background"),
    )
    .into();

    let mut left = div().flex().items_center().gap(px(6.0));

    if let Some(leading_visual) = leading_visual {
        left = left.child(leading_visual);
    }

    left = left.child(render_highlighted_text(
        &label,
        &label_hits,
        label_foreground,
        accent,
    ));

    // Interaction state ladder comes from the shared row-state resolver while
    // this family retains its dense geometry and local foreground tiers.

    let mut row = div()
        .id(id)
        .w_full()
        .h(px(CONTEXT_SELECTOR_ROW_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .pl(px(10.0))
        .pr(px(14.0))
        .py(px(4.0))
        .bg(if is_selected {
            selected_row_bg
        } else {
            gpui::transparent_black()
        })
        .when(!is_selected, |d| d.hover(|el| el.bg(hover_row_bg)))
        .child(left)
        .when(show_meta, |d| {
            d.child(render_highlighted_meta(
                &meta,
                &meta_hits,
                muted_foreground.opacity(COMMAND_OPACITY),
                accent.opacity(HINT),
            ))
        });

    if let Some(accessory) = accessory {
        row = row.child(accessory);
    }

    row
}

fn render_soft_compact_label(
    text: &str,
    hits: &HashSet<usize>,
    base: Hsla,
    accent: Hsla,
) -> AnyElement {
    if hits.is_empty() {
        return div()
            .text_size(px(13.0))
            .line_height(px(18.0))
            .font_weight(FontWeight::NORMAL)
            .text_color(base)
            .text_ellipsis()
            .child(SharedString::from(text.to_string()))
            .into_any_element();
    }

    let mut spans: Vec<AnyElement> = Vec::new();
    let mut current = String::new();
    let mut current_highlighted = false;

    for (ix, ch) in text.chars().enumerate() {
        let is_hit = hits.contains(&ix);
        if ix > 0 && is_hit != current_highlighted {
            spans.push(
                div()
                    .text_size(px(13.0))
                    .line_height(px(18.0))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(if current_highlighted { accent } else { base })
                    .child(SharedString::from(std::mem::take(&mut current)))
                    .into_any_element(),
            );
        }
        current_highlighted = is_hit;
        current.push(ch);
    }
    if !current.is_empty() {
        spans.push(
            div()
                .text_size(px(13.0))
                .line_height(px(18.0))
                .font_weight(FontWeight::NORMAL)
                .text_color(if current_highlighted { accent } else { base })
                .child(SharedString::from(current))
                .into_any_element(),
        );
    }

    div()
        .flex()
        .items_center()
        .text_ellipsis()
        .children(spans)
        .into_any_element()
}

fn render_soft_compact_meta_badge(
    meta: SharedString,
    hits: &HashSet<usize>,
    is_selected: bool,
    colors: crate::components::inline_dropdown::InlineDropdownColors,
) -> AnyElement {
    div()
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(colors.foreground.opacity(GHOST))
        .child(render_soft_compact_meta_text(
            &meta,
            hits,
            if is_selected {
                colors.foreground.opacity(MUTED_OP)
            } else {
                colors.muted_foreground.opacity(HINT)
            },
            colors.accent.opacity(HINT),
        ))
        .into_any_element()
}

fn render_soft_compact_meta_text(
    text: &str,
    hits: &HashSet<usize>,
    base: Hsla,
    accent: Hsla,
) -> AnyElement {
    if hits.is_empty() {
        return div()
            .text_size(px(10.5))
            .line_height(px(14.0))
            .font_family(FONT_MONO)
            .text_color(base)
            .text_ellipsis()
            .child(SharedString::from(text.to_string()))
            .into_any_element();
    }

    let mut spans: Vec<AnyElement> = Vec::new();
    let mut current = String::new();
    let mut current_highlighted = false;

    for (ix, ch) in text.chars().enumerate() {
        let is_hit = hits.contains(&ix);
        if ix > 0 && is_hit != current_highlighted {
            spans.push(
                div()
                    .text_size(px(10.5))
                    .line_height(px(14.0))
                    .font_family(FONT_MONO)
                    .text_color(if current_highlighted { accent } else { base })
                    .child(SharedString::from(std::mem::take(&mut current)))
                    .into_any_element(),
            );
        }
        current_highlighted = is_hit;
        current.push(ch);
    }
    if !current.is_empty() {
        spans.push(
            div()
                .text_size(px(10.5))
                .line_height(px(14.0))
                .font_family(FONT_MONO)
                .text_color(if current_highlighted { accent } else { base })
                .child(SharedString::from(current))
                .into_any_element(),
        );
    }

    div()
        .flex()
        .items_center()
        .text_ellipsis()
        .children(spans)
        .into_any_element()
}

/// Render label text with theme-accent highlights on matched characters.
pub(crate) fn render_highlighted_text(
    text: &str,
    hits: &HashSet<usize>,
    base: Hsla,
    accent: Hsla,
) -> AnyElement {
    if hits.is_empty() {
        return div()
            .text_size(px(NAME_FONT_SIZE))
            .line_height(px(NAME_LINE_HEIGHT))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(base)
            .text_ellipsis()
            .child(SharedString::from(text.to_string()))
            .into_any_element();
    }

    let mut spans: Vec<AnyElement> = Vec::new();
    let mut current = String::new();
    let mut current_highlighted = false;

    for (ix, ch) in text.chars().enumerate() {
        let is_hit = hits.contains(&ix);
        if ix > 0 && is_hit != current_highlighted {
            spans.push(
                div()
                    .text_size(px(NAME_FONT_SIZE))
                    .line_height(px(NAME_LINE_HEIGHT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if current_highlighted { accent } else { base })
                    .child(SharedString::from(std::mem::take(&mut current)))
                    .into_any_element(),
            );
        }
        current_highlighted = is_hit;
        current.push(ch);
    }
    if !current.is_empty() {
        spans.push(
            div()
                .text_size(px(NAME_FONT_SIZE))
                .line_height(px(NAME_LINE_HEIGHT))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if current_highlighted { accent } else { base })
                .child(SharedString::from(current))
                .into_any_element(),
        );
    }

    div()
        .flex()
        .items_center()
        .text_ellipsis()
        .children(spans)
        .into_any_element()
}

/// Render right-side meta text in FONT_MONO with optional highlights.
pub(crate) fn render_highlighted_meta(
    text: &str,
    hits: &HashSet<usize>,
    base: Hsla,
    accent: Hsla,
) -> AnyElement {
    if hits.is_empty() {
        return div()
            .text_size(px(12.0))
            .line_height(px(16.0))
            .font_weight(FontWeight::SEMIBOLD)
            .font_family(FONT_MONO)
            .text_color(base)
            .text_ellipsis()
            .child(SharedString::from(text.to_string()))
            .into_any_element();
    }

    let mut spans: Vec<AnyElement> = Vec::new();
    let mut current = String::new();
    let mut current_highlighted = false;

    for (ix, ch) in text.chars().enumerate() {
        let is_hit = hits.contains(&ix);
        if ix > 0 && is_hit != current_highlighted {
            spans.push(
                div()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .font_family(FONT_MONO)
                    .text_color(if current_highlighted { accent } else { base })
                    .child(SharedString::from(std::mem::take(&mut current)))
                    .into_any_element(),
            );
        }
        current_highlighted = is_hit;
        current.push(ch);
    }
    if !current.is_empty() {
        spans.push(
            div()
                .text_size(px(12.0))
                .line_height(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .font_family(FONT_MONO)
                .text_color(if current_highlighted { accent } else { base })
                .child(SharedString::from(current))
                .into_any_element(),
        );
    }

    div()
        .flex()
        .items_center()
        .text_ellipsis()
        .children(spans)
        .into_any_element()
}

/// Compact bottom synopsis for the focused picker item.
pub(crate) fn render_compact_synopsis_strip(
    label: SharedString,
    meta: SharedString,
    description: SharedString,
    foreground: Hsla,
    muted_foreground: Hsla,
) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let footer_colors = crate::components::prompt_footer::PromptFooterColors::from_theme(&theme);
    let footer_bg = gpui::rgba(crate::components::prompt_footer::footer_surface_rgba(
        footer_colors,
    ));
    let border = gpui::rgba((footer_colors.border << 8) | 0x50);
    let hint_opacity_byte =
        (crate::window_resize::main_layout::HINT_TEXT_OPACITY * 255.0).round() as u32;
    let hint_color = gpui::rgba((theme.colors.text.primary << 8) | hint_opacity_byte);
    let meta_text = meta.clone();

    // Intrinsic height: the description text measures its own wrapped lines
    // (capped by `line_clamp`), so no chars-per-line height estimate is needed.
    div()
        .min_h(px(CONTEXT_SELECTOR_SYNOPSIS_HEIGHT))
        .flex_shrink_0()
        .overflow_hidden()
        .bg(footer_bg)
        .border_t_1()
        .border_color(border)
        .px(px(crate::window_resize::main_layout::HINT_STRIP_PADDING_X))
        .py(px(8.0))
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_xs()
                        .line_height(px(14.0))
                        .text_color(hint_color)
                        .child(label),
                )
                .when(!meta_text.is_empty(), |d| {
                    d.child(
                        div()
                            .text_xs()
                            .font_family(FONT_MONO)
                            .line_height(px(14.0))
                            .text_color(muted_foreground.opacity(COMMAND_OPACITY))
                            .child(meta_text),
                    )
                }),
        )
        .child(
            div()
                .text_xs()
                .line_height(px(CONTEXT_SELECTOR_SYNOPSIS_LINE_HEIGHT))
                .line_clamp(CONTEXT_SELECTOR_SYNOPSIS_MAX_LINES)
                .text_color(foreground.opacity(MUTED_OP))
                .child(description),
        )
        .into_any_element()
}
