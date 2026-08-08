use serde::{Deserialize, Serialize};

// ============================================================
// DEBUG GRID OVERLAY
// ============================================================

/// Options for the debug grid overlay
///
/// Used with ShowGrid message to configure the visual debugging overlay
/// that displays grid lines, component bounds, and alignment guides.
///
/// # Note on Default
/// The `Default` implementation manually matches the serde defaults to ensure
/// consistency between `GridOptions::default()` (Rust code) and deserialized
/// defaults (from JSON with missing fields).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GridOptions {
    /// Grid line spacing in pixels (8 or 16)
    #[serde(default = "default_grid_size")]
    pub grid_size: u32,

    /// Show component bounding boxes with labels
    #[serde(default)]
    pub show_bounds: bool,

    /// Show CSS box model (padding/margin) visualization
    #[serde(default)]
    pub show_box_model: bool,

    /// Show alignment guides between components
    #[serde(default)]
    pub show_alignment_guides: bool,

    /// Show component dimensions in labels (e.g., "Header (500x45)")
    #[serde(default)]
    pub show_dimensions: bool,

    /// Which components to show bounds for
    /// - "prompts": Top-level prompts only
    /// - "all": All rendered elements
    /// - ["name1", "name2"]: Specific component names
    #[serde(default)]
    pub depth: GridDepthOption,

    /// Optional custom color scheme
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<GridColorScheme>,
}

fn default_grid_size() -> u32 {
    8
}

/// Manual Default implementation to match serde defaults exactly.
/// This ensures GridOptions::default() produces the same values as
/// deserializing an empty JSON object {}.
impl Default for GridOptions {
    fn default() -> Self {
        Self {
            grid_size: default_grid_size(), // 8, not 0
            show_bounds: false,
            show_box_model: false,
            show_alignment_guides: false,
            show_dimensions: false,
            depth: GridDepthOption::default(),
            color_scheme: None,
        }
    }
}

/// Depth option for grid bounds display
///
/// Controls which components have their bounding boxes shown.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GridDepthOption {
    /// Preset mode: "prompts" or "all"
    Preset(String),
    /// Specific component names to show bounds for
    Components(Vec<String>),
}

impl Default for GridDepthOption {
    fn default() -> Self {
        GridDepthOption::Preset("prompts".to_string())
    }
}

/// Custom color scheme for the debug grid overlay
///
/// All colors are in "#RRGGBBAA" or "#RRGGBB" hex format.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GridColorScheme {
    /// Color for grid lines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_lines: Option<String>,

    /// Color for prompt bounding boxes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_bounds: Option<String>,

    /// Color for input bounding boxes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_bounds: Option<String>,

    /// Color for button bounding boxes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button_bounds: Option<String>,

    /// Color for list bounding boxes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_bounds: Option<String>,

    /// Fill color for padding visualization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_fill: Option<String>,

    /// Fill color for margin visualization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_fill: Option<String>,

    /// Color for alignment guide lines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment_guide: Option<String>,
}

// ============================================================
// ERROR DATA
// ============================================================

/// Script error data for structured error reporting
///
/// Sent when a script execution fails, providing detailed error information
/// for display in the UI with actionable suggestions.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptErrorData {
    /// User-friendly error message
    pub error_message: String,
    /// Raw stderr output if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_output: Option<String>,
    /// Process exit code if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Parsed stack trace if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<String>,
    /// Path to the script that failed
    pub script_path: String,
    /// Actionable fix suggestions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
    /// When the error occurred (ISO 8601 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

impl ScriptErrorData {
    /// Create a new ScriptErrorData with required fields
    pub fn new(error_message: String, script_path: String) -> Self {
        ScriptErrorData {
            error_message,
            stderr_output: None,
            exit_code: None,
            stack_trace: None,
            script_path,
            suggestions: Vec::new(),
            timestamp: None,
        }
    }

    /// Add stderr output
    pub fn with_stderr(mut self, stderr: String) -> Self {
        self.stderr_output = Some(stderr);
        self
    }

    /// Add exit code
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    /// Add stack trace
    pub fn with_stack_trace(mut self, trace: String) -> Self {
        self.stack_trace = Some(trace);
        self
    }

    /// Add suggestions
    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = suggestions;
        self
    }

    /// Add a single suggestion
    pub fn add_suggestion(mut self, suggestion: String) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Add timestamp
    pub fn with_timestamp(mut self, timestamp: String) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

// ============================================================
// LAYOUT INFO (AI Agent Debugging)
// ============================================================

/// Computed box model for a component (padding, margin, gap)
///
/// All values are in pixels. This provides the "why" behind spacing -
/// AI agents can understand if space comes from padding, margin, or gap.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComputedBoxModel {
    /// Padding values (inner spacing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<BoxModelSides>,
    /// Margin values (outer spacing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<BoxModelSides>,
    /// Gap between flex/grid children
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<f32>,
}

/// Box model sides (top, right, bottom, left)
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BoxModelSides {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl BoxModelSides {
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// Computed flex properties for a component
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComputedFlexStyle {
    /// Flex direction: "row" or "column"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Flex grow value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grow: Option<f32>,
    /// Flex shrink value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shrink: Option<f32>,
    /// Align items: "start", "center", "end", "stretch"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_items: Option<String>,
    /// Justify content: "start", "center", "end", "space-between", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justify_content: Option<String>,
}

/// Bounding rectangle in pixels
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct LayoutBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Rounded-corner radii in pixels for visual compliance receipts.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutCornerRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl LayoutCornerRadius {
    pub fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }
}

/// Visual/style metadata used by DevTools to audit Tahoe/Liquid Glass UI rules.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutVisualStyle {
    /// content, functionalChrome, navigationChrome, floatingTransient, or statusOnly.
    pub chrome_layer: String,
    /// none, solidThemeToken, NSVisualEffectView, NSGlassEffectView, GPUIRgba, or unknown.
    pub material_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<LayoutCornerRadius>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual_bounds: Option<LayoutBounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hit_bounds: Option<LayoutBounds>,
    /// Internal text/content inset (bezel/edge -> first glyph) for input-like
    /// controls. Lets the Apple-guideline conformance engine MEASURE search-field
    /// padding against the native NSTextField baseline (9pt H / 3pt V on macOS 26)
    /// instead of reporting it as an `unmeasured` gap. Serialized as `contentInsets`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_insets: Option<BoxModelSides>,
    /// Rendered text metrics for text-bearing nodes (e.g. the search input). Lets
    /// the Apple-guideline conformance engine classify font size/weight/line-height
    /// against the measured-native macOS baseline (13pt Regular / 16pt line height)
    /// instead of guessing. Emitted from the SAME accessors the renderer uses, so
    /// the receipt cannot drift from what is actually drawn. Serialized as `typography`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typography: Option<LayoutTypographyInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception: Option<String>,
}

/// Rendered typography metrics for a text-bearing layout node.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTypographyInfo {
    /// Semantic role: searchInput, resultPrimary, resultSecondary, resultMeta.
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size_pt: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight_numeric: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_height_pt: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
}

/// Component type for categorization
///
/// # Forward Compatibility
/// The `Unknown` variant with `#[serde(other)]` ensures forward compatibility:
/// if a newer protocol version adds new component types, older receivers
/// will deserialize them as `Unknown` instead of failing entirely.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LayoutComponentType {
    Prompt,
    Input,
    Button,
    List,
    /// `listItem` in the current protocol, with legacy `listitem` support.
    #[serde(alias = "listitem")]
    ListItem,
    Header,
    #[default]
    Container,
    Panel,
    Other,
    /// Unknown component type (forward compatibility fallback)
    /// When deserializing, any unrecognized type string becomes Unknown
    #[serde(other)]
    Unknown,
}

/// Semantic geometry roles used to decide whether two measurements are comparable.
///
/// Related roles may have containment or spacing contracts without sharing one
/// numeric rectangle. DevTools must never join unlike roles merely because their
/// names look similar.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum GeometryRole {
    WindowBackdrop,
    MainHeaderChrome,
    ContextZone,
    InputControl,
    ContentViewport,
    RowSlot,
    SectionSlot,
    FooterNativeHost,
    FooterRail,
    FooterActionRow,
    FooterActionSlot,
    KeycapInnerFrame,
    PopupShell,
    PopupAnchor,
    TextLineBox,
    GlyphBounds,
    FocusRing,
    #[default]
    Other,
}

fn canonical_measurement_slug(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut previous_was_separator = true;
    let mut previous_was_lower_or_digit = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase()
                && previous_was_lower_or_digit
                && !previous_was_separator
            {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            previous_was_separator = false;
            previous_was_lower_or_digit =
                character.is_ascii_lowercase() || character.is_ascii_digit();
        } else {
            if !previous_was_separator && !slug.is_empty() {
                slug.push('-');
            }
            previous_was_separator = true;
            previous_was_lower_or_digit = false;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn geometry_role_for_slug(slug: &str) -> GeometryRole {
    if slug == "window" || slug.ends_with("window-backdrop") {
        GeometryRole::WindowBackdrop
    } else if slug.contains("main-view-header") || slug == "main-header" {
        GeometryRole::MainHeaderChrome
    } else if slug.contains("context-zone") {
        GeometryRole::ContextZone
    } else if slug.contains("input") || slug.contains("editor") || slug.contains("composer") {
        GeometryRole::InputControl
    } else if slug.contains("footer-native-host") || slug == "native-main-window-footer-spacer" {
        GeometryRole::FooterNativeHost
    } else if slug.contains("footer-rail") {
        GeometryRole::FooterRail
    } else if slug.contains("footer-action-row") {
        GeometryRole::FooterActionRow
    } else if slug.contains("footer-action-slot") || slug.contains("footer-button") {
        GeometryRole::FooterActionSlot
    } else if slug.contains("keycap") && slug.contains("glyph") {
        GeometryRole::KeycapInnerFrame
    } else if slug.contains("popup-anchor") {
        GeometryRole::PopupAnchor
    } else if slug.contains("popup") || slug.contains("panel") || slug.contains("dialog") {
        GeometryRole::PopupShell
    } else if slug.contains("section") || slug.contains("separator") {
        GeometryRole::SectionSlot
    } else if slug.contains("line-box") || slug.contains("text-line") || slug.contains("-line-") {
        GeometryRole::TextLineBox
    } else if slug.contains("glyph") || slug.contains("text-run") {
        GeometryRole::GlyphBounds
    } else if slug.contains("focus-ring") {
        GeometryRole::FocusRing
    } else if slug.contains("row") || slug.contains("list-item") {
        GeometryRole::RowSlot
    } else if slug.contains("viewport")
        || slug.contains("content")
        || slug.contains("main-view-main")
        || slug.contains("script-list")
    {
        GeometryRole::ContentViewport
    } else {
        GeometryRole::Other
    }
}

/// Information about a single component in the layout tree
///
/// This is the core data structure for `getLayoutInfo()`.
/// It provides everything an AI agent needs to understand "why"
/// a component is positioned/sized the way it is.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutComponentInfo {
    /// Component name/identifier
    pub name: String,
    /// Component type for categorization
    #[serde(rename = "type")]
    pub component_type: LayoutComponentType,
    /// Optional semantic peer when this geometry belongs to an interactive node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_id: Option<String>,
    /// Stable cross-layer identity used to join intended, model, and rendered facts.
    #[serde(default)]
    pub measurement_id: String,
    /// Typed role that must match before two rectangles are compared.
    #[serde(default)]
    pub geometry_role: GeometryRole,
    /// Bounding rectangle (absolute position and size)
    pub bounds: LayoutBounds,
    /// Computed box model (padding, margin, gap)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_model: Option<ComputedBoxModel>,
    /// Computed flex properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex: Option<ComputedFlexStyle>,
    /// Nesting depth (0 = root, 1 = child of root, etc.)
    pub depth: u32,
    /// Parent component name (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Child component names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    /// Human-readable explanation of why this component has its current size/position
    /// Example: "Height is 45px = padding(8) + content(28) + padding(8) + divider(1)"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    /// Optional visual/style receipt for Liquid Glass and accessibility audits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual_style: Option<LayoutVisualStyle>,
    /// How the bounds were obtained. Fidelity probes require `paint-time`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_provenance: Option<String>,
    /// Coordinate system for `bounds`; paint receipts use window logical pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinate_space: Option<String>,
    /// Rectangular portion of `bounds` that survives the ancestor clip chain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_bounds: Option<LayoutBounds>,
    /// Active ancestor content mask used to derive `visible_bounds`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_bounds: Option<LayoutBounds>,
    /// Monotonic identity of the completed GPUI frame that supplied these bounds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_frame_generation: Option<u64>,
}

/// One capture-only GPUI fidelity scope summarized from an actual completed
/// paint frame. The manifest pins identity and hierarchy; this runtime record
/// supplies current geometry and ordered primitive evidence.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FidelityLayoutNode {
    /// Stable closed-world identity for the paint-owning node.
    pub id: String,
    /// Semantic node category such as `element`, `textRun`, or `scrollbar`.
    pub kind: String,
    /// Stable ID of the nearest enclosing fidelity node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Full node bounds in window-relative logical pixels.
    pub bounds: LayoutBounds,
    /// Full bounds clipped by all visible ancestors.
    pub visible_bounds: LayoutBounds,
    /// Active ancestor clip rectangle.
    pub clip_bounds: LayoutBounds,
    /// Union of all raster primitives owned by this node.
    pub union_paint_bounds: LayoutBounds,
    /// Number of raster primitives owned by this node.
    pub primitive_count: usize,
    /// Deterministic ordered digest of the owned raster primitives.
    pub primitive_digest: String,
    /// Stable closed-world order among visible fidelity nodes.
    pub paint_order: u64,
    /// First raster primitive order owned by this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_paint_order: Option<u64>,
    /// Last raster primitive order owned by this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_paint_order: Option<u64>,
    /// Completed GPUI frame that supplied every measurement on this node.
    pub measurement_frame_generation: u64,
    /// Explicit evidence that the geometry came from GPUI's completed paint frame.
    pub measurement_provenance: String,
    /// Coordinate space used by every bounds field on this node.
    pub coordinate_space: String,
    /// Optional SHA-256 hash of exact UTF-8 source bytes for text-bearing nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_hash: Option<String>,
    /// Optional digest of text shaping and layout metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_layout_hash: Option<String>,
    /// Node-kind-specific finite telemetry such as scrollbar prepaint values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Paint calls that occurred outside any stable fidelity scope. The Agent
/// Chat proof fails closed when non-background atoms remain here.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FidelityUnscopedPaintSummary {
    /// Number of primitives emitted outside a stable fidelity scope.
    pub primitive_count: usize,
    /// Deterministic ordered digest of unscoped primitives.
    pub primitive_digest: String,
    /// Distinct primitive entry points present in the unscoped bucket.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primitive_kinds: Vec<String>,
    /// Union of all unscoped primitive bounds.
    pub union_paint_bounds: LayoutBounds,
}

/// Finite RGBA evidence extracted from an AppKit view or backing layer.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppKitFidelityColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

/// Capture-only text evidence for an identified AppKit text field.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppKitFidelityText {
    pub value: String,
    pub value_sha256: String,
    pub font_name: String,
    pub font_size: f64,
    pub font_weight: i64,
    pub alignment: i64,
    pub fitting_size: LayoutBounds,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<AppKitFidelityColor>,
}

/// Capture-only image evidence for an identified AppKit image view.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppKitFidelityImage {
    pub width: f64,
    pub height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tint: Option<AppKitFidelityColor>,
}

/// Backing-layer evidence for an identified AppKit view.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppKitFidelityLayer {
    pub contents_scale: f64,
    pub masks_to_bounds: bool,
    pub border_width: f64,
    pub corner_radius: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<AppKitFidelityColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<AppKitFidelityColor>,
    #[serde(default)]
    pub shadow_opacity: f64,
    #[serde(default)]
    pub shadow_radius: f64,
    #[serde(default)]
    pub shadow_offset_x: f64,
    #[serde(default)]
    pub shadow_offset_y: f64,
    #[serde(default)]
    pub has_shadow_path: bool,
}

/// One identified AppKit view in the main-window footer host tree.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppKitFidelityNode {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub class_name: String,
    pub subview_order: usize,
    /// Frame in the immediate superview's AppKit coordinate space.
    pub frame: LayoutBounds,
    /// Bounds in the view's local AppKit coordinate space.
    pub bounds: LayoutBounds,
    /// Bounds converted into the main content view's bottom-left coordinate space.
    pub window_frame: LayoutBounds,
    /// Bounds converted into the captured PNG's top-left logical coordinate space.
    pub screenshot_frame: LayoutBounds,
    pub hidden: bool,
    pub alpha: f64,
    pub clips_to_bounds: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<AppKitFidelityLayer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<AppKitFidelityText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<AppKitFidelityImage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_selector: Option<String>,
    /// Semantic peer installed as the NSAccessibility identifier. Structural
    /// AppKit `id` remains separate for paint/styling ownership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility_role: Option<String>,
    /// Product-static label evidence is fingerprinted so this projection never
    /// establishes a general raw AX text escape hatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility_label_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility_label_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility_focused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility_element: Option<bool>,
}

/// Runtime allocation selected for the constrained leading footer lane.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppKitFooterLeftAllocation {
    pub degradation: String,
    pub available_width: f64,
    pub cwd_label_width: f64,
    pub primary_label_width: f64,
}

/// Main-window AppKit footer-host evidence. This never includes the separate
/// GPUI footer overlay child window.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppKitFidelitySnapshot {
    pub target_id: String,
    pub target_kind: String,
    pub coordinate_space: String,
    pub window_bounds: LayoutBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_backdrop_frame: Option<LayoutBounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer_container_frame: Option<LayoutBounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent_gap_points: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop_footer_intersection_area: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outer_window_has_shadow: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_backdrop_layer: Option<AppKitFidelityLayer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer_left_allocation: Option<AppKitFooterLeftAllocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_bearing_view_ids: Vec<String>,
    pub nodes: Vec<AppKitFidelityNode>,
}

/// Fidelity telemetry for a separate GPUI paint target such as the footer
/// overlay child window. Keeping this distinct prevents main-window receipts
/// from claiming that overlay glyphs are part of the main raster.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FidelityPaintTargetSnapshot {
    pub target_id: String,
    pub target_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_target_id: Option<String>,
    pub window_bounds: LayoutBounds,
    pub frame_generation: u64,
    pub nodes: Vec<FidelityLayoutNode>,
    pub unscoped: FidelityUnscopedPaintSummary,
}

/// Fail-closed result for an auxiliary fidelity capture plane.
///
/// Legacy receipts deserialize as `notRequested`; a current Agent Chat
/// capture must report either `captured` or a concrete blocking reason.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FidelityCaptureStatus {
    #[default]
    NotRequested,
    Captured,
    MissingWindow,
    MissingContentView,
    MissingFooterHost,
    EmptyInventory,
    DuplicateIdentifiers,
    UnsupportedPlatform,
    MissingOverlay,
}

/// Capture-only finite paint inventory for one completed GPUI frame.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FidelityLayoutSnapshot {
    /// Bounded capture target that enabled this snapshot.
    pub capture_target: String,
    /// Completed GPUI frame shared by every node in this snapshot.
    pub frame_generation: u64,
    /// Closed-world visible fidelity nodes.
    pub nodes: Vec<FidelityLayoutNode>,
    /// Paint primitives not owned by any stable node.
    pub unscoped: FidelityUnscopedPaintSummary,
    /// Explicit result for the native main-window footer-host capture.
    #[serde(default, rename = "appKitStatus", alias = "appkitStatus")]
    pub appkit_status: FidelityCaptureStatus,
    /// Native main-window footer-host evidence, collected from AppKit.
    #[serde(
        default,
        rename = "appKit",
        alias = "appkit",
        skip_serializing_if = "Option::is_none"
    )]
    pub appkit: Option<AppKitFidelitySnapshot>,
    /// Explicit result for the separate GPUI footer-overlay capture.
    #[serde(default)]
    pub overlay_status: FidelityCaptureStatus,
    /// Separate GPUI overlay-window inventories; never composited into the
    /// main-window target implicitly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlays: Vec<FidelityPaintTargetSnapshot>,
}

impl LayoutComponentInfo {
    pub fn new(name: impl Into<String>, component_type: LayoutComponentType) -> Self {
        let name = name.into();
        let measurement_slug = canonical_measurement_slug(&name);
        Self {
            name,
            component_type,
            semantic_id: None,
            measurement_id: format!("layout:{measurement_slug}"),
            geometry_role: geometry_role_for_slug(&measurement_slug),
            bounds: LayoutBounds::default(),
            box_model: None,
            flex: None,
            depth: 0,
            parent: None,
            children: Vec::new(),
            explanation: None,
            visual_style: None,
            measurement_provenance: Some("model".to_string()),
            coordinate_space: Some("window".to_string()),
            visible_bounds: None,
            clip_bounds: None,
            measurement_frame_generation: None,
        }
    }

    pub fn with_geometry_identity(
        mut self,
        measurement_id: impl Into<String>,
        semantic_id: Option<String>,
        role: GeometryRole,
    ) -> Self {
        self.measurement_id = measurement_id.into();
        self.semantic_id = semantic_id;
        self.geometry_role = role;
        self
    }

    pub fn with_measurement(
        mut self,
        provenance: impl Into<String>,
        coordinate_space: impl Into<String>,
    ) -> Self {
        self.measurement_provenance = Some(provenance.into());
        self.coordinate_space = Some(coordinate_space.into());
        self
    }

    pub fn with_measurement_frame(mut self, generation: u64) -> Self {
        self.measurement_frame_generation = Some(generation);
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_paint_visibility(
        mut self,
        visible_x: f32,
        visible_y: f32,
        visible_width: f32,
        visible_height: f32,
        clip_x: f32,
        clip_y: f32,
        clip_width: f32,
        clip_height: f32,
    ) -> Self {
        self.visible_bounds = Some(LayoutBounds {
            x: visible_x,
            y: visible_y,
            width: visible_width,
            height: visible_height,
        });
        self.clip_bounds = Some(LayoutBounds {
            x: clip_x,
            y: clip_y,
            width: clip_width,
            height: clip_height,
        });
        self
    }

    pub fn with_bounds(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.bounds = LayoutBounds {
            x,
            y,
            width,
            height,
        };
        self
    }

    pub fn with_padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        let box_model = self.box_model.get_or_insert_with(ComputedBoxModel::default);
        box_model.padding = Some(BoxModelSides {
            top,
            right,
            bottom,
            left,
        });
        self
    }

    pub fn with_margin(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        let box_model = self.box_model.get_or_insert_with(ComputedBoxModel::default);
        box_model.margin = Some(BoxModelSides {
            top,
            right,
            bottom,
            left,
        });
        self
    }

    pub fn with_gap(mut self, gap: f32) -> Self {
        let box_model = self.box_model.get_or_insert_with(ComputedBoxModel::default);
        box_model.gap = Some(gap);
        self
    }

    pub fn with_flex_column(mut self) -> Self {
        let flex = self.flex.get_or_insert_with(ComputedFlexStyle::default);
        flex.direction = Some("column".to_string());
        self
    }

    pub fn with_flex_row(mut self) -> Self {
        let flex = self.flex.get_or_insert_with(ComputedFlexStyle::default);
        flex.direction = Some("row".to_string());
        self
    }

    pub fn with_flex_grow(mut self, grow: f32) -> Self {
        let flex = self.flex.get_or_insert_with(ComputedFlexStyle::default);
        flex.grow = Some(grow);
        self
    }

    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }

    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    pub fn with_visual_style(
        mut self,
        chrome_layer: impl Into<String>,
        material_source: impl Into<String>,
        corner_radius: Option<f32>,
    ) -> Self {
        self.visual_style = Some(LayoutVisualStyle {
            chrome_layer: chrome_layer.into(),
            material_source: material_source.into(),
            token_source: None,
            corner_radius: corner_radius.map(LayoutCornerRadius::uniform),
            visual_bounds: Some(self.bounds.clone()),
            hit_bounds: Some(self.bounds.clone()),
            content_insets: None,
            typography: None,
            exception: None,
        });
        self
    }

    /// Declare the rendered typography for a text-bearing node so the Apple-guideline
    /// conformance engine can classify it against the measured-native baseline.
    #[allow(clippy::too_many_arguments)]
    pub fn with_typography(
        mut self,
        role: impl Into<String>,
        font_family: Option<String>,
        font_size_pt: f32,
        font_weight: impl Into<String>,
        font_weight_numeric: f32,
        line_height_pt: f32,
        text_align: impl Into<String>,
    ) -> Self {
        let style = self
            .visual_style
            .get_or_insert_with(LayoutVisualStyle::default);
        style.typography = Some(LayoutTypographyInfo {
            role: role.into(),
            font_family,
            font_size_pt: Some(font_size_pt),
            font_weight: Some(font_weight.into()),
            font_weight_numeric: Some(font_weight_numeric),
            line_height_pt: Some(line_height_pt),
            text_align: Some(text_align.into()),
        });
        self
    }

    /// Declare the internal content inset (edge -> first glyph) for an input-like
    /// control so the Apple-guideline conformance engine can measure its padding.
    pub fn with_content_insets(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        let style = self
            .visual_style
            .get_or_insert_with(LayoutVisualStyle::default);
        style.content_insets = Some(BoxModelSides {
            top,
            right,
            bottom,
            left,
        });
        self
    }

    pub fn with_visual_token(mut self, token_source: impl Into<String>) -> Self {
        let style = self
            .visual_style
            .get_or_insert_with(LayoutVisualStyle::default);
        style.token_source = Some(token_source.into());
        self
    }

    pub fn with_hit_bounds(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        let style = self
            .visual_style
            .get_or_insert_with(LayoutVisualStyle::default);
        style.hit_bounds = Some(LayoutBounds {
            x,
            y,
            width,
            height,
        });
        self
    }

    pub fn with_visual_exception(mut self, exception: impl Into<String>) -> Self {
        let style = self
            .visual_style
            .get_or_insert_with(LayoutVisualStyle::default);
        style.exception = Some(exception.into());
        self
    }
}

/// Full layout information for the current UI state
///
/// Returned by `getLayoutInfo()` SDK function.
/// Contains the component tree and window-level information.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutInfo {
    /// Window dimensions
    pub window_width: f32,
    pub window_height: f32,
    /// Current prompt type (e.g., "arg", "div", "editor", "mainMenu")
    pub prompt_type: String,
    /// All components in the layout tree
    pub components: Vec<LayoutComponentInfo>,
    /// Present only when SCRIPT_KIT_FIDELITY_CAPTURE selects a bounded proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fidelity: Option<FidelityLayoutSnapshot>,
    /// Handler-form-specific layout details for DevTools focus/scroll receipts.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "handlerForm"
    )]
    pub handler_form: Option<serde_json::Value>,
    /// Timestamp when layout was captured (ISO 8601)
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::{
        AppKitFidelityNode, AppKitFidelitySnapshot, FidelityCaptureStatus, FidelityLayoutNode,
        FidelityLayoutSnapshot, FidelityPaintTargetSnapshot, FidelityUnscopedPaintSummary,
        GeometryRole, LayoutBounds, LayoutComponentInfo, LayoutComponentType, LayoutInfo,
    };

    #[test]
    fn model_and_paint_names_share_stable_measurement_identity() {
        let model = LayoutComponentInfo::new("MainViewHeader", LayoutComponentType::Header);
        let paint = LayoutComponentInfo::new("main-view-header", LayoutComponentType::Header)
            .with_measurement("paint-time", "window");

        assert_eq!(model.measurement_id, "layout:main-view-header");
        assert_eq!(paint.measurement_id, model.measurement_id);
        assert_eq!(model.geometry_role, GeometryRole::MainHeaderChrome);
        assert_eq!(paint.geometry_role, model.geometry_role);
        assert_eq!(model.measurement_provenance.as_deref(), Some("model"));
    }

    #[test]
    fn distinct_footer_roles_are_not_collapsed() {
        let host = LayoutComponentInfo::new(
            "native-main-window-footer-spacer",
            LayoutComponentType::Panel,
        );
        let action_row =
            LayoutComponentInfo::new("footer-action-row", LayoutComponentType::Container);
        assert_eq!(host.geometry_role, GeometryRole::FooterNativeHost);
        assert_eq!(action_row.geometry_role, GeometryRole::FooterActionRow);
        assert_ne!(host.geometry_role, action_row.geometry_role);
    }

    #[test]
    fn visual_style_serializes_for_devtools_receipts() {
        let component = LayoutComponentInfo::new("SearchInput", LayoutComponentType::Input)
            .with_bounds(16.0, 11.0, 534.0, 22.0)
            .with_visual_style("functionalChrome", "solidThemeToken", Some(14.0))
            .with_visual_token("chrome.searchInput")
            .with_hit_bounds(16.0, 8.0, 534.0, 28.0);

        let json = serde_json::to_value(component).expect("serialize visual style component");
        let style = json
            .get("visualStyle")
            .expect("visualStyle should be emitted");

        assert_eq!(style["chromeLayer"], "functionalChrome");
        assert_eq!(style["materialSource"], "solidThemeToken");
        assert_eq!(style["tokenSource"], "chrome.searchInput");
        assert_eq!(style["cornerRadius"]["topLeft"], 14.0);
        assert_eq!(style["visualBounds"]["height"], 22.0);
        assert_eq!(style["hitBounds"]["height"], 28.0);
    }

    #[test]
    fn visual_exception_marks_dense_controls() {
        let component = LayoutComponentInfo::new("LogoButton", LayoutComponentType::Button)
            .with_bounds(714.0, 8.0, 20.0, 28.0)
            .with_visual_style("functionalChrome", "solidThemeToken", Some(14.0))
            .with_hit_bounds(714.0, 8.0, 28.0, 28.0)
            .with_visual_exception("compactIconButton");

        let json = serde_json::to_value(component).expect("serialize visual exception");
        assert_eq!(json["visualStyle"]["exception"], "compactIconButton");
        assert_eq!(json["visualStyle"]["hitBounds"]["width"], 28.0);
    }

    #[test]
    fn paint_measurement_metadata_serializes_explicitly() {
        let component =
            LayoutComponentInfo::new("agent-chat-composer-input", LayoutComponentType::Input)
                .with_bounds(12.0, 34.0, 400.0, 28.0)
                .with_measurement("paint-time", "window")
                .with_paint_visibility(12.0, 40.0, 400.0, 22.0, 0.0, 40.0, 750.0, 404.0)
                .with_measurement_frame(42);

        let json = serde_json::to_value(component).expect("serialize paint measurement");
        assert_eq!(json["measurementProvenance"], "paint-time");
        assert_eq!(json["coordinateSpace"], "window");
        assert_eq!(json["measurementFrameGeneration"], 42);
        assert_eq!(json["visibleBounds"]["y"], 40.0);
        assert_eq!(json["clipBounds"]["height"], 404.0);
    }

    #[test]
    fn fidelity_snapshot_serializes_closed_world_paint_evidence() {
        let bounds = LayoutBounds {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        };
        let snapshot = FidelityLayoutSnapshot {
            capture_target: "agent-chat".to_string(),
            frame_generation: 42,
            nodes: vec![FidelityLayoutNode {
                id: "agent-chat.transcript.row.user-16".to_string(),
                kind: "element".to_string(),
                parent_id: Some("agent-chat.transcript.viewport".to_string()),
                bounds: bounds.clone(),
                visible_bounds: bounds.clone(),
                clip_bounds: bounds.clone(),
                union_paint_bounds: bounds.clone(),
                primitive_count: 3,
                primitive_digest: "paint-digest".to_string(),
                paint_order: 1,
                first_paint_order: Some(4),
                last_paint_order: Some(6),
                measurement_frame_generation: 42,
                measurement_provenance: "paint-time".to_string(),
                coordinate_space: "window".to_string(),
                text_hash: Some("text-digest".to_string()),
                text_layout_hash: None,
                metadata: None,
            }],
            unscoped: FidelityUnscopedPaintSummary {
                primitive_count: 0,
                primitive_digest: "empty-digest".to_string(),
                primitive_kinds: Vec::new(),
                union_paint_bounds: LayoutBounds::default(),
            },
            appkit_status: FidelityCaptureStatus::Captured,
            appkit: Some(AppKitFidelitySnapshot {
                target_id: "main-footer-host".to_string(),
                target_kind: "appKitFooterHost".to_string(),
                coordinate_space: "appkit-content-bottom-left+screenshot-top-left".to_string(),
                window_bounds: bounds.clone(),
                main_backdrop_frame: None,
                main_backdrop_layer: None,
                footer_left_allocation: None,
                footer_container_frame: None,
                transparent_gap_points: None,
                backdrop_footer_intersection_area: None,
                outer_window_has_shadow: None,
                material_bearing_view_ids: Vec::new(),
                nodes: vec![AppKitFidelityNode {
                    id: "script-kit-footer-effect".to_string(),
                    class_name: "NSVisualEffectView".to_string(),
                    screenshot_frame: bounds.clone(),
                    ..Default::default()
                }],
            }),
            overlay_status: FidelityCaptureStatus::Captured,
            overlays: vec![FidelityPaintTargetSnapshot {
                target_id: "gpui-footer-overlay".to_string(),
                target_kind: "footerOverlay".to_string(),
                parent_target_id: Some("main".to_string()),
                window_bounds: bounds.clone(),
                frame_generation: 42,
                nodes: Vec::new(),
                unscoped: FidelityUnscopedPaintSummary::default(),
            }],
        };

        let json = serde_json::to_value(snapshot).expect("serialize fidelity snapshot");
        assert_eq!(json["captureTarget"], "agent-chat");
        assert_eq!(json["frameGeneration"], 42);
        assert_eq!(
            json["nodes"][0]["parentId"],
            "agent-chat.transcript.viewport"
        );
        assert_eq!(json["nodes"][0]["primitiveCount"], 3);
        assert_eq!(json["nodes"][0]["measurementFrameGeneration"], 42);
        assert_eq!(json["nodes"][0]["measurementProvenance"], "paint-time");
        assert_eq!(json["nodes"][0]["coordinateSpace"], "window");
        assert_eq!(json["appKitStatus"], "captured");
        assert!(json.get("appkitStatus").is_none());
        assert_eq!(json["appKit"]["targetId"], "main-footer-host");
        assert_eq!(json["appKit"]["nodes"][0]["id"], "script-kit-footer-effect");
        assert!(json.get("appkit").is_none());
        assert_eq!(json["overlayStatus"], "captured");
        assert_eq!(json["overlays"][0]["targetId"], "gpui-footer-overlay");
        assert_eq!(json["overlays"][0]["parentTargetId"], "main");
        assert_eq!(json["unscoped"]["primitiveCount"], 0);
    }

    #[test]
    fn appkit_accessibility_peer_serializes_without_raw_label() {
        let node = AppKitFidelityNode {
            id: "script-kit-footer-button-run".to_string(),
            accessibility_identifier: Some("footer-action:run".to_string()),
            accessibility_role: Some("AXButton".to_string()),
            accessibility_label_sha256: Some("abc123".to_string()),
            accessibility_label_length: Some(3),
            accessibility_enabled: Some(true),
            accessibility_focused: Some(false),
            accessibility_element: Some(true),
            action_selector: Some("runFooterAction:".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_value(node).expect("serialize AppKit AX peer");
        assert_eq!(json["accessibilityIdentifier"], "footer-action:run");
        assert_eq!(json["accessibilityRole"], "AXButton");
        assert_eq!(json["accessibilityLabelSha256"], "abc123");
        assert_eq!(json["accessibilityLabelLength"], 3);
        assert!(json.get("accessibilityLabel").is_none());
    }

    #[test]
    fn fidelity_snapshot_defaults_auxiliary_status_for_legacy_receipts() {
        let legacy = serde_json::json!({
            "captureTarget": "agent-chat",
            "frameGeneration": 42,
            "nodes": [],
            "unscoped": {
                "primitiveCount": 0,
                "primitiveDigest": "empty",
                "primitiveKinds": [],
                "unionPaintBounds": {
                    "x": 0.0,
                    "y": 0.0,
                    "width": 0.0,
                    "height": 0.0
                }
            }
        });

        let decoded: FidelityLayoutSnapshot =
            serde_json::from_value(legacy).expect("deserialize pre-AppKit fidelity receipt");
        assert_eq!(decoded.appkit_status, FidelityCaptureStatus::NotRequested);
        assert_eq!(decoded.appkit, None);
        assert_eq!(decoded.overlay_status, FidelityCaptureStatus::NotRequested);
        assert!(decoded.overlays.is_empty());
    }

    #[test]
    fn fidelity_snapshot_accepts_lowercase_appkit_compatibility_keys() {
        let legacy = serde_json::json!({
            "captureTarget": "agent-chat",
            "frameGeneration": 42,
            "nodes": [],
            "unscoped": {
                "primitiveCount": 0,
                "primitiveDigest": "empty",
                "primitiveKinds": [],
                "unionPaintBounds": LayoutBounds::default()
            },
            "appkitStatus": "captured",
            "appkit": {
                "targetId": "main-footer-host",
                "targetKind": "appKitFooterHost",
                "coordinateSpace": "appkit-content-bottom-left+screenshot-top-left",
                "windowBounds": LayoutBounds::default(),
                "nodes": []
            }
        });

        let decoded: FidelityLayoutSnapshot = serde_json::from_value(legacy)
            .expect("deserialize lowercase AppKit compatibility keys");
        assert_eq!(decoded.appkit_status, FidelityCaptureStatus::Captured);
        assert_eq!(decoded.appkit.unwrap().target_id, "main-footer-host");
    }

    #[test]
    fn layout_info_omits_inactive_fidelity_and_accepts_legacy_receipts() {
        let json = serde_json::to_value(LayoutInfo::default()).expect("serialize default layout");
        assert!(json.get("fidelity").is_none());

        let legacy = serde_json::json!({
            "windowWidth": 750.0,
            "windowHeight": 480.0,
            "promptType": "agentChatChat",
            "components": [],
            "timestamp": "2026-07-11T00:00:00Z"
        });
        let decoded: LayoutInfo =
            serde_json::from_value(legacy).expect("deserialize layout without fidelity");
        assert_eq!(decoded.fidelity, None);
    }
}
