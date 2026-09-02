use crate::{
    ActiveTheme, Anchor, ElementExt, Placement,
    dialog::Dialog,
    input::InputState,
    notification::{Notification, NotificationLayerSnapshot, NotificationList},
    sheet::Sheet,
    window_border,
};
use gpui::{
    AnyView, App, AppContext, Context, DefiniteLength, Entity, FocusHandle, InteractiveElement,
    IntoElement, KeyBinding, ParentElement as _, Render, Styled, WeakFocusHandle, Window, actions,
    div, prelude::FluentBuilder as _,
};
use std::{any::TypeId, cell::Cell, rc::Rc};

actions!(root, [Tab, TabPrev]);

const CONTEXT: &str = "Root";
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", Tab, Some(CONTEXT)),
        KeyBinding::new("shift-tab", TabPrev, Some(CONTEXT)),
    ]);
}

/// Identity of one actual Root-owned dialog lifetime, not a stack position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootDialogId {
    pub root_entity_id: u64,
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootLayerSnapshot {
    pub revision: u64,
    /// Live stack order; the last identity owns modal interaction.
    pub dialogs: Vec<RootDialogId>,
    pub notifications: Vec<NotificationLayerSnapshot>,
    pub notifications_expanded: bool,
}

pub(crate) fn advance_layer_revision(revision: &Cell<u64>) -> u64 {
    let next = revision
        .get()
        .checked_add(1)
        .expect("Root layer revision exhausted");
    revision.set(next);
    next
}

/// Root is a view for the App window for as the top level view (Must be the first view in the window).
///
/// It is used to manage the Sheet, Dialog, and Notification.
pub struct Root {
    pub(crate) active_sheet: Option<ActiveSheet>,
    pub(crate) active_dialogs: Vec<ActiveDialog>,
    pub(super) focused_input: Option<Entity<InputState>>,
    pub notification: Entity<NotificationList>,
    sheet_size: Option<DefiniteLength>,
    view: AnyView,
    paint_background: bool,
    layer_revision: Rc<Cell<u64>>,
}

#[derive(Clone)]
pub(crate) struct ActiveSheet {
    focus_handle: FocusHandle,
    /// The previous focused handle before opening the Sheet.
    previous_focused_handle: Option<WeakFocusHandle>,
    placement: Placement,
    builder: Rc<dyn Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static>,
}

#[derive(Clone)]
pub(crate) struct ActiveDialog {
    id: RootDialogId,
    pub(crate) focus_handle: FocusHandle,
    /// The previous focused handle before opening the Dialog.
    previous_focused_handle: Option<WeakFocusHandle>,
    builder: Rc<dyn Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static>,
}

impl ActiveDialog {
    pub(crate) fn new(
        id: RootDialogId,
        focus_handle: FocusHandle,
        previous_focused_handle: Option<WeakFocusHandle>,
        builder: impl Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
    ) -> Self {
        Self {
            id,
            focus_handle,
            previous_focused_handle,
            builder: Rc::new(builder),
        }
    }
}

impl Root {
    /// Create a new Root view.
    pub fn new(view: impl Into<AnyView>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let notification = cx.new(|cx| NotificationList::new(window, cx));
        let layer_revision = notification.read(cx).layer_revision.clone();
        Self {
            active_sheet: None,
            active_dialogs: Vec::new(),
            focused_input: None,
            notification,
            layer_revision,
            sheet_size: None,
            view: view.into(),
            paint_background: true,
        }
    }

    /// Create a Root whose window-sized wrapper stays transparent.
    ///
    /// Use this when the hosted view owns a smaller, explicitly bounded
    /// background stage inside a larger transparent native window.
    pub fn new_transparent(
        view: impl Into<AnyView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut root = Self::new(view, window, cx);
        root.paint_background = false;
        root
    }

    pub fn update<F, R>(window: &mut Window, cx: &mut App, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> R,
    {
        let root = window
            .root::<Root>()
            .flatten()
            .expect("BUG: window first layer should be a gpui_component::Root.");

        root.update(cx, |root, cx| f(root, window, cx))
    }

    pub fn read<'a>(window: &'a Window, cx: &'a App) -> &'a Self {
        &window
            .root::<Root>()
            .expect("The window root view should be of type `ui::Root`.")
            .unwrap()
            .read(cx)
    }

    /// Read the same source epoch without allocating layer snapshots.
    pub fn layer_revision(&self) -> u64 {
        self.layer_revision.get()
    }

    /// Snapshot only actual retained layer owners; inspection never renders or mutates.
    pub fn layer_snapshot(&self, cx: &App) -> RootLayerSnapshot {
        let notifications = self.notification.read(cx);
        RootLayerSnapshot {
            revision: self.layer_revision(),
            dialogs: self.active_dialogs.iter().map(|dialog| dialog.id).collect(),
            notifications: notifications.layer_snapshot(cx),
            notifications_expanded: notifications.expanded,
        }
    }

    pub fn is_current_dialog(&self, id: RootDialogId) -> bool {
        self.active_dialogs
            .last()
            .is_some_and(|dialog| dialog.id == id)
    }

    /// Reject stale handlers even when a replacement occupies the same stack slot.
    pub fn close_dialog_if_current(id: RootDialogId, window: &mut Window, cx: &mut App) -> bool {
        let Some(root) = window.root::<Root>().flatten() else {
            return false;
        };
        root.update(cx, |root, cx| {
            if !root.is_current_dialog(id) {
                return false;
            }
            root.close_dialog(window, cx);
            true
        })
    }

    /// Dismiss only an entity that still belongs to this Root's live list.
    pub fn dismiss_notification_if_current(
        entity_id: u64,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let Some(root) = window.root::<Root>().flatten() else {
            return false;
        };
        let notification = root
            .read(cx)
            .notification
            .read(cx)
            .notifications
            .iter()
            .find(|note| note.entity_id().as_u64() == entity_id)
            .cloned();
        let Some(notification) = notification else {
            return false;
        };
        notification.update(cx, |note, cx| {
            if note.closing {
                return false;
            }
            note.dismiss_from_control(window, cx);
            true
        })
    }

    // Render Notification layer.
    pub fn render_notification_layer(
        window: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement + use<>> {
        let root = window.root::<Root>()??;

        let active_sheet_placement = root.read(cx).active_sheet.clone().map(|d| d.placement);

        let sheet_size = root.read(cx).sheet_size;
        let (mt, mr, mb, ml) = match active_sheet_placement {
            Some(Placement::Top) => (sheet_size, None, None, None),
            Some(Placement::Right) => (None, sheet_size, None, None),
            Some(Placement::Bottom) => (None, None, sheet_size, None),
            Some(Placement::Left) => (None, None, None, sheet_size),
            _ => (None, None, None, None),
        };

        let placement = cx.theme().notification.placement;

        Some(
            div()
                .absolute()
                .when(matches!(placement, Anchor::TopRight), |this| {
                    this.top_0().right_0()
                })
                .when(matches!(placement, Anchor::TopLeft), |this| {
                    this.top_0().left_0()
                })
                .when(matches!(placement, Anchor::TopCenter), |this| {
                    this.top_0().mx_auto()
                })
                .when(matches!(placement, Anchor::BottomRight), |this| {
                    this.bottom_0().right_0()
                })
                .when(matches!(placement, Anchor::BottomLeft), |this| {
                    this.bottom_0().left_0()
                })
                .when(matches!(placement, Anchor::BottomCenter), |this| {
                    this.bottom_0().mx_auto()
                })
                .when_some(mt, |this, offset| this.mt(offset))
                .when_some(mr, |this, offset| this.mr(offset))
                .when_some(mb, |this, offset| this.mb(offset))
                .when_some(ml, |this, offset| this.ml(offset))
                .child(root.read(cx).notification.clone()),
        )
    }

    /// Render the Sheet layer.
    pub fn render_sheet_layer(
        window: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement + use<>> {
        let root = window.root::<Root>()??;

        if let Some(active_sheet) = root.read(cx).active_sheet.clone() {
            let mut sheet = Sheet::new(window, cx);
            sheet = (active_sheet.builder)(sheet, window, cx);
            sheet.focus_handle = active_sheet.focus_handle.clone();
            sheet.placement = active_sheet.placement;

            let size = sheet.size;

            return Some(
                div()
                    .relative()
                    .child(sheet)
                    .on_prepaint(move |_, _, cx| root.update(cx, |r, _| r.sheet_size = Some(size))),
            );
        }

        None
    }

    /// Render the Dialog layer.
    pub fn render_dialog_layer(
        window: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement + use<>> {
        let root = window.root::<Root>()??;

        let active_dialogs = root.read(cx).active_dialogs.clone();

        if active_dialogs.is_empty() {
            return None;
        }

        let mut show_overlay_ix = None;

        let mut dialogs = active_dialogs
            .iter()
            .enumerate()
            .map(|(i, active_dialog)| {
                let mut dialog = Dialog::new(window, cx);

                dialog = (active_dialog.builder)(dialog, window, cx);
                dialog.layer_id = Some(active_dialog.id);

                // Give the dialog the focus handle, because `dialog` is a temporary value, is not possible to
                // keep the focus handle in the dialog.
                //
                // So we keep the focus handle in the `active_dialog`, this is owned by the `Root`.
                dialog.focus_handle = active_dialog.focus_handle.clone();

                dialog.layer_ix = i;
                // Find the dialog which one needs to show overlay.
                if dialog.has_overlay() {
                    show_overlay_ix = Some(i);
                }

                dialog
            })
            .collect::<Vec<_>>();

        if let Some(ix) = show_overlay_ix {
            if let Some(dialog) = dialogs.get_mut(ix) {
                dialog.overlay_visible = true;
            }
        }

        Some(div().children(dialogs))
    }

    pub fn open_dialog<F>(&mut self, build: F, window: &mut Window, cx: &mut Context<'_, Root>)
    where
        F: Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
    {
        let previous_focused_handle = window.focused(cx).map(|h| h.downgrade());
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        let id = RootDialogId {
            root_entity_id: cx.entity_id().as_u64(),
            generation: advance_layer_revision(&self.layer_revision),
        };
        self.active_dialogs.push(ActiveDialog::new(
            id,
            focus_handle,
            previous_focused_handle,
            build,
        ));
        cx.notify();
    }

    pub fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<'_, Root>) {
        if self.active_dialogs.is_empty() {
            return;
        }
        advance_layer_revision(&self.layer_revision);
        self.focused_input = None;
        if let Some(handle) = self
            .active_dialogs
            .pop()
            .and_then(|d| d.previous_focused_handle)
            .and_then(|h| h.upgrade())
        {
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    pub fn close_all_dialogs(&mut self, window: &mut Window, cx: &mut Context<'_, Root>) {
        if self.active_dialogs.is_empty() {
            return;
        }
        advance_layer_revision(&self.layer_revision);
        self.focused_input = None;
        let previous_focused_handle = self
            .active_dialogs
            .first()
            .and_then(|d| d.previous_focused_handle.clone());
        self.active_dialogs.clear();
        if let Some(handle) = previous_focused_handle.and_then(|h| h.upgrade()) {
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    pub fn open_sheet_at<F>(
        &mut self,
        placement: Placement,
        build: F,
        window: &mut Window,
        cx: &mut Context<'_, Root>,
    ) where
        F: Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static,
    {
        let previous_focused_handle = window.focused(cx).map(|h| h.downgrade());

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        self.active_sheet = Some(ActiveSheet {
            focus_handle,
            previous_focused_handle,
            placement,
            builder: Rc::new(build),
        });
        cx.notify();
    }

    pub fn close_sheet(&mut self, window: &mut Window, cx: &mut Context<'_, Root>) {
        self.focused_input = None;
        if let Some(previous_handle) = self
            .active_sheet
            .as_ref()
            .and_then(|s| s.previous_focused_handle.as_ref())
            .and_then(|h| h.upgrade())
        {
            window.focus(&previous_handle, cx);
        }
        self.active_sheet = None;
        cx.notify();
    }

    pub fn push_notification(
        &mut self,
        note: impl Into<Notification>,
        window: &mut Window,
        cx: &mut Context<'_, Root>,
    ) {
        self.notification
            .update(cx, |view, cx| view.push(note, window, cx));
        cx.notify();
    }

    pub fn remove_notification<T: Sized + 'static>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Root>,
    ) {
        self.notification.update(cx, |view, cx| {
            let id = TypeId::of::<T>();
            view.close(id, window, cx);
        });
        cx.notify();
    }

    pub fn clear_notifications(&mut self, window: &mut Window, cx: &mut Context<'_, Root>) {
        self.notification
            .update(cx, |view, cx| view.clear(window, cx));
        cx.notify();
    }

    /// Return the root view of the Root.
    pub fn view(&self) -> &AnyView {
        &self.view
    }

    fn on_action_tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_next(cx);
    }

    fn on_action_tab_prev(&mut self, _: &TabPrev, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_prev(cx);
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(cx.theme().font_size);

        window_border().child(
            div()
                .id("root")
                .key_context(CONTEXT)
                .on_action(cx.listener(Self::on_action_tab))
                .on_action(cx.listener(Self::on_action_tab_prev))
                .relative()
                .size_full()
                .font_family(cx.theme().font_family.clone())
                .when(self.paint_background, |this| this.bg(cx.theme().background))
                .text_color(cx.theme().foreground)
                .child(self.view.clone()),
        )
    }
}

#[cfg(test)]
mod layer_lifetime_tests {
    use super::*;
    use crate::WindowExt as _;
    use gpui::{AnyWindowHandle, TestAppContext, WindowHandle};
    use std::time::Duration;

    struct LayerTestView;
    impl Render for LayerTestView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .children(Root::render_dialog_layer(window, cx))
                .children(Root::render_notification_layer(window, cx))
        }
    }

    fn open_root(cx: &mut TestAppContext) -> WindowHandle<Root> {
        cx.update(crate::init);
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let content = cx.new(|_| LayerTestView);
                cx.new(|cx| Root::new(content, window, cx))
            })
            .expect("Root test window")
        })
    }

    fn snapshot(handle: WindowHandle<Root>, cx: &mut TestAppContext) -> RootLayerSnapshot {
        handle
            .read_with(cx, |root, cx| root.layer_snapshot(cx))
            .expect("live Root")
    }

    #[gpui::test]
    fn snapshots_are_read_only_and_dialog_ids_reject_replacement_lifetimes(
        cx: &mut TestAppContext,
    ) {
        let handle = open_root(cx);
        let window: AnyWindowHandle = handle.into();
        let initial = snapshot(handle, cx);
        window
            .update(cx, |_, window, cx| {
                window.open_dialog(cx, |dialog, _, _| dialog.title("First").confirm())
            })
            .unwrap();
        let first = snapshot(handle, cx);
        assert!(first.revision > initial.revision);
        assert_eq!(first, snapshot(handle, cx));
        let first_id = first.dialogs[0];
        assert!(
            window
                .update(cx, |_, window, cx| Root::close_dialog_if_current(
                    first_id, window, cx
                ))
                .unwrap()
        );
        let closed = snapshot(handle, cx);
        assert!(closed.dialogs.is_empty());
        assert!(closed.revision > first.revision);
        window
            .update(cx, |_, window, cx| {
                window.open_dialog(cx, |dialog, _, _| dialog.title("Replacement").confirm())
            })
            .unwrap();
        let replacement = snapshot(handle, cx);
        assert_ne!(first_id, replacement.dialogs[0]);
        assert!(
            !window
                .update(cx, |_, window, cx| Root::close_dialog_if_current(
                    first_id, window, cx
                ))
                .unwrap()
        );
        assert_eq!(replacement, snapshot(handle, cx));

        let other = open_root(cx);
        let other_window: AnyWindowHandle = other.into();
        other_window
            .update(cx, |_, window, cx| {
                window.open_dialog(cx, |dialog, _, _| dialog.confirm())
            })
            .unwrap();
        let other_before = snapshot(other, cx);
        assert!(
            !other_window
                .update(cx, |_, window, cx| Root::close_dialog_if_current(
                    replacement.dialogs[0],
                    window,
                    cx
                ))
                .unwrap()
        );
        assert_eq!(other_before, snapshot(other, cx));
    }

    #[gpui::test]
    fn expired_lower_dialog_cannot_pop_a_newer_modal_during_prepaint(cx: &mut TestAppContext) {
        let handle = open_root(cx);
        let window: AnyWindowHandle = handle.into();
        window
            .update(cx, |_, window, cx| {
                window.open_dialog(cx, |dialog, _, _| {
                    dialog.title("Expired owner").keep_open_while(|| false)
                });
                window.open_dialog(cx, |dialog, _, _| dialog.title("Current owner").confirm());
            })
            .unwrap();
        let before = snapshot(handle, cx);
        cx.run_until_parked();
        assert_eq!(snapshot(handle, cx).dialogs, before.dialogs);
        assert!(
            window
                .update(cx, |_, window, cx| Root::close_dialog_if_current(
                    before.dialogs[1],
                    window,
                    cx
                ))
                .unwrap()
        );
        cx.run_until_parked();
        assert!(snapshot(handle, cx).dialogs.is_empty());
    }

    #[gpui::test]
    fn notification_snapshot_tracks_real_closing_and_rejects_old_entity(cx: &mut TestAppContext) {
        let handle = open_root(cx);
        let window: AnyWindowHandle = handle.into();
        window
            .update(cx, |_, window, cx| {
                window
                    .push_notification(Notification::success("Old").id::<u8>().autohide(false), cx)
            })
            .unwrap();
        let first = snapshot(handle, cx);
        let old_id = first.notifications[0].entity_id;
        assert_eq!(first.notifications[0].message.as_deref(), Some("Old"));
        assert!(!first.notifications[0].closing);
        assert!(
            window
                .update(cx, |_, window, cx| Root::dismiss_notification_if_current(
                    old_id, window, cx
                ))
                .unwrap()
        );
        let closing = snapshot(handle, cx);
        assert!(closing.revision > first.revision);
        assert!(closing.notifications[0].closing);
        cx.run_until_parked();
        window
            .update(cx, |_, window, cx| {
                window.push_notification(
                    Notification::success("Replacement")
                        .id::<u8>()
                        .autohide(false),
                    cx,
                )
            })
            .unwrap();
        let replacement = snapshot(handle, cx);
        assert!(replacement.revision > closing.revision);
        assert_ne!(replacement.notifications[0].entity_id, old_id);
        assert!(
            !window
                .update(cx, |_, window, cx| Root::dismiss_notification_if_current(
                    old_id, window, cx
                ))
                .unwrap()
        );
        cx.background_executor
            .advance_clock(Duration::from_millis(150));
        cx.run_until_parked();
        assert_eq!(replacement, snapshot(handle, cx));
        let replacement_id = replacement.notifications[0].entity_id;
        assert!(
            window
                .update(cx, |_, window, cx| Root::dismiss_notification_if_current(
                    replacement_id,
                    window,
                    cx
                ))
                .unwrap()
        );
        cx.run_until_parked();
        cx.background_executor
            .advance_clock(Duration::from_millis(150));
        cx.run_until_parked();
        let removed = snapshot(handle, cx);
        assert!(removed.notifications.is_empty());
        assert!(removed.revision > replacement.revision);
    }
}
