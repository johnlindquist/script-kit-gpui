//! Notes Window
//!
//! A separate floating window for notes, built with gpui-component.
//! This is completely independent from the main Script Kit launcher window.

use anyhow::Result;
use gpui::{
    div, prelude::*, px, size, App, Context, Entity, FocusHandle, Focusable, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, Styled, Subscription, Window, WindowBounds,
    WindowOptions,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    theme::ActiveTheme,
    IconName, Root, Sizable,
};
use tracing::info;

use super::model::{ExportFormat, Note, NoteId};
use super::storage;

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

    /// Search input state
    search_state: Entity<InputState>,

    /// Current search query
    search_query: String,

    /// Whether the sidebar is collapsed
    sidebar_collapsed: bool,

    /// Focus handle for keyboard navigation
    focus_handle: FocusHandle,

    /// Subscriptions to keep alive
    _subscriptions: Vec<Subscription>,
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

        // Create input states - use multi_line for the editor
        let editor_state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(20)
                .placeholder("Start typing your note...")
                .default_value(initial_content)
        });

        let search_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search notes..."));

        let focus_handle = cx.focus_handle();

        // Subscribe to editor changes
        let editor_sub = cx.subscribe_in(&editor_state, window, {
            move |this, _, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.on_editor_change(cx);
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

        info!(note_count = notes.len(), "Notes app initialized");

        Self {
            notes,
            deleted_notes,
            view_mode: NotesViewMode::AllNotes,
            selected_note_id,
            editor_state,
            search_state,
            search_query: String::new(),
            sidebar_collapsed: false,
            focus_handle,
            _subscriptions: vec![editor_sub, search_sub],
        }
    }

    /// Handle editor content changes
    fn on_editor_change(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_note_id {
            let content = self.editor_state.read(cx).value();

            // Update the note in our cache
            if let Some(note) = self.notes.iter_mut().find(|n| n.id == id) {
                note.set_content(content.to_string());

                // Save to storage (debounced in a real implementation)
                if let Err(e) = storage::save_note(note) {
                    tracing::error!(error = %e, "Failed to save note");
                }
            }

            cx.notify();
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

    /// Render the notes sidebar
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let notes = self.get_visible_notes();
        let selected_id = self.selected_note_id;
        let is_trash = self.view_mode == NotesViewMode::Trash;

        Sidebar::left()
            .collapsed(self.sidebar_collapsed)
            .header(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(if is_trash { "Trash" } else { "Notes" })
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .when(!is_trash, |d| {
                                        d.child(
                                            Button::new("new-note")
                                                .ghost()
                                                .small()
                                                .icon(IconName::Plus)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.create_note(window, cx);
                                                })),
                                        )
                                    })
                                    .child(
                                        Button::new("toggle-trash")
                                            .ghost()
                                            .small()
                                            .icon(if is_trash {
                                                IconName::ArrowLeft
                                            } else {
                                                IconName::Delete
                                            })
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                let new_mode = if is_trash {
                                                    NotesViewMode::AllNotes
                                                } else {
                                                    NotesViewMode::Trash
                                                };
                                                this.set_view_mode(new_mode, window, cx);
                                            })),
                                    ),
                            ),
                    )
                    .when(!is_trash, |d| d.child(self.render_search(cx))),
            )
            .child(
                SidebarGroup::new("notes-list").child(SidebarMenu::new().children(
                    notes.iter().map(|note| {
                        let note_id = note.id;
                        let is_selected = selected_id == Some(note_id);
                        let title: SharedString = if note.title.is_empty() {
                            "Untitled Note".into()
                        } else {
                            note.title.clone().into()
                        };

                        SidebarMenuItem::new(title)
                            .active(is_selected)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_note(note_id, window, cx);
                            }))
                    }),
                )),
            )
    }

    /// Render the main editor area
    fn render_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_trash = self.view_mode == NotesViewMode::Trash;
        let has_selection = self.selected_note_id.is_some();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            .p_4()
            .child(
                // Editor header with title and actions
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(
                                self.selected_note_id
                                    .and_then(|id| {
                                        self.get_visible_notes().iter().find(|n| n.id == id)
                                    })
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
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .when(has_selection && !is_trash, |d| {
                                d.child(self.render_export_menu(cx)).child(
                                    Button::new("delete")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Delete)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.delete_selected_note(cx);
                                        })),
                                )
                            })
                            .when(has_selection && is_trash, |d| {
                                d.child(
                                    Button::new("restore")
                                        .ghost()
                                        .small()
                                        .label("Restore")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.restore_note(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("permanent-delete")
                                        .ghost()
                                        .small()
                                        .label("Delete Forever")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.permanently_delete_note(cx);
                                        })),
                                )
                            }),
                    ),
            )
            .when(!is_trash && has_selection, |d| {
                d.child(self.render_toolbar(cx))
            })
            .child(
                // Editor content - full height multi-line input
                div()
                    .flex_1()
                    .pt_4()
                    .child(Input::new(&self.editor_state).h_full()),
            )
    }
}

impl Focusable for NotesApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for NotesApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                // Handle keyboard shortcuts
                let key = event.keystroke.key.as_str();
                let modifiers = &event.keystroke.modifiers;

                // platform modifier = Cmd on macOS, Ctrl on Windows/Linux
                if modifiers.platform {
                    match key {
                        "n" => this.create_note(window, cx),
                        "b" => this.insert_formatting("**", "**", cx),
                        "i" => this.insert_formatting("_", "_", cx),
                        _ => {}
                    }
                }
            }))
            .child(self.render_sidebar(cx))
            .child(self.render_editor(cx))
    }
}

/// Open the notes window (or focus it if already open)
pub fn open_notes_window(cx: &mut App) -> Result<()> {
    let window_handle = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = window_handle.lock().unwrap();

    // Check if window already exists and is valid
    if let Some(ref handle) = *guard {
        // Try to focus the existing window
        if handle.update(cx, |_, _, cx| cx.notify()).is_ok() {
            info!("Focusing existing notes window");
            return Ok(());
        }
    }

    // Create new window
    info!("Opening new notes window");

    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(gpui::Bounds::centered(
            None,
            size(px(900.), px(700.)),
            cx,
        ))),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("Script Kit Notes".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        focus: true,
        show: true,
        kind: gpui::WindowKind::Normal,
        ..Default::default()
    };

    let handle = cx.open_window(window_options, |window, cx| {
        let view = cx.new(|cx| NotesApp::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })?;

    *guard = Some(handle);

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
