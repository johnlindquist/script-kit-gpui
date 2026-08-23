/// Build the second-level rewind checkpoint picker actions (latest first,
/// since the most recent message is the most likely edit target).
pub(crate) fn get_agent_chat_fork_picker_actions(
    fork_points: &[crate::ai::agent_chat::ui::AgentChatForkPoint],
) -> Vec<Action> {
    fork_points
        .iter()
        .rev()
        .map(|point| {
            Action::new(
                agent_chat_fork_edit_action_id(&point.entry_id),
                agent_chat_message_action_title(&point.text),
                Some("Rewind here and edit this message; later replies are discarded".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Pencil)
            .with_section("Messages")
        })
        .collect()
}

/// Build an `ActionsDialogRoute` for the rewind checkpoint picker.
pub(crate) fn get_agent_chat_fork_picker_route_for_host(
    fork_points: &[crate::ai::agent_chat::ui::AgentChatForkPoint],
    host: AgentChatActionsDialogHost,
) -> crate::actions::ActionsDialogRoute {
    crate::actions::ActionsDialogRoute {
        id: AGENT_CHAT_FORK_PICKER_ROUTE_ID.to_string(),
        actions: filter_agent_chat_actions_for_host(
            host,
            get_agent_chat_fork_picker_actions(fork_points),
        ),
        context_title: Some("Rewind & Edit".to_string()),
        search_placeholder: Some("Search messages...".to_string()),
        initial_selected_action_id: fork_points
            .last()
            .map(|point| agent_chat_fork_edit_action_id(&point.entry_id)),
    }
}

/// Build an `ActionsDialogRoute` for the Agent Chat profile picker sub-route.
pub(crate) fn get_agent_chat_profile_picker_route_for_host(
    host: AgentChatActionsDialogHost,
) -> crate::actions::ActionsDialogRoute {
    crate::actions::ActionsDialogRoute {
        id: AGENT_CHAT_PROFILE_PICKER_ROUTE_ID.to_string(),
        actions: filter_agent_chat_actions_for_host(host, get_agent_chat_profile_picker_actions()),
        context_title: Some("Profile picker".to_string()),
        search_placeholder: Some("Search profiles...".to_string()),
        initial_selected_action_id: Some(agent_chat_switch_profile_action_id(
            &selected_agent_chat_profile_picker_id(),
        )),
    }
}

/// Build an `ActionsDialogRoute` for the Agent Chat profile picker sub-route (shared host).
#[allow(dead_code)]
pub(crate) fn get_agent_chat_profile_picker_route() -> crate::actions::ActionsDialogRoute {
    get_agent_chat_profile_picker_route_for_host(AgentChatActionsDialogHost::Shared)
}

/// Build an `ActionsDialogRoute` for the Agent Chat root menu (shared host).
#[allow(dead_code)]
pub(crate) fn get_agent_chat_root_route(
    available_models: &[crate::ai::agent_chat::ui::config::AgentChatModelEntry],
    selected_model_id: Option<&str>,
) -> crate::actions::ActionsDialogRoute {
    get_agent_chat_root_route_for_host(
        available_models,
        selected_model_id,
        0,
        &[],
        &[],
        crate::components::conversation_actions::AgentChatConversationCommandFacts::default(),
        AgentChatActionsDialogHost::Shared,
    )
}

/// Build an `ActionsDialogRoute` for the Agent Chat model picker sub-route (shared host).
#[allow(dead_code)]
pub(crate) fn get_agent_chat_model_picker_route(
    available_models: &[crate::ai::agent_chat::ui::config::AgentChatModelEntry],
    selected_model_id: Option<&str>,
) -> crate::actions::ActionsDialogRoute {
    get_agent_chat_model_picker_route_for_host(
        available_models,
        selected_model_id,
        AgentChatActionsDialogHost::Shared,
    )
}
