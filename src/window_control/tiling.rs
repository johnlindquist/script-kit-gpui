//! Legacy geometry compatibility helpers.
//!
//! No AX setters live here: tiling execution routes through the plan/
//! transaction engine (see `actions.rs`/`legacy.rs`). This module keeps the
//! `calculate_tile_bounds` compatibility entry point that snap target
//! construction depends on.

use super::types::*;

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
            super::super::tile_window(window.id, TilePosition::LeftHalf)
                .expect("Should tile window");
            println!("Tiled '{}' to left half", window.title);
        } else {
            panic!("No windows found to test with");
        }
    }
}
