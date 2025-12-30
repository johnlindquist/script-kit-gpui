//! Shared ListItem component for script list and arg prompt choice list
//!
//! This module provides a reusable, theme-aware list item component that can be
//! used in both the main script list and arg prompt choice lists.

#![allow(dead_code)]

use crate::designs::icon_variations::{icon_name_from_str, IconName};
use crate::logging;
use gpui::*;
use std::sync::Arc;

/// Icon type for list items - supports emoji strings, SVG icons, and pre-decoded images
#[derive(Clone)]
pub enum IconKind {
    /// Text/emoji icon (e.g., "📜", "⚡")
    Emoji(String),
    /// Pre-decoded render image (for app icons) - MUST be pre-decoded, not raw PNG bytes
    Image(Arc<RenderImage>),
    /// SVG icon by name (e.g., "File", "Terminal", "Code")
    /// Maps to IconName from designs::icon_variations
    Svg(String),
}

/// Fixed height for list items (same as main script list)
/// Height must accommodate: name (18px line-height) + description (14px line-height) + padding (12px)
/// Total content height: 18 + 14 + 12 = 44px minimum, using 48px for comfortable spacing
pub const LIST_ITEM_HEIGHT: f32 = 48.0;

/// Fixed height for section headers (RECENT, MAIN, etc.)
/// Total height includes: pt(8px) + text (~8px via text_xs) + pb(4px) = ~20px content
/// Using 24px for comfortable spacing while maintaining visual compactness.
///
/// ## Performance Note (uniform_list vs list)
/// The main menu uses GPUI's `list()` component (not `uniform_list`) to support variable heights:
/// - Section headers: 24px (SECTION_HEADER_HEIGHT)
/// - Regular items: 48px (LIST_ITEM_HEIGHT)
///
/// Performance comparison:
/// - `uniform_list`: O(1) scroll position calculation (all items same height)
/// - `list()`: O(log n) scroll position via SumTree (supports variable heights)
///
/// For typical menu sizes (< 1000 items), the performance difference is negligible.
pub const SECTION_HEADER_HEIGHT: f32 = 24.0;

/// Enum for grouped list items - supports both regular items and section headers
///
/// Used with GPUI's `list()` component when rendering grouped results (e.g., frecency with RECENT/MAIN sections).
/// The usize in Item variant is the index into the flat results array.
#[derive(Clone, Debug)]
pub enum GroupedListItem {
    /// A section header (e.g., "RECENT", "MAIN")
    SectionHeader(String),
    /// A regular list item - usize is the index in the flat results array
    Item(usize),
}

/// Pre-computed grouped list state for efficient navigation
///
/// This struct caches header positions and total counts to avoid expensive
/// recalculation on every keypress. Build it once when the list data changes,
/// then reuse for navigation.
///
/// ## Performance
/// - `is_header()`: O(1) lookup via HashSet
/// - `next_selectable()` / `prev_selectable()`: O(k) where k is consecutive headers
/// - Memory: O(h) where h is number of headers (typically < 10)
///
/// ## Usage Pattern
/// ```ignore
/// // Build once when data changes
/// let grouped = GroupedListState::from_groups(&[
///     ("Today", 5),      // 5 items in Today group
///     ("Yesterday", 3),  // 3 items in Yesterday group
/// ]);
///
/// // Use for navigation (fast, no allocation)
/// let next = grouped.next_selectable(current_index);
/// let prev = grouped.prev_selectable(current_index);
/// let is_hdr = grouped.is_header(index);
/// ```
#[derive(Clone, Debug)]
pub struct GroupedListState {
    /// Set of indices that are headers (for O(1) lookup)
    header_indices: std::collections::HashSet<usize>,
    /// Total number of visual items (headers + entries)
    pub total_items: usize,
    /// Index of first selectable item (skips leading header)
    pub first_selectable: usize,
}

impl GroupedListState {
    /// Create from a list of (group_name, item_count) pairs
    ///
    /// Each group gets a header at the start, followed by its items.
    /// Empty groups are skipped (no header for empty groups).
    pub fn from_groups(groups: &[(&str, usize)]) -> Self {
        let mut header_indices = std::collections::HashSet::new();
        let mut idx = 0;

        for (_, count) in groups {
            if *count > 0 {
                header_indices.insert(idx); // Header position
                idx += 1 + count; // Header + items
            }
        }

        let first_selectable = if header_indices.contains(&0) { 1 } else { 0 };

        Self {
            header_indices,
            total_items: idx,
            first_selectable,
        }
    }

    /// Create from pre-built GroupedListItem vec (when you already have the items)
    pub fn from_items(items: &[GroupedListItem]) -> Self {
        let mut header_indices = std::collections::HashSet::new();

        for (idx, item) in items.iter().enumerate() {
            if matches!(item, GroupedListItem::SectionHeader(_)) {
                header_indices.insert(idx);
            }
        }

        let first_selectable = if header_indices.contains(&0) { 1 } else { 0 };

        Self {
            header_indices,
            total_items: items.len(),
            first_selectable,
        }
    }

    /// Create an empty state (no headers, for flat lists)
    pub fn flat(item_count: usize) -> Self {
        Self {
            header_indices: std::collections::HashSet::new(),
            total_items: item_count,
            first_selectable: 0,
        }
    }

    /// Check if an index is a header (O(1))
    #[inline]
    pub fn is_header(&self, index: usize) -> bool {
        self.header_indices.contains(&index)
    }

    /// Get next selectable index (skips headers), or None if at end
    pub fn next_selectable(&self, current: usize) -> Option<usize> {
        let mut next = current + 1;
        while next < self.total_items && self.is_header(next) {
            next += 1;
        }
        if next < self.total_items {
            Some(next)
        } else {
            None
        }
    }

    /// Get previous selectable index (skips headers), or None if at start
    pub fn prev_selectable(&self, current: usize) -> Option<usize> {
        if current == 0 {
            return None;
        }
        let mut prev = current - 1;
        while prev > 0 && self.is_header(prev) {
            prev -= 1;
        }
        if !self.is_header(prev) {
            Some(prev)
        } else {
            None
        }
    }

    /// Get number of headers
    pub fn header_count(&self) -> usize {
        self.header_indices.len()
    }
}

/// Pre-computed colors for ListItem rendering
///
/// This struct holds the primitive color values needed for list item rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy)]
pub struct ListItemColors {
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_dimmed: u32,
    pub accent_selected: u32,
    pub accent_selected_subtle: u32,
    pub background: u32,
    pub background_selected: u32,
}

impl ListItemColors {
    /// Create from theme reference
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        Self {
            text_primary: theme.colors.text.primary,
            text_secondary: theme.colors.text.secondary,
            text_muted: theme.colors.text.muted,
            text_dimmed: theme.colors.text.dimmed,
            accent_selected: theme.colors.accent.selected,
            accent_selected_subtle: theme.colors.accent.selected_subtle,
            background: theme.colors.background.main,
            background_selected: theme.colors.accent.selected_subtle,
        }
    }

    /// Create from design colors for GLOBAL theming support
    pub fn from_design(colors: &crate::designs::DesignColors) -> Self {
        Self {
            text_primary: colors.text_primary,
            text_secondary: colors.text_secondary,
            text_muted: colors.text_muted,
            text_dimmed: colors.text_dimmed,
            accent_selected: colors.accent,
            accent_selected_subtle: colors.background_selected,
            background: colors.background,
            background_selected: colors.background_selected,
        }
    }
}

/// Callback type for hover events on list items.
/// The callback receives the item index and a boolean indicating hover state (true = entered, false = left).
pub type OnHoverCallback = Box<dyn Fn(usize, bool) + 'static>;

/// A reusable list item component for displaying selectable items
///
/// Supports:
/// - Name (required)
/// - Description (optional, shown below name)
/// - Icon (optional, emoji or PNG image displayed left of name)
/// - Shortcut badge (optional, right-aligned)
/// - Selection state with themed colors (full focus styling)
/// - Hover state with subtle visual feedback (separate from selection)
/// - Hover callback for mouse interaction (optional)
/// - Semantic ID for AI-driven targeting (optional)
///
/// # Example
/// ```ignore
/// let colors = ListItemColors::from_theme(&theme);
/// ListItem::new("My Script", colors)
///     .icon("📜")
///     .description("A helpful script")
///     .shortcut("⌘K")
///     .selected(true)
///     .hovered(false)
///     .index(0)
///     .semantic_id("choice:0:my-script")
///     .on_hover(Box::new(|index, hovered| {
///         if hovered { println!("Hovered item {}", index); }
///     }))
/// ```
#[derive(IntoElement)]
pub struct ListItem {
    name: SharedString,
    description: Option<String>,
    shortcut: Option<String>,
    icon: Option<IconKind>,
    selected: bool,
    /// Whether this item is being hovered (subtle visual feedback, separate from selected)
    hovered: bool,
    colors: ListItemColors,
    /// Index of this item in the list (needed for hover callback)
    index: Option<usize>,
    /// Optional callback triggered when mouse enters/leaves this item
    on_hover: Option<OnHoverCallback>,
    /// Semantic ID for AI-driven UX targeting. Format: {type}:{index}:{value}
    semantic_id: Option<String>,
    /// Show left accent bar when selected (3px colored bar on left edge)
    show_accent_bar: bool,
}

/// Width of the left accent bar for selected items
pub const ACCENT_BAR_WIDTH: f32 = 3.0;

impl ListItem {
    /// Create a new list item with the given name and pre-computed colors
    pub fn new(name: impl Into<SharedString>, colors: ListItemColors) -> Self {
        Self {
            name: name.into(),
            description: None,
            shortcut: None,
            icon: None,
            selected: false,
            hovered: false,
            colors,
            index: None,
            on_hover: None,
            semantic_id: None,
            show_accent_bar: false,
        }
    }

    /// Enable the left accent bar (3px colored bar shown when selected)
    pub fn with_accent_bar(mut self, show: bool) -> Self {
        self.show_accent_bar = show;
        self
    }

    /// Set the index of this item in the list (required for hover callback to work)
    pub fn index(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }

    /// Set a callback to be triggered when mouse enters or leaves this item.
    /// The callback receives (index, is_hovered) where is_hovered is true when entering.
    pub fn on_hover(mut self, callback: OnHoverCallback) -> Self {
        self.on_hover = Some(callback);
        self
    }

    /// Set the semantic ID for AI-driven UX targeting.
    /// Format: {type}:{index}:{value} (e.g., "choice:0:apple")
    pub fn semantic_id(mut self, id: impl Into<String>) -> Self {
        self.semantic_id = Some(id.into());
        self
    }

    /// Set an optional semantic ID (convenience for Option<String>)
    pub fn semantic_id_opt(mut self, id: Option<String>) -> Self {
        self.semantic_id = id;
        self
    }

    /// Set the description text (shown below the name)
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// Set an optional description (convenience for Option<String>)
    pub fn description_opt(mut self, d: Option<String>) -> Self {
        self.description = d;
        self
    }

    /// Set the shortcut badge text (shown right-aligned)
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    /// Set an optional shortcut (convenience for Option<String>)
    pub fn shortcut_opt(mut self, s: Option<String>) -> Self {
        self.shortcut = s;
        self
    }

    /// Set the icon (emoji) to display on the left side
    pub fn icon(mut self, i: impl Into<String>) -> Self {
        self.icon = Some(IconKind::Emoji(i.into()));
        self
    }

    /// Set an optional emoji icon (convenience for Option<String>)
    pub fn icon_opt(mut self, i: Option<String>) -> Self {
        self.icon = i.map(IconKind::Emoji);
        self
    }

    /// Set a pre-decoded RenderImage icon
    pub fn icon_image(mut self, image: Arc<RenderImage>) -> Self {
        self.icon = Some(IconKind::Image(image));
        self
    }

    /// Set an optional pre-decoded image icon
    pub fn icon_image_opt(mut self, image: Option<Arc<RenderImage>>) -> Self {
        self.icon = image.map(IconKind::Image);
        self
    }

    /// Set icon from IconKind enum (for mixed icon types)
    pub fn icon_kind(mut self, kind: IconKind) -> Self {
        self.icon = Some(kind);
        self
    }

    /// Set an optional icon from IconKind
    pub fn icon_kind_opt(mut self, kind: Option<IconKind>) -> Self {
        self.icon = kind;
        self
    }

    /// Set whether this item is selected
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Set whether this item is hovered (subtle visual feedback)
    ///
    /// Hovered items show a subtle background tint (25% opacity).
    /// This is separate from `selected` which shows full focus styling
    /// (50% opacity background + accent bar).
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }
}

impl RenderOnce for ListItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let colors = self.colors;
        let index = self.index;
        let on_hover_callback = self.on_hover;
        let semantic_id = self.semantic_id;

        // Selection colors with alpha
        let selected_bg = rgba((colors.accent_selected_subtle << 8) | 0x80);
        let hover_bg = rgba((colors.accent_selected_subtle << 8) | 0x40);

        // Icon element (if present) - displayed on the left
        // Supports both emoji strings and PNG image data
        // Icon text color matches the item's text color (primary when selected, secondary otherwise)
        let icon_text_color = if self.selected {
            rgb(colors.text_primary)
        } else {
            rgb(colors.text_secondary)
        };
        let icon_element = match &self.icon {
            Some(IconKind::Emoji(emoji)) => div()
                .w(px(20.))
                .h(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(icon_text_color)
                .flex_shrink_0()
                .child(emoji.clone()),
            Some(IconKind::Image(render_image)) => {
                // Render pre-decoded image directly (no decoding on render - critical for perf)
                let image = render_image.clone();
                div()
                    .w(px(20.))
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        img(move |_window: &mut Window, _cx: &mut App| Some(Ok(image.clone())))
                            .w(px(20.))
                            .h(px(20.))
                            .object_fit(ObjectFit::Contain),
                    )
            }
            Some(IconKind::Svg(name)) => {
                // Convert string to IconName and render SVG
                // Use external_path() for file system SVGs (not path() which is for embedded assets)
                if let Some(icon_name) = icon_name_from_str(name) {
                    let svg_path = icon_name.external_path();
                    div()
                        .w(px(20.))
                        .h(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .child(
                            svg()
                                .external_path(svg_path)
                                .size(px(16.))
                                .text_color(icon_text_color),
                        )
                } else {
                    // Fallback to Code icon if name not recognized
                    let svg_path = IconName::Code.external_path();
                    div()
                        .w(px(20.))
                        .h(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .child(
                            svg()
                                .external_path(svg_path)
                                .size(px(16.))
                                .text_color(icon_text_color),
                        )
                }
            }
            None => {
                div().w(px(0.)).h(px(0.)) // No space if no icon
            }
        };

        // Build content with name + description (tighter spacing)
        let mut item_content = div()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .justify_center();

        // Name - 14px font, medium weight (tighter than before)
        // Single-line with ellipsis truncation for long content
        item_content = item_content.child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .line_height(px(18.))
                .child(self.name),
        );

        // Description - 12px font, muted color (never changes on selection - only bg shows selection)
        // Single-line with ellipsis truncation for long content
        if let Some(desc) = self.description {
            let desc_color = rgb(colors.text_muted);
            item_content = item_content.child(
                div()
                    .text_size(px(12.))
                    .line_height(px(14.))
                    .text_color(desc_color)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(desc),
            );
        }

        // Shortcut badge (if present) - right-aligned
        let shortcut_element = if let Some(sc) = self.shortcut {
            div()
                .text_size(px(11.))
                .text_color(rgb(colors.text_dimmed))
                .px(px(6.))
                .py(px(2.))
                .rounded(px(3.))
                .bg(rgba((colors.background << 8) | 0x40))
                .child(sc)
        } else {
            div()
        };

        // Determine background color based on selection/hover state
        // Priority: selected (full focus styling) > hovered (subtle feedback) > transparent
        let bg_color = if self.selected {
            selected_bg // 50% opacity - full focus styling
        } else if self.hovered {
            hover_bg // 25% opacity - subtle hover feedback
        } else {
            rgba(0x00000000) // transparent
        };

        // Build the inner content div with all styling
        // Horizontal padding px(12.) and vertical padding py(6.) for comfortable spacing
        // NOTE: We manage hover state explicitly via is_hovered, not CSS .hover() pseudo-state
        let inner_content = div()
            .w_full()
            .h_full()
            .px(px(12.))
            .py(px(6.))
            .bg(bg_color)
            .text_color(if self.selected {
                rgb(colors.text_primary)
            } else {
                rgb(colors.text_secondary)
            })
            .font_family(".AppleSystemUIFont")
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(icon_element)
            .child(item_content)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_shrink_0()
                    .child(shortcut_element),
            );

        // Use semantic_id for element ID if available, otherwise fall back to index
        // This allows AI agents to target elements by their semantic meaning
        let element_id = if let Some(ref sem_id) = semantic_id {
            // Use semantic ID as the element ID for better targeting
            ElementId::Name(sem_id.clone().into())
        } else {
            // Fall back to index-based ID
            let element_idx = index.unwrap_or(0);
            ElementId::NamedInteger("list-item".into(), element_idx as u64)
        };

        // Accent bar: Use LEFT BORDER instead of child div because:
        // 1. GPUI clamps corner radii to ≤ half the shortest side
        // 2. A 3px-wide child with 12px radius gets clamped to ~1.5px (invisible)
        // 3. A border on the container follows rounded corners naturally
        let accent_color = rgb(colors.accent_selected);

        // Base container with ID for stateful interactivity
        // Use left border for accent indicator - always reserve space, toggle color
        let mut container = div()
            .w_full()
            .h(px(LIST_ITEM_HEIGHT))
            .pr(px(4.)) // Right padding only
            .flex()
            .flex_row()
            .items_center()
            .id(element_id);

        // Apply accent bar as left border (only when enabled)
        if self.show_accent_bar {
            container = container
                .border_l(px(ACCENT_BAR_WIDTH))
                .border_color(if self.selected {
                    accent_color
                } else {
                    rgba(0x00000000)
                });
        }

        // Add hover handler if we have both index and callback
        if let (Some(idx), Some(callback)) = (index, on_hover_callback) {
            // Use Rc to allow sharing the callback in the closure
            let callback = std::rc::Rc::new(callback);

            container = container.on_hover(move |hovered: &bool, _window, _cx| {
                // Log the mouse enter/leave event
                if *hovered {
                    logging::log_mouse_enter(idx, None);
                } else {
                    logging::log_mouse_leave(idx, None);
                }
                // Call the user-provided callback
                callback(idx, *hovered);
            });
        }

        // Add content (no separate accent bar child needed)
        container.child(inner_content)
    }
}

/// Decode PNG bytes to GPUI RenderImage
///
/// Decode PNG bytes to a GPUI RenderImage
///
/// Uses the `image` crate to decode PNG data and creates a GPUI-compatible
/// RenderImage for display. Returns an Arc<RenderImage> for caching.
///
/// **IMPORTANT**: Call this ONCE when loading icons, NOT during rendering.
/// Decoding PNGs on every render frame causes severe performance issues.
pub fn decode_png_to_render_image(png_data: &[u8]) -> Result<Arc<RenderImage>, image::ImageError> {
    decode_png_to_render_image_internal(png_data, false)
}

/// Decode PNG bytes to GPUI RenderImage with RGBA→BGRA conversion for Metal
///
/// GPUI/Metal expects BGRA pixel format. When creating RenderImage directly
/// from image::Frame (bypassing GPUI's internal loaders), we must do the
/// RGBA→BGRA conversion ourselves. This matches what GPUI does internally
/// in platform.rs for loaded images.
///
/// **IMPORTANT**: Call this ONCE when loading icons, NOT during rendering.
pub fn decode_png_to_render_image_with_bgra_conversion(
    png_data: &[u8],
) -> Result<Arc<RenderImage>, image::ImageError> {
    decode_png_to_render_image_internal(png_data, true)
}

fn decode_png_to_render_image_internal(
    png_data: &[u8],
    convert_to_bgra: bool,
) -> Result<Arc<RenderImage>, image::ImageError> {
    use image::GenericImageView;
    use smallvec::SmallVec;

    // Decode PNG
    let img = image::load_from_memory(png_data)?;

    // Convert to RGBA8
    let mut rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    // Convert RGBA to BGRA for Metal/GPUI rendering
    // GPUI's internal image loading does this swap (see gpui/src/platform.rs)
    // We must do the same when creating RenderImage directly from image::Frame
    if convert_to_bgra {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2); // Swap R and B: RGBA -> BGRA
        }
    }

    // Create Frame from buffer (now in BGRA order if converted)
    let buffer = image::RgbaImage::from_raw(width, height, rgba.into_raw())
        .expect("Failed to create image buffer");
    let frame = image::Frame::new(buffer);

    // Create RenderImage
    let render_image = RenderImage::new(SmallVec::from_elem(frame, 1));

    Ok(Arc::new(render_image))
}

/// Create an IconKind from PNG bytes by pre-decoding them
///
/// Returns None if decoding fails. This should be called once when loading
/// icons, not during rendering.
pub fn icon_from_png(png_data: &[u8]) -> Option<IconKind> {
    decode_png_to_render_image(png_data)
        .ok()
        .map(IconKind::Image)
}

/// Render a section header for grouped lists (e.g., "Recent", "Main")
///
/// Visual design for section headers:
/// - Standard casing (not uppercase)
/// - Small font (~10-11px via text_xs)
/// - Semi-bold weight (SEMIBOLD for subtlety)
/// - Dimmed color (subtle but readable)
/// - Compact vertical footprint within the 48px uniform_list row
/// - Large top padding to create visual compression (appears ~24px tall)
/// - Left-aligned with list item padding
/// - No background, no border
///
/// ## Technical Note: uniform_list Height Constraint
/// GPUI's `uniform_list` requires fixed heights for O(1) scroll calculation.
/// We cannot use actual variable heights. Instead, we use a visual trick:
/// - Actual height: 48px (LIST_ITEM_HEIGHT, for uniform_list)
/// - Visual height: ~24px (via top padding compression)
/// - Content is pushed to the bottom 24px of the container
///
/// This gives the appearance of 50% height while maintaining uniform_list compatibility.
///
/// # Arguments
/// * `label` - The section label (displayed as-is, standard casing)
/// * `colors` - ListItemColors for theme-aware styling
///
/// # Example
/// ```ignore
/// let colors = ListItemColors::from_theme(&theme);
/// render_section_header("Recent", colors)
/// ```
pub fn render_section_header(label: &str, colors: ListItemColors) -> impl IntoElement {
    // Compact section header with explicit height (SECTION_HEADER_HEIGHT = 24px)
    // Used with GPUI's list() component which supports variable-height items.
    //
    // Layout: 24px total height
    // - pt(8px) top padding for visual separation from above item
    // - ~8px text height (text_xs)
    // - pb(4px) bottom padding for visual separation from below item
    div()
        .w_full()
        .h(px(SECTION_HEADER_HEIGHT)) // Explicit 24px height for variable-height list
        .px(px(16.))
        .pt(px(8.)) // Top padding for visual separation
        .pb(px(4.)) // Bottom padding
        .flex()
        .flex_col()
        .justify_center() // Center content vertically
        .child(
            div()
                .text_xs() // 10-11px font
                .font_weight(FontWeight::SEMIBOLD) // Slightly lighter than BOLD
                .text_color(rgb(colors.text_dimmed))
                .child(label.to_string()), // Standard casing (not uppercased)
        )
}

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The LIST_ITEM_HEIGHT constant is 52.0 and the component is integration-tested
// via the main application's script list and arg prompt rendering.
