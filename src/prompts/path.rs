//! PathPrompt - File/folder picker prompt
//!
//! Features:
//! - Browse file system starting from optional path
//! - Filter files/folders by name
//! - Navigate with keyboard
//! - Submit selected path

use gpui::{
    div, prelude::*, uniform_list, Context, FocusHandle, Focusable, Render,
    UniformListScrollHandle, Window,
};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::components::{
    PromptContainer, PromptContainerColors, PromptContainerConfig, PromptHeader,
    PromptHeaderColors, PromptHeaderConfig,
};
use crate::designs::{get_tokens, DesignVariant};
use crate::list_item::{IconKind, ListItem, ListItemColors};
use crate::logging;
use crate::theme;

/// Callback for prompt submission
/// Signature: (id: String, value: Option<String>)
pub type SubmitCallback = Arc<dyn Fn(String, Option<String>) + Send + Sync>;

/// Information about a file/folder path for context-aware actions
/// Used for path-specific actions in the actions dialog
#[derive(Debug, Clone)]
pub struct PathInfo {
    /// Display name of the file/folder
    pub name: String,
    /// Full path to the file/folder
    pub path: String,
    /// Whether this is a directory
    pub is_dir: bool,
}

impl PathInfo {
    pub fn new(name: impl Into<String>, path: impl Into<String>, is_dir: bool) -> Self {
        PathInfo {
            name: name.into(),
            path: path.into(),
            is_dir,
        }
    }
}

/// Callback for showing actions dialog
/// Signature: (path_info: PathInfo)
pub type ShowActionsCallback = Arc<dyn Fn(PathInfo) + Send + Sync>;

/// Callback for closing actions dialog (toggle behavior)
/// Signature: ()
pub type CloseActionsCallback = Arc<dyn Fn() + Send + Sync>;

/// PathPrompt - File/folder picker
///
/// Provides a file browser interface for selecting files or directories.
/// Supports starting from a specified path and filtering by name.
pub struct PathPrompt {
    /// Unique ID for this prompt instance
    pub id: String,
    /// Starting directory path (defaults to home if None)
    pub start_path: Option<String>,
    /// Hint text to display
    pub hint: Option<String>,
    /// Current directory being browsed
    pub current_path: String,
    /// Filter text for narrowing down results
    pub filter_text: String,
    /// Currently selected index in the list
    pub selected_index: usize,
    /// List of entries in current directory
    pub entries: Vec<PathEntry>,
    /// Filtered entries based on filter_text
    pub filtered_entries: Vec<PathEntry>,
    /// Focus handle for keyboard input
    pub focus_handle: FocusHandle,
    /// Callback when user submits a selection
    pub on_submit: SubmitCallback,
    /// Theme for styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling
    pub design_variant: DesignVariant,
    /// Scroll handle for the list
    pub list_scroll_handle: UniformListScrollHandle,
    /// Optional callback to show actions dialog
    pub on_show_actions: Option<ShowActionsCallback>,
    /// Optional callback to close actions dialog (for toggle behavior)
    pub on_close_actions: Option<CloseActionsCallback>,
    /// Shared state tracking if actions dialog is currently showing
    /// Used by PathPrompt to implement toggle behavior for Cmd+K
    pub actions_showing: Arc<Mutex<bool>>,
    /// Shared state for actions search text (displayed in header when actions showing)
    pub actions_search_text: Arc<Mutex<String>>,
    /// Whether to show blinking cursor (for focused state)
    pub cursor_visible: bool,
}

/// A file system entry (file or directory)
#[derive(Clone, Debug)]
pub struct PathEntry {
    /// Display name
    pub name: String,
    /// Full path
    pub path: String,
    /// Whether this is a directory
    pub is_dir: bool,
}

impl PathPrompt {
    pub fn new(
        id: String,
        start_path: Option<String>,
        hint: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let current_path = start_path.clone().unwrap_or_else(|| {
            dirs::home_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/".to_string())
        });

        logging::log(
            "PROMPTS",
            &format!("PathPrompt::new starting at: {}", current_path),
        );

        // Load entries from current path
        let entries = Self::load_entries(&current_path);
        let filtered_entries = entries.clone();

        PathPrompt {
            id,
            start_path,
            hint,
            current_path,
            filter_text: String::new(),
            selected_index: 0,
            entries,
            filtered_entries,
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
            list_scroll_handle: UniformListScrollHandle::new(),
            on_show_actions: None,
            on_close_actions: None,
            actions_showing: Arc::new(Mutex::new(false)),
            actions_search_text: Arc::new(Mutex::new(String::new())),
            cursor_visible: true,
        }
    }

    /// Set the callback for showing actions dialog
    pub fn with_show_actions(mut self, callback: ShowActionsCallback) -> Self {
        self.on_show_actions = Some(callback);
        self
    }

    /// Set the show actions callback (mutable version)
    pub fn set_show_actions(&mut self, callback: ShowActionsCallback) {
        self.on_show_actions = Some(callback);
    }

    /// Set the close actions callback (for toggle behavior)
    pub fn with_close_actions(mut self, callback: CloseActionsCallback) -> Self {
        self.on_close_actions = Some(callback);
        self
    }

    /// Set the shared actions_showing state (for toggle behavior)
    pub fn with_actions_showing(mut self, actions_showing: Arc<Mutex<bool>>) -> Self {
        self.actions_showing = actions_showing;
        self
    }

    /// Set the shared actions_search_text state (for header display)
    pub fn with_actions_search_text(mut self, actions_search_text: Arc<Mutex<String>>) -> Self {
        self.actions_search_text = actions_search_text;
        self
    }

    /// Load directory entries from a path
    fn load_entries(dir_path: &str) -> Vec<PathEntry> {
        let path = Path::new(dir_path);
        let mut entries = Vec::new();

        // No ".." entry - use left arrow to navigate to parent

        // Read directory entries
        if let Ok(read_dir) = std::fs::read_dir(path) {
            let mut dirs: Vec<PathEntry> = Vec::new();
            let mut files: Vec<PathEntry> = Vec::new();

            for entry in read_dir.flatten() {
                let entry_path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files (starting with .)
                if name.starts_with('.') {
                    continue;
                }

                let is_dir = entry_path.is_dir();
                let path_entry = PathEntry {
                    name,
                    path: entry_path.to_string_lossy().to_string(),
                    is_dir,
                };

                if is_dir {
                    dirs.push(path_entry);
                } else {
                    files.push(path_entry);
                }
            }

            // Sort alphabetically (case insensitive)
            dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            // Add dirs first, then files
            entries.extend(dirs);
            entries.extend(files);
        }

        logging::log(
            "PROMPTS",
            &format!(
                "PathPrompt loaded {} entries from {}",
                entries.len(),
                dir_path
            ),
        );
        entries
    }

    /// Update filtered entries based on filter text
    fn update_filtered(&mut self) {
        if self.filter_text.is_empty() {
            self.filtered_entries = self.entries.clone();
        } else {
            let filter_lower = self.filter_text.to_lowercase();
            self.filtered_entries = self
                .entries
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect();
        }

        // Reset selection to 0 if out of bounds
        if self.selected_index >= self.filtered_entries.len() {
            self.selected_index = 0;
        }
    }

    /// Navigate into a directory
    pub fn navigate_to(&mut self, path: &str, cx: &mut Context<Self>) {
        self.current_path = path.to_string();
        self.entries = Self::load_entries(path);
        self.filter_text.clear();
        self.filtered_entries = self.entries.clone();
        self.selected_index = 0;
        cx.notify();
    }

    /// Show actions dialog for the selected entry
    fn show_actions(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.filtered_entries.get(self.selected_index) {
            if let Some(ref callback) = self.on_show_actions {
                let path_info = PathInfo::new(entry.name.clone(), entry.path.clone(), entry.is_dir);
                logging::log(
                    "PROMPTS",
                    &format!(
                        "PathPrompt showing actions for: {} (is_dir={})",
                        path_info.path, path_info.is_dir
                    ),
                );
                (callback)(path_info);
                // Trigger re-render to show ActionsDialog
                cx.notify();
            }
        }
    }

    /// Close actions dialog (for toggle behavior)
    fn close_actions(&mut self, cx: &mut Context<Self>) {
        if let Some(ref callback) = self.on_close_actions {
            logging::log("PROMPTS", "PathPrompt closing actions dialog");
            (callback)();
            cx.notify();
        }
    }

    /// Toggle actions dialog - show if hidden, close if showing
    pub fn toggle_actions(&mut self, cx: &mut Context<Self>) {
        let is_showing = self.actions_showing.lock().map(|g| *g).unwrap_or(false);

        if is_showing {
            logging::log(
                "PROMPTS",
                "PathPrompt toggle: closing actions (was showing)",
            );
            self.close_actions(cx);
        } else {
            logging::log("PROMPTS", "PathPrompt toggle: showing actions (was hidden)");
            self.show_actions(cx);
        }
    }

    /// Submit the selected path - always submits, never navigates
    /// For files and directories: submit the path (script will handle it)
    /// Navigation into directories is handled by → and Tab keys
    fn submit_selected(&mut self, _cx: &mut Context<Self>) {
        if let Some(entry) = self.filtered_entries.get(self.selected_index) {
            // Always submit the path, whether it's a file or directory
            // The calling script or default handler will decide what to do with it
            logging::log(
                "PROMPTS",
                &format!(
                    "PathPrompt submitting path: {} (is_dir={})",
                    entry.path, entry.is_dir
                ),
            );
            (self.on_submit)(self.id.clone(), Some(entry.path.clone()));
        } else if !self.filter_text.is_empty() {
            // If no entry selected but filter has text, submit the filter as a path
            logging::log(
                "PROMPTS",
                &format!(
                    "PathPrompt submitting filter text as path: {}",
                    self.filter_text
                ),
            );
            (self.on_submit)(self.id.clone(), Some(self.filter_text.clone()));
        }
    }

    /// Handle Enter key - always submit the selected path
    /// The calling code (main.rs) will open it with system default via std::process::Command
    pub fn handle_enter(&mut self, cx: &mut Context<Self>) {
        // Always submit directly - no actions dialog on Enter
        // Actions are available via Cmd+K
        self.submit_selected(cx);
    }

    /// Cancel - submit None
    pub fn submit_cancel(&mut self) {
        logging::log(
            "PROMPTS",
            &format!(
                "PathPrompt submit_cancel called - submitting None for id: {}",
                self.id
            ),
        );
        (self.on_submit)(self.id.clone(), None);
    }

    /// Move selection up
    pub fn move_up(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.list_scroll_handle
                .scroll_to_item(self.selected_index, gpui::ScrollStrategy::Top);
            cx.notify();
        }
    }

    /// Move selection down
    pub fn move_down(&mut self, cx: &mut Context<Self>) {
        if self.selected_index < self.filtered_entries.len().saturating_sub(1) {
            self.selected_index += 1;
            self.list_scroll_handle
                .scroll_to_item(self.selected_index, gpui::ScrollStrategy::Top);
            cx.notify();
        }
    }

    /// Handle character input
    fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        self.filter_text.push(ch);
        self.update_filtered();
        cx.notify();
    }

    /// Handle backspace - if filter empty, go up one directory
    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.filter_text.is_empty() {
            self.filter_text.pop();
            self.update_filtered();
            cx.notify();
        } else {
            // If filter is empty, navigate up one directory
            let path = Path::new(&self.current_path);
            if let Some(parent) = path.parent() {
                let parent_path = parent.to_string_lossy().to_string();
                self.navigate_to(&parent_path, cx);
            }
        }
    }

    /// Navigate to parent directory (left arrow / shift+tab)
    pub fn navigate_to_parent(&mut self, cx: &mut Context<Self>) {
        let path = Path::new(&self.current_path);
        if let Some(parent) = path.parent() {
            let parent_path = parent.to_string_lossy().to_string();
            logging::log(
                "PROMPTS",
                &format!("PathPrompt navigating to parent: {}", parent_path),
            );
            self.navigate_to(&parent_path, cx);
        }
        // If at root, do nothing
    }

    /// Navigate into selected directory (right arrow / tab)
    pub fn navigate_into_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.filtered_entries.get(self.selected_index) {
            if entry.is_dir {
                let path = entry.path.clone();
                logging::log("PROMPTS", &format!("PathPrompt navigating into: {}", path));
                self.navigate_to(&path, cx);
            }
            // If selected entry is a file, do nothing
        }
    }

    /// Get the currently selected path info (for actions dialog)
    pub fn get_selected_path_info(&self) -> Option<PathInfo> {
        self.filtered_entries
            .get(self.selected_index)
            .map(|entry| PathInfo::new(entry.name.clone(), entry.path.clone(), entry.is_dir))
    }
}

impl Focusable for PathPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PathPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = get_tokens(self.design_variant);
        let design_colors = tokens.colors();

        let handle_key = cx.listener(
            |this: &mut Self,
             event: &gpui::KeyDownEvent,
             _window: &mut Window,
             cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                let has_cmd = event.keystroke.modifiers.platform;

                // Check if actions dialog is showing - if so, don't handle most keys
                // The ActionsDialog has its own key handler and will handle them
                let actions_showing = this.actions_showing.lock().map(|g| *g).unwrap_or(false);

                // Cmd+K always toggles actions (whether showing or not)
                if has_cmd && key_str == "k" {
                    this.toggle_actions(cx);
                    return;
                }

                // When actions are showing, let the ActionsDialog handle all other keys
                // The ActionsDialog is focused and has its own on_key_down handler
                if actions_showing {
                    // Don't handle any other keys - let them bubble to ActionsDialog
                    return;
                }

                match key_str.as_str() {
                    "up" | "arrowup" => this.move_up(cx),
                    "down" | "arrowdown" => this.move_down(cx),
                    "left" | "arrowleft" => this.navigate_to_parent(cx),
                    "right" | "arrowright" => this.navigate_into_selected(cx),
                    "tab" => {
                        if event.keystroke.modifiers.shift {
                            this.navigate_to_parent(cx);
                        } else {
                            this.navigate_into_selected(cx);
                        }
                    }
                    "enter" => this.handle_enter(cx),
                    "escape" => {
                        logging::log(
                            "PROMPTS",
                            "PathPrompt: Escape key pressed - calling submit_cancel()",
                        );
                        this.submit_cancel();
                    }
                    "backspace" => this.handle_backspace(cx),
                    _ => {
                        if let Some(ref key_char) = event.keystroke.key_char {
                            if let Some(ch) = key_char.chars().next() {
                                if !ch.is_control() {
                                    this.handle_char(ch, cx);
                                }
                            }
                        }
                    }
                }
            },
        );

        // Use ListItemColors for consistent theming
        let list_colors = if self.design_variant == DesignVariant::Default {
            ListItemColors::from_theme(&self.theme)
        } else {
            ListItemColors::from_design(&design_colors)
        };

        // Clone values needed for the closure
        let filtered_count = self.filtered_entries.len();
        let selected_index = self.selected_index;

        // Clone entries for the closure (uniform_list callback doesn't have access to self)
        let entries_for_list: Vec<(String, bool)> = self
            .filtered_entries
            .iter()
            .map(|e| (e.name.clone(), e.is_dir))
            .collect();

        // Build list items using ListItem component for consistent styling
        let list = uniform_list(
            "path-list",
            filtered_count,
            move |visible_range: std::ops::Range<usize>, _window, _cx| {
                visible_range
                    .map(|ix| {
                        let (name, is_dir) = &entries_for_list[ix];
                        let is_selected = ix == selected_index;

                        // Choose icon based on entry type
                        let icon = if *is_dir {
                            IconKind::Emoji("📁".to_string())
                        } else {
                            IconKind::Emoji("📄".to_string())
                        };

                        // No description needed - folder icon 📁 is sufficient
                        let description: Option<String> = None;

                        // Use ListItem component for consistent styling with main menu
                        ListItem::new(name.clone(), list_colors)
                            .index(ix)
                            .icon_kind(icon)
                            .description_opt(description)
                            .selected(is_selected)
                            .with_accent_bar(true)
                            .into_any_element()
                    })
                    .collect()
            },
        )
        .track_scroll(&self.list_scroll_handle)
        .flex_1()
        .w_full();

        // Get entity handles for click callbacks
        let handle_select = cx.entity().downgrade();
        let handle_actions = cx.entity().downgrade();

        // Check if actions are currently showing (for CLS-free toggle)
        let show_actions = self.actions_showing.lock().map(|g| *g).unwrap_or(false);

        // Get actions search text from shared state
        let actions_search_text = self
            .actions_search_text
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();

        // Create path prefix for display in search input
        let path_prefix = format!("{}/", self.current_path.trim_end_matches('/'));

        // Create header colors and config using shared components
        let header_colors = if self.design_variant == DesignVariant::Default {
            PromptHeaderColors::from_theme(&self.theme)
        } else {
            PromptHeaderColors::from_design(&design_colors)
        };

        let header_config = PromptHeaderConfig::new()
            .filter_text(self.filter_text.clone())
            .placeholder("Type to filter...")
            .path_prefix(Some(path_prefix))
            .primary_button_label("Select")
            .primary_button_shortcut("↵")
            .show_actions_button(true)
            .cursor_visible(self.cursor_visible)
            .actions_mode(show_actions)
            .actions_search_text(actions_search_text)
            .focused(!show_actions);

        let header = PromptHeader::new(header_config, header_colors)
            .on_primary_click(Box::new(move |_, _window, cx| {
                logging::log("CLICK", "PathPrompt primary button (Select) clicked");
                if let Some(prompt) = handle_select.upgrade() {
                    prompt.update(cx, |this, cx| {
                        this.submit_selected(cx);
                    });
                }
            }))
            .on_actions_click(Box::new(move |_, _window, cx| {
                logging::log("CLICK", "PathPrompt actions button clicked");
                if let Some(prompt) = handle_actions.upgrade() {
                    prompt.update(cx, |this, cx| {
                        this.toggle_actions(cx);
                    });
                }
            }));

        // Create hint text for footer
        let hint_text = self.hint.clone().unwrap_or_else(|| {
            format!("{} items • ↑↓ navigate • ←→ in/out • Enter open • Tab into • ⌘K actions • Esc cancel", filtered_count)
        });

        // Create container colors and config
        let container_colors = if self.design_variant == DesignVariant::Default {
            PromptContainerColors::from_theme(&self.theme)
        } else {
            PromptContainerColors::from_design(&design_colors)
        };

        let container_config = PromptContainerConfig::new()
            .show_divider(true)
            .hint(hint_text);

        // Build the final container with the outer wrapper for key handling and focus
        div()
            .id(gpui::ElementId::Name("window:path".into()))
            .w_full()
            .h_full()
            .key_context("path_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(
                PromptContainer::new(container_colors)
                    .config(container_config)
                    .header(header)
                    .content(list),
            )
    }
}
