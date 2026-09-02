//! Agent Chat OS notification decision logic and dispatch shell.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatNotificationVisibility {
    Unknown,
    VisibleAndKey,
    HiddenOrNotKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatNotificationEvent {
    TurnFinished,
    Failed,
    WaitingForPermission { request_id: u64 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AgentChatNotificationDebounce {
    terminal_notified_turn: Option<u64>,
    permission_notified_requests: std::collections::HashSet<u64>,
}

pub(crate) fn should_notify_agent_chat_event(
    event: AgentChatNotificationEvent,
    visibility: AgentChatNotificationVisibility,
    config_enabled: bool,
    turn_id: u64,
    debounce: &mut AgentChatNotificationDebounce,
) -> bool {
    if !config_enabled || visibility != AgentChatNotificationVisibility::HiddenOrNotKey {
        return false;
    }

    match event {
        AgentChatNotificationEvent::TurnFinished | AgentChatNotificationEvent::Failed => {
            if debounce.terminal_notified_turn == Some(turn_id) {
                return false;
            }
            debounce.terminal_notified_turn = Some(turn_id);
            true
        }
        AgentChatNotificationEvent::WaitingForPermission { request_id } => {
            debounce.permission_notified_requests.insert(request_id)
        }
    }
}

pub(crate) fn truncate_notification_body(text: &str) -> String {
    const LIMIT: usize = 80;
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let body: String = chars.by_ref().take(LIMIT).collect();
    if chars.next().is_some() {
        format!("{body}…")
    } else {
        body
    }
}

pub(crate) fn dispatch_agent_chat_notification(title: &'static str, body: String) {
    if crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Notification).is_err() {
        return;
    }
    tracing::info!(
        target: "script_kit::agent_chat",
        event = "agent_chat_os_notification_dispatch",
        backend = "notify-rust",
        title,
        body_len = body.chars().count(),
    );

    let title = title.to_string();
    let _ = std::thread::Builder::new()
        .name("agent-chat-notify-rust-dispatch".to_string())
        .spawn(move || {
            if let Err(error) = notify_rust::Notification::new()
                .summary(&title)
                .body(&body)
                .show()
            {
                tracing::warn!(
                    target: "script_kit::agent_chat",
                    event = "agent_chat_os_notification_failed",
                    backend = "notify-rust",
                    error = %error,
                );
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_finished_notifies() {
        let mut debounce = AgentChatNotificationDebounce::default();
        assert!(should_notify_agent_chat_event(
            AgentChatNotificationEvent::TurnFinished,
            AgentChatNotificationVisibility::HiddenOrNotKey,
            true,
            1,
            &mut debounce,
        ));
    }

    #[test]
    fn visible_suppresses_notification() {
        let mut debounce = AgentChatNotificationDebounce::default();
        assert!(!should_notify_agent_chat_event(
            AgentChatNotificationEvent::TurnFinished,
            AgentChatNotificationVisibility::VisibleAndKey,
            true,
            1,
            &mut debounce,
        ));
    }

    #[test]
    fn config_off_suppresses_notification() {
        let mut debounce = AgentChatNotificationDebounce::default();
        assert!(!should_notify_agent_chat_event(
            AgentChatNotificationEvent::Failed,
            AgentChatNotificationVisibility::HiddenOrNotKey,
            false,
            1,
            &mut debounce,
        ));
    }

    #[test]
    fn double_terminal_notification_is_suppressed_for_turn() {
        let mut debounce = AgentChatNotificationDebounce::default();
        assert!(should_notify_agent_chat_event(
            AgentChatNotificationEvent::Failed,
            AgentChatNotificationVisibility::HiddenOrNotKey,
            true,
            1,
            &mut debounce,
        ));
        assert!(!should_notify_agent_chat_event(
            AgentChatNotificationEvent::TurnFinished,
            AgentChatNotificationVisibility::HiddenOrNotKey,
            true,
            1,
            &mut debounce,
        ));
    }
}
