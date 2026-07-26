//! Window capability collection via public AX settable/action queries.
//!
//! Capabilities are OBSERVED, never assumed: `AXUIElementIsAttributeSettable`
//! for position/size/minimized/fullscreen, and the supported action list for
//! raise/close. CG-only rows are fully non-actionable with reason
//! `core_graphics_only`; inaccessible rows carry the observed AX error code.

use super::ax::{ax_action_names, ax_attribute_is_settable, get_ax_attribute};
use super::cf::cf_release;
use super::ffi::AXUIElementRef;
use super::types::WindowCapabilities;

/// Pure assembly of capabilities from observed facts (unit testable).
pub(super) fn capabilities_from_parts(
    position_settable: bool,
    size_settable: bool,
    minimized_settable: bool,
    fullscreen_settable: bool,
    actions: &[String],
    has_close_button: bool,
) -> WindowCapabilities {
    let can_raise = actions.iter().any(|action| action == "AXRaise");
    let can_close = has_close_button;
    let actionable = position_settable
        || size_settable
        || minimized_settable
        || fullscreen_settable
        || can_raise
        || can_close;
    WindowCapabilities {
        can_move: position_settable,
        can_resize: size_settable,
        can_minimize: minimized_settable,
        can_close,
        can_raise,
        can_set_fullscreen: fullscreen_settable,
        actionable,
        non_actionable_reason: if actionable {
            None
        } else {
            Some("no supported AX capabilities".to_string())
        },
    }
}

/// Collect live capabilities for one AX window element.
pub(super) fn collect_ax_capabilities(window: AXUIElementRef) -> WindowCapabilities {
    let position_settable = ax_attribute_is_settable(window, "AXPosition").unwrap_or(false);
    let size_settable = ax_attribute_is_settable(window, "AXSize").unwrap_or(false);
    let minimized_settable = ax_attribute_is_settable(window, "AXMinimized").unwrap_or(false);
    let fullscreen_settable = ax_attribute_is_settable(window, "AXFullScreen").unwrap_or(false);
    let actions = ax_action_names(window).unwrap_or_default();
    // can_close: AXCloseButton exists and supports AXPress.
    let has_close_button = match get_ax_attribute(window, "AXCloseButton") {
        Ok(button) => {
            let supports_press = ax_action_names(button as AXUIElementRef)
                .map(|names| names.iter().any(|name| name == "AXPress"))
                .unwrap_or(false);
            cf_release(button);
            supports_press
        }
        Err(_) => false,
    };
    capabilities_from_parts(
        position_settable,
        size_settable,
        minimized_settable,
        fullscreen_settable,
        &actions,
        has_close_button,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settable_flags_map_directly_onto_capabilities() {
        let capabilities = capabilities_from_parts(
            true,
            false,
            true,
            false,
            &["AXRaise".to_string(), "AXPress".to_string()],
            true,
        );
        assert!(capabilities.can_move);
        assert!(!capabilities.can_resize);
        assert!(capabilities.can_minimize);
        assert!(!capabilities.can_set_fullscreen);
        assert!(capabilities.can_raise);
        assert!(capabilities.can_close);
        assert!(capabilities.actionable);
        assert!(capabilities.non_actionable_reason.is_none());
    }

    #[test]
    fn nothing_settable_and_no_actions_is_non_actionable_with_reason() {
        let capabilities = capabilities_from_parts(false, false, false, false, &[], false);
        assert!(!capabilities.actionable);
        assert!(capabilities.non_actionable_reason.is_some());
    }

    #[test]
    fn raise_requires_the_ax_raise_action() {
        let capabilities =
            capabilities_from_parts(false, false, false, false, &["AXPress".to_string()], false);
        assert!(!capabilities.can_raise);
    }
}
