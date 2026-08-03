use std::{
    any::TypeId,
    collections::{HashMap, VecDeque},
    rc::Rc,
    time::Duration,
};

use crate::{
    ActiveTheme as _, Anchor, Edges, Icon, IconName, Sizable as _, StyledExt, TITLE_BAR_HEIGHT,
    animation::cubic_bezier,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, ClickEvent, Context, DismissEvent,
    ElementId, Entity, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, SharedString, StatefulInteractiveElement, StyleRefinement,
    Styled, Subscription, WeakFocusHandle, Window, div, prelude::FluentBuilder, px,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default)]
pub enum NotificationType {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationType {
    fn icon(&self, cx: &App) -> Icon {
        match self {
            Self::Info => Icon::new(IconName::Info).text_color(cx.theme().info),
            Self::Success => Icon::new(IconName::CircleCheck).text_color(cx.theme().success),
            Self::Warning => Icon::new(IconName::TriangleAlert).text_color(cx.theme().warning),
            Self::Error => Icon::new(IconName::CircleX).text_color(cx.theme().danger),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub(crate) enum NotificationId {
    Id(TypeId),
    IdAndElementId(TypeId, ElementId),
}

impl From<TypeId> for NotificationId {
    fn from(type_id: TypeId) -> Self {
        Self::Id(type_id)
    }
}

impl From<(TypeId, ElementId)> for NotificationId {
    fn from((type_id, id): (TypeId, ElementId)) -> Self {
        Self::IdAndElementId(type_id, id)
    }
}

/// A notification element.
pub struct Notification {
    /// The id is used make the notification unique.
    /// Then you push a notification with the same id, the previous notification will be replaced.
    ///
    /// None means the notification will be added to the end of the list.
    id: NotificationId,
    root_element_id: ElementId,
    style: StyleRefinement,
    type_: Option<NotificationType>,
    title: Option<SharedString>,
    message: Option<SharedString>,
    icon: Option<Icon>,
    autohide: bool,
    autohide_duration: Duration,
    action_builder: Option<Rc<dyn Fn(&mut Self, &mut Window, &mut Context<Self>) -> Button>>,
    content_builder: Option<Rc<dyn Fn(&mut Self, &mut Window, &mut Context<Self>) -> AnyElement>>,
    content_only: bool,
    dismissible: bool,
    focus_handle: Option<FocusHandle>,
    previous_focused_handle: Option<WeakFocusHandle>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    closing: bool,
}

impl From<String> for Notification {
    fn from(s: String) -> Self {
        Self::new().message(s)
    }
}

impl From<SharedString> for Notification {
    fn from(s: SharedString) -> Self {
        Self::new().message(s)
    }
}

impl From<&'static str> for Notification {
    fn from(s: &'static str) -> Self {
        Self::new().message(s)
    }
}

impl From<(NotificationType, &'static str)> for Notification {
    fn from((type_, content): (NotificationType, &'static str)) -> Self {
        Self::new().message(content).with_type(type_)
    }
}

impl From<(NotificationType, SharedString)> for Notification {
    fn from((type_, content): (NotificationType, SharedString)) -> Self {
        Self::new().message(content).with_type(type_)
    }
}

struct DefaultIdType;

impl Notification {
    /// Create a new notification.
    ///
    /// The default id is a random UUID.
    pub fn new() -> Self {
        let id: SharedString = uuid::Uuid::new_v4().to_string().into();
        let root_element_id: ElementId = id.clone().into();
        let id = (TypeId::of::<DefaultIdType>(), root_element_id.clone());

        Self {
            id: id.into(),
            root_element_id,
            style: StyleRefinement::default(),
            title: None,
            message: None,
            type_: None,
            icon: None,
            autohide: true,
            autohide_duration: Duration::from_secs(5),
            action_builder: None,
            content_builder: None,
            content_only: false,
            dismissible: true,
            focus_handle: None,
            previous_focused_handle: None,
            on_click: None,
            closing: false,
        }
    }

    /// Set the message of the notification, default is None.
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Create an info notification with the given message.
    pub fn info(message: impl Into<SharedString>) -> Self {
        Self::new()
            .message(message)
            .with_type(NotificationType::Info)
    }

    /// Create a success notification with the given message.
    pub fn success(message: impl Into<SharedString>) -> Self {
        Self::new()
            .message(message)
            .with_type(NotificationType::Success)
    }

    /// Create a warning notification with the given message.
    pub fn warning(message: impl Into<SharedString>) -> Self {
        Self::new()
            .message(message)
            .with_type(NotificationType::Warning)
    }

    /// Create an error notification with the given message.
    pub fn error(message: impl Into<SharedString>) -> Self {
        Self::new()
            .message(message)
            .with_type(NotificationType::Error)
    }

    /// Set the type for unique identification of the notification.
    ///
    /// ```rs
    /// struct MyNotificationKind;
    /// let notification = Notification::new("Hello").id::<MyNotificationKind>();
    /// ```
    pub fn id<T: Sized + 'static>(mut self) -> Self {
        self.id = TypeId::of::<T>().into();
        self.root_element_id = std::any::type_name::<T>().into();
        self
    }

    /// Set the type and id of the notification, used to uniquely identify the notification.
    pub fn id1<T: Sized + 'static>(mut self, key: impl Into<ElementId>) -> Self {
        let key = key.into();
        self.id = (TypeId::of::<T>(), key.clone()).into();
        self.root_element_id = key;
        self
    }

    /// Set the title of the notification, default is None.
    ///
    /// If title is None, the notification will not have a title.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the icon of the notification.
    ///
    /// If icon is None, the notification will use the default icon of the type.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set the type of the notification, default is NotificationType::Info.
    pub fn with_type(mut self, type_: NotificationType) -> Self {
        self.type_ = Some(type_);
        self
    }

    /// Set the auto hide of the notification, default is true.
    pub fn autohide(mut self, autohide: bool) -> Self {
        self.autohide = autohide;
        self
    }

    /// Set the exact active-focus time before automatic dismissal.
    pub fn autohide_after(mut self, duration: Duration) -> Self {
        self.autohide = true;
        self.autohide_duration = duration;
        self
    }

    /// Hide the built-in close control when custom content owns dismissal.
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Set the click callback of the notification.
    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// Set the action button of the notification.
    ///
    /// When an action is set, the notification will not autohide.
    pub fn action<F>(mut self, action: F) -> Self
    where
        F: Fn(&mut Self, &mut Window, &mut Context<Self>) -> Button + 'static,
    {
        self.action_builder = Some(Rc::new(action));
        self.autohide = false;
        self
    }

    /// Dismiss the notification.
    pub fn dismiss(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
        cx.notify();

        // Dismiss the notification after 0.15s to show the animation.
        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            cx.update(|cx| {
                if let Some(view) = view.upgrade() {
                    view.update(cx, |view, cx| {
                        view.closing = false;
                        cx.emit(DismissEvent);
                    });
                }
            })
        })
        .detach()
    }

    /// Set the content of the notification.
    pub fn content(
        mut self,
        content: impl Fn(&mut Self, &mut Window, &mut Context<Self>) -> AnyElement + 'static,
    ) -> Self {
        self.content_builder = Some(Rc::new(content));
        self
    }

    /// Render custom content as the notification root while retaining queue,
    /// focus-return, and autohide lifecycle ownership.
    pub fn content_only(
        mut self,
        content: impl Fn(&mut Self, &mut Window, &mut Context<Self>) -> AnyElement + 'static,
    ) -> Self {
        self.content_builder = Some(Rc::new(content));
        self.content_only = true;
        self.dismissible = false;
        self
    }

    pub fn dismiss_from_control(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self
            .previous_focused_handle
            .as_ref()
            .and_then(WeakFocusHandle::upgrade)
        {
            window.focus(&handle, cx);
        }
        self.dismiss(window, cx);
    }

    pub(crate) fn controls_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus_handle
            .as_ref()
            .is_some_and(|handle| handle.contains_focused(window, cx))
    }
}
impl EventEmitter<DismissEvent> for Notification {}
impl FluentBuilder for Notification {}
impl Styled for Notification {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl Render for Notification {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = window
            .use_keyed_state(self.root_element_id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        self.focus_handle = Some(focus_handle.clone());
        let content = self
            .content_builder
            .clone()
            .map(|builder| builder(self, window, cx));
        if self.content_only {
            return div()
                .id(self.root_element_id.clone())
                .track_focus(&focus_handle.tab_stop(false))
                .when_some(content, |this, content| this.child(content))
                .into_any_element();
        }
        let action = self
            .action_builder
            .clone()
            .map(|builder| builder(self, window, cx).small().mr_3p5());

        let closing = self.closing;
        let icon = match self.type_ {
            None => self.icon.clone(),
            Some(type_) => Some(type_.icon(cx)),
        };
        let has_icon = icon.is_some();
        let placement = cx.theme().notification.placement;

        h_flex()
            .id(self.root_element_id.clone())
            .track_focus(&focus_handle.tab_stop(false))
            .group("")
            .occlude()
            .relative()
            .w_112()
            .border_1()
            .border_color(cx.theme().border)
            .when(self.style.background.is_none(), |this| {
                this.bg(cx.theme().popover)
            })
            .rounded(cx.theme().radius_lg)
            .shadow_md()
            .py_3p5()
            .px_4()
            .gap_3()
            .refine_style(&self.style)
            .when_some(icon, |this, icon| {
                this.child(div().absolute().py_3p5().left_4().child(icon))
            })
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .when(has_icon, |this| this.pl_6())
                    .when_some(self.title.clone(), |this, title| {
                        this.child(div().text_sm().font_semibold().child(title))
                    })
                    .when_some(self.message.clone(), |this, message| {
                        this.child(div().text_sm().child(message))
                    })
                    .when_some(content, |this, content| this.child(content)),
            )
            .when_some(action, |this, action| this.child(action))
            .when(self.dismissible, |this| {
                this.child(
                    div().absolute().top_1().right_1().child(
                        Button::new("notification-dismiss")
                            .icon(IconName::Close)
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.dismiss_from_control(window, cx)
                            })),
                    ),
                )
            })
            .when_some(self.on_click.clone(), |this, on_click| {
                this.on_click(cx.listener(move |view, event, window, cx| {
                    view.dismiss(window, cx);
                    on_click(event, window, cx);
                }))
            })
            .with_animation(
                ElementId::NamedInteger("slide-down".into(), closing as u64),
                Animation::new(Duration::from_secs_f64(0.25))
                    .with_easing(cubic_bezier(0.4, 0., 0.2, 1.)),
                move |this, delta| {
                    if closing {
                        let opacity = 1. - delta;
                        let that = this
                            .shadow_none()
                            .opacity(opacity)
                            .when(opacity < 0.85, |this| this.shadow_none());
                        match placement {
                            Anchor::TopRight | Anchor::BottomRight => {
                                let x_offset = px(0.) + delta * px(45.);
                                that.left(px(0.) + x_offset)
                            }
                            Anchor::TopLeft | Anchor::BottomLeft => {
                                let x_offset = px(0.) - delta * px(45.);
                                that.left(px(0.) + x_offset)
                            }
                            Anchor::TopCenter => {
                                let y_offset = px(0.) - delta * px(45.);
                                that.top(px(0.) + y_offset)
                            }
                            Anchor::BottomCenter => {
                                let y_offset = px(0.) + delta * px(45.);
                                that.top(px(0.) + y_offset)
                            }
                        }
                    } else {
                        let y_offset = match placement {
                            placement if placement.is_top() => px(-45.) + delta * px(45.),
                            placement if placement.is_bottom() => px(45.) - delta * px(45.),
                            _ => px(0.),
                        };
                        let opacity = delta;
                        this.top(px(0.) + y_offset)
                            .opacity(opacity)
                            .when(opacity < 0.85, |this| this.shadow_none())
                    }
                },
            )
            .into_any_element()
    }
}

/// The settings for notifications.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NotificationSettings {
    /// The placement of the notification, default: [`Anchor::TopRight`]
    pub placement: Anchor,
    /// The margins of the notification with respect to the window edges.
    pub margins: Edges<Pixels>,
    /// The maximum number of notifications to show at once, default: 10
    pub max_items: usize,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        let offset = px(16.);
        Self {
            placement: Anchor::TopRight,
            margins: Edges {
                top: TITLE_BAR_HEIGHT + offset, // avoid overlap with title bar
                right: offset,
                bottom: offset,
                left: offset,
            },
            max_items: 10,
        }
    }
}

/// A list of notifications.
pub struct NotificationList {
    /// Notifications that will be auto hidden.
    pub(crate) notifications: VecDeque<Entity<Notification>>,
    expanded: bool,
    _subscriptions: HashMap<NotificationId, Subscription>,
}

impl NotificationList {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            notifications: VecDeque::new(),
            expanded: false,
            _subscriptions: HashMap::new(),
        }
    }

    pub fn push(
        &mut self,
        notification: impl Into<Notification>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut notification = notification.into();
        notification.previous_focused_handle = window.focused(cx).map(|handle| handle.downgrade());
        let id = notification.id.clone();
        let autohide = notification.autohide;
        let autohide_duration = notification.autohide_duration;

        // Remove the notification by id, for keep unique.
        self.notifications.retain(|note| note.read(cx).id != id);

        let notification = cx.new(|_| notification);

        self._subscriptions.insert(
            id.clone(),
            cx.subscribe(&notification, move |view, _, _: &DismissEvent, cx| {
                view.notifications.retain(|note| id != note.read(cx).id);
                view._subscriptions.remove(&id);
            }),
        );

        self.notifications.push_back(notification.clone());
        if autohide {
            // Count only time when no notification control owns focus. This
            // preserves the configured duration while keyboard users read and
            // operate actions; replacing a notification cannot let its old
            // entity timer dismiss the replacement generation.
            cx.spawn_in(window, async move |_, cx| {
                const TICK: Duration = Duration::from_millis(25);
                let mut remaining = autohide_duration;
                while remaining > Duration::ZERO {
                    cx.background_executor().timer(TICK.min(remaining)).await;
                    let controls_focused = match notification
                        .update_in(cx, |note, window, cx| note.controls_focused(window, cx))
                    {
                        Ok(focused) => focused,
                        Err(_) => return,
                    };
                    if !controls_focused {
                        remaining = remaining.saturating_sub(TICK);
                    }
                }

                if let Err(err) =
                    notification.update_in(cx, |note, window, cx| note.dismiss(window, cx))
                {
                    tracing::error!("failed to auto hide notification: {:?}", err);
                }
            })
            .detach();
        }
        cx.notify();
    }

    pub(crate) fn close(
        &mut self,
        id: impl Into<NotificationId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id: NotificationId = id.into();
        if let Some(n) = self.notifications.iter().find(|n| n.read(cx).id == id) {
            n.update(cx, |note, cx| note.dismiss(window, cx))
        }
        cx.notify();
    }

    pub fn clear(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.notifications.clear();
        cx.notify();
    }

    pub fn notifications(&self) -> Vec<Entity<Notification>> {
        self.notifications.iter().cloned().collect()
    }
}

impl Render for NotificationList {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let size = window.viewport_size();
        let items = self.notifications.iter().rev().take(10).rev().cloned();

        let placement = cx.theme().notification.placement;
        let margins = &cx.theme().notification.margins;

        v_flex()
            .id("notification-list")
            .max_h(size.height)
            .pt(margins.top)
            .pb(margins.bottom)
            .gap_3()
            .when(
                matches!(placement, Anchor::TopRight),
                |this| this.pr(margins.right), // ignore left
            )
            .when(
                matches!(placement, Anchor::TopLeft),
                |this| this.pl(margins.left), // ignore right
            )
            .when(
                matches!(placement, Anchor::BottomLeft),
                |this| this.flex_col_reverse().pl(margins.left), // ignore right
            )
            .when(
                matches!(placement, Anchor::BottomRight),
                |this| this.flex_col_reverse().pr(margins.right), // ignore left
            )
            .when(matches!(placement, Anchor::BottomCenter), |this| {
                this.flex_col_reverse()
            })
            .on_hover(cx.listener(|view, hovered, _, cx| {
                view.expanded = *hovered;
                cx.notify()
            }))
            .children(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Keystroke, TestAppContext};

    fn open_notification_list(cx: &mut TestAppContext) -> gpui::WindowHandle<NotificationList> {
        cx.update(crate::init);
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| NotificationList::new(window, cx))
            })
            .expect("notification test window should open")
        })
    }

    fn push(
        window_handle: &gpui::WindowHandle<NotificationList>,
        cx: &mut TestAppContext,
        notification: Notification,
    ) -> Entity<Notification> {
        window_handle
            .update(cx, |list, window, cx| {
                list.push(notification, window, cx);
                list.notifications
                    .back()
                    .expect("pushed notification should be retained")
                    .clone()
            })
            .expect("notification test window should remain available")
    }

    fn advance(cx: &mut TestAppContext, duration: Duration) {
        let mut remaining = duration;
        while remaining > Duration::ZERO {
            let step = Duration::from_millis(25).min(remaining);
            cx.background_executor.advance_clock(step);
            cx.run_until_parked();
            remaining = remaining.saturating_sub(step);
        }
    }

    #[gpui::test]
    fn focused_controls_pause_exact_autohide_budget_until_focus_leaves(cx: &mut TestAppContext) {
        let window_handle = open_notification_list(cx);
        let prior_focus = window_handle
            .update(cx, |_, window, cx| {
                let handle = cx.focus_handle();
                window.focus(&handle, cx);
                handle
            })
            .expect("notification test window should remain available");
        let notification = push(
            &window_handle,
            cx,
            Notification::new()
                .id1::<u8>("focus-pause")
                .message("Focus pauses me")
                .autohide_after(Duration::from_millis(100)),
        );
        cx.run_until_parked();

        window_handle
            .update(cx, |_, window, cx| {
                let notification_focus = notification
                    .read(cx)
                    .focus_handle
                    .clone()
                    .expect("rendered notification should publish a focus root");
                window.focus(&notification_focus, cx);
            })
            .expect("notification test window should remain available");
        advance(cx, Duration::from_millis(250));
        assert!(!notification.read_with(cx, |note, _| note.closing));

        window_handle
            .update(cx, |_, window, cx| window.focus(&prior_focus, cx))
            .expect("notification test window should remain available");
        advance(cx, Duration::from_millis(75));
        assert!(!notification.read_with(cx, |note, _| note.closing));
        advance(cx, Duration::from_millis(25));
        assert!(notification.read_with(cx, |note, _| note.closing));
    }

    #[gpui::test]
    fn dismiss_from_control_restores_prior_focus_and_removes_the_entity(cx: &mut TestAppContext) {
        let window_handle = open_notification_list(cx);
        let prior_focus = window_handle
            .update(cx, |_, window, cx| {
                let handle = cx.focus_handle();
                window.focus(&handle, cx);
                handle
            })
            .expect("notification test window should remain available");
        let notification = push(
            &window_handle,
            cx,
            Notification::new()
                .id1::<u8>("focus-return")
                .message("Return focus")
                .autohide_after(Duration::from_secs(10)),
        );
        cx.run_until_parked();

        window_handle
            .update(cx, |_, window, cx| {
                let notification_focus = notification
                    .read(cx)
                    .focus_handle
                    .clone()
                    .expect("rendered notification should publish a focus root");
                window.focus(&notification_focus, cx);
                notification.update(cx, |note, cx| note.dismiss_from_control(window, cx));
                assert!(prior_focus.is_focused(window));
            })
            .expect("notification test window should remain available");
        cx.run_until_parked();
        advance(cx, Duration::from_millis(150));
        assert!(
            window_handle
                .read_with(cx, |list, _| list.notifications.is_empty())
                .expect("notification test window should remain available")
        );
    }

    #[gpui::test]
    fn stale_timer_cannot_dismiss_same_id_replacement(cx: &mut TestAppContext) {
        let window_handle = open_notification_list(cx);
        let old = push(
            &window_handle,
            cx,
            Notification::new()
                .id1::<u8>("replace-me")
                .message("Old")
                .autohide_after(Duration::from_millis(50)),
        );
        advance(cx, Duration::from_millis(25));
        let replacement = push(
            &window_handle,
            cx,
            Notification::new()
                .id1::<u8>("replace-me")
                .message("Replacement")
                .autohide(false),
        );
        assert_ne!(old.entity_id(), replacement.entity_id());

        advance(cx, Duration::from_millis(175));
        assert!(
            window_handle
                .read_with(cx, |list, _| {
                    list.notifications.len() == 1
                        && list.notifications[0].entity_id() == replacement.entity_id()
                })
                .expect("notification test window should remain available")
        );
    }

    #[gpui::test]
    fn built_in_dismiss_button_accepts_space_from_real_dispatch(cx: &mut TestAppContext) {
        let window_handle = open_notification_list(cx);
        let notification = push(
            &window_handle,
            cx,
            Notification::new()
                .id1::<u8>("keyboard-dismiss")
                .message("Dismiss with Space")
                .autohide(false),
        );
        cx.run_until_parked();

        window_handle
            .update(cx, |_, window, cx| window.focus_next(cx))
            .expect("notification test window should remain available");
        cx.dispatch_keystroke(
            *window_handle,
            Keystroke::parse("space").expect("valid Space keystroke"),
        );
        assert!(notification.read_with(cx, |note, _| note.closing));
    }
}
