//! Notes Module - Raycast Notes Feature Parity
//!
//! A separate floating notes window built with gpui-component library.
//!
//! ## Features
//! - Floating notes window with global hotkey access
//! - Markdown/rich text editing with live preview
//! - Multiple notes, one visible at a time (Cmd+P note switcher; no sidebar)
//! - Quick capture from anywhere
//! - Auto-sizing window that grows with content
//! - Persistent storage (local SQLite)
//! - Formatting toolbar with keyboard shortcuts
//! - Search across all notes
//! - Export (plain text, markdown, HTML)
//! - Menu bar integration
//! - Recently deleted notes (soft delete with recovery)
//!
//! ## Architecture
//! The Notes feature runs in a completely separate window from the main Script Kit
//! launcher. It uses gpui-component for UI components (Input, Sidebar, Button, etc.)
//! and follows the Root wrapper pattern required by gpui-component.
//!
//! # API Visibility
//!
//! Storage internals are private implementation details of the notes window.
//!
//! ```compile_fail
//! use script_kit_gpui::notes::save_note;
//! use script_kit_gpui::notes::get_all_notes;
//! ```
//!

// Allow dead code in this module - many functions are designed for future use
#![allow(dead_code)]

mod actions_panel;
pub(crate) mod code_highlight;
pub(crate) mod day_switcher;
pub(crate) mod deeplink_activation;
pub(crate) mod ghost;
pub(crate) mod ghost_llm;
pub(crate) mod markdown;
pub(crate) mod markdown_highlighting;
pub(crate) mod menu_syntax_capture;
pub(crate) mod metadata;
mod model;
mod storage;
pub(crate) mod window;

// Re-export notes action catalog for CommandBar action builders
#[allow(unused_imports)]
pub use actions_panel::NotesAction;

#[allow(unused_imports)]
pub(crate) use model::{Note, NoteCartItem, NoteCartItemPayload, NoteId};
#[allow(unused_imports)]
pub(crate) use storage::{
    count_active_notes_with_tag, delete_note_cart_item, delete_note_cart_items,
    delete_note_permanently, get_all_notes, get_deleted_notes, get_note, get_note_aliases,
    get_note_backlink_count, get_note_backlinks, get_note_outbound_link_count, get_note_tags,
    init_notes_db, list_note_cart_items, list_note_cart_items_deduped, note_file_path,
    notes_brain_days_dir, root_notes_query_is_eligible, save_note, save_note_cart_item,
    search_notes, search_root_notes_meta, search_root_notes_meta_cached,
    search_root_notes_meta_direct, NoteBacklinkSummary, RootNoteSearchHit, RootNotesSectionOptions,
};

/// Tag that promotes a note to a standing agent instruction.
///
/// Notes tagged `#instructions` (or `tags: [instructions]` frontmatter) are
/// automatically staged as context on new Agent Chat threads.
pub const NOTES_INSTRUCTIONS_TAG: &str = "instructions";

/// MCP resource URI that resolves all instruction notes with full bodies.
pub const NOTES_INSTRUCTIONS_RESOURCE_URI: &str = "kit://notes?tag=instructions&full=true";

/// Label shown on the instructions context chip in Agent Chat.
pub const NOTES_INSTRUCTIONS_LABEL: &str = "Note Instructions";

/// Provenance URI recorded in `source:` frontmatter for notes created from an
/// Agent Chat thread. Keeps the note ↔ conversation relationship in the data
/// layer so any surface can link back.
pub fn agent_chat_thread_source(ui_thread_id: &str) -> String {
    format!("scriptkit://agent-chat/{ui_thread_id}")
}

// Re-export key types - suppress unused warnings since these are public API
#[allow(unused_imports)]
pub(crate) use window::update_notes_window_detached;
#[allow(unused_imports)]
pub use window::{
    accept_notes_ghost_for_automation, apply_mcp_notes_mutation_on_main_thread,
    close_notes_embedded_agent_chat, close_notes_window, get_notes_app_entity_and_handle,
    get_notes_editor_runtime_info, get_notes_editor_text, get_notes_surface_mode,
    handle_notes_editor_key_for_automation, handle_notes_ghost_key_for_automation,
    inject_text_into_notes, is_notes_window, is_notes_window_open, open_day_note_in_notes_window,
    open_note_in_notes_window, open_notes_search, open_notes_window,
    open_notes_window_without_launcher_restore, quick_capture, register_notes_run_command_executor,
    save_note_with_content, save_note_with_content_and_source, toggle_notes_popup_for_automation,
    NotesApp, NotesRunCommandExecutor, NotesSurfaceMode,
};
