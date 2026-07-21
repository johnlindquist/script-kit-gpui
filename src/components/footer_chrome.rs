use gpui::{
    div, prelude::FluentBuilder, px, rgba, svg, Animation, AnimationExt, AnyElement, App, Div,
    FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use std::time::Duration;

use crate::list_item::FONT_SYSTEM_UI;
use crate::theme::opacity::OPACITY_TEXT_MUTED;
use crate::theme::Theme;
use crate::ui_foundation::HexColorExt;

#[allow(dead_code)]
pub(crate) const FOOTER_HINT_FONT_WEIGHT_APPKIT: f64 = 0.14;
// Footer action/keycap text weight. Lowered 560 -> 500 (Medium) so the footer
// buttons no longer read as bold/semibold. Apple's macOS type system uses Regular
// for ordinary control labels and reserves Semibold for emphasis; Medium keeps the
// hints legible over the translucent Liquid Glass footer without the bold look.
// Roughly matches the non-bold AppKit weight trait (0.14) used by the native footer.
#[allow(dead_code)]
pub(crate) const FOOTER_HINT_FONT_WEIGHT_GPUI: FontWeight = FontWeight(500.0);
pub(crate) const FOOTER_KEYCAP_HEIGHT_PX: f32 = 20.0;
pub(crate) const FOOTER_KEYCAP_PADDING_X_PX: f32 = 5.0;
pub(crate) const FOOTER_KEYCAP_RADIUS_PX: f32 = 6.0;
pub(crate) const FOOTER_KEY_GLYPH_NUDGE_Y_PX: f32 = 1.0;
pub(crate) const FOOTER_RETURN_GLYPH_NUDGE_Y_PX: f32 = 1.0;
#[allow(dead_code)]
pub(crate) const FOOTER_SEMICOLON_GLYPH_NUDGE_Y_PX: f32 = -1.0;
pub(crate) const FOOTER_BUTTON_VERTICAL_INSET_PX: f32 = 2.0;
// Inter-item gap for footer hint chips. Kept compact now that only actual
// keycaps carry borders; label text no longer needs the wider bordered-chip
// spacing rhythm.
pub(crate) const FOOTER_ACTION_ITEM_GAP_PX: f32 = 2.0;
/// Resting separation between adjacent native Liquid Glass capsules.
pub(crate) const FOOTER_GLASS_BUTTON_GAP_PX: f32 = 6.0;
pub(crate) const FOOTER_ACTION_CONTENT_GAP_PX: f32 = 4.0;
pub(crate) const FOOTER_ACTION_CONTENT_PADDING_X_PX: f32 = 4.0;
// Extra inner x width (total, split across both sides) that trailing centered
// action buttons (Actions, Agent, Apply, Close, ...) reserve beyond
// `button_padding_x`, so their content and hover pill don't hug the
// label/keycaps. Consumed by both the native AppKit footer
// (`footer_hint_legacy_extra_padding`) and the GPUI flexbox footer overlay so
// the two renderers stay in lockstep.
pub(crate) const FOOTER_TRAILING_ACTION_EXTRA_PADDING_X_PX: f32 = 12.0;
pub(crate) const FOOTER_KEY_ANCHORED_CONTENT_PADDING_X_PX: f32 = 6.0;
pub(crate) const FOOTER_ACTION_BUTTON_RADIUS_PX: f32 = 6.0;
pub(crate) const FOOTER_RUN_SLOT_MIN_WIDTH_PX: f32 = 92.0;
pub(crate) const FOOTER_RUN_SLOT_MAX_WIDTH_PX: f32 = 242.0;
pub(crate) const FOOTER_ACTIONS_SLOT_WIDTH_PX: f32 = 92.0;
pub(crate) const FOOTER_AI_SLOT_WIDTH_PX: f32 = 52.0;
pub(crate) const FOOTER_APPLY_SLOT_WIDTH_PX: f32 = 84.0;
pub(crate) const FOOTER_CLOSE_SLOT_WIDTH_PX: f32 = 84.0;
pub(crate) const FOOTER_STOP_SLOT_WIDTH_PX: f32 = 76.0;
pub(crate) const FOOTER_PASTE_RESPONSE_SLOT_WIDTH_PX: f32 = 140.0;
pub(crate) const FOOTER_SHORTCUT_LAYOUT_MEASUREMENT_SOURCE: &str =
    "runtime.footerChrome.shortcutKeycapLayoutModel";

/// The default radius for every shared native glass capsule opt-in.
pub(crate) fn default_glass_capsule_radius_px() -> f32 {
    FOOTER_ACTION_BUTTON_RADIUS_PX
}

pub(crate) fn glass_capsules_enabled() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::platform::glass_button_host::glass_buttons_enabled()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Shared row opt-in for native glass capsules. Capsule geometry always comes
/// from the supplied elements' real painted bounds; hover events address the
/// same stable group/index pair used by the native registry.
pub(crate) fn glass_capsule_row(
    group: &'static str,
    skip_leading_children: usize,
    radius_px: Option<f32>,
    row: Div,
    capsules: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    glass_capsule_row_with_cleanup(
        group,
        skip_leading_children,
        radius_px,
        row,
        capsules,
        Vec::new(),
    )
}

fn glass_capsule_row_with_cleanup(
    group: &'static str,
    skip_leading_children: usize,
    radius_px: Option<f32>,
    row: Div,
    capsules: impl IntoIterator<Item = AnyElement>,
    groups_to_remove: Vec<&'static str>,
) -> AnyElement {
    let radius = f64::from(radius_px.unwrap_or_else(default_glass_capsule_radius_px));
    let capsules = capsules
        .into_iter()
        .enumerate()
        .map(|(index, capsule)| {
            let wrapper_id = SharedString::from(format!("glass-capsule-{group}-{index}"));
            let wrapper = div()
                .id(wrapper_id)
                .min_w(px(0.0))
                .flex()
                .flex_row()
                .items_center()
                .child(capsule);

            #[cfg(target_os = "macos")]
            let wrapper = wrapper.on_hover(move |hovered: &bool, window, _cx| {
                crate::platform::glass_button_host::set_hover(window, group, index, *hovered);
            });

            wrapper.into_any_element()
        })
        .collect::<Vec<_>>();

    let enabled = glass_capsules_enabled();
    let row = row.children(capsules);

    #[cfg(target_os = "macos")]
    let row = if enabled {
        row.on_children_prepainted(move |bounds, window, _cx| {
            for stale_group in &groups_to_remove {
                crate::platform::glass_button_host::remove_group(window, stale_group);
            }
            let frames = bounds
                .into_iter()
                .skip(skip_leading_children)
                .map(|bounds| {
                    (
                        f64::from(bounds.origin.x.as_f32()),
                        f64::from(bounds.origin.y.as_f32()),
                        f64::from(bounds.size.width.as_f32()),
                        f64::from(bounds.size.height.as_f32()),
                        radius,
                    )
                })
                .collect::<Vec<_>>();
            crate::platform::glass_button_host::sync_for_window(window, group, &frames);
        })
    } else {
        row
    };

    #[cfg(not(target_os = "macos"))]
    let _ = (enabled, radius, skip_leading_children);

    row.id(group).into_any_element()
}

/// Standalone-chip form of [`glass_capsule_row`].
pub(crate) fn glass_capsule(
    group: &'static str,
    radius_px: Option<f32>,
    capsule: impl IntoElement,
) -> AnyElement {
    glass_capsule_row(
        group,
        0,
        radius_px,
        div().min_w(px(0.0)).flex().flex_row().items_center(),
        [capsule.into_any_element()],
    )
}

pub(crate) fn remove_glass_capsule_group(window: &Window, group: &'static str) {
    #[cfg(target_os = "macos")]
    crate::platform::glass_button_host::remove_group(window, group);
    #[cfg(not(target_os = "macos"))]
    let _ = (window, group);
}

pub(crate) fn remove_glass_capsule_window(window: &Window) {
    #[cfg(target_os = "macos")]
    crate::platform::glass_button_host::remove_for_window(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub(crate) const FOOTER_CHIP_BORDER_ALPHA: f32 = 0.18;
pub(crate) const FOOTER_CHIP_BORDER_HOVER_ALPHA: f32 = 0.34;
pub(crate) const FOOTER_CHIP_BORDER_SELECTED_ALPHA: f32 = 0.40;
pub(crate) const FOOTER_MIC_ICON_TOKEN: &str = "mic";
// Embedded Lucide asset paths, resolved by AppAssets via svg().path().
// Compile-time CARGO_MANIFEST_DIR/vendor filesystem paths broke in released
// bundles — they point at the CI runner (P0 2026-06-11).
pub(crate) const FOOTER_MIC_ICON_PATH: &str = "icons/mic.svg";
pub(crate) const FOOTER_PROFILE_ICON_TOKEN: &str = "bot";
pub(crate) const FOOTER_PROFILE_ICON_PATH: &str = "icons/bot.svg";
pub(crate) const FOOTER_STATUS_DOT_SIZE_PX: f32 = 6.0;
pub(crate) const FOOTER_LEFT_INFO_GAP_PX: f32 = 6.0;
pub(crate) const FOOTER_BRAILLE_SPINNER_FONT_PX: f32 = 15.0;
pub(crate) const FOOTER_BRAILLE_SPINNER_LANE_PX: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum FooterActionSlot {
    Run,
    Actions,
    Ai,
    Apply,
    Replace,
    Append,
    Copy,
    Expand,
    Retry,
    Close,
    Stop,
    PasteResponse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FooterRailChrome {
    pub height_px: f32,
    pub side_inset_px: f32,
    pub item_gap_px: f32,
    pub surface_rgba: u32,
    pub divider_rgba: u32,
    pub hover_rgba: u32,
    pub active_rgba: u32,
    pub button_radius_px: f32,
}

pub(crate) enum FooterHintKeyMode {
    Shortcut,
}

pub(crate) struct FooterHintButtonSpec {
    pub(crate) label: SharedString,
    pub(crate) key: SharedString,
    pub(crate) slot_width_px: Option<f32>,
    pub(crate) key_first: bool,
    pub(crate) justify: FooterHintContentJustify,
    pub(crate) label_font_size_px: Option<f32>,
    pub(crate) keycap_font_size_px: Option<f32>,
    pub(crate) keycap_height_px: Option<f32>,
    pub(crate) hover_text_alpha: Option<u32>,
    pub(crate) hover_glyph_alpha: Option<u32>,
    pub(crate) hover_keycap_border_alpha: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FooterHintButtonLayoutOverrides {
    pub(crate) button_padding_x_px: Option<f32>,
    pub(crate) button_padding_y_px: Option<f32>,
    pub(crate) content_gap_px: Option<f32>,
    pub(crate) button_radius_px: Option<f32>,
    pub(crate) edge_padding_x_px: Option<f32>,
    pub(crate) shrink_frame_to_content_px: bool,
    /// When true, the OUTER slot also hugs the rendered content (bounded by
    /// `slot_width_px` as a max) instead of reserving the fixed slot width.
    /// Use for footers that must stay whole in narrow windows; fixed-slot
    /// grids (main footer, modal action rows) leave this false.
    pub(crate) hug_frame_to_content: bool,
}

pub(crate) struct FooterHintActionButtonFrameSpec {
    pub(crate) id: &'static str,
    pub(crate) label: SharedString,
    pub(crate) key: SharedString,
    pub(crate) slot_width_px: f32,
    pub(crate) height_px: f32,
    pub(crate) selected: bool,
    pub(crate) key_first: bool,
    pub(crate) justify: FooterHintContentJustify,
    pub(crate) layout: FooterHintButtonLayoutOverrides,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FooterHintContentJustify {
    Start,
    Center,
    KeyAnchored,
}

pub(crate) fn footer_action_slot_width(slot: FooterActionSlot) -> f32 {
    let metrics = current_main_menu_footer_metrics();
    footer_action_slot_width_for_metrics(slot, metrics)
}

pub(crate) fn footer_action_slot_width_for_metrics(
    slot: FooterActionSlot,
    metrics: crate::designs::FooterMetricsTokens,
) -> f32 {
    match slot {
        FooterActionSlot::Run => metrics.run_slot_min_width,
        FooterActionSlot::Actions => metrics.actions_slot_width,
        FooterActionSlot::Ai => metrics.ai_slot_width,
        FooterActionSlot::Apply => metrics.apply_slot_width,
        FooterActionSlot::Replace => metrics.apply_slot_width,
        FooterActionSlot::Append => metrics.apply_slot_width,
        FooterActionSlot::Copy => metrics.apply_slot_width,
        FooterActionSlot::Expand => metrics.apply_slot_width,
        FooterActionSlot::Retry => metrics.stop_slot_width,
        FooterActionSlot::Close => metrics.close_slot_width,
        FooterActionSlot::Stop => metrics.stop_slot_width,
        FooterActionSlot::PasteResponse => metrics.paste_response_slot_width,
    }
}

pub(crate) fn current_main_menu_footer_metrics() -> crate::designs::FooterMetricsTokens {
    crate::designs::current_main_menu_theme()
        .def()
        .footer
        .metrics
}

pub(crate) fn current_main_menu_footer_height() -> f32 {
    current_main_menu_footer_metrics().height_px
}

pub(crate) fn current_main_menu_footer_appkit_font_weight() -> f64 {
    let gpui_weight = current_main_menu_footer_metrics().font_weight.0;
    ((gpui_weight - 400.0) / 700.0).clamp(-1.0, 1.0) as f64
}

pub(crate) fn footer_rail_chrome(theme: &Theme) -> FooterRailChrome {
    let chrome = crate::theme::AppChromeColors::from_theme(theme);
    let metrics = current_main_menu_footer_metrics();
    FooterRailChrome {
        height_px: metrics.height_px,
        side_inset_px: metrics.side_inset_px,
        item_gap_px: metrics.item_gap_px,
        surface_rgba: chrome.inline_dropdown_surface_rgba,
        divider_rgba: chrome.divider_rgba,
        hover_rgba: chrome.hover_rgba,
        active_rgba: chrome.selection_rgba,
        button_radius_px: metrics.button_radius,
    }
}

/// Canonical in-window footer rail geometry. Callers supply footer-chrome
/// button frames; this helper keeps the glass fallback aligned with the
/// native footer's height, side insets, and inter-item spacing.
pub(crate) fn render_footer_action_rail(
    id: &'static str,
    buttons: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    render_footer_action_rail_with_leading(id, None, buttons)
}

/// Rail variant with a left-info slot: `leading` (status text, spinner, …)
/// sits at the left inset while the buttons stay right-aligned, mirroring the
/// native footer's left-info + actions anatomy.
pub(crate) fn render_footer_action_rail_with_leading(
    id: &'static str,
    leading: Option<AnyElement>,
    buttons: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    render_footer_action_rail_with_leading_and_cleanup(id, leading, buttons, Vec::new())
}

fn render_footer_action_rail_with_leading_and_cleanup(
    id: &'static str,
    leading: Option<AnyElement>,
    buttons: impl IntoIterator<Item = AnyElement>,
    groups_to_remove: Vec<&'static str>,
) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let rail = footer_rail_chrome(&theme);
    let has_leading = leading.is_some();
    let buttons = buttons.into_iter().collect::<Vec<_>>();

    let glass_buttons_enabled = glass_capsules_enabled();

    let item_gap_px = if glass_buttons_enabled {
        FOOTER_GLASS_BUTTON_GAP_PX
    } else {
        rail.item_gap_px
    };

    let rail_element = div()
        .w_full()
        .h(px(rail.height_px))
        .min_h(px(rail.height_px))
        .px(px(rail.side_inset_px))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(item_gap_px))
        .when_some(leading, |rail, leading| {
            rail.child(
                div()
                    .flex_none()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .child(leading),
            )
        })
        .child(div().flex_1());

    glass_capsule_row_with_cleanup(
        id,
        usize::from(has_leading) + 1,
        Some(rail.button_radius_px),
        rail_element,
        buttons,
        groups_to_remove,
    )
}

fn footer_config_action_slot(action: crate::footer_popup::FooterAction) -> FooterActionSlot {
    use crate::footer_popup::FooterAction;

    match action {
        FooterAction::Run => FooterActionSlot::Run,
        FooterAction::Actions => FooterActionSlot::Actions,
        FooterAction::Ai | FooterAction::Cwd | FooterAction::AgentModel | FooterAction::Tips => {
            FooterActionSlot::Ai
        }
        FooterAction::Apply => FooterActionSlot::Apply,
        FooterAction::Replace => FooterActionSlot::Replace,
        FooterAction::Append => FooterActionSlot::Append,
        FooterAction::Copy => FooterActionSlot::Copy,
        FooterAction::Expand => FooterActionSlot::Expand,
        FooterAction::Retry => FooterActionSlot::Retry,
        FooterAction::Close => FooterActionSlot::Close,
        FooterAction::Stop => FooterActionSlot::Stop,
        FooterAction::PasteResponse => FooterActionSlot::PasteResponse,
    }
}

fn footer_config_button_is_left_pinned(button: &crate::footer_popup::FooterButtonConfig) -> bool {
    use crate::footer_popup::FooterAction;

    button.left_pinned
        || matches!(button.action, FooterAction::Cwd | FooterAction::AgentModel)
        || (matches!(button.action, FooterAction::Ai)
            && button.key.as_ref() == FOOTER_MIC_ICON_TOKEN)
}

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
    let hover_bg = rgba(themed_footer_button_hover_rgba(theme));
    let active_bg = rgba(themed_footer_button_active_rgba(theme));
    let hover_text = footer_hover_text_color(theme, None);
    let hover_glyph = footer_hover_glyph_color(theme, None);
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
            .hover(move |style| style.bg(hover_bg))
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
                .text_color(footer_hint_text_color(theme))
                .group_hover("config-footer-left-marker", move |style| {
                    style.text_color(hover_glyph)
                }),
        );
    }
    if let Some(keycap) = keycap.filter(|key| !key.trim().is_empty()) {
        marker = marker.child(render_footer_shortcut_keycaps_with_metrics(
            keycap.to_string(),
            theme,
            None,
            None,
            None,
            None,
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
                .text_color(footer_hint_text_color(theme))
                .group_hover("config-footer-left-marker", move |style| {
                    style.text_color(hover_text)
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

/// Render a complete native-footer config inside a GPUI window. This is the
/// sole in-window glass footer renderer: surfaces resolve the same config the
/// AppKit footer consumes, then supply only their action dispatcher.
#[allow(dead_code)] // callers live in binary-only render modules
pub(crate) fn render_main_window_footer_config_rail<H>(
    config: crate::footer_popup::MainWindowFooterConfig,
    on_action: H,
) -> AnyElement
where
    H: Fn(crate::footer_popup::FooterAction, &mut Window, &mut App) + Clone + 'static,
{
    let theme = crate::theme::get_cached_theme();
    let rail = footer_rail_chrome(&theme);
    let button_height = footer_button_height(rail.height_px);
    let button_ids = [
        "config-footer-button-0",
        "config-footer-button-1",
        "config-footer-button-2",
        "config-footer-button-3",
        "config-footer-button-4",
        "config-footer-button-overflow",
    ];
    let mut left_buttons = Vec::new();
    let mut right_buttons = Vec::new();
    let mut active_left_info_groups = Vec::new();
    if let Some(info) = config.left_info.as_ref() {
        if info.cwd_chip.is_some() {
            active_left_info_groups.push("config-footer-cwd-chip");
        }
        if info.profile_name.is_some() {
            if info.action.is_some() {
                active_left_info_groups.push("config-footer-profile-chip");
            }
            active_left_info_groups.push("config-footer-model-chip");
        } else if info.action.is_some() {
            active_left_info_groups.push("config-footer-left-info");
        }
    }
    let stale_left_info_groups = [
        "config-footer-cwd-chip",
        "config-footer-profile-chip",
        "config-footer-model-chip",
        "config-footer-left-info",
    ]
    .into_iter()
    .filter(|group| !active_left_info_groups.contains(group))
    .collect::<Vec<_>>();

    for (index, button) in config.buttons.into_iter().enumerate() {
        let action = button.action;
        let enabled = button.enabled && button.disabled_reason.is_none();
        let left_pinned = footer_config_button_is_left_pinned(&button);
        let key_first = left_pinned
            && !matches!(
                action,
                crate::footer_popup::FooterAction::Cwd
                    | crate::footer_popup::FooterAction::AgentModel
            );
        let justify = if left_pinned {
            FooterHintContentJustify::Start
        } else if matches!(action, crate::footer_popup::FooterAction::Run) {
            FooterHintContentJustify::KeyAnchored
        } else {
            FooterHintContentJustify::Center
        };
        let mut frame = render_footer_hint_action_button_frame(
            FooterHintActionButtonFrameSpec {
                id: button_ids
                    .get(index)
                    .copied()
                    .unwrap_or("config-footer-button-overflow"),
                label: button.label,
                key: button.key,
                slot_width_px: footer_action_slot_width(footer_config_action_slot(action)),
                height_px: button_height,
                selected: button.selected,
                key_first,
                justify,
                layout: FooterHintButtonLayoutOverrides {
                    shrink_frame_to_content_px: true,
                    hug_frame_to_content: true,
                    ..FooterHintButtonLayoutOverrides::default()
                },
            },
            &theme,
        );
        if enabled {
            let handler = on_action.clone();
            frame = frame.on_click(move |_event, window, cx| handler(action, window, cx));
        } else {
            frame = frame.opacity(0.45);
        }
        if left_pinned {
            left_buttons.push(frame.into_any_element());
        } else {
            right_buttons.push(frame.into_any_element());
        }
    }

    let left_info = config
        .left_info
        .map(|info| render_footer_config_left_info(info, &theme, on_action));
    let leading = div()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .items_center()
        .gap(px(rail.item_gap_px))
        .overflow_hidden()
        .children(left_buttons)
        .children(left_info)
        .into_any_element();

    render_footer_action_rail_with_leading_and_cleanup(
        "main-window-footer-config-rail",
        Some(leading),
        right_buttons,
        stale_left_info_groups,
    )
}

#[allow(dead_code)] // callers live in binary-only render modules
fn footer_hint_button_parts(hint: &str) -> (SharedString, SharedString) {
    let Some((candidate_key, label)) = hint.trim().split_once(' ') else {
        return (
            SharedString::from(hint.trim().to_owned()),
            SharedString::from(""),
        );
    };
    let is_shortcut = matches!(
        candidate_key.to_ascii_lowercase().as_str(),
        "esc" | "escape"
    ) || candidate_key
        .chars()
        .any(|ch| matches!(ch, '↵' | '⇥' | '⌘' | '⇧' | '⌥' | '⌃'));
    if is_shortcut {
        (
            SharedString::from(label.to_owned()),
            SharedString::from(candidate_key.to_owned()),
        )
    } else {
        (
            SharedString::from(hint.trim().to_owned()),
            SharedString::from(""),
        )
    }
}

/// Static footer hints rendered with the same frames and optical keycap
/// metrics as clickable footer actions. Composite hint strings retain each
/// advertised shortcut as its own footer button.
#[allow(dead_code)] // callers live in binary-only render modules
pub(crate) fn render_static_footer_hint_action_rail(
    id: &'static str,
    hints: impl IntoIterator<Item = SharedString>,
) -> AnyElement {
    render_static_footer_hint_action_rail_with_leading(id, hints, None)
}

/// Static hint rail with the native footer's left-info slot: `leading`
/// (contextual status text) rides at the left inset, hints render as shared
/// footer button frames on the right.
#[allow(dead_code)] // callers live in binary-only render modules
pub(crate) fn render_static_footer_hint_action_rail_with_leading(
    id: &'static str,
    hints: impl IntoIterator<Item = SharedString>,
    leading: Option<AnyElement>,
) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let rail = footer_rail_chrome(&theme);
    let button_height = footer_button_height(rail.height_px);
    let button_ids = [
        "static-footer-hint-0",
        "static-footer-hint-1",
        "static-footer-hint-2",
        "static-footer-hint-3",
        "static-footer-hint-4",
        "static-footer-hint-5",
        "static-footer-hint-6",
        "static-footer-hint-7",
    ];
    let mut buttons = Vec::new();

    for segment in hints
        .into_iter()
        .flat_map(|hint| hint.split(" · ").map(str::to_owned).collect::<Vec<_>>())
    {
        let index = buttons.len();
        let button_id = button_ids
            .get(index)
            .copied()
            .unwrap_or("static-footer-hint-overflow");
        let (label, key) = footer_hint_button_parts(&segment);
        buttons.push(
            render_footer_hint_action_button_frame(
                FooterHintActionButtonFrameSpec {
                    id: button_id,
                    label,
                    key,
                    slot_width_px: footer_action_slot_width(FooterActionSlot::Run),
                    height_px: button_height,
                    selected: false,
                    key_first: false,
                    justify: FooterHintContentJustify::Center,
                    layout: FooterHintButtonLayoutOverrides {
                        shrink_frame_to_content_px: true,
                        hug_frame_to_content: true,
                        ..FooterHintButtonLayoutOverrides::default()
                    },
                },
                &theme,
            )
            .into_any_element(),
        );
    }

    render_footer_action_rail_with_leading(id, leading, buttons)
}

pub(crate) type FooterHintClickHandler =
    Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>;

/// Clickable variant of the static hint rail: each hint renders as a shared
/// footer button frame and fires its handler on click. Keeps clickable
/// prompt footers (template/textarea/drop) on the same footer language as
/// every other in-window glass footer.
#[allow(dead_code)] // callers live in binary-only render modules
pub(crate) fn render_clickable_footer_hint_action_rail(
    id: &'static str,
    hints: impl IntoIterator<Item = (SharedString, Option<FooterHintClickHandler>)>,
) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let rail = footer_rail_chrome(&theme);
    let button_height = footer_button_height(rail.height_px);
    let button_ids = [
        "clickable-footer-hint-0",
        "clickable-footer-hint-1",
        "clickable-footer-hint-2",
        "clickable-footer-hint-3",
        "clickable-footer-hint-4",
        "clickable-footer-hint-5",
    ];
    let mut buttons = Vec::new();

    for (hint, handler) in hints {
        let index = buttons.len();
        let button_id = button_ids
            .get(index)
            .copied()
            .unwrap_or("clickable-footer-hint-overflow");
        let (label, key) = footer_hint_button_parts(hint.as_ref());
        let frame = render_footer_hint_action_button_frame(
            FooterHintActionButtonFrameSpec {
                id: button_id,
                label,
                key,
                slot_width_px: footer_action_slot_width(FooterActionSlot::Run),
                height_px: button_height,
                selected: false,
                key_first: false,
                justify: FooterHintContentJustify::Center,
                layout: FooterHintButtonLayoutOverrides {
                    shrink_frame_to_content_px: true,
                    hug_frame_to_content: true,
                    ..FooterHintButtonLayoutOverrides::default()
                },
            },
            &theme,
        );
        buttons.push(match handler {
            Some(handler) => frame
                .on_click(move |event, window, cx| handler(event, window, cx))
                .into_any_element(),
            None => frame.into_any_element(),
        });
    }

    render_footer_action_rail(id, buttons)
}

fn current_footer_button_theme_rgba(theme: &Theme, alpha: u32) -> u32 {
    let def = crate::designs::current_main_menu_theme().def();
    let chrome = crate::theme::AppChromeColors::from_theme(theme);
    let hex = if def.footer.button.uses_accent {
        chrome.accent_hex
    } else {
        theme.colors.text.primary
    };
    (hex << 8) | alpha
}

pub(crate) fn themed_footer_button_rest_rgba(theme: &Theme) -> Option<u32> {
    crate::designs::current_main_menu_theme()
        .def()
        .footer
        .button
        .rest
        .map(|alpha| current_footer_button_theme_rgba(theme, alpha))
}

pub(crate) fn themed_footer_button_hover_rgba(theme: &Theme) -> u32 {
    let alpha = crate::designs::current_main_menu_theme()
        .def()
        .footer
        .button
        .hover;
    current_footer_button_theme_rgba(theme, alpha)
}

pub(crate) fn themed_footer_button_active_rgba(theme: &Theme) -> u32 {
    let alpha = crate::designs::current_main_menu_theme()
        .def()
        .footer
        .button
        .active;
    current_footer_button_theme_rgba(theme, alpha)
}

pub(crate) fn themed_footer_button_border_alpha(theme: &Theme, selected: bool) -> f32 {
    let theme_alpha = crate::designs::current_main_menu_theme()
        .def()
        .footer
        .button
        .border_alpha as f32
        / 255.0;
    footer_keycap_border_alpha(theme, selected).max(theme_alpha)
}

fn normalize_footer_key_token(token: &str) -> String {
    match token.trim().to_lowercase().as_str() {
        "esc" | "escape" => "⎋".to_string(),
        _ => token.to_string(),
    }
}

pub(crate) fn footer_keycap_border_alpha(theme: &Theme, selected: bool) -> f32 {
    let opacity = theme.get_opacity();
    if selected {
        opacity.selected.max(FOOTER_CHIP_BORDER_SELECTED_ALPHA)
    } else {
        opacity.hover.max(FOOTER_CHIP_BORDER_ALPHA)
    }
}

pub(crate) fn footer_keycap_border_hover_alpha(theme: &Theme) -> f32 {
    let design_alpha = crate::designs::current_main_menu_theme()
        .def()
        .footer
        .button
        .hover_border_alpha as f32
        / 255.0;
    theme
        .get_opacity()
        .hover
        .max(FOOTER_CHIP_BORDER_HOVER_ALPHA)
        .max(design_alpha)
}

pub(crate) fn footer_hint_text_color(theme: &Theme) -> gpui::Rgba {
    theme
        .colors
        .text
        .primary
        .with_opacity(OPACITY_TEXT_MUTED)
        .to_rgb()
}

pub(crate) fn footer_keycap_border_color_for_state(theme: &Theme, selected: bool) -> gpui::Hsla {
    let alpha = themed_footer_button_border_alpha(theme, selected);
    theme.colors.text.primary.with_opacity(alpha)
}

pub(crate) fn footer_keycap_border_color(theme: &Theme) -> gpui::Hsla {
    footer_keycap_border_color_for_state(theme, false)
}

pub(crate) fn split_footer_shortcut(shortcut: &str) -> Vec<String> {
    let s = shortcut.trim();
    if s.is_empty() {
        return Vec::new();
    }
    if s.contains('+') {
        let mut tokens: Vec<String> = s
            .split('+')
            .filter(|part| !part.is_empty())
            .map(normalize_footer_key_token)
            .collect();
        // A trailing '+' is the plus key itself ("⌘+" is Cmd-Plus,
        // "Ctrl++" is Ctrl-Plus), not a separator before an empty keycap.
        if s.ends_with('+') {
            tokens.push("+".to_string());
        }
        return tokens;
    }
    if s.chars().any(char::is_whitespace) {
        return s
            .split_whitespace()
            .map(normalize_footer_key_token)
            .collect();
    }

    let lower = s.to_lowercase();
    let known_words = [
        "enter",
        "return",
        "space",
        "tab",
        "esc",
        "escape",
        "backspace",
        "del",
        "delete",
        "up",
        "down",
        "left",
        "right",
        "home",
        "end",
        "cmd",
        "ctrl",
        "alt",
        "shift",
        "win",
        "click",
    ];
    if known_words.contains(&lower.as_str()) {
        return vec![normalize_footer_key_token(s)];
    }

    let mut tokens = Vec::new();
    let mut text_run = String::new();
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            text_run.push(ch);
        } else {
            if !text_run.is_empty() {
                tokens.push(normalize_footer_key_token(&text_run));
                text_run.clear();
            }
            tokens.push(ch.to_string());
        }
    }
    if !text_run.is_empty() {
        tokens.push(normalize_footer_key_token(&text_run));
    }
    tokens
}

pub(crate) fn is_footer_return_key_glyph(key: &str) -> bool {
    matches!(key, "↵")
}

/// Multi-char word keycaps ("Space", "Enter") render with word padding and
/// the word baseline nudge; single glyphs keep the square min-width chip.
pub(crate) fn is_footer_word_key_token(key: &str) -> bool {
    key.chars().count() > 1
}

pub(crate) fn footer_key_glyph_nudge_y(key: &str) -> f32 {
    let metrics = current_main_menu_footer_metrics();
    if is_footer_return_key_glyph(key) {
        metrics.key_glyph_nudge_y + metrics.return_glyph_nudge_y
    } else if key == ";" {
        metrics.semicolon_glyph_nudge_y
    } else if key == "⌘" {
        metrics.cmd_glyph_nudge_y
    } else if is_footer_word_key_token(key) {
        metrics.word_glyph_nudge_y
    } else {
        metrics.key_glyph_nudge_y
    }
}

/// Optical horizontal correction; only ⌘ needs one today (its ink sits left
/// of its advance-box center at keycap sizes).
pub(crate) fn footer_key_glyph_nudge_x(key: &str) -> f32 {
    if key == "⌘" {
        current_main_menu_footer_metrics().cmd_glyph_nudge_x
    } else {
        0.0
    }
}

fn footer_keycap_padding_x_for_token(
    token: &str,
    metrics: &crate::designs::FooterMetricsTokens,
) -> f32 {
    if is_footer_word_key_token(token) && !is_footer_icon_token(token) {
        metrics.word_keycap_padding_x
    } else {
        metrics.keycap_padding_x
    }
}

pub(crate) fn footer_appkit_glyph_y(key: &str, chip_height: f64, glyph_height: f64) -> f64 {
    let centered_y = ((chip_height - glyph_height) / 2.0).round();
    centered_y - footer_key_glyph_nudge_y(key) as f64
}

/// Single formula owner for footer action-button height: host band minus
/// vertical button padding. Explicit-padding variant so deterministic
/// resolvers (e.g. the design-contract exporter) can pass base footer
/// metrics instead of the runtime-override-sensitive current metrics.
pub(crate) fn footer_button_height_in(footer_height: f32, button_padding_y: f32) -> f32 {
    (footer_height - (button_padding_y * 2.0)).max(0.0)
}

pub(crate) fn footer_button_height(footer_height: f32) -> f32 {
    let metrics = current_main_menu_footer_metrics();
    footer_button_height_in(footer_height, metrics.button_padding_y)
}

pub(crate) fn footer_centered_action_edge_padding_x() -> f32 {
    current_main_menu_footer_metrics().button_padding_x
        + FOOTER_TRAILING_ACTION_EXTRA_PADDING_X_PX / 2.0
}

pub(crate) fn footer_centered_action_button_layout() -> FooterHintButtonLayoutOverrides {
    let metrics = current_main_menu_footer_metrics();
    FooterHintButtonLayoutOverrides {
        button_padding_x_px: Some(metrics.button_padding_x),
        button_padding_y_px: Some(metrics.button_padding_y),
        content_gap_px: Some(metrics.content_gap),
        button_radius_px: Some(metrics.button_radius),
        edge_padding_x_px: Some(footer_centered_action_edge_padding_x()),
        shrink_frame_to_content_px: false,
        hug_frame_to_content: false,
    }
}

pub(crate) fn render_footer_hint_content(
    label: SharedString,
    key: SharedString,
    mode: FooterHintKeyMode,
    theme: &Theme,
) -> AnyElement {
    render_footer_hint_content_impl(
        label,
        key,
        mode,
        theme,
        None,
        false,
        FooterHintContentJustify::Center,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        FooterHintButtonLayoutOverrides::default(),
    )
}

/// Flexbox-native footer hint content. Instead of estimating text width in
/// Rust and truncating against a precomputed slot width, the label is a
/// shrinkable flex item that ellipsizes under real layout pressure while the
/// keycaps keep their intrinsic size. Callers control overall pressure with
/// `min_w`/`max_w` on the button container; no slot math is required.
pub(crate) fn render_footer_hint_content_flex(
    label: SharedString,
    key: SharedString,
    mode: FooterHintKeyMode,
    theme: &Theme,
    key_first: bool,
    justify: FooterHintContentJustify,
) -> AnyElement {
    render_footer_hint_content_flex_with_layout(
        label,
        key,
        mode,
        theme,
        key_first,
        justify,
        FooterHintButtonLayoutOverrides::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_footer_hint_content_flex_with_layout(
    label: SharedString,
    key: SharedString,
    mode: FooterHintKeyMode,
    theme: &Theme,
    key_first: bool,
    justify: FooterHintContentJustify,
    layout: FooterHintButtonLayoutOverrides,
) -> AnyElement {
    let footer_text = footer_hint_text_color(theme);
    let hover_text = footer_hover_text_color(theme, None);
    let hover_glyph = footer_hover_glyph_color(theme, None);
    let metrics = current_main_menu_footer_metrics();
    let default_edge_padding_x = match justify {
        FooterHintContentJustify::KeyAnchored => metrics.run_button_padding_x,
        // Trailing centered action buttons keep the same comfortable inner x
        // padding the native AppKit footer reserves via
        // `footer_hint_legacy_extra_padding` (half of the extra per side on
        // top of the base button padding).
        FooterHintContentJustify::Center => footer_centered_action_edge_padding_x(),
        FooterHintContentJustify::Start => metrics.button_padding_x,
    };
    let edge_padding_x = layout
        .edge_padding_x_px
        .or(layout.button_padding_x_px)
        .unwrap_or(default_edge_padding_x);

    let labelcap = render_footer_labelcap_constrained(
        label,
        theme,
        footer_text,
        hover_text,
        None,
        false,
        None,
        true,
    );
    let content_gap = layout.content_gap_px.unwrap_or(metrics.content_gap);
    let keycaps = match mode {
        FooterHintKeyMode::Shortcut => render_footer_shortcut_keycaps_with_metrics(
            key.to_string(),
            theme,
            None,
            None,
            Some(content_gap),
            Some(FooterKeycapHoverStyle {
                text: hover_text,
                glyph: hover_glyph,
                border_alpha: None,
            }),
        ),
    };
    let keycaps = div().flex_none().child(keycaps);

    let mut row = div()
        .min_w(px(0.0))
        .overflow_hidden()
        .pl(px(edge_padding_x))
        .pr(px(edge_padding_x))
        .py(px(layout
            .button_padding_y_px
            .unwrap_or(metrics.button_padding_y)))
        .rounded(px(layout.button_radius_px.unwrap_or(metrics.button_radius)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(content_gap))
        .group("footer-action-button");

    row = match justify {
        FooterHintContentJustify::Start | FooterHintContentJustify::KeyAnchored => {
            row.justify_start()
        }
        FooterHintContentJustify::Center => row.justify_center(),
    };

    if key_first {
        row.child(keycaps).child(labelcap).into_any_element()
    } else {
        row.child(labelcap).child(keycaps).into_any_element()
    }
}

pub(crate) fn render_footer_hint_button_like(
    spec: FooterHintButtonSpec,
    theme: &Theme,
) -> AnyElement {
    render_footer_hint_button_like_with_layout(
        spec,
        FooterHintButtonLayoutOverrides::default(),
        theme,
    )
}

pub(crate) fn render_footer_hint_button_like_with_layout(
    spec: FooterHintButtonSpec,
    layout: FooterHintButtonLayoutOverrides,
    theme: &Theme,
) -> AnyElement {
    render_footer_hint_button_like_with_layout_and_shrinkable_label(spec, layout, false, theme)
}

/// Footer-language hint content whose label yields to its keycaps under real
/// flex pressure. Header context chips use this so long cwd/profile labels
/// ellipsize inside their lane instead of painting across the opposite lane.
pub(crate) fn render_footer_hint_button_like_shrinkable(
    spec: FooterHintButtonSpec,
    theme: &Theme,
) -> AnyElement {
    render_footer_hint_button_like_with_layout_and_shrinkable_label(
        spec,
        FooterHintButtonLayoutOverrides::default(),
        true,
        theme,
    )
}

fn render_footer_hint_button_like_with_layout_and_shrinkable_label(
    spec: FooterHintButtonSpec,
    layout: FooterHintButtonLayoutOverrides,
    shrinkable_label: bool,
    theme: &Theme,
) -> AnyElement {
    render_footer_hint_content_impl(
        spec.label,
        spec.key,
        FooterHintKeyMode::Shortcut,
        theme,
        spec.slot_width_px,
        spec.key_first,
        spec.justify,
        spec.label_font_size_px,
        spec.keycap_font_size_px,
        spec.keycap_height_px,
        spec.hover_text_alpha,
        spec.hover_glyph_alpha,
        spec.hover_keycap_border_alpha,
        shrinkable_label,
        layout,
    )
}

pub(crate) fn render_footer_hint_action_button_frame(
    spec: FooterHintActionButtonFrameSpec,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    let metrics = current_main_menu_footer_metrics();
    let radius = spec
        .layout
        .button_radius_px
        .unwrap_or(metrics.button_radius);
    let default_edge_padding_x = match spec.justify {
        FooterHintContentJustify::KeyAnchored => metrics.run_button_padding_x,
        FooterHintContentJustify::Center => footer_centered_action_edge_padding_x(),
        FooterHintContentJustify::Start => metrics.button_padding_x,
    };
    let edge_padding_x = spec
        .layout
        .edge_padding_x_px
        .or(spec.layout.button_padding_x_px)
        .unwrap_or(default_edge_padding_x);
    let content_layout = FooterHintButtonLayoutOverrides {
        button_padding_x_px: Some(edge_padding_x),
        ..spec.layout
    };
    let hover_bg = gpui::rgba(themed_footer_button_hover_rgba(theme));
    let active_bg = gpui::rgba(themed_footer_button_active_rgba(theme));

    // Flexbox-native frame: when shrinking to content, the highlight pill hugs
    // the rendered label + keycaps (no estimated text widths), bounded by the
    // slot. Otherwise it fills the fixed slot exactly as before.
    let shrink_to_content = spec.layout.shrink_frame_to_content_px;
    let content = if shrink_to_content {
        render_footer_hint_content_flex_with_layout(
            spec.label,
            spec.key,
            FooterHintKeyMode::Shortcut,
            theme,
            spec.key_first,
            spec.justify,
            content_layout,
        )
    } else {
        render_footer_hint_button_like_with_layout(
            FooterHintButtonSpec {
                label: spec.label,
                key: spec.key,
                slot_width_px: Some(spec.slot_width_px),
                key_first: spec.key_first,
                justify: spec.justify,
                label_font_size_px: None,
                keycap_font_size_px: None,
                keycap_height_px: None,
                hover_text_alpha: None,
                hover_glyph_alpha: None,
                hover_keycap_border_alpha: None,
            },
            content_layout,
            theme,
        )
    };

    let hug_frame = spec.layout.hug_frame_to_content;
    div()
        .id(spec.id)
        .when(!hug_frame, |style| style.w(px(spec.slot_width_px)))
        // Hug mode: intrinsic content width that never shrinks or truncates —
        // a footer button that ellipsizes its label or keycap is useless.
        .when(hug_frame, |style| style.flex_none())
        .h(px(spec.height_px))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .group("footer-action-button-slot")
        .child(
            div()
                .when(!shrink_to_content, |style| style.w(px(spec.slot_width_px)))
                .when(!hug_frame, |style| style.max_w(px(spec.slot_width_px)))
                .when(hug_frame, |style| style.flex_none())
                .min_w(px(0.0))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .rounded(px(radius))
                .when(spec.selected, |style| style.bg(active_bg))
                .group_hover("footer-action-button-slot", move |style| style.bg(hover_bg))
                .child(content),
        )
}

pub(crate) fn footer_hover_text_color(theme: &Theme, alpha: Option<u32>) -> gpui::Hsla {
    let alpha = alpha.unwrap_or_else(|| {
        crate::designs::current_main_menu_theme()
            .def()
            .footer
            .button
            .hover_text_alpha
    });
    theme
        .colors
        .text
        .primary
        .with_opacity((alpha as f32 / 255.0).clamp(0.0, 1.0))
}

pub(crate) fn footer_hover_glyph_color(theme: &Theme, alpha: Option<u32>) -> gpui::Hsla {
    let alpha = alpha.unwrap_or_else(|| {
        crate::designs::current_main_menu_theme()
            .def()
            .footer
            .button
            .hover_glyph_alpha
    });
    theme
        .colors
        .text
        .primary
        .with_opacity((alpha as f32 / 255.0).clamp(0.0, 1.0))
}

fn footer_keycap_border_hover_color_with_alpha(theme: &Theme, alpha: Option<u32>) -> gpui::Hsla {
    let alpha = alpha
        .map(|alpha| (alpha as f32 / 255.0).clamp(0.0, 1.0))
        .unwrap_or_else(|| footer_keycap_border_hover_alpha(theme));
    theme.colors.text.primary.with_opacity(alpha)
}

#[allow(clippy::too_many_arguments)]
fn render_footer_hint_content_impl(
    label: SharedString,
    key: SharedString,
    mode: FooterHintKeyMode,
    theme: &Theme,
    slot_width_px: Option<f32>,
    key_first: bool,
    justify: FooterHintContentJustify,
    label_font_size_px: Option<f32>,
    keycap_font_size_px: Option<f32>,
    keycap_height_px: Option<f32>,
    hover_text_alpha: Option<u32>,
    hover_glyph_alpha: Option<u32>,
    hover_keycap_border_alpha: Option<u32>,
    shrinkable_label: bool,
    layout: FooterHintButtonLayoutOverrides,
) -> AnyElement {
    let footer_text = footer_hint_text_color(theme);
    let hover_text = footer_hover_text_color(theme, hover_text_alpha);
    let hover_glyph = footer_hover_glyph_color(theme, hover_glyph_alpha);
    let metrics = current_main_menu_footer_metrics();
    let default_edge_padding_x = match justify {
        FooterHintContentJustify::KeyAnchored => metrics.run_button_padding_x,
        FooterHintContentJustify::Center => footer_centered_action_edge_padding_x(),
        FooterHintContentJustify::Start => metrics.button_padding_x,
    };
    let edge_padding_x = layout.button_padding_x_px.unwrap_or(default_edge_padding_x);
    let button_padding_y = layout
        .button_padding_y_px
        .unwrap_or(metrics.button_padding_y);
    let content_gap = layout.content_gap_px.unwrap_or(metrics.content_gap);
    let button_radius = layout.button_radius_px.unwrap_or(metrics.button_radius);
    // Flexbox-native slot layout: the keycaps keep their intrinsic flex_none
    // width and the label is a shrinkable flex item, so the label's budget is
    // whatever the slot leaves over — no estimated keycap text widths.
    let labelcap = if slot_width_px.is_some() || shrinkable_label {
        let shrinkable_labelcap = render_footer_labelcap_constrained(
            label,
            theme,
            footer_text,
            hover_text,
            None,
            false,
            label_font_size_px,
            true,
        );
        if slot_width_px.is_some() && matches!(justify, FooterHintContentJustify::KeyAnchored) {
            // Key-anchored: the label claims all leftover slot width so the
            // keycaps sit pinned at the slot's trailing edge.
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .flex()
                .justify_start()
                .child(shrinkable_labelcap)
                .into_any_element()
        } else {
            shrinkable_labelcap
        }
    } else {
        render_footer_labelcap(label, theme, footer_text, hover_text, label_font_size_px)
    };
    let keycaps = match mode {
        FooterHintKeyMode::Shortcut => render_footer_shortcut_keycaps_with_metrics(
            key.to_string(),
            theme,
            keycap_font_size_px,
            keycap_height_px,
            Some(content_gap),
            Some(FooterKeycapHoverStyle {
                text: hover_text,
                glyph: hover_glyph,
                border_alpha: hover_keycap_border_alpha,
            }),
        ),
    };
    let keycaps = if shrinkable_label {
        div().flex_none().child(keycaps).into_any_element()
    } else {
        keycaps
    };

    let mut row = div()
        .pl(px(edge_padding_x))
        .pr(px(edge_padding_x))
        .py(px(button_padding_y))
        .rounded(px(button_radius))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(content_gap))
        .group("footer-action-button")
        .min_w(px(0.0))
        .overflow_hidden();

    if shrinkable_label {
        row = row.w_full();
    }

    if let Some(slot_width_px) = slot_width_px {
        row = row.w_full().max_w(px(slot_width_px));
    }

    row = match justify {
        FooterHintContentJustify::Start => row.justify_start(),
        FooterHintContentJustify::Center => row.justify_center(),
        FooterHintContentJustify::KeyAnchored => row.justify_start(),
    };

    if key_first {
        row.child(keycaps).child(labelcap).into_any_element()
    } else {
        row.child(labelcap).child(keycaps).into_any_element()
    }
}

/// Measured keycap-run width: real glyph advances from the text system plus
/// the same paddings/minimums `render_footer_keycap_with_metrics` applies.
pub(crate) fn footer_shortcut_keycaps_measured_width_px(shortcut: &str, cx: &gpui::App) -> f32 {
    let metrics = current_main_menu_footer_metrics();
    let tokens = split_footer_shortcut(shortcut);
    if tokens.is_empty() {
        return 0.0;
    }

    let keys_width = tokens
        .iter()
        .map(|token| footer_keycap_measured_width_px(token, cx))
        .sum::<f32>();
    keys_width + tokens.len().saturating_sub(1) as f32 * metrics.content_gap
}

/// Measured width of a single keycap: real glyph advances from the text
/// system plus the same paddings/minimums `render_footer_keycap_with_metrics`
/// applies.
pub(crate) fn footer_keycap_measured_width_px(token: &str, cx: &gpui::App) -> f32 {
    let metrics = current_main_menu_footer_metrics();
    if is_footer_icon_token(token) {
        // Icon keycaps render an svg of (font_size + 1).max(10) inside
        // the keycap paddings, never narrower than the square keycap.
        let icon = (metrics.keycap_font_size + 1.0).max(10.0);
        return (icon + metrics.keycap_padding_x * 2.0).max(metrics.keycap_height);
    }

    let text_system = cx.text_system();
    let font_id = text_system.resolve_font(&gpui::font(FONT_SYSTEM_UI));
    let font_size = px(metrics.keycap_font_size);
    let glyphs_width: f32 = token
        .chars()
        .map(|ch| f32::from(text_system.layout_width(font_id, font_size, ch)))
        .sum();
    (glyphs_width + footer_keycap_padding_x_for_token(token, &metrics) * 2.0)
        .max(metrics.keycap_height)
        .ceil()
}

/// Total width of a horizontal run of items laid out with a constant gap
/// between adjacent items (no leading/trailing gap). This is the single source
/// of truth both the left-pinned chip group and the trailing action group use,
/// so the two sides advance with identical math.
#[cfg(test)]
pub(crate) fn footer_horizontal_run_width_px(widths: &[f32], gap_px: f32) -> f32 {
    if widths.is_empty() {
        return 0.0;
    }
    widths
        .iter()
        .copied()
        .map(|width| width.max(0.0))
        .sum::<f32>()
        + gap_px * widths.len().saturating_sub(1) as f32
}

/// Per-item left origins for a horizontal run advanced by `width + gap`, anchored
/// at `origin_x`. Origins are rounded to whole pixels at each boundary to match
/// the AppKit `NSPoint` rounding style and avoid subpixel drift.
#[cfg(test)]
pub(crate) fn footer_horizontal_run_origins_px(
    widths: &[f32],
    gap_px: f32,
    origin_x: f32,
) -> Vec<f32> {
    let mut x = origin_x;
    widths
        .iter()
        .map(|width| {
            let origin = x.round();
            x += width.max(0.0) + gap_px;
            origin
        })
        .collect()
}

pub(crate) fn is_footer_icon_token(token: &str) -> bool {
    footer_icon_path(token).is_some()
}

pub(crate) fn footer_icon_path(token: &str) -> Option<String> {
    match token {
        FOOTER_MIC_ICON_TOKEN => Some(FOOTER_MIC_ICON_PATH.to_string()),
        FOOTER_PROFILE_ICON_TOKEN => Some(FOOTER_PROFILE_ICON_PATH.to_string()),
        _ => {
            let trimmed = token.trim();
            if trimmed.is_empty()
                || trimmed.contains('/')
                || trimmed.contains('\\')
                || trimmed.contains("..")
            {
                return None;
            }
            // Validate against the EMBEDDED Lucide set (AppAssets), not the
            // repo checkout — vendor/ does not exist next to a released .app.
            let path = format!("icons/{trimmed}.svg");
            crate::utils::assets::embedded_asset_exists(&path).then_some(path)
        }
    }
}

pub(crate) fn footer_icon_path_or_profile(token: &str) -> String {
    footer_icon_path(token).unwrap_or_else(|| FOOTER_PROFILE_ICON_PATH.to_string())
}

fn render_footer_labelcap(
    label: SharedString,
    theme: &Theme,
    footer_text: gpui::Rgba,
    full_text: gpui::Hsla,
    label_font_size_px: Option<f32>,
) -> AnyElement {
    render_footer_labelcap_constrained(
        label,
        theme,
        footer_text,
        full_text,
        None,
        false,
        label_font_size_px,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_footer_labelcap_constrained(
    label: SharedString,
    _theme: &Theme,
    footer_text: gpui::Rgba,
    full_text: gpui::Hsla,
    max_width_px: Option<f32>,
    force_width: bool,
    label_font_size_px: Option<f32>,
    shrinkable: bool,
) -> AnyElement {
    let metrics = current_main_menu_footer_metrics();
    let label_font_size = label_font_size_px.unwrap_or(metrics.label_font_size);
    let mut cap = div();
    // A shrinkable labelcap participates in flex shrink (ellipsizing under
    // real layout pressure); the legacy path keeps its intrinsic width and
    // relies on precomputed slot widths.
    if shrinkable {
        cap = cap.overflow_hidden();
    } else {
        cap = cap.flex_none();
    }
    let mut cap = cap
        .min_w(px(metrics.keycap_height))
        .min_h(px(metrics.keycap_height))
        .h(px(metrics.keycap_height))
        .line_height(px(metrics.keycap_height))
        .px(px(metrics.keycap_padding_x))
        .py(px(metrics.keycap_padding_y))
        .rounded(px(metrics.keycap_radius))
        .flex()
        .items_center()
        .justify_center()
        .font_family(FONT_SYSTEM_UI)
        .font_weight(metrics.font_weight)
        .text_size(px(label_font_size))
        .text_color(footer_text)
        .group_hover("footer-action-button", move |s| s.text_color(full_text));

    if let Some(max_width_px) = max_width_px {
        cap = cap.max_w(px(max_width_px)).overflow_hidden();
        if force_width {
            cap = cap.w(px(max_width_px));
        }
    }

    cap.child(
        div()
            .min_w(px(0.0))
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .child(label),
    )
    .into_any_element()
}

#[derive(Clone, Copy)]
pub(crate) struct FooterKeycapHoverStyle {
    text: gpui::Hsla,
    glyph: gpui::Hsla,
    border_alpha: Option<u32>,
}

pub(crate) fn render_footer_shortcut_keycaps(shortcut: String, theme: &Theme) -> AnyElement {
    render_footer_shortcut_keycaps_with_metrics(shortcut, theme, None, None, None, None)
}

pub(crate) fn render_footer_shortcut_keycaps_with_metrics(
    shortcut: String,
    theme: &Theme,
    keycap_font_size_px: Option<f32>,
    keycap_height_px: Option<f32>,
    content_gap_px: Option<f32>,
    hover_style: Option<FooterKeycapHoverStyle>,
) -> AnyElement {
    let tokens = split_footer_shortcut(&shortcut);
    render_footer_shortcut_keycaps_from_tokens_with_metrics(
        tokens.iter().map(String::as_str),
        theme,
        keycap_font_size_px,
        keycap_height_px,
        content_gap_px,
        hover_style,
    )
}

pub(crate) fn render_footer_row_shortcut_keycaps_from_tokens<'a>(
    tokens: impl IntoIterator<Item = &'a str>,
    theme: &Theme,
) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .group("footer-action-button")
        .child(render_footer_shortcut_keycaps_from_tokens(tokens, theme))
        .into_any_element()
}

pub(crate) fn render_footer_shortcut_keycaps_from_tokens<'a>(
    tokens: impl IntoIterator<Item = &'a str>,
    theme: &Theme,
) -> AnyElement {
    render_footer_shortcut_keycaps_from_tokens_with_metrics(tokens, theme, None, None, None, None)
}

fn render_footer_shortcut_keycaps_from_tokens_with_metrics<'a>(
    tokens: impl IntoIterator<Item = &'a str>,
    theme: &Theme,
    keycap_font_size_px: Option<f32>,
    keycap_height_px: Option<f32>,
    content_gap_px: Option<f32>,
    hover_style: Option<FooterKeycapHoverStyle>,
) -> AnyElement {
    let content_gap =
        content_gap_px.unwrap_or_else(|| current_main_menu_footer_metrics().content_gap);
    div()
        .flex()
        .flex_none()
        .flex_row()
        .items_center()
        .gap(px(content_gap))
        .children(tokens.into_iter().map(|token| {
            render_footer_keycap_with_metrics(
                token.to_string(),
                None,
                theme,
                keycap_font_size_px,
                keycap_height_px,
                hover_style,
            )
        }))
        .into_any_element()
}

/// DevTools layout model for a keycap run, backed by real text-system glyph
/// measurement (`footer_shortcut_keycap_layout_model` namespace). Every token
/// bound is exact (`widthExact: true`).
pub(crate) fn footer_shortcut_keycap_layout_model_measured<'a>(
    tokens: impl IntoIterator<Item = &'a str>,
    origin_x: f32,
    origin_y: f32,
    cx: &gpui::App,
) -> serde_json::Value {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let metrics = current_main_menu_footer_metrics();
    let mut x = origin_x;
    let mut token_values = Vec::new();
    let mut token_bounds = Vec::new();

    for token in tokens {
        let width = footer_keycap_measured_width_px(token, cx);
        token_values.push(token.to_string());
        token_bounds.push(serde_json::json!({
            "token": token,
            "kind": if is_footer_icon_token(token) { "iconKeycap" } else { "keycap" },
            "bounds": {
                "x": x,
                "y": origin_y,
                "width": width,
                "height": metrics.keycap_height,
            },
            "widthExact": true,
            "widthSource": "text-system-glyph-measure",
            "heightSource": "footer-metrics-keycap-height",
            "glyphNudgeY": footer_key_glyph_nudge_y(token),
            "glyphNudgeX": footer_key_glyph_nudge_x(token),
            "borderWidth": 1.0,
            "radius": metrics.keycap_radius,
            "paddingX": footer_keycap_padding_x_for_token(token, &metrics),
            "fontSize": metrics.keycap_font_size,
        }));
        x += width + metrics.content_gap;
    }

    if !token_bounds.is_empty() {
        x -= metrics.content_gap;
    }

    serde_json::json!({
        "tokens": token_values,
        "tokenBounds": token_bounds,
        "bounds": {
            "x": origin_x,
            "y": origin_y,
            "width": (x - origin_x).max(0.0),
            "height": if token_bounds.is_empty() { 0.0 } else { metrics.keycap_height },
        },
        "boundsAvailable": true,
        "coordinateSpace": "providedOriginLogicalPx",
        "units": "logicalPx",
        "gap": metrics.content_gap,
        "heightSource": "footer-metrics-keycap-height",
        "widthSource": "footer-keycap-token-model",
        "exactTokenBounds": true,
        "measurementSource": FOOTER_SHORTCUT_LAYOUT_MEASUREMENT_SOURCE,
        "stopReason": serde_json::Value::Null,
    })
}

#[allow(dead_code)]
fn render_footer_keycap(token: String, max_width_px: Option<f32>, theme: &Theme) -> AnyElement {
    render_footer_keycap_with_metrics(token, max_width_px, theme, None, None, None)
}

fn render_footer_keycap_with_metrics(
    token: String,
    max_width_px: Option<f32>,
    theme: &Theme,
    keycap_font_size_px: Option<f32>,
    keycap_height_px: Option<f32>,
    hover_style: Option<FooterKeycapHoverStyle>,
) -> AnyElement {
    let footer_text = footer_hint_text_color(theme);
    let hover_text = hover_style
        .map(|style| style.text)
        .unwrap_or_else(|| footer_hover_text_color(theme, None));
    let hover_glyph = hover_style
        .map(|style| style.glyph)
        .unwrap_or_else(|| footer_hover_glyph_color(theme, None));
    let hover_border = footer_keycap_border_hover_color_with_alpha(
        theme,
        hover_style.and_then(|style| style.border_alpha),
    );
    let metrics = current_main_menu_footer_metrics();
    let keycap_height = keycap_height_px.unwrap_or(metrics.keycap_height);
    let keycap_font_size = keycap_font_size_px.unwrap_or(metrics.keycap_font_size);
    let token_child: AnyElement = if let Some(path) = footer_icon_path(&token) {
        svg()
            .path(path)
            .size(px((keycap_font_size + 1.0).max(10.0)))
            .flex_shrink_0()
            .text_color(footer_text)
            .group_hover("footer-action-button", move |s| s.text_color(hover_glyph))
            .into_any_element()
    } else {
        let nudge_x = footer_key_glyph_nudge_x(&token);
        let mut glyph = div()
            .h(px(keycap_height))
            .line_height(px(keycap_height))
            .mt(px(footer_key_glyph_nudge_y(&token)));
        if nudge_x != 0.0 {
            glyph = glyph.relative().left(px(nudge_x));
        }
        glyph.child(token.clone()).into_any_element()
    };

    let mut keycap = div()
        .flex_none()
        .min_w(px(keycap_height))
        .min_h(px(keycap_height))
        .h(px(keycap_height))
        .line_height(px(keycap_height))
        .px(px(footer_keycap_padding_x_for_token(&token, &metrics)))
        .py(px(metrics.keycap_padding_y))
        .rounded(px(metrics.keycap_radius))
        .border_1()
        .border_color(footer_keycap_border_color(theme))
        .flex()
        .items_center()
        .justify_center()
        .font_family(FONT_SYSTEM_UI)
        .font_weight(metrics.font_weight)
        .text_size(px(keycap_font_size))
        .text_color(footer_text)
        .group_hover("footer-action-button", move |s| {
            s.text_color(hover_text).border_color(hover_border)
        })
        .child(token_child);

    if let Some(max_width_px) = max_width_px {
        keycap = keycap
            .max_w(px(max_width_px))
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap();
    }

    keycap.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .expect("function signature should exist");
        let source = &source[start..];
        let open = source.find('{').expect("function body should open");
        let mut depth = 0usize;
        for (offset, byte) in source.as_bytes()[open..].iter().copied().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[open + 1..open + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("function body should close")
    }

    #[test]
    fn config_rail_maps_every_footer_action_to_shared_chrome_slots() {
        use crate::footer_popup::FooterAction;

        for (action, expected) in [
            (FooterAction::Run, FooterActionSlot::Run),
            (FooterAction::Actions, FooterActionSlot::Actions),
            (FooterAction::Ai, FooterActionSlot::Ai),
            (FooterAction::Apply, FooterActionSlot::Apply),
            (FooterAction::Replace, FooterActionSlot::Replace),
            (FooterAction::Append, FooterActionSlot::Append),
            (FooterAction::Copy, FooterActionSlot::Copy),
            (FooterAction::Expand, FooterActionSlot::Expand),
            (FooterAction::Retry, FooterActionSlot::Retry),
            (FooterAction::Close, FooterActionSlot::Close),
            (FooterAction::Stop, FooterActionSlot::Stop),
            (FooterAction::PasteResponse, FooterActionSlot::PasteResponse),
            (FooterAction::Cwd, FooterActionSlot::Ai),
            (FooterAction::AgentModel, FooterActionSlot::Ai),
            (FooterAction::Tips, FooterActionSlot::Ai),
        ] {
            assert_eq!(footer_config_action_slot(action), expected);
        }
    }

    #[test]
    fn config_rail_preserves_explicit_and_contextual_left_pinning() {
        use crate::footer_popup::{FooterAction, FooterButtonConfig};

        assert!(footer_config_button_is_left_pinned(
            &FooterButtonConfig::new(FooterAction::Cwd, "⇥", "Project")
        ));
        assert!(footer_config_button_is_left_pinned(
            &FooterButtonConfig::new(FooterAction::AgentModel, "⇧⇥", "Agent · Model")
        ));
        assert!(footer_config_button_is_left_pinned(
            &FooterButtonConfig::new(FooterAction::Close, "Esc", "Terminate").left_pinned()
        ));
        assert!(!footer_config_button_is_left_pinned(
            &FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
        ));
    }

    /// Optical keycap corrections (user report 2026-07-11): the ⌘ ink sat
    /// low-left of chip center, and word keycaps ("Space") rendered with zero
    /// horizontal padding and the single-glyph down-nudge crowding the
    /// descender against the bottom border. Lock the per-token resolution so
    /// the corrections can't silently regress to the uniform values.
    #[test]
    fn keycap_glyph_nudges_and_padding_resolve_per_token() {
        let metrics = crate::designs::MainMenuThemeVariant::InfoBarBase
            .base_def()
            .footer
            .metrics;

        // ⌘ gets its own optical x/y correction.
        assert_eq!(footer_key_glyph_nudge_x("⌘"), metrics.cmd_glyph_nudge_x);
        assert_eq!(footer_key_glyph_nudge_y("⌘"), metrics.cmd_glyph_nudge_y);
        assert!(metrics.cmd_glyph_nudge_x > 0.0, "⌘ ink sits left of center");
        assert!(
            metrics.cmd_glyph_nudge_y < metrics.key_glyph_nudge_y,
            "⌘ must not inherit the full single-glyph down-nudge"
        );

        // Word tokens ride high of the single-glyph nudge and gain padding.
        assert!(is_footer_word_key_token("Space"));
        assert!(!is_footer_word_key_token("⌘"));
        assert_eq!(
            footer_key_glyph_nudge_y("Space"),
            metrics.word_glyph_nudge_y
        );
        assert!(
            metrics.word_glyph_nudge_y < metrics.key_glyph_nudge_y,
            "descender words sat low in the chip"
        );
        assert!(
            metrics.word_keycap_padding_x > metrics.keycap_padding_x,
            "word keycaps need horizontal breathing room"
        );

        // Single glyphs keep the uniform treatment.
        assert_eq!(footer_key_glyph_nudge_x("K"), 0.0);
        assert_eq!(footer_key_glyph_nudge_y("K"), metrics.key_glyph_nudge_y);
        assert_eq!(
            footer_key_glyph_nudge_y(";"),
            metrics.semicolon_glyph_nudge_y
        );
        assert_eq!(
            footer_key_glyph_nudge_y("↵"),
            metrics.key_glyph_nudge_y + metrics.return_glyph_nudge_y
        );
    }

    #[test]
    fn split_footer_shortcut_parses_simple_and_complex_keys() {
        assert_eq!(split_footer_shortcut(""), Vec::<String>::new());
        assert_eq!(split_footer_shortcut("↵"), vec!["↵"]);
        assert_eq!(split_footer_shortcut("⌘K"), vec!["⌘", "K"]);
        assert_eq!(split_footer_shortcut("⌥↵"), vec!["⌥", "↵"]);
        assert_eq!(split_footer_shortcut("Enter"), vec!["Enter"]);
        assert_eq!(split_footer_shortcut("esc"), vec!["⎋"]);
        assert_eq!(split_footer_shortcut("Escape"), vec!["⎋"]);
        assert_eq!(split_footer_shortcut("Cmd+K"), vec!["Cmd", "K"]);
        assert_eq!(split_footer_shortcut("⌘F1"), vec!["⌘", "F1"]);
        assert_eq!(split_footer_shortcut("⌥⌘I"), vec!["⌥", "⌘", "I"]);
        assert_eq!(split_footer_shortcut("click"), vec!["click"]);
        // A trailing '+' is the plus key (terminal Zoom In), never an
        // empty keycap.
        assert_eq!(split_footer_shortcut("⌘+"), vec!["⌘", "+"]);
        assert_eq!(split_footer_shortcut("Ctrl++"), vec!["Ctrl", "+"]);
        assert_eq!(split_footer_shortcut("⌘-"), vec!["⌘", "-"]);
        assert_eq!(split_footer_shortcut("⌃\\"), vec!["⌃", "\\"]);
    }

    #[test]
    fn split_footer_shortcut_covers_help_guidance_tokens() {
        assert_eq!(split_footer_shortcut("/"), vec!["/"]);
        assert_eq!(split_footer_shortcut("@"), vec!["@"]);
        assert_eq!(split_footer_shortcut("⇧↵"), vec!["⇧", "↵"]);
        assert_eq!(split_footer_shortcut("⌘P"), vec!["⌘", "P"]);
        assert_eq!(split_footer_shortcut(";todo"), vec![";", "todo"]);
        assert_eq!(split_footer_shortcut(":tag:"), vec![":", "tag", ":"]);
    }

    #[test]
    fn footer_action_frame_shrinks_with_flex_content_not_estimated_widths() {
        let source = include_str!("footer_chrome.rs");
        let frame_start = source
            .find("pub(crate) fn render_footer_hint_action_button_frame")
            .expect("action button frame renderer should exist");
        let frame_source = &source[frame_start..];
        let frame_body = &frame_source[..frame_source
            .find("\n}\n")
            .expect("frame renderer should terminate")];

        assert!(
            frame_body.contains("render_footer_hint_content_flex_with_layout"),
            "shrink-to-content frames must hug the rendered flex content"
        );
        assert!(
            !frame_body.contains("footer_hint_action_visual_width_px"),
            "shrink-to-content frames must not derive widths from per-char text estimates"
        );
        assert!(
            frame_body.contains(".max_w(px(spec.slot_width_px))"),
            "the content-hugging frame must stay bounded by the fixed slot"
        );

        // Universal-chrome lock (2026-07-20): glass in-window footers across
        // surfaces render ONE config-driven rail so the GPUI fallback can
        // never drift from the native footer language. The renderer's input
        // must stay MainWindowFooterConfig (the same model the native footer
        // consumes) and it must compose the shared frames above.
        let rail_start = source
            .find("pub(crate) fn render_main_window_footer_config_rail")
            .expect("config-driven footer rail renderer should exist");
        let rail_source = &source[rail_start..];
        let rail_body = &rail_source[..rail_source
            .find("\n}\n")
            .expect("config rail renderer should terminate")];
        assert!(
            rail_body.contains("MainWindowFooterConfig"),
            "the shared rail must consume the same config model as the native footer"
        );
        assert!(
            rail_body.contains("render_footer_hint_action_button_frame"),
            "the shared rail must compose the shared footer button frames"
        );

        // Liquid Glass consistency lock: the footer rail itself never reaches
        // into AppKit. Both row and standalone opt-ins converge on the one
        // painted-bounds wrapper, and that wrapper is the only owner here of
        // raw native sync + group/index hover routing.
        let footer_rail = function_body(
            source,
            "pub(crate) fn render_footer_action_rail_with_leading",
        );
        assert!(footer_rail.contains("render_footer_action_rail_with_leading_and_cleanup("));
        assert!(!footer_rail.contains("glass_button_host::sync_for_window"));

        let capsule_row = function_body(source, "pub(crate) fn glass_capsule_row");
        assert!(capsule_row.contains("glass_capsule_row_with_cleanup("));
        let capsule_row_core = function_body(source, "fn glass_capsule_row_with_cleanup");
        for required in [
            ".on_children_prepainted(",
            "glass_button_host::sync_for_window(window, group, &frames)",
            "glass_button_host::set_hover(window, group, index, *hovered)",
        ] {
            assert!(
                capsule_row_core.contains(required),
                "shared glass row must retain {required}"
            );
        }

        let standalone = function_body(source, "pub(crate) fn glass_capsule(");
        assert!(standalone.contains("glass_capsule_row("));
    }

    #[test]
    fn key_anchored_footer_content_keeps_symmetric_outer_padding() {
        assert_eq!(FOOTER_KEY_ANCHORED_CONTENT_PADDING_X_PX, 6.0);
    }

    #[test]
    fn footer_key_glyph_nudges_match_footer_contract() {
        assert!(is_footer_return_key_glyph("↵"));
        assert!(!is_footer_return_key_glyph("Enter"));
        assert_eq!(footer_key_glyph_nudge_x("⌘"), 0.5);
        assert_eq!(footer_key_glyph_nudge_y("⌘"), 0.0);
        assert_eq!(footer_key_glyph_nudge_y("Space"), -0.75);
        assert_eq!(footer_key_glyph_nudge_y("↵"), 2.0);
        assert_eq!(footer_key_glyph_nudge_y(";"), -1.0);
        assert_eq!(footer_appkit_glyph_y("⌘", 20.0, 10.0), 5.0);
        assert_eq!(footer_appkit_glyph_y("↵", 20.0, 10.0), 3.0);
        assert_eq!(footer_appkit_glyph_y(";", 20.0, 10.0), 6.0);
        assert_eq!(footer_button_height(32.0), 28.0);
    }

    #[test]
    fn footer_horizontal_run_width_uses_gap_only_between_items() {
        // 40 + 20 + 20 + 2 gaps * 2px = 84
        assert_eq!(
            footer_horizontal_run_width_px(&[40.0, 20.0, 20.0], FOOTER_ACTION_ITEM_GAP_PX),
            84.0
        );
        assert_eq!(
            footer_horizontal_run_width_px(&[], FOOTER_ACTION_ITEM_GAP_PX),
            0.0
        );
        // A single item has no inter-item gap.
        assert_eq!(
            footer_horizontal_run_width_px(&[40.0], FOOTER_ACTION_ITEM_GAP_PX),
            40.0
        );
    }

    #[test]
    fn footer_horizontal_run_origins_use_constant_gap() {
        assert_eq!(
            footer_horizontal_run_origins_px(&[40.0, 20.0, 20.0], FOOTER_ACTION_ITEM_GAP_PX, 0.0),
            vec![0.0, 42.0, 64.0]
        );
        // The same run anchored at a non-zero origin just shifts every item.
        assert_eq!(
            footer_horizontal_run_origins_px(&[40.0, 20.0], FOOTER_ACTION_ITEM_GAP_PX, 10.0),
            vec![10.0, 52.0]
        );
    }

    #[test]
    fn footer_action_chrome_tokens_match_native_footer_contract() {
        assert_eq!(FOOTER_ACTION_ITEM_GAP_PX, 2.0);
        assert_eq!(FOOTER_GLASS_BUTTON_GAP_PX, 6.0);
        assert_eq!(FOOTER_ACTION_CONTENT_GAP_PX, 4.0);
        assert_eq!(FOOTER_ACTION_CONTENT_PADDING_X_PX, 4.0);
        assert_eq!(FOOTER_ACTION_BUTTON_RADIUS_PX, 6.0);
        assert_eq!(footer_centered_action_edge_padding_x(), 10.0);
        assert_eq!(FOOTER_RUN_SLOT_MIN_WIDTH_PX, 92.0);
        assert_eq!(FOOTER_RUN_SLOT_MAX_WIDTH_PX, 242.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Actions), 92.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Ai), 52.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Apply), 84.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Close), 84.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Stop), 76.0);
        assert_eq!(
            footer_action_slot_width(FooterActionSlot::PasteResponse),
            140.0
        );

        let mut theme = Theme::dark_default();
        let mut opacity = theme.get_opacity();
        opacity.hover = 0.12;
        opacity.selected = 0.31;
        theme.opacity = Some(opacity);

        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        let rail = footer_rail_chrome(&theme);
        assert_eq!(rail.height_px, current_main_menu_footer_height());
        assert_eq!(
            rail.side_inset_px,
            crate::window_resize::main_layout::HINT_STRIP_PADDING_X
        );
        assert_eq!(rail.surface_rgba, chrome.inline_dropdown_surface_rgba);
        assert_eq!(rail.divider_rgba, chrome.divider_rgba);
        assert_eq!(rail.hover_rgba, chrome.hover_rgba);
        assert_eq!(rail.active_rgba, chrome.selection_rgba);
        assert_eq!(rail.button_radius_px, FOOTER_ACTION_BUTTON_RADIUS_PX);
    }

    #[test]
    fn footer_keycap_border_alpha_is_visible_and_stronger_on_hover() {
        let mut theme = Theme::dark_default();
        let mut opacity = theme.get_opacity();
        opacity.hover = 0.12;
        opacity.selected = 0.31;
        theme.opacity = Some(opacity);

        assert_eq!(
            footer_keycap_border_alpha(&theme, false),
            FOOTER_CHIP_BORDER_ALPHA
        );
        assert_eq!(
            footer_keycap_border_alpha(&theme, true),
            FOOTER_CHIP_BORDER_SELECTED_ALPHA
        );
        assert!(
            (footer_keycap_border_hover_alpha(&theme) - FOOTER_CHIP_BORDER_HOVER_ALPHA).abs()
                <= 0.01
        );
        assert!(footer_keycap_border_color(&theme).a >= FOOTER_CHIP_BORDER_ALPHA - 0.01);
        assert!(
            footer_keycap_border_hover_color_with_alpha(&theme, None).a
                >= FOOTER_CHIP_BORDER_HOVER_ALPHA - 0.01
        );
        assert!(
            footer_keycap_border_color_for_state(&theme, true).a
                >= FOOTER_CHIP_BORDER_SELECTED_ALPHA - 0.01
        );
    }
}
