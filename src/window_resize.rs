//! Dynamic Window Resizing Module
//!
//! Handles dynamic window height calculations for Script Kit GPUI.
//! The window resizes based on content:
//! - Empty/no choices: Compact height (input field only)
//! - With choices: Expands to show list items
//!
//! This matches the behavior of the original Script Kit where
//! the window shrinks when filtering yields no results.

#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSPoint, NSRect, NSSize};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

use gpui::{px, Pixels};

use crate::logging;

/// Layout constants for height calculations
pub mod layout {
    use gpui::{px, Pixels};

    /// Fixed header height (logo + input field area)
    pub const HEADER_HEIGHT: Pixels = px(100.0);

    /// Height of each list item
    pub const LIST_ITEM_HEIGHT: Pixels = px(52.0);

    /// Footer/actions bar height when visible
    pub const FOOTER_HEIGHT: Pixels = px(44.0);

    /// Minimum window height (header only, no list)
    pub const MIN_HEIGHT: Pixels = px(120.0);

    /// Maximum window height (cap to prevent overly tall windows)
    pub const MAX_HEIGHT: Pixels = px(700.0);

    /// Default window width (constant)
    #[allow(dead_code)]
    pub const WINDOW_WIDTH: Pixels = px(750.0);

    /// Maximum number of visible items before scrolling
    pub const MAX_VISIBLE_ITEMS: usize = 10;

    /// Padding at bottom of list area
    pub const LIST_PADDING: Pixels = px(8.0);
}

/// Configuration for window resize behavior
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct ResizeConfig {
    /// Whether to animate the resize (future feature)
    pub animate: bool,
    /// Debounce time in milliseconds for rapid resize events
    pub debounce_ms: u64,
    /// Whether to include footer in height calculation
    pub include_footer: bool,
}

impl Default for ResizeConfig {
    fn default() -> Self {
        Self {
            animate: false,
            debounce_ms: 16, // ~60fps
            include_footer: true,
        }
    }
}

/// Calculate the optimal window height based on content
///
/// # Arguments
/// * `item_count` - Number of items in the filtered list
/// * `config` - Resize configuration options
///
/// # Returns
/// The calculated window height in pixels
pub fn calculate_window_height(item_count: usize, config: &ResizeConfig) -> Pixels {
    use layout::*;

    if item_count == 0 {
        // No items: compact mode (header only)
        logging::log(
            "RESIZE",
            &format!("Compact mode: 0 items -> height={:.0}", f32::from(MIN_HEIGHT)),
        );
        return MIN_HEIGHT;
    }

    // Calculate list height based on visible items
    let visible_items = item_count.min(MAX_VISIBLE_ITEMS);
    let list_height = px(visible_items as f32 * f32::from(LIST_ITEM_HEIGHT));

    // Total height = header + list + optional footer + padding
    let mut total_height = HEADER_HEIGHT + list_height + LIST_PADDING;

    if config.include_footer {
        total_height += FOOTER_HEIGHT;
    }

    // Clamp to min/max bounds
    let final_height = if total_height < MIN_HEIGHT {
        MIN_HEIGHT
    } else if total_height > MAX_HEIGHT {
        MAX_HEIGHT
    } else {
        total_height
    };

    logging::log(
        "RESIZE",
        &format!(
            "Height calc: {} items, {} visible -> header({:.0}) + list({:.0}) + footer({:.0}) = {:.0}",
            item_count,
            visible_items,
            f32::from(HEADER_HEIGHT),
            f32::from(list_height),
            if config.include_footer { f32::from(FOOTER_HEIGHT) } else { 0.0 },
            f32::from(final_height)
        ),
    );

    final_height
}

/// Calculate window height for specific view types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewType {
    /// Script list view (main launcher)
    ScriptList,
    /// Arg prompt with choices
    ArgPromptWithChoices,
    /// Arg prompt without choices (input only)
    ArgPromptNoChoices,
    /// Div prompt (HTML display)
    DivPrompt,
    /// Editor prompt (code editor)
    EditorPrompt,
    /// Terminal prompt
    TermPrompt,
}

/// Get the target height for a specific view type
///
/// # Arguments
/// * `view_type` - The type of view being displayed
/// * `item_count` - Number of items (for list-based views)
///
/// # Returns
/// The optimal window height for this view
pub fn height_for_view(view_type: ViewType, item_count: usize) -> Pixels {
    use layout::*;

    match view_type {
        ViewType::ScriptList | ViewType::ArgPromptWithChoices => {
            calculate_window_height(item_count, &ResizeConfig::default())
        }
        ViewType::ArgPromptNoChoices => {
            // Just header, no list
            MIN_HEIGHT
        }
        ViewType::DivPrompt | ViewType::EditorPrompt | ViewType::TermPrompt => {
            // Full height for content views
            MAX_HEIGHT
        }
    }
}

/// Determine if window resize is needed
///
/// # Arguments
/// * `current_height` - Current window height
/// * `target_height` - Desired window height
/// * `threshold` - Minimum difference to trigger resize (prevents jitter)
///
/// # Returns
/// `true` if resize is needed, `false` if heights are close enough
#[allow(dead_code)]
pub fn needs_resize(current_height: Pixels, target_height: Pixels, threshold: Pixels) -> bool {
    let diff = (f32::from(current_height) - f32::from(target_height)).abs();
    diff > f32::from(threshold)
}

/// Resize the first window to a new height, keeping it centered horizontally.
///
/// This function:
/// 1. Gets the current window frame
/// 2. Calculates new frame with the target height
/// 3. Keeps the window horizontally centered on its current display
/// 4. Anchors the resize from the top (window top stays fixed)
///
/// # Arguments
/// * `target_height` - The desired window height in pixels
///
/// # Platform
/// This function only works on macOS. On other platforms, it's a no-op.
#[cfg(target_os = "macos")]
pub fn resize_first_window_to_height(target_height: Pixels) {
    let height_f64: f64 = f32::from(target_height) as f64;

    unsafe {
        let app: id = NSApp();

        // Get all windows and find our main window
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];

        logging::log(
            "RESIZE",
            &format!(
                "resize_first_window_to_height: target={:.0}, window_count={}",
                height_f64, count
            ),
        );

        if count == 0 {
            logging::log("RESIZE", "ERROR: No windows found!");
            return;
        }

        // Get the first window (our main window)
        let window: id = msg_send![windows, objectAtIndex:0usize];

        if window == nil {
            logging::log("RESIZE", "ERROR: First window is nil!");
            return;
        }

        // Get the PRIMARY screen's height for coordinate conversion
        let screens: id = msg_send![class!(NSScreen), screens];
        let main_screen: id = msg_send![screens, firstObject];
        let main_screen_frame: NSRect = msg_send![main_screen, frame];
        let _primary_screen_height = main_screen_frame.size.height;

        // Get current window frame
        let current_frame: NSRect = msg_send![window, frame];

        logging::log(
            "RESIZE",
            &format!(
                "Current frame: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
                current_frame.origin.x,
                current_frame.origin.y,
                current_frame.size.width,
                current_frame.size.height
            ),
        );

        // Calculate height difference
        let height_delta = height_f64 - current_frame.size.height;

        // macOS coordinate system: Y=0 at bottom, increases upward
        // To keep the TOP of the window fixed, we need to adjust the origin.y
        // When height increases, origin.y should decrease
        // When height decreases, origin.y should increase
        let new_origin_y = current_frame.origin.y - height_delta;

        let new_frame = NSRect::new(
            NSPoint::new(current_frame.origin.x, new_origin_y),
            NSSize::new(current_frame.size.width, height_f64),
        );

        logging::log(
            "RESIZE",
            &format!(
                "New frame: origin=({:.0}, {:.0}) size={:.0}x{:.0} (height_delta={:.0})",
                new_frame.origin.x,
                new_frame.origin.y,
                new_frame.size.width,
                new_frame.size.height,
                height_delta
            ),
        );

        // Apply the new frame (with optional animation in the future)
        let _: () = msg_send![window, setFrame:new_frame display:true animate:false];

        // Verify the resize worked
        let after_frame: NSRect = msg_send![window, frame];
        logging::log(
            "RESIZE",
            &format!(
                "After resize: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
                after_frame.origin.x,
                after_frame.origin.y,
                after_frame.size.width,
                after_frame.size.height
            ),
        );
    }
}

/// Get the current height of the first window
///
/// # Returns
/// The current window height in pixels, or None if no window exists
#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub fn get_first_window_height() -> Option<Pixels> {
    unsafe {
        let app: id = NSApp();
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];

        if count == 0 {
            return None;
        }

        let window: id = msg_send![windows, objectAtIndex:0usize];
        if window == nil {
            return None;
        }

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

/// High-level function to update window size based on item count.
///
/// This is the main entry point for dynamic resizing:
/// 1. Calculates the target height based on item count
/// 2. Gets the current window height
/// 3. Only resizes if the difference exceeds a threshold
///
/// # Arguments
/// * `item_count` - Number of items in the list
/// * `config` - Resize configuration options
///
/// # Returns
/// `true` if resize was performed, `false` if skipped
#[allow(dead_code)]
pub fn update_window_for_item_count(item_count: usize, config: &ResizeConfig) -> bool {
    let target_height = calculate_window_height(item_count, config);

    // Get current height to check if resize is needed
    if let Some(current_height) = get_first_window_height() {
        let threshold = px(10.0); // 10px threshold to prevent jitter

        if needs_resize(current_height, target_height, threshold) {
            logging::log(
                "RESIZE",
                &format!(
                    "Resizing: {} items, current={:.0} -> target={:.0}",
                    item_count,
                    f32::from(current_height),
                    f32::from(target_height)
                ),
            );
            resize_first_window_to_height(target_height);
            return true;
        } else {
            logging::log(
                "RESIZE",
                &format!(
                    "Skip resize: {} items, current={:.0} ≈ target={:.0}",
                    item_count,
                    f32::from(current_height),
                    f32::from(target_height)
                ),
            );
        }
    } else {
        // No current height available, just resize
        logging::log(
            "RESIZE",
            &format!(
                "Resizing (no current height): {} items -> target={:.0}",
                item_count,
                f32::from(target_height)
            ),
        );
        resize_first_window_to_height(target_height);
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn test_zero_items_compact_height() {
        let height = calculate_window_height(0, &ResizeConfig::default());
        assert_eq!(height, layout::MIN_HEIGHT);
    }

    #[test]
    fn test_single_item_height() {
        let height = calculate_window_height(1, &ResizeConfig::default());
        // Should be header + 1 item + footer + padding
        let expected = layout::HEADER_HEIGHT
            + layout::LIST_ITEM_HEIGHT
            + layout::FOOTER_HEIGHT
            + layout::LIST_PADDING;
        assert_eq!(height, expected);
    }

    #[test]
    fn test_max_visible_items() {
        // More items than max visible should cap at MAX_VISIBLE_ITEMS
        let height = calculate_window_height(100, &ResizeConfig::default());
        let max_list_height = px(layout::MAX_VISIBLE_ITEMS as f32 * f32::from(layout::LIST_ITEM_HEIGHT));
        let expected_max = layout::HEADER_HEIGHT
            + max_list_height
            + layout::FOOTER_HEIGHT
            + layout::LIST_PADDING;
        assert_eq!(height, expected_max.min(layout::MAX_HEIGHT));
    }

    #[test]
    fn test_needs_resize_threshold() {
        let current = px(500.0);
        let target_close = px(502.0);
        let target_far = px(600.0);
        let threshold = px(5.0);

        assert!(!needs_resize(current, target_close, threshold));
        assert!(needs_resize(current, target_far, threshold));
    }

    #[test]
    fn test_view_types() {
        assert_eq!(height_for_view(ViewType::ArgPromptNoChoices, 0), layout::MIN_HEIGHT);
        assert_eq!(height_for_view(ViewType::EditorPrompt, 0), layout::MAX_HEIGHT);
        assert_eq!(height_for_view(ViewType::DivPrompt, 0), layout::MAX_HEIGHT);
    }
}
