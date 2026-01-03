# Actions Panel Architecture & UX Expert Bundle

## Executive Summary

This bundle provides a comprehensive view of the Actions panel system in Script Kit GPUI - a context-aware overlay popup for quick access to script management and path operations. The system uses a `ActionsDialog` component with virtualized scrolling, keyboard navigation, and theme-aware styling.

### Key Components:
1. **ActionsDialog** (`src/actions.rs`) - Main dialog component with search, action list, and category support
2. **PathPrompt Integration** - Path-specific actions for file/folder operations  
3. **Main Menu Integration** - Script-specific actions (edit, run, reveal, etc.)
4. **PromptHeader** - Shared header component with actions mode

### Design Patterns:
- **Cmd+K Toggle** - Opens/closes actions panel from any context
- **Context-aware actions** - Different actions for scripts vs paths vs global
- **Focus management** - Dialog takes focus, Escape returns to previous
- **Virtualized list** - `uniform_list` with fixed 42px item height
- **Theme integration** - Uses design tokens, supports focus-aware colors

### Known Issues (from test files):
1. **Duplicate search box** - When actions panel opens, both header search and dialog search may render
2. **SimulateClick not implemented** - Click-outside dismiss exists but no protocol handler yet
3. **keyboard.tap() not working** - Tests require manual Cmd+K trigger

### Files Included:
- `src/actions.rs`: Core ActionsDialog component (1433 lines)
- `src/prompts/path.rs`: PathPrompt with actions integration (652 lines)
- `src/components/prompt_header.rs`: Shared header with actions mode (593 lines)
- `src/main.rs` (excerpts): Integration, state management, handlers
- `tests/smoke/test-actions-*.ts`: Visual and behavior tests

---

## Source Code

### src/actions.rs (Full)

```rs
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

use crate::components::scrollbar::{Scrollbar, ScrollbarColors};
use crate::designs::{get_tokens, DesignColors, DesignVariant};
use crate::logging;
use crate::theme;
use gpui::{
    div, point, prelude::*, px, rgb, rgba, uniform_list, App, BoxShadow, Context, FocusHandle,
    Focusable, Hsla, Render, ScrollStrategy, SharedString, UniformListScrollHandle, Window,
};
use std::sync::Arc;

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

// Import PathInfo from prompts module (use crate:: for local import)
pub use crate::prompts::PathInfo;

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
    ScriptContext, // Actions specific to the focused script
    ScriptOps,     // Edit, Create, Delete script operations
    GlobalOps,     // Settings, Quit, etc.
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

/// Get actions specific to a file/folder path
pub fn get_path_context_actions(path_info: &PathInfo) -> Vec<Action> {
    let mut actions = vec![
        Action::new(
            "copy_path",
            "Copy Path",
            Some("Copy the full path to clipboard".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧C"),
        Action::new(
            "open_in_finder",
            "Open in Finder",
            Some("Reveal in Finder".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧F"),
        Action::new(
            "open_in_editor",
            "Open in Editor",
            Some("Open in $EDITOR".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E"),
        Action::new(
            "open_in_terminal",
            "Open in Terminal",
            Some("Open terminal at this location".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘T"),
        Action::new(
            "copy_filename",
            "Copy Filename",
            Some("Copy just the filename".to_string()),
            ActionCategory::ScriptContext,
        ),
        Action::new(
            "move_to_trash",
            "Move to Trash",
            Some(format!(
                "Delete {}",
                if path_info.is_dir { "folder" } else { "file" }
            )),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⌫"),
    ];

    // Add directory-specific action for navigating into
    if path_info.is_dir {
        actions.insert(
            0,
            Action::new(
                "open_directory",
                format!("Open \"{}\"", path_info.name),
                Some("Navigate into this directory".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("↵"),
        );
    } else {
        actions.insert(
            0,
            Action::new(
                "select_file",
                format!("Select \"{}\"", path_info.name),
                Some("Submit this file".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("↵"),
        );
    }

    actions
}

/// Get actions specific to the focused script
pub fn get_script_context_actions(script: &ScriptInfo) -> Vec<Action> {
    vec![
        Action::new(
            "run_script",
            format!("Run \"{}\"", script.name),
            Some("Execute this script".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("↵"),
        Action::new(
            "edit_script",
            "Edit Script",
            Some("Open in $EDITOR".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E"),
        Action::new(
            "view_logs",
            "View Logs",
            Some("Show script execution logs".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘L"),
        Action::new(
            "reveal_in_finder",
            "Reveal in Finder",
            Some("Show script file in Finder".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧F"),
        Action::new(
            "copy_path",
            "Copy Path",
            Some("Copy script path to clipboard".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧C"),
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
        )
        .with_shortcut("⌘N"),
        Action::new(
            "reload_scripts",
            "Reload Scripts",
            Some("Refresh the scripts list".to_string()),
            ActionCategory::GlobalOps,
        )
        .with_shortcut("⌘R"),
        Action::new(
            "settings",
            "Settings",
            Some("Configure preferences".to_string()),
            ActionCategory::GlobalOps,
        )
        .with_shortcut("⌘,"),
        Action::new(
            "quit",
            "Quit Script Kit",
            Some("Exit the application".to_string()),
            ActionCategory::GlobalOps,
        )
        .with_shortcut("⌘Q"),
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
    pub script_info: Option<ScriptInfo>,
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling (matches main app design)
    pub design_variant: DesignVariant,
    /// Scroll handle for virtualized action list
    pub list_scroll_handle: UniformListScrollHandle,
    /// Whether cursor should be visible (for blinking animation)
    pub cursor_visible: bool,
    /// Whether to hide the built-in search input (when header provides search)
    pub hide_search: bool,
}

impl ActionsDialog {
    pub fn new(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        script_info: Option<ScriptInfo>,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let mut actions = Vec::new();

        // Add script context actions if a script is focused
        if let Some(ref script) = script_info {
            actions.extend(get_script_context_actions(script));
        }

        // Add global actions
        actions.extend(get_global_actions());

        let filtered_actions: Vec<usize> = (0..actions.len()).collect();

        logging::log(
            "ACTIONS",
            &format!(
                "ActionsDialog created with {} actions, script: {:?}, design: {:?}",
                actions.len(),
                script_info.as_ref().map(|s| &s.name),
                DesignVariant::default()
            ),
        );

        ActionsDialog {
            actions,
            filtered_actions,
            selected_index: 0,
            search_text: String::new(),
            focus_handle,
            on_select,
            script_info,
            theme,
            design_variant: DesignVariant::default(),
            list_scroll_handle: UniformListScrollHandle::new(),
            cursor_visible: true,
            hide_search: false,
        }
    }

    /// Create ActionsDialog for a path (file/folder) with path-specific actions
    pub fn with_path(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        path_info: &PathInfo,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let actions = get_path_context_actions(path_info);
        let filtered_actions: Vec<usize> = (0..actions.len()).collect();

        logging::log(
            "ACTIONS",
            &format!(
                "ActionsDialog created for path: {} (is_dir={}) with {} actions",
                path_info.path,
                path_info.is_dir,
                actions.len()
            ),
        );

        ActionsDialog {
            actions,
            filtered_actions,
            selected_index: 0,
            search_text: String::new(),
            focus_handle,
            on_select,
            script_info: None,
            theme,
            design_variant: DesignVariant::default(),
            list_scroll_handle: UniformListScrollHandle::new(),
            cursor_visible: true,
            hide_search: false,
        }
    }

    /// Create ActionsDialog with script context
    pub fn with_script(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        script_info: Option<ScriptInfo>,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::new(focus_handle, on_select, script_info, theme)
    }

    /// Set cursor visibility (for blinking animation)
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    /// Set whether to hide the built-in search input
    pub fn set_hide_search(&mut self, hide: bool) {
        self.hide_search = hide;
    }

    /// Update the search text (called from external handler)
    pub fn set_search_text(&mut self, text: String) {
        self.search_text = text;
        self.filter_actions();
    }

    /// Handle character input for search
    pub fn handle_char(&mut self, ch: char) {
        self.search_text.push(ch);
        self.filter_actions();
    }

    /// Handle backspace for search
    pub fn handle_backspace(&mut self) {
        self.search_text.pop();
        self.filter_actions();
    }

    /// Filter actions based on search text
    fn filter_actions(&mut self) {
        if self.search_text.is_empty() {
            self.filtered_actions = (0..self.actions.len()).collect();
        } else {
            let search_lower = self.search_text.to_lowercase();
            self.filtered_actions = self
                .actions
                .iter()
                .enumerate()
                .filter(|(_, action)| {
                    action.title.to_lowercase().contains(&search_lower)
                        || action
                            .description
                            .as_ref()
                            .map(|d| d.to_lowercase().contains(&search_lower))
                            .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
        }

        // Reset selection to first item
        self.selected_index = 0;
        // Scroll to top
        self.list_scroll_handle
            .scroll_to_item(0, ScrollStrategy::Top);

        logging::log_debug(
            "ACTIONS",
            &format!(
                "Filtered actions: {} matches for '{}'",
                self.filtered_actions.len(),
                self.search_text
            ),
        );
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.list_scroll_handle
                .scroll_to_item(self.selected_index, ScrollStrategy::Top);
            logging::log_debug(
                "ACTIONS",
                &format!("Selection moved up to {}", self.selected_index),
            );
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index < self.filtered_actions.len().saturating_sub(1) {
            self.selected_index += 1;
            self.list_scroll_handle
                .scroll_to_item(self.selected_index, ScrollStrategy::Top);
            logging::log_debug(
                "ACTIONS",
                &format!("Selection moved down to {}", self.selected_index),
            );
        }
    }

    /// Submit the selected action
    pub fn submit(&self) {
        if let Some(&action_idx) = self.filtered_actions.get(self.selected_index) {
            if let Some(action) = self.actions.get(action_idx) {
                logging::log("ACTIONS", &format!("Submitting action: {}", action.id));
                (self.on_select)(action.id.clone());
            }
        }
    }

    /// Cancel the dialog
    pub fn cancel(&self) {
        logging::log("ACTIONS", "Dialog cancelled");
        (self.on_select)("__cancel__".to_string());
    }

    /// Helper method to check if click is outside dialog bounds
    /// Called from main.rs to handle dismiss-on-click-outside
    pub fn dismiss_on_click_outside(&self) {
        logging::log(
            "ACTIONS",
            "ActionsDialog dismiss-on-click-outside triggered",
        );
        self.cancel();
    }

    /// Helper to convert hex color with alpha to u32 format for rgba()
    fn hex_with_alpha(hex: u32, alpha: u8) -> u32 {
        (hex << 8) | (alpha as u32)
    }

    /// Get colors for the search input area based on design variant
    fn get_search_colors(&self, colors: &DesignColors) -> (Hsla, Hsla, Hsla, Hsla, Hsla) {
        if self.design_variant == DesignVariant::Default {
            // Use theme colors
            (
                rgba(Self::hex_with_alpha(
                    self.theme.colors.background.search_box,
                    0xff,
                )),
                rgba(Self::hex_with_alpha(self.theme.colors.ui.border, 0x60)),
                rgb(self.theme.colors.text.muted),
                rgb(self.theme.colors.text.dimmed),
                rgb(self.theme.colors.text.secondary),
            )
        } else {
            // Use design colors
            (
                rgba(Self::hex_with_alpha(colors.background_secondary, 0xff)),
                rgba(Self::hex_with_alpha(colors.border_subtle, 0x60)),
                rgb(colors.text_muted),
                rgb(colors.text_dimmed),
                rgb(colors.text_secondary),
            )
        }
    }
}

/// Helper to convert hex to rgba with alpha
fn hex_with_alpha(hex: u32, alpha: u8) -> u32 {
    (hex << 8) | (alpha as u32)
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

        // NOTE: Key handling is done by the parent (ScriptListApp in main.rs)
        // which routes all keyboard events to this dialog's methods.
        // We do NOT attach our own on_key_down handler to avoid double-processing.

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
        let input_container = div()
            .w(px(POPUP_WIDTH))
            .min_w(px(POPUP_WIDTH))
            .max_w(px(POPUP_WIDTH))
            .h(px(44.0))
            .min_h(px(44.0))
            .max_h(px(44.0))
            .overflow_hidden()
            .px(px(spacing.item_padding_x))
            .py(px(spacing.item_padding_y + 2.0))
            .bg(search_box_bg)
            .border_t_1()
            .border_color(border_color)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(spacing.gap_md))
            .child(
                // Search icon or indicator - fixed width
                div()
                    .w(px(24.0))
                    .min_w(px(24.0))
                    .text_color(dimmed_text)
                    .text_xs()
                    .child("⌘K"),
            )
            .child(
                // Search input field with focus indicator
                div()
                    .flex_shrink_0()
                    .w(px(240.0))
                    .min_w(px(240.0))
                    .max_w(px(240.0))
                    .h(px(28.0))
                    .min_h(px(28.0))
                    .max_h(px(28.0))
                    .overflow_hidden()
                    .px(px(spacing.padding_sm))
                    .py(px(spacing.padding_xs))
                    .bg(if self.design_variant == DesignVariant::Default {
                        rgba(hex_with_alpha(
                            self.theme.colors.background.main,
                            if self.search_text.is_empty() { 0x20 } else { 0x40 },
                        ))
                    } else {
                        rgba(hex_with_alpha(
                            colors.background,
                            if self.search_text.is_empty() { 0x20 } else { 0x40 },
                        ))
                    })
                    .rounded(px(visual.radius_sm))
                    .border_1()
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
                    .when(self.search_text.is_empty(), |d| {
                        d.child(
                            div()
                                .w(px(2.))
                                .h(px(16.))
                                .mr(px(2.))
                                .rounded(px(1.))
                                .when(self.cursor_visible, |d| d.bg(accent_color)),
                        )
                    })
                    .child(search_display)
                    .when(!self.search_text.is_empty(), |d| {
                        d.child(
                            div()
                                .w(px(2.))
                                .h(px(16.))
                                .ml(px(2.))
                                .rounded(px(1.))
                                .when(self.cursor_visible, |d| d.bg(accent_color)),
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
            let selected_index = self.selected_index;
            let filtered_len = self.filtered_actions.len();
            let design_variant = self.design_variant;

            // Calculate scrollbar parameters
            let search_box_height = if self.hide_search { 0.0 } else { 60.0 };
            let container_height = (filtered_len as f32 * ACTION_ITEM_HEIGHT)
                .min(POPUP_MAX_HEIGHT - search_box_height);
            let visible_items = (container_height / ACTION_ITEM_HEIGHT) as usize;

            let scroll_offset = if selected_index > visible_items.saturating_sub(1) {
                selected_index.saturating_sub(visible_items / 2)
            } else {
                0
            };

            let scrollbar_colors = if self.design_variant == DesignVariant::Default {
                ScrollbarColors::from_theme(&self.theme)
            } else {
                ScrollbarColors::from_design(&colors)
            };

            let scrollbar =
                Scrollbar::new(filtered_len, visible_items, scroll_offset, scrollbar_colors)
                    .container_height(container_height);

            let list = uniform_list(
                "actions-list",
                filtered_len,
                cx.processor(
                    move |this: &mut ActionsDialog, visible_range, _window, _cx| {
                        let item_tokens = get_tokens(design_variant);
                        let item_colors = item_tokens.colors();
                        let item_spacing = item_tokens.spacing();

                        let (selected_bg, hover_bg, primary_text, secondary_text, dimmed_text) =
                            if design_variant == DesignVariant::Default {
                                (
                                    rgba((this.theme.colors.accent.selected_subtle << 8) | 0x80),
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
                        let separator_color = if design_variant == DesignVariant::Default {
                            rgba(hex_with_alpha(this.theme.colors.ui.border, 0x40))
                        } else {
                            rgba(hex_with_alpha(item_colors.border_subtle, 0x40))
                        };

                        for idx in visible_range {
                            if let Some(&action_idx) = this.filtered_actions.get(idx) {
                                if let Some(action) = this.actions.get(action_idx) {
                                    let is_selected = idx == selected_index;

                                    // Check for category separator
                                    let is_category_start = if idx > 0 {
                                        if let Some(&prev_action_idx) =
                                            this.filtered_actions.get(idx - 1)
                                        {
                                            if let Some(prev_action) =
                                                this.actions.get(prev_action_idx)
                                            {
                                                prev_action.category != action.category
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    };

                                    let shortcut_color = dimmed_text;
                                    let shortcut_opt: Option<String> = action.shortcut.clone();

                                    // Build action item
                                    let item = div()
                                        .h(px(ACTION_ITEM_HEIGHT))
                                        .w_full()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            // Category separator line
                                            div()
                                                .h(px(1.0))
                                                .w_full()
                                                .when(is_category_start, |d| d.bg(separator_color)),
                                        )
                                        .child(
                                            // Action row
                                            div()
                                                .flex_1()
                                                .w_full()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .px(px(item_spacing.item_padding_x))
                                                .rounded(px(6.0))
                                                .when(is_selected, |d| d.bg(selected_bg))
                                                .child(
                                                    // Left accent bar
                                                    div()
                                                        .w(px(ACCENT_BAR_WIDTH))
                                                        .h(px(ACTION_ITEM_HEIGHT - 8.0))
                                                        .rounded(px(2.0))
                                                        .when(is_selected, |d| {
                                                            d.bg(rgb(if design_variant
                                                                == DesignVariant::Default
                                                            {
                                                                this.theme.colors.accent.selected
                                                            } else {
                                                                item_colors.accent
                                                            }))
                                                        }),
                                                )
                                                .child(
                                                    // Title and description
                                                    div()
                                                        .flex_1()
                                                        .flex()
                                                        .flex_col()
                                                        .ml(px(item_spacing.gap_sm))
                                                        .overflow_hidden()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(primary_text)
                                                                .child(action.title.clone()),
                                                        )
                                                        .when_some(
                                                            action.description.clone(),
                                                            |d, desc| {
                                                                d.child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(secondary_text)
                                                                        .child(desc),
                                                                )
                                                            },
                                                        ),
                                                )
                                                .child(
                                                    // Keyboard shortcut pill
                                                    if let Some(shortcut) = shortcut_opt {
                                                        let shortcut_bg = rgba(hex_with_alpha(
                                                            if design_variant
                                                                == DesignVariant::Default
                                                            {
                                                                this.theme.colors.ui.border
                                                            } else {
                                                                item_colors.border_subtle
                                                            },
                                                            0x40,
                                                        ));
                                                        div()
                                                            .px(px(6.0))
                                                            .py(px(2.0))
                                                            .bg(shortcut_bg)
                                                            .rounded(px(4.0))
                                                            .text_color(shortcut_color)
                                                            .text_xs()
                                                            .child(shortcut)
                                                            .into_any_element()
                                                    } else {
                                                        div().into_any_element()
                                                    },
                                                ),
                                        );

                                    items.push(item.into_any_element());
                                }
                            }
                        }

                        items
                    },
                ),
            )
            .track_scroll(&self.list_scroll_handle)
            .flex_1()
            .w_full();

            div()
                .flex()
                .flex_row()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .child(list)
                .child(scrollbar)
                .into_any_element()
        };

        // Build the complete dialog
        let dialog = div()
            .w(px(POPUP_WIDTH))
            .max_h(px(POPUP_MAX_HEIGHT))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(POPUP_CORNER_RADIUS))
            .bg(if self.design_variant == DesignVariant::Default {
                rgb(self.theme.colors.background.main)
            } else {
                rgb(colors.background)
            })
            .border_1()
            .border_color(if self.design_variant == DesignVariant::Default {
                rgba(hex_with_alpha(self.theme.colors.ui.border, 0x80))
            } else {
                rgba(hex_with_alpha(colors.border, 0x80))
            })
            .shadow(vec![BoxShadow {
                color: rgba(0x00000060),
                offset: point(px(0.), px(4.)),
                blur_radius: px(16.),
                spread_radius: px(0.),
            }])
            .child(actions_container)
            .when(!self.hide_search, |d| d.child(input_container))
            .key_context("actions_dialog")
            .track_focus(&self.focus_handle);

        dialog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_creation() {
        let action = Action::new("test", "Test Action", None, ActionCategory::GlobalOps);
        assert_eq!(action.id, "test");
        assert_eq!(action.title, "Test Action");
        assert!(action.description.is_none());
        assert!(action.shortcut.is_none());
    }

    #[test]
    fn test_action_with_shortcut() {
        let action =
            Action::new("test", "Test Action", None, ActionCategory::GlobalOps).with_shortcut("⌘T");
        assert_eq!(action.shortcut, Some("⌘T".to_string()));
    }

    #[test]
    fn test_script_context_actions() {
        let script = ScriptInfo::new("hello", "/path/to/hello.ts");
        let actions = get_script_context_actions(&script);
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| a.id == "run_script"));
        assert!(actions.iter().any(|a| a.id == "edit_script"));
    }

    #[test]
    fn test_path_context_actions_file() {
        let path_info = PathInfo::new("file.txt", "/path/to/file.txt", false);
        let actions = get_path_context_actions(&path_info);
        assert!(actions.iter().any(|a| a.id == "select_file"));
        assert!(actions.iter().any(|a| a.id == "copy_path"));
    }

    #[test]
    fn test_path_context_actions_directory() {
        let path_info = PathInfo::new("folder", "/path/to/folder", true);
        let actions = get_path_context_actions(&path_info);
        assert!(actions.iter().any(|a| a.id == "open_directory"));
        assert!(actions.iter().any(|a| a.id == "copy_path"));
    }

    #[test]
    fn test_global_actions() {
        let actions = get_global_actions();
        assert!(actions.iter().any(|a| a.id == "create_script"));
        assert!(actions.iter().any(|a| a.id == "settings"));
        assert!(actions.iter().any(|a| a.id == "quit"));
    }
}
```

### src/main.rs (Key Integration Points)

#### State Management
```rs
// Actions popup overlay state (main.rs lines 1603-1646)
show_actions_popup: bool,
actions_dialog: Option<Entity<ActionsDialog>>,

// Path actions state for PathPrompt integration
pending_path_action: Arc<Mutex<Option<PathInfo>>>,
close_path_actions: Arc<Mutex<bool>>,
path_actions_showing: Arc<Mutex<bool>>,
path_actions_search_text: Arc<Mutex<String>>,
pending_path_action_result: Arc<Mutex<Option<(String, PathInfo)>>>,
```

#### Toggle Actions (Cmd+K Handler)
```rs
fn toggle_actions(&mut self, cx: &mut Context<Self>, window: &mut Window) {
    logging::log("KEY", "Toggling actions popup");
    if self.show_actions_popup {
        // Close - return focus to main filter
        self.show_actions_popup = false;
        self.actions_dialog = None;
        self.focused_input = FocusedInput::MainFilter;
        window.focus(&self.focus_handle, cx);
        logging::log("FOCUS", "Actions closed, focus returned to MainFilter");
    } else {
        // Open - create dialog entity
        self.show_actions_popup = true;
        self.focused_input = FocusedInput::ActionsSearch;
        let script_info = self.get_focused_script_info();

        let theme_arc = std::sync::Arc::new(self.theme.clone());
        let dialog = cx.new(|cx| {
            let focus_handle = cx.focus_handle();
            ActionsDialog::with_script(
                focus_handle,
                std::sync::Arc::new(|_action_id| {}), // Callback handled separately
                script_info,
                theme_arc,
            )
        });

        // Hide the dialog's built-in search input since header already has search
        dialog.update(cx, |d, _| d.set_hide_search(true));

        // Focus the dialog's internal focus handle
        let dialog_focus_handle = dialog.read(cx).focus_handle.clone();
        self.actions_dialog = Some(dialog.clone());
        window.focus(&dialog_focus_handle, cx);
        logging::log("FOCUS", "Actions opened, focus moved to ActionsSearch");
    }
    cx.notify();
}
```

#### Action Handler (Executes Selected Action)
```rs
fn handle_action(&mut self, action_id: String, cx: &mut Context<Self>) {
    logging::log("UI", &format!("Action selected: {}", action_id));
    self.current_view = AppView::ScriptList;

    match action_id.as_str() {
        "create_script" => { /* Opens ~/.sk/kit/scripts/ in Finder */ }
        "run_script" => { self.execute_selected(cx); }
        "view_logs" => { self.toggle_logs(cx); }
        "reveal_in_finder" => { /* Opens Finder at script path */ }
        "copy_path" => { /* Copies path via pbcopy/arboard */ }
        "edit_script" => { /* Opens in configured editor */ }
        "settings" => { /* Opens settings */ }
        "quit" => { cx.quit(); }
        "__cancel__" => { /* Dialog cancelled */ }
        _ => { logging::log("UI", &format!("Unknown action: {}", action_id)); }
    }
    cx.notify();
}
```

#### Path Action Handler
```rs
fn execute_path_action(
    &mut self,
    action_id: &str,
    path_info: &PathInfo,
    path_prompt_entity: &Entity<PathPrompt>,
    cx: &mut Context<Self>,
) {
    match action_id {
        "select_file" | "open_directory" => {
            path_prompt_entity.update(cx, |prompt, cx| {
                if path_info.is_dir && action_id == "open_directory" {
                    prompt.navigate_to(&path_info.path, cx);
                } else {
                    (prompt.on_submit)(prompt.id.clone(), Some(path_info.path.clone()));
                }
            });
        }
        "copy_path" => { /* Copy via pbcopy */ }
        "copy_filename" => { /* Copy filename only */ }
        "open_in_finder" => { /* open -R path */ }
        "open_in_editor" => { /* Open in $EDITOR */ }
        "open_in_terminal" => { /* Open Terminal at path */ }
        "move_to_trash" => { /* Trash file/folder */ }
        _ => {}
    }
}
```

### src/prompts/path.rs (PathPrompt with Actions Integration)

```rs
/// PathPrompt - File/folder picker with actions support
pub struct PathPrompt {
    // ... other fields ...
    
    /// Optional callback to show actions dialog
    pub on_show_actions: Option<ShowActionsCallback>,
    /// Optional callback to close actions dialog (for toggle behavior)
    pub on_close_actions: Option<CloseActionsCallback>,
    /// Shared state tracking if actions dialog is currently showing
    pub actions_showing: Arc<Mutex<bool>>,
    /// Shared state for actions search text (displayed in header)
    pub actions_search_text: Arc<Mutex<String>>,
}

impl PathPrompt {
    /// Toggle actions dialog - show if hidden, close if showing
    pub fn toggle_actions(&mut self, cx: &mut Context<Self>) {
        let is_showing = self.actions_showing.lock().map(|g| *g).unwrap_or(false);
        
        if is_showing {
            self.close_actions(cx);
        } else {
            self.show_actions(cx);
        }
    }
    
    /// Show actions dialog for the selected entry
    fn show_actions(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.filtered_entries.get(self.selected_index) {
            if let Some(ref callback) = self.on_show_actions {
                let path_info = PathInfo::new(
                    entry.name.clone(), 
                    entry.path.clone(), 
                    entry.is_dir
                );
                (callback)(path_info);
                cx.notify();
            }
        }
    }
}

// Key handler in render()
match key_str.as_str() {
    // Cmd+K always toggles actions
    _ if has_cmd && key_str == "k" => {
        this.toggle_actions(cx);
        return;
    }
    // When actions are showing, let ActionsDialog handle all other keys
    _ if actions_showing => return,
    // Normal key handling...
}
```

### src/components/prompt_header.rs (Actions Mode in Header)

```rs
/// Configuration for PromptHeader display
pub struct PromptHeaderConfig {
    /// When true, show actions search input instead of buttons
    pub actions_mode: bool,
    /// Actions search text (when in actions_mode)
    pub actions_search_text: String,
    // ... other fields
}

impl PromptHeaderConfig {
    pub fn actions_mode(mut self, mode: bool) -> Self {
        self.actions_mode = mode;
        self
    }
    
    pub fn actions_search_text(mut self, text: String) -> Self {
        self.actions_search_text = text;
        self
    }
}
```

---

## Test Files

### tests/smoke/test-actions-autonomous.ts

Key findings from this test:
1. **Bug identified**: Duplicate search box appears when actions panel opens
2. **Root cause**: Two search inputs are rendered:
   - Header search input (main.rs:3331-3392) 
   - ActionsDialog search input (actions.rs:504-575)
3. **Fix options**:
   - Pass `hide_search: true` to ActionsDialog (currently done via `set_hide_search(true)`)
   - Remove redundant header search input when dialog is active

### tests/smoke/test-actions-click-outside.ts

Documents that:
- `ActionsDialog::dismiss_on_click_outside()` method exists
- `SimulateClick` protocol message is defined but handler not yet in main.rs
- Tests require manual interaction until SimulateClick is implemented

---

## Implementation Guide

### Step 1: Verify Search Input Deduplication

The dialog's search is hidden via `set_hide_search(true)` in `toggle_actions()`:

```rust
// File: src/main.rs
// Location: toggle_actions() function

// Hide the dialog's built-in search input since header already has search
dialog.update(cx, |d, _| d.set_hide_search(true));
```

The header shows actions search when `actions_mode` is true:
```rust
// File: src/prompts/path.rs or wherever header is created
let header_config = PromptHeaderConfig::new()
    .actions_mode(show_actions)
    .actions_search_text(actions_search_text)
```

### Step 2: Review Focus Flow

```
1. User presses Cmd+K
2. toggle_actions() called
3. ActionsDialog entity created
4. Dialog focus handle obtained
5. window.focus(&dialog_focus_handle, cx)
6. focused_input = FocusedInput::ActionsSearch
7. cx.notify() triggers re-render
```

On close:
```
1. Escape pressed or action selected
2. show_actions_popup = false
3. actions_dialog = None
4. focused_input = FocusedInput::MainFilter
5. window.focus(&self.focus_handle, cx)
```

### Step 3: Verify State Reset on Script Exit

```rust
// File: src/main.rs
// Location: reset_to_script_list() function

// Clear actions popup state (prevents stale actions dialog from persisting)
self.show_actions_popup = false;
self.actions_dialog = None;

// Clear pending path action and close signal
if let Ok(mut guard) = self.pending_path_action.lock() {
    *guard = None;
}
if let Ok(mut guard) = self.close_path_actions.lock() {
    *guard = false;
}
```

### Testing

1. **Manual verification**:
   ```bash
   cargo build && echo '{"type":"run","path":"'$(pwd)'/tests/smoke/test-actions-visual.ts"}' | \
     SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1
   ```
   Then press Cmd+K and verify only ONE search input appears.

2. **Check for duplicate search**:
   - Look in screenshot for two input fields
   - Header should show actions search (when actions mode)
   - Dialog should NOT show its own search (hide_search=true)

3. **Focus behavior**:
   - Cmd+K opens dialog and focuses it
   - Escape closes and returns focus to main filter
   - Arrow keys navigate action list
   - Enter submits selected action

---

## Instructions For The Next AI Agent

You are reading the "Actions Panel Architecture & UX Expert Bundle". This file is self-contained and includes all the context you should assume you have.

Your job:

* Design and describe the minimal, safe changes needed to fully resolve the issues described in the Executive Summary and Key Problems.
* Operate **only** on the files and code snippets included in this bundle. If you need additional files or context, clearly say so.

When you propose changes, follow these rules strictly:

1. Always provide **precise code snippets** that can be copy-pasted directly into the repo.
2. Always include **exact file paths** (e.g. `src/main.rs`) and, when possible, line numbers or a clear description of the location (e.g. "replace the existing `toggle_actions` function").
3. Never describe code changes only in prose. Show the full function or block as it should look **after** the change, or show both "before" and "after" versions.
4. Keep instructions **unmistakable and unambiguous**. A human or tool following your instructions should not need to guess what to do.
5. Assume you cannot see any files outside this bundle. If you must rely on unknown code, explicitly note assumptions and risks.

When you answer, you do not need to restate this bundle. Work directly with the code and instructions it contains and return a clear, step-by-step plan plus exact code edits.
