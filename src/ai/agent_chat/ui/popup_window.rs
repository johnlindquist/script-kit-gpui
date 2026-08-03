//! Agent Chat popup-window facade.
//!
//! Window mechanics (bounds math, no-focus-steal config, child-window attach,
//! AppKit pointer plumbing) moved to the shared
//! [`crate::components::inline_popup_window`] module so Agent Chat and the
//! menu-syntax `:`, `;`, and `!` trigger pickers share a single implementation.
//!
//! This file remains as a thin compatibility facade: every `DENSE_PICKER_*`
//! / `dense_picker_*` / `popup_*` name historically exposed by this module is
//! re-exported from the shared implementation under the same Agent Chat-specific
//! alias, so existing Agent Chat call sites in `history_popup.rs`, `view.rs`,
//! and the source-text audit tests in `src/ai/agent_chat/ui/tests.rs` all
//! continue to compile without edits. Add the Agent Chat-flavored
//! `dense_picker_height(item_count)` convenience
//! on top so callers can keep passing a bare item count and get
//! `CONTEXT_SELECTOR_ROW_HEIGHT` applied automatically.

use crate::components::inline_dropdown::CONTEXT_SELECTOR_ROW_HEIGHT;
use gpui::{AnyWindowHandle, Bounds, Pixels};

// Re-export constants under Agent Chat-compatible names. Consumers continue to
// reference them via `super::popup_window::DENSE_PICKER_*` without knowing the
// implementation now lives under `components::inline_popup_window`.
pub(crate) use crate::components::inline_popup_window::{
    INLINE_POPUP_DEFAULT_WIDTH as DENSE_PICKER_DEFAULT_WIDTH,
    INLINE_POPUP_EDGE_GUTTER as DENSE_PICKER_EDGE_GUTTER,
    INLINE_POPUP_EMPTY_HEIGHT as DENSE_PICKER_EMPTY_HEIGHT,
    INLINE_POPUP_LEFT_MARGIN as DENSE_PICKER_LEFT_MARGIN,
    INLINE_POPUP_MAX_VISIBLE_ROWS as DENSE_PICKER_MAX_VISIBLE_ROWS,
    INLINE_POPUP_MIN_WIDTH as DENSE_PICKER_MIN_WIDTH,
    INLINE_POPUP_VERTICAL_PADDING as DENSE_PICKER_VERTICAL_PADDING,
};

// Re-export neutral helpers under Agent Chat-compatible names.
pub(crate) use crate::components::inline_popup_window::{
    configure_inline_popup_window as configure_popup_window,
    footer_anchored_inline_popup_top as footer_anchored_popup_top,
    inline_popup_bounds as popup_bounds,
    inline_popup_height_for_row_height as dense_picker_height_for_row_height,
    inline_popup_width_for_window as dense_picker_width_for_window,
    inline_popup_window_options as popup_window_options,
    set_inline_popup_window_bounds as set_popup_window_bounds,
};

#[cfg(target_os = "macos")]
#[allow(unused_imports)]
pub(crate) use crate::components::inline_popup_window::{
    attach_inline_popup_to_parent_window as attach_popup_to_parent_window,
    inline_popup_ns_window as popup_ns_window,
};

/// Agent Chat-flavored convenience: popup height in rows measured against the Agent Chat
/// context-selector row height. The neutral
/// [`crate::components::inline_popup_window::inline_popup_height_for_row_height`]
/// is what callers use when their row height differs (e.g. the history
/// popup's taller header rows).
pub(crate) fn dense_picker_height(item_count: usize) -> f32 {
    dense_picker_height_for_row_height(item_count, CONTEXT_SELECTOR_ROW_HEIGHT)
}

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

#[cfg(test)]
mod tests {
    use super::{dense_picker_height, DENSE_PICKER_EMPTY_HEIGHT};
    use crate::components::inline_popup_window::INLINE_POPUP_MAX_VISIBLE_ROWS;

    #[test]
    fn dense_picker_height_uses_shared_row_contract() {
        assert_eq!(dense_picker_height(0), DENSE_PICKER_EMPTY_HEIGHT);
        // Agent Chat convenience should cap at the shared max-visible-rows value.
        assert_eq!(
            dense_picker_height(INLINE_POPUP_MAX_VISIBLE_ROWS + 4),
            dense_picker_height(INLINE_POPUP_MAX_VISIBLE_ROWS),
        );
    }
}
