use std::time::Duration;

use gpui::{App, IntoElement as _, Window};
use gpui_component::notification::Notification;

use crate::components::ToastAction;

use super::PendingToast;

struct ScriptKitToastNotification;

/// Convert one queued Script Kit toast into the active entity-backed
/// notification runtime without discarding actions, dismissal, identity, or
/// the configured lifetime.
pub fn pending_toast_to_notification(toast: PendingToast) -> Notification {
    let model = toast.toast;
    let notification_id = model.get_id().control_id("notification");
    let duration_ms = toast.duration_ms;

    let notification = Notification::new()
        .id1::<ScriptKitToastNotification>(notification_id)
        .dismissible(false)
        .content_only(move |_notification, _window, cx| {
            let mut rendered = model.clone().clear_actions().clear_on_dismiss();

            for action in model.get_actions().iter().cloned() {
                let original = action.callback.clone();
                let action_id = action.id.as_str().to_string();
                let label = action.label.clone();
                let handler = cx.listener(move |notification, event, window, cx| {
                    (original)(event, window, cx);
                    notification.dismiss_from_control(window, cx);
                });
                rendered = rendered.action(ToastAction::new(action_id, label, Box::new(handler)));
            }

            if model.is_dismissible() {
                let original = model.get_on_dismiss();
                let notification = cx.weak_entity();
                rendered =
                    rendered.on_dismiss(Box::new(move |window: &mut Window, cx: &mut App| {
                        if let Some(callback) = original.as_ref() {
                            callback(window, cx);
                        }
                        let _ = notification.update(cx, |notification, cx| {
                            notification.dismiss_from_control(window, cx);
                        });
                    }));
            }

            rendered.into_any_element()
        });

    match duration_ms {
        Some(duration_ms) => notification.autohide_after(Duration::from_millis(duration_ms)),
        None => notification.autohide(false),
    }
}
