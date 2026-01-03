//! Debug Grid Overlay Module
//!
//! Provides a visual testing grid overlay for debugging layout and spacing issues.
//! This module renders grid lines, component bounds, box model visualization,
//! and alignment guides on top of the UI.
//!
//! # Usage
//! ```ignore
//! use debug_grid::{GridConfig, ComponentBounds, render_grid_overlay};
//!
//! let config = GridConfig::default();
//! let overlay = render_grid_overlay(&config, window_bounds, &component_bounds);
//! ```

#![allow(dead_code)]

use gpui::{
    div, px, rgba, Bounds, InteractiveElement, IntoElement, ParentElement, Pixels, Point, Size,
    Styled,
};

/// Configuration for the debug grid overlay
#[derive(Clone, Debug)]
pub struct GridConfig {
    /// Grid spacing in pixels (typically 8 or 16)
    pub grid_size: u32,
    /// Show component bounding boxes
    pub show_bounds: bool,
    /// Show padding/margin visualization (CSS DevTools style)
    pub show_box_model: bool,
    /// Show alignment snap lines between components
    pub show_alignment_guides: bool,
    /// Which components to show bounds for
    pub depth: GridDepth,
    /// Color scheme for the overlay
    pub color_scheme: GridColorScheme,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            grid_size: 8,
            show_bounds: true,
            show_box_model: false,
            show_alignment_guides: true,
            depth: GridDepth::Prompts,
            color_scheme: GridColorScheme::default(),
        }
    }
}

/// Controls which components to show in the overlay
#[derive(Clone, Debug, Default)]
pub enum GridDepth {
    /// Top-level prompts only (ArgPrompt, DivPrompt, etc.)
    #[default]
    Prompts,
    /// All rendered elements
    All,
    /// Specific named components
    Components(Vec<String>),
}

/// Box model measurements (padding/margin)
#[derive(Clone, Debug, Default)]
pub struct BoxModel {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl BoxModel {
    /// Create a uniform box model with the same value on all sides
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create a box model with horizontal and vertical values
    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// Type of component for color coding bounds
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ComponentType {
    /// ArgPrompt, DivPrompt, EditorPrompt, etc.
    Prompt,
    /// Text inputs, search box
    Input,
    /// Action buttons
    Button,
    /// Script list, choice list
    List,
    /// Individual list items
    ListItem,
    /// Headers, titles
    Header,
    /// Generic containers
    #[default]
    Container,
    /// Other unclassified components
    Other,
}

/// Bounds info for a component
#[derive(Clone, Debug)]
pub struct ComponentBounds {
    /// Component name/type for labeling
    pub name: String,
    /// Bounding rectangle
    pub bounds: Bounds<Pixels>,
    /// Type of component (for color coding)
    pub component_type: ComponentType,
    /// Padding values (inner spacing)
    pub padding: Option<BoxModel>,
    /// Margin values (outer spacing)
    pub margin: Option<BoxModel>,
}

impl ComponentBounds {
    /// Create a new ComponentBounds with just name and bounds
    pub fn new(name: impl Into<String>, bounds: Bounds<Pixels>) -> Self {
        Self {
            name: name.into(),
            bounds,
            component_type: ComponentType::Container,
            padding: None,
            margin: None,
        }
    }

    /// Set the component type
    pub fn with_type(mut self, component_type: ComponentType) -> Self {
        self.component_type = component_type;
        self
    }

    /// Set padding values
    pub fn with_padding(mut self, padding: BoxModel) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Set margin values
    pub fn with_margin(mut self, margin: BoxModel) -> Self {
        self.margin = Some(margin);
        self
    }
}

/// Color scheme for the debug grid overlay
///
/// All colors are in 0xRRGGBBAA format (RGBA with alpha)
#[derive(Clone, Debug)]
pub struct GridColorScheme {
    /// Grid line color (semi-transparent gray)
    pub grid_lines: u32,
    /// Prompt bounding box color
    pub prompt_bounds: u32,
    /// Input bounding box color
    pub input_bounds: u32,
    /// Button bounding box color
    pub button_bounds: u32,
    /// List bounding box color
    pub list_bounds: u32,
    /// List item bounding box color
    pub list_item_bounds: u32,
    /// Header bounding box color
    pub header_bounds: u32,
    /// Container bounding box color
    pub container_bounds: u32,
    /// Other component bounding box color
    pub other_bounds: u32,
    /// Padding visualization color (green-ish, semi-transparent)
    pub padding_fill: u32,
    /// Margin visualization color (orange-ish, semi-transparent)
    pub margin_fill: u32,
    /// Alignment guide color (hot pink)
    pub alignment_guide: u32,
    /// Label background color
    pub label_background: u32,
    /// Label text color
    pub label_text: u32,
}

impl Default for GridColorScheme {
    fn default() -> Self {
        Self {
            grid_lines: 0x80808040,       // Gray at 25% opacity
            prompt_bounds: 0xFF6B6BFF,    // Red (full opacity for border)
            input_bounds: 0x4ECDC4FF,     // Teal
            button_bounds: 0xFFE66DFF,    // Yellow
            list_bounds: 0x95E1D3FF,      // Mint
            list_item_bounds: 0xA8E6CFFF, // Light mint
            header_bounds: 0xDDA0DDFF,    // Plum
            container_bounds: 0x87CEEBFF, // Sky blue
            other_bounds: 0xD3D3D3FF,     // Light gray
            padding_fill: 0x98D8AA40,     // Green at 25% opacity
            margin_fill: 0xF7DC6F40,      // Orange at 25% opacity
            alignment_guide: 0xFF69B4AA,  // Hot pink at 66% opacity
            label_background: 0x000000CC, // Black at 80% opacity
            label_text: 0xFFFFFFFF,       // White
        }
    }
}

impl GridColorScheme {
    /// Get the bounds color for a given component type
    pub fn color_for_type(&self, component_type: &ComponentType) -> u32 {
        match component_type {
            ComponentType::Prompt => self.prompt_bounds,
            ComponentType::Input => self.input_bounds,
            ComponentType::Button => self.button_bounds,
            ComponentType::List => self.list_bounds,
            ComponentType::ListItem => self.list_item_bounds,
            ComponentType::Header => self.header_bounds,
            ComponentType::Container => self.container_bounds,
            ComponentType::Other => self.other_bounds,
        }
    }
}

/// Helper to extract f32 from Pixels
fn pixels_to_f32(p: Pixels) -> f32 {
    let val: f64 = p.into();
    val as f32
}

/// Render the complete grid overlay
///
/// This is the main entry point for rendering the debug overlay.
/// It combines grid lines, component bounds, box model visualization,
/// and alignment guides based on the configuration.
pub fn render_grid_overlay(
    config: &GridConfig,
    bounds: Bounds<Pixels>,
    components: &[ComponentBounds],
) -> impl IntoElement {
    let colors = &config.color_scheme;

    div()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .w(bounds.size.width)
        .h(bounds.size.height)
        // Pointer-events: none equivalent - overlay should not capture events
        .occlude()
        .children(
            // Grid lines
            Some(render_grid_lines(
                bounds,
                config.grid_size,
                colors.grid_lines,
            )),
        )
        .children(
            // Component bounds
            if config.show_bounds {
                Some(render_all_component_bounds(components, colors))
            } else {
                None
            },
        )
        .children(
            // Box model visualization
            if config.show_box_model {
                Some(render_all_box_models(components, colors))
            } else {
                None
            },
        )
        .children(
            // Alignment guides
            if config.show_alignment_guides {
                Some(render_alignment_guides(components, colors.alignment_guide))
            } else {
                None
            },
        )
}

/// Render grid lines at the specified interval
pub fn render_grid_lines(bounds: Bounds<Pixels>, grid_size: u32, color: u32) -> impl IntoElement {
    let grid_size_f = grid_size as f32;
    let width = pixels_to_f32(bounds.size.width);
    let height = pixels_to_f32(bounds.size.height);

    // Calculate number of lines
    let v_lines = (width / grid_size_f).ceil() as usize;
    let h_lines = (height / grid_size_f).ceil() as usize;

    div()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .w(bounds.size.width)
        .h(bounds.size.height)
        // Vertical lines
        .children((0..=v_lines).map(move |i| {
            let x = i as f32 * grid_size_f;
            div()
                .absolute()
                .left(px(x))
                .top(px(0.))
                .w(px(1.))
                .h(bounds.size.height)
                .bg(rgba(color))
        }))
        // Horizontal lines
        .children((0..=h_lines).map(move |i| {
            let y = i as f32 * grid_size_f;
            div()
                .absolute()
                .left(px(0.))
                .top(px(y))
                .w(bounds.size.width)
                .h(px(1.))
                .bg(rgba(color))
        }))
}

/// Render a single component's bounding box with label
pub fn render_component_bounds(
    component: &ComponentBounds,
    colors: &GridColorScheme,
) -> impl IntoElement {
    let color = colors.color_for_type(&component.component_type);
    let bounds = component.bounds;

    div()
        .absolute()
        .left(bounds.origin.x)
        .top(bounds.origin.y)
        .w(bounds.size.width)
        .h(bounds.size.height)
        .border_1()
        .border_color(rgba(color))
        // Label in top-left corner
        .child(
            div()
                .absolute()
                .left(px(0.))
                .top(px(0.))
                .px(px(4.))
                .py(px(2.))
                .bg(rgba(colors.label_background))
                .text_color(rgba(colors.label_text))
                .text_xs()
                .child(component.name.clone()),
        )
}

/// Render all component bounds
fn render_all_component_bounds(
    components: &[ComponentBounds],
    colors: &GridColorScheme,
) -> impl IntoElement {
    div().absolute().children(
        components
            .iter()
            .map(|c| render_component_bounds(c, colors)),
    )
}

/// Render box model visualization (padding and margin)
///
/// CSS DevTools style:
/// - Padding: green inner region
/// - Margin: orange outer region
pub fn render_box_model(component: &ComponentBounds, colors: &GridColorScheme) -> impl IntoElement {
    let bounds = component.bounds;

    div()
        .absolute()
        .children(
            // Render margin (outer region)
            component.margin.as_ref().map(|margin| {
                // Top margin
                let top = div()
                    .absolute()
                    .left(bounds.origin.x - px(margin.left))
                    .top(bounds.origin.y - px(margin.top))
                    .w(bounds.size.width + px(margin.left + margin.right))
                    .h(px(margin.top))
                    .bg(rgba(colors.margin_fill));

                // Bottom margin
                let bottom = div()
                    .absolute()
                    .left(bounds.origin.x - px(margin.left))
                    .top(bounds.origin.y + bounds.size.height)
                    .w(bounds.size.width + px(margin.left + margin.right))
                    .h(px(margin.bottom))
                    .bg(rgba(colors.margin_fill));

                // Left margin
                let left = div()
                    .absolute()
                    .left(bounds.origin.x - px(margin.left))
                    .top(bounds.origin.y)
                    .w(px(margin.left))
                    .h(bounds.size.height)
                    .bg(rgba(colors.margin_fill));

                // Right margin
                let right = div()
                    .absolute()
                    .left(bounds.origin.x + bounds.size.width)
                    .top(bounds.origin.y)
                    .w(px(margin.right))
                    .h(bounds.size.height)
                    .bg(rgba(colors.margin_fill));

                div()
                    .absolute()
                    .child(top)
                    .child(bottom)
                    .child(left)
                    .child(right)
            }),
        )
        .children(
            // Render padding (inner region)
            component.padding.as_ref().map(|padding| {
                // Top padding
                let top = div()
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y)
                    .w(bounds.size.width)
                    .h(px(padding.top))
                    .bg(rgba(colors.padding_fill));

                // Bottom padding
                let bottom = div()
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y + bounds.size.height - px(padding.bottom))
                    .w(bounds.size.width)
                    .h(px(padding.bottom))
                    .bg(rgba(colors.padding_fill));

                // Left padding
                let left = div()
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y + px(padding.top))
                    .w(px(padding.left))
                    .h(bounds.size.height - px(padding.top + padding.bottom))
                    .bg(rgba(colors.padding_fill));

                // Right padding
                let right = div()
                    .absolute()
                    .left(bounds.origin.x + bounds.size.width - px(padding.right))
                    .top(bounds.origin.y + px(padding.top))
                    .w(px(padding.right))
                    .h(bounds.size.height - px(padding.top + padding.bottom))
                    .bg(rgba(colors.padding_fill));

                div()
                    .absolute()
                    .child(top)
                    .child(bottom)
                    .child(left)
                    .child(right)
            }),
        )
}

/// Render all box models
fn render_all_box_models(
    components: &[ComponentBounds],
    colors: &GridColorScheme,
) -> impl IntoElement {
    div()
        .absolute()
        .children(components.iter().map(|c| render_box_model(c, colors)))
}

/// Alignment edge for detecting aligned components
#[derive(Debug, Clone, Copy, PartialEq)]
enum AlignmentEdge {
    Left(f32),
    Right(f32),
    Top(f32),
    Bottom(f32),
}

/// Render alignment guides between aligned components
///
/// Detects components that share the same x or y coordinates
/// and draws dashed lines connecting their aligned edges.
pub fn render_alignment_guides(components: &[ComponentBounds], color: u32) -> impl IntoElement {
    // Threshold for considering edges "aligned" (within N pixels)
    const ALIGNMENT_THRESHOLD: f32 = 2.0;

    let mut vertical_guides: Vec<f32> = Vec::new();
    let mut horizontal_guides: Vec<f32> = Vec::new();

    // Collect all edges
    let mut edges: Vec<AlignmentEdge> = Vec::new();
    for component in components {
        let b = &component.bounds;
        let x: f32 = pixels_to_f32(b.origin.x);
        let y: f32 = pixels_to_f32(b.origin.y);
        let w: f32 = pixels_to_f32(b.size.width);
        let h: f32 = pixels_to_f32(b.size.height);
        edges.push(AlignmentEdge::Left(x));
        edges.push(AlignmentEdge::Right(x + w));
        edges.push(AlignmentEdge::Top(y));
        edges.push(AlignmentEdge::Bottom(y + h));
    }

    // Find aligned vertical edges (left/right)
    for i in 0..edges.len() {
        for j in (i + 1)..edges.len() {
            let (val_i, val_j, is_vertical) = match (&edges[i], &edges[j]) {
                (AlignmentEdge::Left(a), AlignmentEdge::Left(b))
                | (AlignmentEdge::Right(a), AlignmentEdge::Right(b))
                | (AlignmentEdge::Left(a), AlignmentEdge::Right(b))
                | (AlignmentEdge::Right(a), AlignmentEdge::Left(b)) => (*a, *b, true),
                (AlignmentEdge::Top(a), AlignmentEdge::Top(b))
                | (AlignmentEdge::Bottom(a), AlignmentEdge::Bottom(b))
                | (AlignmentEdge::Top(a), AlignmentEdge::Bottom(b))
                | (AlignmentEdge::Bottom(a), AlignmentEdge::Top(b)) => (*a, *b, false),
                _ => continue,
            };

            if (val_i - val_j).abs() < ALIGNMENT_THRESHOLD {
                let avg = (val_i + val_j) / 2.0;
                if is_vertical {
                    if !vertical_guides
                        .iter()
                        .any(|&v| (v - avg).abs() < ALIGNMENT_THRESHOLD)
                    {
                        vertical_guides.push(avg);
                    }
                } else if !horizontal_guides
                    .iter()
                    .any(|&v| (v - avg).abs() < ALIGNMENT_THRESHOLD)
                {
                    horizontal_guides.push(avg);
                }
            }
        }
    }

    // Calculate overall bounds for guide lines
    let (min_x, max_x, min_y, max_y) = components.iter().fold(
        (f32::MAX, f32::MIN, f32::MAX, f32::MIN),
        |(min_x, max_x, min_y, max_y), c| {
            let b = &c.bounds;
            let x: f32 = pixels_to_f32(b.origin.x);
            let y: f32 = pixels_to_f32(b.origin.y);
            let w: f32 = pixels_to_f32(b.size.width);
            let h: f32 = pixels_to_f32(b.size.height);
            (
                min_x.min(x),
                max_x.max(x + w),
                min_y.min(y),
                max_y.max(y + h),
            )
        },
    );

    div()
        .absolute()
        // Vertical alignment guides
        .children(
            vertical_guides
                .into_iter()
                .map(move |x| render_dashed_line_vertical(x, min_y, max_y, color)),
        )
        // Horizontal alignment guides
        .children(
            horizontal_guides
                .into_iter()
                .map(move |y| render_dashed_line_horizontal(y, min_x, max_x, color)),
        )
}

/// Render a vertical dashed line
fn render_dashed_line_vertical(x: f32, y_start: f32, y_end: f32, color: u32) -> impl IntoElement {
    const DASH_LENGTH: f32 = 4.0;
    const GAP_LENGTH: f32 = 4.0;
    const LINE_WIDTH: f32 = 1.0;

    let total_length = y_end - y_start;
    let dash_count = (total_length / (DASH_LENGTH + GAP_LENGTH)).ceil() as usize;

    div().absolute().children((0..dash_count).map(move |i| {
        let y = y_start + (i as f32 * (DASH_LENGTH + GAP_LENGTH));
        let remaining = y_end - y;
        let dash_height = DASH_LENGTH.min(remaining);

        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(LINE_WIDTH))
            .h(px(dash_height))
            .bg(rgba(color))
    }))
}

/// Render a horizontal dashed line
fn render_dashed_line_horizontal(y: f32, x_start: f32, x_end: f32, color: u32) -> impl IntoElement {
    const DASH_LENGTH: f32 = 4.0;
    const GAP_LENGTH: f32 = 4.0;
    const LINE_WIDTH: f32 = 1.0;

    let total_length = x_end - x_start;
    let dash_count = (total_length / (DASH_LENGTH + GAP_LENGTH)).ceil() as usize;

    div().absolute().children((0..dash_count).map(move |i| {
        let x = x_start + (i as f32 * (DASH_LENGTH + GAP_LENGTH));
        let remaining = x_end - x;
        let dash_width = DASH_LENGTH.min(remaining);

        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(dash_width))
            .h(px(LINE_WIDTH))
            .bg(rgba(color))
    }))
}

/// Helper function to create bounds from origin and size
pub fn bounds_from_origin_size(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds {
        origin: Point { x: px(x), y: px(y) },
        size: Size {
            width: px(width),
            height: px(height),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_grid_config() {
        let config = GridConfig::default();
        assert_eq!(config.grid_size, 8);
        assert!(config.show_bounds);
        assert!(!config.show_box_model);
        assert!(config.show_alignment_guides);
    }

    #[test]
    fn test_default_color_scheme() {
        let colors = GridColorScheme::default();
        assert_eq!(colors.grid_lines, 0x80808040);
        assert_eq!(colors.prompt_bounds, 0xFF6B6BFF);
    }

    #[test]
    fn test_color_for_type() {
        let colors = GridColorScheme::default();
        assert_eq!(
            colors.color_for_type(&ComponentType::Prompt),
            colors.prompt_bounds
        );
        assert_eq!(
            colors.color_for_type(&ComponentType::Input),
            colors.input_bounds
        );
        assert_eq!(
            colors.color_for_type(&ComponentType::Button),
            colors.button_bounds
        );
    }

    #[test]
    fn test_box_model_uniform() {
        let bm = BoxModel::uniform(8.0);
        assert_eq!(bm.top, 8.0);
        assert_eq!(bm.right, 8.0);
        assert_eq!(bm.bottom, 8.0);
        assert_eq!(bm.left, 8.0);
    }

    #[test]
    fn test_box_model_symmetric() {
        let bm = BoxModel::symmetric(10.0, 20.0);
        assert_eq!(bm.top, 10.0);
        assert_eq!(bm.bottom, 10.0);
        assert_eq!(bm.left, 20.0);
        assert_eq!(bm.right, 20.0);
    }

    #[test]
    fn test_component_bounds_builder() {
        let bounds = bounds_from_origin_size(0.0, 0.0, 100.0, 50.0);
        let component = ComponentBounds::new("Test", bounds)
            .with_type(ComponentType::Button)
            .with_padding(BoxModel::uniform(8.0))
            .with_margin(BoxModel::symmetric(4.0, 8.0));

        assert_eq!(component.name, "Test");
        assert_eq!(component.component_type, ComponentType::Button);
        assert!(component.padding.is_some());
        assert!(component.margin.is_some());
    }

    #[test]
    fn test_bounds_from_origin_size() {
        let bounds = bounds_from_origin_size(10.0, 20.0, 100.0, 50.0);
        assert_eq!(pixels_to_f32(bounds.origin.x), 10.0);
        assert_eq!(pixels_to_f32(bounds.origin.y), 20.0);
        assert_eq!(pixels_to_f32(bounds.size.width), 100.0);
        assert_eq!(pixels_to_f32(bounds.size.height), 50.0);
    }

    #[test]
    fn test_pixels_to_f32() {
        assert_eq!(pixels_to_f32(px(10.0)), 10.0);
        assert_eq!(pixels_to_f32(px(0.0)), 0.0);
        assert_eq!(pixels_to_f32(px(100.5)), 100.5);
    }
}
