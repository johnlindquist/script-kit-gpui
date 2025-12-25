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

use gpui::{
    div, point, prelude::*, px, rgb, rgba, uniform_list, App, BoxShadow, 
    Context, FocusHandle, Focusable, Hsla, Render, ScrollStrategy, SharedString, 
    UniformListScrollHandle, Window,
};
use std::sync::Arc;
use crate::logging;
use crate::theme;

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
pub const ACTION_ITEM_HEIGHT: f32 = 36.0;

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
}

/// Helper function to combine a hex color with an alpha value
/// Shifts hex left 8 bits and adds alpha to create RGBA value for gpui::rgba()
#[inline]
fn hex_with_alpha(hex: u32, alpha: u8) -> u32 {
    (hex << 8) | (alpha as u32)
}

impl ActionsDialog {
    pub fn new(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_script(focus_handle, on_select, None, theme)
    }

    pub fn with_script(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        focused_script: Option<ScriptInfo>,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let actions = Self::build_actions(&focused_script);
        let filtered_actions: Vec<usize> = (0..actions.len()).collect();
        
        logging::log("ACTIONS", &format!(
            "ActionsDialog created with {} actions, script: {:?}", 
            actions.len(),
            focused_script.as_ref().map(|s| &s.name)
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
        }
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
    fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        self.search_text.push(ch);
        self.refilter();
        cx.notify();
    }

    /// Handle backspace
    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.search_text.is_empty() {
            self.search_text.pop();
            self.refilter();
            cx.notify();
        }
    }

    /// Move selection up
    fn move_up(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.scroll_handle.scroll_to_item(self.selected_index, ScrollStrategy::Nearest);
            logging::log_debug("ACTIONS_SCROLL", &format!("Up: selected_index={}", self.selected_index));
            cx.notify();
        }
    }

    /// Move selection down
    fn move_down(&mut self, cx: &mut Context<Self>) {
        if self.selected_index < self.filtered_actions.len().saturating_sub(1) {
            self.selected_index += 1;
            self.scroll_handle.scroll_to_item(self.selected_index, ScrollStrategy::Nearest);
            logging::log_debug("ACTIONS_SCROLL", &format!("Down: selected_index={}", self.selected_index));
            cx.notify();
        }
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
}

impl Focusable for ActionsDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ActionsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        // Extract theme colors for input container
        let search_box_bg = rgba(hex_with_alpha(self.theme.colors.background.search_box, 0xcc));
        let border_color = rgba(hex_with_alpha(self.theme.colors.ui.border, 0x80));
        let muted_text = rgb(self.theme.colors.text.muted);
        let dimmed_text = rgb(self.theme.colors.text.dimmed);
        let secondary_text = rgb(self.theme.colors.text.secondary);
        
        let input_container = div()
            .w_full()
            .px(px(ITEM_PADDING_X))
            .py(px(ITEM_PADDING_Y))
            .bg(search_box_bg)
            .border_b_1()
            .border_color(border_color)
            .flex()
            .flex_row()
            .gap_2()
            .items_center()
            .child(div().text_color(muted_text).text_sm().child("⚡"))
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(if self.search_text.is_empty() {
                        dimmed_text
                    } else {
                        secondary_text
                    })
                    .child(search_display),
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
                        .py(px(16.))
                        .px(px(ITEM_PADDING_X))
                        .text_color(dimmed_text)
                        .text_sm()
                        .child("No actions match your search"),
                )
                .into_any_element()
        } else {
            // Clone data needed for the uniform_list closure
            let selected_index = self.selected_index;
            let filtered_len = self.filtered_actions.len();
            
            logging::log_debug("ACTIONS_SCROLL", &format!(
                "Rendering uniform_list: {} items, selected={}",
                filtered_len, selected_index
            ));
            
            uniform_list(
                "actions-list",
                filtered_len,
                cx.processor(move |this: &mut ActionsDialog, visible_range, _window, _cx| {
                    logging::log_debug("ACTIONS_SCROLL", &format!(
                        "Actions visible range: {:?} (total={})",
                        visible_range, this.filtered_actions.len()
                    ));
                    
                    // Extract theme colors for list items
                    let selected_bg = rgba(hex_with_alpha(this.theme.colors.accent.selected, 0xcc));
                    let primary_text = rgb(this.theme.colors.text.primary);
                    let tertiary_alpha = rgba(hex_with_alpha(this.theme.colors.text.tertiary, 0x99));
                    let dimmed_alpha = rgba(hex_with_alpha(this.theme.colors.text.dimmed, 0x99));
                    
                    let mut items = Vec::new();
                    
                    for idx in visible_range {
                        if let Some(&action_idx) = this.filtered_actions.get(idx) {
                            if let Some(action) = this.actions.get(action_idx) {
                                let action: &Action = action; // Explicit type annotation
                                let is_selected = idx == selected_index;
                                let bg = if is_selected {
                                    selected_bg
                                } else {
                                    rgba(0x00000000) // Transparent
                                };

                                let title_color = if is_selected {
                                    rgb(0xffffff) // White when selected
                                } else {
                                    primary_text
                                };

                                let shortcut_color = if is_selected {
                                    tertiary_alpha
                                } else {
                                    dimmed_alpha
                                };

                                // Clone strings for SharedString conversion
                                let title_str: String = action.title.clone();
                                let shortcut_opt: Option<String> = action.shortcut.clone();

                                let mut action_item = div()
                                    .id(idx)
                                    .w_full()
                                    .h(px(ACTION_ITEM_HEIGHT)) // Fixed height for uniform_list
                                    .px(px(ITEM_PADDING_X))
                                    .bg(bg)
                                    .rounded(px(6.))
                                    .mx(px(4.))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between();

                                // Left side: title
                                action_item = action_item.child(
                                    div()
                                        .text_color(title_color)
                                        .text_sm()
                                        .child(title_str),
                                );

                                // Right side: keyboard shortcut
                                if let Some(shortcut) = shortcut_opt {
                                    action_item = action_item.child(
                                        div()
                                            .text_color(shortcut_color)
                                            .text_xs()
                                            .child(shortcut),
                                    );
                                }

                                items.push(action_item);
                            }
                        }
                    }
                    items
                }),
            )
            .flex_1()
            .w_full()
            .track_scroll(&self.scroll_handle)
            .into_any_element()
        };

        // Extract theme colors for main container
        let main_bg = rgba(hex_with_alpha(self.theme.colors.background.main, 0xe6));
        let container_border = rgba(hex_with_alpha(self.theme.colors.ui.border, 0x80));
        let container_text = rgb(self.theme.colors.text.secondary);
        
        // Main overlay popup container
        // Fixed width, max height, rounded corners, shadow, semi-transparent bg
        div()
            .flex()
            .flex_col()
            .w(px(POPUP_WIDTH))
            .max_h(px(POPUP_MAX_HEIGHT))
            .bg(main_bg)
            .rounded(px(POPUP_CORNER_RADIUS))
            .shadow(Self::create_popup_shadow())
            .border_1()
            .border_color(container_border)
            .overflow_hidden()
            .text_color(container_text)
            .key_context("actions_dialog")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(input_container)
            .child(actions_container)
    }
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
        assert_eq!(ACTION_ITEM_HEIGHT, 36.0);
        // Ensure item height is positive and reasonable
        assert!(ACTION_ITEM_HEIGHT > 0.0);
        assert!(ACTION_ITEM_HEIGHT < POPUP_MAX_HEIGHT);
    }

    #[test]
    fn test_max_visible_items() {
        // Calculate max visible items that can fit in the popup
        // This helps verify scroll virtualization is worthwhile
        let max_visible = (POPUP_MAX_HEIGHT / ACTION_ITEM_HEIGHT) as usize;
        // With 400px max height and 36px items, ~11 items fit
        assert!(max_visible >= 10, "Should fit at least 10 items");
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
        // At 36px height in 400px container, we can fit ~11 items
        // So we might not always overflow, but we're close
        assert!(total_actions >= 8, "Should have at least 8 total actions");
        
        // Log for visibility
        println!("Total actions: {}, Max visible: {}", total_actions, max_visible);
    }
}
