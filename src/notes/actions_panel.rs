//! Notes Actions Panel
//!
//! Modal overlay panel triggered by Cmd+K in the Notes window.
//! Provides searchable action list for note operations.
//!
//! ## Actions
//! - New Note (⌘N) - Create a new note
//! - Duplicate Note (⌘D) - Create a copy of the current note
//! - Browse Notes (⌘P) - Open note browser/picker
//! - Find in Note (⌘F) - Search within current note
//! - Copy Note As... (⇧⌘C) - Copy note in a chosen format
//! - Copy Deeplink (⇧⌘D) - Copy a deeplink to the note
//! - Create Quicklink (⇧⌘L) - Copy a quicklink to the note
//! - Export... (⇧⌘E) - Export note content
//! - Move List Item Up (⌃⌘↑) - Reorder notes list (disabled)
//! - Move List Item Down (⌃⌘↓) - Reorder notes list (disabled)
//! - Format... (⇧⌘T) - Formatting commands
//!
//! ## Keyboard Navigation
//! - Arrow Up/Down: Navigate actions
//! - Enter: Execute selected action
//! - Escape: Close panel
//! - Type to search/filter actions

use gpui::{
    div, point, prelude::*, px, uniform_list, App, BoxShadow, Context, FocusHandle, Focusable,
    Hsla, MouseButton, Render, ScrollStrategy, SharedString, UniformListScrollHandle, Window,
};
use gpui_component::{theme::{ActiveTheme, Theme}, Icon, IconName};
use std::sync::Arc;
use tracing::debug;

/// Callback type for action execution
/// The String parameter is the action ID
pub type NotesActionCallback = Arc<dyn Fn(NotesAction) + Send + Sync>;

/// Available actions in the Notes actions panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesAction {
    /// Create a new note
    NewNote,
    /// Duplicate the current note
    DuplicateNote,
    /// Open the note browser/picker
    BrowseNotes,
    /// Search within the current note
    FindInNote,
    /// Copy note content as a formatted export
    CopyNoteAs,
    /// Copy deeplink to the current note
    CopyDeeplink,
    /// Copy quicklink to the current note
    CreateQuicklink,
    /// Export note content
    Export,
    /// Move list item up (disabled placeholder)
    MoveListItemUp,
    /// Move list item down (disabled placeholder)
    MoveListItemDown,
    /// Open formatting commands
    Format,
    /// Enable auto-sizing (window grows/shrinks with content)
    EnableAutoSizing,
    /// Panel was cancelled (Escape pressed)
    Cancel,
}

impl NotesAction {
    /// Get all available actions (excluding Cancel)
    pub fn all() -> &'static [NotesAction] {
        &[
            NotesAction::NewNote,
            NotesAction::DuplicateNote,
            NotesAction::BrowseNotes,
            NotesAction::FindInNote,
            NotesAction::CopyNoteAs,
            NotesAction::CopyDeeplink,
            NotesAction::CreateQuicklink,
            NotesAction::Export,
            NotesAction::MoveListItemUp,
            NotesAction::MoveListItemDown,
            NotesAction::Format,
        ]
    }

    /// Get the display label for this action
    pub fn label(&self) -> &'static str {
        match self {
            NotesAction::NewNote => "New Note",
            NotesAction::DuplicateNote => "Duplicate Note",
            NotesAction::BrowseNotes => "Browse Notes",
            NotesAction::FindInNote => "Find in Note",
            NotesAction::CopyNoteAs => "Copy Note As...",
            NotesAction::CopyDeeplink => "Copy Deeplink",
            NotesAction::CreateQuicklink => "Create Quicklink",
            NotesAction::Export => "Export...",
            NotesAction::MoveListItemUp => "Move List Item Up",
            NotesAction::MoveListItemDown => "Move List Item Down",
            NotesAction::Format => "Format...",
            NotesAction::EnableAutoSizing => "Enable Auto-Sizing",
            NotesAction::Cancel => "Cancel",
        }
    }

    /// Get the keyboard shortcut key (without modifier)
    pub fn shortcut_key(&self) -> &'static str {
        match self {
            NotesAction::NewNote => "N",
            NotesAction::DuplicateNote => "D",
            NotesAction::BrowseNotes => "P",
            NotesAction::FindInNote => "F",
            NotesAction::CopyNoteAs => "C",
            NotesAction::CopyDeeplink => "D",
            NotesAction::CreateQuicklink => "L",
            NotesAction::Export => "E",
            NotesAction::MoveListItemUp => "↑",
            NotesAction::MoveListItemDown => "↓",
            NotesAction::Format => "T",
            NotesAction::EnableAutoSizing => "A",
            NotesAction::Cancel => "Esc",
        }
    }

    /// Get shortcut keys for keycap rendering
    pub fn shortcut_keys(&self) -> &'static [&'static str] {
        const CMD_N: [&str; 2] = ["⌘", "N"];
        const CMD_D: [&str; 2] = ["⌘", "D"];
        const CMD_P: [&str; 2] = ["⌘", "P"];
        const CMD_F: [&str; 2] = ["⌘", "F"];
        const SHIFT_CMD_C: [&str; 3] = ["⇧", "⌘", "C"];
        const SHIFT_CMD_D: [&str; 3] = ["⇧", "⌘", "D"];
        const SHIFT_CMD_L: [&str; 3] = ["⇧", "⌘", "L"];
        const SHIFT_CMD_E: [&str; 3] = ["⇧", "⌘", "E"];
        const CTRL_CMD_UP: [&str; 3] = ["⌃", "⌘", "↑"];
        const CTRL_CMD_DOWN: [&str; 3] = ["⌃", "⌘", "↓"];
        const SHIFT_CMD_T: [&str; 3] = ["⇧", "⌘", "T"];
        const CMD_A: [&str; 2] = ["⌘", "A"];
        const ESC: [&str; 1] = ["Esc"];

        match self {
            NotesAction::NewNote => &CMD_N,
            NotesAction::DuplicateNote => &CMD_D,
            NotesAction::BrowseNotes => &CMD_P,
            NotesAction::FindInNote => &CMD_F,
            NotesAction::CopyNoteAs => &SHIFT_CMD_C,
            NotesAction::CopyDeeplink => &SHIFT_CMD_D,
            NotesAction::CreateQuicklink => &SHIFT_CMD_L,
            NotesAction::Export => &SHIFT_CMD_E,
            NotesAction::MoveListItemUp => &CTRL_CMD_UP,
            NotesAction::MoveListItemDown => &CTRL_CMD_DOWN,
            NotesAction::Format => &SHIFT_CMD_T,
            NotesAction::EnableAutoSizing => &CMD_A,
            NotesAction::Cancel => &ESC,
        }
    }

    /// Get the formatted shortcut display string
    pub fn shortcut_display(&self) -> String {
        if self.shortcut_keys().is_empty() {
            return String::new();
        }

        self.shortcut_keys().join("")
    }

    /// Get the icon for this action
    pub fn icon(&self) -> IconName {
        match self {
            NotesAction::NewNote => IconName::Plus,
            NotesAction::DuplicateNote => IconName::Copy,
            NotesAction::BrowseNotes => IconName::FolderOpen,
            NotesAction::FindInNote => IconName::Search,
            NotesAction::CopyNoteAs => IconName::Copy,
            NotesAction::CopyDeeplink => IconName::ExternalLink,
            NotesAction::CreateQuicklink => IconName::Star,
            NotesAction::Export => IconName::ArrowRight,
            NotesAction::MoveListItemUp => IconName::ArrowUp,
            NotesAction::MoveListItemDown => IconName::ArrowDown,
            NotesAction::Format => IconName::ALargeSmall,
            NotesAction::EnableAutoSizing => IconName::Maximize,
            NotesAction::Cancel => IconName::Close,
        }
    }

    /// Get action ID for lookup
    pub fn id(&self) -> &'static str {
        match self {
            NotesAction::NewNote => "new_note",
            NotesAction::DuplicateNote => "duplicate_note",
            NotesAction::BrowseNotes => "browse_notes",
            NotesAction::FindInNote => "find_in_note",
            NotesAction::CopyNoteAs => "copy_note_as",
            NotesAction::CopyDeeplink => "copy_deeplink",
            NotesAction::CreateQuicklink => "create_quicklink",
            NotesAction::Export => "export",
            NotesAction::MoveListItemUp => "move_list_item_up",
            NotesAction::MoveListItemDown => "move_list_item_down",
            NotesAction::Format => "format",
            NotesAction::EnableAutoSizing => "enable_auto_sizing",
            NotesAction::Cancel => "cancel",
        }
    }
}

/// Action list sections for visual grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotesActionSection {
    Primary,
    Actions,
    Move,
    Format,
    Utility,
}

impl NotesActionSection {
    fn for_action(action: NotesAction) -> Self {
        match action {
            NotesAction::NewNote
            | NotesAction::DuplicateNote
            | NotesAction::BrowseNotes => NotesActionSection::Primary,
            NotesAction::FindInNote
            | NotesAction::CopyNoteAs
            | NotesAction::CopyDeeplink
            | NotesAction::CreateQuicklink
            | NotesAction::Export => NotesActionSection::Actions,
            NotesAction::MoveListItemUp | NotesAction::MoveListItemDown => NotesActionSection::Move,
            NotesAction::Format => NotesActionSection::Format,
            NotesAction::EnableAutoSizing | NotesAction::Cancel => NotesActionSection::Utility,
        }
    }
}

/// Action entry with enabled state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotesActionItem {
    pub action: NotesAction,
    pub enabled: bool,
}

impl NotesActionItem {
    fn section(&self) -> NotesActionSection {
        NotesActionSection::for_action(self.action)
    }
}

/// Panel dimensions and styling constants (matches main ActionsDialog)
pub const PANEL_WIDTH: f32 = 320.0;
pub const PANEL_MAX_HEIGHT: f32 = 580.0;
pub const PANEL_CORNER_RADIUS: f32 = 12.0;
pub const ACTION_ITEM_HEIGHT: f32 = 42.0;
pub const ACCENT_BAR_WIDTH: f32 = 3.0;
pub const PANEL_SEARCH_HEIGHT: f32 = 44.0;
pub const PANEL_BORDER_HEIGHT: f32 = 2.0;

pub fn panel_height_for_rows(row_count: usize) -> f32 {
    let items_height =
        (row_count as f32 * ACTION_ITEM_HEIGHT).min(PANEL_MAX_HEIGHT - (PANEL_SEARCH_HEIGHT + 16.0));
    items_height + PANEL_SEARCH_HEIGHT + PANEL_BORDER_HEIGHT
}

/// Notes Actions Panel - Modal overlay for note operations
pub struct NotesActionsPanel {
    /// Available actions
    actions: Vec<NotesActionItem>,
    /// Filtered action indices
    filtered_indices: Vec<usize>,
    /// Currently selected index (within filtered)
    selected_index: usize,
    /// Search text
    search_text: String,
    /// Focus handle
    focus_handle: FocusHandle,
    /// Callback for action selection
    on_action: NotesActionCallback,
    /// Scroll handle for virtualization
    scroll_handle: UniformListScrollHandle,
    /// Cursor blink visibility
    cursor_visible: bool,
}

impl NotesActionsPanel {
    /// Create a new NotesActionsPanel
    pub fn new(
        focus_handle: FocusHandle,
        actions: Vec<NotesActionItem>,
        on_action: NotesActionCallback,
    ) -> Self {
        let filtered_indices: Vec<usize> = (0..actions.len()).collect();
        let selected_index = actions
            .iter()
            .position(|item| item.enabled)
            .unwrap_or(0);

        debug!(action_count = actions.len(), "Notes actions panel created");

        Self {
            actions,
            filtered_indices,
            selected_index,
            search_text: String::new(),
            focus_handle,
            on_action,
            scroll_handle: UniformListScrollHandle::new(),
            cursor_visible: true,
        }
    }

    /// Set cursor visibility (for blink animation)
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
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
        self.move_selection(-1, cx);
    }

    /// Move selection down
    pub fn move_down(&mut self, cx: &mut Context<Self>) {
        self.move_selection(1, cx);
    }

    /// Submit the selected action
    pub fn submit_selected(&mut self) {
        if let Some(&action_idx) = self.filtered_indices.get(self.selected_index) {
            if let Some(action) = self.actions.get(action_idx) {
                if action.enabled {
                    debug!(action = ?action.action, "Notes action selected");
                    (self.on_action)(action.action);
                }
            }
        }
    }

    /// Cancel and close
    pub fn cancel(&mut self) {
        debug!("Notes actions panel cancelled");
        (self.on_action)(NotesAction::Cancel);
    }

    /// Get currently selected action
    pub fn get_selected_action(&self) -> Option<NotesAction> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&idx| self.actions.get(idx))
            .and_then(|item| if item.enabled { Some(item.action) } else { None })
    }

    /// Refilter actions based on search text
    fn refilter(&mut self) {
        if self.search_text.is_empty() {
            self.filtered_indices = (0..self.actions.len()).collect();
        } else {
            let search_lower = self.search_text.to_lowercase();
            self.filtered_indices = self
                .actions
                .iter()
                .enumerate()
                .filter(|(_, action)| {
                    action
                        .action
                        .label()
                        .to_lowercase()
                        .contains(&search_lower)
                })
                .map(|(idx, _)| idx)
                .collect();
        }

        self.ensure_valid_selection();

        // Scroll to keep selection visible
        if !self.filtered_indices.is_empty() {
            self.scroll_handle
                .scroll_to_item(self.selected_index, ScrollStrategy::Nearest);
        }
    }

    fn ensure_valid_selection(&mut self) {
        if self.filtered_indices.is_empty() {
            self.selected_index = 0;
            return;
        }

        if self.selected_index >= self.filtered_indices.len()
            || !self.is_selectable(self.selected_index)
        {
            if let Some(index) = (0..self.filtered_indices.len())
                .find(|&idx| self.is_selectable(idx))
            {
                self.selected_index = index;
            } else {
                self.selected_index = 0;
            }
        }
    }

    fn is_selectable(&self, filtered_idx: usize) -> bool {
        self.filtered_indices
            .get(filtered_idx)
            .and_then(|&idx| self.actions.get(idx))
            .map(|item| item.enabled)
            .unwrap_or(false)
    }

    fn move_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        let filtered_len = self.filtered_indices.len();
        if filtered_len == 0 {
            return;
        }

        let mut next_index = self.selected_index as i32;
        loop {
            next_index += delta;
            if next_index < 0 || next_index >= filtered_len as i32 {
                break;
            }

            let next = next_index as usize;
            if self.is_selectable(next) {
                self.selected_index = next;
                self.scroll_handle
                    .scroll_to_item(self.selected_index, ScrollStrategy::Nearest);
                cx.notify();
                return;
            }
        }
    }

    /// Create box shadow for the overlay
    fn create_shadow() -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.0,
                    a: 0.3,
                },
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(16.0),
                spread_radius: px(0.0),
            },
            BoxShadow {
                color: Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.0,
                    a: 0.15,
                },
                offset: point(px(0.0), px(8.0)),
                blur_radius: px(32.0),
                spread_radius: px(-4.0),
            },
        ]
    }
}

impl Focusable for NotesActionsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for NotesActionsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // Colors from gpui-component theme
        let bg_color = theme.background;
        let border_color = theme.border;
        let text_primary = theme.foreground;
        let text_muted = theme.muted_foreground;
        let accent_color = theme.accent;

        // Search display
        let search_display = if self.search_text.is_empty() {
            SharedString::from("Search for actions...")
        } else {
            SharedString::from(self.search_text.clone())
        };

        // Build search input row
        let search_input = div()
            .w_full()
            .h(px(PANEL_SEARCH_HEIGHT))
            .px(px(12.0))
            .py(px(8.0))
            .bg(theme.secondary)
            .border_b_1()
            .border_color(border_color)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            // Search icon
            .child(
                Icon::new(IconName::Search)
                    .size_4()
                    .text_color(text_muted),
            )
            // Search field
            .child(
                div()
                    .flex_1()
                    .h(px(28.0))
                    .px(px(8.0))
                    .bg(theme.input)
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(if self.search_text.is_empty() {
                        border_color
                    } else {
                        accent_color
                    })
                    .flex()
                    .flex_row()
                    .items_center()
                    .text_sm()
                    .text_color(if self.search_text.is_empty() {
                        text_muted
                    } else {
                        text_primary
                    })
                    // Cursor when empty
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
                    // Cursor when has text
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

        // Build actions list
        let selected_index = self.selected_index;
        let filtered_len = self.filtered_indices.len();

        let actions_list = if self.filtered_indices.is_empty() {
            div()
                .flex_1()
                .w_full()
                .py(px(16.0))
                .px(px(12.0))
                .text_color(text_muted)
                .text_sm()
                .child("No actions match your search")
                .into_any_element()
        } else {
            uniform_list(
                "notes-actions-list",
                filtered_len,
                cx.processor(
                    move |this: &mut NotesActionsPanel, visible_range, _window, cx| {
                        let theme = cx.theme();
                        let mut items = Vec::new();

                        for idx in visible_range {
                            if let Some(&action_idx) = this.filtered_indices.get(idx) {
                                if let Some(action) = this.actions.get(action_idx) {
                                    let action: &NotesActionItem = action;
                                    let is_enabled = action.enabled;
                                    let is_selected = idx == selected_index && is_enabled;
                                    let is_section_start = if idx > 0 {
                                        this.filtered_indices
                                            .get(idx - 1)
                                            .and_then(|&prev_idx| this.actions.get(prev_idx))
                                            .map(|prev: &NotesActionItem| {
                                                prev.section() != action.section()
                                            })
                                            .unwrap_or(false)
                                    } else {
                                        false
                                    };

                                    // Transparent Hsla for unselected state
                                    let transparent = Hsla {
                                        h: 0.0,
                                        s: 0.0,
                                        l: 0.0,
                                        a: 0.0,
                                    };

                                    let action_row = div()
                                        .id(idx)
                                        .w_full()
                                        .h(px(ACTION_ITEM_HEIGHT))
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .bg(if is_selected {
                                            theme.list_active
                                        } else {
                                            transparent
                                        })
                                        // Left accent bar for selection
                                        .border_l(px(ACCENT_BAR_WIDTH))
                                        .border_color(if is_selected {
                                            theme.accent
                                        } else {
                                            transparent
                                        })
                                        .when(is_section_start, |d| {
                                            d.border_t_1().border_color(theme.border)
                                        })
                                        .when(is_enabled, |d| d.hover(|s| s.bg(theme.list_hover)))
                                        .when(is_enabled, |d| d.cursor_pointer())
                                        .when(!is_enabled, |d| d.opacity(0.5))
                                        // Content
                                        .child(
                                            div()
                                                .flex_1()
                                                .px(px(12.0))
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .justify_between()
                                                // Left: icon + label
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap(px(8.0))
                                                        // Icon - render using gpui_component Icon
                                                        .child({
                                                            let icon_name: IconName =
                                                                action.action.icon();
                                                            Icon::new(icon_name)
                                                                .size_4()
                                                                .text_color(if is_enabled {
                                                                    theme.foreground
                                                                } else {
                                                                    theme.muted_foreground
                                                                })
                                                        })
                                                        // Label
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(if is_enabled {
                                                                    theme.foreground
                                                                } else {
                                                                    theme.muted_foreground
                                                                })
                                                                .font_weight(if is_selected {
                                                                    gpui::FontWeight::MEDIUM
                                                                } else {
                                                                    gpui::FontWeight::NORMAL
                                                                })
                                                                .child(action.action.label()),
                                                        ),
                                                )
                                                // Right: shortcut badge
                                                .child(
                                                    render_shortcut_keys(
                                                        action.action.shortcut_keys(),
                                                        theme,
                                                    ),
                                                ),
                                        )
                                        .when(is_enabled, |d| {
                                            d.on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.selected_index = idx;
                                                    this.submit_selected();
                                                    cx.notify();
                                                }),
                                            )
                                        });

                                    items.push(action_row);
                                }
                            }
                        }
                        items
                    },
                ),
            )
            .flex_1()
            .w_full()
            .track_scroll(&self.scroll_handle)
            .into_any_element()
        };

        // Calculate dynamic height
        let items_height = (filtered_len as f32 * ACTION_ITEM_HEIGHT)
            .min(PANEL_MAX_HEIGHT - (PANEL_SEARCH_HEIGHT + 16.0));
        let total_height = items_height + PANEL_SEARCH_HEIGHT + PANEL_BORDER_HEIGHT;

        // Main container
        div()
            .flex()
            .flex_col()
            .w(px(PANEL_WIDTH))
            .h(px(total_height))
            .bg(bg_color)
            .rounded(px(PANEL_CORNER_RADIUS))
            .shadow(Self::create_shadow())
            .border_1()
            .border_color(border_color)
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .child(search_input)
            .child(actions_list)
    }
}

fn render_shortcut_keys(keys: &[&'static str], theme: &Theme) -> impl IntoElement {
    if keys.is_empty() {
        return div().into_any_element();
    }

    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0));

    for key in keys {
        row = row.child(
            div()
                .min_w(px(18.0))
                .px(px(6.0))
                .py(px(2.0))
                .bg(theme.muted)
                .border_1()
                .border_color(theme.border)
                .rounded(px(5.0))
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(*key),
        );
    }

    row.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notes_action_labels() {
        assert_eq!(NotesAction::NewNote.label(), "New Note");
        assert_eq!(NotesAction::DuplicateNote.label(), "Duplicate Note");
        assert_eq!(NotesAction::BrowseNotes.label(), "Browse Notes");
        assert_eq!(NotesAction::FindInNote.label(), "Find in Note");
        assert_eq!(NotesAction::CopyNoteAs.label(), "Copy Note As...");
        assert_eq!(NotesAction::CopyDeeplink.label(), "Copy Deeplink");
        assert_eq!(NotesAction::CreateQuicklink.label(), "Create Quicklink");
        assert_eq!(NotesAction::Export.label(), "Export...");
        assert_eq!(NotesAction::MoveListItemUp.label(), "Move List Item Up");
        assert_eq!(NotesAction::MoveListItemDown.label(), "Move List Item Down");
        assert_eq!(NotesAction::Format.label(), "Format...");
    }

    #[test]
    fn test_notes_action_shortcuts() {
        assert_eq!(NotesAction::NewNote.shortcut_display(), "⌘N");
        assert_eq!(NotesAction::DuplicateNote.shortcut_display(), "⌘D");
        assert_eq!(NotesAction::BrowseNotes.shortcut_display(), "⌘P");
        assert_eq!(NotesAction::FindInNote.shortcut_display(), "⌘F");
        assert_eq!(NotesAction::CopyNoteAs.shortcut_display(), "⇧⌘C");
        assert_eq!(NotesAction::CopyDeeplink.shortcut_display(), "⇧⌘D");
        assert_eq!(NotesAction::CreateQuicklink.shortcut_display(), "⇧⌘L");
        assert_eq!(NotesAction::Export.shortcut_display(), "⇧⌘E");
        assert_eq!(NotesAction::MoveListItemUp.shortcut_display(), "⌃⌘↑");
        assert_eq!(NotesAction::MoveListItemDown.shortcut_display(), "⌃⌘↓");
        assert_eq!(NotesAction::Format.shortcut_display(), "⇧⌘T");
    }

    #[test]
    fn test_notes_action_all() {
        let all = NotesAction::all();
        assert_eq!(all.len(), 11);
        assert!(all.contains(&NotesAction::NewNote));
        assert!(all.contains(&NotesAction::DuplicateNote));
        assert!(all.contains(&NotesAction::BrowseNotes));
        assert!(all.contains(&NotesAction::FindInNote));
        assert!(all.contains(&NotesAction::CopyNoteAs));
        assert!(all.contains(&NotesAction::CopyDeeplink));
        assert!(all.contains(&NotesAction::CreateQuicklink));
        assert!(all.contains(&NotesAction::Export));
        assert!(all.contains(&NotesAction::MoveListItemUp));
        assert!(all.contains(&NotesAction::MoveListItemDown));
        assert!(all.contains(&NotesAction::Format));
    }

    #[test]
    fn test_notes_action_ids() {
        assert_eq!(NotesAction::NewNote.id(), "new_note");
        assert_eq!(NotesAction::DuplicateNote.id(), "duplicate_note");
        assert_eq!(NotesAction::BrowseNotes.id(), "browse_notes");
        assert_eq!(NotesAction::FindInNote.id(), "find_in_note");
        assert_eq!(NotesAction::CopyNoteAs.id(), "copy_note_as");
        assert_eq!(NotesAction::CopyDeeplink.id(), "copy_deeplink");
        assert_eq!(NotesAction::CreateQuicklink.id(), "create_quicklink");
        assert_eq!(NotesAction::Export.id(), "export");
        assert_eq!(NotesAction::MoveListItemUp.id(), "move_list_item_up");
        assert_eq!(NotesAction::MoveListItemDown.id(), "move_list_item_down");
        assert_eq!(NotesAction::Format.id(), "format");
    }

    #[test]
    fn test_panel_constants() {
        // Verify panel matches main ActionsDialog dimensions
        assert_eq!(PANEL_WIDTH, 320.0);
        assert_eq!(PANEL_MAX_HEIGHT, 580.0);
        assert_eq!(PANEL_CORNER_RADIUS, 12.0);
        assert_eq!(ACTION_ITEM_HEIGHT, 42.0);
    }
}
