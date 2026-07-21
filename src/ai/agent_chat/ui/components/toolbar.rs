use gpui::{
    div, prelude::*, px, rgb, rgba, App, Context, Entity, FocusHandle, IntoElement, ParentElement,
    Render, Window,
};

use super::super::thread::AgentChatThreadStatus;
use super::super::types::AgentChatComposerParentWindow;
use crate::theme;

pub enum AgentChatToolbarEvent {
    ToggleProfileSelector(AgentChatComposerParentWindow),
    ToggleModelSelector(AgentChatComposerParentWindow),
    ExportThread,
    ClearThread,
    OpenHistory,
    CloseChat,
}

impl gpui::EventEmitter<AgentChatToolbarEvent> for AgentChatToolbar {}

pub struct AgentChatToolbar {
    status: AgentChatThreadStatus,
    profile_name: String,
    model_name: String,
    focus_handle: FocusHandle,
}

impl AgentChatToolbar {
    pub fn new(
        status: AgentChatThreadStatus,
        profile_name: String,
        model_name: String,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            status,
            profile_name,
            model_name,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_status(&mut self, status: AgentChatThreadStatus, cx: &mut Context<Self>) {
        self.status = status;
        cx.notify();
    }

    pub fn set_model_name(&mut self, model_name: String, cx: &mut Context<Self>) {
        self.model_name = model_name;
        cx.notify();
    }

    pub fn set_profile_name(&mut self, profile_name: String, cx: &mut Context<Self>) {
        self.profile_name = profile_name;
        cx.notify();
    }
}

impl Render for AgentChatToolbar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme::get_cached_theme();
        let rail = crate::components::footer_chrome::footer_rail_chrome(&theme);

        div()
            .id("agent_chat-toolbar")
            .w_full()
            .h(px(rail.height_px))
            .min_h(px(rail.height_px))
            .flex()
            .items_center()
            .justify_between()
            .px(px(rail.side_inset_px))
            .bg(if theme.is_vibrancy_enabled() {
                rgba(0x00000000)
            } else {
                rgb(theme.colors.background.main)
            })
            .border_b_1()
            .border_color(rgb(theme.colors.ui.border))
            .child(
                div()
                    .id("agent-chat-profile-selector")
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .cursor_pointer()
                    .on_click(cx.listener(|_view, _event, window, cx| {
                        let parent = toolbar_parent_window(window, cx);
                        cx.emit(AgentChatToolbarEvent::ToggleProfileSelector(parent));
                    }))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme.colors.text.primary))
                            .child(self.profile_name.clone()),
                    ),
            )
            .child(
                div()
                    .id("agent_chat-toolbar-model-selector")
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .cursor_pointer()
                    .on_click(cx.listener(|_view, _event, window, cx| {
                        let parent = toolbar_parent_window(window, cx);
                        cx.emit(AgentChatToolbarEvent::ToggleModelSelector(parent));
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme.colors.text.muted))
                            .child(self.model_name.clone()),
                    ),
            )
            .child(
                div().flex().items_center().gap(px(12.0)).child(
                    div()
                        .text_xs()
                        .text_color(rgb(theme.colors.text.muted))
                        .child(format!("{:?}", self.status)),
                ),
            )
    }
}

fn toolbar_parent_window(window: &mut Window, cx: &mut App) -> AgentChatComposerParentWindow {
    let display = window.display(cx);
    AgentChatComposerParentWindow {
        handle: window.window_handle(),
        bounds: window.bounds(),
        display_id: display.as_ref().map(|display| display.id()),
        display_bounds: display.as_ref().map(|display| display.visible_bounds()),
    }
}
