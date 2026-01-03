//! Notes Module - Raycast Notes Feature Parity
//!
//! A separate floating notes window built with gpui-component library.
//!
//! ## Features
//! - Floating notes window with global hotkey access
//! - Markdown/rich text editing with live preview
//! - Multiple notes management with sidebar list
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


// Allow dead code in this module - many functions are designed for future use
#![allow(dead_code)]

mod actions_panel;
mod browse_panel;
mod model;
mod storage;
mod window;

// Re-export actions panel types for use by window.rs
#[allow(unused_imports)]
pub use actions_panel::{NotesAction, NotesActionCallback, NotesActionItem, NotesActionsPanel};

// Re-export browse panel types for use by window.rs
#[allow(unused_imports)]
pub use browse_panel::{BrowsePanel, NoteAction, NoteListItem};

// Re-export key types - suppress unused warnings since these are public API
#[allow(unused_imports)]
pub use model::*;
#[allow(unused_imports)]
pub use storage::*;
#[allow(unused_imports)]
pub use window::{
    close_notes_window, is_notes_window_open, open_notes_window, quick_capture, NotesApp,
};
