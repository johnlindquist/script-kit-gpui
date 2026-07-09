#![allow(unexpected_cfgs)]
#![allow(dead_code)] // The binary pulls in shared modules that intentionally expose unused paths outside this target.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

//! Binary entrypoint and GPUI application composition for Script Kit GPUI.
//! It defines the `ScriptListApp` runtime, wires stdin protocol handling in `main`,
//! and includes prompt/builtin render implementations for the primary window.
//! This module depends on app subsystems like `protocol`, `hotkeys`, `watcher`, and `window_manager`.

use gpui::{
    div, hsla, list, point, prelude::*, px, rgb, rgba, size, svg, uniform_list, AnyElement, App,
    BoxShadow, Context, CursorStyle, Div, ElementId, Entity, FocusHandle, Focusable, FontWeight,
    KeyDownEvent, ListAlignment, ListOffset, ListSizingBehavior, ListState, MouseMoveEvent, Render,
    ScrollHandle, ScrollStrategy, SharedString, Subscription, UniformListScrollHandle, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
};

// gpui-component Root wrapper for theme and context provision
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::Root;
use gpui_component::{ActiveTheme, Sizable, Size};
use itertools::Itertools;
use std::sync::atomic::{AtomicBool, Ordering};

mod process_manager;
mod profile_search;
use cocoa::base::id;
use cocoa::foundation::NSRect;
use process_manager::PROCESS_MANAGER;

// Platform utilities - mouse position, display info, window movement, screenshots
use platform::{
    calculate_eye_line_bounds_on_mouse_display, capture_targeted_screenshot,
    capture_window_by_title_via_resolver,
};
#[macro_use]
extern crate objc;

mod about;
#[allow(dead_code)] // The bin only uses trigger_sdk_action; the rest is shared with lib/tests.
mod action_helpers;
mod actions;
#[cfg(test)]
mod actions_button_visibility_tests;
mod agents;
mod ai;
mod aliases;
mod brain;
mod branding;
pub mod calculator;
mod camera;
mod components;
mod config;
mod confirm;
mod context_snapshot;
mod designs;
mod dev_marker;
mod flows;
mod dev_style_tool {
    pub mod actions_popup_catalog;
    pub mod agent_chat_catalog;
    pub mod catalog;
    pub mod confirm_modal_catalog;
    pub mod copy_catalog;
    pub(crate) mod export;
    pub mod kitchen_sink_targets;
    pub(crate) mod render;
    pub mod runtime_overrides;
    pub(crate) mod stories;
    pub mod theme_catalog;
    pub mod window;

    #[allow(unused_imports)]
    pub use actions_popup_catalog::*;
    #[allow(unused_imports)]
    pub use agent_chat_catalog::*;
    #[allow(unused_imports)]
    pub use catalog::*;
    #[allow(unused_imports)]
    pub use confirm_modal_catalog::*;
    #[allow(unused_imports)]
    pub use copy_catalog::*;
    #[allow(unused_imports)]
    pub(crate) use kitchen_sink_targets::*;
    #[allow(unused_imports)]
    pub use theme_catalog::*;
}
mod dictation;
mod editor;
mod effects;
#[allow(dead_code)] // Public API in lib.rs; binary only uses ErrorSeverity currently
mod error;
mod executor;
mod filter_coalescer;
mod focus_coordinator;
mod footer_popup;
mod form_prompt;
mod formatting;
mod hotkeys;
#[allow(dead_code)] // Symbols used by hint_strip for SVG keyboard glyph icons
mod icons;
#[allow(unused_imports)]
#[cfg(test)]
mod keyboard_routing_tests;
mod list_item;
#[cfg(test)]
mod list_item_tests;
#[cfg(test)]
mod list_state_init_tests;
mod logging;
mod login_item;
mod menu_syntax;
mod navigation;
mod nux;
mod panel;
mod pasted_image;
mod pasted_text;
mod perf;
mod platform;
mod plugins;
mod prompts;
mod protocol;
mod protocol_stats;
mod scripts;
mod scrolling;
#[cfg(target_os = "macos")]
mod selected_text;
mod setup;
mod shortcuts;
mod spine;
mod stdin_commands;
mod sync;
mod syntax;
mod term_prompt;
mod terminal;
#[cfg(test)]
mod test_utils;
mod theme;
mod transitions;
mod tray;
mod ui;
mod ui_foundation;
mod updates;
mod utils;
mod warning_banner;
mod watcher;
mod window_manager;
mod window_ops;
mod window_orchestrator;
mod window_resize;
mod window_state;
#[cfg(test)]
mod window_state_persistence_tests;
mod windows;

// Phase 1 system API modules
mod clipboard_history;
mod clipboard_preview_helpers;
mod file_search;
mod toast_manager;
mod window_control;

// Render helper data (proper modules so tests are discoverable)
mod actions_dialog_items;

// Secrets - age-encrypted secrets storage (replacement for keyring)
mod secrets;

// System actions - macOS AppleScript-based system commands
#[cfg(target_os = "macos")]
mod system_actions;

// Script creation - Create new scripts and scriptlets
mod script_creation;

// Permissions wizard - Check and request macOS permissions
mod permissions_wizard;

// Built-in features registry
mod ai_vault;
mod app_launcher;
mod browser_history;
mod browser_tabs;
mod builtins;
mod fallbacks;
mod favicons;
mod favorites;
mod menu_bar;

// Frontmost app tracker - Background observer for tracking active application
#[cfg(target_os = "macos")]
mod frontmost_app_tracker;

mod context_templates;

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

// Input history for shell-like up/down navigation through previous inputs
mod input_history;

// VSCode snippet syntax parser for template() SDK function
mod snippet;

// HTML form parsing for form() prompt
mod form_parser;

// Centralized template variable substitution system
mod template_variables;

// Text expansion system components (macOS only)
#[cfg(target_os = "macos")]
mod keyboard_monitor;
mod keystroke_logger;
mod keyword_matcher;
mod text_injector;

// Keyword manager - text expansion system integration
#[cfg(target_os = "macos")]
mod keyword_manager;

// Script scheduling with cron expressions and natural language
mod scheduler;
mod script_sharing;

// Startup profile for dev-fast mode and deferred service bootstrap
mod startup_profile;

// HUD manager - system-level overlay notifications (separate floating windows)
mod hud_manager;

// Debug grid overlay for visual testing
mod debug_grid;

// Main-window Execution Contract preflight rail
mod main_window_preflight;

// MCP Server modules for AI agent integration
mod computer_use;
mod mcp_clipboard_tools;
mod mcp_computer_use_tools;
mod mcp_config_tools;
mod mcp_control;
mod mcp_kit_tools;
mod mcp_notes_tools;
mod mcp_protocol;
mod mcp_resources;
mod mcp_script_tools;
mod mcp_scripts_tools;
mod mcp_server;
mod mcp_streaming;

// Notes - Raycast Notes feature parity (separate floating window)
mod notes;

use crate::components::text_input::TextInputState;
use crate::components::toast::{Toast, ToastAction};
use crate::error::ErrorSeverity;
use crate::filter_coalescer::FilterCoalescer;
use crate::form_prompt::FormPromptState;
use crate::navigation::{NavCoalescer, NavDirection};
use crate::toast_manager::ToastManager;
use editor::EditorPrompt;
use prompts::{
    ContainerOptions, ContainerPadding, DivPrompt, DropPrompt, EnvPrompt, PathInfo, PathPrompt,
    PathPromptEvent, SelectPrompt, TemplatePrompt,
};
use tray::{TrayManager, TrayMenuAction};
use ui_foundation::get_vibrancy_background;
use warning_banner::{WarningBanner, WarningBannerColors};
use window_resize::{initial_window_height, reset_resize_debounce, resize_to_view_sync, ViewType};

use components::{FormFieldColors, PromptFooterColors, PromptFooterConfig};
use designs::{get_tokens, render_design_item, DesignVariant};
use frecency::FrecencyStore;
use list_item::{
    render_section_header, GroupedListItem, ListItem, ListItemColors, FONT_MONO, LIST_ITEM_HEIGHT,
    LOG_PANEL_MAX_HEIGHT,
};
// strip_html_tags removed - DivPrompt now renders HTML properly

use actions::{
    close_actions_window, is_actions_window_open, notify_actions_window, open_actions_window,
    ActionsDialog, ScriptInfo,
};
use panel::{
    CURSOR_HEIGHT_LG, CURSOR_MARGIN_Y, CURSOR_WIDTH, HEADER_GAP, HEADER_PADDING_X, HEADER_PADDING_Y,
};
use parking_lot::Mutex as ParkingMutex;
use protocol::{Choice, Message, ProtocolAction};
use std::sync::{mpsc, Arc, Mutex};
use syntax::highlight_code_lines;

/// Channel for sending prompt messages from script thread to UI
#[allow(dead_code)]
type PromptChannel = (mpsc::Sender<PromptMessage>, mpsc::Receiver<PromptMessage>);

// Import utilities from modules
use stdin_commands::{
    create_stdout_response_sender, start_stdin_listener, validate_capture_window_output_path,
    ExternalCommand, StdinCommand, StdinCommandEnvelope,
};
use utils::render_path_with_highlights;

// Global state for hotkey signaling between threads
static NEEDS_RESET: AtomicBool = AtomicBool::new(false); // Track if window needs reset to script list on next show

/// Keep the main-hotkey gesture classifier (a binary-crate static) aligned
/// with the shared visibility flag. Installed into the library's
/// `set_main_window_visible` chokepoint so EVERY hide path — including
/// lib-side ones like the active-Space-change observer — re-arms the
/// classifier. Without this, an implicit hide leaves the classifier `Open`,
/// the next key-down never classifies as `ShowImmediate`, and the main
/// hotkey goes permanently dead.
fn main_window_visibility_transition_hook(visible: bool) {
    if visible {
        crate::hotkeys::sync_main_gesture_window_shown();
    } else {
        crate::hotkeys::reset_main_gesture_classifier();
    }
}

pub use script_kit_gpui::{
    agentic_protocol_bus, clear_main_state_restore_after_focus_loss,
    consume_main_state_restore_after_focus_loss, emoji, emoji_usage, get_main_window_handle,
    is_main_window_visible, main_window_visibility_generation,
    mark_main_state_restore_after_focus_loss, set_main_window_handle, set_main_window_visible,
    terminal_history,
};
// Oracle-Session `window-activation-invariants-guard` PR1 — the
// `PANEL_CONFIGURED` one-shot lives at each crate root so
// `platform::ensure_main_panel_configured()` can reach it from both the bin
// and lib compilations of the include!-merged platform module. The lib-side
// mirror is at `script_kit_gpui::PANEL_CONFIGURED` (see `src/lib.rs`).
static PANEL_CONFIGURED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false); // Track if shutdown signal received (prevents new script spawns)

include!("main_sections/deeplink.rs");
include!("main_sections/window_visibility.rs");
include!("main_sections/fallbacks.rs");
include!("main_sections/fonts.rs");
include!("main_sections/day_page_types.rs");
include!("main_sections/app_view_state.rs");
include!("main_sections/prompt_messages.rs");
include!("main_sections/app_state.rs");
#[path = "main_sections/root_search_store.rs"]
mod root_search_store;
use root_search_store::RootSearchStore;
include!("main_sections/day_page_markdown_refs.rs");
include!("main_sections/day_page_spine.rs");
include!("main_sections/day_page_layout.rs");
include!("main_sections/day_page_view.rs");
include!("main_sections/day_page_switcher.rs");
include!("main_sections/day_page_actions.rs");
include!("main_sections/day_page_context_round_trip.rs");
include!("main_sections/gesture_routing.rs");
include!("main_sections/release_hints.rs");
include!("main_sections/kitchen_sink_fixture.rs");
// Core ScriptListApp implementation extracted to app_impl/mod.rs
include!("app_impl/mod.rs");

// Script execution logic (execute_interactive) extracted
include!("execute_script/mod.rs");

// Prompt message handling (handle_prompt_message) extracted
include!("prompt_handler/mod.rs");

// App navigation methods (selection movement, scrolling)
include!("app_navigation.rs");

// App execution methods (execute_builtin, execute_app, execute_window_focus)
include!("app_execute.rs");

// App actions handling (handle_action, trigger_action_by_name)
include!("app_actions.rs");

// Layout calculation methods (build_component_bounds, build_layout_info)
include!("app_layout.rs");

include!("main_sections/render_impl.rs");
// Render methods extracted to app_render.rs for maintainability
include!("app_render.rs");

// Builtin view render methods (clipboard, app launcher, window switcher)
include!("render_builtins.rs");

// Prompt render methods - split into separate files for maintainability
// Each file adds render_*_prompt methods to ScriptListApp via impl blocks
include!("render_prompts/key_handler.rs");
include!("render_prompts/arg.rs");
include!("render_prompts/div.rs");
include!("render_prompts/form.rs");
include!("render_prompts/term.rs");
include!("render_prompts/editor.rs");
include!("render_prompts/path.rs");
include!("render_prompts/other.rs");
include!("render_prompts/mini.rs");
include!("render_prompts/micro.rs");

// Script list render method
include!("render_script_list/mod.rs");

fn execute_notes_run_command_from_main(
    command_id: &str,
    cx: &mut gpui::App,
) -> Result<bool, String> {
    let Some(handle) = crate::get_main_window_handle() else {
        tracing::warn!(
            event = "notes_run_command_main_window_unavailable",
            command_id = %command_id,
        );
        return Err("Main window handle is unavailable".to_string());
    };

    handle
        .update(cx, |any_view, _window, cx| -> Result<bool, String> {
            let root = any_view
                .downcast::<gpui_component::Root>()
                .map_err(|_| "Main window root is not gpui_component::Root".to_string())?;
            let inner = root.read(cx).view().clone();
            let app = inner
                .downcast::<ScriptListApp>()
                .map_err(|_| "Main window root view is not ScriptListApp".to_string())?;
            app.update(cx, |app, cx| {
                #[cfg(debug_assertions)]
                if let Some(path) = std::env::var_os("SCRIPT_KIT_TEST_NOTES_RUN_EXEC_RECEIPT") {
                    return write_notes_run_exec_probe_receipt(path, command_id).map(|()| false);
                }

                Ok(app.execute_by_command_id_or_path(command_id, cx))
            })
        })
        .map_err(|error| format!("Failed to update main window: {error}"))?
}

#[cfg(debug_assertions)]
fn write_notes_run_exec_probe_receipt(
    path: std::ffi::OsString,
    command_id: &str,
) -> Result<(), String> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Failed to open Notes run receipt: {error}"))?;
    writeln!(
        file,
        "{}",
        serde_json::json!({
            "source": "notesRunDeeplink",
            "commandId": command_id,
        })
    )
    .map_err(|error| format!("Failed to write Notes run receipt: {error}"))
}

fn main() {
    script_kit_gpui::install_main_window_visibility_transition_hook(
        main_window_visibility_transition_hook,
    );
    crate::notes::register_notes_run_command_executor(execute_notes_run_command_from_main);
    include!("main_entry/app_run_setup.rs");
}
#[cfg(test)]
mod tests {
    use super::{is_main_window_visible, set_main_window_visible};

    /// The visibility flag and the gesture classifier are process-wide state;
    /// tests touching them must serialize through this lock.
    static VISIBILITY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn main_window_visibility_is_shared_with_library() {
        let _guard = VISIBILITY_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set_main_window_visible(false);
        script_kit_gpui::set_main_window_visible(false);

        set_main_window_visible(true);
        assert!(
            script_kit_gpui::is_main_window_visible(),
            "library visibility should mirror main visibility"
        );

        script_kit_gpui::set_main_window_visible(false);
        assert!(
            !is_main_window_visible(),
            "main visibility should mirror library visibility"
        );
    }

    /// Regression: hide paths that only clear the visibility flag (click-outside
    /// focus loss, active-Space change) used to leave the gesture classifier
    /// `Open`, so the next hotkey key-down never classified as `ShowImmediate`
    /// and routing swallowed every press — the main hotkey went permanently
    /// dead. The transition hook installed in `main()` must re-arm it.
    #[test]
    fn implicit_hide_rearms_main_hotkey_show_immediate() {
        let _guard = VISIBILITY_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        script_kit_gpui::install_main_window_visibility_transition_hook(
            super::main_window_visibility_transition_hook,
        );

        // Shown steady state: flag true, classifier Open.
        set_main_window_visible(true);
        crate::hotkeys::sync_main_gesture_window_shown();

        // Implicit hide: only the shared flag is cleared — no explicit
        // classifier reset, exactly like the Space-change/focus-loss paths.
        set_main_window_visible(false);

        let events = crate::hotkeys::process_main_hotkey_physical_event(
            crate::hotkeys::MainHotkeyPhysicalEvent {
                phase: crate::hotkeys::MainHotkeyPhase::KeyDown,
                correlation_id: "test:implicit-hide-rearm".to_string(),
                at: std::time::Instant::now(),
            },
        );
        assert!(
            events.contains(&crate::hotkeys::gesture::GestureEvent::ShowImmediate),
            "key-down after an implicit hide must classify as ShowImmediate, got {events:?}"
        );

        // Leave the classifier in the closed steady state for other tests.
        crate::hotkeys::reset_main_gesture_classifier();
    }
}
