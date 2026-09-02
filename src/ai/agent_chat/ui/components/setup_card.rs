use gpui::{
    div, prelude::*, px, rgb, rgba, Context, FocusHandle, IntoElement, KeyDownEvent, ParentElement,
    Render, Window,
};

use super::super::catalog::{
    AgentChatAgentAuthState, AgentChatAgentCatalogEntry, AgentChatAgentConfigState,
    AgentChatAgentInstallState,
};
use super::super::preflight::AgentChatLaunchRequirements;
use super::super::setup_state::{AgentChatInlineSetupState, AgentChatSetupAction};
use crate::theme;
use crate::ui_foundation;

/// State for the setup-mode agent selection picker.
#[derive(Debug, Clone)]
pub struct AgentChatSetupAgentPickerState {
    pub items: Vec<AgentChatAgentCatalogEntry>,
    pub selected_index: usize,
    pub visible_start: usize,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum AgentChatSetupCardEvent {
    ConfirmAgent(AgentChatAgentCatalogEntry),
    CancelPicker,
    ActivateAction(AgentChatSetupAction),
}

impl gpui::EventEmitter<AgentChatSetupCardEvent> for AgentChatSetupCard {}

pub struct AgentChatSetupCard {
    state: AgentChatInlineSetupState,
    pub(crate) agent_picker: Option<AgentChatSetupAgentPickerState>,
    focus_handle: FocusHandle,
    interaction_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SetupActionControl {
    label: &'static str,
    activation: AgentChatSetupAction,
}

fn setup_action_control(action: AgentChatSetupAction) -> SetupActionControl {
    match action {
        AgentChatSetupAction::Retry => SetupActionControl {
            label: "Retry",
            activation: AgentChatSetupAction::Retry,
        },
        // Installation and authentication happen in the selected agent's own
        // CLI. The useful in-app action afterward is a real retry, not the
        // previous no-op event advertised as though Script Kit performed it.
        AgentChatSetupAction::Install | AgentChatSetupAction::Authenticate => SetupActionControl {
            label: "Retry",
            activation: AgentChatSetupAction::Retry,
        },
        AgentChatSetupAction::OpenCatalog => SetupActionControl {
            label: "Open Agent Catalog",
            activation: AgentChatSetupAction::OpenCatalog,
        },
        AgentChatSetupAction::SelectAgent => SetupActionControl {
            label: "Choose Agent",
            activation: AgentChatSetupAction::SelectAgent,
        },
    }
}

impl AgentChatSetupCard {
    pub(crate) fn interaction_revision(&self) -> u64 {
        self.interaction_revision
    }

    fn advance_interaction_revision(&mut self) {
        self.interaction_revision = self.interaction_revision.strict_add(1);
    }

    pub fn new(
        state: AgentChatInlineSetupState,
        agent_picker: Option<AgentChatSetupAgentPickerState>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            state,
            agent_picker,
            focus_handle: cx.focus_handle(),
            interaction_revision: 0,
        }
    }

    pub fn set_state(&mut self, state: AgentChatInlineSetupState, cx: &mut Context<Self>) {
        self.state = state;
        cx.notify();
    }

    pub fn set_agent_picker(
        &mut self,
        picker: Option<AgentChatSetupAgentPickerState>,
        cx: &mut Context<Self>,
    ) {
        if self.agent_picker.is_none() && picker.is_none() {
            return;
        }
        self.agent_picker = picker;
        self.advance_interaction_revision();
        cx.notify();
    }

    pub fn select_agent_by_id(&mut self, agent_id: &str, cx: &mut Context<Self>) -> bool {
        if let Some(ref mut picker) = self.agent_picker {
            if let Some(idx) = picker.items.iter().position(|item| item.id == agent_id) {
                if picker.selected_index != idx {
                    picker.selected_index = idx;
                    self.advance_interaction_revision();
                }
                cx.notify();
                return true;
            }
        }
        false
    }

    /// Project only the controls the card currently renders, using its real
    /// action mapping and picker state rather than the view's legacy picker.
    pub(crate) fn collect_semantic_elements(&self) -> Vec<crate::protocol::ElementInfo> {
        use crate::protocol::{ElementContentKind, ElementInfo, ElementType};

        let control_element = |semantic_id, element_type, text, kind: &str| ElementInfo {
            semantic_id,
            element_type,
            text: Some(text),
            value: None,
            content: None,
            selected: None,
            focused: None,
            index: None,
            role: Some("agentChatSetupControl".into()),
            kind: Some(kind.into()),
            source: Some("AgentChatSetupCard".into()),
            source_name: None,
            selectable: Some(true),
            status_kind: None,
            action_disabled: None,
            style: None,
        };

        if let Some(picker) = &self.agent_picker {
            return picker
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let mut element = control_element(
                        format!("agent_chat-setup-agent-{index}"),
                        ElementType::Choice,
                        item.display_name.to_string(),
                        "selectAgent",
                    );
                    element.value = Some(item.id.to_string());
                    element.selected = Some(index == picker.selected_index);
                    element.index = Some(index);
                    element.status_kind = Some(format_setup_agent_picker_status(
                        item,
                        self.state.launch_requirements,
                    ));
                    element.redact_content(ElementContentKind::UserContent)
                })
                .collect();
        }

        [
            Some((
                "agent-chat-setup-primary-action",
                self.state.primary_action,
                Some("↵"),
            )),
            self.state
                .secondary_action
                .map(|action| ("agent-chat-setup-secondary-action", action, None)),
        ]
        .into_iter()
        .flatten()
        .map(|(id, action, shortcut)| {
            let control = setup_action_control(action);
            let kind = match control.activation {
                AgentChatSetupAction::Retry => "retry",
                AgentChatSetupAction::Install => "install",
                AgentChatSetupAction::Authenticate => "authenticate",
                AgentChatSetupAction::OpenCatalog => "openCatalog",
                AgentChatSetupAction::SelectAgent => "selectAgent",
            };
            let mut element =
                control_element(id.into(), ElementType::Button, control.label.into(), kind);
            element.value = shortcut.map(str::to_string);
            element
        })
        .collect()
    }

    pub fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let key = event.keystroke.key.as_str();

        if let Some(ref mut picker) = self.agent_picker {
            if ui_foundation::is_key_up(key) {
                if picker.selected_index > 0 {
                    picker.selected_index -= 1;
                    self.advance_interaction_revision();
                }
                cx.notify();
                return true;
            }
            if ui_foundation::is_key_down(key) {
                if picker.selected_index + 1 < picker.items.len() {
                    picker.selected_index += 1;
                    self.advance_interaction_revision();
                }
                cx.notify();
                return true;
            }
            if ui_foundation::is_key_enter(key) {
                if let Some(agent) = picker.items.get(picker.selected_index).cloned() {
                    cx.emit(AgentChatSetupCardEvent::ConfirmAgent(agent));
                }
                return true;
            }
            if ui_foundation::is_key_escape(key) {
                self.agent_picker = None;
                self.advance_interaction_revision();
                cx.emit(AgentChatSetupCardEvent::CancelPicker);
                cx.notify();
                return true;
            }
            return false;
        }

        // Tab is deliberately unbound here: the Retry event it used to emit
        // is a no-op in the subscription handler, and the card's hints only
        // document Enter. An unadvertised half-dead Tab binding just makes
        // the displayed Tab information wrong.

        if ui_foundation::is_key_enter(key) {
            cx.emit(AgentChatSetupCardEvent::ActivateAction(
                setup_action_control(self.state.primary_action).activation,
            ));
            return true;
        }

        false
    }

    fn render_agent_picker(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let picker = self.agent_picker.as_ref()?;
        let theme = theme::get_cached_theme();

        let rows: Vec<gpui::AnyElement> = picker
            .items
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                let is_selected = ix == picker.selected_index;
                let status_text =
                    format_setup_agent_picker_status(item, self.state.launch_requirements);
                div()
                    .id(gpui::ElementId::Name(
                        format!("agent_chat-setup-agent-{ix}").into(),
                    ))
                    .w_full()
                    .px(px(10.0))
                    .py(px(5.0))
                    .when(is_selected, |d| {
                        d.bg(rgba((theme.colors.accent.selected << 8) | 0x14))
                            .border_l_2()
                            .border_color(rgb(theme.colors.accent.selected))
                    })
                    .when(!is_selected, |d| {
                        d.border_l_2().border_color(gpui::transparent_black())
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(theme.colors.text.primary))
                                    .child(item.display_name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme.colors.text.muted))
                                    .child(status_text),
                            ),
                    )
                    .into_any_element()
            })
            .collect();

        Some(
            div()
                .w_full()
                .max_w(px(400.0))
                .max_h(px(300.0))
                .overflow_y_hidden()
                .bg(rgb(theme.colors.background.main))
                .border_1()
                .border_color(rgb(theme.colors.ui.border))
                .rounded_md()
                .children(rows)
                .into_any_element(),
        )
    }
}

impl Render for AgentChatSetupCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme::get_cached_theme();
        let title = self.state.title.clone();
        let body = self.state.body.clone();
        let agent_name = self
            .state
            .selected_agent
            .as_ref()
            .map(|agent| agent.display_name.clone());
        let primary = setup_action_control(self.state.primary_action);
        let secondary = self.state.secondary_action.map(setup_action_control);
        let info = crate::components::render_info_state(
            crate::components::agent_setup_info_spec(title, body, agent_name),
            &theme,
            cx,
        );
        let button_colors = crate::components::ButtonColors::from_theme(&theme);

        let primary_button = crate::components::Button::new(
            "agent-chat-setup-primary-action",
            primary.label,
            button_colors,
        )
        .shortcut("↵")
        .on_click(Box::new(cx.listener(move |_this, _, _window, cx| {
            cx.emit(AgentChatSetupCardEvent::ActivateAction(primary.activation));
            cx.stop_propagation();
        })));

        let secondary_button = secondary.map(|control| {
            crate::components::Button::new(
                "agent-chat-setup-secondary-action",
                control.label,
                button_colors,
            )
            .variant(crate::components::ButtonVariant::Ghost)
            .on_click(Box::new(cx.listener(move |_this, _, _window, cx| {
                cx.emit(AgentChatSetupCardEvent::ActivateAction(control.activation));
                cx.stop_propagation();
            })))
            .into_any_element()
        });

        let actions = if self.agent_picker.is_none() {
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(crate::components::INFO_SPACING.xs))
                    .child(primary_button)
                    .children(secondary_button)
                    .into_any_element(),
            )
        } else {
            None
        };
        let picker = self.render_agent_picker(window, cx);

        div()
            .id("agent_chat-inline-setup")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(crate::components::INFO_SPACING.md))
            .track_focus(&self.focus_handle)
            .child(
                div()
                    .w_full()
                    .max_w(px(crate::components::info_metrics(
                        crate::components::InfoStateDensity::Comfortable,
                    )
                    .max_width))
                    .child(info),
            )
            .children(actions)
            .children(picker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_action_controls_only_advertise_actions_the_card_can_perform() {
        assert_eq!(
            setup_action_control(AgentChatSetupAction::SelectAgent),
            SetupActionControl {
                label: "Choose Agent",
                activation: AgentChatSetupAction::SelectAgent,
            }
        );
        assert_eq!(
            setup_action_control(AgentChatSetupAction::OpenCatalog).label,
            "Open Agent Catalog"
        );
        for external_action in [
            AgentChatSetupAction::Install,
            AgentChatSetupAction::Authenticate,
        ] {
            let control = setup_action_control(external_action);
            assert_eq!(control.label, "Retry");
            assert_eq!(control.activation, AgentChatSetupAction::Retry);
        }
    }
}

fn setup_agent_install_label(state: AgentChatAgentInstallState) -> &'static str {
    match state {
        AgentChatAgentInstallState::Ready => "ready",
        AgentChatAgentInstallState::NeedsInstall => "install",
        AgentChatAgentInstallState::Unsupported => "unsupported",
    }
}

fn setup_agent_auth_label(state: AgentChatAgentAuthState) -> &'static str {
    match state {
        AgentChatAgentAuthState::Unknown => "auth?",
        AgentChatAgentAuthState::Authenticated => "authed",
        AgentChatAgentAuthState::NeedsAuthentication => "login",
    }
}

fn setup_agent_config_label(state: AgentChatAgentConfigState) -> &'static str {
    match state {
        AgentChatAgentConfigState::Valid => "config-ok",
        AgentChatAgentConfigState::Missing => "config-missing",
        AgentChatAgentConfigState::Invalid => "config-invalid",
    }
}

fn setup_agent_capability_label(
    entry: &AgentChatAgentCatalogEntry,
    requirements: AgentChatLaunchRequirements,
) -> Option<&'static str> {
    if !requirements.needs_embedded_context && !requirements.needs_image {
        return None;
    }
    if entry.satisfies_requirements(requirements) {
        Some("compatible")
    } else if requirements.needs_image {
        Some("image-mismatch")
    } else {
        Some("context-mismatch")
    }
}

fn format_setup_agent_picker_status(
    entry: &AgentChatAgentCatalogEntry,
    requirements: AgentChatLaunchRequirements,
) -> String {
    let mut parts = vec![
        format!("{:?}", entry.source),
        setup_agent_install_label(entry.install_state).to_string(),
        setup_agent_auth_label(entry.auth_state).to_string(),
        setup_agent_config_label(entry.config_state).to_string(),
    ];
    if let Some(capability) = setup_agent_capability_label(entry, requirements) {
        parts.push(capability.to_string());
    }
    if entry.last_session_ok {
        parts.push("last-ok".to_string());
    }
    parts.join(" \u{00b7} ")
}
