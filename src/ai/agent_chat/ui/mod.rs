//! Agent Chat UI implementation.
//!
//! This module is the live implementation of the Agent Chat chat surface. The
//! The active backend is Pi (`crate::ai::agent_chat`). Feature-level code should
//! import Agent Chat UI types from this module.
//!
//! # Module Layout
//!
//! - `catalog` — Schema-versioned agent catalog file and readiness types.
//! - `config` — `AgentChatAgentConfig` for agent discovery and command configuration.
//! - `preflight` — Launch readiness resolution (blockers before thread spawn).
//! - `events` — Typed Agent Chat turn/event primitives (`AgentChatEvent`).
//! - `permission_broker` — Full-option-set permission forwarding to the UI.
//! - `types` — Bridging types between Agent Chat and Script Kit internals.

use gpui::AppContext as _;

pub(crate) mod catalog;
pub(crate) mod chat_window;
pub(crate) mod components;
pub(crate) mod composer_state;
pub(crate) mod config;
pub(crate) mod context;
pub(crate) mod conversation_export;
pub(crate) mod events;
pub(crate) mod export;
pub(crate) mod favorite_models;
pub(crate) mod history;
pub(crate) mod history_attachment;
pub(crate) mod history_popup;
pub(crate) mod hosted;
pub(crate) mod labels;
pub(crate) mod layout;
pub(crate) mod notifications;
pub(crate) mod permission_broker;
pub(crate) mod popup_registry;
pub(crate) mod popup_window;
pub(crate) mod portal_contract;
pub(crate) mod preflight;
pub(crate) mod setup_state;
pub(crate) mod streaming_buffer;
pub(crate) mod style_contract;
pub(crate) mod surface_state;
pub(crate) mod thread;
pub(crate) mod tool_card;
pub(crate) mod types;
pub(crate) mod ui_variant;
pub(crate) mod view;

#[cfg(test)]
mod tests;

pub(crate) use catalog::{
    default_agent_chat_agents_path, AgentChatAgentAuthState, AgentChatAgentCatalogEntry,
    AgentChatAgentCatalogFile, AgentChatAgentConfigState, AgentChatAgentInstallState,
    AgentChatAgentSource,
};
pub(crate) use config::{
    agent_chat_cwd_recents_for_profile, claude_code_agent_config_cached,
    ensure_agent_chat_agents_catalog_seeded, load_agent_chat_agent_catalog_entries,
    load_agent_chat_agent_configs, load_agent_chat_agent_runtime_states,
    open_agent_chat_agents_catalog_in_editor, persist_agent_chat_agent_runtime_state,
    prewarm_agent_config, record_agent_chat_cwd_recent,
    refresh_agent_chat_agent_catalog_entries_with_snapshot, AgentChatAgentConfig,
    AgentChatAgentRuntimeState, AgentChatAgentRuntimeStateFile,
};
#[allow(deprecated)]
pub(crate) use context::{
    build_tab_ai_agent_chat_context_blocks, build_tab_ai_agent_chat_guidance_blocks,
    build_tab_ai_agent_chat_guidance_blocks_for_prompt,
};
pub(crate) use events::{AgentChatEvent, AgentChatEventRx, AgentChatForkPoint};
pub(crate) use permission_broker::{
    approval_request_input, AgentChatApprovalOption, AgentChatApprovalPreview,
    AgentChatApprovalPreviewKind, AgentChatApprovalRequest, AgentChatApprovalRequestInput,
    AgentChatPermissionBroker,
};
pub(crate) use preflight::{
    resolve_agent_chat_launch_with_requirements, resolve_default_agent_chat_launch,
    resolve_explicit_agent_chat_launch_with_requirements, setup_title_for_resolution,
    AgentChatLaunchBlocker, AgentChatLaunchRequirements, AgentChatLaunchResolution,
};
pub(crate) use setup_state::{AgentChatInlineSetupState, AgentChatSetupAction};
pub(crate) use thread::{
    AgentChatContextBootstrapState, AgentChatThread, AgentChatThreadInit, AgentChatThreadMessage,
    AgentChatThreadStatus, AgentChatToolCallState,
};
pub(crate) use view::{
    build_skill_context_part, build_skill_slash_command_text, build_staged_skill_prompt,
    AgentChatHistoryResumeRequest, AgentChatRetryRequest, AgentChatSession, AgentChatThreadSummary,
    AgentChatView,
};

pub(crate) fn open_or_focus_chat_with_input(
    input: String,
    cx: &mut gpui::App,
) -> Result<(), String> {
    if let Some(entity) = chat_window::get_detached_agent_chat_view_entity() {
        entity
            .update(cx, |chat, cx| {
                if chat.is_setup_mode() {
                    return Err("Agent Chat is in setup mode".to_string());
                }
                chat.set_input(input, cx);
                Ok::<(), String>(())
            })
            .map_err(|error| error.to_string())?;
        chat_window::activate_chat_window(cx);
        return Ok(());
    }

    if chat_window::is_chat_window_open() {
        chat_window::close_chat_window(cx);
    }

    let profile_ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
    let ai_preferences = crate::config::load_user_preferences().ai;
    let pi_launch =
        crate::ai::agent_chat::launch::resolve_selected_pi_launch(&ai_preferences, &profile_ctx)
            .map_err(|error| error.to_string())?;
    let warm_spec = pi_launch.warm_spec();
    let manager = crate::ai::agent_chat::launch::warm_session_manager();
    // Acquire without blocking the UI thread: a Ready slot is reused, an
    // in-flight Preparing slot is joined, and only a true miss cold-spawns —
    // the runtime boot then happens on the pi worker, not here. The old
    // prepare_warm call blocked this thread for the full warm-up on a miss.
    let (lease, _origin) = manager
        .acquire_ready_or_spawn_cold(warm_spec)
        .map_err(|error| format!("Failed to start Pi Agent Chat session: {error}"))?;

    let (_broker, permission_rx) = AgentChatPermissionBroker::new();
    let thread = cx.new(|cx| {
        AgentChatThread::new(
            lease.connection.clone(),
            permission_rx,
            AgentChatThreadInit {
                ui_thread_id: lease.ui_thread_id.clone(),
                cwd: lease.cwd.clone(),
                initial_input: Some(input),
                initial_context_parts: Vec::new(),
                display_name: pi_launch.profile.name.clone().into(),
                profile_id: pi_launch.profile.id.clone(),
                profile_display_name: Some(pi_launch.profile.name.clone().into()),
                profile_icon_name: pi_launch.profile.icon_name.clone(),
                selected_agent: None,
                available_agents: Vec::new(),
                launch_requirements: AgentChatLaunchRequirements::default(),
                available_models: pi_launch.available_models.clone(),
                selected_model_id: pi_launch.selected_model_id.clone(),
            },
            cx,
        )
    });

    thread.update(cx, |thread, cx| {
        thread.mark_context_bootstrap_ready(cx);
    });

    chat_window::open_chat_window_with_thread(thread, None, cx)
        .map_err(|error| error.to_string())?;
    Ok(())
}
