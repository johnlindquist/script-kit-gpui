#![allow(unexpected_cfgs)]

use gpui::{
    div, hsla, list, point, prelude::*, px, rgb, rgba, size, svg, uniform_list, AnyElement, App,
    Application, BoxShadow, Context, ElementId, Entity, FocusHandle, Focusable, ListAlignment,
    ListSizingBehavior, ListState, Render, ScrollStrategy, SharedString, Timer,
    UniformListScrollHandle, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowOptions,
};

// gpui-component Root wrapper for theme and context provision
use gpui_component::notification::{Notification, NotificationType};
use gpui_component::Root;
use std::sync::atomic::{AtomicBool, Ordering};

mod process_manager;
use cocoa::base::id;
use cocoa::foundation::NSRect;
use process_manager::PROCESS_MANAGER;

// Platform utilities - mouse position, display info, window movement, screenshots
use platform::{
    calculate_eye_line_bounds_on_mouse_display, capture_app_screenshot, capture_window_by_title,
    move_first_window_to_bounds,
};
#[macro_use]
extern crate objc;

mod actions;
mod ai;
mod components;
mod config;
mod designs;
mod editor;
mod error;
mod executor;
mod filter_coalescer;
mod form_prompt;
#[allow(dead_code)] // TODO: Re-enable once hotkey_pollers is updated for Root wrapper
mod hotkey_pollers;
mod hotkeys;
mod list_item;
mod logging;
mod login_item;
mod navigation;
mod panel;
mod perf;
mod platform;
mod prompts;
mod protocol;
mod scripts;
#[cfg(target_os = "macos")]
mod selected_text;
mod setup;
mod shortcuts;
mod stdin_commands;
mod syntax;
mod term_prompt;
mod terminal;
mod theme;
mod transitions;
mod tray;
mod utils;
mod watcher;
mod window_manager;
mod window_resize;

// Phase 1 system API modules
mod clipboard_history;
mod file_search;
mod toast_manager;
mod window_control;

// Built-in features registry
mod app_launcher;
mod builtins;

// Frecency tracking for script usage
mod frecency;

// Scriptlet parsing and variable substitution
mod scriptlets;

// Typed metadata parser for new `metadata = {}` global syntax
mod metadata_parser;

// Schema parser for `schema = { input: {}, output: {} }` definitions
mod schema_parser;

// Scriptlet codefence metadata parser for ```metadata and ```schema blocks
mod scriptlet_metadata;

// VSCode snippet syntax parser for template() SDK function
mod snippet;

// HTML form parsing for form() prompt
mod form_parser;

// Centralized template variable substitution system
mod template_variables;

// Text expansion system components (macOS only)
mod expand_matcher;
#[cfg(target_os = "macos")]
mod keyboard_monitor;
mod text_injector;

// Expand manager - text expansion system integration
#[cfg(target_os = "macos")]
mod expand_manager;

// Script scheduling with cron expressions and natural language
mod scheduler;

// HUD manager - system-level overlay notifications (separate floating windows)
mod hud_manager;

// MCP Server modules for AI agent integration
mod mcp_kit_tools;
mod mcp_protocol;
mod mcp_resources;
mod mcp_script_tools;
mod mcp_server;
mod mcp_streaming;

// Notes - Raycast Notes feature parity (separate floating window)
mod notes;

use crate::components::text_input::TextInputState;
use crate::components::toast::{Toast, ToastAction};
use crate::error::ErrorSeverity;
use crate::filter_coalescer::FilterCoalescer;
use crate::form_prompt::FormPromptState;
// TODO: Re-enable when hotkey_pollers.rs is updated for Root wrapper
// use crate::hotkey_pollers::start_hotkey_event_handler;
use crate::navigation::{NavCoalescer, NavDirection, NavRecord};
use crate::toast_manager::{PendingToast, ToastManager};
use components::ToastVariant;
use editor::EditorPrompt;
use prompts::{
    ContainerOptions, ContainerPadding, DivPrompt, DropPrompt, EnvPrompt, PathInfo, PathPrompt,
    SelectPrompt, TemplatePrompt,
};
use tray::{TrayManager, TrayMenuAction};
use window_resize::{
    defer_resize_to_view, height_for_view, initial_window_height, reset_resize_debounce,
    resize_first_window_to_height, ViewType,
};

use components::{
    Button, ButtonColors, ButtonVariant, FormFieldColors, Scrollbar, ScrollbarColors,
};
use designs::{get_tokens, render_design_item, DesignVariant};
use frecency::FrecencyStore;
use list_item::{
    render_section_header, GroupedListItem, ListItem, ListItemColors, LIST_ITEM_HEIGHT,
    SECTION_HEADER_HEIGHT,
};
use scripts::get_grouped_results;
// strip_html_tags removed - DivPrompt now renders HTML properly

use actions::{ActionsDialog, ScriptInfo};
use panel::{
    CURSOR_GAP_X, CURSOR_HEIGHT_LG, CURSOR_MARGIN_Y, CURSOR_WIDTH, DEFAULT_PLACEHOLDER, HEADER_GAP,
    HEADER_PADDING_X, HEADER_PADDING_Y,
};
use parking_lot::Mutex as ParkingMutex;
use protocol::{Choice, Message, ProtocolAction};
use std::sync::{mpsc, Arc, Mutex};
use syntax::highlight_code_lines;

/// Channel for sending prompt messages from script thread to UI
#[allow(dead_code)]
type PromptChannel = (mpsc::Sender<PromptMessage>, mpsc::Receiver<PromptMessage>);

// Import utilities from modules
use stdin_commands::{start_stdin_listener, ExternalCommand};
use utils::render_path_with_highlights;

// Global state for hotkey signaling between threads
static MAIN_WINDOW_VISIBLE: AtomicBool = AtomicBool::new(false); // Track window visibility for toggle (starts hidden)
static NEEDS_RESET: AtomicBool = AtomicBool::new(false); // Track if window needs reset to script list on next show

/// Check if the main window is currently visible
pub fn is_main_window_visible() -> bool {
    MAIN_WINDOW_VISIBLE.load(Ordering::SeqCst)
}

/// Set the main window visibility state  
pub fn set_main_window_visible(visible: bool) {
    MAIN_WINDOW_VISIBLE.store(visible, Ordering::SeqCst);
}
static PANEL_CONFIGURED: AtomicBool = AtomicBool::new(false); // Track if floating panel has been configured (one-time setup on first show)
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false); // Track if shutdown signal received (prevents new script spawns)

/// Convert our ToastVariant to gpui-component's NotificationType
fn toast_variant_to_notification_type(variant: ToastVariant) -> NotificationType {
    match variant {
        ToastVariant::Success => NotificationType::Success,
        ToastVariant::Warning => NotificationType::Warning,
        ToastVariant::Error => NotificationType::Error,
        ToastVariant::Info => NotificationType::Info,
    }
}

/// Convert a PendingToast to a gpui-component Notification
fn pending_toast_to_notification(toast: &PendingToast) -> Notification {
    let notification_type = toast_variant_to_notification_type(toast.variant);

    let mut notification = Notification::new()
        .message(&toast.message)
        .with_type(notification_type);

    // Add title for errors/warnings (makes them stand out more)
    match toast.variant {
        ToastVariant::Error => {
            notification = notification.title("Error");
        }
        ToastVariant::Warning => {
            notification = notification.title("Warning");
        }
        _ => {}
    }

    // Note: gpui-component Notification has fixed 5s autohide
    // For persistent toasts, set autohide(false)
    if toast.duration_ms.is_none() {
        notification = notification.autohide(false);
    }

    notification
}

/// Check if shutdown has been requested (prevents new script spawns during shutdown)
#[allow(dead_code)]
pub fn is_shutting_down() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Application state - what view are we currently showing
#[derive(Debug, Clone)]
enum AppView {
    /// Showing the script list
    ScriptList,
    /// Showing the actions dialog (mini searchable popup)
    #[allow(dead_code)]
    ActionsDialog,
    /// Showing an arg prompt from a script
    ArgPrompt {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        actions: Option<Vec<ProtocolAction>>,
    },
    /// Showing a div prompt from a script
    DivPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<DivPrompt>,
    },
    /// Showing a form prompt from a script (HTML form with submit button)
    FormPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<FormPromptState>,
    },
    /// Showing a terminal prompt from a script
    TermPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<term_prompt::TermPrompt>,
    },
    /// Showing an editor prompt from a script
    EditorPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<EditorPrompt>,
        /// Separate focus handle for the editor (not shared with parent)
        focus_handle: FocusHandle,
    },
    /// Showing a select prompt from a script (multi-select)
    SelectPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<SelectPrompt>,
    },
    /// Showing a path prompt from a script (file/folder picker)
    PathPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<PathPrompt>,
        focus_handle: FocusHandle,
    },
    /// Showing env prompt for environment variable input with keyring storage
    EnvPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<EnvPrompt>,
    },
    /// Showing drop prompt for drag and drop file handling
    DropPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<DropPrompt>,
    },
    /// Showing template prompt for string template editing
    TemplatePrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<TemplatePrompt>,
    },
    /// Showing clipboard history
    ClipboardHistoryView {
        entries: Vec<clipboard_history::ClipboardEntry>,
        filter: String,
        selected_index: usize,
    },
    /// Showing app launcher
    AppLauncherView {
        apps: Vec<app_launcher::AppInfo>,
        filter: String,
        selected_index: usize,
    },
    /// Showing window switcher
    WindowSwitcherView {
        windows: Vec<window_control::WindowInfo>,
        filter: String,
        selected_index: usize,
    },
    /// Showing design gallery (separator and icon variations)
    DesignGalleryView {
        filter: String,
        selected_index: usize,
    },
}

/// Wrapper to hold a script session that can be shared across async boundaries
/// Uses parking_lot::Mutex which doesn't poison on panic, avoiding .unwrap() calls
type SharedSession = Arc<ParkingMutex<Option<executor::ScriptSession>>>;

/// Tracks which input field currently has focus for cursor display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedInput {
    /// Main script list filter input
    MainFilter,
    /// Actions dialog search input
    ActionsSearch,
    /// Arg prompt input (when running a script)
    ArgPrompt,
    /// No input focused (e.g., terminal prompt)
    None,
}

/// Messages sent from the prompt poller back to the main app
#[derive(Debug, Clone)]
enum PromptMessage {
    ShowArg {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        actions: Option<Vec<ProtocolAction>>,
    },
    ShowDiv {
        id: String,
        html: String,
        /// Tailwind classes for the content container
        container_classes: Option<String>,
        actions: Option<Vec<ProtocolAction>>,
        /// Placeholder text (header)
        placeholder: Option<String>,
        /// Hint text
        hint: Option<String>,
        /// Footer text
        footer: Option<String>,
        /// Container background color
        container_bg: Option<String>,
        /// Container padding (number or "none")
        container_padding: Option<serde_json::Value>,
        /// Container opacity (0-100)
        opacity: Option<u8>,
    },
    ShowForm {
        id: String,
        html: String,
        actions: Option<Vec<ProtocolAction>>,
    },
    ShowTerm {
        id: String,
        command: Option<String>,
        actions: Option<Vec<ProtocolAction>>,
    },
    ShowEditor {
        id: String,
        content: Option<String>,
        language: Option<String>,
        template: Option<String>,
        actions: Option<Vec<ProtocolAction>>,
    },
    /// Path picker prompt for file/folder selection
    ShowPath {
        id: String,
        start_path: Option<String>,
        hint: Option<String>,
    },
    /// Environment variable prompt with optional secret handling
    ShowEnv {
        id: String,
        key: String,
        prompt: Option<String>,
        secret: bool,
    },
    /// Drag and drop prompt for file uploads
    ShowDrop {
        id: String,
        placeholder: Option<String>,
        hint: Option<String>,
    },
    /// Template prompt for tab-through string templates
    ShowTemplate {
        id: String,
        template: String,
    },
    /// Multi-select prompt from choices
    ShowSelect {
        id: String,
        placeholder: Option<String>,
        choices: Vec<Choice>,
        multiple: bool,
    },
    HideWindow,
    OpenBrowser {
        url: String,
    },
    ScriptExit,
    /// External command to run a script by path
    RunScript {
        path: String,
    },
    /// Script error with detailed information for toast display
    ScriptError {
        error_message: String,
        stderr_output: Option<String>,
        exit_code: Option<i32>,
        stack_trace: Option<String>,
        script_path: String,
        suggestions: Vec<String>,
    },
    /// Protocol parsing error reported from script stdout
    ProtocolError {
        correlation_id: String,
        summary: String,
        details: Option<String>,
        severity: ErrorSeverity,
        script_path: String,
    },
    /// Unhandled message type from script - shows warning toast
    UnhandledMessage {
        message_type: String,
    },
    /// Request to get current UI state - triggers StateResult response
    GetState {
        request_id: String,
    },
    /// Force submit the current prompt with a value (from SDK's submit() function)
    ForceSubmit {
        value: serde_json::Value,
    },
    /// Set the current prompt input text
    SetInput {
        text: String,
    },
    /// Show HUD overlay message
    ShowHud {
        text: String,
        duration_ms: Option<u64>,
    },
    /// Set SDK actions for the ActionsDialog
    SetActions {
        actions: Vec<protocol::ProtocolAction>,
    },
}

struct ScriptListApp {
    scripts: Vec<scripts::Script>,
    scriptlets: Vec<scripts::Scriptlet>,
    builtin_entries: Vec<builtins::BuiltInEntry>,
    /// Cached list of installed applications for main search
    apps: Vec<app_launcher::AppInfo>,
    selected_index: usize,
    /// Main menu filter input with selection and clipboard support
    filter_input: TextInputState,
    last_output: Option<SharedString>,
    focus_handle: FocusHandle,
    show_logs: bool,
    theme: theme::Theme,
    #[allow(dead_code)]
    config: config::Config,
    // Scroll activity tracking for scrollbar fade
    /// Whether scroll activity is happening (scrollbar should be visible)
    is_scrolling: bool,
    /// Timestamp of last scroll activity (for fade-out timer)
    last_scroll_time: Option<std::time::Instant>,
    // Interactive script state
    current_view: AppView,
    script_session: SharedSession,
    // Prompt-specific state (used when view is ArgPrompt or DivPrompt)
    // Uses TextInputState for selection and clipboard support
    arg_input: TextInputState,
    arg_selected_index: usize,
    // Channel for receiving prompt messages from script thread (async_channel for event-driven)
    prompt_receiver: Option<async_channel::Receiver<PromptMessage>>,
    // Channel for sending responses back to script
    response_sender: Option<mpsc::Sender<Message>>,
    // List state for variable-height list (supports section headers at 24px + items at 48px)
    main_list_state: ListState,
    // Scroll handle for uniform_list (still used for backward compat in some views)
    list_scroll_handle: UniformListScrollHandle,
    // P0: Scroll handle for virtualized arg prompt choices
    arg_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for clipboard history list
    clipboard_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for window switcher list
    window_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for design gallery list
    design_gallery_scroll_handle: UniformListScrollHandle,
    // Actions popup overlay
    show_actions_popup: bool,
    // ActionsDialog entity for focus management
    actions_dialog: Option<Entity<ActionsDialog>>,
    // Cursor blink state and focus tracking
    cursor_visible: bool,
    /// Which input currently has focus (for cursor display)
    focused_input: FocusedInput,
    // Current script process PID for explicit cleanup (belt-and-suspenders)
    current_script_pid: Option<u32>,
    // P1: Cache for filtered_results() - invalidate on filter_text change only
    cached_filtered_results: Vec<scripts::SearchResult>,
    filter_cache_key: String,
    // P1: Cache for get_grouped_results() - invalidate on filter_text change only
    // This avoids recomputing grouped results 9+ times per keystroke
    // P1-Arc: Use Arc<[T]> for cheap clone in render closures
    cached_grouped_items: Arc<[GroupedListItem]>,
    cached_grouped_flat_results: Arc<[scripts::SearchResult]>,
    grouped_cache_key: String,
    // P3: Two-stage filter - display vs search separation with coalescing
    /// What the search cache is built from (may lag behind filter_text during rapid typing)
    computed_filter_text: String,
    /// Coalesces filter updates and keeps only the latest value per tick
    filter_coalescer: FilterCoalescer,
    // Scroll stabilization: track last scrolled-to index to avoid redundant scroll_to_item calls
    last_scrolled_index: Option<usize>,
    // Preview cache: avoid re-reading file and re-highlighting on every render
    preview_cache_path: Option<String>,
    preview_cache_lines: Vec<syntax::HighlightedLine>,
    // Current design variant for hot-swappable UI designs
    current_design: DesignVariant,
    // Toast manager for notification queue
    toast_manager: ToastManager,
    // Cache for decoded clipboard images (entry_id -> RenderImage)
    clipboard_image_cache: std::collections::HashMap<String, Arc<gpui::RenderImage>>,
    // Frecency store for tracking script usage
    frecency_store: FrecencyStore,
    // Mouse hover tracking - independent from selected_index (keyboard focus)
    // hovered_index shows subtle visual feedback, selected_index shows full focus styling
    hovered_index: Option<usize>,
    // P0-2: Debounce hover notify calls (16ms window to reduce 50% unnecessary re-renders)
    last_hover_notify: std::time::Instant,
    // Pending path action - when set, show ActionsDialog for this path
    // Uses Arc<Mutex<>> so callbacks can write to it
    pending_path_action: Arc<Mutex<Option<PathInfo>>>,
    // Signal to close path actions dialog (set by callback on Escape/__cancel__)
    close_path_actions: Arc<Mutex<bool>>,
    // Shared state: whether path actions dialog is currently showing
    // Used by PathPrompt to implement toggle behavior for Cmd+K
    path_actions_showing: Arc<Mutex<bool>>,
    // Shared state: current search text in path actions dialog
    // Used by PathPrompt to display search in header (like main menu does)
    path_actions_search_text: Arc<Mutex<String>>,
    // Pending path action result - when set, execute this action on the stored path
    // Tuple of (action_id, path_info) to handle the action
    pending_path_action_result: Arc<Mutex<Option<(String, PathInfo)>>>,
    /// Alias registry: lowercase_alias -> script_path (for O(1) lookup)
    /// Conflict rule: first-registered wins
    alias_registry: std::collections::HashMap<String, String>,
    /// Shortcut registry: shortcut -> script_path (for O(1) lookup)
    /// Conflict rule: first-registered wins
    shortcut_registry: std::collections::HashMap<String, String>,
    /// SDK actions set via setActions() - stored for trigger_action_by_name lookup
    sdk_actions: Option<Vec<protocol::ProtocolAction>>,
    /// SDK action shortcuts: normalized_shortcut -> action_name (for O(1) lookup)
    action_shortcuts: std::collections::HashMap<String, String>,
    // Navigation coalescing for rapid arrow key events (20ms window)
    nav_coalescer: NavCoalescer,
    // Window focus tracking - for detecting focus lost and auto-dismissing prompts
    // When window loses focus while in a dismissable prompt, close and reset
    was_window_focused: bool,
    // Scroll stabilization: track last scrolled-to index for each scroll handle
    #[allow(dead_code)]
    last_scrolled_main: Option<usize>,
    #[allow(dead_code)]
    last_scrolled_arg: Option<usize>,
    #[allow(dead_code)]
    last_scrolled_clipboard: Option<usize>,
    #[allow(dead_code)]
    last_scrolled_window: Option<usize>,
    #[allow(dead_code)]
    last_scrolled_design_gallery: Option<usize>,
}

/// Result of alias matching - either a Script or Scriptlet
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
enum AliasMatch {
    Script(scripts::Script),
    Scriptlet(scripts::Scriptlet),
}

// Core ScriptListApp implementation extracted to app_impl.rs
include!("app_impl.rs");

// Script execution logic (execute_interactive) extracted
include!("execute_script.rs");

// Prompt message handling (handle_prompt_message) extracted
include!("prompt_handler.rs");

// App navigation methods (selection movement, scrolling)
include!("app_navigation.rs");

// App execution methods (execute_builtin, execute_app, execute_window_focus)
include!("app_execute.rs");

// App actions handling (handle_action, trigger_action_by_name)
include!("app_actions.rs");

impl Focusable for ScriptListApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ScriptListApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Flush any pending toasts to gpui-component's NotificationList
        // This is needed because toast push sites don't have window access
        self.flush_pending_toasts(window, cx);

        // P0-4: Focus handling using reference match (avoids clone for focus check)
        // Focus handling depends on the view:
        // - For EditorPrompt: Use its own focus handle (not the parent's)
        // - For other views: Use the parent's focus handle
        match &self.current_view {
            AppView::EditorPrompt { focus_handle, .. } => {
                // EditorPrompt has its own focus handle - focus it
                let is_focused = focus_handle.is_focused(window);
                if !is_focused {
                    // Clone focus handle to satisfy borrow checker
                    let fh = focus_handle.clone();
                    window.focus(&fh, cx);
                }
            }
            AppView::PathPrompt { focus_handle, .. } => {
                // PathPrompt has its own focus handle - focus it
                // But if actions dialog is showing, focus the dialog instead
                if self.show_actions_popup {
                    if let Some(ref dialog) = self.actions_dialog {
                        let dialog_focus_handle = dialog.read(cx).focus_handle.clone();
                        let is_focused = dialog_focus_handle.is_focused(window);
                        if !is_focused {
                            window.focus(&dialog_focus_handle, cx);
                        }
                    }
                } else {
                    let is_focused = focus_handle.is_focused(window);
                    if !is_focused {
                        let fh = focus_handle.clone();
                        window.focus(&fh, cx);
                    }
                }
            }
            AppView::FormPrompt { entity, .. } => {
                // FormPrompt uses delegated Focusable - get focus handle from the currently focused field
                // This prevents the parent from stealing focus from form text fields
                let form_focus_handle = entity.read(cx).focus_handle(cx);
                let is_focused = form_focus_handle.is_focused(window);
                if !is_focused {
                    window.focus(&form_focus_handle, cx);
                }
            }
            _ => {
                // Other views use the parent's focus handle
                let is_focused = self.focus_handle.is_focused(window);
                if !is_focused {
                    window.focus(&self.focus_handle, cx);
                }
            }
        }

        // Focus-lost auto-dismiss: Close dismissable prompts when user clicks another app
        // This makes the app feel more native - clicking away from a prompt dismisses it
        let is_app_active = platform::is_app_active();
        if self.was_window_focused && !is_app_active {
            // Window just lost focus (user clicked on another app)
            // Only auto-dismiss if we're in a dismissable view AND window is visible
            if self.is_dismissable_view() && script_kit_gpui::is_main_window_visible() {
                logging::log(
                    "FOCUS",
                    "Window lost focus while in dismissable view - closing",
                );
                self.close_and_reset_window(cx);
            }
        }
        self.was_window_focused = is_app_active;

        // NOTE: Prompt messages are now handled via event-driven async_channel listener
        // spawned in execute_interactive() - no polling needed in render()

        // P0-4: Clone current_view only for dispatch (needed to call &mut self methods)
        // The clone is unavoidable due to borrow checker: we need &mut self for render methods
        // but also need to match on self.current_view. Future optimization: refactor render
        // methods to take &str/&[T] references instead of owned values.
        //
        // HUD is now handled by hud_manager as a separate floating window
        // No need to render it as part of this view
        let current_view = self.current_view.clone();
        match current_view {
            AppView::ScriptList => self.render_script_list(cx),
            AppView::ActionsDialog => self.render_actions_dialog(cx),
            AppView::ArgPrompt {
                id,
                placeholder,
                choices,
                actions,
            } => self.render_arg_prompt(id, placeholder, choices, actions, cx),
            AppView::DivPrompt { entity, .. } => self.render_div_prompt(entity, cx),
            AppView::FormPrompt { entity, .. } => self.render_form_prompt(entity, cx),
            AppView::TermPrompt { entity, .. } => self.render_term_prompt(entity, cx),
            AppView::EditorPrompt { entity, .. } => self.render_editor_prompt(entity, cx),
            AppView::SelectPrompt { entity, .. } => self.render_select_prompt(entity, cx),
            AppView::PathPrompt { entity, .. } => self.render_path_prompt(entity, cx),
            AppView::EnvPrompt { entity, .. } => self.render_env_prompt(entity, cx),
            AppView::DropPrompt { entity, .. } => self.render_drop_prompt(entity, cx),
            AppView::TemplatePrompt { entity, .. } => self.render_template_prompt(entity, cx),
            AppView::ClipboardHistoryView {
                entries,
                filter,
                selected_index,
            } => self.render_clipboard_history(entries, filter, selected_index, cx),
            AppView::AppLauncherView {
                apps,
                filter,
                selected_index,
            } => self.render_app_launcher(apps, filter, selected_index, cx),
            AppView::WindowSwitcherView {
                windows,
                filter,
                selected_index,
            } => self.render_window_switcher(windows, filter, selected_index, cx),
            AppView::DesignGalleryView {
                filter,
                selected_index,
            } => self.render_design_gallery(filter, selected_index, cx),
        }
    }
}

// Render methods extracted to app_render.rs for maintainability
include!("app_render.rs");

// Builtin view render methods (clipboard, app launcher, window switcher)
include!("render_builtins.rs");

// Prompt render methods (arg, div, form, term, editor, etc.)
include!("render_prompts.rs");

// Script list render method
include!("render_script_list.rs");

fn main() {
    logging::init();

    // Ensure ~/.kenv environment is properly set up (directories, SDK, config, etc.)
    // This is idempotent - it creates missing directories and files without overwriting user configs
    let setup_result = setup::ensure_kenv_setup();
    if setup_result.is_fresh_install {
        logging::log(
            "APP",
            &format!(
                "Fresh install detected - created ~/.kenv at {}",
                setup_result.kenv_path.display()
            ),
        );
    }
    for warning in &setup_result.warnings {
        logging::log("APP", &format!("Setup warning: {}", warning));
    }
    if !setup_result.bun_available {
        logging::log(
            "APP",
            "Warning: bun not found in PATH or common locations. Scripts may not run.",
        );
    }

    // Write main PID file for orphan detection on crash
    if let Err(e) = PROCESS_MANAGER.write_main_pid() {
        logging::log("APP", &format!("Failed to write main PID file: {}", e));
    } else {
        logging::log("APP", "Main PID file written");
    }

    // Clean up any orphaned processes from a previous crash
    let orphans_killed = PROCESS_MANAGER.cleanup_orphans();
    if orphans_killed > 0 {
        logging::log(
            "APP",
            &format!(
                "Cleaned up {} orphaned process(es) from previous session",
                orphans_killed
            ),
        );
    }

    // Register signal handlers for graceful shutdown
    // Using libc directly since ctrlc crate is not available
    #[cfg(unix)]
    {
        extern "C" fn handle_signal(sig: libc::c_int) {
            logging::log("SIGNAL", &format!("Received signal {}", sig));

            // Set shutdown flag to prevent new script spawns
            SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);

            // Kill all tracked child processes
            logging::log("SIGNAL", "Killing all child processes");
            PROCESS_MANAGER.kill_all_processes();

            // Remove main PID file
            PROCESS_MANAGER.remove_main_pid();

            // For SIGINT/SIGTERM, exit gracefully
            // Note: We can't call cx.quit() from here since we're in a signal handler
            // The process will terminate after killing children
            logging::log("SIGNAL", "Exiting after signal cleanup");
            std::process::exit(0);
        }

        unsafe {
            // Register handlers for common termination signals
            libc::signal(libc::SIGINT, handle_signal as libc::sighandler_t);
            libc::signal(libc::SIGTERM, handle_signal as libc::sighandler_t);
            libc::signal(libc::SIGHUP, handle_signal as libc::sighandler_t);
            logging::log(
                "APP",
                "Signal handlers registered (SIGINT, SIGTERM, SIGHUP)",
            );
        }
    }

    // Load config early so we can use it for hotkey registration AND clipboard history settings
    // This avoids duplicate config::load_config() calls (~100-300ms startup savings)
    let loaded_config = config::load_config();
    logging::log(
        "APP",
        &format!(
            "Loaded config: hotkey={:?}+{}, bun_path={:?}",
            loaded_config.hotkey.modifiers, loaded_config.hotkey.key, loaded_config.bun_path
        ),
    );
    clipboard_history::set_max_text_content_len(
        loaded_config.get_clipboard_history_max_text_length(),
    );

    // Initialize clipboard history monitoring (background thread)
    if let Err(e) = clipboard_history::init_clipboard_history() {
        logging::log(
            "APP",
            &format!("Failed to initialize clipboard history: {}", e),
        );
    } else {
        logging::log("APP", "Clipboard history monitoring initialized");
    }

    // Initialize text expansion system (background thread with keyboard monitoring)
    // This must be done early, before the GPUI run loop starts
    #[cfg(target_os = "macos")]
    {
        use expand_manager::ExpandManager;

        // Spawn initialization in a thread to not block startup
        std::thread::spawn(move || {
            logging::log("EXPAND", "Initializing text expansion system");

            // Check accessibility permissions first
            if !ExpandManager::has_accessibility_permission() {
                logging::log(
                    "EXPAND",
                    "Accessibility permissions not granted - text expansion disabled",
                );
                logging::log(
                    "EXPAND",
                    "Enable in System Preferences > Privacy & Security > Accessibility",
                );
                return;
            }

            let mut manager = ExpandManager::new();

            // Load scriptlets with expand triggers
            match manager.load_scriptlets() {
                Ok(count) => {
                    if count == 0 {
                        logging::log("EXPAND", "No expand triggers found in scriptlets");
                        return;
                    }
                    logging::log("EXPAND", &format!("Loaded {} expand triggers", count));
                }
                Err(e) => {
                    logging::log("EXPAND", &format!("Failed to load scriptlets: {}", e));
                    return;
                }
            }

            // Enable keyboard monitoring
            match manager.enable() {
                Ok(()) => {
                    logging::log("EXPAND", "Text expansion system enabled");

                    // List registered triggers
                    for (trigger, name) in manager.list_triggers() {
                        logging::log("EXPAND", &format!("  Trigger '{}' -> {}", trigger, name));
                    }

                    // Keep the manager alive - it will run until the process exits
                    // The keyboard monitor thread is managed by the KeyboardMonitor
                    std::mem::forget(manager);
                }
                Err(e) => {
                    logging::log(
                        "EXPAND",
                        &format!("Failed to enable text expansion: {:?}", e),
                    );
                }
            }
        });
    }

    // Clone before start_hotkey_listener consumes original
    let config_for_app = loaded_config.clone();

    // Start MCP server for AI agent integration
    // Server runs on localhost:43210 with Bearer token authentication
    // Discovery file written to ~/.kenv/server.json
    let _mcp_handle = match mcp_server::McpServer::with_defaults() {
        Ok(server) => match server.start() {
            Ok(handle) => {
                logging::log(
                    "MCP",
                    &format!(
                        "MCP server started on {} (token in ~/.kenv/agent-token)",
                        server.url()
                    ),
                );
                Some(handle)
            }
            Err(e) => {
                logging::log("MCP", &format!("Failed to start MCP server: {}", e));
                None
            }
        },
        Err(e) => {
            logging::log("MCP", &format!("Failed to create MCP server: {}", e));
            None
        }
    };

    hotkeys::start_hotkey_listener(loaded_config);

    let (mut appearance_watcher, appearance_rx) = watcher::AppearanceWatcher::new();
    if let Err(e) = appearance_watcher.start() {
        logging::log("APP", &format!("Failed to start appearance watcher: {}", e));
    }

    let (mut config_watcher, config_rx) = watcher::ConfigWatcher::new();
    if let Err(e) = config_watcher.start() {
        logging::log("APP", &format!("Failed to start config watcher: {}", e));
    }

    let (mut script_watcher, script_rx) = watcher::ScriptWatcher::new();
    if let Err(e) = script_watcher.start() {
        logging::log("APP", &format!("Failed to start script watcher: {}", e));
    }

    // Initialize script scheduler
    // Creates the scheduler and scans for scripts with // Cron: or // Schedule: metadata
    let (mut scheduler, scheduler_rx) = scheduler::Scheduler::new();
    let scheduled_count = scripts::register_scheduled_scripts(&scheduler);
    logging::log(
        "APP",
        &format!("Registered {} scheduled scripts", scheduled_count),
    );

    // Start the scheduler background thread (checks every 30 seconds for due scripts)
    if scheduled_count > 0 {
        if let Err(e) = scheduler.start() {
            logging::log("APP", &format!("Failed to start scheduler: {}", e));
        } else {
            logging::log("APP", "Scheduler started successfully");
        }
    } else {
        logging::log("APP", "No scheduled scripts found, scheduler not started");
    }

    // Wrap scheduler in Arc<Mutex<>> for thread-safe access (needed for re-scanning on file changes)
    let scheduler = Arc::new(Mutex::new(scheduler));

    Application::new().run(move |cx: &mut App| {
        logging::log("APP", "GPUI Application starting");

        // Initialize gpui-component (theme, context providers)
        // Must be called before opening windows that use Root wrapper
        gpui_component::init(cx);

        // Sync Script Kit theme with gpui-component's ThemeColor system
        // This ensures all gpui-component widgets use our colors
        theme::sync_gpui_component_theme(cx);

        // Initialize tray icon and menu
        // MUST be done after Application::new() creates the NSApplication
        let tray_manager = match TrayManager::new() {
            Ok(tm) => {
                logging::log("TRAY", "Tray icon initialized successfully");
                Some(tm)
            }
            Err(e) => {
                logging::log("TRAY", &format!("Failed to initialize tray icon: {}", e));
                None
            }
        };

        // Calculate window bounds: centered on display with mouse, at eye-line height
        let window_size = size(px(750.), initial_window_height());
        let bounds = calculate_eye_line_bounds_on_mouse_display(window_size);

        // Store the ScriptListApp entity for direct access (needed since Root wraps the view)
        let app_entity_holder: Arc<Mutex<Option<Entity<ScriptListApp>>>> = Arc::new(Mutex::new(None));
        let app_entity_for_closure = app_entity_holder.clone();

        let window: WindowHandle<Root> = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                is_movable: true,
                window_background: WindowBackgroundAppearance::Blurred,
                show: false, // Start hidden - only show on hotkey press
                focus: false, // Don't focus on creation
                ..Default::default()
            },
            |window, cx| {
                logging::log("APP", "Window opened, creating ScriptListApp wrapped in Root");
                let view = cx.new(|cx| ScriptListApp::new(config_for_app, cx));
                // Store the entity for external access
                *app_entity_for_closure.lock().unwrap() = Some(view.clone());
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();

        // Extract the app entity for use in callbacks
        let app_entity = app_entity_holder.lock().unwrap().clone().expect("App entity should be set");

        // Set initial focus via the Root window
        // We access the app entity within the window context to properly focus it
        let app_entity_for_focus = app_entity.clone();
        window
            .update(cx, |_root, win, root_cx| {
                app_entity_for_focus.update(root_cx, |view, ctx| {
                    let focus_handle = view.focus_handle(ctx);
                    win.focus(&focus_handle, ctx);
                    logging::log("APP", "Focus set on ScriptListApp via Root");
                });
            })
            .unwrap();

        // Register the main window with WindowManager before tray init
        // This must happen after GPUI creates the window but before tray creates its windows
        // so we can reliably find our main window by its expected size (~750x500)
        window_manager::find_and_register_main_window();

        // Window starts hidden - no activation, no panel configuration yet
        // Panel will be configured on first show via hotkey
        // WINDOW_VISIBLE is already false by default (static initializer)
        logging::log("HOTKEY", "Window created but not shown (use hotkey to show)");

        // Main window hotkey listener - uses Entity<ScriptListApp> instead of WindowHandle
        let app_entity_for_hotkey = app_entity.clone();
        let window_for_hotkey = window;
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "Main hotkey listener started");
            while let Ok(()) = hotkeys::hotkey_channel().1.recv().await {
                logging::log("VISIBILITY", "");
                logging::log("VISIBILITY", "╔════════════════════════════════════════════════════════════╗");
                logging::log("VISIBILITY", "║  HOTKEY TRIGGERED - TOGGLE WINDOW                          ║");
                logging::log("VISIBILITY", "╚════════════════════════════════════════════════════════════╝");

                let is_visible = script_kit_gpui::is_main_window_visible();
                logging::log("VISIBILITY", &format!("State: WINDOW_VISIBLE={}", is_visible));

                let app_entity_inner = app_entity_for_hotkey.clone();
                let window_inner = window_for_hotkey;

                if is_visible {
                    logging::log("VISIBILITY", "Decision: HIDE");
                    script_kit_gpui::set_main_window_visible(false);

                    // Check if Notes or AI windows are open BEFORE the closure
                    let notes_open = notes::is_notes_window_open();
                    let ai_open = ai::is_ai_window_open();
                    logging::log(
                        "VISIBILITY",
                        &format!(
                            "Secondary windows: notes_open={}, ai_open={}",
                            notes_open, ai_open
                        ),
                    );

                    let _ = cx.update(move |cx: &mut gpui::App| {
                        // Cancel any active prompt and reset UI
                        app_entity_inner.update(cx, |view, ctx| {
                            if view.is_in_prompt() {
                                logging::log("HOTKEY", "Canceling prompt before hiding");
                                view.cancel_script_execution(ctx);
                            }
                            view.reset_to_script_list(ctx);
                        });

                        // CRITICAL: Only hide main window if Notes/AI are open
                        // cx.hide() hides the ENTIRE app (all windows), so we use
                        // platform::hide_main_window() to hide only the main window
                        if notes_open || ai_open {
                            logging::log(
                                "HOTKEY",
                                "Using hide_main_window() - secondary windows are open",
                            );
                            platform::hide_main_window();
                        } else {
                            logging::log("HOTKEY", "Using cx.hide() - no secondary windows");
                            cx.hide();
                        }
                        logging::log("HOTKEY", "Main window hidden");
                    });
                } else {
                    logging::log("VISIBILITY", "Decision: SHOW");
                    script_kit_gpui::set_main_window_visible(true);

                    let _ = cx.update(move |cx: &mut gpui::App| {
                        // Position window on mouse display at eye-line
                        platform::ensure_move_to_active_space();

                        let window_size = gpui::size(px(750.), initial_window_height());
                        let bounds = platform::calculate_eye_line_bounds_on_mouse_display(window_size);
                        platform::move_first_window_to_bounds(&bounds);

                        // Configure as floating panel on first show
                        if !PANEL_CONFIGURED.load(std::sync::atomic::Ordering::SeqCst) {
                            platform::configure_as_floating_panel();
                            PANEL_CONFIGURED.store(true, std::sync::atomic::Ordering::SeqCst);
                        }

                        // Activate window
                        cx.activate(true);
                        let _ = window_inner.update(cx, |_root, window, _cx| {
                            window.activate_window();
                        });

                        logging::log("HOTKEY", "Window shown and activated");
                    });
                }
            }
            logging::log("HOTKEY", "Main hotkey listener exiting");
        }).detach();

        // Notes hotkey listener - event-driven via async_channel
        // The hotkey thread dispatches via GPUI's ForegroundExecutor, which wakes this task
        // This works even before main window activates because the executor is initialized first
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "Notes hotkey listener started (event-driven)");
            // Event-driven: .recv().await blocks until a message arrives
            // This is more efficient than polling and responds immediately
            while let Ok(()) = hotkeys::notes_hotkey_channel().1.recv().await {
                logging::log("HOTKEY", "Notes hotkey triggered - opening notes window");
                let _ = cx.update(|cx: &mut gpui::App| {
                    if let Err(e) = notes::open_notes_window(cx) {
                        logging::log("HOTKEY", &format!("Failed to open notes window: {}", e));
                    }
                });
            }
            logging::log("HOTKEY", "Notes hotkey listener exiting (channel closed)");
        }).detach();

        // AI hotkey listener - event-driven via async_channel
        // Same pattern as Notes hotkey - works immediately on app launch
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "AI hotkey listener started (event-driven)");
            // Event-driven: .recv().await blocks until a message arrives
            while let Ok(()) = hotkeys::ai_hotkey_channel().1.recv().await {
                logging::log("HOTKEY", "AI hotkey triggered - opening AI window");
                let _ = cx.update(|cx: &mut gpui::App| {
                    if let Err(e) = ai::open_ai_window(cx) {
                        logging::log("HOTKEY", &format!("Failed to open AI window: {}", e));
                    }
                });
            }
            logging::log("HOTKEY", "AI hotkey listener exiting (channel closed)");
        }).detach();

        // Appearance change watcher - event-driven with async_channel
        let app_entity_for_appearance = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            // Event-driven: blocks until appearance change event received
            while let Ok(_event) = appearance_rx.recv().await {
                logging::log("APP", "System appearance changed, updating theme");
                let _ = cx.update(|cx| {
                    // Sync gpui-component theme with new system appearance
                    theme::sync_gpui_component_theme(cx);

                    app_entity_for_appearance.update(cx, |view, ctx| {
                        view.update_theme(ctx);
                    });
                });
            }
            logging::log("APP", "Appearance watcher channel closed");
        }).detach();

        // Config reload watcher - watches ~/.kenv/config.ts for changes
        let app_entity_for_config = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            loop {
                Timer::after(std::time::Duration::from_millis(200)).await;

                if config_rx.try_recv().is_ok() {
                    logging::log("APP", "Config file changed, reloading");
                    let _ = cx.update(|cx| {
                        app_entity_for_config.update(cx, |view, ctx| {
                            view.update_config(ctx);
                        });
                    });
                }
            }
        }).detach();

        // Script/scriptlets reload watcher - watches ~/.kenv/scripts/ and ~/.kenv/scriptlets/
        // Also re-scans for scheduled scripts to pick up new/modified schedules
        let app_entity_for_scripts = app_entity.clone();
        let scheduler_for_scripts = scheduler.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            loop {
                Timer::after(std::time::Duration::from_millis(200)).await;

                if script_rx.try_recv().is_ok() {
                    logging::log("APP", "Scripts or scriptlets changed, reloading");

                    // Re-scan for scheduled scripts when files change
                    if let Ok(scheduler_guard) = scheduler_for_scripts.lock() {
                        let new_count = scripts::register_scheduled_scripts(&scheduler_guard);
                        if new_count > 0 {
                            logging::log("APP", &format!("Re-registered {} scheduled scripts after file change", new_count));
                        }
                    }

                    let _ = cx.update(|cx| {
                        app_entity_for_scripts.update(cx, |view, ctx| {
                            view.refresh_scripts(ctx);
                        });
                    });
                }
            }
        }).detach();

        // NOTE: Prompt message listener is now spawned per-script in execute_interactive()
        // using event-driven async_channel instead of 50ms polling

        // Scheduler event handler - runs scripts when their cron schedule triggers
        // Uses std::sync::mpsc::Receiver which requires a polling approach
        let _window_for_scheduler = window;
        std::thread::spawn(move || {
            logging::log("APP", "Scheduler event handler started");

            loop {
                // Use recv_timeout to periodically check for events without blocking forever
                match scheduler_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                    Ok(event) => {
                        match event {
                            scheduler::SchedulerEvent::RunScript(path) => {
                                logging::log("SCHEDULER", &format!("Executing scheduled script: {}", path.display()));

                                // Execute the script using the existing executor infrastructure
                                // This spawns it in the background without blocking the scheduler
                                let path_str = path.to_string_lossy().to_string();

                                // Use bun to run the script directly (non-interactive for scheduled scripts)
                                // Find bun path (same logic as executor)
                                let bun_path = std::env::var("BUN_PATH")
                                    .ok()
                                    .or_else(|| {
                                        // Check common locations
                                        for candidate in &[
                                            "/opt/homebrew/bin/bun",
                                            "/usr/local/bin/bun",
                                            std::env::var("HOME").ok().map(|h| format!("{}/.bun/bin/bun", h)).unwrap_or_default().as_str(),
                                        ] {
                                            if std::path::Path::new(candidate).exists() {
                                                return Some(candidate.to_string());
                                            }
                                        }
                                        None
                                    })
                                    .unwrap_or_else(|| "bun".to_string());

                                // Spawn bun process to run the script
                                match std::process::Command::new(&bun_path)
                                    .arg("run")
                                    .arg("--preload")
                                    .arg(format!("{}/.kenv/sdk/kit-sdk.ts", std::env::var("HOME").unwrap_or_default()))
                                    .arg(&path_str)
                                    .stdout(std::process::Stdio::piped())
                                    .stderr(std::process::Stdio::piped())
                                    .spawn()
                                {
                                    Ok(child) => {
                                        let pid = child.id();
                                        // Track the process
                                        PROCESS_MANAGER.register_process(pid, &path_str);
                                        logging::log("SCHEDULER", &format!("Spawned scheduled script PID {}: {}", pid, path_str));

                                        // Wait for completion in a separate thread to not block scheduler
                                        let path_for_log = path_str.clone();
                                        std::thread::spawn(move || {
                                            match child.wait_with_output() {
                                                Ok(output) => {
                                                    // Unregister the process now that it's done
                                                    PROCESS_MANAGER.unregister_process(pid);

                                                    if output.status.success() {
                                                        logging::log("SCHEDULER", &format!("Scheduled script completed: {}", path_for_log));
                                                    } else {
                                                        let stderr = String::from_utf8_lossy(&output.stderr);
                                                        logging::log("SCHEDULER", &format!("Scheduled script failed: {} - {}", path_for_log, stderr));
                                                    }
                                                }
                                                Err(e) => {
                                                    // Unregister on error too
                                                    PROCESS_MANAGER.unregister_process(pid);
                                                    logging::log("SCHEDULER", &format!("Scheduled script error: {} - {}", path_for_log, e));
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        logging::log("SCHEDULER", &format!("Failed to spawn scheduled script: {} - {}", path_str, e));
                                    }
                                }
                            }
                            scheduler::SchedulerEvent::Error(msg) => {
                                logging::log("SCHEDULER", &format!("Scheduler error: {}", msg));
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Normal timeout, continue loop
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        logging::log("APP", "Scheduler event channel disconnected, exiting handler");
                        break;
                    }
                }
            }
        });

        // Test command file watcher - allows smoke tests to trigger script execution
        let app_entity_for_test = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            let cmd_file = std::path::PathBuf::from("/tmp/script-kit-gpui-cmd.txt");
            loop {
                Timer::after(std::time::Duration::from_millis(500)).await;

                if cmd_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(&cmd_file) {
                        let _ = std::fs::remove_file(&cmd_file); // Remove immediately to prevent re-processing

                        for line in content.lines() {
                            if line.starts_with("run:") {
                                let script_name = line.strip_prefix("run:").unwrap_or("").trim();
                                logging::log("TEST", &format!("Test command: run script '{}'", script_name));

                                let script_name_owned = script_name.to_string();
                                let app_entity_inner = app_entity_for_test.clone();
                                let _ = cx.update(|cx| {
                                    app_entity_inner.update(cx, |view, ctx| {
                                        // Find and run the script interactively
                                        if let Some(script) = view.scripts.iter().find(|s| s.name == script_name_owned || s.path.to_string_lossy().contains(&script_name_owned)).cloned() {
                                            logging::log("TEST", &format!("Found script: {}", script.name));
                                            view.execute_interactive(&script, ctx);
                                        } else {
                                            logging::log("TEST", &format!("Script not found: {}", script_name_owned));
                                        }
                                    });
                                });
                            }
                        }
                    }
                }
            }
        }).detach();

        // External command listener - receives commands via stdin (event-driven, no polling)
        let stdin_rx = start_stdin_listener();
        let window_for_stdin = window;
        let app_entity_for_stdin = app_entity.clone();

        // Track if we've received any stdin commands (for timeout warning)
        static STDIN_RECEIVED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

        // Spawn a timeout warning task - helps AI agents detect when they forgot to use stdin protocol
        cx.spawn(async move |_cx: &mut gpui::AsyncApp| {
            Timer::after(std::time::Duration::from_secs(2)).await;
            if !STDIN_RECEIVED.load(std::sync::atomic::Ordering::SeqCst) {
                logging::log("STDIN", "");
                logging::log("STDIN", "╔════════════════════════════════════════════════════════════════════════════╗");
                logging::log("STDIN", "║  WARNING: No stdin JSON received after 2 seconds                          ║");
                logging::log("STDIN", "║                                                                            ║");
                logging::log("STDIN", "║  If you're testing, use the stdin JSON protocol:                          ║");
                logging::log("STDIN", "║  echo '{\"type\":\"run\",\"path\":\"...\"}' | ./target/debug/script-kit-gpui     ║");
                logging::log("STDIN", "║                                                                            ║");
                logging::log("STDIN", "║  Command line args do NOT work:                                           ║");
                logging::log("STDIN", "║  ./target/debug/script-kit-gpui test.ts  # WRONG - does nothing!          ║");
                logging::log("STDIN", "╚════════════════════════════════════════════════════════════════════════════╝");
                logging::log("STDIN", "");
            }
        }).detach();

        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("STDIN", "Async stdin command handler started");

            // Event-driven: recv().await yields until a command arrives
            while let Ok(cmd) = stdin_rx.recv().await {
                // Mark that we've received stdin (clears the timeout warning)
                STDIN_RECEIVED.store(true, std::sync::atomic::Ordering::SeqCst);
                logging::log("STDIN", &format!("Processing external command: {:?}", cmd));

                let app_entity_inner = app_entity_for_stdin.clone();
                let _ = cx.update(|cx| {
                    // Use the Root window to get Window reference, then update the app entity
                    let _ = window_for_stdin.update(cx, |_root, window, root_cx| {
                        app_entity_inner.update(root_cx, |view, ctx| {
                            // Note: We have both `window` from Root and `view` from entity here
                            // ctx is Context<ScriptListApp>, window is &mut Window
                        match cmd {
                            ExternalCommand::Run { ref path } => {
                                logging::log("STDIN", &format!("Executing script: {}", path));
                                // Show and focus window first
                                script_kit_gpui::set_main_window_visible(true);
                                ctx.activate(true);
                                window.activate_window();
                                let focus_handle = view.focus_handle(ctx);
                                window.focus(&focus_handle, ctx);

                                // Send RunScript message to be handled
                                view.handle_prompt_message(PromptMessage::RunScript { path: path.clone() }, ctx);
                            }
                            ExternalCommand::Show => {
                                logging::log("STDIN", "Showing window");
                                script_kit_gpui::set_main_window_visible(true);
                                ctx.activate(true);
                                window.activate_window();
                                let focus_handle = view.focus_handle(ctx);
                                window.focus(&focus_handle, ctx);
                            }
                            ExternalCommand::Hide => {
                                logging::log("STDIN", "Hiding main window");
                                script_kit_gpui::set_main_window_visible(false);
                                ctx.hide();
                            }
                            ExternalCommand::SetFilter { ref text } => {
                                logging::log("STDIN", &format!("Setting filter to: '{}'", text));
                                view.filter_input.set_text(text.clone());
                                view.computed_filter_text = text.clone();
                                view.filter_coalescer.reset();
                                let _ = view.get_filtered_results_cached(); // Update cache
                                view.selected_index = 0;
                                view.update_window_size();
                            }
                            ExternalCommand::TriggerBuiltin { ref name } => {
                                logging::log("STDIN", &format!("Triggering built-in: '{}'", name));
                                // Match built-in name and trigger the corresponding feature
                                match name.to_lowercase().as_str() {
                                    "design-gallery" | "designgallery" | "design gallery" => {
                                        view.current_view = AppView::DesignGalleryView {
                                            filter: String::new(),
                                            selected_index: 0,
                                        };
                                        view.update_window_size();
                                    }
                                    "clipboard" | "clipboard-history" | "clipboardhistory" => {
                                        let entries = clipboard_history::get_cached_entries(100);
                                        view.current_view = AppView::ClipboardHistoryView {
                                            entries,
                                            filter: String::new(),
                                            selected_index: 0,
                                        };
                                        view.update_window_size();
                                    }
                                    "apps" | "app-launcher" | "applauncher" => {
                                        let apps = view.apps.clone();
                                        view.current_view = AppView::AppLauncherView {
                                            apps,
                                            filter: String::new(),
                                            selected_index: 0,
                                        };
                                        view.update_window_size();
                                    }
                                    _ => {
                                        logging::log("ERROR", &format!("Unknown built-in: '{}'", name));
                                    }
                                }
                            }
                            ExternalCommand::SimulateKey { ref key, ref modifiers } => {
                                logging::log("STDIN", &format!("Simulating key: '{}' with modifiers: {:?}", key, modifiers));

                                // Parse modifiers
                                let has_cmd = modifiers.iter().any(|m| m == "cmd" || m == "meta" || m == "command");
                                let _has_shift = modifiers.iter().any(|m| m == "shift");
                                let _has_alt = modifiers.iter().any(|m| m == "alt" || m == "option");
                                let _has_ctrl = modifiers.iter().any(|m| m == "ctrl" || m == "control");

                                // Handle key based on current view
                                let key_lower = key.to_lowercase();

                                match &view.current_view {
                                    AppView::ScriptList => {
                                        // Main script list key handling
                                        if has_cmd && key_lower == "k" {
                                            logging::log("STDIN", "SimulateKey: Cmd+K - toggle actions");
                                            view.toggle_actions(ctx, window);
                                        } else {
                                            match key_lower.as_str() {
                                                "up" | "arrowup" => {
                                                    // Use move_selection_up to properly skip section headers
                                                    view.move_selection_up(ctx);
                                                }
                                                "down" | "arrowdown" => {
                                                    // Use move_selection_down to properly skip section headers
                                                    view.move_selection_down(ctx);
                                                }
                                                "enter" => {
                                                    logging::log("STDIN", "SimulateKey: Enter - execute selected");
                                                    view.execute_selected(ctx);
                                                }
                                                "escape" => {
                                                    logging::log("STDIN", "SimulateKey: Escape - clear filter or hide");
                                                    if !view.filter_input.is_empty() {
                                                        view.update_filter(None, false, true, ctx);
                                                    } else {
                                                        script_kit_gpui::set_main_window_visible(false);
                                                        ctx.hide();
                                                    }
                                                }
                                                _ => {
                                                    logging::log("STDIN", &format!("SimulateKey: Unhandled key '{}' in ScriptList", key_lower));
                                                }
                                            }
                                        }
                                    }
                                    AppView::PathPrompt { entity, .. } => {
                                        // Path prompt key handling
                                        logging::log("STDIN", &format!("SimulateKey: Dispatching '{}' to PathPrompt", key_lower));
                                        let entity_clone = entity.clone();
                                        entity_clone.update(ctx, |path_prompt: &mut PathPrompt, path_cx| {
                                            if has_cmd && key_lower == "k" {
                                                path_prompt.toggle_actions(path_cx);
                                            } else {
                                                match key_lower.as_str() {
                                                    "up" | "arrowup" => path_prompt.move_up(path_cx),
                                                    "down" | "arrowdown" => path_prompt.move_down(path_cx),
                                                    "enter" => path_prompt.handle_enter(path_cx),
                                                    "escape" => path_prompt.submit_cancel(),
                                                    "left" | "arrowleft" => path_prompt.navigate_to_parent(path_cx),
                                                    "right" | "arrowright" => path_prompt.navigate_into_selected(path_cx),
                                                    _ => {
                                                        logging::log("STDIN", &format!("SimulateKey: Unhandled key '{}' in PathPrompt", key_lower));
                                                    }
                                                }
                                            }
                                        });
                                    }
                                    AppView::ArgPrompt { id, .. } => {
                                        // Arg prompt key handling via SimulateKey
                                        logging::log("STDIN", &format!("SimulateKey: Dispatching '{}' to ArgPrompt (actions_popup={})", key_lower, view.show_actions_popup));

                                        // Check for Cmd+K to toggle actions popup
                                        if has_cmd && key_lower == "k" {
                                            logging::log("STDIN", "SimulateKey: Cmd+K - toggle arg actions");
                                            view.toggle_arg_actions(ctx, window);
                                        } else if view.show_actions_popup {
                                            // If actions popup is open, route to it
                                            if let Some(ref dialog) = view.actions_dialog {
                                                match key_lower.as_str() {
                                                    "up" | "arrowup" => {
                                                        logging::log("STDIN", "SimulateKey: Up in actions dialog");
                                                        dialog.update(ctx, |d, cx| d.move_up(cx));
                                                    }
                                                    "down" | "arrowdown" => {
                                                        logging::log("STDIN", "SimulateKey: Down in actions dialog");
                                                        dialog.update(ctx, |d, cx| d.move_down(cx));
                                                    }
                                                    "enter" => {
                                                        logging::log("STDIN", "SimulateKey: Enter in actions dialog");
                                                        let action_id = dialog.read(ctx).get_selected_action_id();
                                                        let should_close = dialog.read(ctx).selected_action_should_close();
                                                        if let Some(action_id) = action_id {
                                                            logging::log("ACTIONS", &format!("SimulateKey: Executing action: {} (close={})", action_id, should_close));
                                                            if should_close {
                                                                view.show_actions_popup = false;
                                                                view.actions_dialog = None;
                                                                view.focused_input = FocusedInput::ArgPrompt;
                                                                window.focus(&view.focus_handle, ctx);
                                                            }
                                                            view.trigger_action_by_name(&action_id, ctx);
                                                        }
                                                    }
                                                    "escape" => {
                                                        logging::log("STDIN", "SimulateKey: Escape - close actions dialog");
                                                        view.show_actions_popup = false;
                                                        view.actions_dialog = None;
                                                        view.focused_input = FocusedInput::ArgPrompt;
                                                        window.focus(&view.focus_handle, ctx);
                                                    }
                                                    _ => {
                                                        logging::log("STDIN", &format!("SimulateKey: Unhandled key '{}' in ArgPrompt actions dialog", key_lower));
                                                    }
                                                }
                                            }
                                        } else {
                                            // Normal arg prompt key handling
                                            let prompt_id = id.clone();
                                            match key_lower.as_str() {
                                                "up" | "arrowup" => {
                                                    if view.arg_selected_index > 0 {
                                                        view.arg_selected_index -= 1;
                                                        view.arg_list_scroll_handle.scroll_to_item(view.arg_selected_index, ScrollStrategy::Nearest);
                                                        logging::log("STDIN", &format!("SimulateKey: Arg up, index={}", view.arg_selected_index));
                                                    }
                                                }
                                                "down" | "arrowdown" => {
                                                    let filtered = view.filtered_arg_choices();
                                                    if view.arg_selected_index < filtered.len().saturating_sub(1) {
                                                        view.arg_selected_index += 1;
                                                        view.arg_list_scroll_handle.scroll_to_item(view.arg_selected_index, ScrollStrategy::Nearest);
                                                        logging::log("STDIN", &format!("SimulateKey: Arg down, index={}", view.arg_selected_index));
                                                    }
                                                }
                                                "enter" => {
                                                    logging::log("STDIN", "SimulateKey: Enter - submit selection");
                                                    let filtered = view.filtered_arg_choices();
                                                    if let Some((_, choice)) = filtered.get(view.arg_selected_index) {
                                                        let value = choice.value.clone();
                                                        view.submit_prompt_response(prompt_id, Some(value), ctx);
                                                    } else if !view.arg_input.is_empty() {
                                                        let value = view.arg_input.text().to_string();
                                                        view.submit_prompt_response(prompt_id, Some(value), ctx);
                                                    }
                                                }
                                                "escape" => {
                                                    logging::log("STDIN", "SimulateKey: Escape - cancel script");
                                                    view.submit_prompt_response(prompt_id, None, ctx);
                                                    view.cancel_script_execution(ctx);
                                                }
                                                _ => {
                                                    logging::log("STDIN", &format!("SimulateKey: Unhandled key '{}' in ArgPrompt", key_lower));
                                                }
                                            }
                                        }
                                    }
                                    _ => {
                                        logging::log("STDIN", &format!("SimulateKey: View {:?} not supported for key simulation", std::mem::discriminant(&view.current_view)));
                                    }
                                }
                            }
                            ExternalCommand::OpenNotes => {
                                logging::log("STDIN", "Opening notes window via stdin command");
                                if let Err(e) = notes::open_notes_window(ctx) {
                                    logging::log("STDIN", &format!("Failed to open notes window: {}", e));
                                }
                            }
                            ExternalCommand::OpenAi => {
                                logging::log("STDIN", "Opening AI window via stdin command");
                                if let Err(e) = ai::open_ai_window(ctx) {
                                    logging::log("STDIN", &format!("Failed to open AI window: {}", e));
                                }
                            }
                            ExternalCommand::OpenAiWithMockData => {
                                logging::log("STDIN", "Opening AI window with mock data via stdin command");
                                // First insert mock data
                                if let Err(e) = ai::insert_mock_data() {
                                    logging::log("STDIN", &format!("Failed to insert mock data: {}", e));
                                } else {
                                    logging::log("STDIN", "Mock data inserted successfully");
                                }
                                // Then open the window
                                if let Err(e) = ai::open_ai_window(ctx) {
                                    logging::log("STDIN", &format!("Failed to open AI window: {}", e));
                                }
                            }
                            ExternalCommand::CaptureWindow { title, path } => {
                                logging::log("STDIN", &format!("Capturing window with title '{}' to '{}'", title, path));
                                match capture_window_by_title(&title, false) {
                                    Ok((png_data, width, height)) => {
                                        // Save to file
                                        if let Err(e) = std::fs::write(&path, &png_data) {
                                            logging::log("STDIN", &format!("Failed to write screenshot: {}", e));
                                        } else {
                                            logging::log("STDIN", &format!("Screenshot saved: {} ({}x{})", path, width, height));
                                        }
                                    }
                                    Err(e) => {
                                        logging::log("STDIN", &format!("Failed to capture window: {}", e));
                                    }
                                }
                            }
                        }
                        ctx.notify();
                        }); // close app_entity_inner.update
                    }); // close window_for_stdin.update
                }); // close cx.update
            }

            logging::log("STDIN", "Async stdin command handler exiting");
        }).detach();

        // Tray menu event handler - polls for menu events
        // Clone config for use in tray handler
        let config_for_tray = config::load_config();
        if let Some(tray_mgr) = tray_manager {
            let window_for_tray = window;
            let app_entity_for_tray = app_entity.clone();
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                logging::log("TRAY", "Tray menu event handler started");

                loop {
                    // Poll for tray menu events every 100ms
                    Timer::after(std::time::Duration::from_millis(100)).await;

                    // Check for menu events
                    if let Ok(event) = tray_mgr.menu_event_receiver().try_recv() {
                        match tray_mgr.match_menu_event(&event) {
                            Some(TrayMenuAction::OpenScriptKit) => {
                                logging::log("TRAY", "Open Script Kit menu item clicked");
                                let app_entity_inner = app_entity_for_tray.clone();
                                let _ = cx.update(|cx| {
                                    // Show and focus window (same logic as hotkey handler)
                                    script_kit_gpui::set_main_window_visible(true);

                                    // Calculate new bounds on display with mouse
                                    let window_size = size(px(750.), initial_window_height());
                                    let new_bounds = calculate_eye_line_bounds_on_mouse_display(window_size);

                                    // Move window first
                                    move_first_window_to_bounds(&new_bounds);

                                    // Activate the app
                                    cx.activate(true);

                                    // Configure as floating panel on first show
                                    if !PANEL_CONFIGURED.swap(true, Ordering::SeqCst) {
                                        platform::configure_as_floating_panel();
                                    }

                                    // Focus the window via Root, then update app entity
                                    let _ = window_for_tray.update(cx, |_root, win, root_cx| {
                                        win.activate_window();
                                        app_entity_inner.update(root_cx, |view, ctx| {
                                            let focus_handle = view.focus_handle(ctx);
                                            win.focus(&focus_handle, ctx);

                                            // Reset if needed and ensure proper sizing
                                            reset_resize_debounce();

                                            if NEEDS_RESET.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                                                view.reset_to_script_list(ctx);
                                            } else {
                                                view.update_window_size();
                                            }
                                        });
                                    });
                                });
                            }
                            Some(TrayMenuAction::OpenNotes) => {
                                logging::log("TRAY", "Notes menu item clicked");
                                let _ = cx.update(|cx| {
                                    if let Err(e) = notes::open_notes_window(cx) {
                                        logging::log(
                                            "TRAY",
                                            &format!("Failed to open notes window: {}", e),
                                        );
                                    }
                                });
                            }
                            Some(TrayMenuAction::NewNote) => {
                                logging::log("TRAY", "New Note menu item clicked");
                                let _ = cx.update(|cx| {
                                    if let Err(e) = notes::quick_capture(cx) {
                                        logging::log(
                                            "TRAY",
                                            &format!("Failed to quick capture note: {}", e),
                                        );
                                    }
                                });
                            }
                            Some(TrayMenuAction::OpenAiChat) => {
                                logging::log("TRAY", "AI Chat menu item clicked");
                                let _ = cx.update(|cx| {
                                    if let Err(e) = ai::open_ai_window(cx) {
                                        logging::log(
                                            "TRAY",
                                            &format!("Failed to open AI window: {}", e),
                                        );
                                    }
                                });
                            }
                            Some(TrayMenuAction::LaunchAtLogin) => {
                                // Toggle is handled inside match_menu_event
                                logging::log("TRAY", "Launch at Login toggled");
                            }
                            Some(TrayMenuAction::Settings) => {
                                logging::log("TRAY", "Settings menu item clicked");
                                // Open config file in editor
                                let editor = config_for_tray.get_editor();
                                let config_path = shellexpand::tilde("~/.kenv/config.ts").to_string();

                                logging::log("TRAY", &format!("Opening {} in editor '{}'", config_path, editor));
                                match std::process::Command::new(&editor)
                                    .arg(&config_path)
                                    .spawn()
                                {
                                    Ok(_) => logging::log("TRAY", &format!("Spawned editor: {}", editor)),
                                    Err(e) => logging::log("TRAY", &format!("Failed to spawn editor '{}': {}", editor, e)),
                                }
                            }
                            Some(TrayMenuAction::Quit) => {
                                logging::log("TRAY", "Quit menu item clicked");
                                // Clean up processes and PID file before quitting
                                PROCESS_MANAGER.kill_all_processes();
                                PROCESS_MANAGER.remove_main_pid();
                                let _ = cx.update(|cx| {
                                    cx.quit();
                                });
                                break; // Exit the polling loop
                            }
                            None => {
                                logging::log("TRAY", "Unknown menu event received");
                            }
                        }
                    }
                }

                logging::log("TRAY", "Tray menu event handler exiting");
            }).detach();
        }

        logging::log("APP", "Application ready - Cmd+; to show, Esc to hide, Cmd+K for actions");
    });
}
