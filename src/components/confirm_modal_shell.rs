use gpui::{div, prelude::*, px, AnyElement, Div, Rgba, SharedString, Stateful};

use crate::components::footer_chrome::{
    render_footer_hint_action_button_frame, FooterHintActionButtonFrameSpec,
    FooterHintButtonLayoutOverrides, FooterHintContentJustify,
};
use crate::components::prompt_footer::FooterClickCallback;
use crate::theme::Theme;

pub(crate) const CONFIRM_MODAL_SHELL_ID: &str = "modal-shell:confirm";
pub(crate) const MODAL_WIDTH_PX: f32 = 360.0;
/// Wider request used by parent-attached dialogs. Native modal windows clamp
/// through `MODAL_WIDTH_PX`, so this variant exists only for callers whose
/// parent surface can host the wider layout.
pub(crate) const PARENT_MODAL_WIDTH_PX: f32 = 448.0;
pub(crate) const MODAL_ACTION_DISABLED_OPACITY: f32 = 0.45;
pub(crate) const MODAL_ACTION_ROW_TOP_MARGIN_PX: f32 = 12.0;
pub(crate) const CONFIRM_MODAL_HEADER_ACCENT_WIDTH: f32 = 2.0;
pub(crate) const CONFIRM_MODAL_HEADER_ACCENT_HEIGHT: f32 = 14.0;
pub(crate) const CONFIRM_MODAL_HEADER_GAP: f32 = 8.0;
pub(crate) const CONFIRM_MODAL_RADIUS: f32 = 8.0;

#[derive(Clone)]
pub(crate) struct ConfirmModalShellConfig {
    pub(crate) content_id: &'static str,
    pub(crate) width: Option<f32>,
    pub(crate) padding_x: f32,
    pub(crate) padding_y: f32,
    pub(crate) gap: f32,
    pub(crate) background: Option<Rgba>,
    pub(crate) border: Rgba,
    pub(crate) radius: f32,
    pub(crate) offset_y: f32,
    pub(crate) opacity: f32,
}

pub(crate) struct ModalActionRowButton {
    pub(crate) id: &'static str,
    pub(crate) label: SharedString,
    pub(crate) key: SharedString,
    pub(crate) slot_width_px: f32,
    pub(crate) height_px: f32,
    pub(crate) selected: bool,
    pub(crate) enabled: bool,
    pub(crate) layout: FooterHintButtonLayoutOverrides,
    pub(crate) on_click: FooterClickCallback,
}

pub(crate) fn confirm_modal_header(
    title: impl Into<SharedString>,
    accent: Rgba,
    title_color: Rgba,
) -> Div {
    let accent_width = CONFIRM_MODAL_HEADER_ACCENT_WIDTH;
    let accent_height = CONFIRM_MODAL_HEADER_ACCENT_HEIGHT;
    let header_gap = CONFIRM_MODAL_HEADER_GAP;

    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(header_gap))
        .child(
            div()
                .w(px(accent_width))
                .h(px(accent_height))
                .rounded(px(1.0))
                .bg(accent),
        )
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(title_color)
                .child(title.into()),
        )
}

pub(crate) fn modal_action_row(
    id: &'static str,
    gap_px: f32,
    buttons: Vec<ModalActionRowButton>,
    theme: &Theme,
) -> Stateful<Div> {
    let mut row = div()
        .id(id)
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .gap(px(gap_px));

    for button in buttons {
        let enabled = button.enabled;
        let on_click = button.on_click;
        row = row.child(
            render_footer_hint_action_button_frame(
                FooterHintActionButtonFrameSpec {
                    id: button.id,
                    label: button.label,
                    key: button.key,
                    slot_width_px: button.slot_width_px,
                    height_px: button.height_px,
                    selected: button.selected,
                    key_first: false,
                    justify: FooterHintContentJustify::Center,
                    layout: button.layout,
                },
                theme,
            )
            .when(!enabled, |style| {
                style.opacity(MODAL_ACTION_DISABLED_OPACITY)
            })
            .on_click(move |event, window, cx| {
                if enabled {
                    on_click(event, window, cx);
                }
            }),
        );
    }

    row
}

pub(crate) fn confirm_modal_shell(
    config: ConfirmModalShellConfig,
    children: Vec<AnyElement>,
) -> Stateful<Div> {
    let padding_x = config.padding_x;
    let padding_y = config.padding_y;
    let gap = config.gap;
    let radius = config.radius;

    let mut content = div()
        .id(config.content_id)
        .w_full()
        .p_0()
        .flex()
        .flex_col()
        .gap(px(gap));

    for child in children {
        content = content.child(child);
    }

    let mut shell = div()
        .id(CONFIRM_MODAL_SHELL_ID)
        .px(px(padding_x))
        .py(px(padding_y))
        .border_1()
        .border_color(config.border)
        .rounded(px(radius))
        .flex()
        .flex_col()
        .mt(px(config.offset_y))
        .opacity(config.opacity)
        .overflow_hidden()
        .child(content);

    if let Some(width) = config.width {
        shell = shell.w(px(width));
    } else {
        shell = shell.size_full();
    }

    if let Some(background) = config.background {
        shell = shell.bg(background);
    }

    shell
}
