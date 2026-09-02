//! Host-neutral Agent Chat thread bootstrap.
//!
//! Extracts the warm Pi connection/thread creation logic out of the
//! detached-only `open_or_focus_chat_with_input` so a host surface can spawn
//! a live Agent Chat thread without knowing the window ownership details.
//! (The former `spawn_hosted_view` factory was removed with the Notes-hosted
//! Agent Chat surface — the main window and detached chat own their views.)

use gpui::{App, AppContext as _, Entity};

use super::thread::{AgentChatThread, AgentChatThreadInit};
use super::view::AgentChatView;
use super::{AgentChatLaunchRequirements, AgentChatPermissionBroker};

/// Spawn a new `AgentChatThread` entity with the standard catalog/preflight/connection
/// bootstrap.  The returned entity is ready for embedding in any host surface.
pub(crate) fn spawn_hosted_thread(
    initial_input: Option<String>,
    requirements: AgentChatLaunchRequirements,
    cx: &mut App,
) -> Result<Entity<AgentChatThread>, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)
        .map_err(|error| error.to_string())?;
    let profile_ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
    let ai_preferences = crate::config::load_user_preferences().ai;
    let pi_launch =
        crate::ai::agent_chat::launch::resolve_selected_pi_launch(&ai_preferences, &profile_ctx)
            .map_err(|error| error.to_string())?;
    let manager = crate::ai::agent_chat::launch::warm_session_manager();
    // Acquire-or-cold-spawn instead of prepare+acquire: when the warm slot for
    // this key is already leased to a live thread (e.g. starting a second
    // thread in the same profile/cwd), this path spawns a fresh connection
    // rather than failing on the Acquired slot.
    let (lease, _origin) = manager
        .acquire_ready_or_spawn_cold(pi_launch.warm_spec())
        .map_err(|error| format!("Failed to start Pi Agent Chat session: {error}"))?;

    let (_broker, permission_rx) = AgentChatPermissionBroker::new();

    let thread = cx.new(|cx| {
        AgentChatThread::new(
            lease.connection.clone(),
            permission_rx,
            AgentChatThreadInit {
                ui_thread_id: lease.ui_thread_id.clone(),
                cwd: lease.cwd.clone(),
                initial_input,
                initial_context_parts: Vec::new(),
                display_name: pi_launch.profile.name.clone().into(),
                profile_id: pi_launch.profile.id.clone(),
                profile_display_name: Some(pi_launch.profile.name.clone().into()),
                profile_icon_name: pi_launch.profile.icon_name.clone(),
                selected_agent: None,
                available_agents: Vec::new(),
                launch_requirements: requirements,
                available_models: pi_launch.available_models.clone(),
                selected_model_id: pi_launch.selected_model_id.clone(),
                session_policy: super::capabilities::AgentChatSessionPolicy::Full,
            },
            cx,
        )
    });

    thread.update(cx, |thread: &mut AgentChatThread, cx| {
        thread.mark_context_bootstrap_ready(cx);
    });

    Ok(thread)
}
