const AGENT_CHAT_MOD_SOURCE: &str = include_str!("../src/ai/agent_chat/mod.rs");
const AGENT_CHAT_EVENTS_SOURCE: &str = include_str!("../src/ai/agent_chat/events.rs");
const AGENT_CHAT_RUNTIME_SOURCE: &str = include_str!("../src/ai/agent_chat/runtime.rs");
const AGENT_CHAT_LAUNCH_SOURCE: &str = include_str!("../src/ai/agent_chat/launch.rs");
const AGENT_CHAT_THREAD_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/thread.rs");
const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/view.rs");
const APP_STATE_SOURCE: &str = include_str!("../src/main_sections/app_state.rs");
const TAB_AI_MODE_SOURCE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");
const TAB_AI_AGENT_CHAT_LAUNCH_SOURCE: &str =
    include_str!("../src/app_impl/agent_handoff/agent_chat_launch.rs");

#[test]
fn agent_chat_runtime_modules_are_declared() {
    assert!(AGENT_CHAT_MOD_SOURCE.contains("pub mod events;"));
    assert!(AGENT_CHAT_MOD_SOURCE.contains("pub(crate) mod launch;"));
    assert!(AGENT_CHAT_MOD_SOURCE.contains("pub mod runtime;"));
    assert!(AGENT_CHAT_MOD_SOURCE.contains("pub mod metrics;"));
}

#[test]
fn agent_chat_connection_trait_is_backend_neutral_and_object_safe_by_shape() {
    assert!(AGENT_CHAT_RUNTIME_SOURCE.contains("pub(crate) trait AgentChatConnection"));
    assert!(AGENT_CHAT_RUNTIME_SOURCE.contains("Send + Sync + 'static"));
    assert!(AGENT_CHAT_RUNTIME_SOURCE.contains("fn start_turn(&self"));
    assert!(AGENT_CHAT_RUNTIME_SOURCE.contains("fn cancel_turn(&self"));
    assert!(AGENT_CHAT_RUNTIME_SOURCE.contains("fn prepare_session("));
    assert!(
        !AGENT_CHAT_RUNTIME_SOURCE.contains("async fn"),
        "runtime seam must remain object-safe"
    );
    assert!(
        !AGENT_CHAT_RUNTIME_SOURCE.contains("PiLaunchSpec"),
        "Phase 2 seam must not know about Pi launch specs"
    );
}

#[test]
fn phase_two_event_boundary_aliases_current_agent_chat_stream() {
    assert!(AGENT_CHAT_EVENTS_SOURCE
        .contains("type AgentChatEvent = crate::ai::agent_chat::ui::AgentChatEvent"));
    assert!(AGENT_CHAT_EVENTS_SOURCE
        .contains("type AgentChatEventRx = crate::ai::agent_chat::ui::AgentChatEventRx"));
    assert!(
        !AGENT_CHAT_EVENTS_SOURCE.contains("enum AgentChatEvent"),
        "Phase 2 must not fork a second event enum"
    );
}

#[test]
fn agent_chat_thread_depends_on_trait_object_not_concrete_agent_chat_runtime() {
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("Arc<dyn AgentChatConnection>"),
        "AgentChatThread should store the neutral runtime seam"
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("connection: Arc<dyn AgentChatConnection>"),
        "AgentChatThread must store the neutral Agent Chat runtime seam"
    );
}

#[test]
fn phase_two_keeps_view_out_of_runtime_refactor() {
    assert!(
        !AGENT_CHAT_VIEW_SOURCE.contains("AgentChatConnection"),
        "AgentChatView should keep owning UI state only; runtime seam belongs in AgentChatThread"
    );
    assert!(
        !AGENT_CHAT_VIEW_SOURCE.contains("PiLaunchSpec"),
        "Phase 2 must not route the view toward Pi"
    );
}

#[test]
fn pi_routing_is_owned_by_agent_chat_launch_and_tab_entry_only() {
    assert!(AGENT_CHAT_LAUNCH_SOURCE.contains("PiAgentChatLaunch"));
    assert!(AGENT_CHAT_LAUNCH_SOURCE.contains("PiAgentChatLaunch::from_profile"));
    assert!(AGENT_CHAT_LAUNCH_SOURCE.contains("PiRpcRuntime::spawn"));
    assert!(AGENT_CHAT_LAUNCH_SOURCE.contains("AgentChatWarmSessionSpec"));
    assert!(
        TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("open_tab_ai_pi_view_from_launch"),
        "Tab launch should route selected Pi profiles through an explicit helper"
    );
    assert!(
        AGENT_CHAT_LAUNCH_SOURCE.contains("resolve_effective_profile")
            && AGENT_CHAT_LAUNCH_SOURCE.contains("PiAgentChatLaunch::from_profile"),
        "Tab launch must branch on the effective Agent Chat profile"
    );

    for (name, source) in [
        ("agent_chat_runtime", AGENT_CHAT_RUNTIME_SOURCE),
        ("agent_chat_thread", AGENT_CHAT_THREAD_SOURCE),
    ] {
        assert!(
            !source.contains("PiLaunchSpec"),
            "{} must stay backend-neutral and not import PiLaunchSpec",
            name
        );
        assert!(
            !source.contains("agent_chat::pi"),
            "{} must stay backend-neutral and not import Pi runtime modules",
            name
        );
    }
}

#[test]
fn tab_launch_uses_pi_warm_path_without_agent_chat_runtime_fallback() {
    assert!(
        !AGENT_CHAT_LAUNCH_SOURCE.contains("spawn_with_approval"),
        "Agent Chat launch must not instantiate the legacy Agent Chat runtime"
    );
    assert!(
        AGENT_CHAT_LAUNCH_SOURCE.contains("PiRpcRuntime")
            || AGENT_CHAT_LAUNCH_SOURCE.contains("PiRpcRuntime"),
        "Pi profiles should be routed to the Pi RPC runtime"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("dismiss_active_agent_chat_warm_lease"),
        "Agent Chat close must reset the acquired Pi warm lease"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("closing_pi_agent_chat")
            && TAB_AI_MODE_SOURCE.contains("self.embedded_agent_chat = None;"),
        "Normal Pi Agent Chat dismissal must not leave the old embedded entity reusable"
    );
    assert!(
        APP_STATE_SOURCE.contains("active_agent_chat_warm_lease"),
        "App state must retain the acquired warm lease until chat dismissal"
    );
}
