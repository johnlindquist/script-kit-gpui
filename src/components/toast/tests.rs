use super::types::{
    TOAST_ACTIONS_GAP_PX, TOAST_ACTIONS_MARGIN_TOP_PX, TOAST_BORDER_WIDTH_PX, TOAST_CONTENT_GAP_PX,
    TOAST_CONTENT_PADDING_X_PX, TOAST_CONTENT_PADDING_Y_PX, TOAST_ICON_SIZE_PX, TOAST_MAX_WIDTH_PX,
    TOAST_MESSAGE_COLUMN_GAP_PX, TOAST_RADIUS_PX,
};
use super::{Toast, ToastAction, ToastColors, ToastVariant};
use crate::designs::DesignColors;
use crate::theme::Theme;
use gpui::{
    div, AppContext as _, Context, IntoElement, ParentElement as _, Render, TestAppContext, Window,
};

struct DuplicateToastIdentityProbe;

impl Render for DuplicateToastIdentityProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(
                Toast::new("Same message", ToastColors::default())
                    .with_id("duplicate-a")
                    .action(ToastAction::new(
                        "open-local",
                        "Open",
                        Box::new(|_, _, _| {}),
                    )),
            )
            .child(
                Toast::new("Same message", ToastColors::default())
                    .with_id("duplicate-b")
                    .action(ToastAction::new(
                        "open-remote",
                        "Open",
                        Box::new(|_, _, _| {}),
                    )),
            )
    }
}

#[test]
fn duplicate_messages_and_action_labels_render_unique_control_ids() {
    let mut cx = TestAppContext::single();
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |_, cx| {
            cx.new(|_| DuplicateToastIdentityProbe)
        })
        .expect("duplicate-toast identity window should open")
    });

    window
        .update(&mut cx, |_, window, _| {
            let bounds = window.debug_bounds();
            for id in [
                "toast:duplicate-a:root",
                "toast:duplicate-a:action:open-local",
                "toast:duplicate-a:dismiss",
                "toast:duplicate-b:root",
                "toast:duplicate-b:action:open-remote",
                "toast:duplicate-b:dismiss",
            ] {
                assert!(bounds.contains_key(id), "missing rendered control ID {id}");
            }
        })
        .expect("duplicate-toast identity window should remain available");
}

#[test]
fn duplicate_toast_messages_have_distinct_lifetime_ids() {
    let colors = ToastColors::default();
    let first = Toast::new("Same message", colors);
    let second = Toast::new("Same message", colors);

    assert_ne!(first.get_id(), second.get_id());
    assert_eq!(first.get_message(), second.get_message());
}

#[test]
fn toast_message_changes_do_not_define_identity() {
    let toast = Toast::new("Before", ToastColors::default()).with_id("sync:status");
    let id = toast.get_id().clone();
    let changed = Toast::new("After", ToastColors::default()).with_id("sync:status");

    assert_eq!(id, *changed.get_id());
    assert_ne!(toast.get_message(), changed.get_message());
}

#[test]
fn duplicate_toast_action_labels_have_distinct_required_ids() {
    let toast = Toast::new("Choose", ToastColors::default())
        .action(ToastAction::new(
            "open:local",
            "Open",
            Box::new(|_, _, _| {}),
        ))
        .action(ToastAction::new(
            "open:remote",
            "Open",
            Box::new(|_, _, _| {}),
        ));

    assert_eq!(toast.get_actions().len(), 2);
    assert_ne!(toast.get_actions()[0].id, toast.get_actions()[1].id);
    assert_eq!(toast.get_actions()[0].label, toast.get_actions()[1].label);
}

#[test]
#[should_panic(expected = "Toast action ID must not be empty")]
fn empty_toast_action_id_is_rejected() {
    let _ = ToastAction::new("", "Open", Box::new(|_, _, _| {}));
}

#[test]
fn test_toast_colors_from_theme_uses_selected_subtle_for_details_background() {
    let mut theme = Theme::default();
    theme.colors.accent.selected_subtle = 0x334455;

    let colors = ToastColors::from_theme(&theme, ToastVariant::Info);
    assert_eq!(colors.details_bg, 0x33445520);
}

#[test]
fn test_toast_colors_from_design_uses_selected_background_for_details_background() {
    let design = DesignColors {
        background_selected: 0x556677,
        ..Default::default()
    };

    let colors = ToastColors::from_design(&design, ToastVariant::Info);
    assert_eq!(colors.details_bg, 0x55667720);
}

#[test]
fn test_toast_layout_tokens_stay_consistent_when_spacing_is_adjusted() {
    assert_eq!(TOAST_MAX_WIDTH_PX, 400.0);
    assert_eq!(TOAST_BORDER_WIDTH_PX, 2.0);
    assert_eq!(TOAST_RADIUS_PX, 8.0);
    assert_eq!(TOAST_CONTENT_GAP_PX, 10.0);
    assert_eq!(TOAST_CONTENT_PADDING_X_PX, 12.0);
    assert_eq!(TOAST_CONTENT_PADDING_Y_PX, 10.0);
    assert_eq!(TOAST_ICON_SIZE_PX, 18.0);
    assert_eq!(TOAST_MESSAGE_COLUMN_GAP_PX, 6.0);
    assert_eq!(TOAST_ACTIONS_GAP_PX, 8.0);
    assert_eq!(TOAST_ACTIONS_MARGIN_TOP_PX, 4.0);
}
