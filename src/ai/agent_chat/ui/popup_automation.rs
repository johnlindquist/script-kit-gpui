//! Agent Chat identities over the shared exact popup host registry.
use gpui::{AnyWindowHandle, App, Bounds, Pixels};

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
    _parent_bounds: Bounds<Pixels>,
) -> anyhow::Result<String> {
    crate::windows::list_automation_windows()
        .into_iter()
        .find(|window| {
            window.generation.is_some_and(|generation| {
                crate::windows::get_runtime_window_handle_for_generation(&window.id, generation)
                    == Some(parent_window_handle)
            })
        })
        .map(|window| window.id)
        .ok_or_else(|| anyhow::anyhow!("popup_parent_identity_missing"))
}

pub(crate) fn register_agent_chat_prompt_popup_automation_window(
    automation_id: &'static str,
    title: &'static str,
    handle: AnyWindowHandle,
    popup_bounds: Bounds<Pixels>,
    focus_return: &crate::components::inline_popup_window::InlinePopupFocusReturn,
    cx: &mut App,
) -> anyhow::Result<u64> {
    let parent = crate::windows::automation_window_by_id(&focus_return.parent_automation_id)
        .ok_or_else(|| anyhow::anyhow!("popup_parent_missing"))?;
    anyhow::ensure!(
        parent.generation == Some(focus_return.parent_generation),
        "stale_popup_parent"
    );
    let info = crate::windows::register_runtime_window_instance(
        crate::protocol::AutomationWindowInfo {
            id: automation_id.into(),
            kind: crate::protocol::AutomationWindowKind::PromptPopup,
            title: Some(title.into()),
            focused: false,
            visible: !focus_return.host_policy.is_hidden(),
            semantic_surface: Some("promptPopup".into()),
            bounds: Some(automation_bounds(popup_bounds)),
            parent_window_id: Some(parent.id),
            parent_kind: Some(parent.kind),
            parent_window_generation: Some(focus_return.parent_generation),
            generation: None,
            pid: Some(std::process::id()),
        },
        handle,
        cx,
    )?;
    info.generation
        .ok_or_else(|| anyhow::anyhow!("popup_generation_missing"))
}

pub(crate) fn unregister_agent_chat_prompt_popup_automation_window(
    automation_id: &'static str,
    generation: u64,
) {
    crate::windows::remove_runtime_window_instance(automation_id, generation);
}
