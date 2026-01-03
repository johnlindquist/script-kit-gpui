//! Notes Window
//!
//! A separate floating window for notes, built with gpui-component.
//! This is completely independent from the main Script Kit launcher window.

use anyhow::Result;
use gpui::{
    div, hsla, point, prelude::*, px, rgb, size, App, BoxShadow, Context, Entity, FocusHandle,
    Focusable, Hsla, IntoElement, KeyDownEvent, ParentElement, Render, Styled, Subscription,
    Window, WindowBounds, WindowOptions,
};

#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
use gpui_component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, Search},
    theme::{ActiveTheme, Theme as GpuiTheme, ThemeColor, ThemeMode},
    IconName, Root, Sizable,
};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

use super::actions_panel::{
    panel_height_for_rows, NotesAction, NotesActionItem, NotesActionsPanel,
};
use super::browse_panel::{BrowsePanel, NoteAction, NoteListItem};
use super::model::{ExportFormat, Note, NoteId};
use super::storage;
use crate::watcher::ThemeWatcher;

/// Global handle to the notes window
static NOTES_WINDOW: std::sync::OnceLock<std::sync::Mutex<Option<gpui::WindowHandle<Root>>>> =
    std::sync::OnceLock::new();

/// View mode for the notes list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotesViewMode {
    /// Show all active notes
    #[default]
    AllNotes,
    /// Show deleted notes (trash)
    Trash,
}

/// The main notes application view
///
/// Raycast-style single-note view:
/// - No sidebar - displays one note at a time
/// - Titlebar with note title and hover-reveal action icons
/// - Auto-resize: window height grows with content
/// - Footer with type indicator and character count
pub struct NotesApp {
    /// All notes (cached from storage)
    notes: Vec<Note>,

    /// Deleted notes (for trash view)
    deleted_notes: Vec<Note>,

    /// Current view mode
    view_mode: NotesViewMode,

    /// Currently selected note ID
    selected_note_id: Option<NoteId>,

    /// Editor input state (using gpui-component's Input)
    editor_state: Entity<InputState>,

    /// Search input state (for future browse panel)
    search_state: Entity<InputState>,

    /// Current search query (for future browse panel)
    search_query: String,

    /// Whether the titlebar is being hovered (for showing/hiding icons)
    titlebar_hovered: bool,

    /// Whether the entire window is being hovered (for traffic lights)
    window_hovered: bool,

    /// Forces hover chrome for visual tests
    force_hovered: bool,

    /// Whether the formatting toolbar is pinned open
    show_format_toolbar: bool,

    /// Last known content line count for auto-resize
    last_line_count: usize,

    /// Initial window height - used as minimum for auto-resize
    initial_height: f32,

    /// Whether auto-sizing is enabled
    /// When enabled: window grows AND shrinks to fit content (min = initial_height)
    /// When disabled: window size is fixed until user re-enables via actions panel
    /// Disabled automatically when user manually resizes the window
    auto_sizing_enabled: bool,

    /// Last known window height - used to detect manual resize
    last_window_height: f32,

    /// Focus handle for keyboard navigation
    focus_handle: FocusHandle,

    /// Subscriptions to keep alive
    _subscriptions: Vec<Subscription>,

    /// Whether the actions panel is shown (Cmd+K)
    show_actions_panel: bool,

    /// Whether the browse panel is shown (Cmd+P)
    show_browse_panel: bool,

    /// Entity for the actions panel (when shown)
    actions_panel: Option<Entity<NotesActionsPanel>>,

    /// Entity for the browse panel (when shown)
    browse_panel: Option<Entity<super::browse_panel::BrowsePanel>>,

    /// Pending action from actions panel clicks
    pending_action: Arc<Mutex<Option<NotesAction>>>,

    /// Previous height before showing the actions panel
    actions_panel_prev_height: Option<f32>,
}

impl NotesApp {
    /// Create a new NotesApp
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Initialize storage
        if let Err(e) = storage::init_notes_db() {
            tracing::error!(error = %e, "Failed to initialize notes database");
        }

        // Load notes from storage
        let notes = storage::get_all_notes().unwrap_or_default();
        let deleted_notes = storage::get_deleted_notes().unwrap_or_default();
        let selected_note_id = notes.first().map(|n| n.id);

        // Get initial content if we have a selected note
        let initial_content = selected_note_id
            .and_then(|id| notes.iter().find(|n| n.id == id))
            .map(|n| n.content.clone())
            .unwrap_or_default();

        // Calculate initial line count for auto-resize (before moving content)
        let initial_line_count = initial_content.lines().count().max(1);

        // Create input states - use multi_line for the editor
        let editor_state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .searchable(true)
                .rows(20)
                .placeholder("Start typing your note...")
                .default_value(initial_content)
        });

        let search_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search notes..."));

        let focus_handle = cx.focus_handle();

        // Subscribe to editor changes - passes window for auto-resize
        let editor_sub = cx.subscribe_in(&editor_state, window, {
            move |this, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.on_editor_change(window, cx);
                }
            }
        });

        // Subscribe to search changes
        let search_sub = cx.subscribe_in(&search_state, window, {
            move |this, _, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.on_search_change(cx);
                }
            }
        });

        // Get initial window height to use as minimum
        let initial_height: f32 = window.bounds().size.height.into();

        info!(
            note_count = notes.len(),
            initial_height = initial_height,
            "Notes app initialized"
        );

        Self {
            notes,
            deleted_notes,
            view_mode: NotesViewMode::AllNotes,
            selected_note_id,
            editor_state,
            search_state,
            search_query: String::new(),
            titlebar_hovered: false,
            window_hovered: false,
            force_hovered: false,
            show_format_toolbar: false,
            last_line_count: initial_line_count,
            initial_height,
            auto_sizing_enabled: true,          // Auto-sizing ON by default
            last_window_height: initial_height, // Track for manual resize detection
            focus_handle,
            _subscriptions: vec![editor_sub, search_sub],
            show_actions_panel: false,
            show_browse_panel: false,
            actions_panel: None,
            browse_panel: None,
            pending_action: Arc::new(Mutex::new(None)),
            actions_panel_prev_height: None,
        }
    }

    /// Handle editor content changes with auto-resize
    fn on_editor_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_note_id {
            let content = self.editor_state.read(cx).value();
            let content_string = content.to_string();

            // Update the note in our cache
            if let Some(note) = self.notes.iter_mut().find(|n| n.id == id) {
                note.set_content(content_string.clone());

                // Save to storage (debounced in a real implementation)
                if let Err(e) = storage::save_note(note) {
                    tracing::error!(error = %e, "Failed to save note");
                }
            }

            // Auto-resize: adjust window height based on content
            let new_line_count = content_string.lines().count().max(1);
            if new_line_count != self.last_line_count {
                self.last_line_count = new_line_count;
                self.update_window_height(window, new_line_count, cx);
            }

            cx.notify();
        }
    }

    /// Update window height based on content line count
    /// Raycast-style: window grows AND shrinks to fit content when auto_sizing_enabled
    /// IMPORTANT: Window never shrinks below initial_height (the height at window creation)
    fn update_window_height(
        &mut self,
        window: &mut Window,
        line_count: usize,
        _cx: &mut Context<Self>,
    ) {
        // Skip if auto-sizing is disabled (user manually resized)
        if !self.auto_sizing_enabled {
            return;
        }

        // Constants for layout calculation - adjusted for compact sticky-note style
        const TITLEBAR_HEIGHT: f32 = 32.0;
        const FOOTER_HEIGHT: f32 = 24.0;
        const PADDING: f32 = 24.0; // Top + bottom padding in editor area
        const LINE_HEIGHT: f32 = 20.0; // Approximate line height
        const MAX_HEIGHT: f32 = 600.0; // Don't grow too large

        // Use initial_height as minimum - never shrink below starting size
        let min_height = self.initial_height;

        // Calculate desired height
        let content_height = (line_count as f32) * LINE_HEIGHT;
        let total_height = TITLEBAR_HEIGHT + content_height + FOOTER_HEIGHT + PADDING;
        let clamped_height = total_height.clamp(min_height, MAX_HEIGHT);

        // Get current bounds and update height
        let current_bounds = window.bounds();
        let old_height: f32 = current_bounds.size.height.into();

        // Resize if height needs to change (both grow AND shrink)
        // Use a small threshold to avoid constant tiny adjustments
        const RESIZE_THRESHOLD: f32 = 5.0;
        if (clamped_height - old_height).abs() > RESIZE_THRESHOLD {
            let new_size = size(current_bounds.size.width, px(clamped_height));

            debug!(
                old_height = old_height,
                new_height = clamped_height,
                min_height = min_height,
                line_count = line_count,
                auto_sizing = self.auto_sizing_enabled,
                "Auto-resize: adjusting window height"
            );

            window.resize(new_size);
            self.last_window_height = clamped_height;
        }
    }

    /// Enable auto-sizing (called from actions panel)
    pub fn enable_auto_sizing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.auto_sizing_enabled = true;
        // Re-calculate and apply the correct height
        let line_count = self.last_line_count;
        self.update_window_height(window, line_count, cx);
        info!("Auto-sizing enabled");
        cx.notify();
    }

    /// Check if user manually resized the window and disable auto-sizing if so
    fn detect_manual_resize(&mut self, window: &Window) {
        if !self.auto_sizing_enabled {
            return; // Already disabled
        }

        let current_height: f32 = window.bounds().size.height.into();

        // If height differs significantly from what we set, user resized manually
        const MANUAL_RESIZE_THRESHOLD: f32 = 10.0;
        if (current_height - self.last_window_height).abs() > MANUAL_RESIZE_THRESHOLD {
            self.auto_sizing_enabled = false;
            self.last_window_height = current_height;
            debug!(
                current_height = current_height,
                last_height = self.last_window_height,
                "Manual resize detected - auto-sizing disabled"
            );
        }
    }

    /// Handle search query changes
    fn on_search_change(&mut self, cx: &mut Context<Self>) {
        let query = self.search_state.read(cx).value().to_string();
        self.search_query = query.clone();

        // If search is not empty, use FTS search
        if !query.trim().is_empty() {
            match storage::search_notes(&query) {
                Ok(results) => {
                    self.notes = results;
                    // Update selection if current note not in results
                    if let Some(id) = self.selected_note_id {
                        if !self.notes.iter().any(|n| n.id == id) {
                            self.selected_note_id = self.notes.first().map(|n| n.id);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Search failed");
                }
            }
        } else {
            // Reload all notes when search is cleared
            self.notes = storage::get_all_notes().unwrap_or_default();
        }

        cx.notify();
    }

    /// Create a new note
    fn create_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let note = Note::new();
        let id = note.id;

        // Save to storage
        if let Err(e) = storage::save_note(&note) {
            tracing::error!(error = %e, "Failed to create note");
            return;
        }

        // Add to cache and select it
        self.notes.insert(0, note);
        self.select_note(id, window, cx);

        info!(note_id = %id, "New note created");
    }

    /// Select a note for editing
    fn select_note(&mut self, id: NoteId, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_note_id = Some(id);

        // Load content into editor
        let note_list = if self.view_mode == NotesViewMode::Trash {
            &self.deleted_notes
        } else {
            &self.notes
        };

        if let Some(note) = note_list.iter().find(|n| n.id == id) {
            self.editor_state.update(cx, |state, cx| {
                state.set_value(&note.content, window, cx);
            });
        }

        // Focus the editor after selecting a note
        self.editor_state.update(cx, |state, cx| {
            state.focus(window, cx);
        });

        cx.notify();
    }

    /// Delete the currently selected note (soft delete)
    fn delete_selected_note(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_note_id {
            if let Some(note) = self.notes.iter_mut().find(|n| n.id == id) {
                note.soft_delete();

                if let Err(e) = storage::save_note(note) {
                    tracing::error!(error = %e, "Failed to delete note");
                }

                // Move to deleted notes
                self.deleted_notes.insert(0, note.clone());
            }

            // Remove from visible list and select next
            self.notes.retain(|n| n.id != id);
            self.selected_note_id = self.notes.first().map(|n| n.id);

            cx.notify();
        }
    }

    /// Permanently delete the selected note from trash
    fn permanently_delete_note(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_note_id {
            if let Err(e) = storage::delete_note_permanently(id) {
                tracing::error!(error = %e, "Failed to permanently delete note");
                return;
            }

            self.deleted_notes.retain(|n| n.id != id);
            self.selected_note_id = self.deleted_notes.first().map(|n| n.id);

            info!(note_id = %id, "Note permanently deleted");
            cx.notify();
        }
    }

    /// Restore the selected note from trash
    fn restore_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_note_id {
            if let Some(note) = self.deleted_notes.iter_mut().find(|n| n.id == id) {
                note.restore();

                if let Err(e) = storage::save_note(note) {
                    tracing::error!(error = %e, "Failed to restore note");
                    return;
                }

                // Move back to active notes
                self.notes.insert(0, note.clone());
            }

            self.deleted_notes.retain(|n| n.id != id);
            self.view_mode = NotesViewMode::AllNotes;
            self.selected_note_id = Some(id);
            self.select_note(id, window, cx);

            info!(note_id = %id, "Note restored");
            cx.notify();
        }
    }

    /// Switch view mode
    fn set_view_mode(&mut self, mode: NotesViewMode, window: &mut Window, cx: &mut Context<Self>) {
        self.view_mode = mode;

        // Select first note in new view
        let notes = match mode {
            NotesViewMode::AllNotes => &self.notes,
            NotesViewMode::Trash => &self.deleted_notes,
        };

        if let Some(note) = notes.first() {
            self.select_note(note.id, window, cx);
        } else {
            self.selected_note_id = None;
            self.editor_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
        }

        cx.notify();
    }

    /// Export the current note
    fn export_note(&self, format: ExportFormat) {
        if let Some(id) = self.selected_note_id {
            if let Some(note) = self.notes.iter().find(|n| n.id == id) {
                let content = match format {
                    ExportFormat::PlainText => note.content.clone(),
                    ExportFormat::Markdown => {
                        format!("# {}\n\n{}", note.title, note.content)
                    }
                    ExportFormat::Html => {
                        format!(
                            "<!DOCTYPE html>\n<html>\n<head><title>{}</title></head>\n<body>\n<h1>{}</h1>\n<pre>{}</pre>\n</body>\n</html>",
                            note.title, note.title, note.content
                        )
                    }
                };

                // Copy to clipboard
                #[cfg(target_os = "macos")]
                {
                    use std::process::Command;
                    let _ = Command::new("pbcopy")
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                        .and_then(|mut child| {
                            use std::io::Write;
                            if let Some(stdin) = child.stdin.as_mut() {
                                stdin.write_all(content.as_bytes())?;
                            }
                            child.wait()
                        });
                    info!(format = ?format, "Note exported to clipboard");
                }
            }
        }
    }

    /// Insert markdown formatting at cursor position
    fn insert_formatting(&mut self, prefix: &str, suffix: &str, cx: &mut Context<Self>) {
        let current = self.editor_state.read(cx).value().to_string();
        // For simplicity, append to end. A real implementation would insert at cursor.
        let formatted = format!("{}{}{}", current, prefix, suffix);
        // Note: We can't directly update with cursor position, so this is simplified
        info!(prefix = prefix, "Formatting inserted");
        let _ = formatted; // Would update editor in full implementation
        cx.notify();
    }

    /// Get filtered notes based on search query
    fn get_visible_notes(&self) -> &[Note] {
        match self.view_mode {
            NotesViewMode::AllNotes => &self.notes,
            NotesViewMode::Trash => &self.deleted_notes,
        }
    }

    /// Get the character count of the current note
    fn get_character_count(&self, cx: &Context<Self>) -> usize {
        self.editor_state.read(cx).value().chars().count()
    }

    /// Copy the current note content to clipboard
    fn copy_note_to_clipboard(&self, cx: &Context<Self>) {
        let content = self.editor_state.read(cx).value().to_string();
        self.copy_text_to_clipboard(&content);
    }

    fn copy_text_to_clipboard(&self, content: &str) {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(stdin) = child.stdin.as_mut() {
                        stdin.write_all(content.as_bytes())?;
                    }
                    child.wait()
                });
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = content; // Avoid unused warning
        }
    }

    fn note_deeplink(&self, id: NoteId) -> String {
        format!("kit://notes/{}", id.as_str())
    }

    fn copy_note_as_markdown(&self) {
        self.export_note(ExportFormat::Markdown);
    }

    fn copy_note_deeplink(&self) {
        if let Some(id) = self.selected_note_id {
            let deeplink = self.note_deeplink(id);
            self.copy_text_to_clipboard(&deeplink);
        }
    }

    fn create_note_quicklink(&self) {
        if let Some(id) = self.selected_note_id {
            let title = self
                .notes
                .iter()
                .find(|note| note.id == id)
                .map(|note| {
                    if note.title.is_empty() {
                        "Untitled Note".to_string()
                    } else {
                        note.title.clone()
                    }
                })
                .unwrap_or_else(|| "Untitled Note".to_string());
            let deeplink = self.note_deeplink(id);
            let quicklink = format!("[{}]({})", title, deeplink);
            self.copy_text_to_clipboard(&quicklink);
        }
    }

    fn duplicate_selected_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_note_id else {
            return;
        };
        let Some(note) = self.notes.iter().find(|note| note.id == id) else {
            return;
        };

        let duplicate = Note::with_content(note.content.clone());
        if let Err(e) = storage::save_note(&duplicate) {
            tracing::error!(error = %e, "Failed to duplicate note");
            return;
        }

        self.notes.insert(0, duplicate.clone());
        self.select_note(duplicate.id, window, cx);
    }

    fn build_action_items(&self) -> Vec<NotesActionItem> {
        let has_selection = self.selected_note_id.is_some();
        let is_trash = self.view_mode == NotesViewMode::Trash;
        let can_edit = has_selection && !is_trash;

        let mut items: Vec<NotesActionItem> = NotesAction::all()
            .iter()
            .map(|action| {
                let enabled = match action {
                    NotesAction::NewNote | NotesAction::BrowseNotes => true,
                    NotesAction::DuplicateNote
                    | NotesAction::FindInNote
                    | NotesAction::CopyNoteAs
                    | NotesAction::CopyDeeplink
                    | NotesAction::CreateQuicklink
                    | NotesAction::Export
                    | NotesAction::Format => can_edit,
                    NotesAction::MoveListItemUp | NotesAction::MoveListItemDown => false,
                    NotesAction::EnableAutoSizing => !self.auto_sizing_enabled,
                    NotesAction::Cancel => true,
                };

                NotesActionItem {
                    action: *action,
                    enabled,
                }
            })
            .collect();

        if !self.auto_sizing_enabled {
            items.push(NotesActionItem {
                action: NotesAction::EnableAutoSizing,
                enabled: true,
            });
        }

        items
    }

    fn open_actions_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let actions = self.build_action_items();
        let action_count = actions.len();
        let pending_action = self.pending_action.clone();

        let panel = cx.new(|cx| {
            let focus_handle = cx.focus_handle();
            NotesActionsPanel::new(
                focus_handle,
                actions,
                Arc::new(move |action| {
                    if let Ok(mut pending) = pending_action.lock() {
                        *pending = Some(action);
                    }
                }),
            )
        });

        let panel_focus_handle = panel.read(cx).focus_handle();
        self.actions_panel = Some(panel);
        self.show_actions_panel = true;
        self.show_browse_panel = false;
        self.browse_panel = None;
        window.focus(&panel_focus_handle, cx);

        self.ensure_actions_panel_height(window, action_count);
        cx.notify();
    }

    fn close_actions_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_actions_panel = false;
        self.actions_panel = None;
        self.restore_actions_panel_height(window);
        
        // Refocus the editor after closing the actions panel
        self.editor_state.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        
        cx.notify();
    }

    fn ensure_actions_panel_height(&mut self, window: &mut Window, row_count: usize) {
        const ACTIONS_PANEL_WINDOW_MARGIN: f32 = 64.0;

        let panel_height = panel_height_for_rows(row_count);
        let desired_height = panel_height + ACTIONS_PANEL_WINDOW_MARGIN;
        let current_bounds = window.bounds();
        let current_height: f32 = current_bounds.size.height.into();

        if current_height + 1.0 < desired_height {
            self.actions_panel_prev_height = Some(current_height);
            window.resize(size(current_bounds.size.width, px(desired_height)));
            self.last_window_height = desired_height;
        }
    }

    fn restore_actions_panel_height(&mut self, window: &mut Window) {
        let Some(prev_height) = self.actions_panel_prev_height.take() else {
            return;
        };

        let current_bounds = window.bounds();
        window.resize(size(current_bounds.size.width, px(prev_height)));
        self.last_window_height = prev_height;
    }

    fn drain_pending_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pending_action = self
            .pending_action
            .lock()
            .ok()
            .and_then(|mut pending| pending.take());

        if let Some(action) = pending_action {
            self.handle_action(action, window, cx);
        }
    }

    /// Handle action from the actions panel (Cmd+K)
    fn handle_action(&mut self, action: NotesAction, window: &mut Window, cx: &mut Context<Self>) {
        debug!(?action, "Handling notes action");
        match action {
            NotesAction::NewNote => self.create_note(window, cx),
            NotesAction::DuplicateNote => self.duplicate_selected_note(window, cx),
            NotesAction::BrowseNotes => {
                // Close actions panel first, then open browse panel
                // Don't call close_actions_panel here - it refocuses editor
                // Instead, just clear the state and let open_browse_panel handle focus
                self.show_actions_panel = false;
                self.actions_panel = None;
                self.restore_actions_panel_height(window);
                self.show_browse_panel = true;
                self.open_browse_panel(window, cx);
                cx.notify();
                return; // Early return - browse panel handles its own focus
            }
            NotesAction::FindInNote => {
                self.close_actions_panel(window, cx);
                self.editor_state.update(cx, |state, cx| {
                    state.focus(window, cx);
                });
                cx.dispatch_action(&Search);
                return; // Early return - already handled focus
            }
            NotesAction::CopyNoteAs => self.copy_note_as_markdown(),
            NotesAction::CopyDeeplink => self.copy_note_deeplink(),
            NotesAction::CreateQuicklink => self.create_note_quicklink(),
            NotesAction::Export => self.export_note(ExportFormat::Html),
            NotesAction::MoveListItemUp | NotesAction::MoveListItemDown => {}
            NotesAction::Format => {
                self.show_format_toolbar = !self.show_format_toolbar;
            }
            NotesAction::EnableAutoSizing => {
                self.enable_auto_sizing(window, cx);
            }
            NotesAction::Cancel => {
                // Panel was cancelled, nothing to do
            }
        }
        // Default: close actions panel and refocus editor
        self.close_actions_panel(window, cx);
        cx.notify();
    }

    /// Open the browse panel with current notes
    fn open_browse_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Create NoteListItems from current notes
        let note_items: Vec<NoteListItem> = self
            .notes
            .iter()
            .map(|note| NoteListItem::from_note(note, Some(note.id) == self.selected_note_id))
            .collect();

        let browse_panel = cx.new(|cx| BrowsePanel::new(note_items, window, cx));
        
        // Focus the browse panel
        let panel_focus_handle = browse_panel.read(cx).focus_handle(cx);
        window.focus(&panel_focus_handle, cx);

        self.browse_panel = Some(browse_panel);
        cx.notify();
    }

    /// Handle note selection from browse panel
    fn handle_browse_select(&mut self, id: NoteId, window: &mut Window, cx: &mut Context<Self>) {
        self.show_browse_panel = false;
        self.browse_panel = None;
        // select_note already focuses the editor
        self.select_note(id, window, cx);
        cx.notify();
    }

    /// Handle note action from browse panel
    fn handle_browse_action(&mut self, id: NoteId, action: NoteAction, cx: &mut Context<Self>) {
        match action {
            NoteAction::TogglePin => {
                if let Some(note) = self.notes.iter_mut().find(|n| n.id == id) {
                    note.is_pinned = !note.is_pinned;
                    if let Err(e) = storage::save_note(note) {
                        tracing::error!(error = %e, "Failed to save note pin state");
                    }
                }
            }
            NoteAction::Delete => {
                let current_id = self.selected_note_id;
                self.selected_note_id = Some(id);
                self.delete_selected_note(cx);
                // Restore selection if different note was deleted
                if current_id != Some(id) {
                    self.selected_note_id = current_id;
                }
            }
        }
        // Update browse panel's note list
        if let Some(ref browse_panel) = self.browse_panel {
            let note_items: Vec<NoteListItem> = self
                .notes
                .iter()
                .map(|note| NoteListItem::from_note(note, Some(note.id) == self.selected_note_id))
                .collect();
            browse_panel.update(cx, |panel, cx| {
                panel.set_notes(note_items, cx);
            });
        }
        cx.notify();
    }

    /// Close the browse panel and refocus the editor
    fn close_browse_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_browse_panel = false;
        self.browse_panel = None;
        
        // Refocus the editor after closing the browse panel
        self.editor_state.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        
        cx.notify();
    }

    /// Render the search input
    fn render_search(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .px_2()
            .py_1()
            .child(Input::new(&self.search_state).w_full().small())
    }

    /// Render the formatting toolbar
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .py_1()
            .child(
                Button::new("bold")
                    .ghost()
                    .xsmall()
                    .label("B")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.insert_formatting("**", "**", cx);
                    })),
            )
            .child(
                Button::new("italic")
                    .ghost()
                    .xsmall()
                    .label("I")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.insert_formatting("_", "_", cx);
                    })),
            )
            .child(
                Button::new("heading")
                    .ghost()
                    .xsmall()
                    .label("H")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.insert_formatting("\n## ", "", cx);
                    })),
            )
            .child(
                Button::new("list")
                    .ghost()
                    .xsmall()
                    .label("•")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.insert_formatting("\n- ", "", cx);
                    })),
            )
            .child(
                Button::new("code")
                    .ghost()
                    .xsmall()
                    .label("</>")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.insert_formatting("`", "`", cx);
                    })),
            )
            .child(
                Button::new("codeblock")
                    .ghost()
                    .xsmall()
                    .label("```")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.insert_formatting("\n```\n", "\n```", cx);
                    })),
            )
            .child(
                Button::new("link")
                    .ghost()
                    .xsmall()
                    .label("🔗")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.insert_formatting("[", "](url)", cx);
                    })),
            )
    }

    /// Render the export menu
    fn render_export_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap_1()
            .child(
                Button::new("export-txt")
                    .ghost()
                    .xsmall()
                    .label("TXT")
                    .on_click(cx.listener(|this, _, _, _cx| {
                        this.export_note(ExportFormat::PlainText);
                    })),
            )
            .child(
                Button::new("export-md")
                    .ghost()
                    .xsmall()
                    .label("MD")
                    .on_click(cx.listener(|this, _, _, _cx| {
                        this.export_note(ExportFormat::Markdown);
                    })),
            )
            .child(
                Button::new("export-html")
                    .ghost()
                    .xsmall()
                    .label("HTML")
                    .on_click(cx.listener(|this, _, _, _cx| {
                        this.export_note(ExportFormat::Html);
                    })),
            )
    }

    // Note: Sidebar removed for Raycast-style single-note view.
    // Browse panel (Cmd+P) will be implemented as a separate overlay in the future.

    /// Render the main editor area with Raycast-style clean UI
    fn render_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_trash = self.view_mode == NotesViewMode::Trash;
        let has_selection = self.selected_note_id.is_some();
        let show_toolbar = self.show_format_toolbar;
        let char_count = self.get_character_count(cx);

        // Get note title
        let title = self
            .selected_note_id
            .and_then(|id| self.get_visible_notes().iter().find(|n| n.id == id))
            .map(|n| {
                if n.title.is_empty() {
                    "Untitled Note".to_string()
                } else {
                    n.title.clone()
                }
            })
            .unwrap_or_else(|| {
                if is_trash {
                    "No deleted notes".to_string()
                } else {
                    "No note selected".to_string()
                }
            });

        // Raycast-style: titlebar only visible on hover, centered title, right-aligned actions
        let window_hovered = self.window_hovered || self.force_hovered;
        
        // Get muted foreground color for subtle icons/text
        let muted_color = cx.theme().muted_foreground;
        
        let titlebar = div()
            .id("notes-titlebar")
            .flex()
            .items_center()
            .justify_center() // Center the title
            .h(px(32.))
            .px_3()
            .relative() // For absolute positioning of icons
            // No background - blends with window
            .bg(cx.theme().background)
            // Only show titlebar elements when window is hovered
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if this.force_hovered {
                    return;
                }

                this.titlebar_hovered = *hovered;
                cx.notify();
            }))
            .child(
                // Note title (truncated) - CENTERED in titlebar
                div()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(muted_color) // Use muted color for subtle title
                    .when(!window_hovered, |d| d.opacity(0.))
                    .child(title),
            )
            // Conditionally show icons based on state - only when window is hovered
            // Raycast-style: 3 icons on the right - settings (actions), panel (browse), + (new)
            // Use absolute positioning to keep title centered
            .when(window_hovered && has_selection && !is_trash, |d| {
                d.child(
                    div()
                        .absolute()
                        .right_3() // Align to right with same padding as px_3
                        .flex()
                        .items_center()
                        .gap_2() // Even spacing between icons
                        // Icon 1: Command key icon - opens actions panel (⌘K)
                        .child(
                            div()
                                .id("titlebar-cmd-icon")
                                .text_sm()
                                .text_color(muted_color.opacity(0.7)) // Muted, subtle icon
                                .cursor_pointer()
                                .hover(|s| s.text_color(muted_color)) // Slightly brighter on hover
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if this.show_actions_panel {
                                        this.close_actions_panel(window, cx);
                                    } else {
                                        this.open_actions_panel(window, cx);
                                    }
                                }))
                                .child("⌘"),
                        )
                        // Icon 2: List icon - for browsing notes
                        .child(
                            div()
                                .id("titlebar-browse-icon")
                                .text_sm()
                                .text_color(muted_color.opacity(0.7)) // Muted, subtle icon
                                .cursor_pointer()
                                .hover(|s| s.text_color(muted_color)) // Slightly brighter on hover
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if this.show_browse_panel {
                                        this.close_browse_panel(window, cx);
                                    } else {
                                        this.close_actions_panel(window, cx);
                                        this.show_browse_panel = true;
                                        this.open_browse_panel(window, cx);
                                    }
                                }))
                                .child("≡"),
                        )
                        // Icon 3: Plus icon - for new note
                        .child(
                            div()
                                .id("titlebar-new-icon")
                                .text_sm()
                                .text_color(muted_color.opacity(0.7)) // Muted, subtle icon
                                .cursor_pointer()
                                .hover(|s| s.text_color(muted_color)) // Slightly brighter on hover
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.create_note(window, cx);
                                }))
                                .child("+"),
                        ),
                )
            })
            .when(has_selection && is_trash, |d| {
                // Trash actions (always visible)
                d.child(
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            Button::new("restore")
                                .ghost()
                                .xsmall()
                                .label("Restore")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.restore_note(window, cx);
                                })),
                        )
                        .child(
                            Button::new("permanent-delete")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Delete)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.permanently_delete_note(cx);
                                })),
                        ),
                )
            });

        // Build character count footer - only visible on hover
        // Raycast style: character count CENTERED, T icon on RIGHT
        let footer = div()
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .h(px(24.))
            .px_3()
            // No border, same background as window
            .bg(cx.theme().background)
            // Hide when window not hovered
            .when(!window_hovered, |d| d.opacity(0.))
            .child(
                // Character count CENTERED (Raycast style)
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} character{}",
                        char_count,
                        if char_count == 1 { "" } else { "s" }
                    )),
            )
            .child(
                // Type indicator (T for text) on RIGHT (Raycast style)
                div()
                    .absolute()
                    .right_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("T"),
            );

        // Build main editor layout - Raycast style: clean, no visible input borders
        div()
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            .bg(cx.theme().background) // Unified background
            .child(titlebar)
            // Toolbar hidden by default - only show when pinned
            .when(!is_trash && has_selection && show_toolbar, |d| {
                d.child(self.render_toolbar(cx))
            })
            .child(
                div()
                    .flex_1()
                    .p_3()
                    .bg(cx.theme().background) // Same background - no visible input box
                    // Use a styled input that blends with background
                    .child(
                        Input::new(&self.editor_state).h_full().appearance(false), // No input styling - blends with background
                    ),
            )
            .when(has_selection && !is_trash, |d| d.child(footer))
    }

    /// Render the actions panel overlay (Cmd+K)
    /// 
    /// IMPORTANT: Uses items_start + fixed top padding to keep the search input
    /// at a stable position. Without this, the panel would re-center when items
    /// are filtered out, causing the search input to jump around.
    fn render_actions_panel_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = self
            .actions_panel
            .as_ref()
            .map(|panel| panel.clone().into_any_element())
            .unwrap_or_else(|| div().into_any_element());

        // Fixed top offset so search input stays at same position regardless of item count
        const PANEL_TOP_OFFSET: f32 = 32.0;

        div()
            .id("actions-panel-overlay")
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000080))
            .flex()
            .flex_col()
            .items_center()      // Horizontally centered
            .justify_start()     // Vertically aligned to top (not centered!)
            .pt(px(PANEL_TOP_OFFSET)) // Fixed offset from top
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.close_actions_panel(window, cx);
                }),
            )
            .child(
                div()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {
                        // Stop propagation - don't close when clicking panel
                    })
                    .child(panel),
            )
    }

    /// Render the browse panel overlay (Cmd+P)
    fn render_browse_panel_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // If we have a browse panel entity, render it
        // Otherwise render an empty container that will close on click
        if let Some(ref browse_panel) = self.browse_panel {
            div()
                .id("browse-panel-overlay")
                .absolute()
                .inset_0()
                .child(browse_panel.clone())
        } else {
            // Fallback: create inline browse panel
            let note_items: Vec<NoteListItem> = self
                .notes
                .iter()
                .map(|note| NoteListItem::from_note(note, Some(note.id) == self.selected_note_id))
                .collect();

            // We need a simple inline version since we can't create entities in render
            div()
                .id("browse-panel-overlay")
                .absolute()
                .inset_0()
                .bg(gpui::rgba(0x00000080))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(|this, _, window, cx| {
                    this.close_browse_panel(window, cx);
                }))
                .child(
                    div()
                        .w(px(500.))
                        .max_h(px(400.))
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_lg()
                        .shadow_lg()
                        .p_4()
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {
                            // Stop propagation
                        })
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} notes available", note_items.len())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .mt_2()
                                .child("Press Escape to close"),
                        ),
                )
        }
    }

    /// Create box shadows from theme configuration
    /// Uses the same drop_shadow settings as the main window
    fn create_box_shadows(&self) -> Vec<BoxShadow> {
        let theme = crate::theme::load_theme();
        let shadow_config = theme.get_drop_shadow();

        if !shadow_config.enabled {
            return vec![];
        }

        // Convert hex color to HSLA
        let r = ((shadow_config.color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((shadow_config.color >> 8) & 0xFF) as f32 / 255.0;
        let b = (shadow_config.color & 0xFF) as f32 / 255.0;

        // Simple RGB to HSL conversion
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        let (h, s) = if max == min {
            (0.0, 0.0)
        } else {
            let d = max - min;
            let s = if l > 0.5 {
                d / (2.0 - max - min)
            } else {
                d / (max + min)
            };
            let h = if max == r {
                (g - b) / d + if g < b { 6.0 } else { 0.0 }
            } else if max == g {
                (b - r) / d + 2.0
            } else {
                (r - g) / d + 4.0
            };
            (h / 6.0, s)
        };

        vec![BoxShadow {
            color: hsla(h, s, l, shadow_config.opacity),
            offset: point(px(shadow_config.offset_x), px(shadow_config.offset_y)),
            blur_radius: px(shadow_config.blur_radius),
            spread_radius: px(shadow_config.spread_radius),
        }]
    }
}

impl Focusable for NotesApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for NotesApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Detect if user manually resized the window (disables auto-sizing)
        self.detect_manual_resize(window);
        self.drain_pending_action(window, cx);

        let show_actions = self.show_actions_panel;
        let show_browse = self.show_browse_panel;

        // Raycast-style single-note view: no sidebar, editor fills full width
        // Track window hover for traffic lights visibility
        let box_shadows = self.create_box_shadows();

        div()
            .id("notes-window-root")
            .flex()
            .flex_col()
            .size_full()
            .relative()
            .bg(cx.theme().background)
            .shadow(box_shadows)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus_handle)
            // Track window hover for showing/hiding chrome
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if this.force_hovered {
                    return;
                }

                this.window_hovered = *hovered;
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                // Handle keyboard shortcuts
                let key = event.keystroke.key.to_lowercase();
                let modifiers = &event.keystroke.modifiers;

                if this.show_actions_panel {
                    if key == "escape" || (modifiers.platform && key == "k") || key == "esc" {
                        this.close_actions_panel(window, cx);
                        return;
                    }

                    if let Some(ref panel) = this.actions_panel {
                        match key.as_str() {
                            "up" | "arrowup" => {
                                panel.update(cx, |panel, cx| panel.move_up(cx));
                            }
                            "down" | "arrowdown" => {
                                panel.update(cx, |panel, cx| panel.move_down(cx));
                            }
                            "enter" => {
                                if let Some(action) = panel.read(cx).get_selected_action() {
                                    this.handle_action(action, window, cx);
                                }
                            }
                            "backspace" => {
                                panel.update(cx, |panel, cx| panel.handle_backspace(cx));
                            }
                            _ => {
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            panel.update(cx, |panel, cx| {
                                                panel.handle_char(ch, cx);
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }

                    return;
                }

                // Handle Escape to close panels
                if key == "escape" {
                    if this.show_actions_panel {
                        this.close_actions_panel(window, cx);
                        return;
                    }
                    if this.show_browse_panel {
                        this.close_browse_panel(window, cx);
                        return;
                    }
                }

                // platform modifier = Cmd on macOS, Ctrl on Windows/Linux
                if modifiers.platform {
                    match key.as_str() {
                        "k" => {
                            // Toggle actions panel
                            if this.show_actions_panel {
                                this.close_actions_panel(window, cx);
                            } else {
                                this.open_actions_panel(window, cx);
                            }
                        }
                        "p" => {
                            // Toggle browse panel
                            this.show_browse_panel = !this.show_browse_panel;
                            this.close_actions_panel(window, cx);
                            if this.show_browse_panel {
                                this.open_browse_panel(window, cx);
                            } else {
                                this.browse_panel = None;
                            }
                            cx.notify();
                        }
                        "n" => this.create_note(window, cx),
                        "d" => this.duplicate_selected_note(window, cx),
                        "b" => this.insert_formatting("**", "**", cx),
                        "i" => this.insert_formatting("_", "_", cx),
                        _ => {}
                    }
                }
            }))
            // Single note view - editor takes full width
            .child(self.render_editor(cx))
            // Overlay panels
            .when(show_actions, |d| {
                d.child(self.render_actions_panel_overlay(cx))
            })
            .when(show_browse, |d| {
                d.child(self.render_browse_panel_overlay(cx))
            })
    }
}

/// Convert a u32 hex color to Hsla
#[inline]
fn hex_to_hsla(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Map Script Kit's ColorScheme to gpui-component's ThemeColor
///
/// NOTE: We intentionally do NOT apply opacity.* values to theme colors here.
/// The opacity values are for window-level transparency (vibrancy effect),
/// not for making UI elements semi-transparent. UI elements should remain solid.
fn map_scriptkit_to_gpui_theme(sk_theme: &crate::theme::Theme) -> ThemeColor {
    let colors = &sk_theme.colors;
    // NOTE: opacity is NOT used here - it's only for window background vibrancy

    // Get default dark theme as base and override with Script Kit colors
    let mut theme_color = *ThemeColor::dark();

    // Main background and foreground
    theme_color.background = hex_to_hsla(colors.background.main);
    theme_color.foreground = hex_to_hsla(colors.text.primary);

    // Accent colors (Script Kit yellow/gold)
    theme_color.accent = hex_to_hsla(colors.accent.selected);
    theme_color.accent_foreground = hex_to_hsla(colors.text.primary);

    // Border
    theme_color.border = hex_to_hsla(colors.ui.border);
    theme_color.input = hex_to_hsla(colors.ui.border);

    // List/sidebar colors
    theme_color.list = hex_to_hsla(colors.background.main);
    theme_color.list_active = hex_to_hsla(colors.accent.selected_subtle);
    theme_color.list_active_border = hex_to_hsla(colors.accent.selected);
    theme_color.list_hover = hex_to_hsla(colors.accent.selected_subtle);
    theme_color.list_even = hex_to_hsla(colors.background.main);
    theme_color.list_head = hex_to_hsla(colors.background.title_bar);

    // Sidebar (use slightly lighter background)
    theme_color.sidebar = hex_to_hsla(colors.background.title_bar);
    theme_color.sidebar_foreground = hex_to_hsla(colors.text.primary);
    theme_color.sidebar_border = hex_to_hsla(colors.ui.border);
    theme_color.sidebar_accent = hex_to_hsla(colors.accent.selected_subtle);
    theme_color.sidebar_accent_foreground = hex_to_hsla(colors.text.primary);
    theme_color.sidebar_primary = hex_to_hsla(colors.accent.selected);
    theme_color.sidebar_primary_foreground = hex_to_hsla(colors.text.primary);

    // Primary (accent-colored buttons)
    theme_color.primary = hex_to_hsla(colors.accent.selected);
    theme_color.primary_foreground = hex_to_hsla(colors.background.main);
    theme_color.primary_hover = hex_to_hsla(colors.accent.selected);
    theme_color.primary_active = hex_to_hsla(colors.accent.selected);

    // Secondary (muted buttons)
    theme_color.secondary = hex_to_hsla(colors.background.search_box);
    theme_color.secondary_foreground = hex_to_hsla(colors.text.primary);
    theme_color.secondary_hover = hex_to_hsla(colors.background.title_bar);
    theme_color.secondary_active = hex_to_hsla(colors.background.title_bar);

    // Muted (disabled states, subtle elements)
    theme_color.muted = hex_to_hsla(colors.background.search_box);
    theme_color.muted_foreground = hex_to_hsla(colors.text.muted);

    // Title bar
    theme_color.title_bar = hex_to_hsla(colors.background.title_bar);
    theme_color.title_bar_border = hex_to_hsla(colors.ui.border);

    // Popover
    theme_color.popover = hex_to_hsla(colors.background.main);
    theme_color.popover_foreground = hex_to_hsla(colors.text.primary);

    // Status colors
    theme_color.success = hex_to_hsla(colors.ui.success);
    theme_color.success_foreground = hex_to_hsla(colors.text.primary);
    theme_color.danger = hex_to_hsla(colors.ui.error);
    theme_color.danger_foreground = hex_to_hsla(colors.text.primary);
    theme_color.warning = hex_to_hsla(colors.ui.warning);
    theme_color.warning_foreground = hex_to_hsla(colors.text.primary);
    theme_color.info = hex_to_hsla(colors.ui.info);
    theme_color.info_foreground = hex_to_hsla(colors.text.primary);

    // Scrollbar
    theme_color.scrollbar = hex_to_hsla(colors.background.main);
    theme_color.scrollbar_thumb = hex_to_hsla(colors.text.dimmed);
    theme_color.scrollbar_thumb_hover = hex_to_hsla(colors.text.muted);

    // Caret (cursor) - cyan by default
    theme_color.caret = hex_to_hsla(0x00ffff);

    // Selection
    theme_color.selection = hex_to_hsla(colors.accent.selected_subtle);

    // Ring (focus ring)
    theme_color.ring = hex_to_hsla(colors.accent.selected);

    // Tab colors
    theme_color.tab = hex_to_hsla(colors.background.main);
    theme_color.tab_active = hex_to_hsla(colors.background.search_box);
    theme_color.tab_active_foreground = hex_to_hsla(colors.text.primary);
    theme_color.tab_foreground = hex_to_hsla(colors.text.secondary);
    theme_color.tab_bar = hex_to_hsla(colors.background.title_bar);

    debug!(
        background = format!("#{:06x}", colors.background.main),
        accent = format!("#{:06x}", colors.accent.selected),
        "Script Kit theme mapped to gpui-component"
    );

    theme_color
}

/// Initialize gpui-component theme and sync with Script Kit theme
fn ensure_theme_initialized(cx: &mut App) {
    // First, initialize gpui-component (this sets up the default theme)
    gpui_component::init(cx);

    // Load Script Kit's theme
    let sk_theme = crate::theme::load_theme();

    // Map Script Kit colors to gpui-component ThemeColor
    let custom_colors = map_scriptkit_to_gpui_theme(&sk_theme);

    // Apply the custom colors to the global theme
    let theme = GpuiTheme::global_mut(cx);
    theme.colors = custom_colors;
    theme.mode = ThemeMode::Dark; // Script Kit uses dark mode by default

    info!("Notes window theme synchronized with Script Kit");
}

/// Calculate window bounds positioned in the top-right corner of the display containing the mouse.
fn calculate_top_right_bounds(width: f32, height: f32, padding: f32) -> gpui::Bounds<gpui::Pixels> {
    use crate::platform::{get_global_mouse_position, get_macos_displays};

    let displays = get_macos_displays();

    // Find display containing mouse
    let target_display = if let Some((mouse_x, mouse_y)) = get_global_mouse_position() {
        displays
            .iter()
            .find(|display| {
                mouse_x >= display.origin_x
                    && mouse_x < display.origin_x + display.width
                    && mouse_y >= display.origin_y
                    && mouse_y < display.origin_y + display.height
            })
            .cloned()
    } else {
        None
    };

    // Use found display or fall back to primary
    let display = target_display.or_else(|| displays.first().cloned());

    if let Some(display) = display {
        // Position in top-right corner with padding
        let x = display.origin_x + display.width - width as f64 - padding as f64;
        let y = display.origin_y + padding as f64;

        gpui::Bounds::new(
            gpui::Point::new(px(x as f32), px(y as f32)),
            gpui::Size::new(px(width), px(height)),
        )
    } else {
        // Fallback to centered on primary
        gpui::Bounds::new(
            gpui::Point::new(px(100.0), px(100.0)),
            gpui::Size::new(px(width), px(height)),
        )
    }
}

/// Toggle the notes window (open if closed, close if open)
pub fn open_notes_window(cx: &mut App) -> Result<()> {
    use crate::logging;

    logging::log("PANEL", "open_notes_window called - checking toggle state");

    // Ensure gpui-component theme is initialized before opening window
    ensure_theme_initialized(cx);

    let window_handle = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = window_handle.lock().unwrap();

    // Check if window already exists and is valid
    if let Some(ref handle) = *guard {
        // Window exists - check if it's valid and close it (toggle OFF)
        if handle
            .update(cx, |_, window, _cx| {
                window.remove_window();
            })
            .is_ok()
        {
            logging::log("PANEL", "Notes window was open - closing (toggle OFF)");
            *guard = None;

            // NOTE: We intentionally do NOT call cx.hide() here.
            // Closing Notes should not affect the main window's ability to be shown.
            // The main window hotkey handles its own visibility state.
            // If the user wants to hide everything, they can press the main hotkey
            // when the main window is visible.

            return Ok(());
        }
        // Window handle was invalid, fall through to create new window
        logging::log("PANEL", "Notes window handle was invalid - creating new");
    }

    // If main window is visible, hide it (Notes takes focus)
    // Use platform::hide_main_window() to only hide the main window, not the whole app
    // IMPORTANT: Set visibility to false so the main hotkey knows to SHOW (not hide) next time
    if crate::is_main_window_visible() {
        logging::log(
            "PANEL",
            "Main window was visible - hiding it since Notes is opening",
        );
        crate::set_main_window_visible(false);
        crate::platform::hide_main_window();
    }

    // Create new window (toggle ON)
    logging::log("PANEL", "Notes window not open - creating new (toggle ON)");
    info!("Opening new notes window");

    // Calculate position: top-right corner of the display containing the mouse
    let window_width = 350.0_f32;
    let window_height = 280.0_f32;
    let padding = 20.0_f32; // Padding from screen edges

    let bounds = calculate_top_right_bounds(window_width, window_height, padding);

    // Load theme to determine window background appearance (vibrancy)
    let theme = crate::theme::load_theme();
    let window_background = if theme.is_vibrancy_enabled() {
        gpui::WindowBackgroundAppearance::Blurred
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };

    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("Notes".into()),
            appears_transparent: true,
            traffic_light_position: Some(gpui::Point {
                x: px(8.),
                y: px(8.),
            }),
        }),
        window_background,
        focus: true,
        show: true,
        kind: gpui::WindowKind::Normal,
        ..Default::default()
    };

    // Store the NotesApp entity so we can focus it after window creation
    let notes_app_holder: std::sync::Arc<std::sync::Mutex<Option<Entity<NotesApp>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let notes_app_for_closure = notes_app_holder.clone();

    let handle = cx.open_window(window_options, |window, cx| {
        let view = cx.new(|cx| NotesApp::new(window, cx));
        *notes_app_for_closure.lock().unwrap() = Some(view.clone());
        cx.new(|cx| Root::new(view, window, cx))
    })?;

    // CRITICAL: Activate the app FIRST before focusing the window
    // This brings the app to the foreground on macOS, which is required
    // for the window to receive keyboard focus when the app wasn't already active
    cx.activate(true);

    // CRITICAL: Hide the main window AFTER activating the app
    // When we activate the app, macOS may bring all windows to the front.
    // We need to explicitly hide the main window to prevent it from appearing.
    // This uses orderOut: which hides just the main window, not the entire app.
    crate::platform::hide_main_window();

    // Focus the editor input in the Notes window
    if let Some(notes_app) = notes_app_holder.lock().unwrap().clone() {
        let _ = handle.update(cx, |_root, window, cx| {
            window.activate_window();

            // Focus the NotesApp's editor input
            notes_app.update(cx, |app, cx| {
                // Call the InputState's focus method which handles both focus handle and internal state
                app.editor_state.update(cx, |state, inner_cx| {
                    state.focus(window, inner_cx);
                });

                if std::env::var("SCRIPT_KIT_TEST_NOTES_HOVERED")
                    .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
                {
                    app.force_hovered = true;
                    app.window_hovered = true;
                    app.titlebar_hovered = true;
                }

                if std::env::var("SCRIPT_KIT_TEST_NOTES_ACTIONS_PANEL")
                    .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
                {
                    app.open_actions_panel(window, cx);
                }

                cx.notify();
            });
        });
    }

    *guard = Some(handle);

    // Configure as floating panel (always on top) after window is created
    configure_notes_as_floating_panel();

    // Theme hot-reload watcher for Notes window
    // Spawns a background task that watches ~/.sk/kit/theme.json for changes
    if let Some(notes_app) = notes_app_holder.lock().unwrap().clone() {
        let notes_app_for_theme = notes_app.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            let (mut theme_watcher, theme_rx) = ThemeWatcher::new();
            if theme_watcher.start().is_err() {
                return;
            }
            loop {
                gpui::Timer::after(std::time::Duration::from_millis(200)).await;
                if theme_rx.try_recv().is_ok() {
                    info!("Notes window: theme.json changed, reloading");
                    let _ = cx.update(|cx| {
                        // Re-sync gpui-component theme with updated Script Kit theme
                        crate::theme::sync_gpui_component_theme(cx);
                        // Notify the Notes window to re-render with new colors
                        notes_app_for_theme.update(cx, |_app, cx| {
                            cx.notify();
                        });
                    });
                }
            }
        })
        .detach();
    }

    Ok(())
}

/// Quick capture - open notes with a new note ready for input
pub fn quick_capture(cx: &mut App) -> Result<()> {
    open_notes_window(cx)?;

    // TODO: Focus the editor and optionally create a new note
    // This requires accessing the NotesApp through the Root wrapper

    Ok(())
}

/// Close the notes window
pub fn close_notes_window(cx: &mut App) {
    let window_handle = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = window_handle.lock().unwrap();

    if let Some(handle) = guard.take() {
        let _ = handle.update(cx, |_, window, _| {
            window.remove_window();
        });
    }
}

/// Check if the notes window is currently open
///
/// Returns true if the Notes window exists and is valid.
/// This is used by other parts of the app to check if Notes is open
/// without affecting it.
pub fn is_notes_window_open() -> bool {
    let window_handle = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let guard = window_handle.lock().unwrap();
    guard.is_some()
}

/// Configure the Notes window as a floating panel (always on top).
///
/// This sets:
/// - NSFloatingWindowLevel (3) - floats above normal windows
/// - NSWindowCollectionBehaviorMoveToActiveSpace - moves to current space when shown
/// - Disabled window restoration - prevents macOS position caching
#[cfg(target_os = "macos")]
fn configure_notes_as_floating_panel() {
    use crate::logging;
    use std::ffi::CStr;

    unsafe {
        let app: id = NSApp();
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];

        for i in 0..count {
            let window: id = msg_send![windows, objectAtIndex: i];
            let title: id = msg_send![window, title];

            if title != nil {
                let title_cstr: *const i8 = msg_send![title, UTF8String];
                if !title_cstr.is_null() {
                    let title_str = CStr::from_ptr(title_cstr).to_string_lossy();

                    if title_str == "Notes" {
                        // Found the Notes window - configure it

                        // NSFloatingWindowLevel = 3
                        let floating_level: i32 = 3;
                        let _: () = msg_send![window, setLevel:floating_level];

                        // NSWindowCollectionBehaviorMoveToActiveSpace = 2
                        let collection_behavior: u64 = 2;
                        let _: () = msg_send![window, setCollectionBehavior:collection_behavior];

                        // Ensure window content is shareable for captureScreenshot()
                        let sharing_type: i64 = 1; // NSWindowSharingReadOnly
                        let _: () = msg_send![window, setSharingType:sharing_type];

                        // Disable window restoration
                        let _: () = msg_send![window, setRestorable:false];

                        logging::log(
                            "PANEL",
                            "Notes window configured as floating panel (level=3, MoveToActiveSpace)",
                        );
                        return;
                    }
                }
            }
        }

        logging::log(
            "PANEL",
            "Warning: Notes window not found by title for floating panel config",
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_notes_as_floating_panel() {
    // No-op on non-macOS platforms
}
