//! Exact integer geometry for window placement.
//!
//! `GeometryMode::LegacyV1` reproduces every historical tile/snap rectangle
//! byte-for-byte (integer division truncation and all); `ExactV2` computes
//! exact grid edges so odd dimensions are fully covered with no accumulated
//! one-pixel gaps. Every existing SDK, scriptlet, Window Switcher, and snap
//! route stays on LegacyV1 this wave; new plan APIs may use ExactV2.

use anyhow::Result;

use super::types::Bounds;

/// Which geometry rules to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryMode {
    /// Historical formulas (truncating integer division per cell).
    LegacyV1,
    /// Exact edge arithmetic (no gaps, no overlaps, full coverage).
    ExactV2,
}

/// Axis interpretation policy for directional presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisPolicy {
    /// Physical left/right/up/down (current behavior).
    Physical,
    /// Interpret the primary axis along the display's longest edge
    /// (adaptive portrait thirds; new profiles only).
    LongestDisplayEdge,
}

/// One cell (with span) in a uniform grid over a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCell {
    pub columns: u32,
    pub rows: u32,
    pub column: u32,
    pub row: u32,
    pub column_span: u32,
    pub row_span: u32,
}

impl GridCell {
    pub fn new(columns: u32, rows: u32, column: u32, row: u32) -> Self {
        Self {
            columns,
            rows,
            column,
            row,
            column_span: 1,
            row_span: 1,
        }
    }

    pub fn with_span(mut self, column_span: u32, row_span: u32) -> Self {
        self.column_span = column_span;
        self.row_span = row_span;
        self
    }
}

/// A frame normalized against a display's usable frame (0.0..=1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Anchor for pixel-sized placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// Exact grid edge: `origin + floor(length * index / divisions)`.
///
/// Computing BOTH edges of a cell independently guarantees adjacent cells
/// share an edge exactly and the final edge equals `origin + length`.
pub fn edge(origin: i32, length: u32, index: u32, divisions: u32) -> i32 {
    debug_assert!(divisions > 0);
    debug_assert!(index <= divisions);
    origin + ((u64::from(length) * u64::from(index)) / u64::from(divisions)) as i32
}

/// Exact bounds of one grid cell within a frame.
pub fn grid_bounds(frame: Bounds, cell: GridCell) -> Result<Bounds> {
    anyhow::ensure!(
        cell.columns > 0 && cell.rows > 0,
        "grid divisions must be nonzero"
    );
    anyhow::ensure!(
        cell.column_span > 0 && cell.row_span > 0,
        "grid spans must be nonzero"
    );
    anyhow::ensure!(
        cell.column + cell.column_span <= cell.columns,
        "grid column span exceeds frame"
    );
    anyhow::ensure!(
        cell.row + cell.row_span <= cell.rows,
        "grid row span exceeds frame"
    );
    let x0 = edge(frame.x, frame.width, cell.column, cell.columns);
    let x1 = edge(
        frame.x,
        frame.width,
        cell.column + cell.column_span,
        cell.columns,
    );
    let y0 = edge(frame.y, frame.height, cell.row, cell.rows);
    let y1 = edge(frame.y, frame.height, cell.row + cell.row_span, cell.rows);
    Ok(Bounds::new(
        x0,
        y0,
        u32::try_from(x1 - x0)?,
        u32::try_from(y1 - y0)?,
    ))
}

/// Normalize a frame against a usable display frame.
pub fn normalize(bounds: Bounds, frame: Bounds) -> NormalizedBounds {
    let width = frame.width.max(1) as f64;
    let height = frame.height.max(1) as f64;
    NormalizedBounds {
        x: (bounds.x - frame.x) as f64 / width,
        y: (bounds.y - frame.y) as f64 / height,
        width: bounds.width as f64 / width,
        height: bounds.height as f64 / height,
    }
}

/// Denormalize a frame against a usable display frame.
pub fn denormalize(normalized: NormalizedBounds, frame: Bounds) -> Bounds {
    let width = frame.width as f64;
    let height = frame.height as f64;
    Bounds::new(
        frame.x + (normalized.x * width).round() as i32,
        frame.y + (normalized.y * height).round() as i32,
        (normalized.width * width).round().max(0.0) as u32,
        (normalized.height * height).round().max(0.0) as u32,
    )
}

/// Resolve an anchored pixel size within a frame, clamped to the frame.
pub fn anchored_pixels(
    frame: Bounds,
    width: u32,
    height: u32,
    anchor: Anchor,
    offset_x: i32,
    offset_y: i32,
) -> Bounds {
    let width = width.min(frame.width);
    let height = height.min(frame.height);
    let slack_x = (frame.width - width) as i32;
    let slack_y = (frame.height - height) as i32;
    let (ax, ay) = match anchor {
        Anchor::TopLeft => (0, 0),
        Anchor::Top => (slack_x / 2, 0),
        Anchor::TopRight => (slack_x, 0),
        Anchor::Left => (0, slack_y / 2),
        Anchor::Center => (slack_x / 2, slack_y / 2),
        Anchor::Right => (slack_x, slack_y / 2),
        Anchor::BottomLeft => (0, slack_y),
        Anchor::Bottom => (slack_x / 2, slack_y),
        Anchor::BottomRight => (slack_x, slack_y),
    };
    let x = (frame.x + ax + offset_x).clamp(frame.x, frame.x + slack_x);
    let y = (frame.y + ay + offset_y).clamp(frame.y, frame.y + slack_y);
    Bounds::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic LCG so property tests never depend on ambient randomness.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
        fn range(&mut self, low: i64, high: i64) -> i64 {
            low + (self.next() % (high - low + 1) as u64) as i64
        }
    }

    fn random_frame(rng: &mut Lcg) -> Bounds {
        Bounds::new(
            rng.range(-4000, 4000) as i32,
            rng.range(-4000, 4000) as i32,
            rng.range(101, 5121) as u32,
            rng.range(101, 5121) as u32,
        )
    }

    #[test]
    fn ten_thousand_frames_have_exact_coverage_without_gaps_or_overlaps() {
        let mut rng = Lcg(0x5EED_CAFE);
        for _ in 0..10_000 {
            let frame = random_frame(&mut rng);
            let columns = rng.range(1, 6) as u32;
            let rows = rng.range(1, 4) as u32;

            let mut right_edges = Vec::new();
            for row in 0..rows {
                let mut x_cursor = frame.x;
                for column in 0..columns {
                    let cell = grid_bounds(frame, GridCell::new(columns, rows, column, row))
                        .expect("valid cell");
                    // Inside the frame.
                    assert!(cell.x >= frame.x && cell.y >= frame.y);
                    assert!(cell.x + cell.width as i32 <= frame.x + frame.width as i32);
                    assert!(cell.y + cell.height as i32 <= frame.y + frame.height as i32);
                    // No gap/overlap with the previous cell in this row.
                    assert_eq!(cell.x, x_cursor, "columns must tile exactly");
                    x_cursor = cell.x + cell.width as i32;
                }
                // Final right edge equals the frame edge exactly.
                assert_eq!(x_cursor, frame.x + frame.width as i32);
                right_edges.push(x_cursor);
            }
            // Bottom edge exactness for the last row.
            let last = grid_bounds(frame, GridCell::new(columns, rows, columns - 1, rows - 1))
                .expect("valid cell");
            assert_eq!(
                last.y + last.height as i32,
                frame.y + frame.height as i32,
                "final bottom edge must equal the frame exactly"
            );
        }
    }

    #[test]
    fn vertical_columns_tile_exactly_too() {
        let mut rng = Lcg(0xF00D);
        for _ in 0..2_000 {
            let frame = random_frame(&mut rng);
            let rows = rng.range(1, 6) as u32;
            let mut y_cursor = frame.y;
            for row in 0..rows {
                let cell = grid_bounds(frame, GridCell::new(1, rows, 0, row)).expect("valid cell");
                assert_eq!(cell.y, y_cursor);
                y_cursor = cell.y + cell.height as i32;
            }
            assert_eq!(y_cursor, frame.y + frame.height as i32);
        }
    }

    #[test]
    fn spans_cover_the_same_area_as_their_member_cells() {
        let frame = Bounds::new(-1921, 37, 3843, 2163); // odd everything
        let spanned = grid_bounds(frame, GridCell::new(3, 1, 0, 0).with_span(2, 1)).unwrap();
        let first = grid_bounds(frame, GridCell::new(3, 1, 0, 0)).unwrap();
        let second = grid_bounds(frame, GridCell::new(3, 1, 1, 0)).unwrap();
        assert_eq!(spanned.x, first.x);
        assert_eq!(
            spanned.x + spanned.width as i32,
            second.x + second.width as i32
        );
    }

    #[test]
    fn invalid_grids_are_rejected() {
        let frame = Bounds::new(0, 0, 100, 100);
        assert!(grid_bounds(frame, GridCell::new(0, 1, 0, 0)).is_err());
        assert!(grid_bounds(frame, GridCell::new(2, 1, 2, 0)).is_err());
        assert!(grid_bounds(frame, GridCell::new(2, 1, 0, 0).with_span(3, 1)).is_err());
        assert!(grid_bounds(frame, GridCell::new(2, 2, 0, 0).with_span(0, 1)).is_err());
    }

    #[test]
    fn normalized_round_trip_error_is_at_most_one_point() {
        let mut rng = Lcg(0xBEEF);
        for _ in 0..10_000 {
            let frame = random_frame(&mut rng);
            let bounds = Bounds::new(
                frame.x + rng.range(0, frame.width as i64 / 2) as i32,
                frame.y + rng.range(0, frame.height as i64 / 2) as i32,
                rng.range(50, (frame.width / 2).max(51) as i64) as u32,
                rng.range(50, (frame.height / 2).max(51) as i64) as u32,
            );
            let round_tripped = denormalize(normalize(bounds, frame), frame);
            assert!((round_tripped.x - bounds.x).abs() <= 1);
            assert!((round_tripped.y - bounds.y).abs() <= 1);
            assert!((round_tripped.width as i32 - bounds.width as i32).abs() <= 1);
            assert!((round_tripped.height as i32 - bounds.height as i32).abs() <= 1);
        }
    }

    #[test]
    fn anchored_pixels_clamp_to_the_frame_and_respect_anchors() {
        let frame = Bounds::new(100, 50, 1000, 800);
        let centered = anchored_pixels(frame, 400, 300, Anchor::Center, 0, 0);
        assert_eq!(centered, Bounds::new(400, 300, 400, 300));

        let oversize = anchored_pixels(frame, 5000, 5000, Anchor::Center, 0, 0);
        assert_eq!(oversize, Bounds::new(100, 50, 1000, 800));

        let bottom_right = anchored_pixels(frame, 200, 100, Anchor::BottomRight, 0, 0);
        assert_eq!(bottom_right, Bounds::new(900, 750, 200, 100));

        // Offsets never push the frame outside the display.
        let pushed = anchored_pixels(frame, 200, 100, Anchor::TopLeft, -500, -500);
        assert_eq!(pushed.x, 100);
        assert_eq!(pushed.y, 50);
    }
}
