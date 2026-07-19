//! Notes Window
//!
//! A separate floating window for notes, built with gpui-component.
//! This is completely independent from the main Script Kit launcher window.

use anyhow::Result;
use gpui::{
    div, prelude::*, px, rgba, size, AnyElement, App, Context, CursorStyle, Entity, FocusHandle,
    Focusable, IntoElement, KeyDownEvent, MouseMoveEvent, ParentElement, Render, Styled,
    Subscription, Window, WindowBounds, WindowOptions,
};

use chrono::NaiveDate;
#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
use gpui_component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, Search},
    theme::ActiveTheme,
    IconName, Root, Sizable, WindowExt as _,
};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{debug, info};

// Use the unified ActionsDialog/CommandBar system
use crate::actions::{
    get_day_note_switcher_actions, get_note_switcher_actions, get_notes_command_bar_actions,
    CommandBar, CommandBarConfig, NoteSwitcherNoteInfo, NotesInfo,
};
use crate::confirm;
use crate::theme;

use super::actions_panel::NotesAction;
use super::markdown;
use super::model::{ExportFormat, Note, NoteId};
use super::storage;
use crate::components::notes_editor::spine::NotesEditorSpineRuntime;
use crate::components::notes_editor::NotesEditor;

/// Global handle to the notes window
static NOTES_WINDOW: std::sync::OnceLock<std::sync::Mutex<Option<gpui::WindowHandle<Root>>>> =
    std::sync::OnceLock::new();

/// Global handle to the NotesApp entity for quick_capture access
static NOTES_APP_ENTITY: std::sync::OnceLock<std::sync::Mutex<Option<Entity<NotesApp>>>> =
    std::sync::OnceLock::new();

/// Monotonic id distinguishing NotesApp instances. GPUI releases entities
/// lazily (on a later effect flush), so a replaced instance's Drop can run
/// AFTER a new window has already stored fresh globals; Drop must only clear
/// globals its own instance still owns (see `impl Drop for NotesApp`).
static NOTES_INSTANCE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CURRENT_NOTES_INSTANCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub type NotesRunCommandExecutor = fn(command_id: &str, cx: &mut App) -> Result<bool, String>;

static NOTES_RUN_COMMAND_EXECUTOR: std::sync::OnceLock<NotesRunCommandExecutor> =
    std::sync::OnceLock::new();

pub fn register_notes_run_command_executor(executor: NotesRunCommandExecutor) {
    if NOTES_RUN_COMMAND_EXECUTOR.set(executor).is_err() {
        tracing::warn!(
            event = "notes_run_command_executor_already_registered",
            "Notes run command executor was already registered"
        );
    }
}

pub(crate) fn execute_notes_run_command_deeplink(
    command_id: &str,
    cx: &mut App,
) -> Result<bool, String> {
    let Some(executor) = NOTES_RUN_COMMAND_EXECUTOR.get().copied() else {
        return Err("No Notes run command executor is registered".to_string());
    };
    executor(command_id, cx)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotesCloseBehavior {
    RestoreLauncher,
    LeaveLauncherHidden,
}

static NOTES_CLOSE_BEHAVIOR: std::sync::OnceLock<std::sync::Mutex<NotesCloseBehavior>> =
    std::sync::OnceLock::new();

#[derive(Debug, Clone)]
struct NotesDayBinding {
    date: NaiveDate,
    path: PathBuf,
    content: String,
    /// Disk content last read/written for this binding — the baseline used to
    /// detect and merge concurrent external appends (`;todo`, clipboard
    /// sediment, dictation) so a capture is never clobbered by an editor save.
    base_disk_content: String,
}

// NOTE: Theme watching is now centralized in crate::theme::service
// The per-window NOTES_THEME_WATCHER_RUNNING flag has been removed

// =============================================================================
// Layout constants — all on the 4 px micro-grid / 8 px base grid
// =============================================================================

/// Note-switcher panel width (used by DevTools layout introspection).
const BROWSE_PANEL_WIDTH: f32 = 500.0;

/// Note-switcher panel max height (used by DevTools layout introspection).
const BROWSE_PANEL_MAX_HEIGHT: f32 = 400.0;

/// Actions-panel overlay top offset (keeps search input stable).
const ACTIONS_PANEL_TOP_OFFSET: f32 = 32.0;

// =============================================================================
// Semantic opacity levels — keeps contrast consistent and reduces magic numbers.
//
//   OPACITY_DISABLED  → 0.4  (disabled / inactive — floor for legibility)
//   OPACITY_SUBTLE    → 0.5  (rest-state chrome, de-emphasized metadata)
//   OPACITY_MUTED     → 0.7  (secondary info that should still be easy to read)
//   OPACITY_VISIBLE   → 1.0  (full emphasis)
//
// When applied to `muted_foreground`, the lowest tier (0.4) still exceeds
// WCAG 2.2 AA 3:1 non-text contrast on both dark and light surfaces.
// =============================================================================

/// Opacity for disabled / completely de-emphasized elements.
pub(super) const OPACITY_DISABLED: f32 = 0.4;

/// Opacity for rest-state chrome (unhovered footer, subtle metadata).
const OPACITY_SUBTLE: f32 = 0.5;

/// Opacity for secondary information (icons, timestamps, line info).
const OPACITY_MUTED: f32 = 0.7;

/// Minimum interactive target size (px) — WCAG 2.2 §2.5.8 (24 × 24 CSS px).
const MIN_TARGET_SIZE: f32 = 24.0;

// =============================================================================
// Derived / contextual opacity tokens — extracted from inline magic numbers
// so every transparency in this file is auditable in one place.
// =============================================================================

/// Very subtle border dividers inside overlays (section separators).
const OPACITY_SECTION_BORDER: f32 = 0.2;

/// Accent-tinted border for trash-view indicator.
const OPACITY_ACCENT_BORDER: f32 = 0.35;

/// Width reserved for macOS traffic-light buttons in the titlebar (px).
const TITLEBAR_TRAFFIC_LIGHT_W: f32 = 60.0;

/// Width reserved for the right-side icon cluster in the titlebar (px).
const TITLEBAR_ICONS_W: f32 = 100.0;

/// Footer separator: Unicode middle-dot used between stats in the footer.
const FOOTER_SEP: &str = " · ";

// =============================================================================
// Auto-resize constants — govern how the window grows / shrinks with content.
// =============================================================================

/// Delta beyond which we assume the user grabbed the edge to resize manually.
const MANUAL_RESIZE_THRESHOLD: f32 = 10.0;

// =============================================================================
// Timing constants
// =============================================================================

/// Duration (ms) of the brief "✓ Saved" flash after a successful save.
const SAVED_FLASH_MS: u64 = 1500;

/// Duration (ms) of action feedback flash ("Deleted", "Pinned", etc.).
const ACTION_FEEDBACK_MS: u64 = 2000;

/// Extra vertical space reserved around the actions panel overlay (px).
const ACTIONS_PANEL_WINDOW_MARGIN: f32 = 64.0;

/// View mode for the notes list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotesViewMode {
    /// Show all active notes
    #[default]
    AllNotes,
    /// Show deleted notes (trash)
    Trash,
}

/// Which surface is currently visible inside the Notes window.
///
/// The Notes window is a persistent host that can show either the editor
/// or an embedded Agent Chat chat.  Switching modes does not destroy state — the
/// inactive surface is hidden, not dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotesSurfaceMode {
    /// The Notes editor (default).
    #[default]
    Notes,
    /// An embedded Agent Chat chat session inside the Notes window.
    AgentChat,
}

#[derive(Debug, Clone)]
struct NotesFocusTransition {
    generation: u64,
    phase: &'static str,
    surface: focus::NotesFocusSurface,
    previous_surface: focus::NotesFocusSurface,
    command_bar_open: bool,
    note_switcher_open: bool,
    has_active_dialog: bool,
    surface_mode: NotesSurfaceMode,
    recorded_at: Instant,
}

#[derive(Debug, Clone)]
struct NotesAutosizeTransition {
    generation: u64,
    cause: &'static str,
    before_height: f32,
    after_height: f32,
    before_width: f32,
    after_width: f32,
    line_count: usize,
    desired_height: f32,
    clamped_height: f32,
    applied: bool,
    skipped_reason: Option<&'static str>,
    recorded_at: Instant,
}

#[derive(Debug, Clone)]
struct NotesMentionPortalEditSession {
    mention_range: Range<usize>,
    original_token: String,
}

#[derive(Debug, Clone)]
struct NotesGhostActionReceipt {
    kind: &'static str,
    source_kind: crate::notes::ghost::NotesGhostSourceKind,
    suffix_len: usize,
    inserted_len: usize,
    remaining_len: usize,
    accepted_fingerprint: Option<String>,
    accepted_leading_whitespace_len: usize,
    accepted_non_whitespace_len: usize,
    suffix_fingerprint: String,
    recorded_at: Instant,
}

impl NotesGhostActionReceipt {
    fn accepted_word(
        prediction: &crate::notes::ghost::NotesGhostPrediction,
        inserted_suffix: &str,
    ) -> Self {
        Self::from_prediction("acceptedWord", prediction, inserted_suffix)
    }

    fn accepted_full(prediction: &crate::notes::ghost::NotesGhostPrediction) -> Self {
        Self::from_prediction("acceptedFull", prediction, &prediction.suffix)
    }

    fn dismissed(prediction: &crate::notes::ghost::NotesGhostPrediction) -> Self {
        Self::from_prediction("dismissed", prediction, "")
    }

    fn stale(prediction: &crate::notes::ghost::NotesGhostPrediction) -> Self {
        Self::from_prediction("stale", prediction, "")
    }

    fn from_prediction(
        kind: &'static str,
        prediction: &crate::notes::ghost::NotesGhostPrediction,
        inserted_suffix: &str,
    ) -> Self {
        let suffix_len = prediction.suffix.chars().count();
        let inserted_len = inserted_suffix.chars().count();
        Self {
            kind,
            source_kind: prediction.source_kind,
            suffix_len,
            inserted_len,
            remaining_len: suffix_len.saturating_sub(inserted_len),
            accepted_fingerprint: if inserted_suffix.is_empty() {
                None
            } else {
                Some(NotesApp::devtools_text_fingerprint(inserted_suffix))
            },
            accepted_leading_whitespace_len: inserted_suffix
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .count(),
            accepted_non_whitespace_len: inserted_suffix
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .count(),
            suffix_fingerprint: NotesApp::devtools_text_fingerprint(&prediction.suffix),
            recorded_at: Instant::now(),
        }
    }
}

/// A cached, already-sanitized LLM ghost suffix (see `recompute_notes_ghost`).
#[derive(Debug, Clone)]
struct NotesGhostLlmCacheEntry {
    note_id: Option<NoteId>,
    line_prefix: String,
    suffix: String,
    inserted_at: Instant,
}

#[derive(Debug, Clone)]
pub(super) struct NotesKitResourcePreviewState {
    title: String,
    uri: String,
    mime_type: String,
    text: String,
    truncated: bool,
}

impl From<crate::notes::deeplink_activation::KitResourcePreview> for NotesKitResourcePreviewState {
    fn from(preview: crate::notes::deeplink_activation::KitResourcePreview) -> Self {
        Self {
            title: preview.title,
            uri: preview.uri,
            mime_type: preview.mime_type,
            text: preview.text,
            truncated: preview.truncated,
        }
    }
}

/// Sort mode for the notes list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotesSortMode {
    /// Sort by last updated (most recent first) — default
    #[default]
    Updated,
    /// Sort by creation date (newest first)
    Created,
    /// Sort alphabetically by title (A→Z)
    Alphabetical,
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

    /// Shared markdown editor component (Day Page will host the same surface).
    notes_editor: Entity<NotesEditor>,

    /// Editor input state (using gpui-component's Input)
    pub(crate) editor_state: Entity<InputState>,

    /// Shared editor-spine runtime for safe Notes inline command rows.
    spine_runtime: NotesEditorSpineRuntime<crate::spine::SpineListRow>,

    /// Search input state (for future browse panel)
    search_state: Entity<InputState>,

    /// Current search query (for future browse panel)
    search_query: String,

    /// Whether the titlebar is being hovered (for showing/hiding icons)
    titlebar_hovered: bool,

    /// Whether the entire window is being hovered (for traffic lights)
    window_hovered: bool,

    /// Whether the mouse cursor is currently hidden
    mouse_cursor_hidden: bool,

    /// Forces hover chrome for visual tests
    force_hovered: bool,

    /// Whether the formatting toolbar is pinned open
    show_format_toolbar: bool,

    /// Whether the search bar is shown (Cmd+F)
    show_search: bool,

    /// Whether markdown preview is enabled (Cmd+Shift+P)
    preview_enabled: bool,

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
    /// Monotonic generation for DevTools auto-resize transition receipts.
    autosize_generation: u64,
    /// Last auto-resize decision for before/after resize comparison proof.
    last_autosize_transition: Option<NotesAutosizeTransition>,
    /// Focus handle for keyboard navigation
    focus_handle: FocusHandle,

    /// Subscriptions to keep alive
    _subscriptions: Vec<Subscription>,

    /// Render receipt: the deeplink hover hint chip built during the last
    /// editor-body render (`{"verb", "href"}`), or `None` when no chip was
    /// rendered. Written only on the render path so automation can prove the
    /// hover→re-render→chip pipeline, not just the hover state.
    pub(super) last_deeplink_hover_hint: Option<serde_json::Value>,

    /// Command bar component (Cmd+K) - uses unified CommandBar wrapper
    /// Opens in a separate vibrancy window for proper macOS blur effect
    command_bar: CommandBar,

    /// Note switcher command bar (Cmd+P) - uses unified CommandBar wrapper
    /// Opens in a separate vibrancy window for proper macOS blur effect
    note_switcher: CommandBar,

    /// File-backed day note currently open in the Notes editor.
    active_day_binding: Option<NotesDayBinding>,

    /// Merged day-page content awaiting reconciliation into the editor buffer.
    /// Set when a save merged in concurrent external appends; drained on the
    /// next render tick so the editor shows the captured lines.
    pending_day_editor_reconcile: Option<String>,

    /// Debounce: Whether the current note has unsaved changes
    has_unsaved_changes: bool,

    /// Which NotesApp generation this is; guards Drop's global cleanup.
    instance_id: u64,

    /// Debounce: Last time we saved (to avoid too-frequent saves)
    last_save_time: Option<Instant>,

    /// Track last persisted bounds for debounced save on close paths
    /// (traffic light, Cmd+W, toggle) that don't go through close_notes_window
    last_persisted_bounds: Option<gpui::WindowBounds>,

    /// Last time we saved bounds (debounce to avoid too-frequent saves)
    last_bounds_save: Instant,

    /// Theme revision seen - used to detect theme changes and recompute cached values
    theme_rev_seen: u64,

    /// History stack for back navigation (Cmd+[)
    /// Stores previously viewed note IDs (most recent at the end)
    history_back: Vec<NoteId>,

    /// History stack for forward navigation (Cmd+])
    /// Populated when user navigates back
    history_forward: Vec<NoteId>,

    /// Flag to suppress history push during back/forward navigation
    navigating_history: bool,

    /// Whether focus mode is enabled (Cmd+.) — hides all chrome for distraction-free writing
    focus_mode: bool,

    /// Current sort mode for notes list
    sort_mode: NotesSortMode,

    /// Instant when the last save completed — used for brief "Saved" flash in footer
    last_save_confirmed: Option<Instant>,

    /// Brief action feedback message shown in footer (e.g. "Deleted", "Pinned", "Duplicated")
    /// Tuple of (message, accent_colored, timestamp). Clears after 2 seconds.
    action_feedback: Option<(String, bool, Instant)>,

    /// Pending focus surface request — applied in the next render frame.
    /// Used to defer focus changes until after dialog dismissal completes.
    pending_focus_surface: Option<focus::NotesFocusSurface>,
    /// Monotonic generation for DevTools focus-owner transition receipts.
    focus_transition_generation: u64,
    /// Bounded recent focus-owner transition timeline for runtime UX proof.
    focus_transition_log: Vec<NotesFocusTransition>,

    /// Current deterministic ghost autocomplete prediction for the editor.
    notes_ghost_prediction: Option<crate::notes::ghost::NotesGhostPrediction>,
    /// Monotonic generation used to reject stale ghost autocomplete accepts.
    notes_ghost_generation: u64,
    /// Last ghost autocomplete action, redacted for DevTools receipts.
    notes_ghost_last_action: Option<NotesGhostActionReceipt>,
    /// Monotonic generation used to reject stale LLM ghost side-channel
    /// results (bumped on cancel; see `cancel_notes_ghost_llm`).
    notes_ghost_llm_generation: u64,
    /// Cooperative cancel for the in-flight LLM ghost request, if any.
    notes_ghost_llm_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Tiny TTL cache of sanitized LLM ghost suffixes so backspacing over a
    /// hint re-serves it without another model call.
    notes_ghost_llm_cache: std::collections::VecDeque<NotesGhostLlmCacheEntry>,

    // ── Agent Chat host surface ──────────────────────────────────────────────
    /// Which surface is currently visible (Notes editor or embedded Agent Chat).
    surface_mode: NotesSurfaceMode,

    /// Cached Agent Chat chat entity — survives mode switches so conversation state
    /// is preserved when toggling between Notes and Agent Chat.
    embedded_agent_chat: Option<Entity<crate::ai::agent_chat::ui::AgentChatView>>,
    /// Generation for the currently embedded Agent Chat view, used to reject stale popup actions.
    notes_agent_chat_generation: u64,
    /// Active inline mention replacement session for note-local `@note`
    /// reopen/replace flows via the note switcher.
    mention_portal_edit: Option<NotesMentionPortalEditSession>,
    /// Read-only preview opened from a `kit://` resource link in note markdown.
    kit_resource_preview: Option<NotesKitResourcePreviewState>,
}

mod agent_chat_host;
mod clipboard_ops;
pub(crate) mod contract;
mod editor_formatting;
mod editor_ops_a;
mod editor_ops_b;
mod focus;
pub(crate) mod style;
use focus::NotesFocusSurface;
mod init;
mod keyboard;
mod navigation;
mod notes;
mod notes_actions;
mod panels;
mod render;
mod render_editor;
mod render_editor_body;
mod render_editor_footer;
mod render_editor_titlebar;
mod render_ui;
mod traits;
mod vibrancy;
mod window_ops;

pub use agent_chat_host::close_notes_embedded_agent_chat;
pub(crate) use agent_chat_host::NOTES_EMBEDDED_AI_AUTOMATION_ID;
pub(crate) use window_ops::update_notes_window_detached;
pub use window_ops::{
    accept_notes_ghost_for_automation, apply_mcp_notes_mutation_on_main_thread, close_notes_window,
    get_notes_app_entity_and_handle, get_notes_editor_runtime_info, get_notes_editor_text,
    handle_notes_editor_key_for_automation, handle_notes_ghost_key_for_automation,
    inject_text_into_notes, is_notes_window, is_notes_window_open, open_day_note_in_notes_window,
    open_note_in_notes_window, open_notes_search, open_notes_window,
    open_notes_window_without_launcher_restore, quick_capture, save_note_with_content,
    save_note_with_content_and_source, toggle_notes_popup_for_automation,
};

#[cfg(test)]
mod tests;
