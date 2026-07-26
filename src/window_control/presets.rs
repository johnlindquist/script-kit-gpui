//! Placement presets and layout targets.
//!
//! `resolve_legacy_preset` carries the historical `calculate_tile_bounds`
//! formulas VERBATIM (truncating thirds and all) — snap calibration and every
//! legacy route depend on byte-identical output. `resolve_exact_preset` maps
//! the same presets onto exact grids (`GeometryMode::ExactV2`) for new plan
//! APIs. `legacy_adjacent_display_bounds` preserves the historical
//! next/previous-display transform as a pure function.

use anyhow::{bail, Context, Result};

use super::geometry::{grid_bounds, Anchor, GeometryMode, GridCell, NormalizedBounds};
use super::types::{Bounds, DisplayDescriptor, TilePosition};

/// Every internal placement preset (display routing excluded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresetId {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    TopLeftSixth,
    TopCenterSixth,
    TopRightSixth,
    BottomLeftSixth,
    BottomCenterSixth,
    BottomRightSixth,
    LeftThird,
    CenterThird,
    RightThird,
    TopThird,
    MiddleThird,
    BottomThird,
    FirstTwoThirds,
    LastTwoThirds,
    TopTwoThirds,
    BottomTwoThirds,
    Center,
    AlmostMaximize,
    Maximize,
}

/// A general placement target for plans.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutTarget {
    Preset(PresetId),
    GridCell(GridCell),
    Normalized(NormalizedBounds),
    Pixels {
        width: u32,
        height: u32,
        anchor: Anchor,
        offset_x: i32,
        offset_y: i32,
    },
    Maximize,
    /// Restore the previous frame — unsupported until window memory lands.
    Restore,
}

/// Map a legacy `TilePosition` to its preset. Routing positions return None.
pub(super) fn preset_for_tile_position(position: TilePosition) -> Option<PresetId> {
    Some(match position {
        TilePosition::LeftHalf => PresetId::LeftHalf,
        TilePosition::RightHalf => PresetId::RightHalf,
        TilePosition::TopHalf => PresetId::TopHalf,
        TilePosition::BottomHalf => PresetId::BottomHalf,
        TilePosition::TopLeft => PresetId::TopLeft,
        TilePosition::TopRight => PresetId::TopRight,
        TilePosition::BottomLeft => PresetId::BottomLeft,
        TilePosition::BottomRight => PresetId::BottomRight,
        TilePosition::TopLeftSixth => PresetId::TopLeftSixth,
        TilePosition::TopCenterSixth => PresetId::TopCenterSixth,
        TilePosition::TopRightSixth => PresetId::TopRightSixth,
        TilePosition::BottomLeftSixth => PresetId::BottomLeftSixth,
        TilePosition::BottomCenterSixth => PresetId::BottomCenterSixth,
        TilePosition::BottomRightSixth => PresetId::BottomRightSixth,
        TilePosition::LeftThird => PresetId::LeftThird,
        TilePosition::CenterThird => PresetId::CenterThird,
        TilePosition::RightThird => PresetId::RightThird,
        TilePosition::TopThird => PresetId::TopThird,
        TilePosition::MiddleThird => PresetId::MiddleThird,
        TilePosition::BottomThird => PresetId::BottomThird,
        TilePosition::FirstTwoThirds => PresetId::FirstTwoThirds,
        TilePosition::LastTwoThirds => PresetId::LastTwoThirds,
        TilePosition::TopTwoThirds => PresetId::TopTwoThirds,
        TilePosition::BottomTwoThirds => PresetId::BottomTwoThirds,
        TilePosition::Center => PresetId::Center,
        TilePosition::AlmostMaximize => PresetId::AlmostMaximize,
        TilePosition::Fullscreen => PresetId::Maximize,
        TilePosition::NextDisplay | TilePosition::PreviousDisplay => return None,
    })
}

/// The HISTORICAL formulas, moved verbatim from `calculate_tile_bounds`.
/// Do not "fix" the truncating arithmetic: snap calibration and legacy
/// parity depend on these exact values.
pub(super) fn resolve_legacy_preset(display: Bounds, preset: PresetId) -> Bounds {
    let half_width = display.width / 2;
    let half_height = display.height / 2;
    let third_width = display.width / 3;
    let third_height = display.height / 3;
    let two_thirds_width = (display.width * 2) / 3;
    let two_thirds_height = (display.height * 2) / 3;

    match preset {
        PresetId::LeftHalf => Bounds {
            x: display.x,
            y: display.y,
            width: half_width,
            height: display.height,
        },
        PresetId::RightHalf => Bounds {
            x: display.x + half_width as i32,
            y: display.y,
            width: half_width,
            height: display.height,
        },
        PresetId::TopHalf => Bounds {
            x: display.x,
            y: display.y,
            width: display.width,
            height: half_height,
        },
        PresetId::BottomHalf => Bounds {
            x: display.x,
            y: display.y + half_height as i32,
            width: display.width,
            height: half_height,
        },
        PresetId::TopLeft => Bounds {
            x: display.x,
            y: display.y,
            width: half_width,
            height: half_height,
        },
        PresetId::TopRight => Bounds {
            x: display.x + half_width as i32,
            y: display.y,
            width: half_width,
            height: half_height,
        },
        PresetId::BottomLeft => Bounds {
            x: display.x,
            y: display.y + half_height as i32,
            width: half_width,
            height: half_height,
        },
        PresetId::BottomRight => Bounds {
            x: display.x + half_width as i32,
            y: display.y + half_height as i32,
            width: half_width,
            height: half_height,
        },
        PresetId::TopLeftSixth => Bounds {
            x: display.x,
            y: display.y,
            width: third_width,
            height: half_height,
        },
        PresetId::TopCenterSixth => Bounds {
            x: display.x + third_width as i32,
            y: display.y,
            width: third_width,
            height: half_height,
        },
        PresetId::TopRightSixth => Bounds {
            x: display.x + two_thirds_width as i32,
            y: display.y,
            width: third_width,
            height: half_height,
        },
        PresetId::BottomLeftSixth => Bounds {
            x: display.x,
            y: display.y + half_height as i32,
            width: third_width,
            height: half_height,
        },
        PresetId::BottomCenterSixth => Bounds {
            x: display.x + third_width as i32,
            y: display.y + half_height as i32,
            width: third_width,
            height: half_height,
        },
        PresetId::BottomRightSixth => Bounds {
            x: display.x + two_thirds_width as i32,
            y: display.y + half_height as i32,
            width: third_width,
            height: half_height,
        },
        PresetId::LeftThird => Bounds {
            x: display.x,
            y: display.y,
            width: third_width,
            height: display.height,
        },
        PresetId::CenterThird => Bounds {
            x: display.x + third_width as i32,
            y: display.y,
            width: third_width,
            height: display.height,
        },
        PresetId::RightThird => Bounds {
            x: display.x + (two_thirds_width) as i32,
            y: display.y,
            width: third_width,
            height: display.height,
        },
        PresetId::TopThird => Bounds {
            x: display.x,
            y: display.y,
            width: display.width,
            height: third_height,
        },
        PresetId::MiddleThird => Bounds {
            x: display.x,
            y: display.y + third_height as i32,
            width: display.width,
            height: third_height,
        },
        PresetId::BottomThird => Bounds {
            x: display.x,
            y: display.y + two_thirds_height as i32,
            width: display.width,
            height: third_height,
        },
        PresetId::FirstTwoThirds => Bounds {
            x: display.x,
            y: display.y,
            width: two_thirds_width,
            height: display.height,
        },
        PresetId::LastTwoThirds => Bounds {
            x: display.x + third_width as i32,
            y: display.y,
            width: two_thirds_width,
            height: display.height,
        },
        PresetId::TopTwoThirds => Bounds {
            x: display.x,
            y: display.y,
            width: display.width,
            height: two_thirds_height,
        },
        PresetId::BottomTwoThirds => Bounds {
            x: display.x,
            y: display.y + third_height as i32,
            width: display.width,
            height: two_thirds_height,
        },
        PresetId::Center => {
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
        PresetId::AlmostMaximize => {
            let margin_x = (display.width * 5) / 100;
            let margin_y = (display.height * 5) / 100;
            Bounds {
                x: display.x + margin_x as i32,
                y: display.y + margin_y as i32,
                width: display.width - (margin_x * 2),
                height: display.height - (margin_y * 2),
            }
        }
        PresetId::Maximize => display,
    }
}

/// ExactV2 grid mapping per preset.
fn exact_grid_for(preset: PresetId) -> Option<GridCell> {
    Some(match preset {
        PresetId::LeftHalf => GridCell::new(2, 1, 0, 0),
        PresetId::RightHalf => GridCell::new(2, 1, 1, 0),
        PresetId::TopHalf => GridCell::new(1, 2, 0, 0),
        PresetId::BottomHalf => GridCell::new(1, 2, 0, 1),
        PresetId::TopLeft => GridCell::new(2, 2, 0, 0),
        PresetId::TopRight => GridCell::new(2, 2, 1, 0),
        PresetId::BottomLeft => GridCell::new(2, 2, 0, 1),
        PresetId::BottomRight => GridCell::new(2, 2, 1, 1),
        PresetId::TopLeftSixth => GridCell::new(3, 2, 0, 0),
        PresetId::TopCenterSixth => GridCell::new(3, 2, 1, 0),
        PresetId::TopRightSixth => GridCell::new(3, 2, 2, 0),
        PresetId::BottomLeftSixth => GridCell::new(3, 2, 0, 1),
        PresetId::BottomCenterSixth => GridCell::new(3, 2, 1, 1),
        PresetId::BottomRightSixth => GridCell::new(3, 2, 2, 1),
        PresetId::LeftThird => GridCell::new(3, 1, 0, 0),
        PresetId::CenterThird => GridCell::new(3, 1, 1, 0),
        PresetId::RightThird => GridCell::new(3, 1, 2, 0),
        PresetId::TopThird => GridCell::new(1, 3, 0, 0),
        PresetId::MiddleThird => GridCell::new(1, 3, 0, 1),
        PresetId::BottomThird => GridCell::new(1, 3, 0, 2),
        PresetId::FirstTwoThirds => GridCell::new(3, 1, 0, 0).with_span(2, 1),
        PresetId::LastTwoThirds => GridCell::new(3, 1, 1, 0).with_span(2, 1),
        PresetId::TopTwoThirds => GridCell::new(1, 3, 0, 0).with_span(1, 2),
        PresetId::BottomTwoThirds => GridCell::new(1, 3, 0, 1).with_span(1, 2),
        PresetId::Center | PresetId::AlmostMaximize | PresetId::Maximize => return None,
    })
}

/// Resolve a preset within a display frame under the chosen geometry mode.
pub(super) fn resolve_preset(display: Bounds, preset: PresetId, mode: GeometryMode) -> Bounds {
    match mode {
        GeometryMode::LegacyV1 => resolve_legacy_preset(display, preset),
        GeometryMode::ExactV2 => match exact_grid_for(preset) {
            Some(cell) => grid_bounds(display, cell)
                .unwrap_or_else(|_| resolve_legacy_preset(display, preset)),
            // Center/AlmostMaximize/Maximize keep their percentage semantics.
            None => resolve_legacy_preset(display, preset),
        },
    }
}

/// Resolve a legacy tile position under the chosen mode. Routing positions
/// (NextDisplay/PreviousDisplay) return the display frame, as historically.
pub(super) fn resolve_tile_position(
    display: Bounds,
    position: TilePosition,
    mode: GeometryMode,
) -> Bounds {
    match preset_for_tile_position(position) {
        Some(preset) => resolve_preset(display, preset, mode),
        None => display,
    }
}

/// Resolve a general layout target within a display.
pub(super) fn resolve_layout_target(
    target: &LayoutTarget,
    display: &DisplayDescriptor,
    mode: GeometryMode,
) -> Result<Bounds> {
    let frame = display.visible_bounds;
    Ok(match target {
        LayoutTarget::Preset(preset) => resolve_preset(frame, *preset, mode),
        LayoutTarget::GridCell(cell) => grid_bounds(frame, *cell)?,
        LayoutTarget::Normalized(normalized) => super::geometry::denormalize(*normalized, frame),
        LayoutTarget::Pixels {
            width,
            height,
            anchor,
            offset_x,
            offset_y,
        } => {
            super::geometry::anchored_pixels(frame, *width, *height, *anchor, *offset_x, *offset_y)
        }
        LayoutTarget::Maximize => frame,
        LayoutTarget::Restore => bail!("unsupported in foundation wave: restore"),
    })
}

/// The HISTORICAL next/previous-display transform as a pure function.
///
/// Preserves exactly: top-left-point source-display selection (fallback 0),
/// legacy enumeration order, relative x/y transform, independent
/// width/height scaling, destination size cap, and truncation direction.
pub(super) fn legacy_adjacent_display_bounds(
    current_bounds: Bounds,
    current_size: (u32, u32),
    displays: &[Bounds],
    next: bool,
) -> Result<Bounds> {
    anyhow::ensure!(!displays.is_empty(), "no displays");
    if displays.len() <= 1 {
        // Historical behavior: single display is a no-op (caller returns Ok).
        return Ok(Bounds::new(
            current_bounds.x,
            current_bounds.y,
            current_size.0,
            current_size.1,
        ));
    }
    let current_x = current_bounds.x;
    let current_y = current_bounds.y;
    let (current_width, current_height) = current_size;

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

    let current_display = displays
        .get(current_display_idx)
        .context("current display index out of range")?;
    let target_display = displays
        .get(target_idx)
        .context("target display index out of range")?;

    let rel_x = (current_x - current_display.x) as f64 / current_display.width as f64;
    let rel_y = (current_y - current_display.y) as f64 / current_display.height as f64;

    let new_x = target_display.x + (rel_x * target_display.width as f64) as i32;
    let new_y = target_display.y + (rel_y * target_display.height as f64) as i32;

    let scale_x = target_display.width as f64 / current_display.width as f64;
    let scale_y = target_display.height as f64 / current_display.height as f64;
    let new_width = (current_width as f64 * scale_x).min(target_display.width as f64) as u32;
    let new_height = (current_height as f64 * scale_y).min(target_display.height as f64) as u32;

    Ok(Bounds::new(new_x, new_y, new_width, new_height))
}

#[cfg(test)]
mod tests {
    use super::super::geometry::edge;
    use super::*;

    /// FROZEN copy of the pre-S7 `calculate_tile_bounds` implementation used
    /// as the parity oracle. Do not update this to match new code — it exists
    /// to prove `resolve_legacy_preset` never drifts.
    fn frozen_legacy_tile_bounds(display: &Bounds, position: TilePosition) -> Bounds {
        let half_width = display.width / 2;
        let half_height = display.height / 2;
        let third_width = display.width / 3;
        let third_height = display.height / 3;
        let two_thirds_width = (display.width * 2) / 3;
        let two_thirds_height = (display.height * 2) / 3;
        match position {
            TilePosition::LeftHalf => Bounds::new(display.x, display.y, half_width, display.height),
            TilePosition::RightHalf => Bounds::new(
                display.x + half_width as i32,
                display.y,
                half_width,
                display.height,
            ),
            TilePosition::TopHalf => Bounds::new(display.x, display.y, display.width, half_height),
            TilePosition::BottomHalf => Bounds::new(
                display.x,
                display.y + half_height as i32,
                display.width,
                half_height,
            ),
            TilePosition::TopLeft => Bounds::new(display.x, display.y, half_width, half_height),
            TilePosition::TopRight => Bounds::new(
                display.x + half_width as i32,
                display.y,
                half_width,
                half_height,
            ),
            TilePosition::BottomLeft => Bounds::new(
                display.x,
                display.y + half_height as i32,
                half_width,
                half_height,
            ),
            TilePosition::BottomRight => Bounds::new(
                display.x + half_width as i32,
                display.y + half_height as i32,
                half_width,
                half_height,
            ),
            TilePosition::TopLeftSixth => {
                Bounds::new(display.x, display.y, third_width, half_height)
            }
            TilePosition::TopCenterSixth => Bounds::new(
                display.x + third_width as i32,
                display.y,
                third_width,
                half_height,
            ),
            TilePosition::TopRightSixth => Bounds::new(
                display.x + two_thirds_width as i32,
                display.y,
                third_width,
                half_height,
            ),
            TilePosition::BottomLeftSixth => Bounds::new(
                display.x,
                display.y + half_height as i32,
                third_width,
                half_height,
            ),
            TilePosition::BottomCenterSixth => Bounds::new(
                display.x + third_width as i32,
                display.y + half_height as i32,
                third_width,
                half_height,
            ),
            TilePosition::BottomRightSixth => Bounds::new(
                display.x + two_thirds_width as i32,
                display.y + half_height as i32,
                third_width,
                half_height,
            ),
            TilePosition::LeftThird => {
                Bounds::new(display.x, display.y, third_width, display.height)
            }
            TilePosition::CenterThird => Bounds::new(
                display.x + third_width as i32,
                display.y,
                third_width,
                display.height,
            ),
            TilePosition::RightThird => Bounds::new(
                display.x + (two_thirds_width) as i32,
                display.y,
                third_width,
                display.height,
            ),
            TilePosition::TopThird => {
                Bounds::new(display.x, display.y, display.width, third_height)
            }
            TilePosition::MiddleThird => Bounds::new(
                display.x,
                display.y + third_height as i32,
                display.width,
                third_height,
            ),
            TilePosition::BottomThird => Bounds::new(
                display.x,
                display.y + two_thirds_height as i32,
                display.width,
                third_height,
            ),
            TilePosition::FirstTwoThirds => {
                Bounds::new(display.x, display.y, two_thirds_width, display.height)
            }
            TilePosition::LastTwoThirds => Bounds::new(
                display.x + third_width as i32,
                display.y,
                two_thirds_width,
                display.height,
            ),
            TilePosition::TopTwoThirds => {
                Bounds::new(display.x, display.y, display.width, two_thirds_height)
            }
            TilePosition::BottomTwoThirds => Bounds::new(
                display.x,
                display.y + third_height as i32,
                display.width,
                two_thirds_height,
            ),
            TilePosition::Center => {
                let width = (display.width * 60) / 100;
                let height = (display.height * 60) / 100;
                let x_offset = (display.width - width) / 2;
                let y_offset = (display.height - height) / 2;
                Bounds::new(
                    display.x + x_offset as i32,
                    display.y + y_offset as i32,
                    width,
                    height,
                )
            }
            TilePosition::AlmostMaximize => {
                let margin_x = (display.width * 5) / 100;
                let margin_y = (display.height * 5) / 100;
                Bounds::new(
                    display.x + margin_x as i32,
                    display.y + margin_y as i32,
                    display.width - (margin_x * 2),
                    display.height - (margin_y * 2),
                )
            }
            TilePosition::Fullscreen
            | TilePosition::NextDisplay
            | TilePosition::PreviousDisplay => *display,
        }
    }

    const ALL_POSITIONS: [TilePosition; 29] = [
        TilePosition::LeftHalf,
        TilePosition::RightHalf,
        TilePosition::TopHalf,
        TilePosition::BottomHalf,
        TilePosition::TopLeft,
        TilePosition::TopRight,
        TilePosition::BottomLeft,
        TilePosition::BottomRight,
        TilePosition::TopLeftSixth,
        TilePosition::TopCenterSixth,
        TilePosition::TopRightSixth,
        TilePosition::BottomLeftSixth,
        TilePosition::BottomCenterSixth,
        TilePosition::BottomRightSixth,
        TilePosition::LeftThird,
        TilePosition::CenterThird,
        TilePosition::RightThird,
        TilePosition::TopThird,
        TilePosition::MiddleThird,
        TilePosition::BottomThird,
        TilePosition::FirstTwoThirds,
        TilePosition::LastTwoThirds,
        TilePosition::TopTwoThirds,
        TilePosition::BottomTwoThirds,
        TilePosition::Center,
        TilePosition::AlmostMaximize,
        TilePosition::Fullscreen,
        TilePosition::NextDisplay,
        TilePosition::PreviousDisplay,
    ];

    #[test]
    fn legacy_mode_matches_the_frozen_implementation_for_every_position() {
        let frames = [
            Bounds::new(0, 25, 1920, 1055),
            Bounds::new(0, 25, 1920, 1080),
            Bounds::new(-1921, -207, 1081, 1919), // odd portrait, negative origin
            Bounds::new(13, 37, 3441, 1441),      // odd ultrawide
            Bounds::new(0, 0, 101, 103),
        ];
        for frame in frames {
            for position in ALL_POSITIONS {
                assert_eq!(
                    resolve_tile_position(frame, position, GeometryMode::LegacyV1),
                    frozen_legacy_tile_bounds(&frame, position),
                    "legacy drift for {position:?} on {frame:?}"
                );
            }
        }
    }

    #[test]
    fn odd_widths_prove_the_legacy_exact_distinction() {
        // 1081 wide: legacy thirds are 360 each (3*360 = 1080, one column lost).
        let frame = Bounds::new(0, 0, 1081, 900);
        let legacy_right =
            resolve_tile_position(frame, TilePosition::RightThird, GeometryMode::LegacyV1);
        assert_eq!(legacy_right.width, 360);
        assert_eq!(
            legacy_right.x + legacy_right.width as i32,
            720 + 360,
            "legacy loses the final pixel column"
        );

        let exact_right =
            resolve_tile_position(frame, TilePosition::RightThird, GeometryMode::ExactV2);
        assert_eq!(
            exact_right.x + exact_right.width as i32,
            frame.x + frame.width as i32,
            "exact mode covers the full width"
        );
        // Exact thirds tile without gaps.
        let exact_left =
            resolve_tile_position(frame, TilePosition::LeftThird, GeometryMode::ExactV2);
        let exact_center =
            resolve_tile_position(frame, TilePosition::CenterThird, GeometryMode::ExactV2);
        assert_eq!(exact_left.x + exact_left.width as i32, exact_center.x);
        assert_eq!(exact_center.x + exact_center.width as i32, exact_right.x);
    }

    #[test]
    fn exact_two_thirds_span_matches_member_edges() {
        let frame = Bounds::new(-3, 7, 2561, 1441);
        let span = resolve_preset(frame, PresetId::FirstTwoThirds, GeometryMode::ExactV2);
        assert_eq!(span.x, frame.x);
        assert_eq!(span.x + span.width as i32, edge(frame.x, frame.width, 2, 3));
    }

    #[test]
    fn every_public_tile_position_resolves_under_both_modes() {
        let frame = Bounds::new(0, 25, 1920, 1055);
        for position in ALL_POSITIONS {
            let legacy = resolve_tile_position(frame, position, GeometryMode::LegacyV1);
            let exact = resolve_tile_position(frame, position, GeometryMode::ExactV2);
            assert!(legacy.width > 0 && legacy.height > 0);
            assert!(exact.width > 0 && exact.height > 0);
        }
    }

    #[test]
    fn legacy_adjacent_display_transform_matches_the_historical_math() {
        // Two displays side by side, second twice as wide.
        let displays = [
            Bounds::new(0, 25, 1000, 975),
            Bounds::new(1000, 0, 2000, 1200),
        ];
        // Window at 25% x / 50% y of display 0.
        let bounds = Bounds::new(250, 512, 400, 300);
        let moved =
            legacy_adjacent_display_bounds(bounds, (400, 300), &displays, true).expect("move");
        // rel_x = 250/1000 = 0.25 -> 1000 + 0.25*2000 = 1500
        assert_eq!(moved.x, 1500);
        // rel_y = (512-25)/975 -> *1200 truncated
        let rel_y = (512.0 - 25.0) / 975.0;
        assert_eq!(moved.y, (rel_y * 1200.0) as i32);
        // width scales by 2, capped at 2000.
        assert_eq!(moved.width, 800);

        // Previous wraps from index 0 to the last display.
        let wrapped =
            legacy_adjacent_display_bounds(bounds, (400, 300), &displays, false).expect("move");
        assert_eq!(wrapped.x >= 1000, true);

        // Single display is a no-op.
        let single =
            legacy_adjacent_display_bounds(bounds, (400, 300), &displays[..1], true).expect("noop");
        assert_eq!(single, Bounds::new(250, 512, 400, 300));
    }

    #[test]
    fn restore_target_is_rejected_in_the_foundation_wave() {
        let display = DisplayDescriptor {
            id: super::super::types::DisplayId(1),
            uuid: "u".into(),
            localized_name: "d".into(),
            full_bounds: Bounds::new(0, 0, 1920, 1080),
            visible_bounds: Bounds::new(0, 25, 1920, 1055),
            usable_insets: Default::default(),
            backing_scale_factor: 2.0,
            orientation: super::super::types::DisplayOrientation::Landscape,
            is_primary: true,
            legacy_order: 0,
            topology_generation: 1,
        };
        let error = resolve_layout_target(&LayoutTarget::Restore, &display, GeometryMode::ExactV2)
            .expect_err("restore unsupported");
        assert!(error.to_string().contains("unsupported in foundation wave"));
    }
}
