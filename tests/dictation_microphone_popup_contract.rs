//! Source-level contract for the dictation overlay microphone picker.
//!
//! The overlay mic button must use an attached PromptPopup-style window, expose
//! safe automation rows, and persist through the shared dictation device path.

const POPUP: &str = include_str!("../src/dictation/microphone_popup_window.rs");
const DEVICE: &str = include_str!("../src/dictation/device.rs");
const COLLECTOR: &str = include_str!("../src/windows/automation_surface_collector.rs");
const PROMPT_HANDLER: &str = include_str!("../src/prompt_handler/mod.rs");

#[test]
fn popup_uses_shared_attached_window_primitives_and_prompt_popup_registration() {
    assert!(
        POPUP.contains("configure_inline_popup_window")
            && POPUP.contains("inline_popup_window_options")
            && POPUP.contains("set_inline_popup_window_bounds")
            && POPUP.contains("dictation_microphone_popup_bounds_above")
            && POPUP.contains("INLINE_POPUP_EDGE_GUTTER")
            && POPUP.contains("parent_bounds.origin.y.as_f32() - height")
            && POPUP.contains("register_attached_popup")
            && POPUP.contains("AutomationWindowKind::PromptPopup")
            && POPUP.contains("dictationMicrophonePopup"),
        "dictation mic selector must reuse attached popup window primitives and appear above the overlay"
    );
}

#[test]
fn popup_selection_persists_through_shared_device_helper() {
    assert!(
        POPUP.contains("apply_device_selection(&row.action)")
            && DEVICE.contains("pub fn apply_device_selection")
            && DEVICE.contains("save_dictation_device_id(None)")
            && DEVICE.contains("save_dictation_device_id(Some(device_id.0.as_str()))")
            && DEVICE.contains("notify_dictation_device_preference_changed()"),
        "dictation popup selections must update the same persisted microphone preference as settings"
    );
}

#[test]
fn popup_automation_rows_are_safe_and_batch_selectable() {
    assert!(
        POPUP.contains("DICTATION_MICROPHONE_POPUP_AUTOMATION_ID")
            && POPUP.contains("dictation-mic-row-{idx}")
            && POPUP.contains("choice:{idx}:{row_id}")
            && COLLECTOR.contains("\"panel:dictation-microphone-popup\"")
            && COLLECTOR.contains("\"list:dictation-microphones\"")
            && COLLECTOR.contains("Some(row.row_id.clone())")
            && PROMPT_HANDLER.contains("batch_select_dictation_microphone_popup_row_by_value")
            && PROMPT_HANDLER
                .contains("batch_select_dictation_microphone_popup_row_by_semantic_id"),
        "dictation popup must expose safe row ids and route PromptPopup batch selection"
    );
}
