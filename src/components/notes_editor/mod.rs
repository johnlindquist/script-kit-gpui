//! Shared markdown notes editor component.
//!
//! Hosts markdown editing, formatting operations, preview rendering, and editor
//! focus for the Notes window and future Day Page surface. The host binds
//! document content, wires save/change callbacks, and owns window chrome.

mod component;
pub(crate) mod contract;
mod formatting;
mod ops;
mod render;
pub(crate) mod spine;
mod toolbar;
mod types;

pub(crate) use component::should_activate_deeplink_from_mouse_up;
pub use component::NotesEditor;
pub(crate) use ops::observed_replacement_range;
pub use toolbar::{
    notes_editor_toolbar_action_by_id, notes_editor_toolbar_action_title, NotesEditorToolbarAction,
    NOTES_EDITOR_TOOLBAR_ACTIONS,
};
pub use types::{
    NotesEditorConfig, NotesEditorInputSizing, NotesEditorLayout, NotesEditorMarkdownConfig,
    NotesEditorSurfaceStyle, NOTES_EDITOR_PREVIEW_RENDER_PATH, NOTES_EDITOR_STYLE_OWNER,
};
