// Script list render method - extracted from app_render.rs
// This file is included via include!() macro in main.rs
use crate::ui_foundation::{
    is_key_down as sk_is_key_down, is_key_enter as sk_is_key_enter,
    is_key_escape as sk_is_key_escape, is_key_tab as sk_is_key_tab, is_key_up as sk_is_key_up,
};
use gpui_component::scroll::Scrollbar as GpuiScrollbar;

// --- merged from part_000.rs ---
fn app_shell_footer_colors(theme: &crate::theme::Theme) -> PromptFooterColors {
    PromptFooterColors::from_theme(theme)
}

fn script_list_footer_info_label(
    window_tweaker_enabled: bool,
    is_dark_mode: bool,
    opacity_percent: i32,
    material: &str,
    appearance: &str,
) -> Option<String> {
    if window_tweaker_enabled && !is_dark_mode {
        Some(format!(
            "{}% | {} | {} | ⌘-/+ ⌘M ⌘⇧A",
            opacity_percent, material, appearance
        ))
    } else {
        None
    }
}

fn inline_calc_list_item_title(formatted_result: &str) -> String {
    format!("= {}", formatted_result)
}

fn inline_calc_list_copy_hint() -> &'static str {
    "↵ Copy"
}

fn inline_calc_list_item_result_text_color(
    is_selected: bool,
    design_variant: DesignVariant,
    theme: &crate::theme::Theme,
    color_resolver: crate::theme::ColorResolver,
) -> u32 {
    if is_selected && design_variant != DesignVariant::Default {
        color_resolver.primary_accent()
    } else if is_selected {
        theme.colors.accent.selected
    } else {
        color_resolver.primary_text_color()
    }
}

fn inline_calc_list_item_hint_text_color(color_resolver: crate::theme::ColorResolver) -> u32 {
    color_resolver.empty_text_color()
}

fn inline_calc_list_item_selected_overlay_rgba(
    theme: &crate::theme::Theme,
    list_tokens: crate::designs::MainMenuListTokens,
    color_resolver: crate::theme::ColorResolver,
) -> u32 {
    let selected_overlay_alpha = ((theme.get_opacity().selected.clamp(0.0, 1.0) * 255.0).round()
        as u32)
        .max(list_tokens.inline_calc_selected_overlay_min_alpha);
    (color_resolver.primary_accent() << 8) | selected_overlay_alpha
}

fn render_inline_calc_list_item(
    calculator: &crate::calculator::CalculatorInlineResult,
    is_selected: bool,
    theme: &crate::theme::Theme,
    list_tokens: crate::designs::MainMenuListTokens,
    design_variant: DesignVariant,
    color_resolver: crate::theme::ColorResolver,
) -> AnyElement {
    let tokens = get_tokens(design_variant);
    let spacing = tokens.spacing();
    let typography = tokens.typography();

    let result_title = inline_calc_list_item_title(&calculator.formatted);
    let result_text_color =
        inline_calc_list_item_result_text_color(is_selected, design_variant, theme, color_resolver);
    let hint_text_color = inline_calc_list_item_hint_text_color(color_resolver);
    let hint_alpha = if is_selected {
        list_tokens.inline_calc_selected_hint_alpha
    } else {
        list_tokens.inline_calc_hint_alpha
    };

    div()
        .w_full()
        .h_full()
        .px(px(spacing.item_padding_x))
        .py(px(spacing.padding_xs))
        .when(is_selected, |div| {
            div.bg(rgba(inline_calc_list_item_selected_overlay_rgba(
                theme,
                list_tokens,
                color_resolver,
            )))
        })
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(spacing.gap_md))
        .child(
            div()
                .flex_1()
                .overflow_x_hidden()
                .text_size(px(list_tokens.inline_calc_result_font_size))
                .font_weight(typography.font_weight_semibold)
                .text_color(rgb(result_text_color))
                .child(result_title),
        )
        .child(
            div()
                .text_size(px(list_tokens.inline_calc_hint_font_size))
                .text_color(rgba((hint_text_color << 8) | hint_alpha))
                .child(inline_calc_list_copy_hint()),
        )
        .into_any_element()
}

fn menu_syntax_hint_tone_color(
    theme: &crate::theme::Theme,
    tone: crate::menu_syntax::MenuSyntaxMainHintTone,
) -> u32 {
    match tone {
        crate::menu_syntax::MenuSyntaxMainHintTone::Neutral => theme.colors.text.secondary,
        crate::menu_syntax::MenuSyntaxMainHintTone::Accent => theme.colors.accent.selected,
        crate::menu_syntax::MenuSyntaxMainHintTone::Info => theme.colors.ui.info,
        crate::menu_syntax::MenuSyntaxMainHintTone::Warning => theme.colors.ui.warning,
        crate::menu_syntax::MenuSyntaxMainHintTone::Success => theme.colors.ui.success,
        crate::menu_syntax::MenuSyntaxMainHintTone::Muted => theme.colors.text.muted,
    }
}

fn menu_syntax_input_span_color(
    theme: &crate::theme::Theme,
    role: crate::menu_syntax::MenuSyntaxFragmentRole,
) -> u32 {
    match role {
        crate::menu_syntax::MenuSyntaxFragmentRole::Prefix
        | crate::menu_syntax::MenuSyntaxFragmentRole::Kv
        | crate::menu_syntax::MenuSyntaxFragmentRole::Tag
        | crate::menu_syntax::MenuSyntaxFragmentRole::Url => theme.colors.accent.selected,
        crate::menu_syntax::MenuSyntaxFragmentRole::ObjectRef => theme.colors.ui.success,
        crate::menu_syntax::MenuSyntaxFragmentRole::Date
        | crate::menu_syntax::MenuSyntaxFragmentRole::DateRange => theme.colors.ui.info,
        crate::menu_syntax::MenuSyntaxFragmentRole::Duration
        | crate::menu_syntax::MenuSyntaxFragmentRole::Priority => theme.colors.ui.warning,
        crate::menu_syntax::MenuSyntaxFragmentRole::Recurrence => theme.colors.ui.success,
        crate::menu_syntax::MenuSyntaxFragmentRole::Unresolved => theme.colors.text.muted,
        crate::menu_syntax::MenuSyntaxFragmentRole::Subject => theme.colors.text.primary,
    }
}

fn menu_syntax_input_span_role_name(
    role: crate::menu_syntax::MenuSyntaxFragmentRole,
) -> &'static str {
    crate::menu_syntax::input_span_role_name(role)
}

fn render_menu_syntax_hint_chip(
    theme: &crate::theme::Theme,
    list_tokens: crate::designs::MainMenuListTokens,
    chip: &crate::menu_syntax::MenuSyntaxMainHintChip,
) -> AnyElement {
    let color = menu_syntax_hint_tone_color(theme, chip.tone);
    div()
        .px(px(list_tokens.main_hint_chip_padding_x))
        .py(px(list_tokens.main_hint_chip_padding_y))
        .rounded(px(list_tokens.main_hint_chip_radius))
        .border_1()
        .border_color(rgba((color << 8) | list_tokens.main_hint_chip_border_alpha))
        .bg(rgba((color << 8) | list_tokens.main_hint_chip_bg_alpha))
        .text_size(px(list_tokens.main_hint_chip_font_size))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(color))
        .child(chip.label.clone())
        .into_any_element()
}

fn menu_syntax_single_line_text_for_gpui(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            '\n' | '\r' => ' ',
            _ => ch,
        })
        .collect()
}

fn menu_syntax_text_contains_line_break(raw: &str) -> bool {
    raw.chars().any(|ch| matches!(ch, '\n' | '\r'))
}

fn render_menu_syntax_hint_row(
    theme: &crate::theme::Theme,
    list_tokens: crate::designs::MainMenuListTokens,
    row: &crate::menu_syntax::MenuSyntaxMainHintRow,
) -> AnyElement {
    let row_value = menu_syntax_single_line_text_for_gpui(&row.value);
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(list_tokens.main_hint_row_gap))
        .child(
            div()
                .w(px(list_tokens.main_hint_row_label_width))
                .flex_shrink_0()
                .text_size(px(list_tokens.main_hint_row_label_font_size))
                .line_height(px(list_tokens.main_hint_row_label_line_height))
                .text_color(rgba(
                    (theme.colors.text.muted << 8) | list_tokens.main_hint_row_label_alpha,
                ))
                .child(row.label.clone()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_ellipsis()
                .text_size(px(list_tokens.main_hint_row_value_font_size))
                .line_height(px(list_tokens.main_hint_row_value_line_height))
                .text_color(rgba(
                    (theme.colors.text.primary << 8) | list_tokens.main_hint_row_value_alpha,
                ))
                .child(row_value),
        )
        .children(
            row.chips
                .iter()
                .map(|chip| render_menu_syntax_hint_chip(theme, list_tokens, chip)),
        )
        .into_any_element()
}

fn render_menu_syntax_fragment_preview_row(
    theme: &crate::theme::Theme,
    list_tokens: crate::designs::MainMenuListTokens,
    row: &crate::menu_syntax::MenuSyntaxFragmentPreviewRow,
) -> AnyElement {
    let color = menu_syntax_hint_tone_color(theme, row.tone);
    let preview_text =
        menu_syntax_single_line_text_for_gpui(&format!("{}: {}", row.label, row.value));
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(list_tokens.main_hint_fragment_row_gap))
        .child(
            div()
                .w(px(list_tokens.main_hint_fragment_role_width))
                .flex_shrink_0()
                .px(px(list_tokens.main_hint_fragment_role_padding_x))
                .py(px(list_tokens.main_hint_fragment_role_padding_y))
                .rounded(px(list_tokens.main_hint_fragment_role_radius))
                .border_1()
                .border_color(rgba(
                    (color << 8) | list_tokens.main_hint_fragment_role_border_alpha,
                ))
                .bg(rgba(
                    (color << 8) | list_tokens.main_hint_fragment_role_bg_alpha,
                ))
                .text_size(px(list_tokens.main_hint_fragment_role_font_size))
                .line_height(px(list_tokens.main_hint_fragment_role_line_height))
                .text_color(rgb(color))
                .child(format!("{:?}", row.role).to_ascii_lowercase()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_ellipsis()
                .text_size(px(list_tokens.main_hint_fragment_value_font_size))
                .line_height(px(list_tokens.main_hint_fragment_value_line_height))
                .text_color(rgba(
                    (theme.colors.text.primary << 8) | list_tokens.main_hint_fragment_value_alpha,
                ))
                .child(preview_text),
        )
        .children(
            row.chips
                .iter()
                .map(|chip| render_menu_syntax_hint_chip(theme, list_tokens, chip)),
        )
        .into_any_element()
}

fn render_menu_syntax_form_field(
    theme: &crate::theme::Theme,
    design_variant: DesignVariant,
    target: &str,
    field: &crate::menu_syntax::MenuSyntaxFormFieldSnapshot,
    input: Option<Entity<gpui_component::input::InputState>>,
) -> AnyElement {
    let colors = crate::components::FormFieldColors::from_theme(theme);
    let metrics =
        crate::components::FormFieldMetrics::from_theme_and_design(theme, design_variant);
    let shell_spec =
        crate::components::menu_syntax_form_field_shell_spec(target, field, metrics);
    let shell_style = crate::components::resolve_form_field_shell_style(&shell_spec, colors);

    let body = if let Some(input) = input {
        let input_element = gpui_component::input::Input::new(&input)
            .w_full()
            .line_height(px(metrics.input_line_height))
            .px(px(0.0))
            .py(px(0.0))
            .with_size(gpui_component::Size::Size(px(metrics.input_font_size)))
            .appearance(false)
            .bordered(false)
            .focus_bordered(false)
            .disabled(shell_spec.disabled);
        let input_element = if field.multiline {
            input_element
                .min_h(px(shell_spec.min_height))
                .max_h(px(shell_spec.max_height.unwrap_or(shell_spec.min_height)))
        } else {
            input_element.h(px(shell_spec.min_height))
        };
        input_element.into_any_element()
    } else {
        let has_value = !field.value.trim().is_empty();
        let display_value = if has_value {
            menu_syntax_single_line_text_for_gpui(&field.value)
        } else {
            field.placeholder.clone()
        };
        div()
            .w_full()
            .min_h(px(metrics.input_line_height))
            .flex()
            .items_center()
            .text_size(px(metrics.input_font_size))
            .line_height(px(metrics.input_line_height))
            .text_color(if has_value {
                shell_style.text
            } else {
                shell_style.placeholder
            })
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(display_value),
            )
            .into_any_element()
    };

    crate::components::render_form_field_shell(&shell_spec, colors, metrics, body)
}

fn render_menu_syntax_form(
    theme: &crate::theme::Theme,
    design_variant: DesignVariant,
    form: &crate::menu_syntax::MenuSyntaxFormSnapshot,
    inputs: &[(String, Entity<gpui_component::input::InputState>)],
    field_bounds: std::rc::Rc<std::cell::RefCell<Vec<gpui::Bounds<gpui::Pixels>>>>,
    _cx: &mut Context<ScriptListApp>,
) -> AnyElement {
    let field_metrics =
        crate::components::FormFieldMetrics::from_theme_and_design(theme, design_variant);
    div()
        .id("menu-syntax-handler-form")
        .w_full()
        .flex()
        .items_start()
        .child(
            div()
                // Record each field's real prepaint bounds so focus-driven
                // reveal scrolls to measured positions, not estimated ones.
                // (Attached before `.id()`: the listener lives on `Div`.)
                .on_children_prepainted(move |bounds, _window, _cx| {
                    *field_bounds.borrow_mut() = bounds;
                })
                .id("menu-syntax-handler-form-fields")
                .min_w(px(0.0))
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(field_metrics.field_gap_px))
                .children(form.fields.iter().map(|field| {
                    let input = inputs
                        .iter()
                        .find_map(|(id, input)| (id == &field.id).then(|| input.clone()));
                    render_menu_syntax_form_field(
                        theme,
                        design_variant,
                        &form.target,
                        field,
                        input,
                    )
                })),
        )
        .into_any_element()
}

fn render_menu_syntax_main_hint(
    hint: &crate::menu_syntax::MenuSyntaxMainHintSnapshot,
    scroll_handle: &ScrollHandle,
    theme: &crate::theme::Theme,
    list_tokens: crate::designs::MainMenuListTokens,
    design_variant: DesignVariant,
    form_inputs: &[(String, Entity<gpui_component::input::InputState>)],
    form_field_bounds: std::rc::Rc<std::cell::RefCell<Vec<gpui::Bounds<gpui::Pixels>>>>,
    cx: &mut Context<ScriptListApp>,
) -> AnyElement {
    let palette = crate::components::non_list_palette(theme);
    let metrics = crate::components::non_list_metrics(crate::components::NonListDensity::Compact);
    // AdvancedQueryGuide hints keep persistent `hint.examples` visible;
    // boundary copy includes: "Plain #tag is launcher search".
    let examples = if hint.examples.is_empty() {
        hint.example.iter().cloned().collect::<Vec<_>>()
    } else {
        hint.examples.clone()
    };

    // Edge-to-edge: the hint content fills the main-list area directly with
    // no nested card chrome (no inner border, bg, or rounded corner). The
    // OUTER div carries the layout pad/centering so content has breathing
    // room, but there's no longer a visible "card within a card" — see
    // story `hint-card-fills-main-window-no-nested-container` (Run 12).
    gpui_component::scroll::ScrollableElement::vertical_scrollbar(
        div()
            .id("menu-syntax-main-hint-scroll")
            .w_full()
            .h_full()
            .flex()
            .items_start()
            .justify_center()
            .px(px(metrics.card_padding_x))
            .pt(px(metrics.card_padding_y))
            .pb(main_list_footer_overlay_total_padding() + px(metrics.card_padding_y))
            .track_scroll(scroll_handle)
            .overflow_y_scroll(),
        scroll_handle,
    )
    .child(
        crate::components::non_list_content_stack(
            "menu-syntax-main-hint-content",
            metrics.max_width,
            metrics.block_gap,
        )
        .min_h(px(0.0))
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(metrics.item_gap))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_size(px(list_tokens.main_hint_title_font_size))
                        .line_height(px(list_tokens.main_hint_title_line_height))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.title)
                        .child(hint.title.clone()),
                )
                .child(
                    div()
                        .id("menu-syntax-main-hint-status-chip-row")
                        .flex()
                        .items_center()
                        .gap(px(list_tokens.main_hint_status_chip_gap))
                        .when_some(hint.mode_chip.as_ref(), |d, chip| {
                            d.child(render_menu_syntax_hint_chip(theme, list_tokens, chip))
                        })
                        // Multi-chip capture-validation row (Pass 22 added the data,
                        // Pass 23 wires the rendering): when `status_chips` is non-empty
                        // the snapshot has already inserted the mode chip at index 0 plus
                        // per-missing-field chips. Skip the mode_chip rendered above to
                        // avoid duplication, and render the rest.
                        .when(!hint.status_chips.is_empty(), |d| {
                            let skip = if hint.mode_chip.is_some() { 1 } else { 0 };
                            d.children(
                                hint.status_chips.iter().skip(skip).map(|chip| {
                                    render_menu_syntax_hint_chip(theme, list_tokens, chip)
                                }),
                            )
                        })
                        // Single-chip status (legacy non-capture path) — only render
                        // when the multi-chip path didn't already populate. Prevents
                        // double-chips on capture composer surfaces.
                        .when_some(
                            hint.status_chip
                                .as_ref()
                                .filter(|_| hint.status_chips.is_empty()),
                            |d, chip| {
                                d.child(render_menu_syntax_hint_chip(theme, list_tokens, chip))
                            },
                        ),
                ),
        )
        .when_some(hint.subtitle.as_ref(), |d, subtitle| {
            d.child(
                div()
                    .text_size(px(list_tokens.main_hint_body_font_size))
                    .line_height(px(list_tokens.main_hint_body_line_height))
                    .text_color(palette.body)
                    .child(subtitle.clone()),
            )
        })
        .when_some(hint.form.as_ref(), |d, form| {
            d.child(render_menu_syntax_form(
                theme,
                design_variant,
                form,
                form_inputs,
                form_field_bounds,
                cx,
            ))
        })
        .when(!hint.rows.is_empty(), |d| {
            d.child(
                div()
                    .id("menu-syntax-main-hint-rows")
                    .flex()
                    .flex_col()
                    .gap(px(list_tokens.main_hint_rows_gap))
                    .children(
                        hint.rows
                            .iter()
                            .map(|row| render_menu_syntax_hint_row(theme, list_tokens, row)),
                    ),
            )
        })
        .when_some(hint.fragment_preview.as_ref(), |d, preview| {
            d.when(!preview.rows.is_empty(), |d| {
                d.child(
                    div()
                        .id("menu-syntax-main-hint-fragment-rows")
                        .flex()
                        .flex_col()
                        .gap(px(list_tokens.main_hint_fragment_rows_gap))
                        .children(preview.rows.iter().map(|row| {
                            render_menu_syntax_fragment_preview_row(theme, list_tokens, row)
                        })),
                )
            })
        })
        .when_some(hint.warning.as_ref(), |d, warning| {
            d.child(
                crate::components::non_list_card("menu-syntax-main-hint-warning", palette, metrics)
                    .border_color(rgba(
                        (theme.colors.ui.warning << 8) | list_tokens.main_hint_warning_border_alpha,
                    ))
                    .bg(rgba(
                        (theme.colors.ui.warning << 8) | list_tokens.main_hint_warning_bg_alpha,
                    ))
                    .text_size(px(list_tokens.main_hint_body_font_size))
                    .line_height(px(list_tokens.main_hint_body_line_height))
                    .text_color(rgb(theme.colors.ui.warning))
                    .child(warning.clone()),
            )
        })
        .child(
            div()
                .id("menu-syntax-main-hint-divider")
                .h(px(list_tokens.main_hint_divider_height))
                .w_full()
                .bg(palette.border),
        )
        .when_some(hint.primary_hint.as_ref(), |d, primary| {
            d.child(
                div()
                    .text_size(px(list_tokens.main_hint_body_font_size))
                    .line_height(px(list_tokens.main_hint_body_line_height))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.accent)
                    .child(primary.clone()),
            )
        })
        .when_some(hint.secondary_hint.as_ref(), |d, secondary| {
            d.child(
                div()
                    .text_size(px(list_tokens.main_hint_body_font_size))
                    .line_height(px(list_tokens.main_hint_body_line_height))
                    .text_color(palette.hint)
                    .child(secondary.clone()),
            )
        })
        .when(!examples.is_empty(), |d| {
            d.child(
                div()
                    .id("menu-syntax-main-hint-examples-group")
                    .flex()
                    .flex_col()
                    .gap(px(list_tokens.main_hint_examples_group_gap))
                    .child(
                        div()
                            .text_size(px(list_tokens.main_hint_example_label_font_size))
                            .line_height(px(list_tokens.main_hint_example_label_line_height))
                            .text_color(palette.hint)
                            .child(if examples.len() == 1 {
                                "Example"
                            } else {
                                "Examples"
                            }),
                    )
                    .child(
                        crate::components::non_list_card(
                            "menu-syntax-main-hint-examples",
                            palette,
                            metrics,
                        )
                        .flex()
                        .flex_col()
                        .gap(px(list_tokens.main_hint_example_row_gap))
                        .text_size(px(list_tokens.main_hint_body_font_size))
                        .line_height(px(list_tokens.main_hint_body_line_height))
                        .font_family("JetBrains Mono")
                        .text_color(palette.body)
                        .children(examples.iter().map(|example| {
                            div()
                                .w_full()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(example.clone())
                        })),
                    ),
            )
        }),
    )
    .into_any_element()
}

fn render_script_list_empty_state(
    filter_text_for_render: &str,
    has_active_filter: bool,
    theme: &crate::theme::Theme,
    cx: &gpui::App,
) -> AnyElement {
    crate::components::render_launcher_empty_or_no_results(
        filter_text_for_render,
        has_active_filter,
        theme,
        cx,
    )
}

fn spine_projection_icon_kind(
    row: &crate::spine::SpineListRow,
) -> Option<crate::list_item::IconKind> {
    let (_, fallback_icon) = row.kind.type_accessory_info();
    row.icon
        .as_ref()
        .and_then(|icon: &gpui::SharedString| {
            crate::list_item::IconKind::from_icon_hint(icon.as_ref())
                .or_else(|| Some(crate::list_item::IconKind::Svg(icon.as_ref().to_string())))
        })
        .or_else(|| crate::list_item::IconKind::from_icon_hint(fallback_icon))
        .or_else(|| Some(crate::list_item::IconKind::Svg(fallback_icon.to_string())))
}

fn render_spine_projection_row(
    row: &crate::spine::SpineListRow,
    ix: usize,
    is_selected: bool,
    is_hovered: bool,
    colors: ListItemColors,
    main_menu_theme: crate::designs::MainMenuThemeVariant,
) -> AnyElement {
    let (type_label, type_icon) = row.kind.type_accessory_info();

    crate::list_item::ListItem::new(row.title.to_string(), colors)
        .index(ix)
        .selected(is_selected)
        .hovered(is_hovered)
        .main_menu_theme(main_menu_theme)
        .semantic_id(row.id.to_string())
        .description_opt(
            row.subtitle
                .as_ref()
                .map(|subtitle: &gpui::SharedString| subtitle.to_string()),
        )
        .icon_kind_opt(spine_projection_icon_kind(row))
        .type_accessory(crate::list_item::TypeAccessory {
            label: type_label,
            icon_name: type_icon,
        })
        .source_hint_opt(
            row.meta
                .as_ref()
                .map(|meta: &gpui::SharedString| meta.to_string()),
        )
        .into_any_element()
}

fn selected_index_for_script_list_render(
    grouped_items: &[GroupedListItem],
    selected_index: usize,
    selection_suppressed: bool,
) -> usize {
    if selection_suppressed {
        usize::MAX
    } else {
        crate::list_item::coerce_selection(grouped_items, selected_index).unwrap_or(0)
    }
}

impl ScriptListApp {
    /// Main-menu Up/Down with the A8 input-history contract (2026-06-09):
    /// recall only ENTERS from an empty input at the top of the list, but once
    /// in history, Up keeps walking older entries and Down past the newest
    /// clears back to the live filter.
    ///
    /// This is the SimulateKey-parity twin of the real-key handlers in
    /// `app_impl/startup.rs` (legacy key path) and
    /// `app_impl/startup_new_arrow.rs` (arrow interceptor). Battery 13 found
    /// the protocol dispatch (`simulate_key_dispatch.rs`) called bare
    /// move_selection_up/down, making history recall unreachable from
    /// automation — a story the probe fleet could never see.
    pub(crate) fn main_menu_arrow_key_with_history(
        &mut self,
        is_up: bool,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        // Key-repeat coalescing: a recalled filter that has not rendered yet
        // means further history steps must wait (same guard as the real-key
        // path).
        if self.history_filter_render_pending.is_some() {
            tracing::info!(
                target: "script_kit::input_history",
                event = "history_key_repeat_coalesced_until_render_simulate",
                is_up,
            );
            return;
        }

        let source_filter_mode = self.source_filter_mode_blocks_input_history_recall();
        let filter_has_text = !self.filter_text.is_empty() || !self.computed_filter_text.is_empty();
        let (grouped_items, _) = self.get_grouped_results_cached();
        let first_item_position = grouped_items
            .iter()
            .position(|item| matches!(item, crate::list_item::GroupedListItem::Item(_)));
        let at_top_of_list = first_item_position
            .map(|position| self.selected_index <= position)
            .unwrap_or(true);
        let in_history = self.input_history.current_index().is_some();

        if is_up {
            // A8: a filled input at the top of the list is a no-op — Up must
            // not clobber typed text with history.
            if !source_filter_mode && filter_has_text && at_top_of_list && !in_history {
                return;
            }
            if !source_filter_mode && (in_history || at_top_of_list) {
                if let Some(text) = self.input_history.navigate_up() {
                    tracing::info!(
                        target: "script_kit::input_history",
                        event = "history_recalled",
                        direction = "up",
                        source = "simulate_key",
                        recalled_len = text.len(),
                        history_index = ?self.input_history.current_index(),
                    );
                    self.history_filter_render_pending = Some(text.clone());
                    self.set_filter_text_immediate(text, window, cx);
                }
                return;
            }
            self.move_selection_up(cx);
        } else {
            if !source_filter_mode && in_history {
                if let Some(text) = self.input_history.navigate_down() {
                    tracing::info!(
                        target: "script_kit::input_history",
                        event = "history_recalled",
                        direction = "down",
                        source = "simulate_key",
                        recalled_len = text.len(),
                        history_index = ?self.input_history.current_index(),
                    );
                    self.history_filter_render_pending = Some(text.clone());
                    self.set_filter_text_immediate(text, window, cx);
                } else {
                    self.input_history.reset_navigation();
                    self.clear_filter(window, cx);
                }
                return;
            }
            self.move_selection_down(cx);
        }
    }

    fn should_preempt_empty_script_list_escape_close(
        &self,
        event: &gpui::KeyDownEvent,
        cx: &gpui::App,
    ) -> bool {
        if !sk_is_key_escape(event.keystroke.key.as_str()) {
            return false;
        }

        let modifiers = &event.keystroke.modifiers;
        if modifiers.platform || modifiers.shift || modifiers.alt || modifiers.control {
            return false;
        }

        if !self.can_preserve_hide_script_list_on_passive_focus_loss()
            || self.is_in_attachment_portal()
            || self.opened_from_main_menu
            || self.show_actions_popup
            || crate::actions::is_actions_window_open()
            || self.menu_syntax_object_selector_owns_main_keyboard()
            || self.menu_syntax_trigger_picker_owns_main_keyboard()
            || self.script_list_escape_should_clear_visible_filter(cx)
        {
            return false;
        }

        if !self.action_shortcuts.is_empty() {
            let key_combo = shortcuts::keystroke_to_shortcut(
                event.keystroke.key.as_str(),
                &event.keystroke.modifiers,
            );
            if self.action_shortcuts.contains_key(&key_combo) {
                return false;
            }
        }

        true
    }

    fn preempt_empty_script_list_escape_close(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.should_preempt_empty_script_list_escape_close(event, cx) {
            return false;
        }

        // The focused gpui_component input turns any keydown into a blink pause,
        // which forces the caret visible and schedules a repaint. For the empty
        // launcher Escape-close case, handle Escape in the capture phase so the
        // input never sees the key and cannot draw a last visible caret frame
        // before the safe deferred native hide runs.
        logging::log(
            "KEY",
            "Capture Escape - close empty ScriptList before input cursor blink pause",
        );
        self.clear_hidden_script_list_filter_before_escape_close(window, cx);
        self.close_and_reset_window(cx);
        true
    }

    fn render_script_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let render_list_start = std::time::Instant::now();
        let filter_for_log = self.filter_text.clone();
        let is_mini = self.main_window_mode == MainWindowMode::Mini;

        // Get grouped or flat results based on filter state (cached) - MUST come first
        // to avoid borrow conflicts with theme access below
        // When filter is empty, use frecency-grouped results with RECENT/MAIN sections
        // When filtering, use flat fuzzy search results
        let (grouped_items, flat_results) = self.get_grouped_results_cached();
        let get_results_elapsed = render_list_start.elapsed();

        // Deduplicate render logs: only log when meaningful state changes (not cursor blink)
        // This reduces log spam from ~2 logs/sec (cursor blink) to only on actual changes
        let state_changed = self.filter_text
            != self.main_menu_render_diagnostics.last_render_log_filter
            || self.selected_index != self.main_menu_render_diagnostics.last_render_log_selection
            || grouped_items.len() != self.main_menu_render_diagnostics.last_render_log_item_count;

        // Set flag for render_preview_panel to check (called later in this render)
        self.main_menu_render_diagnostics.log_this_render = state_changed;
        // Capture item count for deferred state update
        let item_count_for_log = grouped_items.len();

        if state_changed && logging::filter_perf_trace_enabled() {
            logging::log(
                "RENDER_PERF",
                &format!(
                    "[RENDER_SCRIPT_LIST_START] filter='{}' computed_filter='{}' selected_idx={}",
                    filter_for_log, self.computed_filter_text, self.selected_index
                ),
            );
            logging::log(
                "RENDER_PERF",
                &format!(
                    "[RENDER_GET_RESULTS] filter='{}' items={} results={} took={:.2}ms",
                    filter_for_log,
                    grouped_items.len(),
                    flat_results.len(),
                    get_results_elapsed.as_secs_f64() * 1000.0
                ),
            );
        }

        // NOTE: Removed per-frame logging here - was causing 6 log calls per frame
        // which includes mutex locks and file I/O. Log only on cache MISS in get_grouped_results_cached.
        // Clone for use in closures and to avoid borrow issues
        let grouped_items = grouped_items.clone();
        let flat_results = flat_results.clone();

        // Render with a pure, valid selectable snapshot so a newly replaced
        // list cannot paint its first visible item as unfocused for one frame
        // before event-side selection validation catches up.
        let spine_selection_render_index = selected_index_for_script_list_render(
            &grouped_items,
            self.selected_index,
            self.spine_empty_subsearch_selection_suppressed(),
        );

        // --- Storybook live-spec override (read-only, no state mutation) ---
        // grouped_items / flat_results are Arc<[T]> from cache; when the storybook
        // spec requires overrides we swap them for fresh Vecs wrapped in Arc.
        #[cfg(feature = "storybook")]
        let (grouped_items, flat_results, selected_index_for_render, filter_text_for_render) = {
            let live_spec = crate::storybook::adopted_main_menu_live_spec();
            let mut si = spine_selection_render_index;
            let mut ft = self.filter_text.clone();
            let gi;
            let fr;
            if live_spec.force_empty_results {
                gi = std::sync::Arc::<[GroupedListItem]>::from(Vec::new());
                fr = std::sync::Arc::<[scripts::SearchResult]>::from(Vec::new());
                ft = live_spec
                    .filter_text_override
                    .unwrap_or("storybook-empty")
                    .to_string();
                si = 0;
            } else {
                if live_spec.prefer_first_result_selected {
                    if let Some(ix) = grouped_items
                        .iter()
                        .position(|item| matches!(item, GroupedListItem::Item(_)))
                    {
                        si = ix;
                    }
                }
                gi = grouped_items;
                fr = flat_results;
            }
            (gi, fr, si, ft)
        };
        #[cfg(not(feature = "storybook"))]
        let (selected_index_for_render, filter_text_for_render) =
            (spine_selection_render_index, self.filter_text.clone());

        // Get design tokens for current design variant
        let tokens = get_tokens(self.current_design);
        let design_visual = tokens.visual();

        // Theme exploration is row-level (threaded into the list renderer via
        // current_main_menu_theme); no header/footer chrome treatments here.

        // Unified color and spacing resolution
        // Shell uses theme-first so non-default design variants keep the active
        // theme's colors while still using the variant's spacing and shape tokens.
        let color_resolver =
            crate::theme::ColorResolver::new_for_shell(&self.theme, self.current_design);

        // For Default design, use header constants; for others, use design spacing
        let is_default_design = self.current_design.is_default();
        let design_spacing = tokens.spacing();

        let item_count = grouped_items.len();
        let _total_len = self.scripts.len() + self.scriptlets.len();

        // ============================================================
        // RENDER IS READ-ONLY
        // ============================================================
        // NOTE: State mutations (selection validation, list sync) are now done
        // in event handlers via sync_list_state() and validate_selection_bounds(),
        // not during render. This prevents the anti-pattern of mutating state
        // during render which can cause infinite render loops and inconsistent UI.
        //
        // Event handlers that call these methods:
        // - queue_filter_compute() - after filter text changes
        // - set_filter_text_immediate() - for immediate filter updates
        // - refresh_scripts() - after script reload
        // - reset_to_script_list() - on view transitions

        // ============================================================
        // IMMUTABLE BORROWS BLOCK - extract theme values for UI building
        // ============================================================

        // Extract theme values as owned copies for UI building
        let log_panel_bg = self.theme.colors.background.log_panel;
        let log_panel_border = self.theme.colors.ui.border;
        let log_panel_success = self.theme.colors.ui.success;

        // Pre-compute list item colors for closure (Copy type)
        let theme_colors = ListItemColors::from_theme(&self.theme);

        // NOTE: Removed P4 perf log - called every render frame, causing log spam

        // Build script list using uniform_list for proper virtualized scrolling.
        let handler_form_owns_input_for_render =
            self.menu_syntax_capture_form_owns_input_for(&filter_text_for_render);
        // The three-zone launcher keeps Agent discovery in the footer action zone.
        let show_launcher_ask_ai_hint = false;
        let capture_composer_owns_main_list = self
            .menu_syntax_mode
            .capture_composer_owns_input_for(&filter_text_for_render)
            || handler_form_owns_input_for_render;
        let popup_owns_main_list = !handler_form_owns_input_for_render
            && (self.menu_syntax_object_selector_state.owns_main_list()
                || self.menu_syntax_trigger_picker_state.owns_main_list());
        let trigger_picker_owns_main_list_for_render = !handler_form_owns_input_for_render
            && self.menu_syntax_trigger_picker_state.owns_main_list();
        let object_selector_owns_main_list_for_render = !handler_form_owns_input_for_render
            && self.menu_syntax_object_selector_state.owns_main_list();
        let active_filter_head_owns_main_list_for_render = !handler_form_owns_input_for_render
            && crate::menu_syntax::active_filter_head_owns_main_list(&filter_text_for_render);
        let spine_owns_main_list_for_render = self.spine_projection_owns_main_list()
            && self.spine_parse.input == filter_text_for_render;
        let menu_syntax_owns_main_list = popup_owns_main_list
            || (!spine_owns_main_list_for_render
                && (capture_composer_owns_main_list
                    || self
                        .menu_syntax_mode
                        .command_owns_input_for(&filter_text_for_render)));
        // Keep guide cards available for bare/partial `:` refine entry, but
        // let completed advanced queries render their filtered results first.
        // Empty-state hint cards are handled by the item-count branch below.
        let advanced_query_guide_hint = (!menu_syntax_owns_main_list
            && !active_filter_head_owns_main_list_for_render)
            .then(|| self.menu_syntax_main_hint_snapshot(&filter_text_for_render, true))
            .flatten()
            .filter(|hint| {
                matches!(
                    hint.kind,
                    crate::menu_syntax::MenuSyntaxMainHintKind::AdvancedQueryGuide
                )
            });

        let main_hint_list_tokens = self.current_main_menu_theme.def().list;
        let active_filter_empty_copy = "There are no search results with this filter applied.";
        let list_element: AnyElement = if menu_syntax_owns_main_list
            && !trigger_picker_owns_main_list_for_render
            && !object_selector_owns_main_list_for_render
        {
            self.menu_syntax_main_hint_snapshot(&filter_text_for_render, false)
                .map(|hint| {
                    render_menu_syntax_main_hint(
                        &hint,
                        &self.menu_syntax_main_hint_scroll_handle,
                        &self.theme,
                        main_hint_list_tokens,
                        self.current_design,
                        &self.menu_syntax_form_inputs,
                        self.menu_syntax_form_field_bounds.clone(),
                        cx,
                    )
                })
                .unwrap_or_else(|| div().w_full().h_full().into_any_element())
        } else if active_filter_head_owns_main_list_for_render
            && !trigger_picker_owns_main_list_for_render
            && !object_selector_owns_main_list_for_render
        {
            self.menu_syntax_main_hint_snapshot(&filter_text_for_render, true)
                .map(|hint| {
                    render_menu_syntax_main_hint(
                        &hint,
                        &self.menu_syntax_main_hint_scroll_handle,
                        &self.theme,
                        main_hint_list_tokens,
                        self.current_design,
                        &self.menu_syntax_form_inputs,
                        self.menu_syntax_form_field_bounds.clone(),
                        cx,
                    )
                })
                .unwrap_or_else(|| div().w_full().h_full().into_any_element())
        } else if let Some(hint) = advanced_query_guide_hint {
            render_menu_syntax_main_hint(
                &hint,
                &self.menu_syntax_main_hint_scroll_handle,
                &self.theme,
                main_hint_list_tokens,
                self.current_design,
                &self.menu_syntax_form_inputs,
                self.menu_syntax_form_field_bounds.clone(),
                cx,
            )
        } else if item_count == 0 {
            let _ = active_filter_empty_copy;
            if let Some(hint) = self.menu_syntax_main_hint_snapshot(&filter_text_for_render, true) {
                render_menu_syntax_main_hint(
                    &hint,
                    &self.menu_syntax_main_hint_scroll_handle,
                    &self.theme,
                    main_hint_list_tokens,
                    self.current_design,
                    &self.menu_syntax_form_inputs,
                    self.menu_syntax_form_field_bounds.clone(),
                    cx,
                )
            } else {
                let has_active_filter = self
                    .menu_syntax_mode
                    .advanced_query_for(&filter_text_for_render)
                    .is_some_and(|query| query.has_source_filters() || query.has_predicates());
                render_script_list_empty_state(
                    &filter_text_for_render,
                    has_active_filter,
                    &self.theme,
                    cx,
                )
            }
        } else {
            // Use GPUI's list() component for variable-height items
            // Section headers render at 32px, regular items at 40px
            // This gives true visual compression for headers without the uniform_list hack

            // Clone grouped_items and flat_results for the closure
            let grouped_items_clone = grouped_items.clone();
            let flat_results_clone = flat_results.clone();

            let current_main_menu_theme = self.current_main_menu_theme;
            let hide_initial_section_header = current_main_menu_theme
                .def()
                .header_info_bar
                .hide_initial_section_header;
            let effective_section_header_height =
                crate::list_item::effective_section_header_height_for_theme(
                    current_main_menu_theme,
                );
            let effective_list_item_height =
                crate::list_item::effective_list_item_height_for_theme(current_main_menu_theme);

            // Capture entity handle for use in the render closure
            let entity = cx.entity();

            // theme_colors was pre-computed above to avoid borrow conflicts
            let current_design = self.current_design;
            // Track filter for closure logging and highlighting
            let filter_for_closure = filter_text_for_render.clone();
            let filter_for_highlight = filter_text_for_render.clone();

            // Capture selected index for render (may be overridden by storybook live spec)
            let selected_for_list_closure = selected_index_for_render;
            let footer_padding = main_list_footer_overlay_total_padding();
            let row_generation = self.main_list_row_generation;

            let header_overlay_height =
                main_list_header_overlay_height(current_main_menu_theme.def());
            let variable_height_list =
                list(self.main_list_state.clone(), move |ix, _window, cx| {
                    let _item_render_start = std::time::Instant::now();

                    // Access entity state inside the closure
                    entity.update(cx, |this, cx| {
                        let current_selected = selected_for_list_closure;
                        let current_hovered = this.hovered_index;

                        if let Some(grouped_item) = grouped_items_clone.get(ix) {
                            match grouped_item {
                                GroupedListItem::SectionHeader(label, icon) => {
                                    // Section header at 32px height (8px grid) for clear visual separation,
                                    // or 20px if it is the first section header to pull it up closer to input.
                                    let is_first = ix == 0;
                                    let h_px = if is_first && hide_initial_section_header {
                                        0.0
                                    } else if is_first {
                                        crate::list_item::effective_first_section_header_height_for_theme(
                                            current_main_menu_theme,
                                        )
                                    } else {
                                        effective_section_header_height
                                    };
                                    div()
                                        .id(ElementId::NamedInteger(
                                            format!("section-header-gen-{row_generation}").into(),
                                            ix as u64,
                                        ))
                                        .h(px(h_px))
                                        .when(!hide_initial_section_header || !is_first, |div| {
                                            div.child(render_section_header(
                                                label,
                                                icon.as_deref(),
                                                theme_colors,
                                                is_first,
                                            ))
                                        })
                                        .into_any_element()
                                }
                                GroupedListItem::ReservedSectionSlot => div()
                                    .id(ElementId::NamedInteger(
                                        format!("section-slot-reserved-gen-{row_generation}").into(),
                                        ix as u64,
                                    ))
                                    .h(px(
                                        crate::list_item::effective_first_section_header_height_for_theme(
                                            current_main_menu_theme,
                                        ),
                                    ))
                                    .into_any_element(),
                                GroupedListItem::Status(status) => {
                                    div()
                                        .id(ElementId::NamedInteger(
                                            format!("source-status-gen-{row_generation}").into(),
                                            ix as u64,
                                        ))
                                        .h(px(crate::list_item::effective_source_status_row_height_for_theme(
                                            current_main_menu_theme,
                                        )))
                                        .px_4()
                                        .flex()
                                        .items_center()
                                        .text_sm()
                                        .text_color(rgb(theme_colors.text_secondary))
                                        .child(status.label.clone())
                                        .into_any_element()
                                }
                                GroupedListItem::Item(result_idx) => {
                                    // Regular item at 40px height (LIST_ITEM_HEIGHT)
                                    let is_selected = ix == current_selected;
                                    // Hover gating is now handled by ListItem via GPUI input modality
                                    let is_hovered = current_hovered == Some(ix);

                                    let mouse_move_handler = cx.listener(
                                        move |this: &mut ScriptListApp,
                                              _event: &gpui::MouseMoveEvent,
                                              _window,
                                              cx| {
                                            this.note_list_pointer_move(ix, cx);
                                        },
                                    );
                                    let hover_handler = cx.listener(
                                        move |this: &mut ScriptListApp,
                                              hovered: &bool,
                                              _window,
                                              cx| {
                                            if !*hovered {
                                                this.note_list_pointer_leave(ix, cx);
                                            }
                                        },
                                    );
                                    let mouse_down_handler = cx.listener(
                                        move |this: &mut ScriptListApp,
                                              event: &gpui::MouseDownEvent,
                                              window,
                                              cx| {
                                            let trigger_owns = this
                                                .menu_syntax_trigger_picker_owns_main_keyboard();
                                            let row_id = this
                                                .menu_syntax_trigger_row_id_from_main_list_index(ix);
                                            logging::log(
                                                "UI",
                                                &format!(
                                                    "main_list_row_mouse_down_seen ix={} x={:.1} y={:.1} trigger_owns={} row_id={:?} filter='{}' computed='{}' selected_index={}",
                                                    ix,
                                                    event.position.x.as_f32(),
                                                    event.position.y.as_f32(),
                                                    trigger_owns,
                                                    row_id,
                                                    this.filter_text,
                                                    this.computed_filter_text,
                                                    this.selected_index
                                                ),
                                            );
                                            if this.menu_syntax_trigger_picker_owns_main_keyboard()
                                            {
                                                if let Some(row_id) = row_id {
                                                    logging::log(
                                                        "UI",
                                                        &format!(
                                                            "Menu-syntax trigger picker row mouse-down accepting row_id={} item={}",
                                                            row_id, ix
                                                        ),
                                                    );
                                                    this.accept_menu_syntax_trigger_picker_row(
                                                        &row_id,
                                                        Some(window),
                                                        cx,
                                                    );
                                                    this.menu_syntax_trigger_picker_suppress_next_launcher_click = true;
                                                    cx.stop_propagation();
                                                }
                                            }
                                        },
                                    );

                                    // Create click handler matching launcher click semantics
                                    let click_handler = cx.listener(
                                        move |this: &mut ScriptListApp,
                                              event: &gpui::ClickEvent,
                                              window,
                                              cx| {
                                            let click_count = event.click_count();
                                            this.note_list_pointer_click(ix, cx);
                                            if this.menu_syntax_trigger_picker_suppress_next_launcher_click {
                                                logging::log(
                                                    "UI",
                                                    &format!(
                                                        "main_list_row_click_seen ix={} click_count={} suppress_next=true was_selected=false trigger_owns={} filter='{}' computed='{}' selected_index={}",
                                                        ix,
                                                        click_count,
                                                        this.menu_syntax_trigger_picker_owns_main_keyboard(),
                                                        this.filter_text,
                                                        this.computed_filter_text,
                                                        this.selected_index
                                                    ),
                                                );
                                                logging::log(
                                                    "UI",
                                                    &format!(
                                                        "Menu-syntax trigger picker consumed trailing launcher click item={}",
                                                        ix
                                                    ),
                                                );
                                                this.menu_syntax_trigger_picker_suppress_next_launcher_click = false;
                                                cx.stop_propagation();
                                                return;
                                            }
                                            // While the empty colon mode renders unarmed, no row
                                            // reads as "already selected", so the first click
                                            // arms + selects instead of submitting the internal
                                            // selection the user never saw.
                                            let was_selected = this.selected_index == ix
                                                && !this.spine_empty_subsearch_selection_suppressed();
                                            logging::log(
                                                "UI",
                                                &format!(
                                                    "main_list_row_click_seen ix={} click_count={} suppress_next=false was_selected={} trigger_owns={} filter='{}' computed='{}' selected_index={}",
                                                    ix,
                                                    click_count,
                                                    was_selected,
                                                    this.menu_syntax_trigger_picker_owns_main_keyboard(),
                                                    this.filter_text,
                                                    this.computed_filter_text,
                                                    this.selected_index
                                                ),
                                            );
                                            this.arm_spine_empty_subsearch_selection();
                                            this.mark_main_menu_selection_user_moved();
                                            // Always select the item on any click
                                            if !was_selected {
                                                this.clear_menu_syntax_filter_accept_hint();
                                                this.selected_index = ix;
                                                cx.notify();
                                            }

                                            if crate::ui_foundation::should_submit_selected_row_click(
                                                was_selected,
                                                click_count,
                                            ) {
                                                if this.spine_projection_owns_main_list() {
                                                    this.accept_spine_projection_row(window, cx);
                                                } else if this
                                                    .menu_syntax_object_selector_owns_main_keyboard()
                                                {
                                                    if let Some(row_id) = this
                                                        .selected_menu_syntax_object_selector_row_id_from_main_list()
                                                    {
                                                        this.accept_menu_syntax_object_selector_row(
                                                            &row_id,
                                                            Some(window),
                                                            cx,
                                                        );
                                                    }
                                                } else if this
                                                    .menu_syntax_trigger_picker_owns_main_keyboard()
                                                {
                                                    if let Some(row_id) = this
                                                        .selected_menu_syntax_trigger_row_id_from_main_list()
                                                    {
                                                        logging::log(
                                                            "UI",
                                                            &format!(
                                                                "Menu-syntax trigger picker row click accepting row_id={} item={} click_count={}",
                                                                row_id, ix, click_count
                                                            ),
                                                        );
                                                        this.accept_menu_syntax_trigger_picker_row(
                                                            &row_id,
                                                            Some(window),
                                                            cx,
                                                        );
                                                        cx.stop_propagation();
                                                    }
                                                } else {
                                                    logging::log(
                                                        "UI",
                                                        &format!(
                                                            "Launcher row click submitting item {} (click_count={})",
                                                            ix, click_count
                                                        ),
                                                    );
                                                    this.execute_selected(cx);
                                                }
                                            }
                                        },
                                    );

                                    // Dispatch to design-specific item renderer
                                    // Note: Confirmation for dangerous builtins is now handled
                                    // via modal dialog, not inline overlay
                                    let design_render_start = std::time::Instant::now();
                                    let inline_calculator =
                                        this.inline_calculator_for_result_index(*result_idx);
                                    let mut item_name = "inline-calculator";
                                    let item_element = if let Some(calculator) = inline_calculator
                                    {
                                        let _legacy_calculator_renderer = render_calculator_item;
                                        render_inline_calc_list_item(
                                            calculator,
                                            is_selected,
                                            &this.theme,
                                            this.current_main_menu_theme.def().list,
                                            this.current_design,
                                            color_resolver,
                                        )
                                    } else if let Some(result) = flat_results_clone.get(*result_idx)
                                    {
                                        item_name = result.name();
                                        if let crate::scripts::SearchResult::SpineProjection(row) =
                                            result
                                        {
                                            render_spine_projection_row(
                                                row,
                                                ix,
                                                is_selected,
                                                is_hovered,
                                                theme_colors,
                                                current_main_menu_theme,
                                            )
                                        } else {
                                            render_design_item(
                                                current_design,
                                                current_main_menu_theme,
                                                result,
                                                ix,
                                                is_selected,
                                                is_hovered,
                                                theme_colors,
                                                &filter_for_highlight,
                                            )
                                        }
                                    } else {
                                        item_name = "<missing-result>";
                                        div().h(px(effective_list_item_height)).into_any_element()
                                    };
                                    let design_elapsed = design_render_start.elapsed();

                                    // Log slow items (>1ms)
                                    if design_elapsed.as_micros() > 1000 {
                                        logging::log(
                                            "FILTER_PERF",
                                            &format!(
                                                "[SLOW_ITEM] ix={} name='{}' design_render={:.2}ms filter='{}'",
                                                ix,
                                                item_name,
                                                design_elapsed.as_secs_f64() * 1000.0,
                                                filter_for_closure
                                            ),
                                        );
                                    }

                                    div()
                                        .id(ElementId::NamedInteger(
                                            format!("script-item-gen-{row_generation}").into(),
                                            ix as u64,
                                        ))
                                        .h(px(effective_list_item_height))
                                        .on_mouse_move(mouse_move_handler)
                                        .on_mouse_down(gpui::MouseButton::Left, mouse_down_handler)
                                        .on_hover(hover_handler)
                                        .on_click(click_handler)
                                        .child(item_element)
                                        .into_any_element()
                                }
                            }
                        } else {
                            // Fallback for out-of-bounds index
                            div().h(px(effective_list_item_height)).into_any_element()
                        }
                    })
                })
                // Enable proper scroll handling for mouse wheel/trackpad
                // ListSizingBehavior::Infer sets overflow.y = Overflow::Scroll internally
                // which is required for the list's hitbox to capture scroll wheel events
                .with_sizing_behavior(ListSizingBehavior::Infer)
                .h_full()
                .pt(header_overlay_height)
                .pb(footer_padding);

            // Native GPUI ListState owns wheel, line, and momentum scrolling.
            // The surrounding pane observes the post-native viewport only.
            let scrollbar_overlay = {
                let footer_overlay_height = main_list_footer_overlay_total_padding();
                let viewport_height = self.main_list_state.viewport_bounds().size.height;
                let safe_viewport_height =
                    (viewport_height - header_overlay_height - footer_overlay_height).max(px(0.0));
                // Resolve the per-kind heights once: `effective_*_for_theme`
                // rebuilds the full metrics override struct per call, which is
                // too expensive to repeat per item on every render.
                let hide_initial_section_header = current_main_menu_theme
                    .def()
                    .header_info_bar
                    .hide_initial_section_header;
                let first_section_header_height =
                    crate::list_item::effective_first_section_header_height_for_theme(
                        current_main_menu_theme,
                    );
                let section_header_height =
                    crate::list_item::effective_section_header_height_for_theme(
                        current_main_menu_theme,
                    );
                let status_row_height =
                    crate::list_item::effective_source_status_row_height_for_theme(
                        current_main_menu_theme,
                    );
                let list_item_height =
                    crate::list_item::effective_list_item_height_for_theme(current_main_menu_theme);
                let content_height = px(grouped_items
                    .iter()
                    .enumerate()
                    .map(|(ix, item)| match item {
                        GroupedListItem::SectionHeader(..) => {
                            if ix == 0 && hide_initial_section_header {
                                0.0
                            } else if ix == 0 {
                                first_section_header_height
                            } else {
                                section_header_height
                            }
                        }
                        GroupedListItem::ReservedSectionSlot => first_section_header_height,
                        GroupedListItem::Status(..) => status_row_height,
                        GroupedListItem::Item(..) => list_item_height,
                    })
                    .sum::<f32>());

                div()
                    .absolute()
                    .top(header_overlay_height)
                    .right_0()
                    .h(safe_viewport_height)
                    .w(px(self.current_main_menu_theme.def().list.scrollbar_width))
                    .capture_any_mouse_down(cx.listener(
                        |this, event: &gpui::MouseDownEvent, _window, cx| {
                            if event.button != gpui::MouseButton::Left {
                                return;
                            }
                            this.observe_main_list_scrollbar_pointer_down(cx);
                        },
                    ))
                    .child(
                        GpuiScrollbar::vertical(&self.main_list_state)
                            .id("launcher-main-scrollbar")
                            .scroll_size(size(px(0.0), content_height))
                            .into_any_element(),
                    )
                    .into_any_element()
            };

            let boundary_offset_px = self.main_list_boundary_affordance.offset_px;
            let top_fade = self.main_list_top_fade_snapshot();
            let main_list_tokens = current_main_menu_theme.def().list;

            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .w_full()
                .h_full()
                .overflow_hidden()
                .on_scroll_wheel(
                    cx.listener(|this, event: &gpui::ScrollWheelEvent, window, cx| {
                        this.observe_main_list_scroll_wheel(event, window, cx);
                    }),
                )
                // Move only the painted row subtree. Header, logical ListState,
                // scrollbar, and footer remain fixed during boundary pull.
                .child(
                    div()
                        .id("launcher-main-elastic-document")
                        .relative()
                        .top(px(boundary_offset_px))
                        .w_full()
                        .h_full()
                        .child(variable_height_list),
                )
                .when(top_fade.active, |root| {
                    root.child(
                        crate::components::list_scroll_affordance::render_top_occlusion_at(
                            &self.theme,
                            main_list_tokens,
                            top_fade.progress,
                            header_overlay_height,
                        ),
                    )
                })
                .child(scrollbar_overlay)
                .into_any_element()
        };

        // Log panel - uses pre-extracted theme values to avoid borrow conflicts
        let log_panel = if self.show_logs {
            let logs = logging::get_last_logs(10);
            let mut log_container = div()
                .flex()
                .flex_col()
                .w_full()
                .bg(rgb(log_panel_bg))
                .border_t_1()
                .border_color(rgb(log_panel_border))
                .p(px(design_spacing.padding_md))
                .max_h(px(LOG_PANEL_MAX_HEIGHT))
                .font_family(FONT_MONO);

            for log_line in logs.iter().rev() {
                log_container = log_container.child(
                    div()
                        .text_color(rgb(log_panel_success))
                        .text_xs()
                        .child(log_line.clone()),
                );
            }
            Some(log_container)
        } else {
            None
        };

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                if event.keystroke.modifiers.platform {
                    logging::log(
                        "KEY",
                        &format!(
                            "ScriptList key_down saw key={} shortcut={} view={} popup={} focused_input={:?}",
                            event.keystroke.key,
                            shortcuts::keystroke_to_shortcut(
                                event.keystroke.key.as_str(),
                                &event.keystroke.modifiers,
                            ),
                            this.app_view_name(),
                            this.show_actions_popup,
                            this.focused_input
                        ),
                    );
                    tracing::debug!(
                        event = "script_list.key_down",
                        key = %event.keystroke.key,
                        cmd = true,
                        shift = event.keystroke.modifiers.shift,
                        mini_mode = (this.main_window_mode == MainWindowMode::Mini),
                        "script_list key_down: cmd+{}",
                        event.keystroke.key,
                    );
                }
                // Hide cursor while typing - automatically shows when mouse moves
                this.hide_mouse_cursor(cx);

                // If the shortcut recorder is active, don't process any key events.
                // The recorder has its own key handlers and should receive all key events.
                if this.shortcut_recorder_state.is_some() {
                    return;
                }

                let key_str = event.keystroke.key.as_str();
                if this
                    .consume_return_to_script_list_enter_guard(key_str, &event.keystroke.modifiers)
                {
                    cx.stop_propagation();
                    return;
                }

                // Global shortcuts (Cmd+W only - ScriptList has special ESC handling below)
                if this.handle_global_shortcut_with_options(event, GlobalShortcutEscape::CallerOwned, cx) {
                    return;
                }

                let key_char = event.keystroke.key_char.as_deref();
                let has_cmd = event.keystroke.modifiers.platform;

                if matches!(this.current_view, AppView::ScriptList)
                    && this.handle_menu_syntax_form_control_key_input(
                        key_str,
                        key_char,
                        &event.keystroke.modifiers,
                        window,
                        cx,
                    )
                {
                    cx.stop_propagation();
                    return;
                }
                if sk_is_key_tab(key_str)
                    && !has_cmd
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.control
                    && this.menu_syntax_capture_form_owns_input()
                {
                    if event.keystroke.modifiers.shift {
                        this.focus_previous_menu_syntax_form_field(window, cx);
                    } else {
                        this.focus_next_menu_syntax_form_field(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }

                // Check SDK action shortcuts FIRST (before built-in shortcuts)
                // This allows scripts to override default shortcuts via setActions()
                if !this.action_shortcuts.is_empty() {
                    let key_combo =
                        shortcuts::keystroke_to_shortcut(key_str, &event.keystroke.modifiers);
                    if let Some(action_name) = this.action_shortcuts.get(&key_combo).cloned() {
                        logging::log(
                            "ACTIONS",
                            &format!(
                                "SDK action shortcut matched: '{}' -> '{}'",
                                key_combo, action_name
                            ),
                        );
                        if this.trigger_action_by_name(&action_name, cx) {
                            return;
                        }
                    }
                }

                // If actions picker is active, route all keyboard events through the shared router
                match this.route_key_to_actions_dialog(
                    key_str,
                    key_char,
                    &event.keystroke.modifiers,
                    ActionsDialogHost::MainList,
                    window,
                    cx,
                ) {
                    ActionsRoute::Execute {
                        action_id,
                        should_close,
                    } => {
                        this.execute_actions_route_action(
                            ActionsDialogHost::MainList,
                            action_id,
                            should_close,
                            window,
                            cx,
                        );
                        cx.notify();
                        return;
                    }
                    ActionsRoute::Handled => return,
                    ActionsRoute::NotHandled => {}
                }

                if has_cmd
                    && event.keystroke.modifiers.shift
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.control
                    && this.try_execute_main_list_action_shortcut_from_display(
                        key_str,
                        &event.keystroke.modifiers,
                        window,
                        cx,
                    )
                {
                    return;
                }

                if has_cmd {
                    let has_shift = event.keystroke.modifiers.shift;
                    let key_match = key_str.to_ascii_lowercase();

                    match key_match.as_str() {
                        "v" if this.route_large_script_list_paste_to_agent_chat(cx) => {
                            logging::log(
                                "KEY",
                                "Shortcut Cmd+V -> route_large_script_list_paste_to_agent_chat",
                            );
                            cx.stop_propagation();
                            return;
                        }
                        "l" => {
                            logging::log("KEY", "Shortcut Cmd+L -> toggle_logs");
                            this.toggle_logs(cx);
                            return;
                        }
                        // Script context shortcuts (require a selected script)
                        // Note: More specific patterns (with shift) must come BEFORE less specific ones
                        "k" if has_shift => {
                            return;
                        }
                        "k" => {
                            // Cmd+K - Toggle actions menu (routed through shared dispatcher)
                            logging::log("KEY", "Shortcut Cmd+K -> handle_cmd_k_actions_toggle");
                            this.handle_cmd_k_actions_toggle(window, cx);
                            return;
                        }
                        "i" => {
                            // Cmd+I - Toggle Info Panel
                            logging::log("KEY", "Shortcut Cmd+I -> toggle_info");
                            this.handle_action("toggle_info".to_string(), window, cx);
                            return;
                        }
                        "e" => {
                            // Cmd+E - Edit Script
                            logging::log("KEY", "Shortcut Cmd+E -> edit_script");
                            this.handle_action("edit_script".to_string(), window, cx);
                            return;
                        }
                        "f" if has_shift => {
                            // Cmd+Shift+F - Reveal in Finder
                            logging::log("KEY", "Shortcut Cmd+Shift+F -> reveal_in_finder");
                            this.handle_action("reveal_in_finder".to_string(), window, cx);
                            return;
                        }
                        "c" if has_shift => {
                            // Cmd+Shift+C - Copy Path
                            logging::log("KEY", "Shortcut Cmd+Shift+C -> copy_path");
                            this.handle_action("copy_path".to_string(), window, cx);
                            return;
                        }
                        "d" if has_shift => {
                            // Cmd+Shift+D - Copy Deeplink
                            logging::log("KEY", "Shortcut Cmd+Shift+D -> copy_deeplink");
                            this.handle_action("copy_deeplink".to_string(), window, cx);
                            return;
                        }
                        "a" if has_shift => {
                            return;
                        }
                        // Global shortcuts
                        "n" => {
                            // Cmd+N - Create Script
                            logging::log("KEY", "Shortcut Cmd+N -> create_script");
                            this.handle_action("create_script".to_string(), window, cx);
                            return;
                        }
                        "r" => {
                            // Cmd+R - Reload Scripts
                            logging::log("KEY", "Shortcut Cmd+R -> reload_scripts");
                            this.handle_action("reload_scripts".to_string(), window, cx);
                            return;
                        }
                        "," => {
                            // Cmd+, - Settings
                            logging::log("KEY", "Shortcut Cmd+, -> settings");
                            this.handle_action("settings".to_string(), window, cx);
                            return;
                        }
                        "q" => {
                            // Cmd+Q - Quit
                            logging::log("KEY", "Shortcut Cmd+Q -> quit");
                            this.handle_action("quit".to_string(), window, cx);
                            return;
                        }
                        _ => {}
                    }
                }

                // Actions popup keyboard routing is handled above via route_key_to_actions_dialog

                // LEGACY: Check if we're in fallback mode (no script matches, showing fallback commands)
                // Note: This is legacy code that handled a separate fallback rendering path.
                // Now fallbacks flow through GroupedListItem from grouping.rs, so this
                // branch should rarely (if ever) be triggered. The normal navigation below
                // handles fallback items in the unified list.
                if this.main_menu_fallback_state.is_active() {
                    match key_str {
                        key if sk_is_key_up(key) => {
                            if this.main_menu_fallback_state.move_up() {
                                cx.notify();
                            }
                        }
                        key if sk_is_key_down(key) => {
                            if this.main_menu_fallback_state.move_down() {
                                cx.notify();
                            }
                        }
                        key if sk_is_key_enter(key) => {
                            if !this.gpui_input_focused {
                                this.execute_selected_fallback(cx);
                            }
                        }
                        key if sk_is_key_escape(key) => {
                            // Clear filter to exit fallback mode
                            this.clear_filter(window, cx);
                        }
                        _ => {}
                    }
                    return;
                }

                // Run 12 Pass 13 — `ai-proposal-accept-dismiss`. When the
                // inline AI proposal hint card is up, Tab/Enter accepts and
                // Esc dismisses BEFORE the legacy navigation/clear handlers
                // run. Modifier-bearing keys (Cmd+Enter etc.) fall through
                // so the proposal-generation chord still works.
                if this.pending_menu_syntax_ai_proposal.is_some()
                    && !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.control
                {
                    if sk_is_key_tab(key_str)
                        || (sk_is_key_enter(key_str) && !event.keystroke.modifiers.shift)
                    {
                        if this.try_apply_pending_menu_syntax_ai_proposal(
                            crate::menu_syntax_ai_apply::ProposalApplyAction::Accept,
                            window,
                            cx,
                        ) {
                            cx.stop_propagation();
                            return;
                        }
                    } else if sk_is_key_escape(key_str)
                        && this.try_apply_pending_menu_syntax_ai_proposal(
                            crate::menu_syntax_ai_apply::ProposalApplyAction::Dismiss,
                            window,
                            cx,
                        ) {
                            cx.stop_propagation();
                            return;
                        }
                }

                // ── Spine projection Enter intercept ──────────────
                // Spine Enter routing:
                // 1. Projection rows win (accept the selected row).
                // 2. Complete prompt-builder plans submit to Agent Chat chat.
                // 3. Everything else falls through to normal execution.
                if sk_is_key_enter(key_str)
                    && !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.shift
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.control
                    && this.try_handle_spine_enter(window, cx) {
                        cx.stop_propagation();
                        return;
                    }

                // Normal script list navigation
                // NOTE: Arrow keys are now handled by the arrow_interceptor in app_impl.rs
                // which fires before the Input component can consume them. This allows
                // input history navigation + list navigation to work correctly.
                match key_str {
                    key if sk_is_key_enter(key)
                        && event.keystroke.modifiers.platform
                        && !event.keystroke.modifiers.shift
                        && !event.keystroke.modifiers.alt
                        && !event.keystroke.modifiers.control =>
                    {
                        if this.try_route_global_cmd_enter_to_agent_chat_context_capture(cx) {
                            cx.stop_propagation();
                        }
                    }
                    key if sk_is_key_enter(key) => {
                        if !this.gpui_input_focused {
                            this.execute_selected(cx);
                        }
                    }
                    key if sk_is_key_escape(key) => {
                        // Escape order on ScriptList:
                        //   1. attachment portal active → cancel back to its host.
                        //   2. menu-syntax trigger picker visible → close popup
                        //      only, leave filter text untouched. Second Escape
                        //      then falls through to the normal clear-filter
                        //      branch below.
                        //   3. visible filter non-empty → clear filter.
                        //   4. launcher-origin surface → go back to the main launcher.
                        //   5. filter empty → hide main window.
                        if this.try_cancel_script_list_attachment_portal_escape(
                            "physical_bubble",
                            cx,
                        ) {
                            return;
                        }
                        if this.menu_syntax_object_selector_owns_main_keyboard()
                            && this.apply_menu_syntax_object_selector_intent(
                                crate::menu_syntax::InlinePickerKeyIntent::Close,
                                window,
                                cx,
                            ) {
                                return;
                            }
                        if this.menu_syntax_trigger_picker_owns_main_keyboard() {
                            if this.menu_syntax_filter_only_escape_should_clear() {
                                this.clear_filter(window, cx);
                                return;
                            }
                            if this.apply_menu_syntax_trigger_picker_intent(
                                crate::menu_syntax::InlinePickerKeyIntent::Close,
                                window,
                                cx,
                            ) {
                                return;
                            }
                        }
                        // Clear visible text first. Hidden stale canonical
                        // state is reconciled before close/go-back so an empty
                        // input does not require a no-op extra Escape.
                        if this.script_list_escape_should_clear_visible_filter(cx) {
                            this.clear_filter(window, cx);
                        } else if this.opened_from_main_menu {
                            this.clear_hidden_script_list_filter_before_escape_close(window, cx);
                            this.go_back_or_close(window, cx);
                        } else {
                            this.clear_hidden_script_list_filter_before_escape_close(window, cx);
                            // Filter is empty - close window
                            this.close_and_reset_window(cx);
                        }
                    }
                    _ => {}
                }
            },
        );

        let handle_key_up = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyUpEvent,
                  _window: &mut Window,
                  _cx: &mut Context<Self>| {
                let key = event.keystroke.key.as_str();
                this.clear_return_to_script_list_enter_guard_on_key_up(key);
                if sk_is_key_up(key) || sk_is_key_down(key) {
                    tracing::info!(
                        target: "script_kit::input_history",
                        event = "script_list_arrow_key_up",
                        key = %key,
                        selected_index = this.selected_index,
                        history_index = ?this.input_history.current_index(),
                        filter_len = this.filter_text.len(),
                    );
                }
            },
        );

        // Main container with system font and transparency
        // NOTE: Shadow disabled for vibrancy - shadows on transparent elements cause gray fill

        // Use unified color resolver for text and fonts
        let text_primary = color_resolver.primary_text_color();
        let font_family = self.theme_font_family();

        let highlight_text = filter_text_for_render.clone();
        if self.main_menu_render_diagnostics.last_input_highlight_text != highlight_text {
            let capture_targets =
                crate::menu_syntax::registered_capture_targets_from_scripts(&self.scripts);
            let mut next_highlight_ranges: Vec<(std::ops::Range<usize>, gpui::Hsla, String)> =
                crate::menu_syntax::input_spans_for_input_with_targets(
                    &highlight_text,
                    &capture_targets,
                )
                .into_iter()
                .filter(|span| span.role != crate::menu_syntax::MenuSyntaxFragmentRole::Subject)
                .map(|span| {
                    (
                        span.range,
                        rgb(menu_syntax_input_span_color(&self.theme, span.role)).into(),
                        menu_syntax_input_span_role_name(span.role).to_string(),
                    )
                })
                .collect();

            if self.spine_enabled {
                let accent_color: gpui::Hsla = rgb(self.theme.colors.accent.selected).into();
                let spine_projection = self.spine_projection.as_ref();
                let spine_accent_ranges =
                    crate::spine::input_spans::accent_ranges_for_parse_with_resolved_tokens(
                        &self.spine_parse,
                        spine_projection,
                        &|token| self.spine_mention_aliases.contains_key(token),
                    );
                let gpui_input_value = self.gpui_input_state.read(cx).value().to_string();
                if gpui_input_value == self.spine_parse.input {
                    let input_len = gpui_input_value.len();
                    for (range, role) in spine_accent_ranges {
                        if range.end <= input_len
                            && range.start <= range.end
                            && gpui_input_value.is_char_boundary(range.start)
                            && gpui_input_value.is_char_boundary(range.end)
                        {
                            next_highlight_ranges.push((range, accent_color, role.to_string()));
                        }
                    }
                }
            }

            if self
                .main_menu_render_diagnostics
                .last_input_highlight_ranges
                != next_highlight_ranges
            {
                let input_highlight_ranges = next_highlight_ranges.clone();
                self.gpui_input_state.update(cx, |state, _cx| {
                    state.set_highlight_ranges_with_roles(input_highlight_ranges);
                });
                self.main_menu_render_diagnostics
                    .last_input_highlight_ranges = next_highlight_ranges;
            }
            self.main_menu_render_diagnostics.last_input_highlight_text = highlight_text;
        }

        // NOTE: No .bg() here - Root provides vibrancy background for ALL content
        // This ensures main menu, AI chat, and all prompts have consistent styling
        let menu_theme = self.current_main_menu_theme;
        let menu_def = menu_theme.def();
        let shell = menu_def.shell;
        let search = menu_def.search;

        self.sync_main_list_displayed_action_shortcut_keybindings(cx);

        let root = crate::components::main_view_chrome::render_main_view_shell()
            // NOTE: No shadow - shadows on transparent elements cause gray fill with vibrancy
            .text_color(rgb(text_primary))
            .font_family(font_family)
            .key_context("script_list")
            .track_focus(&self.focus_handle)
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if this.preempt_empty_script_list_escape_close(event, window, cx) {
                    cx.stop_propagation();
                }
            }))
            // GPUI auto-transfers keyboard focus to this root's tracked handle on
            // any mouse down that no deeper focusable claimed (see
            // paint_mouse_listeners in vendor/gpui/src/elements/div.rs). Dragging
            // the window by dead space or clicking a list row would dump focus on
            // the script_list root and kill typing in the filter input. Deeper
            // focusables (the filter input itself, overlay dialogs) run earlier in
            // the bubble phase and call prevent_default, and the detached actions
            // popup keeps its own window focus, so this only fires when focus
            // would otherwise land on the root handle.
            .on_any_mouse_down(cx.listener(|this, _event, window, cx| {
                if window.default_prevented() {
                    return;
                }
                let filter_focus = this.gpui_input_state.focus_handle(cx);
                if filter_focus.is_focused(window) {
                    window.prevent_default();
                } else if this.focus_handle.is_focused(window) {
                    this.focus_main_filter(window, cx);
                    window.prevent_default();
                }
            }))
            .on_action(cx.listener(
                |this,
                 action: &crate::actions::MainListDisplayedActionShortcut,
                 window,
                 cx| {
                    logging::log(
                        "KEY_ROUTE",
                        &format!(
                            "Displayed shortcut keybinding received canonical={} context=script_list",
                            action.shortcut
                        ),
                    );
                    this.try_execute_main_list_action_shortcut_canonical(
                        &action.shortcut,
                        window,
                        cx,
                    );
                },
            ))
            .on_key_down(handle_key)
            .on_key_up(handle_key_up);

        let input_body = if handler_form_owns_input_for_render
            && menu_syntax_text_contains_line_break(&filter_text_for_render)
        {
            div()
                .w_full()
                .h(px(search.height))
                .flex()
                .items_center()
                .overflow_hidden()
                .text_ellipsis()
                .text_size(px(search.font_size))
                .font_weight(search.font_weight)
                .line_height(px(search.height))
                .text_color(rgb(text_primary))
                .child(menu_syntax_single_line_text_for_gpui(
                    &filter_text_for_render,
                ))
                .into_any_element()
        } else {
            self.render_search_input_with_ghost(cx).into_any_element()
        };

        let input = crate::components::main_view_chrome::render_main_view_input_shell(
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewInputChrome {
                body: input_body,
                trailing: Vec::new(),
            },
        );

        let header = crate::components::main_view_chrome::MainViewHeaderChrome::canonical(
            menu_def,
            self.render_clickable_main_view_context_zone(menu_def, cx),
            input,
        );
        let divider = crate::components::main_view_chrome::MainViewDividerChrome {
            margin_x: shell.divider_margin_x,
            height: if is_default_design {
                shell.divider_height
            } else {
                design_visual.border_thin
            },
            visible: false,
        };

        if is_mini {
            // Mini mode: single column, toggle between list and info panel
            let main = if self.show_info_panel {
                // Info panel replaces the list when toggled via Cmd+I
                let info_panel = self.render_preview_panel(cx);
                div()
                    .w_full()
                    .h_full()
                    .min_h(px(0.))
                    .pt(px(
                        crate::components::main_view_chrome::main_view_header_metrics(
                            menu_def,
                            Some(menu_def.search.height),
                        )
                        .header_height,
                    ))
                    .child(info_panel)
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .h_full()
                    .min_h(px(0.))
                    .child(list_element)
                    .into_any_element()
            };

            if state_changed && crate::logging::filter_perf_trace_enabled() {
                tracing::info!(
                    target: "script_kit::prompt_chrome",
                    event = "script_list_mini_ai_hint_rendered",
                    tab_hint = show_launcher_ask_ai_hint,
                    cmd_enter_hint = false,
                    "Mini ScriptList header rendered Ask AI keyboard hints"
                );
                let total_elapsed = render_list_start.elapsed();
                tracing::info!(
                    target: "RENDER_PERF",
                    category = "mini_render",
                    event = "render_script_list_end",
                    filter = %filter_for_log,
                    item_count = item_count_for_log,
                    selected_index = self.selected_index,
                    total_ms = format_args!("{:.2}", total_elapsed.as_secs_f64() * 1000.0),
                    mode = "mini",
                    "mini script list render complete"
                );
                self.main_menu_render_diagnostics.last_render_log_filter = self.filter_text.clone();
                self.main_menu_render_diagnostics.last_render_log_selection = self.selected_index;
                self.main_menu_render_diagnostics.last_render_log_item_count = item_count_for_log;
            }

            let mini_footer = if self.main_window_uses_native_footer() {
                crate::components::prompt_layout_shell::render_native_main_window_footer_hover_blocker()
                    .into_any_element()
            } else {
                let config = self
                    .main_window_footer_config_with_cx(Some(&*cx))
                    .expect("Script List owns a main-window footer config");
                let app = cx.entity().downgrade();
                crate::components::footer_chrome::render_main_window_footer_config_rail(
                    config,
                    move |action, window, cx| {
                        if let Some(app) = app.upgrade() {
                            app.update(cx, |this, cx| {
                                this.dispatch_main_window_footer_action(
                                    action,
                                    window,
                                    cx,
                                    "gpui_footer_click",
                                );
                            });
                        }
                    },
                )
            };
            return crate::components::main_view_chrome::render_main_view_chrome_header_overlay_footer_flush(
                root,
                &self.theme,
                menu_def,
                crate::components::main_view_chrome::MainViewChrome {
                    header,
                    divider,
                    main,
                    footer: Some(mini_footer),
                    overlays: log_panel
                        .into_iter()
                        .map(|panel| panel.into_any_element())
                        .collect(),
                },
            );
        }

        // Main content area: list takes full width unless info panel is toggled (Cmd+I)
        let main = div()
            .flex()
            .flex_row()
            .h_full()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            // Left side: Script list — full width when info hidden, 50% when shown
            .child(
                div()
                    .when(self.show_info_panel, |d| d.w_1_2())
                    .when(!self.show_info_panel, |d| d.w_full())
                    .h_full()
                    .min_h(px(0.))
                    .child(list_element),
            )
            // Right side: Info panel (50% width), only rendered when toggled
            .when(self.show_info_panel, |row| {
                let preview_start = std::time::Instant::now();
                let preview_panel = self.render_preview_panel(cx);
                let preview_elapsed = preview_start.elapsed();
                if state_changed {
                    logging::log(
                        "PREVIEW_PERF",
                        &format!(
                            "[PREVIEW_PANEL_DONE] filter='{}' took {:.2}ms",
                            filter_for_log,
                            preview_elapsed.as_secs_f64() * 1000.0
                        ),
                    );
                }
                row.child(
                    div()
                        .relative()
                        .flex()
                        .flex_col()
                        .w_1_2()
                        .h_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .when_some(self.cached_main_window_preflight.clone(), |d, receipt| {
                            d.child(
                                crate::main_window_preflight::render_main_window_preflight_receipt(
                                    self, &receipt,
                                ),
                            )
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.))
                                .pt(px(
                                    crate::components::main_view_chrome::main_view_header_metrics(
                                        menu_def,
                                        Some(menu_def.search.height),
                                    )
                                    .header_height,
                                ))
                                .child(preview_panel),
                        ),
                )
            })
            .into_any_element();

        // Footer: Universal three-key hint strip — ↵ Run · ⌘K Actions · ⌘↵ AI
        let footer = {
            let primary_label = self.main_window_primary_action_label();
            let hints =
                crate::components::universal_prompt_hints_with_primary_label(&primary_label);
            crate::components::emit_prompt_hint_audit("render_script_list::full", &hints);
            tracing::info!(
                target: "script_kit::prompt_chrome",
                event = "script_list_footer_unified",
                mode = "full",
                primary_label = %primary_label,
                "Script list footer rendered with selected enter-action label"
            );
            let config = self
                .main_window_footer_config_with_cx(Some(&*cx))
                .expect("Script List owns a main-window footer config");
            let app = cx.entity().downgrade();
            let gpui_footer =
                crate::components::footer_chrome::render_main_window_footer_config_rail(
                    config,
                    move |action, window, cx| {
                        if let Some(app) = app.upgrade() {
                            app.update(cx, |this, cx| {
                                this.dispatch_main_window_footer_action(
                                    action,
                                    window,
                                    cx,
                                    "gpui_footer_click",
                                );
                            });
                        }
                    },
                );
            self.main_window_footer_slot(gpui_footer)
        };

        // Note: Toast notifications are now handled by gpui-component's NotificationList
        // via the Root wrapper. Toasts are flushed in render() via flush_pending_toasts().

        // Note: HUD overlay is added at the top-level render() method for all views

        // Log total render_script_list time and update tracking state (only if state changed)
        if state_changed {
            let total_elapsed = render_list_start.elapsed();
            logging::log(
                "RENDER_PERF",
                &format!(
                    "[RENDER_SCRIPT_LIST_END] filter='{}' total={:.2}ms",
                    filter_for_log,
                    total_elapsed.as_secs_f64() * 1000.0
                ),
            );
            // Deferred state update: update after all logging (including preview panel) is done
            self.main_menu_render_diagnostics.last_render_log_filter = self.filter_text.clone();
            self.main_menu_render_diagnostics.last_render_log_selection = self.selected_index;
            self.main_menu_render_diagnostics.last_render_log_item_count = item_count_for_log;
        }

        crate::components::main_view_chrome::render_main_view_chrome_header_overlay_footer_flush(
            root,
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header,
                divider,
                main,
                footer,
                overlays: log_panel
                    .into_iter()
                    .map(|panel| panel.into_any_element())
                    .collect(),
            },
        )
    }
}

#[cfg(test)]
mod render_script_list_footer_tests {
    use super::{
        app_shell_footer_colors, inline_calc_list_item_hint_text_color,
        inline_calc_list_item_result_text_color, inline_calc_list_item_selected_overlay_rgba,
        inline_calc_list_item_title, menu_syntax_single_line_text_for_gpui,
        script_list_footer_info_label,
    };
    use crate::designs::DesignVariant;
    use crate::theme::ColorResolver;

    #[test]
    fn test_app_shell_footer_colors_use_theme_accent_tokens() {
        let theme = crate::theme::Theme::default();
        let colors = app_shell_footer_colors(&theme);

        assert_eq!(colors.accent, theme.colors.accent.selected);
        assert_eq!(colors.background, theme.colors.accent.selected_subtle);
        assert_eq!(colors.border, theme.colors.ui.border);
        assert_eq!(colors.text_muted, theme.colors.text.muted);
    }

    #[test]
    fn test_universal_prompt_hints_support_custom_primary_label() {
        let hints = crate::components::universal_prompt_hints_with_primary_label("Open App");
        assert_eq!(hints[0].as_ref(), "↵ Open App");
    }

    #[test]
    fn test_script_list_footer_info_label_hidden_when_window_tweaker_disabled() {
        assert_eq!(
            script_list_footer_info_label(false, false, 75, "acrylic", "light"),
            None
        );
    }

    #[test]
    fn test_script_list_footer_info_label_hidden_in_dark_mode() {
        assert_eq!(
            script_list_footer_info_label(true, true, 75, "acrylic", "dark"),
            None
        );
    }

    #[test]
    fn test_script_list_footer_info_label_formats_window_tweaker_metadata() {
        assert_eq!(
            script_list_footer_info_label(true, false, 75, "acrylic", "light"),
            Some("75% | acrylic | light | ⌘-/+ ⌘M ⌘⇧A".to_string())
        );
    }

    #[test]
    fn test_truncate_str_chars_returns_valid_utf8_boundary_when_filter_text_is_multibyte() {
        let input = "é".repeat(45);
        let truncated = crate::utils::truncate_str_chars(&input, 27);

        assert_eq!(truncated.chars().count(), 27);
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn test_menu_syntax_single_line_text_for_gpui_replaces_newlines() {
        let rendered = menu_syntax_single_line_text_for_gpui("first\ns\r\nthird");

        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\r'));
        assert!(rendered.contains("first"));
        assert!(rendered.contains('s'));
        assert!(rendered.contains("third"));
    }

    #[test]
    fn test_inline_calc_list_item_title_prefixes_equals_sign() {
        assert_eq!(inline_calc_list_item_title("1500"), "= 1500");
    }

    #[test]
    fn test_inline_calc_result_text_color_does_use_resolver_accent_when_selected_non_default() {
        let mut theme = crate::theme::Theme::default();
        theme.colors.accent.selected = 0x112233;
        let color_resolver = ColorResolver::new(&theme, DesignVariant::NeonCyberpunk);

        let color = inline_calc_list_item_result_text_color(
            true,
            DesignVariant::NeonCyberpunk,
            &theme,
            color_resolver,
        );

        assert_eq!(color, color_resolver.primary_accent());
        assert_ne!(color, theme.colors.accent.selected);
    }

    #[test]
    fn test_inline_calc_hint_text_color_does_use_color_resolver_muted_token() {
        let theme = crate::theme::Theme::default();
        let color_resolver = ColorResolver::new(&theme, DesignVariant::NeonCyberpunk);

        assert_eq!(
            inline_calc_list_item_hint_text_color(color_resolver),
            color_resolver.empty_text_color()
        );
    }

    #[test]
    fn test_inline_calc_selected_overlay_does_use_resolver_accent_with_theme_alpha() {
        let mut theme = crate::theme::Theme::default();
        theme.colors.accent.selected_subtle = 0x010203;
        let color_resolver = ColorResolver::new(&theme, DesignVariant::NeonCyberpunk);

        let expected_alpha =
            ((theme.get_opacity().selected.clamp(0.0, 1.0) * 255.0).round() as u32).max(
                crate::designs::MainMenuThemeVariant::InfoBarBase
                    .def()
                    .list
                    .inline_calc_selected_overlay_min_alpha,
            );
        let expected = (color_resolver.primary_accent() << 8) | expected_alpha;

        assert_eq!(
            inline_calc_list_item_selected_overlay_rgba(
                &theme,
                crate::designs::MainMenuThemeVariant::InfoBarBase.def().list,
                color_resolver
            ),
            expected
        );
    }
}

#[cfg(test)]
mod render_script_list_click_contract_tests {
    use std::fs;

    #[test]
    fn launcher_list_uses_shared_selected_row_click_helper() {
        let source = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("Failed to read src/render_script_list/mod.rs");

        assert!(
            source.contains("should_submit_selected_row_click"),
            "render_script_list should use the shared selected-row click helper"
        );
        assert!(
            source.contains("let was_selected = this.selected_index == ix")
                && source.contains("!this.spine_empty_subsearch_selection_suppressed()"),
            "render_script_list click handler should capture whether the row was already selected"
        );
        assert!(
            source.contains("Menu-syntax trigger picker row mouse-down accepting")
                && source.contains("menu_syntax_trigger_row_id_from_main_list_index(ix)")
                && source.contains(
                    "this.menu_syntax_trigger_picker_suppress_next_launcher_click = true;"
                )
                && source.contains("Menu-syntax trigger picker consumed trailing launcher click")
                && source.contains("this.accept_menu_syntax_trigger_picker_row")
                && source.contains("cx.stop_propagation();"),
            "menu-syntax trigger picker pointer selection must accept on mouse down and consume the trailing launcher click"
        );
        assert!(
            source.contains("this.execute_selected(cx);"),
            "render_script_list click handler should still execute the selected row"
        );
    }
}

#[cfg(test)]
mod launcher_empty_info_state_contract_tests {
    use std::fs;

    #[test]
    fn launcher_empty_state_routes_through_info_state() {
        let source = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("failed to read src/render_script_list/mod.rs");
        let old_empty_title = concat!("No scripts or ", "snippets found");
        let old_empty_hint = concat!("Press ", "⌘N", " to create a new script");
        let old_generic_fallback =
            concat!("Try a different search term or press ", "⌘↵", " to ask AI");

        assert!(
            source.contains("render_launcher_empty_or_no_results"),
            "launcher empty/no-results must render through shared InfoState"
        );
        assert!(
            !source.contains(old_empty_title),
            "old launcher empty title must not return"
        );
        assert!(
            !source.contains(old_empty_hint),
            "old launcher empty hint must not return"
        );
        assert!(
            !source.contains(old_generic_fallback),
            "old generic no-results fallback must not return"
        );
    }
}
