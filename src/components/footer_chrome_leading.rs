fn footer_config_dot_color(
    status: crate::footer_popup::FooterDotStatus,
    prefer_accent: bool,
    theme: &Theme,
) -> gpui::Rgba {
    use crate::footer_popup::FooterDotStatus;

    let color = match status {
        FooterDotStatus::Streaming | FooterDotStatus::WaitingForPermission => {
            if prefer_accent {
                theme.colors.accent.selected
            } else {
                theme.colors.text.primary
            }
        }
        FooterDotStatus::Idle => theme.colors.text.secondary,
        FooterDotStatus::Error => theme.colors.ui.error,
        FooterDotStatus::Hidden => theme.colors.text.secondary,
    };
    rgba((color << 8) | 0xff)
}

fn render_footer_config_status_dot(
    status: crate::footer_popup::FooterDotStatus,
    prefer_accent: bool,
    theme: &Theme,
) -> Option<AnyElement> {
    use crate::footer_popup::FooterDotStatus;

    if matches!(status, FooterDotStatus::Hidden) {
        return None;
    }

    let dot = div()
        .id("config-footer-status-dot")
        .size(px(FOOTER_STATUS_DOT_SIZE_PX))
        .flex_none()
        .rounded(px(FOOTER_STATUS_DOT_SIZE_PX / 2.0))
        .bg(footer_config_dot_color(status, prefer_accent, theme));
    let dot: AnyElement = if matches!(
        status,
        FooterDotStatus::Streaming | FooterDotStatus::WaitingForPermission
    ) {
        dot.with_animation(
            "config-footer-status-dot-pulse",
            Animation::new(Duration::from_millis(2_000)).repeat(),
            |dot, delta| dot.opacity(0.6 + 0.4 * (delta * std::f32::consts::TAU).sin().abs()),
        )
        .into_any_element()
    } else {
        dot.into_any_element()
    };
    Some(dot)
}

#[allow(clippy::too_many_arguments)]
fn render_footer_config_left_marker<H>(
    id: &'static str,
    icon_token: Option<&str>,
    dot_status: crate::footer_popup::FooterDotStatus,
    prefer_accent: bool,
    spinner_glyph: Option<&str>,
    keycap: Option<&str>,
    label: &str,
    bold_label: bool,
    selected: bool,
    action: Option<crate::footer_popup::FooterAction>,
    theme: &Theme,
    on_action: H,
) -> AnyElement
where
    H: Fn(crate::footer_popup::FooterAction, &mut Window, &mut App) + Clone + 'static,
{
    let metrics = current_main_menu_footer_metrics();
    let interactive = action.is_some();
    let row_states = resolved_footer_button_visual_colors(theme).row_states;
    let base_state = if selected {
        row_states.active
    } else {
        row_states.rest
    };
    let hover_state = if selected {
        row_states.active
    } else {
        row_states.hover
    };
    let hover_bg = rgba(row_states.hover.background_rgba.unwrap_or_default());
    let active_bg = rgba(row_states.active.background_rgba.unwrap_or_default());
    let base_foreground = rgba(base_state.primary_foreground_rgba);
    let hover_foreground: gpui::Hsla = rgba(hover_state.primary_foreground_rgba).into();
    let mut marker = div()
        .id(id)
        .h(px(footer_button_height(metrics.height_px)))
        .min_w(px(0.0))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(FOOTER_LEFT_INFO_GAP_PX))
        .px(px(footer_centered_action_edge_padding_x()))
        .rounded(px(metrics.button_radius))
        .group("config-footer-left-marker")
        .when(selected, |style| style.bg(active_bg));

    if interactive {
        marker = marker
            .cursor_pointer()
            .when(!selected, |style| {
                style.hover(move |style| style.bg(hover_bg))
            })
            .active(move |style| style.bg(active_bg));
    }

    if let Some(dot) = render_footer_config_status_dot(dot_status, prefer_accent, theme) {
        marker = marker.child(dot);
    }
    if let Some(glyph) = spinner_glyph.filter(|glyph| !glyph.trim().is_empty()) {
        marker = marker.child(
            div()
                .id("config-footer-braille-spinner")
                .w(px(FOOTER_BRAILLE_SPINNER_LANE_PX))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .font_family(crate::list_item::FONT_MONO)
                .text_size(px(FOOTER_BRAILLE_SPINNER_FONT_PX))
                .text_color(rgba((theme.colors.accent.selected << 8) | 0xff))
                .child(glyph.to_string()),
        );
    }
    if let Some(path) = icon_token.and_then(footer_icon_path) {
        marker = marker.child(
            svg()
                .path(path)
                .size(px(13.0))
                .flex_none()
                .text_color(base_foreground)
                .group_hover("config-footer-left-marker", move |style| {
                    style.text_color(hover_foreground)
                }),
        );
    }
    if let Some(keycap) = keycap.filter(|key| !key.trim().is_empty()) {
        marker = marker.child(render_footer_shortcut_keycaps_for_state(
            keycap.to_string(),
            theme,
            selected,
        ));
    }
    if !label.trim().is_empty() {
        marker = marker.child(
            div()
                .min_w(px(0.0))
                .font_family(FONT_SYSTEM_UI)
                .font_weight(if bold_label {
                    FontWeight::SEMIBOLD
                } else {
                    metrics.font_weight
                })
                .text_size(px(metrics.label_font_size))
                .text_color(base_foreground)
                .group_hover("config-footer-left-marker", move |style| {
                    style.text_color(hover_foreground)
                })
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(label.to_string()),
        );
    }

    let interactive = action.is_some();
    if let Some(action) = action {
        marker = marker.on_click(move |_event, window, cx| on_action(action, window, cx));
    }
    if interactive {
        glass_capsule(id, Some(metrics.button_radius), marker)
    } else {
        marker.into_any_element()
    }
}

fn render_footer_config_left_info<H>(
    info: crate::footer_popup::FooterLeftInfo,
    theme: &Theme,
    on_action: H,
) -> AnyElement
where
    H: Fn(crate::footer_popup::FooterAction, &mut Window, &mut App) + Clone + 'static,
{
    let mut row = div()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .items_center()
        .gap(px(FOOTER_ACTION_ITEM_GAP_PX))
        .overflow_hidden();

    if let Some(cwd) = info.cwd_chip.as_ref() {
        row = row.child(render_footer_config_left_marker(
            "config-footer-cwd-chip",
            Some(&cwd.icon_token),
            crate::footer_popup::FooterDotStatus::Hidden,
            false,
            None,
            cwd.key.as_deref(),
            &cwd.label,
            false,
            false,
            Some(crate::footer_popup::FooterAction::Cwd),
            theme,
            on_action.clone(),
        ));
    }

    if let Some(profile_name) = info.profile_name.as_deref() {
        row = row
            .child(render_footer_config_left_marker(
                "config-footer-profile-chip",
                info.icon_token
                    .as_deref()
                    .or(Some(FOOTER_PROFILE_ICON_TOKEN)),
                info.dot_status,
                info.prefer_accent_for_active_states,
                info.spinner_glyph.as_deref(),
                info.keycap.as_deref(),
                profile_name,
                info.bold_label,
                info.selected,
                info.action,
                theme,
                on_action.clone(),
            ))
            .child(render_footer_config_left_marker(
                "config-footer-model-chip",
                None,
                crate::footer_popup::FooterDotStatus::Hidden,
                false,
                None,
                None,
                &info.model_name,
                false,
                false,
                Some(crate::footer_popup::FooterAction::AgentModel),
                theme,
                on_action,
            ));
    } else {
        row = row.child(render_footer_config_left_marker(
            "config-footer-left-info",
            info.icon_token.as_deref(),
            info.dot_status,
            info.prefer_accent_for_active_states,
            info.spinner_glyph.as_deref(),
            info.keycap.as_deref(),
            &info.model_name,
            info.bold_label,
            info.selected,
            info.action,
            theme,
            on_action,
        ));
    }

    row.into_any_element()
}
