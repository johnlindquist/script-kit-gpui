//! Shared ListItem component for script list and arg prompt choice list
//!
//! This module provides a reusable, theme-aware list item component that can be
//! used in both the main script list and arg prompt choice lists.

#![allow(dead_code)]

use gpui::*;
use std::sync::Arc;
use crate::logging;

/// Icon type for list items - supports both emoji strings and pre-decoded images
#[derive(Clone)]
pub enum IconKind {
    /// Text/emoji icon (e.g., "📜", "⚡")
    Emoji(String),
    /// Pre-decoded render image (for app icons) - MUST be pre-decoded, not raw PNG bytes
    Image(Arc<RenderImage>),
}

/// Fixed height for list items (same as main script list)
/// Reduced from 52px to 34px for tighter, more compact layout matching original Script Kit design
pub const LIST_ITEM_HEIGHT: f32 = 34.0;

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
/// - Selection state with themed colors
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
    colors: ListItemColors,
    /// Index of this item in the list (needed for hover callback)
    index: Option<usize>,
    /// Optional callback triggered when mouse enters/leaves this item
    on_hover: Option<OnHoverCallback>,
    /// Semantic ID for AI-driven UX targeting. Format: {type}:{index}:{value}
    semantic_id: Option<String>,
}

impl ListItem {
    /// Create a new list item with the given name and pre-computed colors
    pub fn new(name: impl Into<SharedString>, colors: ListItemColors) -> Self {
        Self {
            name: name.into(),
            description: None,
            shortcut: None,
            icon: None,
            selected: false,
            colors,
            index: None,
            on_hover: None,
            semantic_id: None,
        }
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
        let icon_text_color = if self.selected { rgb(colors.text_primary) } else { rgb(colors.text_secondary) };
        let icon_element = match &self.icon {
            Some(IconKind::Emoji(emoji)) => {
                div()
                    .w(px(20.))
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(icon_text_color)
                    .flex_shrink_0()
                    .child(emoji.clone())
            }
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
                        img(move |_window: &mut Window, _cx: &mut App| {
                            Some(Ok(image.clone()))
                        })
                        .w(px(20.))
                        .h(px(20.))
                        .object_fit(ObjectFit::Contain)
                    )
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
                .child(self.name)
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
                    .child(desc)
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
        
        // Build the inner content div with all styling (reduced horizontal padding)
        let inner_content = div()
            .w_full()
            .h_full()
            .px(px(8.))
            .bg(if self.selected { selected_bg } else { rgba(0x00000000) })
            .hover(|s| s.bg(hover_bg))
            .text_color(if self.selected { rgb(colors.text_primary) } else { rgb(colors.text_secondary) })
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
                    .child(shortcut_element)
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
        
        // Base container with ID for stateful interactivity (reduced horizontal padding)
        let mut container = div()
            .w_full()
            .h(px(LIST_ITEM_HEIGHT))
            .px(px(8.))
            .flex()
            .items_center()
            .id(element_id);
        
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
    use image::GenericImageView;
    use smallvec::SmallVec;
    
    // Decode PNG
    let img = image::load_from_memory(png_data)?;
    
    // Convert to RGBA8
    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();
    
    // Create Frame from RGBA buffer
    let buffer = image::RgbaImage::from_raw(width, height, rgba.into_raw())
        .expect("Failed to create RGBA image buffer");
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

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The LIST_ITEM_HEIGHT constant is 52.0 and the component is integration-tested
// via the main application's script list and arg prompt rendering.
