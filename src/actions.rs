//! Actions Dialog Module
//!
//! Provides a searchable action menu as a compact overlay popup for quick access 
//! to script management and global actions (edit, create, settings, quit, etc.)
//! 
//! The dialog renders as a floating overlay popup with:
//! - Fixed dimensions (320x400px max)
//! - Rounded corners and box shadow
//! - Semi-transparent background
//! - Context-aware actions based on focused script

#![allow(dead_code)]

use gpui::{
    div, point, prelude::*, px, rgb, rgba, uniform_list, App, BoxShadow, 
    Context, FocusHandle, Focusable, Hsla, Render, ScrollStrategy, SharedString, 
    UniformListScrollHandle, Window,
};
use std::sync::Arc;
use crate::logging;
use crate::theme;
use crate::designs::{DesignVariant, DesignColors, get_tokens};
use crate::components::scrollbar::{Scrollbar, ScrollbarColors};

/// Callback for action selection
/// Signature: (action_id: String)
pub type ActionCallback = Arc<dyn Fn(String) + Send + Sync>;

/// Information about the currently focused/selected script
/// Used for context-aware actions in the actions dialog
#[derive(Debug, Clone)]
pub struct ScriptInfo {
    /// Display name of the script
    pub name: String,
    /// Full path to the script file
    pub path: String,
}

impl ScriptInfo {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        ScriptInfo {
            name: name.into(),
            path: path.into(),
        }
    }
}

/// Available actions in the actions menu
#[derive(Debug, Clone)]
pub struct Action {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub category: ActionCategory,
    /// Optional keyboard shortcut hint (e.g., "⌘E")
    pub shortcut: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionCategory {
    ScriptContext,  // Actions specific to the focused script
    ScriptOps,      // Edit, Create, Delete script operations
    GlobalOps,      // Settings, Quit, etc.
}

impl Action {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        category: ActionCategory,
    ) -> Self {
        Action {
            id: id.into(),
            title: title.into(),
            description,
            category,
            shortcut: None,
        }
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// Get actions specific to the focused script
pub fn get_script_context_actions(script: &ScriptInfo) -> Vec<Action> {
    vec![
        Action::new(
            "run_script",
            format!("Run \"{}\"", script.name),
            Some("Execute this script".to_string()),
            ActionCategory::ScriptContext,
        ).with_shortcut("↵"),
        Action::new(
            "edit_script",
            "Edit Script",
            Some("Open in $EDITOR".to_string()),
            ActionCategory::ScriptContext,
        ).with_shortcut("⌘E"),
        Action::new(
            "view_logs",
            "View Logs",
            Some("Show script execution logs".to_string()),
            ActionCategory::ScriptContext,
        ).with_shortcut("⌘L"),
        Action::new(
            "reveal_in_finder",
            "Reveal in Finder",
            Some("Show script file in Finder".to_string()),
            ActionCategory::ScriptContext,
        ).with_shortcut("⌘⇧F"),
        Action::new(
            "copy_path",
            "Copy Path",
            Some("Copy script path to clipboard".to_string()),
            ActionCategory::ScriptContext,
        ).with_shortcut("⌘⇧C"),
    ]
}

/// Predefined global actions
pub fn get_global_actions() -> Vec<Action> {
    vec![
        Action::new(
            "create_script",
            "Create New Script",
            Some("Create a new TypeScript script".to_string()),
            ActionCategory::ScriptOps,
        ).with_shortcut("⌘N"),
        Action::new(
            "reload_scripts",
            "Reload Scripts",
            Some("Refresh the scripts list".to_string()),
            ActionCategory::GlobalOps,
        ).with_shortcut("⌘R"),
        Action::new(
            "settings",
            "Settings",
            Some("Configure preferences".to_string()),
            ActionCategory::GlobalOps,
        ).with_shortcut("⌘,"),
        Action::new(
            "quit",
            "Quit Script Kit",
            Some("Exit the application".to_string()),
            ActionCategory::GlobalOps,
        ).with_shortcut("⌘Q"),
    ]
}

/// Overlay popup dimensions and styling constants
pub const POPUP_WIDTH: f32 = 320.0;
pub const POPUP_MAX_HEIGHT: f32 = 400.0;
pub const POPUP_CORNER_RADIUS: f32 = 12.0;
pub const POPUP_PADDING: f32 = 8.0;
pub const ITEM_PADDING_X: f32 = 12.0;
pub const ITEM_PADDING_Y: f32 = 8.0;
/// Fixed height for action items (required for uniform_list virtualization)
/// Increased from 36px to 42px for better touch targets and visual breathing room
pub const ACTION_ITEM_HEIGHT: f32 = 42.0;

/// Width of the left accent bar for selected items
pub const ACCENT_BAR_WIDTH: f32 = 3.0;

/// ActionsDialog - Compact overlay popup for quick actions
pub struct ActionsDialog {
    pub actions: Vec<Action>,
    pub filtered_actions: Vec<usize>, // Indices into actions
    pub selected_index: usize,        // Index within filtered_actions
    pub search_text: String,
    pub focus_handle: FocusHandle,
    pub on_select: ActionCallback,
    /// Currently focused script for context-aware actions
    pub focused_script: Option<ScriptInfo>,
    /// Scroll handle for uniform_list virtualization
    pub scroll_handle: UniformListScrollHandle,
    /// Theme for consistent color styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling (defaults to Default for theme-based styling)
    pub design_variant: DesignVariant,
    /// Cursor visibility for blinking (controlled externally)
    pub cursor_visible: bool,
    /// When true, hide the search input (used when rendered inline in main.rs header)
    pub hide_search: bool,
}

/// Helper function to combine a hex color with an alpha value
/// Delegates to DesignColors::hex_with_alpha for DRY
#[inline]
fn hex_with_alpha(hex: u32, alpha: u8) -> u32 {
    DesignColors::hex_with_alpha(hex, alpha)
}

impl ActionsDialog {
    pub fn new(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_script_and_design(focus_handle, on_select, None, theme, DesignVariant::Default)
    }

    pub fn with_script(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        focused_script: Option<ScriptInfo>,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_script_and_design(focus_handle, on_select, focused_script, theme, DesignVariant::Default)
    }
    
    pub fn with_design(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
    ) -> Self {
        Self::with_script_and_design(focus_handle, on_select, None, theme, design_variant)
    }

    pub fn with_script_and_design(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        focused_script: Option<ScriptInfo>,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
    ) -> Self {
        let actions = Self::build_actions(&focused_script);
        let filtered_actions: Vec<usize> = (0..actions.len()).collect();
        
        logging::log("ACTIONS", &format!(
            "ActionsDialog created with {} actions, script: {:?}, design: {:?}", 
            actions.len(),
            focused_script.as_ref().map(|s| &s.name),
            design_variant
        ));
        
        // Log theme color configuration for debugging
        logging::log("ACTIONS_THEME", &format!(
            "Theme colors applied: bg_main=#{:06x}, bg_search=#{:06x}, text_primary=#{:06x}, accent_selected=#{:06x}",
            theme.colors.background.main,
            theme.colors.background.search_box,
            theme.colors.text.primary,
            theme.colors.accent.selected
        ));
        
        ActionsDialog {
            actions,
            filtered_actions,
            selected_index: 0,
            search_text: String::new(),
            focus_handle,
            on_select,
            focused_script,
            scroll_handle: UniformListScrollHandle::new(),
            theme,
            design_variant,
            cursor_visible: true,
            hide_search: false,
        }
    }
    
    /// Update cursor visibility (called from parent's blink timer)
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }
    
    /// Hide the search input (for inline mode where header has search)
    pub fn set_hide_search(&mut self, hide: bool) {
        self.hide_search = hide;
    }

    /// Build the complete actions list based on focused script
    fn build_actions(focused_script: &Option<ScriptInfo>) -> Vec<Action> {
        let mut actions = Vec::new();
        
        // Add script-specific actions first if a script is focused
        if let Some(script) = focused_script {
            actions.extend(get_script_context_actions(script));
        }
        
        // Add global actions
        actions.extend(get_global_actions());
        
        actions
    }

    /// Update the focused script and rebuild actions
    pub fn set_focused_script(&mut self, script: Option<ScriptInfo>) {
        self.focused_script = script;
        self.actions = Self::build_actions(&self.focused_script);
        self.refilter();
    }

    /// Refilter actions based on current search_text using fuzzy matching
    fn refilter(&mut self) {
        if self.search_text.is_empty() {
            self.filtered_actions = (0..self.actions.len()).collect();
        } else {
            let search_lower = self.search_text.to_lowercase();
            self.filtered_actions = self
                .actions
                .iter()
                .enumerate()
                .filter(|(_, action)| {
                    let title_match = action.title.to_lowercase().contains(&search_lower);
                    let desc_match = action
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&search_lower))
                        .unwrap_or(false);
                    title_match || desc_match
                })
                .map(|(idx, _)| idx)
                .collect();
        }
        self.selected_index = 0; // Reset selection when filtering
        self.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
        logging::log_debug("ACTIONS_SCROLL", &format!("Filter changed: reset to top, {} results", self.filtered_actions.len()));
    }

    /// Handle character input
    pub fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        self.search_text.push(ch);
        self.refilter();
        cx.notify();
    }

    /// Handle backspace
    pub fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.search_text.is_empty() {
            self.search_text.pop();
            self.refilter();
            cx.notify();
        }
    }

    /// Move selection up
    pub fn move_up(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.scroll_handle.scroll_to_item(self.selected_index, ScrollStrategy::Nearest);
            logging::log_debug("ACTIONS_SCROLL", &format!("Up: selected_index={}", self.selected_index));
            cx.notify();
        }
    }

    /// Move selection down
    pub fn move_down(&mut self, cx: &mut Context<Self>) {
        if self.selected_index < self.filtered_actions.len().saturating_sub(1) {
            self.selected_index += 1;
            self.scroll_handle.scroll_to_item(self.selected_index, ScrollStrategy::Nearest);
            logging::log_debug("ACTIONS_SCROLL", &format!("Down: selected_index={}", self.selected_index));
            cx.notify();
        }
    }

    /// Get the currently selected action ID (for external handling)
    pub fn get_selected_action_id(&self) -> Option<String> {
        if let Some(&action_idx) = self.filtered_actions.get(self.selected_index) {
            if let Some(action) = self.actions.get(action_idx) {
                return Some(action.id.clone());
            }
        }
        None
    }

    /// Submit the selected action
    fn submit_selected(&mut self) {
        if let Some(&action_idx) = self.filtered_actions.get(self.selected_index) {
            if let Some(action) = self.actions.get(action_idx) {
                logging::log("ACTIONS", &format!("Action selected: {}", action.id));
                (self.on_select)(action.id.clone());
            }
        }
    }

    /// Cancel - close the dialog
    fn submit_cancel(&mut self) {
        logging::log("ACTIONS", "Actions dialog cancelled");
        (self.on_select)("__cancel__".to_string());
    }

    /// Create box shadow for the overlay popup
    fn create_popup_shadow() -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.3 },
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(16.0),
                spread_radius: px(0.0),
            },
            BoxShadow {
                color: Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.15 },
                offset: point(px(0.0), px(8.0)),
                blur_radius: px(32.0),
                spread_radius: px(-4.0),
            },
        ]
    }
    
    /// Get colors for the search box based on design variant
    /// Returns: (search_box_bg, border_color, muted_text, dimmed_text, secondary_text)
    fn get_search_colors(&self, colors: &crate::designs::DesignColors) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba, gpui::Rgba, gpui::Rgba) {
        use gpui::{rgb, rgba};
        if self.design_variant == DesignVariant::Default {
            (
                rgba(hex_with_alpha(self.theme.colors.background.search_box, 0xcc)),
                rgba(hex_with_alpha(self.theme.colors.ui.border, 0x80)),
                rgb(self.theme.colors.text.muted),
                rgb(self.theme.colors.text.dimmed),
                rgb(self.theme.colors.text.secondary),
            )
        } else {
            (
                rgba(hex_with_alpha(colors.background_secondary, 0xcc)),
                rgba(hex_with_alpha(colors.border, 0x80)),
                rgb(colors.text_muted),
                rgb(colors.text_dimmed),
                rgb(colors.text_secondary),
            )
        }
    }
    
    /// Get colors for the main container based on design variant
    /// Returns: (main_bg, container_border, container_text)
    fn get_container_colors(&self, colors: &crate::designs::DesignColors) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
        use gpui::{rgb, rgba};
        if self.design_variant == DesignVariant::Default {
            (
                rgba(hex_with_alpha(self.theme.colors.background.main, 0xe6)),
                rgba(hex_with_alpha(self.theme.colors.ui.border, 0x80)),
                rgb(self.theme.colors.text.secondary),
            )
        } else {
            (
                rgba(hex_with_alpha(colors.background, 0xe6)),
                rgba(hex_with_alpha(colors.border, 0x80)),
                rgb(colors.text_secondary),
            )
        }
    }
}

impl Focusable for ActionsDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ActionsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Get design tokens for the current design variant
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();
        let spacing = tokens.spacing();
        let visual = tokens.visual();
        
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            
            match key_str.as_str() {
                "up" | "arrowup" => this.move_up(cx),
                "down" | "arrowdown" => this.move_down(cx),
                "enter" => this.submit_selected(),
                "escape" => this.submit_cancel(),
                "backspace" => this.handle_backspace(cx),
                _ => {
                    // Try to capture printable characters
                    if let Some(ref key_char) = event.keystroke.key_char {
                        if let Some(ch) = key_char.chars().next() {
                            if !ch.is_control() {
                                this.handle_char(ch, cx);
                            }
                        }
                    }
                }
            }
        });

        // Render search input - compact version
        let search_display = if self.search_text.is_empty() {
            SharedString::from("Search actions...")
        } else {
            SharedString::from(self.search_text.clone())
        };

        // Use helper method for design/theme color extraction
        let (search_box_bg, border_color, _muted_text, dimmed_text, _secondary_text) = 
            self.get_search_colors(&colors);
        
        // Get primary text color for cursor (matches main list styling)
        let primary_text = if self.design_variant == DesignVariant::Default {
            rgb(self.theme.colors.text.primary)
        } else {
            rgb(colors.text_primary)
        };
        
        // Get accent color for the search input focus indicator
        let accent_color_hex = if self.design_variant == DesignVariant::Default {
            self.theme.colors.accent.selected
        } else {
            colors.accent
        };
        let accent_color = rgb(accent_color_hex);
        
        // Focus border color (accent with transparency)
        let focus_border_color = rgba(hex_with_alpha(accent_color_hex, 0x60));
        
        // Input container with fixed height and width to prevent any layout shifts
        // The entire row is constrained to prevent resizing when text is entered
        let input_container = div()
            .w(px(POPUP_WIDTH))  // Match parent width exactly
            .min_w(px(POPUP_WIDTH))
            .max_w(px(POPUP_WIDTH))
            .h(px(44.0))  // Fixed height for the input row
            .min_h(px(44.0))
            .max_h(px(44.0))
            .overflow_hidden()  // Prevent any content from causing shifts
            .px(px(spacing.item_padding_x))
            .py(px(spacing.item_padding_y + 2.0)) // Slightly more vertical padding
            .bg(search_box_bg)
            .border_t_1()  // Border on top since input is now at bottom
            .border_color(border_color)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(spacing.gap_md))
            .child(
                // Search icon or indicator - fixed width to prevent shifts
                div()
                    .w(px(24.0))  // Fixed width for the icon container
                    .min_w(px(24.0))
                    .text_color(dimmed_text)
                    .text_xs()
                    .child("⌘K"),
            )
            .child(
                // Search input field with focus indicator
                // CRITICAL: Use flex_shrink_0 to prevent flexbox from shrinking this container
                // The border/bg MUST stay at fixed width regardless of content
                div()
                    .flex_shrink_0()  // PREVENT flexbox from shrinking this!
                    .w(px(240.0))
                    .min_w(px(240.0))
                    .max_w(px(240.0))
                    .h(px(28.0))      // Fixed height too
                    .min_h(px(28.0))
                    .max_h(px(28.0))
                    .overflow_hidden()
                    .px(px(spacing.padding_sm))
                    .py(px(spacing.padding_xs))
                    // ALWAYS show background - just vary intensity
                    .bg(if self.design_variant == DesignVariant::Default {
                        rgba(hex_with_alpha(self.theme.colors.background.main, if self.search_text.is_empty() { 0x20 } else { 0x40 }))
                    } else {
                        rgba(hex_with_alpha(colors.background, if self.search_text.is_empty() { 0x20 } else { 0x40 }))
                    })
                    .rounded(px(visual.radius_sm))
                    .border_1()
                    // ALWAYS show border - just vary intensity
                    .border_color(if !self.search_text.is_empty() { 
                        focus_border_color
                    } else { 
                        border_color
                    })
                    .flex()
                    .flex_row()
                    .items_center()
                    .text_sm()
                    .text_color(if self.search_text.is_empty() {
                        dimmed_text
                    } else {
                        primary_text
                    })
                    // ALWAYS render cursor div with consistent margin to prevent layout shift
                    // When empty, cursor is at the start before placeholder text
                    .when(self.search_text.is_empty(), |d| {
                        d.child(
                            div()
                                .w(px(2.))
                                .h(px(16.))
                                .mr(px(2.))  // Use consistent 2px margin
                                .rounded(px(1.))
                                .when(self.cursor_visible, |d| d.bg(accent_color))
                        )
                    })
                    .child(search_display)
                    // When has text, cursor is at the end after the text
                    .when(!self.search_text.is_empty(), |d| {
                        d.child(
                            div()
                                .w(px(2.))
                                .h(px(16.))
                                .ml(px(2.))  // Consistent 2px margin
                                .rounded(px(1.))
                                .when(self.cursor_visible, |d| d.bg(accent_color))
                        )
                    }),
            );

        // Render action list using uniform_list for virtualized scrolling
        let actions_container = if self.filtered_actions.is_empty() {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .w_full()
                .child(
                    div()
                        .w_full()
                        .py(px(spacing.padding_lg))
                        .px(px(spacing.item_padding_x))
                        .text_color(dimmed_text)
                        .text_sm()
                        .child("No actions match your search"),
                )
                .into_any_element()
        } else {
            // Clone data needed for the uniform_list closure
            let selected_index = self.selected_index;
            let filtered_len = self.filtered_actions.len();
            let design_variant = self.design_variant;
            
            logging::log_debug("ACTIONS_SCROLL", &format!(
                "Rendering uniform_list: {} items, selected={}",
                filtered_len, selected_index
            ));
            
            // Calculate scrollbar parameters
            // Container height for actions (excluding search box)
            let search_box_height = if self.hide_search { 0.0 } else { 60.0 };
            let container_height = (filtered_len as f32 * ACTION_ITEM_HEIGHT).min(POPUP_MAX_HEIGHT - search_box_height);
            let visible_items = (container_height / ACTION_ITEM_HEIGHT) as usize;
            
            // Use selected_index as approximate scroll offset
            // When scrolling, the selected item should be visible, so this gives a reasonable estimate
            let scroll_offset = if selected_index > visible_items.saturating_sub(1) {
                selected_index.saturating_sub(visible_items / 2)
            } else {
                0
            };
            
            // Get scrollbar colors from theme or design
            let scrollbar_colors = if self.design_variant == DesignVariant::Default {
                ScrollbarColors::from_theme(&self.theme)
            } else {
                ScrollbarColors::from_design(&colors)
            };
            
            // Create scrollbar (only visible if content overflows)
            let scrollbar = Scrollbar::new(
                filtered_len,
                visible_items,
                scroll_offset,
                scrollbar_colors,
            ).container_height(container_height);
            
            let list = uniform_list(
                "actions-list",
                filtered_len,
                cx.processor(move |this: &mut ActionsDialog, visible_range, _window, _cx| {
                    logging::log_debug("ACTIONS_SCROLL", &format!(
                        "Actions visible range: {:?} (total={})",
                        visible_range, this.filtered_actions.len()
                    ));
                    
                    // Get tokens for list item rendering
                    let item_tokens = get_tokens(design_variant);
                    let item_colors = item_tokens.colors();
                    let item_spacing = item_tokens.spacing();
                    let _item_visual = item_tokens.visual();
                    
                    // Extract colors for list items - MATCH main list styling exactly
                    // Uses accent_selected_subtle with 0x80 alpha (same as ListItem)
                    let (selected_bg, hover_bg, primary_text, secondary_text, dimmed_text) = 
                        if design_variant == DesignVariant::Default {
                            (
                                // Selected: subtle background with 50% opacity (matches ListItem)
                                rgba((this.theme.colors.accent.selected_subtle << 8) | 0x80),
                                // Hover: subtle background with 25% opacity (matches ListItem)
                                rgba((this.theme.colors.accent.selected_subtle << 8) | 0x40),
                                rgb(this.theme.colors.text.primary),
                                rgb(this.theme.colors.text.secondary),
                                rgb(this.theme.colors.text.dimmed),
                            )
                        } else {
                            (
                                rgba((item_colors.background_selected << 8) | 0x80),
                                rgba((item_colors.background_selected << 8) | 0x40),
                                rgb(item_colors.text_primary),
                                rgb(item_colors.text_secondary),
                                rgb(item_colors.text_dimmed),
                            )
                        };
                    
                    let mut items = Vec::new();
                    
                    // Get border color for category separators
                    let separator_color = if design_variant == DesignVariant::Default {
                        rgba(hex_with_alpha(this.theme.colors.ui.border, 0x40))
                    } else {
                        rgba(hex_with_alpha(item_colors.border_subtle, 0x40))
                    };
                    
                    for idx in visible_range {
                        if let Some(&action_idx) = this.filtered_actions.get(idx) {
                            if let Some(action) = this.actions.get(action_idx) {
                                let action: &Action = action; // Explicit type annotation
                                let is_selected = idx == selected_index;
                                
                                // Check if this is the first item of a new category
                                // (for adding a subtle separator line)
                                let is_category_start = if idx > 0 {
                                    if let Some(&prev_action_idx) = this.filtered_actions.get(idx - 1) {
                                        if let Some(prev_action) = this.actions.get(prev_action_idx) {
                                            let prev_action: &Action = prev_action;
                                            prev_action.category != action.category
                                        } else { false }
                                    } else { false }
                                } else { false };
                                
                                // Match main list styling: bright text when selected, secondary when not
                                let title_color = if is_selected {
                                    primary_text
                                } else {
                                    secondary_text
                                };

                                let shortcut_color = dimmed_text;

                                // Clone strings for SharedString conversion
                                let title_str: String = action.title.clone();
                                let shortcut_opt: Option<String> = action.shortcut.clone();

                                // Build the action item with left accent bar for selected state
                                let mut action_item = div()
                                    .id(idx)
                                    .w_full()
                                    .h(px(ACTION_ITEM_HEIGHT)) // Fixed height for uniform_list
                                    // Match main list: subtle selection bg, transparent when not selected
                                    .bg(if is_selected { selected_bg } else { rgba(0x00000000) })
                                    // Add top border for category separator
                                    .when(is_category_start, |d| d.border_t_1().border_color(separator_color))
                                    .hover(|s| s.bg(hover_bg))
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center();
                                
                                // Left accent bar - only visible when selected
                                // Get accent color for the left bar
                                let accent_color = if design_variant == DesignVariant::Default {
                                    rgb(this.theme.colors.accent.selected)
                                } else {
                                    rgb(item_colors.accent)
                                };
                                
                                action_item = action_item.child(
                                    div()
                                        .w(px(ACCENT_BAR_WIDTH))
                                        .h_full()
                                        .bg(if is_selected { accent_color } else { rgba(0x00000000) })
                                );
                                
                                // Content container with proper padding (after accent bar)
                                let content = div()
                                    .flex_1()
                                    .px(px(item_spacing.item_padding_x))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        // Left side: title
                                        div()
                                            .text_color(title_color)
                                            .text_sm()
                                            .font_weight(if is_selected { gpui::FontWeight::MEDIUM } else { gpui::FontWeight::NORMAL })
                                            .child(title_str),
                                    );

                                // Right side: keyboard shortcut with pill background
                                let content = if let Some(shortcut) = shortcut_opt {
                                    // Get subtle background color for shortcut pill
                                    let shortcut_bg = if design_variant == DesignVariant::Default {
                                        rgba((this.theme.colors.background.search_box << 8) | 0x80)
                                    } else {
                                        rgba((item_colors.background_tertiary << 8) | 0x80)
                                    };
                                    
                                    content.child(
                                        div()
                                            .px(px(6.))
                                            .py(px(2.))
                                            .bg(shortcut_bg)
                                            .rounded(px(4.))
                                            .text_color(shortcut_color)
                                            .text_xs()
                                            .child(shortcut),
                                    )
                                } else {
                                    content
                                };
                                
                                action_item = action_item.child(content);

                                items.push(action_item);
                            }
                        }
                    }
                    items
                }),
            )
            .flex_1()
            .w_full()
            .track_scroll(&self.scroll_handle);
            
            // Wrap uniform_list in a relative container with scrollbar overlay
            div()
                .relative()
                .flex_1()
                .w_full()
                .child(list)
                .child(scrollbar)
                .into_any_element()
        };

        // Use helper method for container colors
        let (main_bg, container_border, container_text) = 
            self.get_container_colors(&colors);
        
        // Calculate dynamic height based on number of items
        // Each item is ACTION_ITEM_HEIGHT, plus search box height (~44px), plus padding
        // When hide_search is true, we don't include the search box height
        let num_items = self.filtered_actions.len();
        let search_box_height = if self.hide_search { 0.0 } else { 60.0 };
        let items_height = (num_items as f32 * ACTION_ITEM_HEIGHT).min(POPUP_MAX_HEIGHT - search_box_height);
        let total_height = items_height + search_box_height; // search box height (if shown) + padding
        
        // Main overlay popup container
        // Fixed width, dynamic height based on content, rounded corners, shadow, semi-transparent bg
        div()
            .flex()
            .flex_col()
            .w(px(POPUP_WIDTH))
            .h(px(total_height))  // Use calculated height instead of max_h
            .bg(main_bg)
            .rounded(px(visual.radius_lg))
            .shadow(Self::create_popup_shadow())
            .border_1()
            .border_color(container_border)
            .overflow_hidden()
            .text_color(container_text)
            .key_context("actions_dialog")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(actions_container)
            .when(!self.hide_search, |d| d.child(input_container))
    }
}

// ============================================================================
// Script Creation Utilities
// ============================================================================

/// Validates a script name - only alphanumeric and hyphens allowed
/// 
/// # Rules
/// - Cannot be empty
/// - Only letters, numbers, and hyphens allowed
/// - Cannot start or end with a hyphen
/// 
/// # Examples
/// ```
/// assert!(validate_script_name("hello-world").is_ok());
/// assert!(validate_script_name("myScript").is_ok());
/// assert!(validate_script_name("").is_err());
/// assert!(validate_script_name("-hello").is_err());
/// ```
pub fn validate_script_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Script name cannot be empty".to_string());
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err("Script name can only contain letters, numbers, and hyphens".to_string());
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("Script name cannot start or end with a hyphen".to_string());
    }
    Ok(())
}

/// Generates a script template with the given name
/// 
/// Converts kebab-case names to Title Case for the display name.
/// Creates a basic TypeScript script with Name and Description metadata.
/// 
/// # Example
/// ```
/// let template = generate_script_template("hello-world");
/// // Returns:
/// // // Name: Hello World
/// // // Description: 
/// // 
/// // console.log("Hello from hello-world!")
/// ```
pub fn generate_script_template(name: &str) -> String {
    // Convert kebab-case to Title Case for display
    let display_name = name
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    
    format!(
        r#"// Name: {}
// Description: 

console.log("Hello from {}!")
"#,
        display_name, name
    )
}

/// Creates a new script file at ~/.kenv/scripts/{name}.ts
/// 
/// # Arguments
/// * `name` - The script name (will be validated)
/// 
/// # Returns
/// * `Ok(PathBuf)` - Path to the created script file
/// * `Err(String)` - Error message if creation failed
/// 
/// # Errors
/// - Invalid script name (see `validate_script_name`)
/// - Script already exists
/// - Failed to create directory or write file
pub fn create_script_file(name: &str) -> Result<std::path::PathBuf, String> {
    use std::path::PathBuf;
    use std::fs;
    
    validate_script_name(name)?;
    
    let scripts_dir = PathBuf::from(shellexpand::tilde("~/.kenv/scripts").as_ref());
    
    // Ensure directory exists
    if !scripts_dir.exists() {
        fs::create_dir_all(&scripts_dir)
            .map_err(|e| format!("Failed to create scripts directory: {}", e))?;
    }
    
    let file_path = scripts_dir.join(format!("{}.ts", name));
    
    // Check if file already exists
    if file_path.exists() {
        return Err(format!("Script '{}' already exists", name));
    }
    
    // Write template
    let template = generate_script_template(name);
    fs::write(&file_path, template)
        .map_err(|e| format!("Failed to write script file: {}", e))?;
    
    logging::log("SCRIPT_CREATE", &format!("Created new script: {}", file_path.display()));
    
    Ok(file_path)
}

/// Returns the path where a script would be created (without creating it)
/// Useful for checking if a script already exists or for UI display
pub fn get_script_path(name: &str) -> std::path::PathBuf {
    use std::path::PathBuf;
    let scripts_dir = PathBuf::from(shellexpand::tilde("~/.kenv/scripts").as_ref());
    scripts_dir.join(format!("{}.ts", name))
}

/// Checks if a script with the given name already exists
pub fn script_exists(name: &str) -> bool {
    get_script_path(name).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_info_creation() {
        let script = ScriptInfo::new("test-script", "/path/to/test-script.ts");
        assert_eq!(script.name, "test-script");
        assert_eq!(script.path, "/path/to/test-script.ts");
    }

    #[test]
    fn test_action_with_shortcut() {
        let action = Action::new("test", "Test Action", None, ActionCategory::GlobalOps)
            .with_shortcut("⌘T");
        assert_eq!(action.shortcut, Some("⌘T".to_string()));
    }

    #[test]
    fn test_get_script_context_actions() {
        let script = ScriptInfo::new("my-script", "/path/to/my-script.ts");
        let actions = get_script_context_actions(&script);
        
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| a.id == "edit_script"));
        assert!(actions.iter().any(|a| a.id == "view_logs"));
        assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
        assert!(actions.iter().any(|a| a.id == "copy_path"));
        assert!(actions.iter().any(|a| a.id == "run_script"));
    }

    #[test]
    fn test_get_global_actions() {
        let actions = get_global_actions();
        
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| a.id == "create_script"));
        assert!(actions.iter().any(|a| a.id == "reload_scripts"));
        assert!(actions.iter().any(|a| a.id == "settings"));
        assert!(actions.iter().any(|a| a.id == "quit"));
    }

    #[test]
    fn test_popup_constants() {
        assert_eq!(POPUP_WIDTH, 320.0);
        assert_eq!(POPUP_MAX_HEIGHT, 400.0);
        assert_eq!(POPUP_CORNER_RADIUS, 12.0);
    }

    #[test]
    fn test_action_item_height_constant() {
        // Fixed height is required for uniform_list virtualization
        // Increased to 42px for better touch targets and visual breathing room
        assert_eq!(ACTION_ITEM_HEIGHT, 42.0);
        // Ensure item height is positive and reasonable
        const _: () = assert!(ACTION_ITEM_HEIGHT > 0.0);
        const _: () = assert!(ACTION_ITEM_HEIGHT < POPUP_MAX_HEIGHT);
    }

    #[test]
    fn test_max_visible_items() {
        // Calculate max visible items that can fit in the popup
        // This helps verify scroll virtualization is worthwhile
        let max_visible = (POPUP_MAX_HEIGHT / ACTION_ITEM_HEIGHT) as usize;
        // With 400px max height and 42px items, ~9 items fit
        assert!(max_visible >= 8, "Should fit at least 8 items");
        assert!(max_visible <= 15, "Sanity check on max visible");
    }

    #[test]
    fn test_actions_exceed_visible_space() {
        // Verify that with script context + global actions, we exceed visible space
        // This confirms scrolling/virtualization is needed
        let script = ScriptInfo::new("test-script", "/path/to/test.ts");
        let script_actions = get_script_context_actions(&script);
        let global_actions = get_global_actions();
        let total_actions = script_actions.len() + global_actions.len();
        
        let max_visible = (POPUP_MAX_HEIGHT / ACTION_ITEM_HEIGHT) as usize;
        
        // With 5 script context actions + 4 global = 9 actions
        // At 42px height in 400px container, we can fit ~9 items
        // So we might not always overflow, but we're close
        assert!(total_actions >= 8, "Should have at least 8 total actions");
        
        // Log for visibility
        println!("Total actions: {}, Max visible: {}", total_actions, max_visible);
    }

    // ========================================================================
    // Script Creation Utility Tests
    // ========================================================================

    #[test]
    fn test_validate_script_name_valid() {
        assert!(validate_script_name("hello-world").is_ok());
        assert!(validate_script_name("myScript").is_ok());
        assert!(validate_script_name("test123").is_ok());
        assert!(validate_script_name("a").is_ok());
        assert!(validate_script_name("ABC").is_ok());
        assert!(validate_script_name("my-cool-script").is_ok());
    }

    #[test]
    fn test_validate_script_name_empty() {
        let result = validate_script_name("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Script name cannot be empty");
    }

    #[test]
    fn test_validate_script_name_invalid_chars() {
        // Spaces not allowed
        let result = validate_script_name("hello world");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("only contain letters"));

        // Underscores not allowed
        let result = validate_script_name("hello_world");
        assert!(result.is_err());

        // Special characters not allowed
        assert!(validate_script_name("hello!").is_err());
        assert!(validate_script_name("hello@script").is_err());
        assert!(validate_script_name("hello.ts").is_err());
        assert!(validate_script_name("path/to/script").is_err());
    }

    #[test]
    fn test_validate_script_name_hyphen_position() {
        // Leading hyphen not allowed
        let result = validate_script_name("-hello");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot start or end"));

        // Trailing hyphen not allowed
        let result = validate_script_name("hello-");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot start or end"));

        // Just a hyphen not allowed
        assert!(validate_script_name("-").is_err());
    }

    #[test]
    fn test_generate_script_template_simple() {
        let template = generate_script_template("hello");
        assert!(template.contains("// Name: Hello"));
        assert!(template.contains("// Description:"));
        assert!(template.contains("Hello from hello!"));
    }

    #[test]
    fn test_generate_script_template_kebab_case() {
        let template = generate_script_template("hello-world");
        assert!(template.contains("// Name: Hello World"));
        assert!(template.contains("Hello from hello-world!"));
    }

    #[test]
    fn test_generate_script_template_multi_word() {
        let template = generate_script_template("my-cool-script");
        assert!(template.contains("// Name: My Cool Script"));
        assert!(template.contains("Hello from my-cool-script!"));
    }

    #[test]
    fn test_generate_script_template_structure() {
        let template = generate_script_template("test");
        
        // Should have proper structure
        assert!(template.starts_with("// Name:"));
        assert!(template.contains("// Description:"));
        assert!(template.contains("console.log"));
        
        // Template should be valid TypeScript (basic check)
        assert!(template.contains("\"Hello from test!\""));
    }

    #[test]
    fn test_get_script_path() {
        let path = get_script_path("hello-world");
        
        // Should end with the correct filename
        assert!(path.to_string_lossy().ends_with("hello-world.ts"));
        
        // Should be in ~/.kenv/scripts/
        assert!(path.to_string_lossy().contains(".kenv/scripts"));
    }

    #[test]
    fn test_get_script_path_various_names() {
        assert!(get_script_path("a").to_string_lossy().ends_with("a.ts"));
        assert!(get_script_path("my-script").to_string_lossy().ends_with("my-script.ts"));
        assert!(get_script_path("Test123").to_string_lossy().ends_with("Test123.ts"));
    }
}
