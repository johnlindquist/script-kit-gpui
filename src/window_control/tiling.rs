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

    let current_display_idx = displays
        .iter()
        .position(|d| {
            current_x >= d.x
                && current_x < d.x + d.width as i32
                && current_y >= d.y
                && current_y < d.y + d.height as i32
        })
        .unwrap_or(0);

    let target_idx = if next {
        (current_display_idx + 1) % displays.len()
    } else if current_display_idx == 0 {
        displays.len() - 1
    } else {
        current_display_idx - 1
    };

    let current_display = &displays[current_display_idx];
    let target_display = &displays[target_idx];

    let rel_x = (current_x - current_display.x) as f64 / current_display.width as f64;
    let rel_y = (current_y - current_display.y) as f64 / current_display.height as f64;

    let new_x = target_display.x + (rel_x * target_display.width as f64) as i32;
    let new_y = target_display.y + (rel_y * target_display.height as f64) as i32;

    let scale_x = target_display.width as f64 / current_display.width as f64;
    let scale_y = target_display.height as f64 / current_display.height as f64;
    let new_width = (current_width as f64 * scale_x).min(target_display.width as f64) as u32;
    let new_height = (current_height as f64 * scale_y).min(target_display.height as f64) as u32;

    set_window_position(window.as_ptr(), new_x, new_y)?;
    set_window_size(window.as_ptr(), new_width, new_height)?;

    info!(
        window_id,
        from_display = current_display_idx,
        to_display = target_idx,
        "Moved window to {} display",
        if next { "next" } else { "previous" }
    );
    Ok(())
}

/// Calculate the bounds for a tiling position within a display.
pub(super) fn calculate_tile_bounds(display: &Bounds, position: TilePosition) -> Bounds {
    let half_width = display.width / 2;
    let half_height = display.height / 2;
    let third_width = display.width / 3;
    let third_height = display.height / 3;
    let two_thirds_width = (display.width * 2) / 3;
    let two_thirds_height = (display.height * 2) / 3;

    match position {
        // Half positions
        TilePosition::LeftHalf => Bounds {
            x: display.x,
            y: display.y,
            width: half_width,
            height: display.height,
        },
        TilePosition::RightHalf => Bounds {
            x: display.x + half_width as i32,
            y: display.y,
            width: half_width,
            height: display.height,
        },
        TilePosition::TopHalf => Bounds {
            x: display.x,
            y: display.y,
            width: display.width,
            height: half_height,
        },
        TilePosition::BottomHalf => Bounds {
            x: display.x,
            y: display.y + half_height as i32,
            width: display.width,
            height: half_height,
        },

        // Quadrant positions
        TilePosition::TopLeft => Bounds {
            x: display.x,
            y: display.y,
            width: half_width,
            height: half_height,
        },
        TilePosition::TopRight => Bounds {
            x: display.x + half_width as i32,
            y: display.y,
            width: half_width,
            height: half_height,
        },
        TilePosition::BottomLeft => Bounds {
            x: display.x,
            y: display.y + half_height as i32,
            width: half_width,
            height: half_height,
        },
        TilePosition::BottomRight => Bounds {
            x: display.x + half_width as i32,
            y: display.y + half_height as i32,
            width: half_width,
            height: half_height,
        },

        // Sixth positions (top/bottom row split into thirds)
        TilePosition::TopLeftSixth => Bounds {
            x: display.x,
            y: display.y,
            width: third_width,
            height: half_height,
        },
        TilePosition::TopCenterSixth => Bounds {
            x: display.x + third_width as i32,
            y: display.y,
            width: third_width,
            height: half_height,
        },
        TilePosition::TopRightSixth => Bounds {
            x: display.x + two_thirds_width as i32,
            y: display.y,
            width: third_width,
            height: half_height,
        },
        TilePosition::BottomLeftSixth => Bounds {
            x: display.x,
            y: display.y + half_height as i32,
            width: third_width,
            height: half_height,
        },
        TilePosition::BottomCenterSixth => Bounds {
            x: display.x + third_width as i32,
            y: display.y + half_height as i32,
            width: third_width,
            height: half_height,
        },
        TilePosition::BottomRightSixth => Bounds {
            x: display.x + two_thirds_width as i32,
            y: display.y + half_height as i32,
            width: third_width,
            height: half_height,
        },

        // Horizontal thirds positions
        TilePosition::LeftThird => Bounds {
            x: display.x,
            y: display.y,
            width: third_width,
            height: display.height,
        },
        TilePosition::CenterThird => Bounds {
            x: display.x + third_width as i32,
            y: display.y,
            width: third_width,
            height: display.height,
        },
        TilePosition::RightThird => Bounds {
            x: display.x + (two_thirds_width) as i32,
            y: display.y,
            width: third_width,
            height: display.height,
        },

        // Vertical thirds positions
        TilePosition::TopThird => Bounds {
            x: display.x,
            y: display.y,
            width: display.width,
            height: third_height,
        },
        TilePosition::MiddleThird => Bounds {
            x: display.x,
            y: display.y + third_height as i32,
            width: display.width,
            height: third_height,
        },
        TilePosition::BottomThird => Bounds {
            x: display.x,
            y: display.y + two_thirds_height as i32,
            width: display.width,
            height: third_height,
        },

        // Horizontal two-thirds positions
        TilePosition::FirstTwoThirds => Bounds {
            x: display.x,
            y: display.y,
            width: two_thirds_width,
            height: display.height,
        },
        TilePosition::LastTwoThirds => Bounds {
            x: display.x + third_width as i32,
            y: display.y,
            width: two_thirds_width,
            height: display.height,
        },

        // Vertical two-thirds positions
        TilePosition::TopTwoThirds => Bounds {
            x: display.x,
            y: display.y,
            width: display.width,
            height: two_thirds_height,
        },
        TilePosition::BottomTwoThirds => Bounds {
            x: display.x,
            y: display.y + third_height as i32,
            width: display.width,
            height: two_thirds_height,
        },

        // Centered positions
        TilePosition::Center => {
            // 60% of screen, centered
            let width = (display.width * 60) / 100;
            let height = (display.height * 60) / 100;
            let x_offset = (display.width - width) / 2;
            let y_offset = (display.height - height) / 2;
            Bounds {
                x: display.x + x_offset as i32,
                y: display.y + y_offset as i32,
                width,
                height,
            }
        }
        TilePosition::AlmostMaximize => {
            // 90% of screen with margins
            let margin_x = (display.width * 5) / 100; // 5% margin on each side
            let margin_y = (display.height * 5) / 100;
            Bounds {
                x: display.x + margin_x as i32,
                y: display.y + margin_y as i32,
                width: display.width - (margin_x * 2),
                height: display.height - (margin_y * 2),
            }
        }

        TilePosition::Fullscreen | TilePosition::NextDisplay | TilePosition::PreviousDisplay => {
            *display
        }
    }
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
