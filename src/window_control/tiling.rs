use anyhow::Result;
use tracing::{info, instrument};

use super::ax::{get_window_position, get_window_size, set_window_position, set_window_size};
use super::display::{get_all_display_bounds, get_visible_display_bounds};
use super::types::*;

/// Tile a window to a predefined position on the screen.
///
/// # Arguments
/// * `window_id` - The unique window identifier from `list_windows()`
/// * `position` - The tiling position (half, quadrant, or fullscreen)
///
/// # Errors
/// Returns error if window not found or operation fails.
#[instrument]
pub(super) fn tile_window(window_id: u32, position: TilePosition) -> Result<()> {
    let (_observation, window) = super::actions::resolve_action_target(window_id)?;

    // Get current position to determine which display the window is on
    let (current_x, current_y) = get_window_position(window.as_ptr()).unwrap_or((0, 0));

    // Get the visible display bounds (accounting for menu bar and dock)
    let display = get_visible_display_bounds(current_x, current_y);

    let bounds = calculate_tile_bounds(&display, position);

    set_window_position(window.as_ptr(), bounds.x, bounds.y)?;
    set_window_size(window.as_ptr(), bounds.width, bounds.height)?;

    info!(window_id, ?position, "Tiled window");
    Ok(())
}

/// Move a window to the next display (cycles through available displays).
#[instrument]
pub(super) fn move_to_next_display(window_id: u32) -> Result<()> {
    move_to_adjacent_display(window_id, true)
}

/// Move a window to the previous display (cycles through available displays).
#[instrument]
pub(super) fn move_to_previous_display(window_id: u32) -> Result<()> {
    move_to_adjacent_display(window_id, false)
}

/// Internal helper to move window to adjacent display
pub(super) fn move_to_adjacent_display(window_id: u32, next: bool) -> Result<()> {
    let (_observation, window) = super::actions::resolve_action_target(window_id)?;

    let (current_x, current_y) = get_window_position(window.as_ptr()).unwrap_or((0, 0));
    let (current_width, current_height) = get_window_size(window.as_ptr()).unwrap_or((800, 600));

    let displays = get_all_display_bounds()?;
    if displays.len() <= 1 {
        info!(window_id, "Only one display, cannot move to adjacent");
        return Ok(());
    }

    let current_bounds = Bounds::new(current_x, current_y, current_width, current_height);
    let target = super::presets::legacy_adjacent_display_bounds(
        current_bounds,
        (current_width, current_height),
        &displays,
        next,
    )?;

    set_window_position(window.as_ptr(), target.x, target.y)?;
    set_window_size(window.as_ptr(), target.width, target.height)?;

    info!(
        window_id,
        "Moved window to {} display",
        if next { "next" } else { "previous" }
    );
    Ok(())
}

/// Calculate the bounds for a tiling position within a display.
///
/// Delegates to the preset engine in `GeometryMode::LegacyV1` — output is
/// byte-identical to the historical formulas (snap calibration depends on it).
pub(super) fn calculate_tile_bounds(display: &Bounds, position: TilePosition) -> Bounds {
    super::presets::resolve_tile_position(
        *display,
        position,
        super::geometry::GeometryMode::LegacyV1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_tile_bounds_left_half() {
        let display = Bounds::new(0, 25, 1920, 1055);
        let bounds = calculate_tile_bounds(&display, TilePosition::LeftHalf);

        assert_eq!(bounds.x, 0);
        assert_eq!(bounds.y, 25);
        assert_eq!(bounds.width, 960);
        assert_eq!(bounds.height, 1055);
    }

    #[test]
    fn test_calculate_tile_bounds_right_half() {
        let display = Bounds::new(0, 25, 1920, 1055);
        let bounds = calculate_tile_bounds(&display, TilePosition::RightHalf);

        assert_eq!(bounds.x, 960);
        assert_eq!(bounds.y, 25);
        assert_eq!(bounds.width, 960);
        assert_eq!(bounds.height, 1055);
    }

    #[test]
    fn test_calculate_tile_bounds_top_left() {
        let display = Bounds::new(0, 25, 1920, 1080);
        let bounds = calculate_tile_bounds(&display, TilePosition::TopLeft);

        assert_eq!(bounds.x, 0);
        assert_eq!(bounds.y, 25);
        assert_eq!(bounds.width, 960);
        assert_eq!(bounds.height, 540);
    }

    #[test]
    fn test_calculate_tile_bounds_top_center_sixth() {
        let display = Bounds::new(0, 25, 1920, 1080);
        let bounds = calculate_tile_bounds(&display, TilePosition::TopCenterSixth);

        assert_eq!(bounds.x, 640);
        assert_eq!(bounds.y, 25);
        assert_eq!(bounds.width, 640);
        assert_eq!(bounds.height, 540);
    }

    #[test]
    fn test_calculate_tile_bounds_bottom_right_sixth() {
        let display = Bounds::new(0, 25, 1920, 1080);
        let bounds = calculate_tile_bounds(&display, TilePosition::BottomRightSixth);

        assert_eq!(bounds.x, 1280);
        assert_eq!(bounds.y, 565);
        assert_eq!(bounds.width, 640);
        assert_eq!(bounds.height, 540);
    }

    #[test]
    fn test_calculate_tile_bounds_fullscreen() {
        let display = Bounds::new(0, 25, 1920, 1055);
        let bounds = calculate_tile_bounds(&display, TilePosition::Fullscreen);

        assert_eq!(bounds, display);
    }

    #[test]
    fn test_calculate_tile_bounds_display_navigation_stubs_return_display() {
        let display = Bounds::new(0, 25, 1920, 1055);
        let next_display = calculate_tile_bounds(&display, TilePosition::NextDisplay);
        let previous_display = calculate_tile_bounds(&display, TilePosition::PreviousDisplay);

        assert_eq!(next_display, display);
        assert_eq!(previous_display, display);
    }

    #[test]
    #[ignore] // Requires accessibility permission and a visible window
    fn test_tile_window_left_half() {
        let windows = super::super::list_windows().expect("Should list windows");
        if let Some(window) = windows.first() {
            tile_window(window.id, TilePosition::LeftHalf).expect("Should tile window");
            println!("Tiled '{}' to left half", window.title);
        } else {
            panic!("No windows found to test with");
        }
    }
}
