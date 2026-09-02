#![allow(unexpected_cfgs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
#![recursion_limit = "8192"]

//! Script Kit GPUI - A GPUI-based launcher for Script Kit
//!
//! This library provides the core functionality for executing scripts
//! with bidirectional JSONL communication.

// Actions - Reusable action dialog component
// Provides ActionsDialog with configurable layout for script actions, AI command bar, etc.
pub mod actions;
pub mod agentic_protocol_bus;

pub mod about;
pub mod atomic_file;
pub mod brain;
pub mod branding;
pub mod calculator;
#[cfg(target_os = "macos")]
pub mod camera;
pub mod chat_hot_counters;
pub mod components;
pub mod config;
pub mod dictation;

// Deterministic AI-relevant desktop context snapshots
pub mod context_snapshot;

// Computer-use vocabulary over the existing automation inspection protocol
pub mod computer_use;

// Unified icon system - single API for all icon sources
// Supports gpui_component IconName, embedded SVGs, SF Symbols, app bundles
pub mod debug_grid;
pub mod design_contract;
// Ordinary macOS libtests exercise catalogue data without compiling native evaluation.
#[cfg(all(test, target_os = "macos"))]
mod design_evaluation {
    mod catalog;
    pub(crate) mod fixture_ids;
}
pub mod designs;
pub mod dev_marker;
pub mod editor;
pub mod effects;
pub mod emoji;
pub mod emoji_usage;
pub mod error;
pub mod executor;
pub mod extension_types;
pub(crate) mod fidelity_capture;
pub mod focus_coordinator;
pub mod form_prompt;
pub mod formatting;
#[cfg(test)]
mod gpui_debug_bounds_tests;
pub mod hotkeys;
pub mod icons;
#[cfg(test)]
pub(crate) mod launch_filter_policy;
pub mod list_item;
pub mod logging;
pub mod menu_syntax;
pub mod navigation;
pub mod panel;
pub mod pasted_image;
pub mod pasted_text;
pub mod perf;
pub mod platform;
#[cfg(test)]
mod prompt_completion;
pub mod prompts;
pub mod protocol;
pub mod runtime_policy;
pub mod scripts;
pub mod scrolling;
pub mod selected_text;
pub mod shortcuts;
pub mod spine;
pub mod status_snapshot;
pub mod sync;
pub mod syntax;
pub mod term_prompt;
pub mod terminal;
pub mod terminal_history;
#[doc(hidden)]
pub mod test_support;
pub mod theme;
pub mod tips;
pub mod toast_manager;

pub mod tray;
pub mod ui;
pub mod updates;
pub mod utils;
pub mod warning_banner;
pub mod window_manager;
pub mod window_ops;
pub mod window_orchestrator;
pub mod window_resize;
pub mod window_state;
pub mod windows;

pub(crate) const ROOT_LAUNCHER_PLACEHOLDER: &str =
    "Search • @ context • / commands • ; capture • : filters";

// Phase 1 system API modules
pub mod clipboard_history;
#[allow(dead_code)]
pub mod clipboard_preview_helpers;
pub mod file_search;
pub mod window_control;

// Render helper data (proper modules so tests are discoverable)
#[allow(dead_code)]
pub mod actions_dialog_items;

// System actions - macOS AppleScript-based system commands
#[cfg(target_os = "macos")]
pub mod system_actions;

// Script creation - Create new scripts and scriptlets
pub mod script_creation;

// Permissions wizard - Check and request macOS permissions
pub mod permissions_wizard;

// One-shot NUX markers (release hints, habit changes)
pub mod nux;

// Menu bar reader - macOS Accessibility API for reading app menus
// Provides get_frontmost_menu_bar() with recursive parsing up to 3 levels
#[cfg(target_os = "macos")]
pub mod menu_bar;

// Menu executor - Execute menu actions via Accessibility API
// Navigates AX hierarchy and performs AXPress on menu items
#[cfg(target_os = "macos")]
pub mod menu_executor;

// Menu cache - SQLite-backed menu bar data caching
// Caches application menu hierarchies by bundle_id to avoid expensive rescans
#[cfg(target_os = "macos")]
pub mod menu_cache;

// Frontmost app tracker - Background observer for tracking active application
// Pre-fetches menu bar items when apps activate (before Script Kit opens)
#[cfg(target_os = "macos")]
pub mod frontmost_app_tracker;

pub(crate) mod context_templates;

// Action helpers - centralized path extraction, SDK action routing, pbcopy
pub mod action_helpers;

// Built-in features registry
pub mod ai_vault;
pub mod app_launcher;
pub mod browser_history;
pub mod browser_tabs;
pub mod builtins;
pub mod day_page;
pub mod favicons;
pub mod hud_manager;

// Typed handle for path-prompt action ids. Physically lives under
// `src/app_impl/` (pulled into the binary via `include!`); the lib
// re-exports the same file so the inline round-trip tests run under
// `cargo test --lib` and MCP/integration consumers can parse ids too.
#[path = "app_impl/path_action.rs"]
pub mod path_action;

// Pure planner from `TriggerBuiltin` to `AppRoute`. Same re-export
// pattern as `path_action` — the physical file lives in `src/app_impl/`
// but the lib crate also carries it so the exhaustive-coverage tests
// and any future golden-transcript consumers can exercise the planner
// under `cargo test --lib`.
#[path = "app_impl/routes.rs"]
pub mod routes;

// Pure menu-syntax trigger picker state machine + row adapter. Same
// `#[path]` re-export pattern as `path_action` and `routes` — the
// binary consumes it via `app_impl::menu_syntax_trigger_picker` while
// the lib crate carries it so unit tests for the state transitions and
// the `TriggerPickerRow` → `InlinePickerRow` adapter run under
// `cargo test --lib`. GPUI wiring and keyboard dispatch land in commit
// D2; this commit ships the pure-library half.
#[path = "app_impl/menu_syntax_trigger_picker.rs"]
pub mod menu_syntax_trigger_picker;

// Pure adapter that bridges `menu_syntax::current_actions` (pure spec) into
// the Cmd+K actions-dialog (live UI). Same `#[path]` re-export pattern as
// `path_action` / `routes` / `menu_syntax_trigger_picker` so unit tests for
// the section title + replace/prepend mode run under `cargo test --lib`.
#[path = "app_impl/menu_syntax_actions.rs"]
pub mod menu_syntax_actions;

// Pure adapter that flattens `menu_syntax::ai::MenuSyntaxAiResponse` into
// the renderable proposal snapshot the inline-AI hint surface consumes.
// Same `#[path]` re-export pattern so unit tests for the response→proposal
// mapping run under `cargo test --lib`.
#[path = "app_impl/menu_syntax_ai.rs"]
pub mod menu_syntax_ai;

// Pure applier for AI proposals (Tab/Enter Accept, Esc Dismiss). Same
// `#[path]` re-export pattern as menu_syntax_ai so the unit tests run
// under `cargo test --lib`.
#[path = "app_impl/menu_syntax_ai_apply.rs"]
pub mod menu_syntax_ai_apply;

// Pure design-contract resolvers for built-in browser surfaces. Same
// `#[path]` re-export pattern as `path_action`: the binary pulls these files
// into its crate root via the `render_builtins/mod.rs` include chain, while
// the lib carries them so the design-token exporter (`src/design_contract`)
// and `cargo test --lib` consume the SAME functions the renderers paint
// with. `builtin_main_input_contract` owns the shared trailing count-label
// style (ALL builtin browsers); `settings_hub_contract` owns the settings
// hub census, labels, and layout.
#[path = "render_builtins/builtin_main_input_contract.rs"]
pub mod builtin_main_input_contract;
#[path = "render_builtins/settings_contract.rs"]
pub mod settings_hub_contract;

// Fallback commands - Raycast-style fallback actions when no scripts match
pub mod fallbacks;

// Favorite script ID storage (~/.scriptkit/favorites.json)
pub mod favorites;

// Frecency tracking for script usage
pub mod frecency;

// Input history for up/down arrow navigation through previous inputs
pub mod input_history;

// Process management for tracking bun script processes
pub mod process_manager;

// Scriptlet parsing and variable substitution
pub mod scriptlets;

// Scriptlet cache for tracking per-file state with change detection
// Used by file watchers to diff scriptlet changes and update registrations incrementally
pub mod scriptlet_cache;

// Typed metadata parser for new `metadata = {}` global syntax
pub mod metadata_parser;

// Schema parser for `schema = { input: {}, output: {} }` definitions
pub mod schema_parser;

// Scriptlet codefence metadata parser for ```metadata and ```schema blocks
pub mod scriptlet_metadata;

// VSCode snippet syntax parser for template() SDK function
pub mod snippet;

// HTML form parsing for form() prompt
pub mod form_parser;

// Centralized template variable substitution system
// Used by expand_manager, template prompts, and future template features
pub mod template_variables;

// Text injection for text expansion/snippet systems
pub mod text_injector;

// Keyword trigger matching for text expansion
pub mod keyword_matcher;

// Debounced keystroke logging for text expansion system
// Consolidates per-keystroke logs into periodic summaries
pub mod keystroke_logger;
pub mod kit_store;

// Global keyboard monitoring for system-wide keystroke capture
// Required for text expansion triggers typed in any application
#[cfg(target_os = "macos")]
pub mod keyboard_monitor;

// Keyword manager - ties together keyboard monitoring, trigger matching,
// and text injection for the complete text expansion system
#[cfg(target_os = "macos")]
pub mod keyword_manager;

// OCR module - macOS Vision framework integration
#[cfg(feature = "ocr")]
pub mod ocr;

// Script scheduling with cron expressions and natural language
pub mod scheduler;
pub mod script_sharing;

// Plugin inventory — canonical plugin manifest, discovery, and skill indexing
pub mod plugins;

// Kenv environment setup and initialization
// Ensures ~/.scriptkit exists with required directories and starter files
pub mod setup;

// MCP Server - HTTP server for Model Context Protocol integration
// Provides localhost:43210 endpoint with Bearer token auth
pub mod mcp_server;

// MCP Streaming - Server-Sent Events (SSE) and audit logging
// Provides real-time event streaming and tool call audit logs
pub mod mcp_streaming;

// MCP Protocol - JSON-RPC 2.0 protocol handler for MCP
// Handles request parsing, method routing, and response generation
pub mod mcp_protocol;

// MCP Mutation Control - shared runtime bridge and mutation registry
pub mod mcp_control;

// MCP Kit Tools - kit/* namespace tools for app control
// Provides kit/show, kit/hide, kit/state tools
pub mod mcp_kit_tools;

// MCP Clipboard Tools - clipboard history mutation tools
pub mod mcp_clipboard_tools;

// MCP Config Tools - config.ts read/write tools backed by config-cli.ts
pub mod mcp_config_tools;

// MCP Notes Tools - notes mutation tools for the full-control plane
pub mod mcp_notes_tools;

// MCP Scripts Tools - script file mutation tools for the full-control plane
pub mod mcp_scripts_tools;

// MCP Computer Use Tools - computer/* namespace for desktop observation
// Provides the stable first slice of agent-facing computer-use capabilities
pub mod mcp_computer_use_tools;

// MCP Script Tools - scripts/* namespace auto-generated tools
// Scripts with schema.input become MCP tools automatically
pub mod mcp_script_tools;

// MCP Resources - read-only data resources for MCP clients
// Provides kit://state, scripts://, and scriptlets:// resources
pub mod mcp_resources;

// Stdin commands - external command handling via stdin
// Provides JSON command protocol for testing and automation
pub mod stdin_commands;

// Protocol-boundary metrics / counters for the JSONL ingress.
pub mod protocol_stats;

// Confirmation dialog - modal confirmation window for destructive actions
pub mod confirm;
#[allow(dead_code)]
pub mod footer_popup;

// Notes - Raycast Notes feature parity
// Separate floating window for note-taking with gpui-component
pub mod notes;

// Agent Chat Chat - Separate floating window for AI conversations
// BYOK (Bring Your Own Key) with SQLite storage at ~/.scriptkit/ai-chats.db
pub mod ai;

// Agents - mdflow agent integration
// Executable markdown prompts that run against Claude, AGY, Codex, or Copilot
// Located in ~/.scriptkit/plugins/*/agents/*.md
pub mod agents;

// Flows - flow-first launcher domain (mdflow front end)
// Contract: docs/ai/flow-ux-protocol.md; the mdflow CLI owns discovery and
// execution, this module is the app-side client (catalog/runner/registry).
pub mod flows;

// Secrets - age-encrypted secrets storage
// Portable alternative to keyring, stores secrets at ~/.scriptkit/secrets.age
// Uses scrypt passphrase-based encryption with machine-derived passphrase
pub mod secrets;

// macOS launch-at-login via SMAppService
// Uses SMAppService on macOS 13+ for modern login item management
pub mod login_item;

// UI transitions/animations (self-contained module, no external crate dependency)
// Provides TransitionColor, Opacity, SlideOffset, AppearTransition, HoverState
// and easing functions (ease_out_quad, ease_in_quad, etc.)
// Used for smooth hover effects, toast animations, and other UI transitions
pub mod transitions;

// UI Foundation - Shared UI patterns for consistent vibrancy and layout
// Extracts common patterns from main menu (render_script_list.rs) into reusable helpers
// Used by term.rs, editor.rs, div.rs, form.rs and other prompts
pub mod ui_foundation;

// File watchers for theme, config, scripts, and system appearance
pub mod watcher;

// Window state management tests - code audit to prevent regressions
// Verifies that app_execute.rs uses close_and_reset_window() correctly
#[doc(hidden)]
pub mod test_utils;

#[cfg(test)]
mod actions_button_visibility_tests;
// Shared window visibility state
// Used to track main window visibility across the app
// Notes/AI windows use this to decide whether to hide the app after closing
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Global handle to the main GPUI window.
/// Used to open in-window dialogs (e.g. parent confirm) from async contexts
/// that only have `&mut AsyncApp` and no `Window` reference.
static MAIN_WINDOW_HANDLE: OnceLock<Mutex<Option<gpui::AnyWindowHandle>>> = OnceLock::new();

/// Store the main window handle after creation.
pub fn set_main_window_handle(handle: gpui::AnyWindowHandle) {
    let storage = MAIN_WINDOW_HANDLE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = storage.lock() {
        *guard = Some(handle);
    }
}

/// Retrieve the main window handle.
pub fn get_main_window_handle() -> Option<gpui::AnyWindowHandle> {
    MAIN_WINDOW_HANDLE
        .get()
        .and_then(|m| m.lock().ok())
        .and_then(|guard| *guard)
}

/// Retire only the lifetime being closed; delayed cleanup cannot clear a reopen.
pub fn clear_main_window_handle_if_matches(handle: gpui::AnyWindowHandle) -> bool {
    let Some(storage) = MAIN_WINDOW_HANDLE.get() else {
        return false;
    };
    let Ok(mut current) = storage.lock() else {
        return false;
    };
    if *current != Some(handle) {
        return false;
    }
    *current = None;
    true
}

/// Global state tracking whether the main window is visible
/// - Used by hotkey toggle to show/hide main window
/// - Used by Notes/AI to prevent main window from appearing when they close
static MAIN_WINDOW_VISIBLE: AtomicBool = AtomicBool::new(false);
static MAIN_WINDOW_VISIBILITY_GENERATION: AtomicU64 = AtomicU64::new(0);

// Oracle-Session `window-activation-invariants-guard` PR1.
// One-shot guard for the main NSPanel's floating-panel configuration. Only
// `platform::ensure_main_panel_configured()` is allowed to flip this — and only
// after `assert_main_panel_invariants` reports `ok()`. Living in the lib crate
// keeps the atomic reachable from `src/platform/app_window_management.rs`,
// which is `include!`-merged into `platform::`.
pub(crate) static PANEL_CONFIGURED: AtomicBool = AtomicBool::new(false);

/// Tracks whether a script requested hiding the window (via Hide message)
/// When ScriptExit is received, if this is true, we show the window again
/// This ensures main menu comes back after scripts that temporarily hide (e.g., getSelectedText)
static SCRIPT_REQUESTED_HIDE: AtomicBool = AtomicBool::new(false);

/// Check if the main window is currently visible
pub fn is_main_window_visible() -> bool {
    MAIN_WINDOW_VISIBLE.load(Ordering::SeqCst)
}

/// Monotonic counter bumped whenever the logical main-window visibility changes.
pub fn main_window_visibility_generation() -> u64 {
    MAIN_WINDOW_VISIBILITY_GENERATION.load(Ordering::SeqCst)
}

/// Hook fired on every real main-window visibility transition.
///
/// The main-hotkey gesture classifier lives in the binary crate (its statics
/// are duplicated per crate), so hide paths compiled into this library — e.g.
/// the active-Space-change observer in `platform::visibility_focus` — cannot
/// reach it directly. The binary installs a hook at startup that keeps the
/// classifier's Closed/Open state aligned with the flag; without it, an
/// implicit hide leaves the classifier `Open` and every later hotkey press
/// classifies as a tap that routing swallows (dead hotkey).
static MAIN_WINDOW_VISIBILITY_TRANSITION_HOOK: OnceLock<fn(bool)> = OnceLock::new();

/// Install the visibility-transition hook (first caller wins).
pub fn install_main_window_visibility_transition_hook(hook: fn(bool)) {
    let _ = MAIN_WINDOW_VISIBILITY_TRANSITION_HOOK.set(hook);
}

/// Set the main window visibility state
pub fn set_main_window_visible(visible: bool) {
    let previous = MAIN_WINDOW_VISIBLE.swap(visible, Ordering::SeqCst);
    if previous != visible {
        if visible {
            crate::tips::advance_footer_tip();
        }
        MAIN_WINDOW_VISIBILITY_GENERATION.fetch_add(1, Ordering::SeqCst);
        if let Some(hook) = MAIN_WINDOW_VISIBILITY_TRANSITION_HOOK.get() {
            hook(visible);
        }
    }
}

/// Set when the main window was hidden by an implicit path (click-outside
/// focus loss, active-Space change) so the next show restores the surface
/// exactly instead of resetting to ScriptList. Lives here rather than in the
/// binary so lib-side hide paths (the Space-change observer) can mark it.
static RESTORE_MAIN_STATE_AFTER_FOCUS_LOSS: AtomicBool = AtomicBool::new(false);

pub fn mark_main_state_restore_after_focus_loss() {
    RESTORE_MAIN_STATE_AFTER_FOCUS_LOSS.store(true, Ordering::SeqCst);
}

pub fn clear_main_state_restore_after_focus_loss() {
    RESTORE_MAIN_STATE_AFTER_FOCUS_LOSS.store(false, Ordering::SeqCst);
}

pub fn consume_main_state_restore_after_focus_loss() -> bool {
    RESTORE_MAIN_STATE_AFTER_FOCUS_LOSS
        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

/// Check if a script requested hiding the window
pub fn script_requested_hide() -> bool {
    SCRIPT_REQUESTED_HIDE.load(Ordering::SeqCst)
}

/// Set the script-requested-hide flag
pub fn set_script_requested_hide(value: bool) {
    SCRIPT_REQUESTED_HIDE.store(value, Ordering::SeqCst);
}

/// Channel for requesting the main window to be shown
/// Used by prompt_handler to signal that the window should come back after script exit
static SHOW_WINDOW_CHANNEL: std::sync::LazyLock<(
    async_channel::Sender<()>,
    async_channel::Receiver<()>,
)> = std::sync::LazyLock::new(|| async_channel::bounded(10));

/// Get the show window channel (sender, receiver)
pub fn show_window_channel() -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>) {
    &SHOW_WINDOW_CHANNEL
}

/// Request showing the main window (called from prompt_handler on ScriptExit)
pub fn request_show_main_window() {
    let (tx, _) = show_window_channel();
    let _ = tx.try_send(());
}

/// Legacy hook retained for split runtime call sites.
///
/// Focus-loss handling must stay immediate; this intentionally does not start
/// a grace timer.
pub fn mark_window_shown() {}

/// Focus-loss grace is disabled so blur-dismiss reacts without an artificial
/// startup/refocus delay.
pub fn is_within_focus_grace_period() -> bool {
    false
}

#[cfg(test)]
mod main_menu_input_guard_tests {
    use std::fs;

    #[test]
    fn test_focus_grace_period_is_disabled() {
        let lib_source = fs::read_to_string("src/lib.rs").expect("Failed to read src/lib.rs");
        let production_source = lib_source
            .split("#[cfg(test)]")
            .next()
            .expect("src/lib.rs should contain production source before tests");
        assert!(
            !production_source.contains("const FOCUS_LOSS_GRACE_PERIOD_MS"),
            "Focus loss should not be delayed by a grace-period constant"
        );
        assert!(!crate::is_within_focus_grace_period());
    }

    #[test]
    fn test_main_render_does_not_delay_focus_loss_with_grace() {
        let render_impl = fs::read_to_string("src/main_sections/render_impl.rs")
            .expect("Failed to read src/main_sections/render_impl.rs");
        assert!(
            render_impl.contains("if !self.was_window_focused && is_window_focused {"),
            "Main render should detect focus gained transition"
        );
        assert!(
            !render_impl.contains("is_within_focus_grace_period"),
            "Main blur handling must not delay focus loss with a grace window"
        );
        assert!(
            !render_impl.contains("within grace period"),
            "Main blur handling must not log a grace-period ignore branch"
        );
    }

    #[test]
    fn test_focus_grace_does_not_block_main_menu_submit() {
        let selection_fallback = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("Failed to read src/app_impl/selection_fallback.rs");
        let startup = fs::read_to_string("src/app_impl/startup.rs")
            .expect("Failed to read src/app_impl/startup.rs");

        let execute_selected_pos = selection_fallback
            .find("pub(crate) fn execute_selected")
            .expect("execute_selected not found");
        let execute_selected_end = (execute_selected_pos + 260).min(selection_fallback.len());
        let execute_selected_section =
            &selection_fallback[execute_selected_pos..execute_selected_end];
        assert!(
            !execute_selected_section.contains("is_within_focus_grace_period")
                && !execute_selected_section
                    .contains("should_ignore_selection_event_during_main_menu_open_guard"),
            "execute_selected must not drop user activation during the focus-loss grace window"
        );

        let press_enter_pos = startup
            .find("InputEvent::PressEnter")
            .expect("PressEnter branch not found");
        let press_enter_end = (press_enter_pos + 1800).min(startup.len());
        let press_enter_section = &startup[press_enter_pos..press_enter_end];
        assert!(
            !press_enter_section.contains("Ignoring PressEnter: within focus grace period"),
            "main-menu PressEnter must submit the selected row even immediately after focus is gained"
        );
    }

    #[test]
    fn test_physical_plain_enter_routes_main_menu_selection() {
        let startup = fs::read_to_string("src/app_impl/startup.rs")
            .expect("Failed to read src/app_impl/startup.rs");
        let physical_enter_pos = startup
            .find("if is_plain_enter {")
            .expect("physical plain Enter interceptor not found");
        let physical_enter_section =
            &startup[physical_enter_pos..(physical_enter_pos + 4500).min(startup.len())];
        let script_issues_pos = physical_enter_section
            .find("AppView::ScriptIssuesView")
            .expect("physical Enter must preserve ScriptIssuesView handling first");
        // Since 8a4ef223f the popup windows are reached through the
        // owns-main-keyboard helpers rather than direct popup-window access;
        // Enter must still Accept the object selector before the trigger picker.
        let object_popup_pos = physical_enter_section
            .find("menu_syntax_object_selector_owns_main_keyboard")
            .expect("physical Enter must preserve menu-syntax object popup Accept");
        let trigger_picker_pos = physical_enter_section
            .find("menu_syntax_trigger_picker_owns_main_keyboard")
            .expect("physical Enter must preserve menu-syntax trigger picker Accept");
        let script_list_pos = physical_enter_section
            .find("global_plain_enter_script_list")
            .expect("physical Enter must route ScriptList through the submit guard");
        let spine_pos = physical_enter_section
            .find("this.try_handle_spine_enter(window, cx)")
            .expect("physical Enter must preserve Spine row acceptance");
        let execute_pos = physical_enter_section
            .find("this.execute_selected(cx);")
            .expect("physical Enter must execute selected ScriptList rows");
        let agent_chat_pos = physical_enter_section
            .find("if let AppView::AgentChatView")
            .expect("physical Enter Agent Chat fallback not found");

        assert!(
            script_issues_pos < object_popup_pos
                && object_popup_pos < trigger_picker_pos
                && trigger_picker_pos < script_list_pos
                && script_list_pos < spine_pos
                && execute_pos < agent_chat_pos,
            "physical plain Enter must keep ScriptIssues/menu-syntax Accept ahead of ScriptList submit, then Spine/canonical selected execution, and only later Agent Chat handling"
        );
        assert!(
            physical_enter_section.contains("!crate::actions::is_actions_window_open()"),
            "physical plain Enter must not steal Enter while the actions window owns it"
        );
        assert!(
            !physical_enter_section.contains("is_within_focus_grace_period"),
            "physical plain Enter must not reintroduce focus-grace submit blocking"
        );
    }
}
