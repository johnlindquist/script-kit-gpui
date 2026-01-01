//! Dynamic Window Resizing Module
//!
//! Handles window height for different view types in Script Kit GPUI.
//!
//! **Key Rules:**
//! - ScriptList (main window with preview): FIXED at 500px, never resizes
//! - ArgPrompt with choices: Dynamic height based on choice count (capped at 500px)
//! - ArgPrompt without choices (input only): Compact input-only height
//! - Editor/Div/Term: Full height 700px

#[cfg(target_os = "macos")]
use cocoa::foundation::{NSPoint, NSRect, NSSize};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

use gpui::{px, Context, Pixels, Render, Timer};
use std::time::Duration;
use tracing::{debug, warn};

use crate::list_item::LIST_ITEM_HEIGHT;
use crate::logging;
use crate::window_manager;

/// Layout constants for height calculations
pub mod layout {
    use crate::panel::{CURSOR_HEIGHT_LG, CURSOR_MARGIN_Y};
    use gpui::{px, Pixels};

    /// Input row vertical padding (matches default design spacing padding_md)
    pub const ARG_INPUT_PADDING_Y: f32 = 12.0;
    /// List container vertical padding (top + bottom, matches default padding_xs)
    pub const ARG_LIST_PADDING_Y: f32 = 8.0;
    /// Divider thickness (matches default design border_thin)
    pub const ARG_DIVIDER_HEIGHT: f32 = 1.0;
    /// Input row text height (cursor height + margins)
    pub const ARG_INPUT_LINE_HEIGHT: f32 = CURSOR_HEIGHT_LG + (CURSOR_MARGIN_Y * 2.0);
    /// Total input-only height (header only, no list)
    pub const ARG_HEADER_HEIGHT: f32 = (ARG_INPUT_PADDING_Y * 2.0) + ARG_INPUT_LINE_HEIGHT;

    /// Minimum window height (input only) - for input-only prompts
    pub const MIN_HEIGHT: Pixels = px(ARG_HEADER_HEIGHT);

    /// Standard height for views with preview panel (script list, arg with choices)
    /// This is FIXED - these views do NOT resize dynamically
    pub const STANDARD_HEIGHT: Pixels = px(500.0);

    /// Maximum window height for full-content views (editor, div, term)
    pub const MAX_HEIGHT: Pixels = px(700.0);
}

/// View types for height calculation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewType {
    /// Script list view (main launcher) - has preview panel, FIXED height
    ScriptList,
    /// Arg prompt with choices - dynamic height based on item count
    ArgPromptWithChoices,
    /// Arg prompt without choices (input only) - compact height
    ArgPromptNoChoices,
    /// Div prompt (HTML display) - full height
    DivPrompt,
    /// Editor prompt (code editor) - full height
    EditorPrompt,
    /// Terminal prompt - full height
    TermPrompt,
}

/// Get the target height for a specific view type
///
/// # Arguments
/// * `view_type` - The type of view being displayed
/// * `item_count` - Number of items in the current view (used for dynamic sizing)
///
/// # Returns
/// The window height for this view type
pub fn height_for_view(view_type: ViewType, item_count: usize) -> Pixels {
    use layout::*;

    let clamp_height = |height: Pixels| -> Pixels {
        let height_f = f32::from(height);
        let min_f = f32::from(MIN_HEIGHT);
        let max_f = f32::from(STANDARD_HEIGHT);
        px(height_f.clamp(min_f, max_f))
    };

    match view_type {
        // Views with preview panel - FIXED height, no dynamic resizing
        // DivPrompt also uses standard height to match main window
        ViewType::ScriptList | ViewType::DivPrompt => STANDARD_HEIGHT,
        ViewType::ArgPromptWithChoices => {
            let visible_items = item_count.max(1) as f32;
            let list_height =
                (visible_items * LIST_ITEM_HEIGHT) + ARG_LIST_PADDING_Y + ARG_DIVIDER_HEIGHT;
            let total_height = ARG_HEADER_HEIGHT + list_height;
            clamp_height(px(total_height))
        }
        // Input-only prompt - compact
        ViewType::ArgPromptNoChoices => MIN_HEIGHT,
        // Full content views (editor, terminal) - max height
        ViewType::EditorPrompt | ViewType::TermPrompt => MAX_HEIGHT,
    }
}

/// Calculate the initial window height for app startup
pub fn initial_window_height() -> Pixels {
    layout::STANDARD_HEIGHT
}

/// Defer a window resize to the next frame to avoid RefCell borrow conflicts.
///
/// This is the **preferred way** to trigger resizes from prompt message handlers.
/// Direct calls to `resize_first_window_to_height` during GPUI's render cycle can cause
/// "RefCell already borrowed" errors because the native macOS `setFrame:display:animate:`
/// call happens synchronously within GPUI's update cycle.
///
/// # Arguments
/// * `view_type` - The type of view to resize for
/// * `item_count` - Item count (used for some view types)
/// * `cx` - The GPUI context (must implement `Render`)
///
/// # Example
/// ```ignore
/// // In a prompt message handler:
/// defer_resize_to_view(ViewType::EditorPrompt, 0, cx);
/// cx.notify();
/// ```
pub fn defer_resize_to_view<T: Render>(
    view_type: ViewType,
    item_count: usize,
    cx: &mut Context<T>,
) {
    let target_height = height_for_view(view_type, item_count);

    cx.spawn(async move |_this, _cx: &mut gpui::AsyncApp| {
        // 16ms delay (~1 frame at 60fps) ensures GPUI render cycle completes
        Timer::after(Duration::from_millis(16)).await;

        // Validate window still exists before resizing
        if window_manager::get_main_window().is_some() {
            resize_first_window_to_height(target_height);
        } else {
            warn!("defer_resize_to_view: window no longer exists, skipping resize");
            logging::log(
                "RESIZE",
                "WARNING: Window gone before deferred resize could execute",
            );
        }
    })
    .detach();
}

/// Force reset the debounce timer (kept for API compatibility)
pub fn reset_resize_debounce() {
    // No-op - we removed debouncing since resizes are now rare
}

/// Resize the main window to a new height, keeping the top edge fixed.
///
/// # Arguments
/// * `target_height` - The desired window height in pixels
///
/// # Platform
/// This function only works on macOS. On other platforms, it's a no-op.
#[cfg(target_os = "macos")]
pub fn resize_first_window_to_height(target_height: Pixels) {
    let height_f64: f64 = f32::from(target_height) as f64;

    // Get the main window from WindowManager
    let window = match window_manager::get_main_window() {
        Some(w) => w,
        None => {
            warn!("Main window not registered in WindowManager, cannot resize");
            logging::log(
                "RESIZE",
                "WARNING: Main window not registered in WindowManager.",
            );
            return;
        }
    };

    unsafe {
        // Get current window frame
        let current_frame: NSRect = msg_send![window, frame];

        // Skip if height is already correct (within 1px tolerance)
        let current_height = current_frame.size.height;
        if (current_height - height_f64).abs() < 1.0 {
            return;
        }

        // Log actual resizes at debug level (these are rare events, not hot-path)
        debug!(
            from_height = current_height,
            to_height = height_f64,
            "Resizing window"
        );
        logging::log(
            "RESIZE",
            &format!("Resize: {:.0} -> {:.0}", current_height, height_f64),
        );

        // Calculate height difference
        let height_delta = height_f64 - current_height;

        // macOS coordinate system: Y=0 at bottom, increases upward
        // To keep the TOP of the window fixed, adjust origin.y
        let new_origin_y = current_frame.origin.y - height_delta;

        let new_frame = NSRect::new(
            NSPoint::new(current_frame.origin.x, new_origin_y),
            NSSize::new(current_frame.size.width, height_f64),
        );

        // Apply the new frame
        let _: () = msg_send![window, setFrame:new_frame display:true animate:false];
    }
}

/// Get the current height of the main window
#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub fn get_first_window_height() -> Option<Pixels> {
    let window = window_manager::get_main_window()?;

    unsafe {
        let frame: NSRect = msg_send![window, frame];
        Some(px(frame.size.height as f32))
    }
}

/// Non-macOS stub for resize function
#[cfg(not(target_os = "macos"))]
pub fn resize_first_window_to_height(_target_height: Pixels) {
    logging::log("RESIZE", "Window resize is only supported on macOS");
}

/// Non-macOS stub for get_first_window_height
#[allow(dead_code)]
#[cfg(not(target_os = "macos"))]
pub fn get_first_window_height() -> Option<Pixels> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn test_script_list_fixed_height() {
        // Script list should always be STANDARD_HEIGHT regardless of item count
        assert_eq!(
            height_for_view(ViewType::ScriptList, 0),
            layout::STANDARD_HEIGHT
        );
        assert_eq!(
            height_for_view(ViewType::ScriptList, 5),
            layout::STANDARD_HEIGHT
        );
        assert_eq!(
            height_for_view(ViewType::ScriptList, 100),
            layout::STANDARD_HEIGHT
        );
    }

    #[test]
    fn test_arg_with_choices_dynamic_height() {
        // Arg with choices should size to items, clamped to STANDARD_HEIGHT
        let base_height =
            layout::ARG_HEADER_HEIGHT + layout::ARG_DIVIDER_HEIGHT + layout::ARG_LIST_PADDING_Y;
        assert_eq!(
            height_for_view(ViewType::ArgPromptWithChoices, 1),
            px(base_height + LIST_ITEM_HEIGHT)
        );
        assert_eq!(
            height_for_view(ViewType::ArgPromptWithChoices, 2),
            px(base_height + (2.0 * LIST_ITEM_HEIGHT))
        );
        assert_eq!(
            height_for_view(ViewType::ArgPromptWithChoices, 100),
            layout::STANDARD_HEIGHT
        );
    }

    #[test]
    fn test_arg_no_choices_compact() {
        // Arg without choices should be MIN_HEIGHT
        assert_eq!(
            height_for_view(ViewType::ArgPromptNoChoices, 0),
            layout::MIN_HEIGHT
        );
    }

    #[test]
    fn test_full_height_views() {
        // Editor and Terminal use MAX_HEIGHT (700px)
        assert_eq!(
            height_for_view(ViewType::EditorPrompt, 0),
            layout::MAX_HEIGHT
        );
        assert_eq!(height_for_view(ViewType::TermPrompt, 0), layout::MAX_HEIGHT);
    }

    #[test]
    fn test_div_prompt_standard_height() {
        // DivPrompt uses STANDARD_HEIGHT (500px) to match main window
        assert_eq!(
            height_for_view(ViewType::DivPrompt, 0),
            layout::STANDARD_HEIGHT
        );
    }

    #[test]
    fn test_initial_window_height() {
        assert_eq!(initial_window_height(), layout::STANDARD_HEIGHT);
    }

    #[test]
    fn test_height_constants() {
        assert_eq!(layout::MIN_HEIGHT, px(layout::ARG_HEADER_HEIGHT));
        assert_eq!(layout::STANDARD_HEIGHT, px(500.0));
        assert_eq!(layout::MAX_HEIGHT, px(700.0));
    }
}
