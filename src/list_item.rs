//! Shared ListItem component for script list and arg prompt choice list
//!
//! This module provides a reusable, theme-aware list item component that can be
//! used in both the main script list and arg prompt choice lists.

#![allow(dead_code)]

use gpui::*;
use crate::logging;

/// Fixed height for list items (same as main script list)
pub const LIST_ITEM_HEIGHT: f32 = 52.0;

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
/// - Shortcut badge (optional, right-aligned)
/// - Selection state with themed colors
/// - Hover callback for mouse interaction (optional)
///
/// # Example
/// ```ignore
/// let colors = ListItemColors::from_theme(&theme);
/// ListItem::new("My Script", colors)
///     .description("A helpful script")
///     .shortcut("⌘K")
///     .selected(true)
///     .index(0)
///     .on_hover(Box::new(|index, hovered| {
///         if hovered { println!("Hovered item {}", index); }
///     }))
/// ```
#[derive(IntoElement)]
pub struct ListItem {
    name: SharedString,
    description: Option<String>,
    shortcut: Option<String>,
    selected: bool,
    colors: ListItemColors,
    /// Index of this item in the list (needed for hover callback)
    index: Option<usize>,
    /// Optional callback triggered when mouse enters/leaves this item
    on_hover: Option<OnHoverCallback>,
}

impl ListItem {
    /// Create a new list item with the given name and pre-computed colors
    pub fn new(name: impl Into<SharedString>, colors: ListItemColors) -> Self {
        Self {
            name: name.into(),
            description: None,
            shortcut: None,
            selected: false,
            colors,
            index: None,
            on_hover: None,
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
        
        // Selection colors with alpha
        let selected_bg = rgba((colors.accent_selected_subtle << 8) | 0x80);
        let hover_bg = rgba((colors.accent_selected_subtle << 8) | 0x40);
        
        // Build content with name + description
        let mut item_content = div()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(2.));
        
        // Name - brighter when selected
        item_content = item_content.child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .overflow_hidden()
                .child(self.name)
        );
        
        // Description - accent color when selected, muted when not
        if let Some(desc) = self.description {
            let desc_color = if self.selected { 
                rgb(colors.accent_selected) 
            } else { 
                rgb(colors.text_muted) 
            };
            item_content = item_content.child(
                div()
                    .text_xs()
                    .text_color(desc_color)
                    .overflow_hidden()
                    .max_h(px(16.))
                    .child(desc)
            );
        }
        
        // Shortcut badge (if present)
        let shortcut_element = if let Some(sc) = self.shortcut {
            div()
                .text_xs()
                .text_color(rgb(colors.text_dimmed))
                .px(px(8.))
                .rounded(px(4.))
                .child(sc)
        } else {
            div()
        };
        
        // Build the inner content div with all styling
        let inner_content = div()
            .w_full()
            .h_full()
            .px(px(12.))
            .bg(if self.selected { selected_bg } else { rgba(0x00000000) })
            .hover(|s| s.bg(hover_bg))
            .text_color(if self.selected { rgb(colors.text_primary) } else { rgb(colors.text_secondary) })
            .font_family(".AppleSystemUIFont")
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_2()
            .child(item_content)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_shrink_0()
                    .child(shortcut_element)
            );
        
        // Use index for element ID (default to 0 if not set)
        let element_idx = index.unwrap_or(0);
        
        // Base container with ID for stateful interactivity
        let mut container = div()
            .w_full()
            .h(px(LIST_ITEM_HEIGHT))
            .px(px(12.))
            .flex()
            .items_center()
            .id(ElementId::NamedInteger("list-item".into(), element_idx as u64));
        
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

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The LIST_ITEM_HEIGHT constant is 52.0 and the component is integration-tested
// via the main application's script list and arg prompt rendering.
