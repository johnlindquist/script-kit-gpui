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
    semantic_id: Option<String>,
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

    let element = div()
        .relative()
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
        );
    #[cfg(feature = "owned-ui-evaluation")]
    let element = element.when(crate::runtime_policy::is_owned_evaluation(), |element| {
        let selected_surface_color = is_selected.then(|| {
            inline_calc_list_item_selected_overlay_rgba(theme, list_tokens, color_resolver)
        });
        let metadata = std::rc::Rc::new(serde_json::json!({
            "selected": is_selected, "selectedSurfaceColor": selected_surface_color,
        }));
        element.child(
            gpui::canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    if window.owned_frame_observation_active() {
                        if let Some(id) = semantic_id.as_ref() {
                            window.record_owned_paint_binding(
                                "mainSearchCalculator",
                                id.clone(),
                                bounds,
                                metadata.clone(),
                            );
                        }
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    });
    #[cfg(not(feature = "owned-ui-evaluation"))]
    let _ = semantic_id;
    element.into_any_element()
}
