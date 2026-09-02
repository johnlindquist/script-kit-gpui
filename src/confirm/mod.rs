//! Confirm Module
//!
//! Native popup confirmation dialogs rendered in a dedicated GPUI
//! `WindowKind::PopUp` window so macOS vibrancy blur comes from the NSPanel
//! itself instead of a translucent in-window overlay.
//!
//! # Example Usage (SDK)
//! ```typescript
//! const confirmed = await confirm("Are you sure you want to delete this?");
//! if (confirmed) {
//!     // proceed with deletion
//! }
//!
//! // With custom button text
//! const proceed = await confirm({
//!     message: "Overwrite existing file?",
//!     confirmText: "Overwrite",
//!     cancelText: "Keep Original"
//! });
//! ```

pub(crate) mod parent_dialog;
pub(crate) mod window;

// Used by include!() code in app_actions/handle_action/scripts.rs — clippy
// cannot trace usage through include!() and reports a false-positive dead_code
// warning for the non-lifecycle variant.
#[allow(unused_imports)]
pub(crate) use crate::components::confirm_modal_shell::PARENT_MODAL_WIDTH_PX;
#[allow(unused_imports)]
pub(crate) use parent_dialog::{
    confirm_with_parent_dialog, open_parent_action_dialog, open_parent_confirm_dialog,
    open_parent_confirm_dialog_for_automation_parent, open_parent_confirm_dialog_for_entity,
    open_parent_confirm_dialog_with_lifecycle, ParentActionDialogOptions, ParentConfirmOptions,
};
// Design-contract resolvers shared by the in-window confirm renderer and the
// token exporter (src/design_contract).
#[allow(unused_imports)]
pub(crate) use parent_dialog::{
    confirm_prompt_line_height_px, resolved_confirm_prompt_colors, resolved_confirm_prompt_metrics,
    ConfirmPromptMetrics, ResolvedConfirmPromptColors, CONFIRM_PROMPT_BODY_FONT_SIZE_PX,
    CONFIRM_PROMPT_BODY_MAX_WIDTH_PX, CONFIRM_PROMPT_TITLE_FONT_SIZE_PX,
};

#[allow(unused_imports)]
pub(crate) use window::close_parent_action_dialog_programmatically;
#[allow(unused_imports)]
pub(crate) use window::consume_main_window_key_while_confirm_open;
#[allow(unused_imports)]
pub(crate) use window::is_confirm_window_open;
#[allow(unused_imports)]
pub(crate) use window::is_confirm_window_open as is_confirm_popup_window_open;
#[allow(unused_imports)]
pub(crate) use window::route_key_to_confirm_popup;
#[allow(unused_imports)]
pub(crate) use window::send_confirm_result;
#[allow(unused_imports)]
pub(crate) use window::{
    batch_select_confirm_button_by_semantic_id, batch_select_confirm_button_by_value,
};
#[allow(unused_imports)]
pub(crate) use window::{close_confirm_window, close_owned_confirm_window};
#[allow(unused_imports)]
pub(crate) use window::{get_confirm_popup_snapshot, ConfirmPopupSnapshot};
