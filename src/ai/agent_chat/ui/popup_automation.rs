//! Agent Chat-specific automation identity for shared inline popup windows.
//!
//! Window mechanics live in `components::inline_popup_window`; this module owns
//! only the Agent Chat parent/child automation registration policy.

use gpui::{AnyWindowHandle, Bounds, Pixels};

pub(crate) fn automation_bounds(bounds: Bounds<Pixels>) -> crate::protocol::AutomationWindowBounds {
    crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    }
}

pub(crate) fn resolve_agent_chat_popup_parent_automation_id(
    parent_window_handle: AnyWindowHandle,
    parent_bounds: Bounds<Pixels>,
) -> anyhow::Result<String> {
    for window in crate::windows::list_automation_windows() {
        if crate::windows::get_runtime_window_handle(&window.id)
            .is_some_and(|handle| handle == parent_window_handle)
        {
            return Ok(window.id);
        }
    }

    if crate::get_main_window_handle().is_some_and(|handle| handle == parent_window_handle) {
        let parent_id = "main".to_string();
        crate::windows::upsert_runtime_window_handle(&parent_id, parent_window_handle);
        let preserved_semantic_surface = crate::windows::list_automation_windows()
            .into_iter()
            .find(|window| window.id == parent_id)
            .and_then(|window| window.semantic_surface)
            .unwrap_or_else(|| "agentChatChat".to_string());
        crate::windows::upsert_automation_window(crate::protocol::AutomationWindowInfo {
            id: parent_id.clone(),
            kind: crate::protocol::AutomationWindowKind::Main,
            title: Some("Script Kit".to_string()),
            focused: true,
            visible: true,
            semantic_surface: Some(preserved_semantic_surface),
            bounds: Some(automation_bounds(parent_bounds)),
            parent_window_id: None,
            parent_kind: None,
            pid: Some(std::process::id()),
            generation: None,
        });
        return Ok(parent_id);
    }

    anyhow::bail!(
        "Cannot register Agent Chat prompt popup: parent automation identity is required"
    );
}

pub(crate) fn register_agent_chat_prompt_popup_automation_window(
    automation_id: &'static str,
    title: &'static str,
    parent_window_handle: AnyWindowHandle,
    parent_bounds: Bounds<Pixels>,
    popup_bounds: Bounds<Pixels>,
    generation: u64,
) -> anyhow::Result<String> {
    let parent_id =
        resolve_agent_chat_popup_parent_automation_id(parent_window_handle, parent_bounds)?;
    crate::windows::register_attached_popup_instance(
        automation_id.to_string(),
        crate::protocol::AutomationWindowKind::PromptPopup,
        Some(title.to_string()),
        Some("promptPopup".to_string()),
        Some(automation_bounds(popup_bounds)),
        Some(parent_id.as_str()),
        Some(generation),
    )?;
    Ok(parent_id)
}

pub(crate) fn unregister_agent_chat_prompt_popup_automation_window(
    automation_id: &'static str,
    generation: u64,
) {
    crate::windows::remove_runtime_window_handle_if_generation(automation_id, generation);
    crate::windows::remove_automation_window_if_generation(automation_id, generation);
}
