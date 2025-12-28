#![allow(unexpected_cfgs)]

use gpui::{
    div, svg, prelude::*, px, point, rgb, rgba, size, App, Application, Bounds, Context, Render,
    Window, WindowBounds, WindowOptions, SharedString, FocusHandle, Focusable, Entity,
    WindowHandle, Timer, Pixels, WindowBackgroundAppearance, AnyElement, BoxShadow, hsla,
    uniform_list, UniformListScrollHandle, ScrollStrategy, ElementId,
};
use global_hotkey::{GlobalHotKeyManager, GlobalHotKeyEvent, HotKeyState, hotkey::{HotKey, Modifiers, Code}};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use cocoa::appkit::NSApp;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSSize};
#[macro_use]
extern crate objc;

mod scripts;
mod executor;
mod logging;
mod theme;
mod watcher;
mod protocol;
mod prompts;
mod config;
mod panel;
mod actions;
mod list_item;
mod syntax;
mod utils;
mod perf;
mod error;
mod designs;
mod term_prompt;
mod terminal;
mod components;
#[cfg(target_os = "macos")]
mod selected_text;
mod tray;
mod editor;
mod window_resize;
mod window_manager;

// Phase 1 system API modules
mod clipboard_history;
mod window_control;
mod file_search;
mod toast_manager;

// Built-in features registry
mod builtins;
mod app_launcher;

use tray::{TrayManager, TrayMenuAction};
use editor::EditorPrompt;
use window_resize::{ViewType, height_for_view, resize_first_window_to_height, initial_window_height, reset_resize_debounce, defer_resize_to_view};
use crate::toast_manager::ToastManager;
use crate::components::toast::{Toast, ToastColors, ToastAction};

use list_item::{ListItem, ListItemColors, LIST_ITEM_HEIGHT};
use utils::strip_html_tags;
use error::ErrorSeverity;
use designs::{DesignVariant, render_design_item, get_tokens};
use components::{Button, ButtonColors, ButtonVariant};

use std::sync::{Arc, Mutex, mpsc};
use protocol::{Message, Choice};
use actions::{ActionsDialog, ScriptInfo};
use syntax::highlight_code_lines;
use panel::DEFAULT_PLACEHOLDER;

/// Channel for sending prompt messages from script thread to UI
#[allow(dead_code)]
type PromptChannel = (mpsc::Sender<PromptMessage>, mpsc::Receiver<PromptMessage>);

/// Get the current global mouse cursor position using macOS Core Graphics API.
/// Returns the position in screen coordinates.
fn get_global_mouse_position() -> Option<(f64, f64)> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let location = event.location();
    Some((location.x, location.y))
}

/// Represents a display's bounds in macOS global coordinate space
#[derive(Debug, Clone)]
struct DisplayBounds {
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
}

/// Get all displays with their actual bounds in macOS global coordinates.
/// This uses NSScreen directly because GPUI's display.bounds() doesn't return
/// correct origins for secondary displays.
fn get_macos_displays() -> Vec<DisplayBounds> {
    unsafe {
        let screens: id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];
        
        // Get primary screen height for coordinate flipping
        // macOS coordinates: Y=0 at bottom of primary screen
        let main_screen: id = msg_send![screens, firstObject];
        let main_frame: NSRect = msg_send![main_screen, frame];
        let primary_height = main_frame.size.height;
        
        let mut displays = Vec::with_capacity(count);
        
        for i in 0..count {
            let screen: id = msg_send![screens, objectAtIndex:i];
            let frame: NSRect = msg_send![screen, frame];
            
            // Convert from macOS bottom-left origin to top-left origin
            // macOS: y=0 at bottom, increasing upward
            // We want: y=0 at top, increasing downward
            let flipped_y = primary_height - frame.origin.y - frame.size.height;
            
            displays.push(DisplayBounds {
                origin_x: frame.origin.x,
                origin_y: flipped_y,
                width: frame.size.width,
                height: frame.size.height,
            });
        }
        
        displays
    }
}

/// Move the key window (focused window) to a new position using native macOS APIs.
/// Position is specified as origin (top-left corner) in screen coordinates.
///
/// IMPORTANT: macOS uses a global coordinate space where Y=0 is at the BOTTOM of the
/// PRIMARY screen, and Y increases upward. The primary screen's origin is always (0,0)
/// at its bottom-left corner. Secondary displays have their own position in this space.
///
/// Move the application's main window to new bounds using WindowManager.
/// This uses the registered main window instead of objectAtIndex:0, which
/// avoids issues with tray icons and other system windows in the array.
fn move_first_window_to(x: f64, y: f64, width: f64, height: f64) {
    unsafe {
        // Use WindowManager to get the main window reliably
        let window = match window_manager::get_main_window() {
            Some(w) => w,
            None => {
                logging::log("POSITION", "WARNING: Main window not registered in WindowManager, cannot move");
                return;
            }
        };
        
        // Get the PRIMARY screen's height for coordinate conversion
        let screens: id = msg_send![class!(NSScreen), screens];
        let main_screen: id = msg_send![screens, firstObject];
        let main_screen_frame: NSRect = msg_send![main_screen, frame];
        let primary_screen_height = main_screen_frame.size.height;
        
        // Log current window position before move
        let current_frame: NSRect = msg_send![window, frame];
        logging::log("POSITION", &format!(
            "Current window frame: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
            current_frame.origin.x, current_frame.origin.y,
            current_frame.size.width, current_frame.size.height
        ));
        
        // Convert from top-left origin (y down) to bottom-left origin (y up)
        let flipped_y = primary_screen_height - y - height;
        
        logging::log("POSITION", &format!(
            "Moving window: target=({:.0}, {:.0}) flipped_y={:.0}",
            x, y, flipped_y
        ));
        
        let new_frame = NSRect::new(
            NSPoint::new(x, flipped_y),
            NSSize::new(width, height),
        );
        
        // Move the window
        let _: () = msg_send![window, setFrame:new_frame display:true animate:false];
        
        // NOTE: We no longer call makeKeyAndOrderFront here.
        // Window ordering/activation is handled by GPUI's cx.activate() and win.activate_window()
        // which is called AFTER ensure_move_to_active_space() sets the collection behavior.
        
        // Verify the move worked
        let after_frame: NSRect = msg_send![window, frame];
        logging::log("POSITION", &format!(
            "Window moved: actual=({:.0}, {:.0}) size={:.0}x{:.0}",
            after_frame.origin.x, after_frame.origin.y,
            after_frame.size.width, after_frame.size.height
        ));
    }
}

/// Move the first window to new bounds (wrapper for Bounds<Pixels>)
fn move_first_window_to_bounds(bounds: &Bounds<Pixels>) {
    let x: f64 = bounds.origin.x.into();
    let y: f64 = bounds.origin.y.into();
    let width: f64 = bounds.size.width.into();
    let height: f64 = bounds.size.height.into();
    move_first_window_to(x, y, width, height);
}

/// Capture a screenshot of the app window using macOS screencapture command.
/// 
/// Returns a tuple of (png_data, width, height) on success.
/// The function:
/// 1. Gets the window ID using AppleScript
/// 2. Runs screencapture with the window ID to capture just the app window
/// 3. Reads the PNG file and extracts dimensions from the header
/// 4. Cleans up the temp file
#[cfg(target_os = "macos")]
fn capture_app_screenshot() -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    use std::process::Command;
    use std::fs;
    
    // Create temp file path with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let screenshot_path = format!("/tmp/script-kit-screenshot-{}.png", timestamp);
    
    // Try to get window ID via AppleScript
    // This looks for the first window of any process whose name contains "script-kit-gpui"
    let window_id_output = Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to get id of first window of (first process whose name contains \"script-kit-gpui\")"])
        .output()?;
    
    let window_id = String::from_utf8_lossy(&window_id_output.stdout).trim().to_string();
    
    tracing::debug!(window_id = %window_id, "Got window ID for screenshot");
    
    // Capture screenshot
    let capture_result = if !window_id.is_empty() && window_id.parse::<i64>().is_ok() {
        // Use -l flag to capture specific window by ID
        // -x: no sound
        // -o: in window capture mode, do not capture the shadow
        Command::new("screencapture")
            .args([&format!("-l{}", window_id), "-x", "-o", &screenshot_path])
            .output()
    } else {
        tracing::warn!("Could not get window ID, falling back to main display capture");
        // Fallback to main display capture
        // -m: capture main display only
        Command::new("screencapture")
            .args(["-m", "-x", &screenshot_path])
            .output()
    };
    
    capture_result?;
    
    // Read the PNG file
    let png_data = fs::read(&screenshot_path).map_err(|e| {
        format!("Failed to read screenshot file: {}", e)
    })?;
    
    // Verify PNG signature and extract dimensions from IHDR chunk
    // PNG structure: 8-byte signature + chunks
    // IHDR chunk: 4 bytes length + "IHDR" + 4 bytes width + 4 bytes height + ...
    if png_data.len() < 24 {
        let _ = fs::remove_file(&screenshot_path);
        return Err("Screenshot file too small to be valid PNG".into());
    }
    
    // Check PNG signature (first 8 bytes)
    let png_signature: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if png_data[0..8] != png_signature {
        let _ = fs::remove_file(&screenshot_path);
        return Err("Invalid PNG signature".into());
    }
    
    // Width is at bytes 16-19, height at bytes 20-23 (big-endian)
    let width = u32::from_be_bytes([png_data[16], png_data[17], png_data[18], png_data[19]]);
    let height = u32::from_be_bytes([png_data[20], png_data[21], png_data[22], png_data[23]]);
    
    tracing::debug!(width = width, height = height, file_size = png_data.len(), "Screenshot captured");
    
    // Clean up temp file
    let _ = fs::remove_file(&screenshot_path);
    
    Ok((png_data, width, height))
}

#[cfg(not(target_os = "macos"))]
fn capture_app_screenshot() -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    Err("Screenshot capture is only supported on macOS".into())
}

/// Calculate window bounds positioned at eye-line height on the display containing the mouse cursor.
/// 
/// - Finds the display where the mouse cursor is located
/// - Centers the window horizontally on that display
/// - Positions the window at "eye-line" height (upper 14% of the screen)
/// 
/// This matches the behavior of Raycast/Alfred where the prompt appears on the active display.
fn calculate_eye_line_bounds_on_mouse_display(
    window_size: gpui::Size<Pixels>,
    _cx: &App,
) -> Bounds<Pixels> {
    // Use native macOS API to get actual display bounds with correct origins
    // GPUI's cx.displays() returns incorrect origins for secondary displays
    let displays = get_macos_displays();
    
    logging::log("POSITION", "");
    logging::log("POSITION", "╔════════════════════════════════════════════════════════════╗");
    logging::log("POSITION", "║  CALCULATING WINDOW POSITION FOR MOUSE DISPLAY             ║");
    logging::log("POSITION", "╚════════════════════════════════════════════════════════════╝");
    logging::log("POSITION", &format!("Available displays: {}", displays.len()));
    
    // Log all available displays for debugging
    for (idx, display) in displays.iter().enumerate() {
        let right = display.origin_x + display.width;
        let bottom = display.origin_y + display.height;
        logging::log("POSITION", &format!(
            "  Display {}: origin=({:.0}, {:.0}) size={:.0}x{:.0} [bounds: x={:.0}..{:.0}, y={:.0}..{:.0}]",
            idx, display.origin_x, display.origin_y, display.width, display.height,
            display.origin_x, right, display.origin_y, bottom
        ));
    }
    
    // Try to get mouse position and find which display contains it
    let target_display = if let Some((mouse_x, mouse_y)) = get_global_mouse_position() {
        logging::log("POSITION", &format!("Mouse cursor at ({:.0}, {:.0})", mouse_x, mouse_y));
        
        // Find the display that contains the mouse cursor
        let found = displays.iter().enumerate().find(|(idx, display)| {
            let contains = mouse_x >= display.origin_x && mouse_x < display.origin_x + display.width &&
                           mouse_y >= display.origin_y && mouse_y < display.origin_y + display.height;
            
            if contains {
                logging::log("POSITION", &format!("  -> Mouse is on display {}", idx));
            }
            contains
        });
        
        found.map(|(_, d)| d.clone())
    } else {
        logging::log("POSITION", "Could not get mouse position, using primary display");
        None
    };
    
    // Use the found display, or fall back to first display (primary)
    let display = target_display
        .or_else(|| {
            logging::log("POSITION", "No display contains mouse, falling back to primary");
            displays.first().cloned()
        });
    
    if let Some(display) = display {
        logging::log("POSITION", &format!(
            "Selected display: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
            display.origin_x, display.origin_y, display.width, display.height
        ));
        
        // Eye-line: position window top at ~14% from screen top (input bar at eye level)
        let eye_line_y = display.origin_y + display.height * 0.14;
        
        // Center horizontally on the display
        let window_width: f64 = window_size.width.into();
        let center_x = display.origin_x + (display.width - window_width) / 2.0;
        
        let final_bounds = Bounds {
            origin: point(px(center_x as f32), px(eye_line_y as f32)),
            size: window_size,
        };
        
        logging::log("POSITION", &format!(
            "Final window bounds: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
            center_x, eye_line_y,
            f64::from(window_size.width), f64::from(window_size.height)
        ));
        
        final_bounds
    } else {
        logging::log("POSITION", "No displays found, using default centered bounds");
        // Fallback: just center on screen using 1512x982 as default (common MacBook)
        Bounds {
            origin: point(px(381.0), px(246.0)),
            size: window_size,
        }
    }
}

// Global state for hotkey signaling between threads
// HOTKEY_CHANNEL: Event-driven async_channel for hotkey events (replaces AtomicBool polling)
static HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> = OnceLock::new();

/// Get the hotkey channel, initializing it on first access
fn hotkey_channel() -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>) {
    HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

static HOTKEY_TRIGGER_COUNT: AtomicU64 = AtomicU64::new(0);
static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(false); // Track window visibility for toggle (starts hidden)
static NEEDS_RESET: AtomicBool = AtomicBool::new(false); // Track if window needs reset to script list on next show
static PANEL_CONFIGURED: AtomicBool = AtomicBool::new(false); // Track if floating panel has been configured (one-time setup on first show)

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
    },
    /// Showing a div prompt from a script
    DivPrompt {
        id: String,
        html: String,
        tailwind: Option<String>,
    },
    /// Showing a form prompt from a script (HTML form with submit button)
    FormPrompt {
        id: String,
        html: String,
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
}

/// Wrapper to hold a script session that can be shared across async boundaries
type SharedSession = Arc<Mutex<Option<executor::ScriptSession>>>;

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
    ShowArg { id: String, placeholder: String, choices: Vec<Choice> },
    ShowDiv { id: String, html: String, tailwind: Option<String> },
    ShowForm { id: String, html: String },
    ShowTerm { id: String, command: Option<String> },
    ShowEditor { id: String, content: Option<String>, language: Option<String> },
    HideWindow,
    OpenBrowser { url: String },
    ScriptExit,
    /// External command to run a script by path
    RunScript { path: String },
    /// Script error with detailed information for toast display
    ScriptError {
        error_message: String,
        stderr_output: Option<String>,
        exit_code: Option<i32>,
        stack_trace: Option<String>,
        script_path: String,
        suggestions: Vec<String>,
    },
    /// Unhandled message type from script - shows warning toast
    UnhandledMessage { message_type: String },
    /// Request to get current UI state - triggers StateResult response
    GetState { request_id: String },
}

/// External commands that can be sent to the app via stdin
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ExternalCommand {
    /// Run a script by path
    Run { path: String },
    /// Show the window
    Show,
    /// Hide the window  
    Hide,
    /// Set the filter text (for testing)
    SetFilter { text: String },
}

/// Start a thread that listens on stdin for external JSONL commands.
/// Returns an async_channel::Receiver that can be awaited without polling.
fn start_stdin_listener() -> async_channel::Receiver<ExternalCommand> {
    use std::io::BufRead;
    
    let (tx, rx) = async_channel::unbounded();
    
    std::thread::spawn(move || {
        logging::log("STDIN", "External command listener started");
        let stdin = std::io::stdin();
        let reader = stdin.lock();
        
        for line in reader.lines() {
            match line {
                Ok(line) if !line.trim().is_empty() => {
                    logging::log("STDIN", &format!("Received: {}", line));
                    match serde_json::from_str::<ExternalCommand>(&line) {
                        Ok(cmd) => {
                            logging::log("STDIN", &format!("Parsed command: {:?}", cmd));
                            // send_blocking is used since we're in a sync thread
                            if tx.send_blocking(cmd).is_err() {
                                logging::log("STDIN", "Command channel closed, exiting");
                                break;
                            }
                        }
                        Err(e) => {
                            logging::log("STDIN", &format!("Failed to parse command: {}", e));
                        }
                    }
                }
                Ok(_) => {} // Empty line, ignore
                Err(e) => {
                    logging::log("STDIN", &format!("Error reading stdin: {}", e));
                    break;
                }
            }
        }
        logging::log("STDIN", "External command listener exiting");
    });
    
    rx
}

/// A simple model that listens for hotkey triggers via async_channel (event-driven)
struct HotkeyPoller {
    window: WindowHandle<ScriptListApp>,
}

impl HotkeyPoller {
    fn new(window: WindowHandle<ScriptListApp>) -> Self {
        Self { window }
    }
    
    fn start_listening(&self, cx: &mut Context<Self>) {
        let window = self.window;
        // Event-driven: recv().await yields immediately when hotkey is pressed
        // No polling - replaces 100ms Timer::after loop
        cx.spawn(async move |_this, cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "Hotkey listener started (event-driven via async_channel)");
            
            while let Ok(()) = hotkey_channel().1.recv().await {
                logging::log("VISIBILITY", "");
                logging::log("VISIBILITY", "╔════════════════════════════════════════════════════════════╗");
                logging::log("VISIBILITY", "║  HOTKEY TRIGGERED - TOGGLE WINDOW                          ║");
                logging::log("VISIBILITY", "╚════════════════════════════════════════════════════════════╝");
                
                // Check current visibility state for toggle behavior
                let is_visible = WINDOW_VISIBLE.load(Ordering::SeqCst);
                let needs_reset = NEEDS_RESET.load(Ordering::SeqCst);
                logging::log("VISIBILITY", &format!("State check: WINDOW_VISIBLE={}, NEEDS_RESET={}", is_visible, needs_reset));
                
                if is_visible {
                    logging::log("VISIBILITY", "Decision: HIDE (window is currently visible)");
                    // Update visibility state FIRST to prevent race conditions
                    // Even though the hide is async, we mark it as hidden immediately
                    WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                    logging::log("VISIBILITY", "WINDOW_VISIBLE set to: false");
                    
                    // Window is visible - check if in prompt mode
                    let window_clone = window;
                    
                    // First check if we're in a prompt - if so, cancel and hide
                    let _ = cx.update(move |cx: &mut App| {
                        let _ = window_clone.update(cx, |view: &mut ScriptListApp, _win: &mut Window, ctx: &mut Context<ScriptListApp>| {
                            if view.is_in_prompt() {
                                logging::log("HOTKEY", "In prompt mode - canceling script before hiding");
                                view.cancel_script_execution(ctx);
                            }
                        });
                        
                        // Always hide the window when hotkey pressed while visible
                        logging::log("HOTKEY", "Hiding window (toggle: visible -> hidden)");
                        // PERF: Measure window hide latency
                        let hide_start = std::time::Instant::now();
                        cx.hide();
                        let hide_elapsed = hide_start.elapsed();
                        logging::log("PERF", &format!(
                            "Window hide took {:.2}ms",
                            hide_elapsed.as_secs_f64() * 1000.0
                        ));
                        logging::log("HOTKEY", "Window hidden via cx.hide()");
                    });
                } else {
                    logging::log("VISIBILITY", "Decision: SHOW (window is currently hidden)");
                    // Update visibility state FIRST to prevent race conditions
                    WINDOW_VISIBLE.store(true, Ordering::SeqCst);
                    logging::log("VISIBILITY", "WINDOW_VISIBLE set to: true");
                    
                    let window_clone = window;
                    let _ = cx.update(move |cx: &mut App| {
                        // Step 0: CRITICAL - Set MoveToActiveSpace BEFORE any activation
                        // This MUST happen before move_first_window_to_bounds, cx.activate(), 
                        // or win.activate_window() to prevent macOS from switching spaces
                        ensure_move_to_active_space();
                        
                        // Step 1: Calculate new bounds on display with mouse, at eye-line height
                        let window_size = size(px(750.), initial_window_height());
                        let new_bounds = calculate_eye_line_bounds_on_mouse_display(window_size, cx);
                        
                        logging::log("HOTKEY", &format!(
                            "Calculated bounds: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
                            f64::from(new_bounds.origin.x),
                            f64::from(new_bounds.origin.y),
                            f64::from(new_bounds.size.width),
                            f64::from(new_bounds.size.height)
                        ));
                        
                        // Step 2: Move window (position only, no activation)
                        // Note: makeKeyAndOrderFront was removed - ordering happens via GPUI below
                        move_first_window_to_bounds(&new_bounds);
                        logging::log("HOTKEY", "Window repositioned to mouse display");
                        
                        // Step 3: NOW activate the app (makes window visible at new position)
                        cx.activate(true);
                        logging::log("HOTKEY", "App activated (window now visible)");
                        
                        // Step 3.5: Configure as floating panel on first show only
                        if !PANEL_CONFIGURED.swap(true, Ordering::SeqCst) {
                            configure_as_floating_panel();
                            logging::log("HOTKEY", "Configured window as floating panel (first show)");
                        }
                        
                        // Step 4: Activate the specific window and focus it
                        let _ = window_clone.update(cx, |view: &mut ScriptListApp, win: &mut Window, cx: &mut Context<ScriptListApp>| {
                            win.activate_window();
                            let focus_handle = view.focus_handle(cx);
                            win.focus(&focus_handle, cx);
                            logging::log("HOTKEY", "Window activated and focused");
                            
                            // Step 5: Check if we need to reset to script list (after script completion)
                            // Reset debounce timer to allow immediate resize after window move
                            reset_resize_debounce();
                            
                            if NEEDS_RESET.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                                logging::log("VISIBILITY", "NEEDS_RESET was true - clearing and resetting to script list");
                                view.reset_to_script_list(cx);
                            } else {
                                // Even without reset, ensure window is properly sized for current content
                                view.update_window_size();
                            }
                        });
                        
                        logging::log("VISIBILITY", "Window show sequence complete");
                    });
                }
                
                let final_visible = WINDOW_VISIBLE.load(Ordering::SeqCst);
                let final_reset = NEEDS_RESET.load(Ordering::SeqCst);
                logging::log("VISIBILITY", &format!("Final state: WINDOW_VISIBLE={}, NEEDS_RESET={}", final_visible, final_reset));
                logging::log("VISIBILITY", "═══════════════════════════════════════════════════════════════");
            }
            
            logging::log("HOTKEY", "Hotkey listener exiting (channel closed)");
        }).detach();
    }
}

/// Error notification to display to the user
#[derive(Debug, Clone)]
struct ErrorNotification {
    /// The error message to display
    message: String,
    /// Severity level (affects styling)
    severity: ErrorSeverity,
    /// Timestamp when the notification was created (for auto-dismiss)
    #[allow(dead_code)]
    created_at: std::time::Instant,
}

struct ScriptListApp {
    scripts: Vec<scripts::Script>,
    scriptlets: Vec<scripts::Scriptlet>,
    builtin_entries: Vec<builtins::BuiltInEntry>,
    /// Cached list of installed applications for main search
    apps: Vec<app_launcher::AppInfo>,
    selected_index: usize,
    filter_text: String,
    last_output: Option<SharedString>,
    focus_handle: FocusHandle,
    show_logs: bool,
    theme: theme::Theme,
    #[allow(dead_code)]
    config: config::Config,
    // Interactive script state
    current_view: AppView,
    script_session: SharedSession,
    // Prompt-specific state (used when view is ArgPrompt or DivPrompt)
    arg_input_text: String,
    arg_selected_index: usize,
    // Channel for receiving prompt messages from script thread (async_channel for event-driven)
    prompt_receiver: Option<async_channel::Receiver<PromptMessage>>,
    // Channel for sending responses back to script
    response_sender: Option<mpsc::Sender<Message>>,
    // Scroll handle for uniform_list (automatic virtualized scrolling)
    list_scroll_handle: UniformListScrollHandle,
    // P0: Scroll handle for virtualized arg prompt choices
    arg_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for clipboard history list
    clipboard_list_scroll_handle: UniformListScrollHandle,
    // Scroll handle for window switcher list
    window_list_scroll_handle: UniformListScrollHandle,
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
    // Scroll stabilization: track last scrolled-to index to avoid redundant scroll_to_item calls
    last_scrolled_index: Option<usize>,
    // Preview cache: avoid re-reading file and re-highlighting on every render
    preview_cache_path: Option<String>,
    preview_cache_lines: Vec<syntax::HighlightedLine>,
    // Error notification for user-friendly error feedback
    error_notification: Option<ErrorNotification>,
    // Current design variant for hot-swappable UI designs
    current_design: DesignVariant,
    // Toast manager for notification queue
    toast_manager: ToastManager,
    // Cache for decoded clipboard images (entry_id -> RenderImage)
    clipboard_image_cache: std::collections::HashMap<String, Arc<gpui::RenderImage>>,
}

impl ScriptListApp {
    fn new(cx: &mut Context<Self>) -> Self {
        // PERF: Measure script loading time
        let load_start = std::time::Instant::now();
        let scripts = scripts::read_scripts();
        let scripts_elapsed = load_start.elapsed();
        
        let scriptlets_start = std::time::Instant::now();
        let scriptlets = scripts::read_scriptlets();
        let scriptlets_elapsed = scriptlets_start.elapsed();
        
        let theme = theme::load_theme();
        let config = config::load_config();
        
        // Load built-in entries based on config
        let builtin_entries = builtins::get_builtin_entries(&config.get_builtins());
        
        // Apps are loaded in the background to avoid blocking startup
        // Start with empty list, will be populated asynchronously
        let apps = Vec::new();
        
        let total_elapsed = load_start.elapsed();
        logging::log("PERF", &format!(
            "Startup loading: {:.2}ms total ({} scripts in {:.2}ms, {} scriptlets in {:.2}ms, apps loading in background)",
            total_elapsed.as_secs_f64() * 1000.0,
            scripts.len(),
            scripts_elapsed.as_secs_f64() * 1000.0,
            scriptlets.len(),
            scriptlets_elapsed.as_secs_f64() * 1000.0
        ));
        logging::log("APP", &format!("Loaded {} scripts from ~/.kenv/scripts", scripts.len()));
        logging::log("APP", &format!("Loaded {} scriptlets from ~/.kenv/scriptlets/scriptlets.md", scriptlets.len()));
        logging::log("APP", &format!("Loaded {} built-in features", builtin_entries.len()));
        logging::log("APP", "Applications loading in background...");
        logging::log("APP", "Loaded theme with system appearance detection");
        logging::log("APP", &format!("Loaded config: hotkey={:?}+{}, bun_path={:?}", 
            config.hotkey.modifiers, config.hotkey.key, config.bun_path));
        
        // Load apps in background thread to avoid blocking startup
        let app_launcher_enabled = config.get_builtins().app_launcher;
        if app_launcher_enabled {
            // Use a channel to send loaded apps back to main thread
            let (tx, rx) = std::sync::mpsc::channel::<(Vec<app_launcher::AppInfo>, std::time::Duration)>();
            
            // Spawn background thread for app scanning
            std::thread::spawn(move || {
                let start = std::time::Instant::now();
                let apps = app_launcher::scan_applications().clone();
                let elapsed = start.elapsed();
                let _ = tx.send((apps, elapsed));
            });
            
            // Poll for results using a spawned task
            cx.spawn(async move |this, cx| {
                // Poll the channel periodically
                loop {
                    Timer::after(std::time::Duration::from_millis(50)).await;
                    match rx.try_recv() {
                        Ok((apps, elapsed)) => {
                            let app_count = apps.len();
                            let _ = cx.update(|cx| {
                                this.update(cx, |app, cx| {
                                    app.apps = apps;
                                    // Invalidate filter cache since apps changed
                                    app.filter_cache_key = String::from("\0_APPS_LOADED_\0");
                                    logging::log("APP", &format!(
                                        "Background app loading complete: {} apps in {:.2}ms",
                                        app_count,
                                        elapsed.as_secs_f64() * 1000.0
                                    ));
                                    cx.notify();
                                })
                            });
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => continue,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    }
                }
            }).detach();
        }
        logging::log("UI", "Script Kit logo SVG loaded for header rendering");
        
        // Start cursor blink timer - updates all inputs that track cursor visibility
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(530)).await;
                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        // Skip cursor blink when window is hidden or no input is focused
                        if !WINDOW_VISIBLE.load(Ordering::SeqCst) || app.focused_input == FocusedInput::None {
                            return;
                        }
                        
                        app.cursor_visible = !app.cursor_visible;
                        // Also update ActionsDialog cursor if it exists
                        if let Some(ref dialog) = app.actions_dialog {
                            dialog.update(cx, |d, _cx| {
                                d.set_cursor_visible(app.cursor_visible);
                            });
                        }
                        cx.notify();
                    })
                });
            }
        }).detach();
        
        ScriptListApp {
            scripts,
            scriptlets,
            builtin_entries,
            apps,
            selected_index: 0,
            filter_text: String::new(),
            last_output: None,
            focus_handle: cx.focus_handle(),
            show_logs: false,
            theme,
            config,
            current_view: AppView::ScriptList,
            script_session: Arc::new(Mutex::new(None)),
            arg_input_text: String::new(),
            arg_selected_index: 0,
            prompt_receiver: None,
            response_sender: None,
            list_scroll_handle: UniformListScrollHandle::new(),
            arg_list_scroll_handle: UniformListScrollHandle::new(),
            clipboard_list_scroll_handle: UniformListScrollHandle::new(),
            window_list_scroll_handle: UniformListScrollHandle::new(),
            show_actions_popup: false,
            actions_dialog: None,
            cursor_visible: true,
            focused_input: FocusedInput::MainFilter,
            current_script_pid: None,
            // P1: Initialize filter cache
            cached_filtered_results: Vec::new(),
            filter_cache_key: String::from("\0_UNINITIALIZED_\0"), // Sentinel value to force initial compute
            // Scroll stabilization: start with no last scrolled index
            last_scrolled_index: None,
            // Preview cache: start empty, will populate on first render
            preview_cache_path: None,
            preview_cache_lines: Vec::new(),
            // Error notification: start with none
            error_notification: None,
            // Design system: start with default design
            current_design: DesignVariant::default(),
            // Toast manager: initialize for error notifications
            toast_manager: ToastManager::new(),
            // Clipboard image cache: decoded RenderImages for thumbnails/preview
            clipboard_image_cache: std::collections::HashMap::new(),
        }
    }
    
    /// Switch to a different design variant
    /// 
    /// Cycle to the next design variant.
    /// Use Cmd+1 to cycle through all designs.
    fn cycle_design(&mut self, cx: &mut Context<Self>) {
        let old_design = self.current_design;
        let new_design = old_design.next();
        let all_designs = DesignVariant::all();
        let old_idx = all_designs.iter().position(|&v| v == old_design).unwrap_or(0);
        let new_idx = all_designs.iter().position(|&v| v == new_design).unwrap_or(0);
        
        logging::log("DESIGN", &format!(
            "Cycling design: {} ({}) -> {} ({}) [total: {}]",
            old_design.name(),
            old_idx,
            new_design.name(),
            new_idx,
            all_designs.len()
        ));
        logging::log("DESIGN", &format!(
            "Design '{}': {}",
            new_design.name(),
            new_design.description()
        ));
        
        self.current_design = new_design;
        logging::log("DESIGN", &format!(
            "self.current_design is now: {:?}",
            self.current_design
        ));
        cx.notify();
    }
    
    /// Show an error notification to the user
    /// 
    /// The notification will auto-dismiss after 5 seconds.
    /// Call this when an operation fails and you want to inform the user.
    #[allow(dead_code)]
    fn show_error(&mut self, message: String, severity: ErrorSeverity, cx: &mut Context<Self>) {
        logging::log("ERROR", &format!("Showing error notification: {} (severity: {:?})", message, severity));
        
        self.error_notification = Some(ErrorNotification {
            message,
            severity,
            created_at: std::time::Instant::now(),
        });
        
        cx.notify();
        
        // Set up auto-dismiss timer (5 seconds)
        cx.spawn(async move |this, cx| {
            Timer::after(std::time::Duration::from_secs(5)).await;
            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    app.clear_error(cx);
                })
            });
        }).detach();
    }
    
    /// Clear the current error notification
    fn clear_error(&mut self, cx: &mut Context<Self>) {
        if self.error_notification.is_some() {
            logging::log("ERROR", "Clearing error notification");
            self.error_notification = None;
            cx.notify();
        }
    }
    
    fn update_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = theme::load_theme();
        logging::log("APP", "Theme reloaded based on system appearance");
        cx.notify();
    }
    
    fn update_config(&mut self, cx: &mut Context<Self>) {
        self.config = config::load_config();
        logging::log("APP", &format!("Config reloaded: padding={:?}", self.config.get_padding()));
        cx.notify();
    }
    
    fn refresh_scripts(&mut self, cx: &mut Context<Self>) {
        self.scripts = scripts::read_scripts();
        self.scriptlets = scripts::read_scriptlets();
        self.selected_index = 0;
        self.last_scrolled_index = None;
        self.list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
        self.last_scrolled_index = Some(0);
        self.invalidate_filter_cache();
        logging::log("APP", &format!("Scripts refreshed: {} scripts, {} scriptlets loaded", self.scripts.len(), self.scriptlets.len()));
        cx.notify();
    }

    /// Get unified filtered results combining scripts and scriptlets
    /// P1: Now uses caching - invalidates only when filter_text changes
    fn filtered_results(&self) -> Vec<scripts::SearchResult> {
        // P1: Return cached results if filter hasn't changed
        if self.filter_text == self.filter_cache_key {
            logging::log_debug("CACHE", &format!("Filter cache HIT for '{}'", self.filter_text));
            return self.cached_filtered_results.clone();
        }
        
        // P1: Cache miss - need to recompute (will be done by get_filtered_results_mut)
        logging::log_debug("CACHE", &format!("Filter cache MISS - need recompute for '{}' (cached key: '{}')", 
            self.filter_text, self.filter_cache_key));
        
        // PERF: Measure search time (only log when actually filtering)
        let search_start = std::time::Instant::now();
        let results = scripts::fuzzy_search_unified(&self.scripts, &self.scriptlets, &self.filter_text);
        let search_elapsed = search_start.elapsed();
        
        // Only log search performance when there's an active filter
        if !self.filter_text.is_empty() {
            logging::log("PERF", &format!(
                "Search '{}' took {:.2}ms ({} results from {} total)",
                self.filter_text,
                search_elapsed.as_secs_f64() * 1000.0,
                results.len(),
                self.scripts.len() + self.scriptlets.len()
            ));
        }
        results
    }
    
    /// P1: Get filtered results with cache update (mutable version)
    /// Call this when you need to ensure cache is updated
    fn get_filtered_results_cached(&mut self) -> &Vec<scripts::SearchResult> {
        if self.filter_text != self.filter_cache_key {
            logging::log_debug("CACHE", &format!("Filter cache MISS - recomputing for '{}'", self.filter_text));
            let search_start = std::time::Instant::now();
            self.cached_filtered_results = scripts::fuzzy_search_unified_all(
                &self.scripts, 
                &self.scriptlets, 
                &self.builtin_entries,
                &self.apps,
                &self.filter_text
            );
            self.filter_cache_key = self.filter_text.clone();
            let search_elapsed = search_start.elapsed();
            
            if !self.filter_text.is_empty() {
                logging::log("PERF", &format!(
                    "Search '{}' took {:.2}ms ({} results from {} total)",
                    self.filter_text,
                    search_elapsed.as_secs_f64() * 1000.0,
                    self.cached_filtered_results.len(),
                    self.scripts.len() + self.scriptlets.len() + self.builtin_entries.len() + self.apps.len()
                ));
            }
        } else {
            logging::log_debug("CACHE", &format!("Filter cache HIT for '{}'", self.filter_text));
        }
        &self.cached_filtered_results
    }
    
    /// P1: Invalidate filter cache (call when scripts/scriptlets change)
    #[allow(dead_code)]
    fn invalidate_filter_cache(&mut self) {
        logging::log_debug("CACHE", "Filter cache INVALIDATED");
        self.filter_cache_key = String::from("\0_INVALIDATED_\0");
    }
    
    /// Get or update the preview cache for syntax-highlighted code lines.
    /// Only re-reads and re-highlights when the script path actually changes.
    /// Returns cached lines if path matches, otherwise updates cache and returns new lines.
    fn get_or_update_preview_cache(&mut self, script_path: &str, lang: &str) -> &[syntax::HighlightedLine] {
        // Check if cache is valid for this path
        if self.preview_cache_path.as_deref() == Some(script_path) && !self.preview_cache_lines.is_empty() {
            logging::log_debug("CACHE", &format!("Preview cache HIT for '{}'", script_path));
            return &self.preview_cache_lines;
        }
        
        // Cache miss - need to re-read and re-highlight
        logging::log_debug("CACHE", &format!("Preview cache MISS - loading '{}'", script_path));
        
        self.preview_cache_path = Some(script_path.to_string());
        self.preview_cache_lines = match std::fs::read_to_string(script_path) {
            Ok(content) => {
                // Only take first 15 lines for preview
                let preview: String = content
                    .lines()
                    .take(15)
                    .collect::<Vec<_>>()
                    .join("\n");
                syntax::highlight_code_lines(&preview, lang)
            }
            Err(e) => {
                logging::log("ERROR", &format!("Failed to read preview: {}", e));
                Vec::new()
            }
        };
        
        &self.preview_cache_lines
    }
    
    /// Invalidate the preview cache (call when selection might change to different script)
    #[allow(dead_code)]
    fn invalidate_preview_cache(&mut self) {
        self.preview_cache_path = None;
        self.preview_cache_lines.clear();
    }

    #[allow(dead_code)]
    fn filtered_scripts(&self) -> Vec<scripts::Script> {
        if self.filter_text.is_empty() {
            self.scripts.clone()
        } else {
            let filter_lower = self.filter_text.to_lowercase();
            self.scripts.iter()
                .filter(|s| s.name.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect()
        }
    }

    fn move_selection_up(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.scroll_to_selected_if_needed("keyboard_up");
            cx.notify();
        }
    }

    fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        let filtered_len = self.filtered_results().len();
        if self.selected_index < filtered_len.saturating_sub(1) {
            self.selected_index += 1;
            self.scroll_to_selected_if_needed("keyboard_down");
            cx.notify();
        }
    }
    
    /// Scroll stabilization helper: only call scroll_to_item if we haven't already scrolled to this index.
    /// This prevents scroll jitter from redundant scroll_to_item calls.
    fn scroll_to_selected_if_needed(&mut self, _reason: &str) {
        let target = self.selected_index;
        
        // Check if we've already scrolled to this index
        if self.last_scrolled_index == Some(target) {
            return;
        }
        
        // Perform the scroll (logging removed for performance)
        self.list_scroll_handle.scroll_to_item(target, ScrollStrategy::Nearest);
        self.last_scrolled_index = Some(target);
    }
    
    /// Update selected index from mouse hover and scroll if needed
    fn set_selected_index_from_hover(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.selected_index != index {
            self.selected_index = index;
            cx.notify();
        }
    }

    fn execute_selected(&mut self, cx: &mut Context<Self>) {
        let filtered = self.filtered_results();
        if let Some(result) = filtered.get(self.selected_index).cloned() {
            match result {
                scripts::SearchResult::Script(script_match) => {
                    logging::log("EXEC", &format!("Executing script: {}", script_match.script.name));
                    self.execute_interactive(&script_match.script, cx);
                }
                scripts::SearchResult::Scriptlet(scriptlet_match) => {
                    logging::log("EXEC", &format!("Executing scriptlet: {}", scriptlet_match.scriptlet.name));
                    self.execute_scriptlet(&scriptlet_match.scriptlet, cx);
                }
                scripts::SearchResult::BuiltIn(builtin_match) => {
                    logging::log("EXEC", &format!("Executing built-in: {}", builtin_match.entry.name));
                    self.execute_builtin(&builtin_match.entry, cx);
                }
                scripts::SearchResult::App(app_match) => {
                    logging::log("EXEC", &format!("Launching app: {}", app_match.app.name));
                    self.execute_app(&app_match.app, cx);
                }
                scripts::SearchResult::Window(window_match) => {
                    logging::log("EXEC", &format!("Focusing window: {}", window_match.window.title));
                    self.execute_window_focus(&window_match.window, cx);
                }
            }
        }
    }

    fn update_filter(&mut self, new_char: Option<char>, backspace: bool, clear: bool, cx: &mut Context<Self>) {
        if clear {
            self.filter_text.clear();
            self.selected_index = 0;
            self.last_scrolled_index = None;
            self.list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
            self.last_scrolled_index = Some(0);
        } else if backspace && !self.filter_text.is_empty() {
            self.filter_text.pop();
            self.selected_index = 0;
            self.last_scrolled_index = None;
            self.list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
            self.last_scrolled_index = Some(0);
        } else if let Some(ch) = new_char {
            self.filter_text.push(ch);
            self.selected_index = 0;
            self.last_scrolled_index = None;
            self.list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
            self.last_scrolled_index = Some(0);
        }
        
        // Trigger window resize based on new filter results
        self.update_window_size();
        
        cx.notify();
    }
    
    fn toggle_logs(&mut self, cx: &mut Context<Self>) {
        self.show_logs = !self.show_logs;
        cx.notify();
    }
    
    /// Update window size based on current view and item count.
    /// This implements dynamic window resizing:
    /// - Script list: resize based on filtered results
    /// - Arg prompt: resize based on filtered choices
    /// - Div/Editor/Term: use full height
    fn update_window_size(&self) {
        let (view_type, item_count) = match &self.current_view {
            AppView::ScriptList => {
                let count = self.filtered_results().len();
                (ViewType::ScriptList, count)
            }
            AppView::ArgPrompt { choices, .. } => {
                let filtered = self.get_filtered_arg_choices(choices);
                if filtered.is_empty() && choices.is_empty() {
                    (ViewType::ArgPromptNoChoices, 0)
                } else {
                    (ViewType::ArgPromptWithChoices, filtered.len())
                }
            }
            AppView::DivPrompt { .. } => (ViewType::DivPrompt, 0),
            AppView::FormPrompt { .. } => (ViewType::DivPrompt, 0), // Use DivPrompt size for forms
            AppView::EditorPrompt { .. } => (ViewType::EditorPrompt, 0),
            AppView::TermPrompt { .. } => (ViewType::TermPrompt, 0),
            AppView::ActionsDialog => {
                // Actions dialog is an overlay, don't resize
                return;
            }
            // Clipboard history and app launcher use standard height (same as script list)
            AppView::ClipboardHistoryView { entries, filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    entries.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    entries.iter().filter(|e| e.content.to_lowercase().contains(&filter_lower)).count()
                };
                (ViewType::ScriptList, filtered_count)
            }
            AppView::AppLauncherView { apps, filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    apps.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    apps.iter().filter(|a| a.name.to_lowercase().contains(&filter_lower)).count()
                };
                (ViewType::ScriptList, filtered_count)
            }
            AppView::WindowSwitcherView { windows, filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    windows.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    windows.iter().filter(|w| {
                        w.title.to_lowercase().contains(&filter_lower) || 
                        w.app.to_lowercase().contains(&filter_lower)
                    }).count()
                };
                (ViewType::ScriptList, filtered_count)
            }
        };
        
        let target_height = height_for_view(view_type, item_count);
        resize_first_window_to_height(target_height);
    }
    
    /// Helper to get filtered arg choices without cloning
    fn get_filtered_arg_choices<'a>(&self, choices: &'a [Choice]) -> Vec<&'a Choice> {
        if self.arg_input_text.is_empty() {
            choices.iter().collect()
        } else {
            let filter = self.arg_input_text.to_lowercase();
            choices
                .iter()
                .filter(|c| c.name.to_lowercase().contains(&filter))
                .collect()
        }
    }
    
    fn toggle_actions(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        logging::log("KEY", "Toggling actions popup");
        if self.show_actions_popup {
            // Close - return focus to main filter
            self.show_actions_popup = false;
            self.actions_dialog = None;
            self.focused_input = FocusedInput::MainFilter;
            window.focus(&self.focus_handle, cx);
            logging::log("FOCUS", "Actions closed, focus returned to MainFilter");
        } else {
            // Open - create dialog entity
            self.show_actions_popup = true;
            self.focused_input = FocusedInput::ActionsSearch;
            let script_info = self.get_focused_script_info();
            
            let theme_arc = std::sync::Arc::new(self.theme.clone());
            let dialog = cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                ActionsDialog::with_script(
                    focus_handle,
                    std::sync::Arc::new(|_action_id| {}), // Callback handled separately
                    script_info,
                    theme_arc,
                )
            });
            
            // Hide the dialog's built-in search input since header already has search
            dialog.update(cx, |d, _| d.set_hide_search(true));
            
            // Focus the dialog's internal focus handle
            let dialog_focus_handle = dialog.read(cx).focus_handle.clone();
            self.actions_dialog = Some(dialog.clone());
            window.focus(&dialog_focus_handle, cx);
            logging::log("FOCUS", "Actions opened, focus moved to ActionsSearch");
        }
        cx.notify();
    }
    
    /// Handle action selection from the actions dialog
    fn handle_action(&mut self, action_id: String, cx: &mut Context<Self>) {
        logging::log("UI", &format!("Action selected: {}", action_id));
        
        // Close the dialog and return to script list
        self.current_view = AppView::ScriptList;
        
        match action_id.as_str() {
            "create_script" => {
                logging::log("UI", "Create script action - opening dialog");
                // TODO: Implement create script dialog
                self.last_output = Some(SharedString::from("Create script action (TODO)"));
            }
            "edit_script" => {
                logging::log("UI", "Edit script action");
                let filtered = self.filtered_results();
                if let Some(result) = filtered.get(self.selected_index) {
                    match result {
                        scripts::SearchResult::Script(script_match) => {
                            self.edit_script(&script_match.script.path);
                        }
                        scripts::SearchResult::Scriptlet(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit scriptlets"));
                        }
                        scripts::SearchResult::BuiltIn(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit built-in features"));
                        }
                        scripts::SearchResult::App(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit applications"));
                        }
                        scripts::SearchResult::Window(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit windows"));
                        }
                    }
                } else {
                    self.last_output = Some(SharedString::from("No script selected"));
                }
            }
            "reload_scripts" => {
                logging::log("UI", "Reload scripts action");
                self.refresh_scripts(cx);
                self.last_output = Some(SharedString::from("Scripts reloaded"));
            }
            "settings" => {
                logging::log("UI", "Settings action");
                self.last_output = Some(SharedString::from("Settings (TODO)"));
            }
            "quit" => {
                logging::log("UI", "Quit action");
                cx.quit();
            }
            "__cancel__" => {
                logging::log("UI", "Actions dialog cancelled");
            }
            _ => {
                logging::log("UI", &format!("Unknown action: {}", action_id));
            }
        }
        
        cx.notify();
    }
    
    /// Edit a script in configured editor (config.editor > $EDITOR > "code")
    #[allow(dead_code)]
    fn edit_script(&mut self, path: &std::path::Path) {
        let editor = self.config.get_editor();
        logging::log("UI", &format!("Opening script in editor '{}': {}", editor, path.display()));
        let path_str = path.to_string_lossy().to_string();
        
        std::thread::spawn(move || {
            use std::process::Command;
            match Command::new(&editor)
                .arg(&path_str)
                .spawn()
            {
                Ok(_) => logging::log("UI", &format!("Successfully spawned editor: {}", editor)),
                Err(e) => logging::log("ERROR", &format!("Failed to spawn editor '{}': {}", editor, e)),
            }
        });
    }
    
    /// Execute a script interactively (for scripts that use arg/div prompts)
    fn execute_interactive(&mut self, script: &scripts::Script, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Starting interactive execution: {}", script.name));
        
        // Store script path for error reporting in reader thread
        let script_path_for_errors = script.path.to_string_lossy().to_string();
        
        match executor::execute_script_interactive(&script.path) {
            Ok(session) => {
                logging::log("EXEC", "Interactive session started successfully");
                
                // Store PID for explicit cleanup (belt-and-suspenders approach)
                let pid = session.pid();
                self.current_script_pid = Some(pid);
                logging::log("EXEC", &format!("Stored script PID {} for cleanup", pid));
                
                *self.script_session.lock().unwrap() = Some(session);
                
                // Create async_channel for script thread to send prompt messages to UI (event-driven)
                let (tx, rx) = async_channel::unbounded();
                let rx_for_listener = rx.clone();
                self.prompt_receiver = Some(rx);
                
                // Spawn event-driven listener for prompt messages (replaces 50ms polling)
                cx.spawn(async move |this, cx| {
                    logging::log("EXEC", "Prompt message listener started (event-driven)");
                    
                    // Event-driven: recv().await yields until a message arrives
                    while let Ok(msg) = rx_for_listener.recv().await {
                        logging::log("EXEC", &format!("Prompt message received: {:?}", msg));
                        let _ = cx.update(|cx| {
                            this.update(cx, |app, cx| {
                                app.handle_prompt_message(msg, cx);
                            })
                        });
                    }
                    
                    logging::log("EXEC", "Prompt message listener exiting (channel closed)");
                }).detach();
                
                // We need separate threads for reading and writing to avoid deadlock
                // The read thread blocks on receive_message(), so we can't check for responses in the same loop
                
                // Take ownership of the session and split it
                let session = self.script_session.lock().unwrap().take().unwrap();
                let split = session.split();
                
                let mut stdin = split.stdin;
                let mut stdout_reader = split.stdout_reader;
                // Capture stderr for error reporting
                let stderr_handle = split.stderr;
                // CRITICAL: Keep process_handle and child alive - they kill the process on drop!
                // We move them into the reader thread so they live until the script exits.
                let _process_handle = split.process_handle;
                let mut _child = split.child;
                
                // Channel for sending responses from UI to writer thread
                let (response_tx, response_rx) = mpsc::channel::<Message>();
                
                // Clone response_tx for the reader thread to handle direct responses
                // (e.g., getSelectedText, setSelectedText, checkAccessibility)
                let reader_response_tx = response_tx.clone();
                
                // Writer thread - handles sending responses to script
                std::thread::spawn(move || {
                    use std::io::Write;
                    loop {
                        match response_rx.recv() {
                            Ok(response) => {
                                let json = match protocol::serialize_message(&response) {
                                    Ok(j) => j,
                                    Err(e) => {
                                        logging::log("EXEC", &format!("Failed to serialize: {}", e));
                                        continue;
                                    }
                                };
                                logging::log("EXEC", &format!("Sending to script: {}", json));
                                if let Err(e) = writeln!(stdin, "{}", json) {
                                    logging::log("EXEC", &format!("Failed to write: {}", e));
                                    break;
                                }
                                if let Err(e) = stdin.flush() {
                                    logging::log("EXEC", &format!("Failed to flush: {}", e));
                                    break;
                                }
                                logging::log("EXEC", "Response sent to script");
                            }
                            Err(_) => {
                                logging::log("EXEC", "Response channel closed, writer exiting");
                                break;
                            }
                        }
                    }
                });
                
                // Reader thread - handles receiving messages from script (blocking is OK here)
                // CRITICAL: Move _process_handle and _child into this thread to keep them alive!
                // When the reader thread exits, they'll be dropped and the process killed.
                let script_path_clone = script_path_for_errors.clone();
                std::thread::spawn(move || {
                    // These variables keep the process alive - they're dropped when the thread exits
                    let _keep_alive_handle = _process_handle;
                    let mut keep_alive_child = _child;
                    let mut stderr_for_errors = stderr_handle;
                    let script_path = script_path_clone;
                    
                    loop {
                        // Use next_message_graceful to skip non-JSON lines (e.g., console.log output)
                        match stdout_reader.next_message_graceful() {
                            Ok(Some(msg)) => {
                                logging::log("EXEC", &format!("Received message: {:?}", msg));
                                
                                // First, try to handle selected text messages directly (no UI needed)
                                match executor::handle_selected_text_message(&msg) {
                                    executor::SelectedTextHandleResult::Handled(response) => {
                                        logging::log("EXEC", &format!("Handled selected text message, sending response: {:?}", response));
                                        if let Err(e) = reader_response_tx.send(response) {
                                            logging::log("EXEC", &format!("Failed to send selected text response: {}", e));
                                        }
                                        continue;
                                    }
                                    executor::SelectedTextHandleResult::NotHandled => {
                                        // Fall through to other message handling
                                    }
                                }
                                
                                // Handle ClipboardHistory directly (no UI needed)
                                if let Message::ClipboardHistory { request_id, action, entry_id } = &msg {
                                    logging::log("EXEC", &format!("ClipboardHistory request: {:?}", action));
                                    
                                    let response = match action {
                                        protocol::ClipboardHistoryAction::List => {
                                            let entries = clipboard_history::get_clipboard_history(100);
                                            let entry_data: Vec<protocol::ClipboardHistoryEntryData> = entries
                                                .into_iter()
                                                .map(|e| {
                                                    // Truncate large content to avoid pipe buffer issues
                                                    // Images are stored as base64 which can be huge
                                                    let content = match e.content_type {
                                                        clipboard_history::ContentType::Image => {
                                                            // For images, send a placeholder with metadata
                                                            format!("[image:{}]", e.id)
                                                        }
                                                        clipboard_history::ContentType::Text => {
                                                            // Truncate very long text entries
                                                            if e.content.len() > 1000 {
                                                                format!("{}...", &e.content[..1000])
                                                            } else {
                                                                e.content
                                                            }
                                                        }
                                                    };
                                                    protocol::ClipboardHistoryEntryData {
                                                        entry_id: e.id,
                                                        content,
                                                        content_type: match e.content_type {
                                                            clipboard_history::ContentType::Text => protocol::ClipboardEntryType::Text,
                                                            clipboard_history::ContentType::Image => protocol::ClipboardEntryType::Image,
                                                        },
                                                        timestamp: chrono::DateTime::from_timestamp(e.timestamp, 0)
                                                            .map(|dt| dt.to_rfc3339())
                                                            .unwrap_or_default(),
                                                        pinned: e.pinned,
                                                    }
                                                })
                                                .collect();
                                            Message::clipboard_history_list_response(request_id.clone(), entry_data)
                                        }
                                        protocol::ClipboardHistoryAction::Pin => {
                                            if let Some(id) = entry_id {
                                                match clipboard_history::pin_entry(id) {
                                                    Ok(()) => Message::clipboard_history_success(request_id.clone()),
                                                    Err(e) => Message::clipboard_history_error(request_id.clone(), e.to_string()),
                                                }
                                            } else {
                                                Message::clipboard_history_error(request_id.clone(), "Missing entry_id".to_string())
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::Unpin => {
                                            if let Some(id) = entry_id {
                                                match clipboard_history::unpin_entry(id) {
                                                    Ok(()) => Message::clipboard_history_success(request_id.clone()),
                                                    Err(e) => Message::clipboard_history_error(request_id.clone(), e.to_string()),
                                                }
                                            } else {
                                                Message::clipboard_history_error(request_id.clone(), "Missing entry_id".to_string())
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::Remove => {
                                            if let Some(id) = entry_id {
                                                match clipboard_history::remove_entry(id) {
                                                    Ok(()) => Message::clipboard_history_success(request_id.clone()),
                                                    Err(e) => Message::clipboard_history_error(request_id.clone(), e.to_string()),
                                                }
                                            } else {
                                                Message::clipboard_history_error(request_id.clone(), "Missing entry_id".to_string())
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::Clear => {
                                            match clipboard_history::clear_history() {
                                                Ok(()) => Message::clipboard_history_success(request_id.clone()),
                                                Err(e) => Message::clipboard_history_error(request_id.clone(), e.to_string()),
                                            }
                                        }
                                    };
                                    
                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log("EXEC", &format!("Failed to send clipboard history response: {}", e));
                                    }
                                    continue;
                                }
                                
                                // Handle Clipboard read/write directly (no UI needed)
                                if let Message::Clipboard { id, action, format, content } = &msg {
                                    logging::log("EXEC", &format!("Clipboard request: {:?} format: {:?}", action, format));
                                    
                                    // If no request ID, we can't send a response, so just handle and continue
                                    let req_id = match id {
                                        Some(rid) => rid.clone(),
                                        None => {
                                            // Handle clipboard operation without response
                                            if let protocol::ClipboardAction::Write = action {
                                                if let Some(text) = content {
                                                    use arboard::Clipboard;
                                                    if let Ok(mut clipboard) = Clipboard::new() {
                                                        let _ = clipboard.set_text(text.clone());
                                                    }
                                                }
                                            }
                                            continue;
                                        }
                                    };
                                    
                                    let response = match action {
                                        protocol::ClipboardAction::Read => {
                                            // Read from clipboard
                                            match format {
                                                Some(protocol::ClipboardFormat::Text) | None => {
                                                    use arboard::Clipboard;
                                                    match Clipboard::new() {
                                                        Ok(mut clipboard) => {
                                                            match clipboard.get_text() {
                                                                Ok(text) => {
                                                                    Message::Submit { id: req_id, value: Some(text) }
                                                                }
                                                                Err(e) => {
                                                                    logging::log("EXEC", &format!("Clipboard read error: {}", e));
                                                                    Message::Submit { id: req_id, value: Some(String::new()) }
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            logging::log("EXEC", &format!("Clipboard init error: {}", e));
                                                            Message::Submit { id: req_id, value: Some(String::new()) }
                                                        }
                                                    }
                                                }
                                                Some(protocol::ClipboardFormat::Image) => {
                                                    use arboard::Clipboard;
                                                    match Clipboard::new() {
                                                        Ok(mut clipboard) => {
                                                            match clipboard.get_image() {
                                                                Ok(img) => {
                                                                    // Convert image to base64
                                                                    use base64::Engine;
                                                                    let bytes = img.bytes.to_vec();
                                                                    let base64_str = base64::engine::general_purpose::STANDARD.encode(&bytes);
                                                                    Message::Submit { id: req_id, value: Some(base64_str) }
                                                                }
                                                                Err(e) => {
                                                                    logging::log("EXEC", &format!("Clipboard read image error: {}", e));
                                                                    Message::Submit { id: req_id, value: Some(String::new()) }
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            logging::log("EXEC", &format!("Clipboard init error: {}", e));
                                                            Message::Submit { id: req_id, value: Some(String::new()) }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        protocol::ClipboardAction::Write => {
                                            // Write to clipboard
                                            use arboard::Clipboard;
                                            match Clipboard::new() {
                                                Ok(mut clipboard) => {
                                                    if let Some(text) = content {
                                                        match clipboard.set_text(text.clone()) {
                                                            Ok(()) => {
                                                                logging::log("EXEC", &format!("Clipboard write success: {} bytes", text.len()));
                                                                Message::Submit { id: req_id, value: Some("ok".to_string()) }
                                                            }
                                                            Err(e) => {
                                                                logging::log("EXEC", &format!("Clipboard write error: {}", e));
                                                                Message::Submit { id: req_id, value: Some(String::new()) }
                                                            }
                                                        }
                                                    } else {
                                                        logging::log("EXEC", "Clipboard write: no content provided");
                                                        Message::Submit { id: req_id, value: Some(String::new()) }
                                                    }
                                                }
                                                Err(e) => {
                                                    logging::log("EXEC", &format!("Clipboard init error: {}", e));
                                                    Message::Submit { id: req_id, value: Some(String::new()) }
                                                }
                                            }
                                        }
                                    };
                                    
                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log("EXEC", &format!("Failed to send clipboard response: {}", e));
                                    }
                                    continue;
                                }
                                
                                // Handle WindowList directly (no UI needed)
                                if let Message::WindowList { request_id } = &msg {
                                    logging::log("EXEC", &format!("WindowList request: {}", request_id));
                                    
                                    let response = match window_control::list_windows() {
                                        Ok(windows) => {
                                            let window_infos: Vec<protocol::SystemWindowInfo> = windows
                                                .into_iter()
                                                .map(|w| protocol::SystemWindowInfo {
                                                    window_id: w.id,
                                                    title: w.title,
                                                    app_name: w.app,
                                                    bounds: Some(protocol::TargetWindowBounds {
                                                        x: w.bounds.x,
                                                        y: w.bounds.y,
                                                        width: w.bounds.width,
                                                        height: w.bounds.height,
                                                    }),
                                                    is_minimized: None,
                                                    is_active: None,
                                                })
                                                .collect();
                                            Message::window_list_result(request_id.clone(), window_infos)
                                        }
                                        Err(e) => {
                                            logging::log("EXEC", &format!("WindowList error: {}", e));
                                            // Return empty list on error
                                            Message::window_list_result(request_id.clone(), vec![])
                                        }
                                    };
                                    
                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log("EXEC", &format!("Failed to send window list response: {}", e));
                                    }
                                    continue;
                                }
                                
                                // Handle WindowAction directly (no UI needed)
                                if let Message::WindowAction { request_id, action, window_id, bounds } = &msg {
                                    logging::log("EXEC", &format!("WindowAction request: {:?} for window {:?}", action, window_id));
                                    
                                    let result = match action {
                                        protocol::WindowActionType::Focus => {
                                            if let Some(id) = window_id {
                                                window_control::focus_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Close => {
                                            if let Some(id) = window_id {
                                                window_control::close_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Minimize => {
                                            if let Some(id) = window_id {
                                                window_control::minimize_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Maximize => {
                                            if let Some(id) = window_id {
                                                window_control::maximize_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Resize => {
                                            if let (Some(id), Some(b)) = (window_id, bounds) {
                                                window_control::resize_window(*id, b.width, b.height)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id or bounds"))
                                            }
                                        }
                                        protocol::WindowActionType::Move => {
                                            if let (Some(id), Some(b)) = (window_id, bounds) {
                                                window_control::move_window(*id, b.x, b.y)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id or bounds"))
                                            }
                                        }
                                    };
                                    
                                    let response = match result {
                                        Ok(()) => Message::window_action_success(request_id.clone()),
                                        Err(e) => Message::window_action_error(request_id.clone(), e.to_string()),
                                    };
                                    
                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log("EXEC", &format!("Failed to send window action response: {}", e));
                                    }
                                    continue;
                                }
                                
                                // Handle FileSearch directly (no UI needed)
                                if let Message::FileSearch { request_id, query, only_in } = &msg {
                                    logging::log("EXEC", &format!("FileSearch request: query='{}', only_in={:?}", query, only_in));
                                    
                                    let results = file_search::search_files(query, only_in.as_deref(), file_search::DEFAULT_LIMIT);
                                    let file_entries: Vec<protocol::FileSearchResultEntry> = results
                                        .into_iter()
                                        .map(|f| protocol::FileSearchResultEntry {
                                            path: f.path,
                                            name: f.name,
                                            is_directory: f.file_type == file_search::FileType::Directory,
                                            size: Some(f.size),
                                            modified_at: chrono::DateTime::from_timestamp(f.modified as i64, 0)
                                                .map(|dt| dt.to_rfc3339()),
                                        })
                                        .collect();
                                    
                                    let response = Message::file_search_result(request_id.clone(), file_entries);
                                    
                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log("EXEC", &format!("Failed to send file search response: {}", e));
                                    }
                                    continue;
                                }
                                
                                // Handle GetWindowBounds directly (no UI needed)
                                if let Message::GetWindowBounds { request_id } = &msg {
                                    logging::log("EXEC", &format!("GetWindowBounds request: {}", request_id));
                                    
                                    #[cfg(target_os = "macos")]
                                    let bounds_json = {
                                        if let Some(window) = window_manager::get_main_window() {
                                            unsafe {
                                                // Get the window frame
                                                let frame: NSRect = msg_send![window, frame];
                                                
                                                // Get the PRIMARY screen's height for coordinate conversion
                                                // macOS uses bottom-left origin, we convert to top-left
                                                let screens: id = msg_send![class!(NSScreen), screens];
                                                let main_screen: id = msg_send![screens, firstObject];
                                                let main_screen_frame: NSRect = msg_send![main_screen, frame];
                                                let primary_screen_height = main_screen_frame.size.height;
                                                
                                                // Convert from bottom-left origin (macOS) to top-left origin
                                                let flipped_y = primary_screen_height - frame.origin.y - frame.size.height;
                                                
                                                logging::log("EXEC", &format!(
                                                    "Window bounds: x={:.0}, y={:.0}, width={:.0}, height={:.0}",
                                                    frame.origin.x, flipped_y, frame.size.width, frame.size.height
                                                ));
                                                
                                                // Create JSON string with bounds
                                                format!(
                                                    r#"{{"x":{},"y":{},"width":{},"height":{}}}"#,
                                                    frame.origin.x as f64,
                                                    flipped_y as f64,
                                                    frame.size.width as f64,
                                                    frame.size.height as f64
                                                )
                                            }
                                        } else {
                                            logging::log("EXEC", "GetWindowBounds: Main window not registered");
                                            r#"{"error":"Main window not found"}"#.to_string()
                                        }
                                    };
                                    
                                    #[cfg(not(target_os = "macos"))]
                                    let bounds_json = r#"{"error":"Not supported on this platform"}"#.to_string();
                                    
                                    let response = Message::Submit { 
                                        id: request_id.clone(), 
                                        value: Some(bounds_json) 
                                    };
                                    logging::log("EXEC", &format!("Sending window bounds response: {:?}", response));
                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log("EXEC", &format!("Failed to send window bounds response: {}", e));
                                    }
                                    continue;
                                }
                                
                                // Handle GetState - needs UI state, forward to UI thread
                                if let Message::GetState { request_id } = &msg {
                                    logging::log("EXEC", &format!("GetState request: {}", request_id));
                                    let prompt_msg = PromptMessage::GetState { request_id: request_id.clone() };
                                    if tx.send_blocking(prompt_msg).is_err() {
                                        logging::log("EXEC", "Prompt channel closed, reader exiting");
                                        break;
                                    }
                                    continue;
                                }
                                
                                // Handle CaptureScreenshot directly (no UI needed)
                                if let Message::CaptureScreenshot { request_id } = &msg {
                                    tracing::info!(request_id = %request_id, "Capturing screenshot");
                                    
                                    let response = match capture_app_screenshot() {
                                        Ok((png_data, width, height)) => {
                                            use base64::Engine;
                                            let base64_data = base64::engine::general_purpose::STANDARD.encode(&png_data);
                                            tracing::info!(
                                                request_id = %request_id,
                                                width = width,
                                                height = height,
                                                data_len = base64_data.len(),
                                                "Screenshot captured successfully"
                                            );
                                            Message::screenshot_result(
                                                request_id.clone(),
                                                base64_data,
                                                width,
                                                height,
                                            )
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                request_id = %request_id,
                                                error = %e,
                                                "Screenshot capture failed"
                                            );
                                            // Send empty result on error
                                            Message::screenshot_result(
                                                request_id.clone(),
                                                String::new(),
                                                0,
                                                0,
                                            )
                                        }
                                    };
                                    
                                    if let Err(e) = reader_response_tx.send(response) {
                                        tracing::error!(error = %e, "Failed to send screenshot response");
                                    }
                                    continue;
                                }
                                
                                let prompt_msg = match msg {
                                    Message::Arg { id, placeholder, choices } => {
                                        Some(PromptMessage::ShowArg { id, placeholder, choices })
                                    }
                                    Message::Div { id, html, tailwind } => {
                                        Some(PromptMessage::ShowDiv { id, html, tailwind })
                                    }
                                    Message::Form { id, html } => {
                                        Some(PromptMessage::ShowForm { id, html })
                                    }
                                                Message::Term { id, command } => {
                                                    Some(PromptMessage::ShowTerm { id, command })
                                                }
                                                Message::Editor { id, content, language, .. } => {
                                                    Some(PromptMessage::ShowEditor { id, content, language })
                                                }
                                                Message::Exit { .. } => {
                                                    Some(PromptMessage::ScriptExit)
                                                }
                                    Message::Hide {} => {
                                        Some(PromptMessage::HideWindow)
                                    }
                                    Message::Browse { url } => {
                                        Some(PromptMessage::OpenBrowser { url })
                                    }
                                    other => {
                                        // Get the message type name for user feedback
                                        let msg_type = format!("{:?}", other);
                                        // Extract just the variant name (before any {})
                                        let type_name = msg_type.split('{').next()
                                            .unwrap_or(&msg_type)
                                            .trim()
                                            .to_string();
                                        logging::log("WARN", &format!("Unhandled message type: {}", type_name));
                                        Some(PromptMessage::UnhandledMessage { message_type: type_name })
                                    }
                                };
                                
                                if let Some(prompt_msg) = prompt_msg {
                                    if tx.send_blocking(prompt_msg).is_err() {
                                        logging::log("EXEC", "Prompt channel closed, reader exiting");
                                        break;
                                    }
                                }
                            }
                            Ok(None) => {
                                logging::log("EXEC", "Script stdout closed (EOF)");
                                
                                // Check if process exited with error
                                let exit_code = match keep_alive_child.try_wait() {
                                    Ok(Some(status)) => status.code(),
                                    Ok(None) => {
                                        // Process still running, wait for it
                                        match keep_alive_child.wait() {
                                            Ok(status) => status.code(),
                                            Err(_) => None,
                                        }
                                    }
                                    Err(_) => None,
                                };
                                
                                logging::log("EXEC", &format!("Script exit code: {:?}", exit_code));
                                
                                // If non-zero exit code, capture stderr and send error
                                if let Some(code) = exit_code {
                                    if code != 0 {
                                        // Read stderr if available
                                        let stderr_output = if let Some(mut stderr) = stderr_for_errors.take() {
                                            use std::io::Read;
                                            let mut stderr_str = String::new();
                                            if stderr.read_to_string(&mut stderr_str).is_ok() && !stderr_str.is_empty() {
                                                Some(stderr_str)
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };
                                        
                                        if let Some(ref stderr_text) = stderr_output {
                                            logging::log("EXEC", &format!("Captured stderr ({} bytes)", stderr_text.len()));
                                            
                                            // Parse error info and generate suggestions
                                            let error_message = executor::extract_error_message(stderr_text);
                                            let stack_trace = executor::parse_stack_trace(stderr_text);
                                            let suggestions = executor::generate_suggestions(stderr_text, Some(code));
                                            
                                            // Send script error message
                                            let _ = tx.send_blocking(PromptMessage::ScriptError {
                                                error_message,
                                                stderr_output: Some(stderr_text.clone()),
                                                exit_code: Some(code),
                                                stack_trace,
                                                script_path: script_path.clone(),
                                                suggestions,
                                            });
                                        } else {
                                            // No stderr, send generic error
                                            let _ = tx.send_blocking(PromptMessage::ScriptError {
                                                error_message: format!("Script exited with code {}", code),
                                                stderr_output: None,
                                                exit_code: Some(code),
                                                stack_trace: None,
                                                script_path: script_path.clone(),
                                                suggestions: vec!["Check the script for errors".to_string()],
                                            });
                                        }
                                    }
                                }
                                
                                let _ = tx.send_blocking(PromptMessage::ScriptExit);
                                break;
                            }
                            Err(e) => {
                                logging::log("EXEC", &format!("Error reading from script: {}", e));
                                
                                // Try to read stderr for error details
                                let stderr_output = if let Some(mut stderr) = stderr_for_errors.take() {
                                    use std::io::Read;
                                    let mut stderr_str = String::new();
                                    if stderr.read_to_string(&mut stderr_str).is_ok() && !stderr_str.is_empty() {
                                        Some(stderr_str)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };
                                
                                if let Some(ref stderr_text) = stderr_output {
                                    let error_message = executor::extract_error_message(stderr_text);
                                    let stack_trace = executor::parse_stack_trace(stderr_text);
                                    let suggestions = executor::generate_suggestions(stderr_text, None);
                                    
                                    let _ = tx.send_blocking(PromptMessage::ScriptError {
                                        error_message,
                                        stderr_output: Some(stderr_text.clone()),
                                        exit_code: None,
                                        stack_trace,
                                        script_path: script_path.clone(),
                                        suggestions,
                                    });
                                }
                                
                                let _ = tx.send_blocking(PromptMessage::ScriptExit);
                                break;
                            }
                        }
                    }
                    logging::log("EXEC", "Reader thread exited, process handle will now be dropped");
                });
                
                // Store the response sender for the UI to use
                self.response_sender = Some(response_tx);
            }
            Err(e) => {
                logging::log("EXEC", &format!("Failed to start interactive session: {}", e));
                self.last_output = Some(SharedString::from(format!("✗ Error: {}", e)));
                cx.notify();
            }
        }
    }
    
    /// Execute a scriptlet (simple code snippet from .md file)
    fn execute_scriptlet(&mut self, scriptlet: &scripts::Scriptlet, _cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Executing scriptlet: {}", scriptlet.name));
        
        // For now, just log it - scriptlets are passive code snippets
        // Future implementation could copy to clipboard, execute, or display
        self.last_output = Some(SharedString::from(format!("Scriptlet: {}", scriptlet.name)));
    }
    
    /// Execute a built-in feature
    fn execute_builtin(&mut self, entry: &builtins::BuiltInEntry, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Executing built-in: {} (id: {})", entry.name, entry.id));
        
        match &entry.feature {
            builtins::BuiltInFeature::ClipboardHistory => {
                logging::log("EXEC", "Opening Clipboard History");
                let entries = clipboard_history::get_clipboard_history(100);
                logging::log("EXEC", &format!("Loaded {} clipboard entries", entries.len()));
                self.current_view = AppView::ClipboardHistoryView {
                    entries,
                    filter: String::new(),
                    selected_index: 0,
                };
                // Use standard height for clipboard history view
                defer_resize_to_view(ViewType::ScriptList, 0, cx);
                cx.notify();
            }
            builtins::BuiltInFeature::AppLauncher => {
                logging::log("EXEC", "Opening App Launcher");
                let apps = app_launcher::scan_applications().clone();
                logging::log("EXEC", &format!("Loaded {} applications", apps.len()));
                self.current_view = AppView::AppLauncherView {
                    apps,
                    filter: String::new(),
                    selected_index: 0,
                };
                // Use standard height for app launcher view
                defer_resize_to_view(ViewType::ScriptList, 0, cx);
                cx.notify();
            }
            builtins::BuiltInFeature::App(app_name) => {
                logging::log("EXEC", &format!("Launching app: {}", app_name));
                // Find and launch the specific application
                let apps = app_launcher::scan_applications();
                if let Some(app) = apps.iter().find(|a| a.name == *app_name) {
                    if let Err(e) = app_launcher::launch_application(app) {
                        logging::log("ERROR", &format!("Failed to launch {}: {}", app_name, e));
                        self.last_output = Some(SharedString::from(format!("Failed to launch: {}", app_name)));
                    } else {
                        logging::log("EXEC", &format!("Launched app: {}", app_name));
                        // Hide window after launching app
                        WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                        cx.hide();
                    }
                } else {
                    logging::log("ERROR", &format!("App not found: {}", app_name));
                    self.last_output = Some(SharedString::from(format!("App not found: {}", app_name)));
                }
                cx.notify();
            }
            builtins::BuiltInFeature::WindowSwitcher => {
                logging::log("EXEC", "Opening Window Switcher");
                // Load windows when view is opened (windows change frequently)
                match window_control::list_windows() {
                    Ok(windows) => {
                        logging::log("EXEC", &format!("Loaded {} windows", windows.len()));
                        self.current_view = AppView::WindowSwitcherView {
                            windows,
                            filter: String::new(),
                            selected_index: 0,
                        };
                        // Use standard height for window switcher view
                        defer_resize_to_view(ViewType::ScriptList, 0, cx);
                    }
                    Err(e) => {
                        logging::log("ERROR", &format!("Failed to list windows: {}", e));
                        self.toast_manager.push(
                            components::toast::Toast::error(
                                format!("Failed to list windows: {}", e),
                                &self.theme
                            ).duration_ms(Some(5000))
                        );
                    }
                }
                cx.notify();
            }
        }
    }
    
    /// Execute an application directly from the main search results
    fn execute_app(&mut self, app: &app_launcher::AppInfo, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Launching app from search: {}", app.name));
        
        if let Err(e) = app_launcher::launch_application(app) {
            logging::log("ERROR", &format!("Failed to launch {}: {}", app.name, e));
            self.last_output = Some(SharedString::from(format!("Failed to launch: {}", app.name)));
            cx.notify();
        } else {
            logging::log("EXEC", &format!("Launched app: {}", app.name));
            // Hide window after launching app
            WINDOW_VISIBLE.store(false, Ordering::SeqCst);
            cx.hide();
        }
    }
    
    /// Focus a window from the main search results
    fn execute_window_focus(&mut self, window: &window_control::WindowInfo, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Focusing window: {} - {}", window.app, window.title));
        
        if let Err(e) = window_control::focus_window(window.id) {
            logging::log("ERROR", &format!("Failed to focus window: {}", e));
            self.toast_manager.push(
                components::toast::Toast::error(
                    format!("Failed to focus window: {}", e),
                    &self.theme
                ).duration_ms(Some(5000))
            );
            cx.notify();
        } else {
            logging::log("EXEC", &format!("Focused window: {}", window.title));
            // Hide Script Kit after focusing window
            WINDOW_VISIBLE.store(false, Ordering::SeqCst);
            cx.hide();
        }
    }
    
    /// Handle a prompt message from the script
    fn handle_prompt_message(&mut self, msg: PromptMessage, cx: &mut Context<Self>) {
        match msg {
            PromptMessage::ShowArg { id, placeholder, choices } => {
                logging::log("UI", &format!("Showing arg prompt: {} with {} choices", id, choices.len()));
                let choice_count = choices.len();
                self.current_view = AppView::ArgPrompt { id, placeholder, choices };
                self.arg_input_text.clear();
                self.arg_selected_index = 0;
                self.focused_input = FocusedInput::ArgPrompt;
                // Resize window based on number of choices
                let view_type = if choice_count == 0 {
                    ViewType::ArgPromptNoChoices
                } else {
                    ViewType::ArgPromptWithChoices
                };
                defer_resize_to_view(view_type, choice_count, cx);
                cx.notify();
            }
            PromptMessage::ShowDiv { id, html, tailwind } => {
                logging::log("UI", &format!("Showing div prompt: {}", id));
                self.current_view = AppView::DivPrompt { id, html, tailwind };
                self.focused_input = FocusedInput::None; // DivPrompt has no text input
                defer_resize_to_view(ViewType::DivPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ShowForm { id, html } => {
                logging::log("UI", &format!("Showing form prompt: {}", id));
                self.current_view = AppView::FormPrompt { id, html };
                self.focused_input = FocusedInput::None; // FormPrompt has no text input (submit button instead)
                defer_resize_to_view(ViewType::DivPrompt, 0, cx); // Use DivPrompt size for now
                cx.notify();
            }
            PromptMessage::ShowTerm { id, command } => {
                logging::log("UI", &format!("Showing term prompt: {} (command: {:?})", id, command));
                
                // Create submit callback for terminal
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> = 
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send terminal response: {}", e));
                            }
                        }
                    });
                
                // Get the target height for terminal view
                let term_height = window_resize::layout::MAX_HEIGHT;
                
                // Create terminal with explicit height - GPUI entities don't inherit parent flex sizing
                match term_prompt::TermPrompt::with_height(
                    id.clone(),
                    command,
                    self.focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                    std::sync::Arc::new(self.config.clone()),
                    Some(term_height),
                ) {
                    Ok(term_prompt) => {
                        let entity = cx.new(|_| term_prompt);
                        self.current_view = AppView::TermPrompt { id, entity };
                        self.focused_input = FocusedInput::None; // Terminal handles its own cursor
                        defer_resize_to_view(ViewType::TermPrompt, 0, cx);
                        cx.notify();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to create terminal");
                        logging::log("ERROR", &format!("Failed to create terminal: {}", e));
                    }
                }
            }
            PromptMessage::ShowEditor { id, content, language } => {
                logging::log("UI", &format!("Showing editor prompt: {} (language: {:?})", id, language));
                
                // Create submit callback for editor
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> = 
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send editor response: {}", e));
                            }
                        }
                    });
                
                // CRITICAL: Create a SEPARATE focus handle for the editor.
                // Using the parent's focus handle causes keyboard event routing issues
                // because the parent checks is_focused() in its render and both parent
                // and child would be tracking the same handle.
                let editor_focus_handle = cx.focus_handle();
                
                // Get the target height for editor view
                let editor_height = window_resize::layout::MAX_HEIGHT;
                
                // Create editor with explicit height - GPUI entities don't inherit parent flex sizing
                let editor_prompt = EditorPrompt::with_height(
                    id.clone(),
                    content.unwrap_or_default(),
                    language.unwrap_or_else(|| "typescript".to_string()),
                    editor_focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                    std::sync::Arc::new(self.config.clone()),
                    Some(editor_height),
                );
                
                let entity = cx.new(|_| editor_prompt);
                self.current_view = AppView::EditorPrompt { id, entity, focus_handle: editor_focus_handle };
                self.focused_input = FocusedInput::None; // Editor handles its own focus
                
                defer_resize_to_view(ViewType::EditorPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ScriptExit => {
                logging::log("VISIBILITY", "=== ScriptExit message received ===");
                let is_visible = WINDOW_VISIBLE.load(Ordering::SeqCst);
                logging::log("VISIBILITY", &format!("WINDOW_VISIBLE is: {} (script exit doesn't change this)", is_visible));
                
                // Set flag so next hotkey show will reset to script list
                NEEDS_RESET.store(true, Ordering::SeqCst);
                logging::log("VISIBILITY", "NEEDS_RESET set to: true");
                
                self.reset_to_script_list(cx);
                logging::log("VISIBILITY", "reset_to_script_list() called");
            }
            PromptMessage::HideWindow => {
                logging::log("VISIBILITY", "=== HideWindow message received ===");
                let was_visible = WINDOW_VISIBLE.load(Ordering::SeqCst);
                logging::log("VISIBILITY", &format!("WINDOW_VISIBLE was: {}", was_visible));
                
                // CRITICAL: Update visibility state so hotkey toggle works correctly
                WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                logging::log("VISIBILITY", "WINDOW_VISIBLE set to: false");
                
                // Set flag so next hotkey show will reset to script list
                NEEDS_RESET.store(true, Ordering::SeqCst);
                logging::log("VISIBILITY", "NEEDS_RESET set to: true");
                
                cx.hide();
                logging::log("VISIBILITY", "cx.hide() called - window should now be hidden");
            }
            PromptMessage::OpenBrowser { url } => {
                logging::log("UI", &format!("Opening browser: {}", url));
                #[cfg(target_os = "macos")]
                {
                    match std::process::Command::new("open")
                        .arg(&url)
                        .spawn()
                    {
                        Ok(_) => logging::log("UI", &format!("Successfully opened URL in browser: {}", url)),
                        Err(e) => logging::log("ERROR", &format!("Failed to open URL '{}': {}", url, e)),
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    match std::process::Command::new("xdg-open")
                        .arg(&url)
                        .spawn()
                    {
                        Ok(_) => logging::log("UI", &format!("Successfully opened URL in browser: {}", url)),
                        Err(e) => logging::log("ERROR", &format!("Failed to open URL '{}': {}", url, e)),
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    match std::process::Command::new("cmd")
                        .args(["/C", "start", &url])
                        .spawn()
                    {
                        Ok(_) => logging::log("UI", &format!("Successfully opened URL in browser: {}", url)),
                        Err(e) => logging::log("ERROR", &format!("Failed to open URL '{}': {}", url, e)),
                    }
                }
            }
            PromptMessage::RunScript { path } => {
                logging::log("EXEC", &format!("RunScript command received: {}", path));
                
                // Create a Script struct from the path
                let script_path = std::path::PathBuf::from(&path);
                let script_name = script_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let extension = script_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ts")
                    .to_string();
                
                let script = scripts::Script {
                    name: script_name.clone(),
                    description: Some(format!("External script: {}", path)),
                    path: script_path,
                    extension,
                };
                
                logging::log("EXEC", &format!("Executing script: {}", script_name));
                self.execute_interactive(&script, cx);
            }
            PromptMessage::ScriptError { 
                error_message, 
                stderr_output, 
                exit_code, 
                stack_trace, 
                script_path, 
                suggestions 
            } => {
                logging::log("ERROR", &format!("Script error received: {} (exit code: {:?})", error_message, exit_code));
                
                // Create error toast with expandable details
                // Use stderr_output if available, otherwise use stack_trace
                let details_text = stderr_output.clone().or_else(|| stack_trace.clone());
                let toast = Toast::error(error_message.clone(), &self.theme)
                    .details_opt(details_text.clone())
                    .duration_ms(Some(10000)); // 10 seconds for errors
                
                // Add copy button action if we have stderr/stack trace
                let toast = if let Some(ref trace) = details_text {
                    let trace_clone = trace.clone();
                    toast.action(ToastAction::new(
                        "Copy Error",
                        Box::new(move |_, _, _| {
                            // Copy to clipboard
                            use arboard::Clipboard;
                            if let Ok(mut clipboard) = Clipboard::new() {
                                let _ = clipboard.set_text(trace_clone.clone());
                                logging::log("UI", "Error copied to clipboard");
                            }
                        }),
                    ))
                } else {
                    toast
                };
                
                // Log suggestions if present
                if !suggestions.is_empty() {
                    logging::log("ERROR", &format!("Suggestions: {:?}", suggestions));
                }
                
                // Push toast to manager
                let toast_id = self.toast_manager.push(toast);
                logging::log("UI", &format!("Toast created for script error: {} (id: {})", script_path, toast_id));
                
                cx.notify();
            }
            PromptMessage::UnhandledMessage { message_type } => {
                logging::log("WARN", &format!("Displaying unhandled message warning: {}", message_type));
                
                let toast = Toast::warning(
                    format!("'{}' is not yet implemented", message_type),
                    &self.theme
                ).duration_ms(Some(5000));
                
                self.toast_manager.push(toast);
                cx.notify();
            }
            PromptMessage::GetState { request_id } => {
                logging::log("UI", &format!("Collecting state for request: {}", request_id));
                
                // Collect current UI state
                let (prompt_type, prompt_id, placeholder, input_value, choice_count, visible_choice_count, selected_index, selected_value) = 
                    match &self.current_view {
                        AppView::ScriptList => {
                            let filtered_len = self.filtered_results().len();
                            let selected_value = if self.selected_index < filtered_len {
                                self.filtered_results().get(self.selected_index).map(|r| {
                                    match r {
                                        scripts::SearchResult::Script(m) => m.script.name.clone(),
                                        scripts::SearchResult::Scriptlet(m) => m.scriptlet.name.clone(),
                                        scripts::SearchResult::BuiltIn(m) => m.entry.name.clone(),
                                        scripts::SearchResult::App(m) => m.app.name.clone(),
                                        scripts::SearchResult::Window(m) => m.window.title.clone(),
                                    }
                                })
                            } else {
                                None
                            };
                            (
                                "none".to_string(),
                                None,
                                None,
                                self.filter_text.clone(),
                                self.scripts.len() + self.scriptlets.len() + self.builtin_entries.len() + self.apps.len(),
                                filtered_len,
                                self.selected_index as i32,
                                selected_value,
                            )
                        }
                        AppView::ArgPrompt { id, placeholder, choices } => {
                            let filtered = self.get_filtered_arg_choices(choices);
                            let selected_value = if self.arg_selected_index < filtered.len() {
                                filtered.get(self.arg_selected_index).map(|c| c.value.clone())
                            } else {
                                None
                            };
                            (
                                "arg".to_string(),
                                Some(id.clone()),
                                Some(placeholder.clone()),
                                self.arg_input_text.clone(),
                                choices.len(),
                                filtered.len(),
                                self.arg_selected_index as i32,
                                selected_value,
                            )
                        }
                        AppView::DivPrompt { id, .. } => {
                            (
                                "div".to_string(),
                                Some(id.clone()),
                                None,
                                String::new(),
                                0,
                                0,
                                -1,
                                None,
                            )
                        }
                        AppView::FormPrompt { id, .. } => {
                            (
                                "form".to_string(),
                                Some(id.clone()),
                                None,
                                String::new(),
                                0,
                                0,
                                -1,
                                None,
                            )
                        }
                        AppView::TermPrompt { id, .. } => {
                            (
                                "term".to_string(),
                                Some(id.clone()),
                                None,
                                String::new(),
                                0,
                                0,
                                -1,
                                None,
                            )
                        }
                        AppView::EditorPrompt { id, .. } => {
                            (
                                "editor".to_string(),
                                Some(id.clone()),
                                None,
                                String::new(),
                                0,
                                0,
                                -1,
                                None,
                            )
                        }
                        AppView::ActionsDialog => {
                            (
                                "actions".to_string(),
                                None,
                                None,
                                String::new(),
                                0,
                                0,
                                -1,
                                None,
                            )
                        }
                        AppView::ClipboardHistoryView { entries, filter, selected_index } => {
                            let filtered_count = if filter.is_empty() {
                                entries.len()
                            } else {
                                let filter_lower = filter.to_lowercase();
                                entries.iter().filter(|e| e.content.to_lowercase().contains(&filter_lower)).count()
                            };
                            (
                                "clipboardHistory".to_string(),
                                None,
                                None,
                                filter.clone(),
                                entries.len(),
                                filtered_count,
                                *selected_index as i32,
                                None,
                            )
                        }
                        AppView::AppLauncherView { apps, filter, selected_index } => {
                            let filtered_count = if filter.is_empty() {
                                apps.len()
                            } else {
                                let filter_lower = filter.to_lowercase();
                                apps.iter().filter(|a| a.name.to_lowercase().contains(&filter_lower)).count()
                            };
                            (
                                "appLauncher".to_string(),
                                None,
                                None,
                                filter.clone(),
                                apps.len(),
                                filtered_count,
                                *selected_index as i32,
                                None,
                            )
                        }
                        AppView::WindowSwitcherView { windows, filter, selected_index } => {
                            let filtered_count = if filter.is_empty() {
                                windows.len()
                            } else {
                                let filter_lower = filter.to_lowercase();
                                windows.iter().filter(|w| {
                                    w.title.to_lowercase().contains(&filter_lower) ||
                                    w.app.to_lowercase().contains(&filter_lower)
                                }).count()
                            };
                            (
                                "windowSwitcher".to_string(),
                                None,
                                None,
                                filter.clone(),
                                windows.len(),
                                filtered_count,
                                *selected_index as i32,
                                None,
                            )
                        }
                    };
                
                // Focus state: we use focused_input as a proxy since we don't have Window access here.
                // When window is visible and we're tracking an input, we're focused.
                let window_visible = WINDOW_VISIBLE.load(Ordering::SeqCst);
                let is_focused = window_visible && self.focused_input != FocusedInput::None;
                
                // Create the response
                let response = Message::state_result(
                    request_id.clone(),
                    prompt_type,
                    prompt_id,
                    placeholder,
                    input_value,
                    choice_count,
                    visible_choice_count,
                    selected_index,
                    selected_value,
                    is_focused,
                    window_visible,
                );
                
                logging::log("UI", &format!("Sending state result for request: {}", request_id));
                
                // Send the response
                if let Some(ref sender) = self.response_sender {
                    if let Err(e) = sender.send(response) {
                        logging::log("ERROR", &format!("Failed to send state result: {}", e));
                    }
                } else {
                    logging::log("ERROR", "No response sender available for state result");
                }
            }
         }
      }
      
    /// Cancel the currently running script and clean up all state
    fn cancel_script_execution(&mut self, cx: &mut Context<Self>) {
        logging::log("EXEC", "=== Canceling script execution ===");
        
        // Send cancel message to script (Exit with cancel code)
        if let Some(ref sender) = self.response_sender {
            // Try to send Exit message to terminate the script cleanly
            let exit_msg = Message::Exit { 
                code: Some(1),  // Non-zero code indicates cancellation
                message: Some("Cancelled by user".to_string()),
            };
            match sender.send(exit_msg) {
                Ok(()) => logging::log("EXEC", "Sent Exit message to script"),
                Err(e) => logging::log("EXEC", &format!("Failed to send Exit: {} (script may have exited)", e)),
            }
        } else {
            logging::log("EXEC", "No response_sender - script may not be running");
        }
        
        // Belt-and-suspenders: Force-kill the process group using stored PID
        // This ensures cleanup even if Drop doesn't fire properly
        if let Some(pid) = self.current_script_pid.take() {
            logging::log("CLEANUP", &format!("Force-killing script process group {}", pid));
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &format!("-{}", pid)])
                    .output();
            }
        }
        
        // Abort script session if it exists
        if let Ok(mut session_guard) = self.script_session.lock() {
            if let Some(_session) = session_guard.take() {
                logging::log("EXEC", "Cleared script session");
            }
        }
        
        // Reset to script list view
        self.reset_to_script_list(cx);
        logging::log("EXEC", "=== Script cancellation complete ===");
    }
    
    /// Reset all state and return to the script list view.
    /// This clears all prompt state and resizes the window appropriately.
    fn reset_to_script_list(&mut self, cx: &mut Context<Self>) {
        let old_view = match &self.current_view {
            AppView::ScriptList => "ScriptList",
            AppView::ActionsDialog => "ActionsDialog",
            AppView::ArgPrompt { .. } => "ArgPrompt",
            AppView::DivPrompt { .. } => "DivPrompt",
            AppView::FormPrompt { .. } => "FormPrompt",
            AppView::TermPrompt { .. } => "TermPrompt",
            AppView::EditorPrompt { .. } => "EditorPrompt",
            AppView::ClipboardHistoryView { .. } => "ClipboardHistoryView",
            AppView::AppLauncherView { .. } => "AppLauncherView",
            AppView::WindowSwitcherView { .. } => "WindowSwitcherView",
        };
        
        let old_focused_input = self.focused_input;
        logging::log("UI", &format!("Resetting to script list (was: {}, focused_input: {:?})", old_view, old_focused_input));
        
        // Belt-and-suspenders: Force-kill the process group using stored PID
        // This runs BEFORE clearing channels to ensure cleanup even if Drop doesn't fire
        if let Some(pid) = self.current_script_pid.take() {
            logging::log("CLEANUP", &format!("Force-killing script process group {} during reset", pid));
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &format!("-{}", pid)])
                    .output();
            }
        }
        
        // Reset view
        self.current_view = AppView::ScriptList;
        
        // CRITICAL: Reset focused_input to MainFilter so the cursor appears
        // This was a bug where focused_input could remain as ArgPrompt/None after
        // script exit, causing the cursor to not show in the main filter.
        self.focused_input = FocusedInput::MainFilter;
        logging::log("FOCUS", "Reset focused_input to MainFilter for cursor display");
        
        // Clear arg prompt state
        self.arg_input_text.clear();
        self.arg_selected_index = 0;
        // P0: Reset arg scroll handle
        self.arg_list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
        
        // Clear filter and selection state for fresh menu
        self.filter_text.clear();
        self.selected_index = 0;
        self.last_scrolled_index = None;
        self.list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
        self.last_scrolled_index = Some(0);
        
        // Resize window for script list content
        let count = self.scripts.len() + self.scriptlets.len();
        resize_first_window_to_height(height_for_view(ViewType::ScriptList, count));
        
        // Clear output
        self.last_output = None;
        
        // Clear channels (they will be dropped, closing the connections)
        self.prompt_receiver = None;
        self.response_sender = None;
        
        // Clear script session
        if let Ok(mut session_guard) = self.script_session.lock() {
            *session_guard = None;
        }
        
        logging::log("UI", "State reset complete - view is now ScriptList (filter, selection, scroll cleared)");
        cx.notify();
    }
    
    /// Check if we're currently in a prompt view (script is running)
    fn is_in_prompt(&self) -> bool {
        matches!(self.current_view, 
            AppView::ArgPrompt { .. } | 
            AppView::DivPrompt { .. } | 
            AppView::FormPrompt { .. } |
            AppView::TermPrompt { .. } | 
            AppView::EditorPrompt { .. } |
            AppView::ClipboardHistoryView { .. } |
            AppView::AppLauncherView { .. } |
            AppView::WindowSwitcherView { .. }
        )
    }
      
      /// Submit a response to the current prompt
     fn submit_prompt_response(&mut self, id: String, value: Option<String>, _cx: &mut Context<Self>) {
        logging::log("UI", &format!("Submitting response for {}: {:?}", id, value));
        
        let response = Message::Submit { id, value };
        
        if let Some(ref sender) = self.response_sender {
            match sender.send(response) {
                Ok(()) => {
                    logging::log("UI", "Response queued for script");
                }
                Err(e) => {
                    logging::log("UI", &format!("Failed to queue response: {}", e));
                }
            }
        } else {
            logging::log("UI", "No response sender available");
        }
        
        // Return to waiting state (script will send next prompt or exit)
        // Don't change view here - wait for next message from script
    }
    
    /// Get filtered choices for arg prompt
    fn filtered_arg_choices(&self) -> Vec<(usize, &Choice)> {
        if let AppView::ArgPrompt { choices, .. } = &self.current_view {
            if self.arg_input_text.is_empty() {
                choices.iter().enumerate().collect()
            } else {
                let filter = self.arg_input_text.to_lowercase();
                choices.iter()
                    .enumerate()
                    .filter(|(_, c)| c.name.to_lowercase().contains(&filter))
                    .collect()
            }
        } else {
            vec![]
        }
    }
    
    /// P0: Get filtered choices as owned data for uniform_list closure
    fn get_filtered_arg_choices_owned(&self) -> Vec<(usize, Choice)> {
        if let AppView::ArgPrompt { choices, .. } = &self.current_view {
            if self.arg_input_text.is_empty() {
                choices.iter().enumerate().map(|(i, c)| (i, c.clone())).collect()
            } else {
                let filter = self.arg_input_text.to_lowercase();
                choices.iter()
                    .enumerate()
                    .filter(|(_, c)| c.name.to_lowercase().contains(&filter))
                    .map(|(i, c)| (i, c.clone()))
                    .collect()
            }
        } else {
            vec![]
        }
    }
    
    /// Convert hex color to rgba with opacity from theme
    fn hex_to_rgba_with_opacity(&self, hex: u32, opacity: f32) -> u32 {
        // Convert opacity (0.0-1.0) to alpha byte (0-255)
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0) as u32;
        (hex << 8) | alpha
    }
    
    /// Create box shadows from theme configuration
    fn create_box_shadows(&self) -> Vec<BoxShadow> {
        let shadow_config = self.theme.get_drop_shadow();
        
        if !shadow_config.enabled {
            return vec![];
        }
        
        // Convert hex color to HSLA
        // For black (0x000000), we use h=0, s=0, l=0
        let r = ((shadow_config.color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((shadow_config.color >> 8) & 0xFF) as f32 / 255.0;
        let b = (shadow_config.color & 0xFF) as f32 / 255.0;
        
        // Simple RGB to HSL conversion for shadow color
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        
        let (h, s) = if max == min {
            (0.0, 0.0) // achromatic
        } else {
            let d = max - min;
            let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
            let h = if max == r {
                (g - b) / d + if g < b { 6.0 } else { 0.0 }
            } else if max == g {
                (b - r) / d + 2.0
            } else {
                (r - g) / d + 4.0
            };
            (h / 6.0, s)
        };
        
        vec![
            BoxShadow {
                color: hsla(h, s, l, shadow_config.opacity),
                offset: point(px(shadow_config.offset_x), px(shadow_config.offset_y)),
                blur_radius: px(shadow_config.blur_radius),
                spread_radius: px(shadow_config.spread_radius),
            }
        ]
    }
}

impl Focusable for ScriptListApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ScriptListApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Dispatch to appropriate view - clone to avoid borrow issues
        let current_view = self.current_view.clone();
        
        // Focus handling depends on the view:
        // - For EditorPrompt: Use its own focus handle (not the parent's)
        // - For other views: Use the parent's focus handle
        match &current_view {
            AppView::EditorPrompt { focus_handle, .. } => {
                // EditorPrompt has its own focus handle - focus it
                let is_focused = focus_handle.is_focused(window);
                if !is_focused {
                    window.focus(focus_handle, cx);
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
        
        // NOTE: Prompt messages are now handled via event-driven async_channel listener
        // spawned in execute_interactive() - no polling needed in render()
        
        match current_view {
            AppView::ScriptList => self.render_script_list(cx),
            AppView::ActionsDialog => self.render_actions_dialog(cx),
            AppView::ArgPrompt { id, placeholder, choices } => self.render_arg_prompt(id, placeholder, choices, cx),
            AppView::DivPrompt { id, html, tailwind } => self.render_div_prompt(id, html, tailwind, cx),
            AppView::FormPrompt { id, html } => self.render_form_prompt(id, html, cx),
            AppView::TermPrompt { entity, .. } => self.render_term_prompt(entity, cx),
            AppView::EditorPrompt { entity, .. } => self.render_editor_prompt(entity, cx),
            AppView::ClipboardHistoryView { entries, filter, selected_index } => {
                self.render_clipboard_history(entries, filter, selected_index, cx)
            }
            AppView::AppLauncherView { apps, filter, selected_index } => {
                self.render_app_launcher(apps, filter, selected_index, cx)
            }
            AppView::WindowSwitcherView { windows, filter, selected_index } => {
                self.render_window_switcher(windows, filter, selected_index, cx)
            }
        }
    }
}

impl ScriptListApp {
    /// Read the first N lines of a script file for preview
    #[allow(dead_code)]
    fn read_script_preview(path: &std::path::Path, max_lines: usize) -> String {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let preview: String = content
                    .lines()
                    .take(max_lines)
                    .collect::<Vec<_>>()
                    .join("\n");
                logging::log("UI", &format!(
                    "Preview loaded: {} ({} lines read)",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    content.lines().count().min(max_lines)
                ));
                preview
            }
            Err(e) => {
                logging::log("UI", &format!(
                    "Preview error: {} - {}",
                    path.display(),
                    e
                ));
                format!("Error reading file: {}", e)
            }
        }
    }
    
    /// Render toast notifications from the toast manager
    /// 
    /// Toasts are positioned in the top-right corner and stack vertically.
    /// Each toast has its own dismiss callback that removes it from the manager.
    fn render_toasts(&mut self, _cx: &mut Context<Self>) -> Option<impl IntoElement> {
        // Tick the manager to handle auto-dismiss
        self.toast_manager.tick();
        
        // Clean up dismissed toasts
        self.toast_manager.cleanup();
        
        // Check if toasts need update (consume the flag to prevent repeated checks)
        // Note: We don't call cx.notify() here as it's an anti-pattern during render.
        // Toast updates are handled by timer-based refresh mechanisms.
        let _ = self.toast_manager.take_needs_notify();
        
        let visible = self.toast_manager.visible_toasts();
        if visible.is_empty() {
            return None;
        }
        
        // Use design tokens for consistent spacing
        let tokens = get_tokens(self.current_design);
        let spacing = tokens.spacing();
        
        // Build toast container (positioned in top-right via absolute positioning)
        let mut toast_container = div()
            .absolute()
            .top(px(spacing.padding_lg))
            .right(px(spacing.padding_lg))
            .flex()
            .flex_col()
            .gap(px(spacing.gap_sm))
            .w(px(380.0));  // Fixed width for toasts
        
        // Add each visible toast
        for notification in visible {
            // Clone the toast for rendering - unfortunately we need to recreate it
            // since Toast::render consumes self
            let toast_colors = ToastColors::from_theme(&self.theme, notification.toast.get_variant());
            let toast = Toast::new(notification.toast.get_message().clone(), toast_colors)
                .variant(notification.toast.get_variant())
                .duration_ms(notification.toast.get_duration_ms())
                .dismissible(true);
            
            // Add details if the toast has them
            let toast = toast.details_opt(notification.toast.get_details().cloned());
            
            toast_container = toast_container.child(toast);
        }
        
        Some(toast_container)
    }
    
    /// Render the error notification if one exists
    /// 
    /// Returns None if no notification is present.
    /// Uses theme colors (colors.ui.error, colors.ui.warning, colors.ui.info)
    /// styled with bg, rounded corners, padding.
    fn render_error_notification(&self) -> Option<impl IntoElement> {
        let notification = self.error_notification.as_ref()?;
        
        // Use design tokens for consistent spacing
        let tokens = get_tokens(self.current_design);
        let spacing = tokens.spacing();
        let visual = tokens.visual();
        let typography = tokens.typography();
        
        // Get the appropriate color based on severity
        let bg_color = match notification.severity {
            ErrorSeverity::Error | ErrorSeverity::Critical => self.theme.colors.ui.error,
            ErrorSeverity::Warning => self.theme.colors.ui.warning,
            ErrorSeverity::Info => self.theme.colors.ui.info,
        };
        
        // Use contrasting text color (white for all severities works well)
        let text_color = 0xffffff;
        
        // Icon based on severity
        let icon = match notification.severity {
            ErrorSeverity::Critical => "⛔",
            ErrorSeverity::Error => "✕",
            ErrorSeverity::Warning => "⚠",
            ErrorSeverity::Info => "ℹ",
        };
        
        Some(
            div()
                .w_full()
                .mx(px(spacing.padding_lg))
                .mt(px(spacing.padding_sm))
                .px(px(spacing.padding_md))
                .py(px(spacing.padding_sm))
                .rounded(px(visual.radius_md))
                .bg(rgb(bg_color))
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .font_family(typography.font_family)
                // Icon
                .child(
                    div()
                        .text_color(rgb(text_color))
                        .text_sm()
                        .child(icon)
                )
                // Message text
                .child(
                    div()
                        .flex_1()
                        .text_color(rgb(text_color))
                        .text_sm()
                        .child(notification.message.clone())
                )
        )
    }
    
    /// Render the preview panel showing details of the selected script/scriptlet
    fn render_preview_panel(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let filtered = self.filtered_results();
        let selected_result = filtered.get(self.selected_index).cloned();
        
        // Use design tokens for GLOBAL theming - design applies to ALL components
        let tokens = get_tokens(self.current_design);
        let colors = tokens.colors();
        let spacing = tokens.spacing();
        let typography = tokens.typography();
        let visual = tokens.visual();
        
        // Map design tokens to local variables (all designs use tokens now)
        let bg_main = colors.background;
        let ui_border = colors.border;
        let text_primary = colors.text_primary;
        let text_muted = colors.text_muted;
        let text_secondary = colors.text_secondary;
        let bg_search_box = colors.background_tertiary;
        let border_radius = visual.radius_md;
        let font_family = typography.font_family;
        
        // Preview panel container with left border separator
        let mut panel = div()
            .w_full()
            .h_full()
            .bg(rgb(bg_main))
            .border_l_1()
            .border_color(rgba((ui_border << 8) | 0x80))
            .p(px(spacing.padding_lg))
            .flex()
            .flex_col()
            .overflow_y_hidden()
            .font_family(font_family);
        
        match selected_result {
            Some(ref result) => {
                match result {
                    scripts::SearchResult::Script(script_match) => {
                        let script = &script_match.script;
                        
                        // Script name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(text_primary))
                                .pb(px(spacing.padding_sm))
                                .child(format!("{}.{}", script.name, script.extension))
                        );
                        

                        
                        // Description (if present)
                        if let Some(desc) = &script.description {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Description")
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(desc.clone())
                                    )
                            );
                        }
                        
                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm))
                        );
                        
                        // Code preview header
                        panel = panel.child(
                            div()
                                .text_xs()
                                .text_color(rgb(text_muted))
                                .pb(px(spacing.padding_sm))
                                .child("Code Preview")
                        );
                        
                        // Use cached syntax-highlighted lines (avoids file I/O and highlighting on every render)
                        let script_path = script.path.to_string_lossy().to_string();
                        let lang = script.extension.clone();
                        let lines = self.get_or_update_preview_cache(&script_path, &lang).to_vec();
                        
                        // Build code container - render line by line with monospace font
                        let mut code_container = div()
                            .w_full()
                            .min_w(px(280.))
                            .p(px(spacing.padding_md))
                            .rounded(px(border_radius))
                            .bg(rgba((bg_search_box << 8) | 0x80))
                            .overflow_hidden()
                            .flex()
                            .flex_col();
                        
                        // Render each line as a row of spans with monospace font
                        for line in lines {
                            let mut line_div = div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .font_family(typography.font_family_mono)
                                .text_xs()
                                .min_h(px(spacing.padding_lg)); // Line height
                            
                            if line.spans.is_empty() {
                                // Empty line - add a space to preserve height
                                line_div = line_div.child(" ");
                            } else {
                                for span in line.spans {
                                    line_div = line_div.child(
                                        div()
                                            .text_color(rgb(span.color))
                                            .child(span.text)
                                    );
                                }
                            }
                            
                            code_container = code_container.child(line_div);
                        }
                        
                        panel = panel.child(code_container);
                    }
                    scripts::SearchResult::Scriptlet(scriptlet_match) => {
                        let scriptlet = &scriptlet_match.scriptlet;
                        
                        // Scriptlet name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(text_primary))
                                .pb(px(spacing.padding_sm))
                                .child(scriptlet.name.clone())
                        );
                        

                        
                        // Description (if present)
                        if let Some(desc) = &scriptlet.description {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Description")
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(desc.clone())
                                    )
                            );
                        }
                        
                        // Shortcut (if present)
                        if let Some(shortcut) = &scriptlet.shortcut {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Hotkey")
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(shortcut.clone())
                                    )
                            );
                        }
                        
                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm))
                        );
                        
                        // Content preview header
                        panel = panel.child(
                            div()
                                .text_xs()
                                .text_color(rgb(text_muted))
                                .pb(px(spacing.padding_sm))
                                .child("Content Preview")
                        );
                        
                        // Display scriptlet code with syntax highlighting (first 15 lines)
                        // Note: Scriptlets store code in memory, no file I/O needed (no cache benefit)
                        let code_preview: String = scriptlet.code
                            .lines()
                            .take(15)
                            .collect::<Vec<_>>()
                            .join("\n");
                        
                        // Determine language from tool (bash, js, etc.)
                        let lang = match scriptlet.tool.as_str() {
                            "bash" | "zsh" | "sh" => "bash",
                            "node" | "bun" => "js",
                            _ => &scriptlet.tool,
                        };
                        let lines = highlight_code_lines(&code_preview, lang);
                        
                        // Build code container - render line by line with monospace font
                        let mut code_container = div()
                            .w_full()
                            .min_w(px(280.))
                            .p(px(spacing.padding_md))
                            .rounded(px(border_radius))
                            .bg(rgba((bg_search_box << 8) | 0x80))
                            .overflow_hidden()
                            .flex()
                            .flex_col();
                        
                        // Render each line as a row of spans with monospace font
                        for line in lines {
                            let mut line_div = div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .font_family(typography.font_family_mono)
                                .text_xs()
                                .min_h(px(spacing.padding_lg)); // Line height
                            
                            if line.spans.is_empty() {
                                // Empty line - add a space to preserve height
                                line_div = line_div.child(" ");
                            } else {
                                for span in line.spans {
                                    line_div = line_div.child(
                                        div()
                                            .text_color(rgb(span.color))
                                            .child(span.text)
                                    );
                                }
                            }
                            
                            code_container = code_container.child(line_div);
                        }
                        
                        panel = panel.child(code_container);
                    }
                    scripts::SearchResult::BuiltIn(builtin_match) => {
                        let builtin = &builtin_match.entry;
                        
                        // Built-in name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(text_primary))
                                .pb(px(spacing.padding_sm))
                                .child(builtin.name.clone())
                        );
                        
                        // Description
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .pb(px(spacing.padding_md))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Description")
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(builtin.description.clone())
                                )
                        );
                        
                        // Keywords
                        if !builtin.keywords.is_empty() {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Keywords")
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(builtin.keywords.join(", "))
                                    )
                            );
                        }
                        
                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm))
                        );
                        
                        // Feature type indicator
                        let feature_type: String = match &builtin.feature {
                            builtins::BuiltInFeature::ClipboardHistory => "Clipboard History Manager".to_string(),
                            builtins::BuiltInFeature::AppLauncher => "Application Launcher".to_string(),
                            builtins::BuiltInFeature::App(name) => name.clone(),
                            builtins::BuiltInFeature::WindowSwitcher => "Window Manager".to_string(),
                        };
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Feature Type")
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(feature_type)
                                )
                        );
                    }
                    scripts::SearchResult::App(app_match) => {
                        let app = &app_match.app;
                        
                        // App name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(text_primary))
                                .pb(px(spacing.padding_sm))
                                .child(app.name.clone())
                        );
                        
                        // Path
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .pb(px(spacing.padding_md))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Path")
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(app.path.to_string_lossy().to_string())
                                )
                        );
                        
                        // Bundle ID (if available)
                        if let Some(bundle_id) = &app.bundle_id {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(spacing.padding_md))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(text_muted))
                                            .pb(px(spacing.padding_xs / 2.0))
                                            .child("Bundle ID")
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(text_secondary))
                                            .child(bundle_id.clone())
                                    )
                            );
                        }
                        
                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm))
                        );
                        
                        // Type indicator
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Type")
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child("Application")
                                )
                        );
                    }
                    scripts::SearchResult::Window(window_match) => {
                        let window = &window_match.window;
                        
                        // Window title header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(text_primary))
                                .pb(px(spacing.padding_sm))
                                .child(window.title.clone())
                        );
                        
                        // App name
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .pb(px(spacing.padding_md))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Application")
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(window.app.clone())
                                )
                        );
                        
                        // Bounds
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .pb(px(spacing.padding_md))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Position & Size")
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(format!("{}×{} at ({}, {})", 
                                            window.bounds.width, window.bounds.height,
                                            window.bounds.x, window.bounds.y))
                                )
                        );
                        
                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(visual.border_thin))
                                .bg(rgba((ui_border << 8) | 0x60))
                                .my(px(spacing.padding_sm))
                        );
                        
                        // Type indicator
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(text_muted))
                                        .pb(px(spacing.padding_xs / 2.0))
                                        .child("Type")
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child("Window")
                                )
                        );
                    }
                }
            }
            None => {
                logging::log("UI", "Preview panel: No selection");
                // Empty state
                panel = panel.child(
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(text_muted))
                        .child(
                            if self.filter_text.is_empty() && self.scripts.is_empty() && self.scriptlets.is_empty() {
                                "No scripts or snippets found"
                            } else if !self.filter_text.is_empty() {
                                "No matching scripts"
                            } else {
                                "Select a script to preview"
                            }
                        )
                );
            }
        }
        
        panel
    }
    
    /// Get the ScriptInfo for the currently focused/selected script
    fn get_focused_script_info(&self) -> Option<ScriptInfo> {
        let filtered = self.filtered_results();
        if let Some(result) = filtered.get(self.selected_index) {
            match result {
                scripts::SearchResult::Script(m) => {
                    Some(ScriptInfo::new(&m.script.name, m.script.path.to_string_lossy()))
                }
                scripts::SearchResult::Scriptlet(m) => {
                    // Scriptlets don't have a path, use name as identifier
                    Some(ScriptInfo::new(&m.scriptlet.name, format!("scriptlet:{}", &m.scriptlet.name)))
                }
                scripts::SearchResult::BuiltIn(m) => {
                    // Built-ins use their id as identifier
                    Some(ScriptInfo::new(&m.entry.name, format!("builtin:{}", &m.entry.id)))
                }
                scripts::SearchResult::App(m) => {
                    // Apps use their path as identifier
                    Some(ScriptInfo::new(&m.app.name, m.app.path.to_string_lossy().to_string()))
                }
                scripts::SearchResult::Window(m) => {
                    // Windows use their id as identifier
                    Some(ScriptInfo::new(&m.window.title, format!("window:{}", m.window.id)))
                }
            }
        } else {
            None
        }
    }
    
    fn render_script_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // P1: Use cached filtered results
        let filtered = self.get_filtered_results_cached();
        let filtered_len = filtered.len();
        let _total_len = self.scripts.len() + self.scriptlets.len();
        let theme = &self.theme;
        
        // Get design tokens for current design variant
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_visual = tokens.visual();
        let design_typography = tokens.typography();
        
        // For Default design, use theme.colors for backward compatibility
        // For other designs, use design tokens
        let is_default_design = self.current_design == DesignVariant::Default;
        
        // Handle edge cases - keep selected_index in valid bounds
        if self.selected_index >= filtered_len && filtered_len > 0 {
            self.selected_index = filtered_len.saturating_sub(1);
        }

        // Note: selected_index is now accessed from `this` inside the processor closure
        
        // P4: Pre-compute theme values using ListItemColors
        let list_colors = ListItemColors::from_theme(theme);
        logging::log_debug("PERF", "P4: Using ListItemColors for render closure");

        // Build script list using uniform_list for proper virtualized scrolling
        // Use design tokens for empty state styling
        let empty_text_color = if is_default_design { theme.colors.text.muted } else { design_colors.text_muted };
        let empty_font_family = if is_default_design { ".AppleSystemUIFont" } else { design_typography.font_family };
        
        let list_element: AnyElement = if filtered_len == 0 {
            div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(empty_text_color))
                .font_family(empty_font_family)
                .child(if self.filter_text.is_empty() {
                    "No scripts or snippets found".to_string()
                } else {
                    format!("No results match '{}'", self.filter_text)
                })
                .into_any_element()
        } else {
            // Use uniform_list for automatic virtualized scrolling
            // Note: Hover-to-select is implemented via on_mouse_down on each item wrapper
            // to update selected_index when the user clicks (selecting on hover alone would
            // be too aggressive - we update on hover enter instead for visual highlight)
            
            uniform_list(
                "script-list",
                filtered_len,
                cx.processor(move |this, visible_range: std::ops::Range<usize>, _window, cx| {
                    let mut items = Vec::new();
                    // Get the current selected_index FIRST before borrowing this via get_filtered_results_cached
                    let current_selected = this.selected_index;
                    // Get current design from app state
                    let design = this.current_design;
                    // P1: Use cached filtered results inside closure
                    let filtered = this.get_filtered_results_cached();
                    
                    for ix in visible_range {
                        if let Some(result) = filtered.get(ix) {
                            let is_selected = ix == current_selected;
                            
                            // Create hover handler that updates selected_index when mouse enters
                            // This gives visual feedback matching keyboard navigation
                            let hover_handler = cx.listener(move |this: &mut ScriptListApp, hovered: &bool, _window, cx| {
                                if *hovered && this.selected_index != ix {
                                    this.set_selected_index_from_hover(ix, cx);
                                }
                            });
                            
                            // Dispatch to design-specific item renderer
                            // This allows each design to have its own unique visual style
                            let item_element = render_design_item(
                                design,
                                result,
                                ix,
                                is_selected,
                                list_colors,
                            );
                            
                            // Wrap in div with hover handler for hover-to-select behavior
                            items.push(
                                div()
                                    .id(ElementId::NamedInteger("script-item".into(), ix as u64))
                                    .on_hover(hover_handler)
                                    .child(item_element),
                            );
                        }
                    }
                    items
                }),
            )
            .h_full()
            .track_scroll(&self.list_scroll_handle)
            .into_any_element()
        };

        // Log panel
        let log_panel = if self.show_logs {
            let logs = logging::get_last_logs(10);
            let mut log_container = div()
                .flex()
                .flex_col()
                .w_full()
                .bg(rgb(theme.colors.background.log_panel))
                .border_t_1()
                .border_color(rgb(theme.colors.ui.border))
                .p(px(design_spacing.padding_md))
                .max_h(px(120.))
                .font_family("SF Mono");
            
            for log_line in logs.iter().rev() {
                log_container = log_container.child(
                    div().text_color(rgb(theme.colors.ui.success)).text_xs().child(log_line.clone())
                );
            }
            Some(log_container)
        } else {
            None
        };

        let filter_display = if self.filter_text.is_empty() {
            SharedString::from(DEFAULT_PLACEHOLDER)
        } else {
            SharedString::from(self.filter_text.clone())
        };
        let filter_is_empty = self.filter_text.is_empty();

        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            let has_cmd = event.keystroke.modifiers.platform;
            
            if has_cmd {
                match key_str.as_str() {
                    "l" => { 
                        this.toggle_logs(cx); 
                        return; 
                    }
                    "k" => { 
                        this.toggle_actions(cx, window); 
                        return; 
                    }
                    // Cmd+1 cycles through all designs
                    "1" => {
                        this.cycle_design(cx);
                        return;
                    }
                    _ => {}
                }
            }
            
            // If actions popup is open, route keyboard events to it
            if this.show_actions_popup {
                if let Some(ref dialog) = this.actions_dialog {
                    match key_str.as_str() {
                        "up" | "arrowup" => {
                            dialog.update(cx, |d, cx| d.move_up(cx));
                            return;
                        }
                        "down" | "arrowdown" => {
                            dialog.update(cx, |d, cx| d.move_down(cx));
                            return;
                        }
                        "enter" => {
                            // Get the selected action and execute it
                            let action_id = dialog.read(cx).get_selected_action_id();
                            if let Some(action_id) = action_id {
                                logging::log("ACTIONS", &format!("Executing action: {}", action_id));
                                this.show_actions_popup = false;
                                this.actions_dialog = None;
                                this.focused_input = FocusedInput::MainFilter;
                                window.focus(&this.focus_handle, cx);
                                this.handle_action(action_id, cx);
                            }
                            return;
                        }
                        "escape" => {
                            this.show_actions_popup = false;
                            this.actions_dialog = None;
                            this.focused_input = FocusedInput::MainFilter;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                            return;
                        }
                        "backspace" => {
                            dialog.update(cx, |d, cx| d.handle_backspace(cx));
                            return;
                        }
                        _ => {
                            // Route character input to the dialog for search
                            if let Some(ref key_char) = event.keystroke.key_char {
                                if let Some(ch) = key_char.chars().next() {
                                    if !ch.is_control() {
                                        dialog.update(cx, |d, cx| d.handle_char(ch, cx));
                                    }
                                }
                            }
                            return;
                        }
                    }
                }
            }
            
            match key_str.as_str() {
                "up" | "arrowup" => this.move_selection_up(cx),
                "down" | "arrowdown" => this.move_selection_down(cx),
                "enter" => this.execute_selected(cx),
                "escape" => {
                    if !this.filter_text.is_empty() {
                        this.update_filter(None, false, true, cx);
                    } else {
                        // Update visibility state for hotkey toggle
                        WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                        logging::log("HOTKEY", "Window hidden via Escape key");
                        // PERF: Measure window hide latency
                        let hide_start = std::time::Instant::now();
                        cx.hide();
                        let hide_elapsed = hide_start.elapsed();
                        logging::log("PERF", &format!(
                            "Window hide (Escape) took {:.2}ms",
                            hide_elapsed.as_secs_f64() * 1000.0
                        ));
                    }
                }
                "backspace" => this.update_filter(None, true, false, cx),
                _ => {
                    if let Some(ref key_char) = event.keystroke.key_char {
                        if let Some(ch) = key_char.chars().next() {
                            if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
                                this.update_filter(Some(ch), false, false, cx);
                            }
                        }
                    }
                }
            }
        });

        // Main container with system font and transparency
        // Use theme opacity settings for background transparency
        let opacity = self.theme.get_opacity();
        
        // Use design tokens for background color (or theme for Default design)
        let bg_hex = if is_default_design {
            theme.colors.background.main
        } else {
            design_colors.background
        };
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        
        // Create box shadows from theme
        let box_shadows = self.create_box_shadows();
        
        // Use design tokens for border radius
        let border_radius = if is_default_design {
            12.0 // Default radius
        } else {
            design_visual.radius_lg
        };
        
        // Use design tokens for text color
        let text_primary = if is_default_design {
            theme.colors.text.primary
        } else {
            design_colors.text_primary
        };
        
        // Use design tokens for font family
        let font_family = if is_default_design {
            ".AppleSystemUIFont"
        } else {
            design_typography.font_family
        };
        
        let mut main_div = div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(border_radius))
            .text_color(rgb(text_primary))
            .font_family(font_family)
            .key_context("script_list")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header: Search Input + Run + Actions + Logo
            // Use design tokens for spacing and colors
            .child({
                // Design token values for header
                let header_padding_x = if is_default_design { 16.0 } else { design_spacing.padding_lg };
                let header_padding_y = if is_default_design { 8.0 } else { design_spacing.padding_sm };
                let header_gap = if is_default_design { 12.0 } else { design_spacing.gap_md };
                let text_muted = if is_default_design { theme.colors.text.muted } else { design_colors.text_muted };
                let text_dimmed = if is_default_design { theme.colors.text.dimmed } else { design_colors.text_dimmed };
                let accent_color = if is_default_design { theme.colors.accent.selected } else { design_colors.accent };
                
                div()
                    .w_full()
                    .px(px(header_padding_x))
                    .py(px(header_padding_y))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(header_gap))
                    // Search input with blinking cursor
                    // Cursor appears at LEFT when input is empty (before placeholder text)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if filter_is_empty { rgb(text_muted) } else { rgb(text_primary) })
                            // When empty: cursor FIRST (at left), then placeholder
                            // When typing: text, then cursor at end
                            // ALWAYS render cursor div to prevent layout shift, but only show bg when focused + visible
                            .when(filter_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .mr(px(design_spacing.padding_xs))
                                    .when(self.focused_input == FocusedInput::MainFilter && self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                            .child(filter_display)
                            .when(!filter_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .ml(px(design_visual.border_normal))
                                    .when(self.focused_input == FocusedInput::MainFilter && self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                    )
                    // CONDITIONAL: When NOT in actions mode, show Run button + Actions button
                    // When IN actions mode, show actions search input instead
                    .when(!self.show_actions_popup, |d| {
                        let button_colors = ButtonColors::from_theme(&self.theme);
                        let handle_run = cx.entity().downgrade();
                        let handle_actions = cx.entity().downgrade();
                        d
                            // Run button with click handler
                            .child(
                                Button::new("Run", button_colors)
                                    .variant(ButtonVariant::Ghost)
                                    .shortcut("↵")
                                    .on_click(Box::new(move |_, _window, cx| {
                                        if let Some(app) = handle_run.upgrade() {
                                            app.update(cx, |this, cx| {
                                                this.execute_selected(cx);
                                            });
                                        }
                                    }))
                            )
                            .child(div().text_color(rgb(text_dimmed)).child("|"))
                            // Actions button with click handler
                            .child(
                                Button::new("Actions", button_colors)
                                    .variant(ButtonVariant::Ghost)
                                    .shortcut("⌘ K")
                                    .on_click(Box::new(move |_, window, cx| {
                                        if let Some(app) = handle_actions.upgrade() {
                                            app.update(cx, |this, cx| {
                                                this.toggle_actions(cx, window);
                                            });
                                        }
                                    }))
                            )
                            .child(div().text_color(rgb(text_dimmed)).child("|"))
                    })
                    // CONDITIONAL: When IN actions mode, show actions search input
                    .when(self.show_actions_popup, |d| {
                        // Get actions search text from the dialog
                        let search_text = self.actions_dialog.as_ref()
                            .map(|dialog| dialog.read(cx).search_text.clone())
                            .unwrap_or_default();
                        let search_is_empty = search_text.is_empty();
                        let search_display = if search_is_empty {
                            SharedString::from("Search actions...")
                        } else {
                            SharedString::from(search_text)
                        };
                        
                        d.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                // ⌘K indicator
                                .child(
                                    div()
                                        .text_color(rgb(text_dimmed))
                                        .text_xs()
                                        .child("⌘K")
                                )
                                // Search input display
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .px(px(8.))
                                        .py(px(4.))
                                        .rounded(px(6.))
                                        .bg(rgba((theme.colors.background.search_box << 8) | 0x80))
                                        .border_1()
                                        .border_color(rgba((accent_color << 8) | 0x40))
                                        .text_sm()
                                        .text_color(if search_is_empty { rgb(text_muted) } else { rgb(text_primary) })
                                        // Cursor before placeholder when empty
                                        .when(search_is_empty, |d| d.child(
                                            div()
                                                .w(px(2.))
                                                .h(px(16.))
                                                .mr(px(4.))
                                                .rounded(px(1.))
                                                .when(self.focused_input == FocusedInput::ActionsSearch && self.cursor_visible, |d| d.bg(rgb(accent_color)))
                                        ))
                                        .child(search_display)
                                        // Cursor after text when not empty
                                        .when(!search_is_empty, |d| d.child(
                                            div()
                                                .w(px(2.))
                                                .h(px(16.))
                                                .ml(px(2.))
                                                .rounded(px(1.))
                                                .when(self.focused_input == FocusedInput::ActionsSearch && self.cursor_visible, |d| d.bg(rgb(accent_color)))
                                        ))
                                )
                                .child(div().text_color(rgb(text_dimmed)).child("|"))
                        )
                    })
                    // Script Kit Logo - ALWAYS visible
                    .child(
                        svg()
                            .external_path(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/logo.svg"))
                            .size(px(20.))
                            .text_color(rgb(accent_color))
                    )
            })
            // Subtle divider - semi-transparent
            // Use design tokens for border color and spacing
            .child({
                let divider_margin = if is_default_design { 16.0 } else { design_spacing.margin_lg };
                let border_color = if is_default_design { theme.colors.ui.border } else { design_colors.border };
                let border_width = if is_default_design { 1.0 } else { design_visual.border_thin };
                
                div()
                    .mx(px(divider_margin))
                    .h(px(border_width))
                    .bg(rgba((border_color << 8) | 0x60))
            });
        
        // Add error notification if present (at the top of the content area)
        if let Some(notification) = self.render_error_notification() {
            main_div = main_div.child(notification);
        }
        
        // Main content area - 50/50 split: List on left, Preview on right
        main_div = main_div
            // Uses min_h(px(0.)) to prevent flex children from overflowing
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))  // Critical: allows flex container to shrink properly
                    .w_full()
                    .overflow_hidden()
                    // Left side: Script list (50% width) - uses uniform_list for auto-scrolling
                    .child(
                        div()
                            .w_1_2()      // 50% width
                            .h_full()     // Take full height
                            .min_h(px(0.))  // Allow shrinking
                            .child(list_element)
                    )
                    // Right side: Preview panel (50% width)
                    // CONDITIONAL: Show actions dialog OR normal preview
                    .child(
                        div()
                            .w_1_2()      // 50% width
                            .h_full()     // Take full height
                            .min_h(px(0.))  // Allow shrinking
                            .overflow_hidden()
                            // When NOT in actions mode, show normal preview
                            .when(!self.show_actions_popup, |d| d.child(self.render_preview_panel(cx)))
                            // When IN actions mode, show actions dialog inline
                            .when_some(
                                if self.show_actions_popup { self.actions_dialog.clone() } else { None },
                                |d, dialog| d.child(dialog)
                            )
                    ),
            );
        
        if let Some(panel) = log_panel {
            main_div = main_div.child(panel);
        }
        
        // Wrap in relative container for toast overlay positioning
        let mut container = div()
            .relative()
            .w_full()
            .h_full()
            .child(main_div);
        
        // Add toast notifications overlay (top-right)
        if let Some(toasts) = self.render_toasts(cx) {
            container = container.child(toasts);
        }
        
        container.into_any_element()
    }
    
    fn render_arg_prompt(&mut self, id: String, placeholder: String, choices: Vec<Choice>, cx: &mut Context<Self>) -> AnyElement {
        let _theme = &self.theme;
        let _filtered = self.filtered_arg_choices();
        
        // Use design tokens for GLOBAL theming - all prompts use current design
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();
        
        // Key handler for arg prompt
        let prompt_id = id.clone();
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            logging::log("KEY", &format!("ArgPrompt key: '{}'", key_str));
            
            match key_str.as_str() {
                "up" | "arrowup" => {
                    if this.arg_selected_index > 0 {
                        this.arg_selected_index -= 1;
                        // P0: Scroll to keep selection visible
                        this.arg_list_scroll_handle.scroll_to_item(this.arg_selected_index, ScrollStrategy::Nearest);
                        logging::log_debug("SCROLL", &format!("P0: Arg up: selected_index={}", this.arg_selected_index));
                        cx.notify();
                    }
                }
                "down" | "arrowdown" => {
                    let filtered = this.filtered_arg_choices();
                    if this.arg_selected_index < filtered.len().saturating_sub(1) {
                        this.arg_selected_index += 1;
                        // P0: Scroll to keep selection visible
                        this.arg_list_scroll_handle.scroll_to_item(this.arg_selected_index, ScrollStrategy::Nearest);
                        logging::log_debug("SCROLL", &format!("P0: Arg down: selected_index={}", this.arg_selected_index));
                        cx.notify();
                    }
                }
                "enter" => {
                    let filtered = this.filtered_arg_choices();
                    if let Some((_, choice)) = filtered.get(this.arg_selected_index) {
                        let value = choice.value.clone();
                        this.submit_prompt_response(prompt_id.clone(), Some(value), cx);
                    }
                }
                "escape" => {
                    logging::log("KEY", "ESC in ArgPrompt - canceling script");
                    // Send cancel response and clean up fully
                    this.submit_prompt_response(prompt_id.clone(), None, cx);
                    this.cancel_script_execution(cx);
                }
                "backspace" => {
                    if !this.arg_input_text.is_empty() {
                        this.arg_input_text.pop();
                        this.arg_selected_index = 0;
                        this.update_window_size();
                        cx.notify();
                    }
                }
                _ => {
                    if let Some(ref key_char) = event.keystroke.key_char {
                        if let Some(ch) = key_char.chars().next() {
                            if !ch.is_control() {
                                this.arg_input_text.push(ch);
                                this.arg_selected_index = 0;
                                this.update_window_size();
                                cx.notify();
                            }
                        }
                    }
                }
            }
        });
        
        let input_display = if self.arg_input_text.is_empty() {
            SharedString::from(placeholder.clone())
        } else {
            SharedString::from(self.arg_input_text.clone())
        };
        let input_is_empty = self.arg_input_text.is_empty();
        
        // P4: Pre-compute theme values for arg prompt using design tokens for GLOBAL theming
        let arg_list_colors = ListItemColors::from_design(&design_colors);
        let text_primary = design_colors.text_primary;
        let text_muted = design_colors.text_muted;
        
        // P0: Clone data needed for uniform_list closure
        let arg_selected_index = self.arg_selected_index;
        let filtered_choices = self.get_filtered_arg_choices_owned();
        let filtered_choices_len = filtered_choices.len();
        logging::log_debug("UI", &format!("P0: Arg prompt has {} filtered choices", filtered_choices_len));
        
        // P0: Build virtualized choice list using uniform_list
        let list_element: AnyElement = if filtered_choices_len == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(design_colors.text_muted))
                .font_family(design_typography.font_family)
                .child("No choices match your filter")
                .into_any_element()
        } else {
            // P0: Use uniform_list for virtualized scrolling of arg choices
            // Now uses shared ListItem component for consistent design with script list
            uniform_list(
                "arg-choices",
                filtered_choices_len,
                move |visible_range, _window, _cx| {
                    logging::log_debug("SCROLL", &format!("P0: Arg choices visible range: {:?}", visible_range.clone()));
                    visible_range.map(|ix| {
                        if let Some((_, choice)) = filtered_choices.get(ix) {
                            let is_selected = ix == arg_selected_index;
                            
                            // Use shared ListItem component for consistent design
                            div()
                                .id(ix)
                                .child(
                                    ListItem::new(choice.name.clone(), arg_list_colors)
                                        .description_opt(choice.description.clone())
                                        .selected(is_selected)
                                )
                        } else {
                            div().id(ix).h(px(LIST_ITEM_HEIGHT))
                        }
                    }).collect()
                },
            )
            .h_full()
            .track_scroll(&self.arg_list_scroll_handle)
            .into_any_element()
        };
        
        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // P4: Pre-compute more theme values for the main container using design tokens
        let ui_border = design_colors.border;
        let text_dimmed = design_colors.text_dimmed;
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("arg_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Search input with blinking cursor (same as main menu)
                    // Cursor appears at LEFT when input is empty (before placeholder text)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if input_is_empty { rgb(text_muted) } else { rgb(text_primary) })
                            // When empty: cursor FIRST (at left), then placeholder
                            // When typing: text, then cursor at end
                            // ALWAYS render cursor div to prevent layout shift, but only show bg when focused + visible
                            .when(input_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .mr(px(design_spacing.padding_xs))
                                    .when(self.focused_input == FocusedInput::ArgPrompt && self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                            .child(input_display)
                            .when(!input_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .ml(px(design_visual.border_normal))
                                    .when(self.focused_input == FocusedInput::ArgPrompt && self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(format!("{} choices", choices.len()))
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(design_spacing.padding_lg))
                    .h(px(design_visual.border_thin))
                    .bg(rgba((ui_border << 8) | 0x60))
            )
            // P0: Choice list using virtualized uniform_list
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))  // P0: Allow flex container to shrink
                    .w_full()
                    .py(px(design_spacing.padding_xs))
                    .child(list_element)
            )
            // Footer
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_sm + design_visual.border_normal))  // 8 + 2 = 10px
                    .border_t_1()
                    .border_color(rgba((ui_border << 8) | 0x60))
                    .text_xs()
                    .text_color(rgb(text_muted))
                    .child("↑↓ navigate • ⏎ select • Esc cancel")
            )
            .into_any_element()
    }
    
    fn render_div_prompt(&mut self, id: String, html: String, _tailwind: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();
        
        // Strip HTML tags for plain text display
        let display_text = strip_html_tags(&html);
        
        let prompt_id = id.clone();
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            logging::log("KEY", &format!("DivPrompt key: '{}'", key_str));
            
            match key_str.as_str() {
                "enter" => {
                    // Enter continues the script (sends response)
                    logging::log("KEY", "Enter in DivPrompt - continuing script");
                    this.submit_prompt_response(prompt_id.clone(), None, cx);
                }
                "escape" => {
                    // ESC cancels the script completely
                    logging::log("KEY", "ESC in DivPrompt - canceling script");
                    this.submit_prompt_response(prompt_id.clone(), None, cx);
                    this.cancel_script_execution(cx);
                }
                _ => {}
            }
        });
        
        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // Use explicit height from layout constants instead of h_full()
        // DivPrompt uses STANDARD_HEIGHT (500px) to match main window
        let content_height = window_resize::layout::STANDARD_HEIGHT;
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h(content_height)
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(design_colors.text_primary))
            .font_family(design_typography.font_family)
            .key_context("div_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Content area
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.))  // Critical: allows flex children to size properly
                    .overflow_hidden()
                    .p(px(design_spacing.padding_xl))
                    .text_lg()
                    .child(display_text)
            )
            // Footer
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .border_t_1()
                    .border_color(rgba((design_colors.border << 8) | 0x60))
                    .text_xs()
                    .text_color(rgb(design_colors.text_muted))
                    .child("Press Enter or Escape to continue")
            )
            .into_any_element()
    }
    
    fn render_form_prompt(&mut self, id: String, html: String, cx: &mut Context<Self>) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();
        
        // Strip HTML tags for plain text display (same as DivPrompt for now)
        let display_text = strip_html_tags(&html);
        
        let prompt_id = id.clone();
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            logging::log("KEY", &format!("FormPrompt key: '{}'", key_str));
            
            match key_str.as_str() {
                "enter" => {
                    // Enter submits the form
                    logging::log("KEY", "Enter in FormPrompt - submitting form");
                    this.submit_prompt_response(prompt_id.clone(), Some("submitted".to_string()), cx);
                }
                "escape" => {
                    // ESC cancels the script completely
                    logging::log("KEY", "ESC in FormPrompt - canceling script");
                    this.submit_prompt_response(prompt_id.clone(), None, cx);
                    this.cancel_script_execution(cx);
                }
                _ => {}
            }
        });
        
        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // Use explicit height from layout constants instead of h_full()
        // FormPrompt uses STANDARD_HEIGHT (500px) to match main window
        let content_height = window_resize::layout::STANDARD_HEIGHT;
        
        // Button colors from theme
        let button_colors = ButtonColors::from_theme(&self.theme);
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h(content_height)
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(design_colors.text_primary))
            .font_family(design_typography.font_family)
            .key_context("form_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Content area
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.))  // Critical: allows flex children to size properly
                    .overflow_hidden()
                    .p(px(design_spacing.padding_xl))
                    .text_lg()
                    .child(display_text)
            )
            // Footer with Submit button
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .border_t_1()
                    .border_color(rgba((design_colors.border << 8) | 0x60))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    // Help text
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(design_colors.text_muted))
                            .child("⏎ submit • Esc cancel")
                    )
                    // Submit button
                    .child(
                        Button::new("Submit", button_colors)
                            .variant(ButtonVariant::Primary)
                    )
            )
            .into_any_element()
    }
    
    fn render_term_prompt(&mut self, entity: Entity<term_prompt::TermPrompt>, _cx: &mut Context<Self>) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        
        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // Use explicit height from layout constants instead of h_full()
        // h_full() doesn't work at the root level because there's no parent to fill
        let content_height = window_resize::layout::MAX_HEIGHT;
        
        // Container with explicit height. We wrap the entity in a sized div because
        // GPUI entities don't automatically inherit parent flex sizing.
        // NOTE: No rounded corners for terminal - it should fill edge-to-edge
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h(content_height)
            .overflow_hidden()
            .child(
                div()
                    .size_full()
                    .child(entity)
            )
            .into_any_element()
    }
    
    fn render_editor_prompt(&mut self, entity: Entity<EditorPrompt>, _cx: &mut Context<Self>) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_visual = tokens.visual();
        
        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // Use explicit height from layout constants instead of h_full()
        // h_full() doesn't work at the root level because there's no parent to fill
        let content_height = window_resize::layout::MAX_HEIGHT;
        
        // NOTE: The EditorPrompt entity has its own track_focus and on_key_down in its render method.
        // We do NOT add track_focus here to avoid duplicate focus tracking on the same handle.
        // 
        // Container with explicit height. We wrap the entity in a sized div because
        // GPUI entities don't automatically inherit parent flex sizing.
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h(content_height)
            .overflow_hidden()
            .rounded(px(design_visual.radius_lg))
            .child(
                div()
                    .size_full()
                    .child(entity)
            )
            .into_any_element()
    }
    
    fn render_actions_dialog(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();
        
        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // Key handler for actions dialog
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            logging::log("KEY", &format!("ActionsDialog key: '{}'", key_str));
            
            if key_str.as_str() == "escape" {
                logging::log("KEY", "ESC in ActionsDialog - returning to script list");
                this.current_view = AppView::ScriptList;
                cx.notify();
            }
        });
        
        // Simple actions dialog stub with design tokens
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .rounded(px(design_visual.radius_lg))
            .p(px(design_spacing.padding_xl))
            .text_color(rgb(design_colors.text_primary))
            .font_family(design_typography.font_family)
            .key_context("actions_dialog")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(
                div()
                    .text_lg()
                    .child("Actions (Cmd+K)")
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(design_colors.text_muted))
                    .mt(px(design_spacing.margin_md))
                    .child("• Create script\n• Edit script\n• Reload\n• Settings\n• Quit")
            )
            .child(
                div()
                    .mt(px(design_spacing.margin_lg))
                    .text_xs()
                    .text_color(rgb(design_colors.text_dimmed))
                    .child("Press Esc to close")
            )
            .into_any_element()
    }
    
    /// Render clipboard history view
    fn render_clipboard_history(
        &mut self,
        entries: Vec<clipboard_history::ClipboardEntry>,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();
        
        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // Pre-decode and cache images for visible entries
        // Do this ONCE before rendering, not inside the render closure
        for entry in &entries {
            if entry.content_type == clipboard_history::ContentType::Image 
               && !self.clipboard_image_cache.contains_key(&entry.id) {
                if let Some(render_image) = clipboard_history::decode_to_render_image(&entry.content) {
                    self.clipboard_image_cache.insert(entry.id.clone(), render_image);
                }
            }
        }
        
        // Clone the cache for use in closures
        let image_cache = self.clipboard_image_cache.clone();
        
        // Filter entries based on current filter
        let filtered_entries: Vec<_> = if filter.is_empty() {
            entries.iter().enumerate().collect()
        } else {
            let filter_lower = filter.to_lowercase();
            entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.content.to_lowercase().contains(&filter_lower)
                })
                .collect()
        };
        let filtered_len = filtered_entries.len();
        
        // Key handler for clipboard history
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            logging::log("KEY", &format!("ClipboardHistory key: '{}'", key_str));
            
            if let AppView::ClipboardHistoryView { entries, filter, selected_index } = &mut this.current_view {
                // Apply filter to get current filtered list
                let filtered_entries: Vec<_> = if filter.is_empty() {
                    entries.iter().enumerate().collect()
                } else {
                    let filter_lower = filter.to_lowercase();
                    entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.content.to_lowercase().contains(&filter_lower))
                        .collect()
                };
                let filtered_len = filtered_entries.len();
                
                match key_str.as_str() {
                    "up" | "arrowup" => {
                        if *selected_index > 0 {
                            *selected_index -= 1;
                            // Scroll to keep selection visible
                            this.clipboard_list_scroll_handle.scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            cx.notify();
                        }
                    }
                    "down" | "arrowdown" => {
                        if *selected_index < filtered_len.saturating_sub(1) {
                            *selected_index += 1;
                            // Scroll to keep selection visible
                            this.clipboard_list_scroll_handle.scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            cx.notify();
                        }
                    }
                    "enter" => {
                        // Copy selected entry to clipboard and hide window
                        if let Some((_, entry)) = filtered_entries.get(*selected_index) {
                            logging::log("EXEC", &format!("Copying clipboard entry: {}", entry.id));
                            if let Err(e) = clipboard_history::copy_entry_to_clipboard(&entry.id) {
                                logging::log("ERROR", &format!("Failed to copy entry: {}", e));
                            } else {
                                logging::log("EXEC", "Entry copied to clipboard");
                                // Hide window after pasting
                                WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                                cx.hide();
                                NEEDS_RESET.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                    "escape" => {
                        logging::log("KEY", "ESC in ClipboardHistory - returning to script list");
                        this.reset_to_script_list(cx);
                    }
                    "backspace" => {
                        if !filter.is_empty() {
                            filter.pop();
                            *selected_index = 0;
                            // Reset scroll to top when filter changes
                            this.clipboard_list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
                            cx.notify();
                        }
                    }
                    _ => {
                        if let Some(ref key_char) = event.keystroke.key_char {
                            if let Some(ch) = key_char.chars().next() {
                                if !ch.is_control() {
                                    filter.push(ch);
                                    *selected_index = 0;
                                    // Reset scroll to top when filter changes
                                    this.clipboard_list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
                                    cx.notify();
                                }
                            }
                        }
                    }
                }
            }
        });
        
        let input_display = if filter.is_empty() {
            SharedString::from("Search clipboard history...")
        } else {
            SharedString::from(filter.clone())
        };
        let input_is_empty = filter.is_empty();
        
        // Pre-compute colors
        let list_colors = ListItemColors::from_design(&design_colors);
        let text_primary = design_colors.text_primary;
        let text_muted = design_colors.text_muted;
        let text_dimmed = design_colors.text_dimmed;
        let ui_border = design_colors.border;
        
        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(design_colors.text_muted))
                .font_family(design_typography.font_family)
                .child(if filter.is_empty() { 
                    "No clipboard history" 
                } else { 
                    "No entries match your filter" 
                })
                .into_any_element()
        } else {
            // Clone data for the closure
            let entries_for_closure: Vec<_> = filtered_entries.iter().map(|(i, e)| (*i, (*e).clone())).collect();
            let selected = selected_index;
            let image_cache_for_list = image_cache.clone();
            
            uniform_list(
                "clipboard-history",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range.map(|ix| {
                        if let Some((_, entry)) = entries_for_closure.get(ix) {
                            let is_selected = ix == selected;
                            
                            // Get cached thumbnail for images
                            let cached_image = if entry.content_type == clipboard_history::ContentType::Image {
                                image_cache_for_list.get(&entry.id).cloned()
                            } else {
                                None
                            };
                            
                            // Truncate content for display (show dimensions for images)
                            let display_content = match entry.content_type {
                                clipboard_history::ContentType::Image => {
                                    // Show image dimensions instead of "[Image]"
                                    if let Some((w, h)) = clipboard_history::get_image_dimensions(&entry.content) {
                                        format!("{}×{} image", w, h)
                                    } else {
                                        "Image".to_string()
                                    }
                                }
                                clipboard_history::ContentType::Text => {
                                    let truncated: String = entry.content.chars().take(50).collect();
                                    if entry.content.len() > 50 {
                                        format!("{}...", truncated)
                                    } else {
                                        truncated
                                    }
                                }
                            };
                            
                            // Format relative time
                            let now = chrono::Utc::now().timestamp();
                            let age_secs = now - entry.timestamp;
                            let relative_time = if age_secs < 60 {
                                "just now".to_string()
                            } else if age_secs < 3600 {
                                format!("{}m ago", age_secs / 60)
                            } else if age_secs < 86400 {
                                format!("{}h ago", age_secs / 3600)
                            } else {
                                format!("{}d ago", age_secs / 86400)
                            };
                            
                            // Add pin indicator
                            let name = if entry.pinned {
                                format!("📌 {}", display_content)
                            } else {
                                display_content
                            };
                            
                            // Build list item with optional thumbnail
                            let mut item = ListItem::new(name, list_colors)
                                .description_opt(Some(relative_time))
                                .selected(is_selected);
                            
                            // Add thumbnail for images
                            if let Some(render_image) = cached_image {
                                item = item.icon_image(render_image);
                            }
                            
                            div()
                                .id(ix)
                                .child(item)
                        } else {
                            div().id(ix).h(px(LIST_ITEM_HEIGHT))
                        }
                    }).collect()
                },
            )
            .h_full()
            .track_scroll(&self.clipboard_list_scroll_handle)
            .into_any_element()
        };
        
        // Build preview panel for selected entry
        let selected_entry = filtered_entries.get(selected_index).map(|(_, e)| (*e).clone());
        let preview_panel = self.render_clipboard_preview_panel(&selected_entry, &image_cache, &design_colors, &design_spacing, &design_typography, &design_visual);
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("clipboard_history")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Search input with blinking cursor
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if input_is_empty { rgb(text_muted) } else { rgb(text_primary) })
                            .when(input_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .mr(px(design_spacing.padding_xs))
                                    .when(self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                            .child(input_display)
                            .when(!input_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .ml(px(design_visual.border_normal))
                                    .when(self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(format!("{} entries", entries.len()))
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(design_spacing.padding_lg))
                    .h(px(design_visual.border_thin))
                    .bg(rgba((ui_border << 8) | 0x60))
            )
            // Main content area - 50/50 split: List on left, Preview on right
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .overflow_hidden()
                    // Left side: Clipboard list (50% width)
                    .child(
                        div()
                            .w_1_2()
                            .h_full()
                            .min_h(px(0.))
                            .py(px(design_spacing.padding_xs))
                            .child(list_element)
                    )
                    // Right side: Preview panel (50% width)
                    .child(
                        div()
                            .w_1_2()
                            .h_full()
                            .min_h(px(0.))
                            .overflow_hidden()
                            .child(preview_panel)
                    )
            )
            .into_any_element()
    }
    
    /// Render the preview panel for clipboard history
    fn render_clipboard_preview_panel(
        &self,
        selected_entry: &Option<clipboard_history::ClipboardEntry>,
        image_cache: &std::collections::HashMap<String, Arc<gpui::RenderImage>>,
        colors: &designs::DesignColors,
        spacing: &designs::DesignSpacing,
        typography: &designs::DesignTypography,
        visual: &designs::DesignVisual,
    ) -> impl IntoElement {
        let bg_main = colors.background;
        let ui_border = colors.border;
        let text_primary = colors.text_primary;
        let text_muted = colors.text_muted;
        let text_secondary = colors.text_secondary;
        let bg_search_box = colors.background_tertiary;
        
        let mut panel = div()
            .w_full()
            .h_full()
            .bg(rgb(bg_main))
            .border_l_1()
            .border_color(rgba((ui_border << 8) | 0x80))
            .p(px(spacing.padding_lg))
            .flex()
            .flex_col()
            .overflow_y_hidden()
            .font_family(typography.font_family);
        
        match selected_entry {
            Some(entry) => {
                // Header with content type
                let content_type_label = match entry.content_type {
                    clipboard_history::ContentType::Text => "Text",
                    clipboard_history::ContentType::Image => "Image",
                };
                
                panel = panel.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .pb(px(spacing.padding_sm))
                        // Content type badge
                        .child(
                            div()
                                .px(px(spacing.padding_sm))
                                .py(px(spacing.padding_xs / 2.0))
                                .rounded(px(visual.radius_sm))
                                .bg(rgba((colors.accent << 8) | 0x30))
                                .text_xs()
                                .text_color(rgb(colors.accent))
                                .child(content_type_label)
                        )
                        // Pin indicator
                        .when(entry.pinned, |d| d.child(
                            div()
                                .px(px(spacing.padding_sm))
                                .py(px(spacing.padding_xs / 2.0))
                                .rounded(px(visual.radius_sm))
                                .bg(rgba((colors.accent << 8) | 0x20))
                                .text_xs()
                                .text_color(rgb(colors.accent))
                                .child("📌 Pinned")
                        ))
                );
                
                // Timestamp
                let now = chrono::Utc::now().timestamp();
                let age_secs = now - entry.timestamp;
                let relative_time = if age_secs < 60 {
                    "just now".to_string()
                } else if age_secs < 3600 {
                    format!("{} minutes ago", age_secs / 60)
                } else if age_secs < 86400 {
                    format!("{} hours ago", age_secs / 3600)
                } else {
                    format!("{} days ago", age_secs / 86400)
                };
                
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .pb(px(spacing.padding_md))
                        .child(relative_time)
                );
                
                // Divider
                panel = panel.child(
                    div()
                        .w_full()
                        .h(px(visual.border_thin))
                        .bg(rgba((ui_border << 8) | 0x60))
                        .my(px(spacing.padding_sm))
                );
                
                // Content preview
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .pb(px(spacing.padding_sm))
                        .child("Content Preview")
                );
                
                match entry.content_type {
                    clipboard_history::ContentType::Text => {
                        // Show full text content in a code-like container
                        let content = entry.content.clone();
                        let char_count = content.chars().count();
                        let line_count = content.lines().count();
                        
                        panel = panel
                            .child(
                                div()
                                    .w_full()
                                    .flex_1()
                                    .p(px(spacing.padding_md))
                                    .rounded(px(visual.radius_md))
                                    .bg(rgba((bg_search_box << 8) | 0x80))
                                    .overflow_hidden()
                                    .font_family(typography.font_family_mono)
                                    .text_sm()
                                    .text_color(rgb(text_primary))
                                    .child(content)
                            )
                            // Stats footer
                            .child(
                                div()
                                    .pt(px(spacing.padding_sm))
                                    .text_xs()
                                    .text_color(rgb(text_secondary))
                                    .child(format!("{} characters • {} lines", char_count, line_count))
                            );
                    }
                    clipboard_history::ContentType::Image => {
                        // Get image dimensions
                        let (width, height) = clipboard_history::get_image_dimensions(&entry.content)
                            .unwrap_or((0, 0));
                        
                        // Try to get cached render image
                        let cached_image = image_cache.get(&entry.id).cloned();
                        
                        let image_container = if let Some(render_image) = cached_image {
                            // Calculate display size that fits in the preview panel
                            // Max size is 300x300, maintain aspect ratio
                            let max_size: f32 = 300.0;
                            let (display_w, display_h) = if width > 0 && height > 0 {
                                let w = width as f32;
                                let h = height as f32;
                                let scale = (max_size / w).min(max_size / h).min(1.0);
                                (w * scale, h * scale)
                            } else {
                                (max_size, max_size)
                            };
                            
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                // Actual image thumbnail
                                .child(
                                    gpui::img(move |_window: &mut Window, _cx: &mut App| {
                                        Some(Ok(render_image.clone()))
                                    })
                                    .w(px(display_w))
                                    .h(px(display_h))
                                    .object_fit(gpui::ObjectFit::Contain)
                                    .rounded(px(visual.radius_sm))
                                )
                                // Dimensions label below image
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_secondary))
                                        .child(format!("{}×{} pixels", width, height))
                                )
                        } else {
                            // Fallback if image not in cache (shouldn't happen)
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_2xl()
                                        .child("🖼️")
                                )
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(text_primary))
                                        .child(format!("{}×{}", width, height))
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(text_muted))
                                        .child("Loading image...")
                                )
                        };
                        
                        panel = panel.child(
                            div()
                                .w_full()
                                .flex_1()
                                .p(px(spacing.padding_lg))
                                .rounded(px(visual.radius_md))
                                .bg(rgba((bg_search_box << 8) | 0x80))
                                .flex()
                                .items_center()
                                .justify_center()
                                .overflow_hidden()
                                .child(image_container)
                        );
                    }
                }
            }
            None => {
                // Empty state
                panel = panel.child(
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(text_muted))
                        .child("No entry selected")
                );
            }
        }
        
        panel
    }
    
    /// Render app launcher view
    fn render_app_launcher(
        &mut self,
        apps: Vec<app_launcher::AppInfo>,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();
        
        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // Filter apps based on current filter
        let filtered_apps: Vec<_> = if filter.is_empty() {
            apps.iter().enumerate().collect()
        } else {
            let filter_lower = filter.to_lowercase();
            apps
                .iter()
                .enumerate()
                .filter(|(_, a)| a.name.to_lowercase().contains(&filter_lower))
                .collect()
        };
        let filtered_len = filtered_apps.len();
        
        // Key handler for app launcher
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            logging::log("KEY", &format!("AppLauncher key: '{}'", key_str));
            
            if let AppView::AppLauncherView { apps, filter, selected_index } = &mut this.current_view {
                // Apply filter to get current filtered list
                let filtered_apps: Vec<_> = if filter.is_empty() {
                    apps.iter().enumerate().collect()
                } else {
                    let filter_lower = filter.to_lowercase();
                    apps
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| a.name.to_lowercase().contains(&filter_lower))
                        .collect()
                };
                let filtered_len = filtered_apps.len();
                
                match key_str.as_str() {
                    "up" | "arrowup" => {
                        if *selected_index > 0 {
                            *selected_index -= 1;
                            cx.notify();
                        }
                    }
                    "down" | "arrowdown" => {
                        if *selected_index < filtered_len.saturating_sub(1) {
                            *selected_index += 1;
                            cx.notify();
                        }
                    }
                    "enter" => {
                        // Launch selected app and hide window
                        if let Some((_, app)) = filtered_apps.get(*selected_index) {
                            logging::log("EXEC", &format!("Launching app: {}", app.name));
                            if let Err(e) = app_launcher::launch_application(app) {
                                logging::log("ERROR", &format!("Failed to launch app: {}", e));
                            } else {
                                logging::log("EXEC", &format!("Launched: {}", app.name));
                                // Hide window after launching
                                WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                                cx.hide();
                                NEEDS_RESET.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                    "escape" => {
                        logging::log("KEY", "ESC in AppLauncher - returning to script list");
                        this.reset_to_script_list(cx);
                    }
                    "backspace" => {
                        if !filter.is_empty() {
                            filter.pop();
                            *selected_index = 0;
                            cx.notify();
                        }
                    }
                    _ => {
                        if let Some(ref key_char) = event.keystroke.key_char {
                            if let Some(ch) = key_char.chars().next() {
                                if !ch.is_control() {
                                    filter.push(ch);
                                    *selected_index = 0;
                                    cx.notify();
                                }
                            }
                        }
                    }
                }
            }
        });
        
        let input_display = if filter.is_empty() {
            SharedString::from("Search applications...")
        } else {
            SharedString::from(filter.clone())
        };
        let input_is_empty = filter.is_empty();
        
        // Pre-compute colors
        let list_colors = ListItemColors::from_design(&design_colors);
        let text_primary = design_colors.text_primary;
        let text_muted = design_colors.text_muted;
        let text_dimmed = design_colors.text_dimmed;
        let ui_border = design_colors.border;
        
        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(design_colors.text_muted))
                .font_family(design_typography.font_family)
                .child(if filter.is_empty() { 
                    "No applications found" 
                } else { 
                    "No apps match your filter" 
                })
                .into_any_element()
        } else {
            // Clone data for the closure
            let apps_for_closure: Vec<_> = filtered_apps.iter().map(|(i, a)| (*i, (*a).clone())).collect();
            let selected = selected_index;
            
            uniform_list(
                "app-launcher",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range.map(|ix| {
                        if let Some((_, app)) = apps_for_closure.get(ix) {
                            let is_selected = ix == selected;
                            
                            // Format app path for description
                            let path_str = app.path.to_string_lossy();
                            let description = if path_str.starts_with("/Applications") {
                                None // No need to show path for standard apps
                            } else {
                                Some(path_str.to_string())
                            };
                            
                            // Use pre-decoded icon if available, fallback to emoji
                            let icon = match &app.icon {
                                Some(img) => list_item::IconKind::Image(img.clone()),
                                None => list_item::IconKind::Emoji("📱".to_string()),
                            };
                            
                            div()
                                .id(ix)
                                .child(
                                    ListItem::new(app.name.clone(), list_colors)
                                        .icon_kind(icon)
                                        .description_opt(description)
                                        .selected(is_selected)
                                )
                        } else {
                            div().id(ix).h(px(LIST_ITEM_HEIGHT))
                        }
                    }).collect()
                },
            )
            .h_full()
            .track_scroll(&self.list_scroll_handle)
            .into_any_element()
        };
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("app_launcher")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Title
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child("🚀 Apps")
                    )
                    // Search input with blinking cursor
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if input_is_empty { rgb(text_muted) } else { rgb(text_primary) })
                            .when(input_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .mr(px(design_spacing.padding_xs))
                                    .when(self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                            .child(input_display)
                            .when(!input_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .ml(px(design_visual.border_normal))
                                    .when(self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(format!("{} apps", apps.len()))
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(design_spacing.padding_lg))
                    .h(px(design_visual.border_thin))
                    .bg(rgba((ui_border << 8) | 0x60))
            )
            // App list
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .py(px(design_spacing.padding_xs))
                    .child(list_element)
            )
            // Footer
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_sm + design_visual.border_normal))
                    .border_t_1()
                    .border_color(rgba((ui_border << 8) | 0x60))
                    .text_xs()
                    .text_color(rgb(text_muted))
                    .child("↑↓ navigate • ⏎ launch • Esc back")
            )
            .into_any_element()
    }
    
    /// Render window switcher view with 50/50 split layout
    fn render_window_switcher(
        &mut self,
        windows: Vec<window_control::WindowInfo>,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Use design tokens for GLOBAL theming
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();
        
        // Use design tokens for global theming
        let opacity = self.theme.get_opacity();
        let bg_hex = design_colors.background;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // Filter windows based on current filter
        let filtered_windows: Vec<_> = if filter.is_empty() {
            windows.iter().enumerate().collect()
        } else {
            let filter_lower = filter.to_lowercase();
            windows
                .iter()
                .enumerate()
                .filter(|(_, w)| {
                    w.title.to_lowercase().contains(&filter_lower) ||
                    w.app.to_lowercase().contains(&filter_lower)
                })
                .collect()
        };
        let filtered_len = filtered_windows.len();
        
        // Key handler for window switcher
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            logging::log("KEY", &format!("WindowSwitcher key: '{}'", key_str));
            
            if let AppView::WindowSwitcherView { windows, filter, selected_index } = &mut this.current_view {
                // Apply filter to get current filtered list
                let filtered_windows: Vec<_> = if filter.is_empty() {
                    windows.iter().enumerate().collect()
                } else {
                    let filter_lower = filter.to_lowercase();
                    windows
                        .iter()
                        .enumerate()
                        .filter(|(_, w)| {
                            w.title.to_lowercase().contains(&filter_lower) ||
                            w.app.to_lowercase().contains(&filter_lower)
                        })
                        .collect()
                };
                let filtered_len = filtered_windows.len();
                
                match key_str.as_str() {
                    "up" | "arrowup" => {
                        if *selected_index > 0 {
                            *selected_index -= 1;
                            this.window_list_scroll_handle.scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            cx.notify();
                        }
                    }
                    "down" | "arrowdown" => {
                        if *selected_index < filtered_len.saturating_sub(1) {
                            *selected_index += 1;
                            this.window_list_scroll_handle.scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            cx.notify();
                        }
                    }
                    "enter" => {
                        // Focus selected window and hide Script Kit
                        if let Some((_, window_info)) = filtered_windows.get(*selected_index) {
                            logging::log("EXEC", &format!("Focusing window: {}", window_info.title));
                            if let Err(e) = window_control::focus_window(window_info.id) {
                                logging::log("ERROR", &format!("Failed to focus window: {}", e));
                                this.toast_manager.push(
                                    components::toast::Toast::error(
                                        format!("Failed to focus window: {}", e),
                                        &this.theme
                                    ).duration_ms(Some(5000))
                                );
                                cx.notify();
                            } else {
                                logging::log("EXEC", &format!("Focused window: {}", window_info.title));
                                WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                                cx.hide();
                                NEEDS_RESET.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                    "escape" => {
                        logging::log("KEY", "ESC in WindowSwitcher - returning to script list");
                        this.reset_to_script_list(cx);
                    }
                    "backspace" => {
                        if !filter.is_empty() {
                            filter.pop();
                            *selected_index = 0;
                            this.window_list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
                            cx.notify();
                        }
                    }
                    // Number keys for quick window actions - extract window_id to avoid borrow issues
                    "1" | "2" | "3" | "4" | "m" | "n" | "c" => {
                        if let Some((_, window_info)) = filtered_windows.get(*selected_index) {
                            let window_id = window_info.id;
                            let action = match key_str.as_str() {
                                "1" => "tile_left",
                                "2" => "tile_right",
                                "3" => "tile_top",
                                "4" => "tile_bottom",
                                "m" => "maximize",
                                "n" => "minimize",
                                "c" => "close",
                                _ => unreachable!(),
                            };
                            // Drop the borrow before calling execute_window_action
                            drop(filtered_windows);
                            this.execute_window_action(window_id, action, cx);
                        }
                    }
                    _ => {
                        if let Some(ref key_char) = event.keystroke.key_char {
                            if let Some(ch) = key_char.chars().next() {
                                if !ch.is_control() && ch.is_alphanumeric() {
                                    filter.push(ch);
                                    *selected_index = 0;
                                    this.window_list_scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
                                    cx.notify();
                                }
                            }
                        }
                    }
                }
            }
        });
        
        let input_display = if filter.is_empty() {
            SharedString::from("Search windows...")
        } else {
            SharedString::from(filter.clone())
        };
        let input_is_empty = filter.is_empty();
        
        // Pre-compute colors
        let list_colors = ListItemColors::from_design(&design_colors);
        let text_primary = design_colors.text_primary;
        let text_muted = design_colors.text_muted;
        let text_dimmed = design_colors.text_dimmed;
        let ui_border = design_colors.border;
        
        // Build virtualized list
        let list_element: AnyElement = if filtered_len == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(design_colors.text_muted))
                .font_family(design_typography.font_family)
                .child(if filter.is_empty() { 
                    "No windows found" 
                } else { 
                    "No windows match your filter" 
                })
                .into_any_element()
        } else {
            // Clone data for the closure
            let windows_for_closure: Vec<_> = filtered_windows.iter().map(|(i, w)| (*i, (*w).clone())).collect();
            let selected = selected_index;
            
            uniform_list(
                "window-switcher",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range.map(|ix| {
                        if let Some((_, window_info)) = windows_for_closure.get(ix) {
                            let is_selected = ix == selected;
                            
                            // Format: "AppName: Window Title"
                            let name = format!("{}: {}", window_info.app, window_info.title);
                            
                            // Format bounds as description
                            let description = format!(
                                "{}×{} at ({}, {})",
                                window_info.bounds.width,
                                window_info.bounds.height,
                                window_info.bounds.x,
                                window_info.bounds.y
                            );
                            
                            div()
                                .id(ix)
                                .child(
                                    ListItem::new(name, list_colors)
                                        .description_opt(Some(description))
                                        .selected(is_selected)
                                )
                        } else {
                            div().id(ix).h(px(LIST_ITEM_HEIGHT))
                        }
                    }).collect()
                },
            )
            .h_full()
            .track_scroll(&self.window_list_scroll_handle)
            .into_any_element()
        };
        
        // Build actions panel for selected window
        let selected_window = filtered_windows.get(selected_index).map(|(_, w)| (*w).clone());
        let actions_panel = self.render_window_actions_panel(&selected_window, &design_colors, &design_spacing, &design_typography, &design_visual, cx);
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .key_context("window_switcher")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Title
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child("🪟 Windows")
                    )
                    // Search input with blinking cursor
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_lg()
                            .text_color(if input_is_empty { rgb(text_muted) } else { rgb(text_primary) })
                            .when(input_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .mr(px(design_spacing.padding_xs))
                                    .when(self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                            .child(input_display)
                            .when(!input_is_empty, |d| d.child(
                                div()
                                    .w(px(design_visual.border_normal))
                                    .h(px(design_typography.font_size_lg * design_typography.line_height_normal))
                                    .ml(px(design_visual.border_normal))
                                    .when(self.cursor_visible, |d| d.bg(rgb(text_primary)))
                            ))
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(format!("{} windows", windows.len()))
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(design_spacing.padding_lg))
                    .h(px(design_visual.border_thin))
                    .bg(rgba((ui_border << 8) | 0x60))
            )
            // Main content area - 50/50 split: Window list on left, Actions on right
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .overflow_hidden()
                    // Left side: Window list (50% width)
                    .child(
                        div()
                            .w_1_2()
                            .h_full()
                            .min_h(px(0.))
                            .py(px(design_spacing.padding_xs))
                            .child(list_element)
                    )
                    // Right side: Actions panel (50% width)
                    .child(
                        div()
                            .w_1_2()
                            .h_full()
                            .min_h(px(0.))
                            .overflow_hidden()
                            .child(actions_panel)
                    )
            )
            // Footer
            .child(
                div()
                    .w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_sm + design_visual.border_normal))
                    .border_t_1()
                    .border_color(rgba((ui_border << 8) | 0x60))
                    .text_xs()
                    .text_color(rgb(text_muted))
                    .child("↑↓ navigate • ⏎ focus • 1-4 tile • M max • N min • C close • Esc back")
            )
            .into_any_element()
    }
    
    /// Render the actions panel for window switcher
    fn render_window_actions_panel(
        &self,
        selected_window: &Option<window_control::WindowInfo>,
        colors: &designs::DesignColors,
        spacing: &designs::DesignSpacing,
        typography: &designs::DesignTypography,
        visual: &designs::DesignVisual,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bg_main = colors.background;
        let ui_border = colors.border;
        let text_primary = colors.text_primary;
        let text_muted = colors.text_muted;
        let text_secondary = colors.text_secondary;
        
        let mut panel = div()
            .w_full()
            .h_full()
            .bg(rgb(bg_main))
            .border_l_1()
            .border_color(rgba((ui_border << 8) | 0x80))
            .p(px(spacing.padding_lg))
            .flex()
            .flex_col()
            .overflow_y_hidden()
            .font_family(typography.font_family);
        
        match selected_window {
            Some(window) => {
                // Window info header
                panel = panel.child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(text_primary))
                        .pb(px(spacing.padding_sm))
                        .child(window.title.clone())
                );
                
                // App name
                panel = panel.child(
                    div()
                        .text_sm()
                        .text_color(rgb(text_secondary))
                        .pb(px(spacing.padding_md))
                        .child(window.app.clone())
                );
                
                // Bounds info
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .pb(px(spacing.padding_lg))
                        .child(format!(
                            "{}×{} at ({}, {})",
                            window.bounds.width,
                            window.bounds.height,
                            window.bounds.x,
                            window.bounds.y
                        ))
                );
                
                // Divider
                panel = panel.child(
                    div()
                        .w_full()
                        .h(px(visual.border_thin))
                        .bg(rgba((ui_border << 8) | 0x60))
                        .mb(px(spacing.padding_lg))
                );
                
                // Actions header
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .pb(px(spacing.padding_md))
                        .child("Actions (keyboard shortcuts)")
                );
                
                // Action buttons grid - using text labels with shortcuts
                let action_items = [
                    ("1", "← Tile Left Half"),
                    ("2", "→ Tile Right Half"),
                    ("3", "↑ Tile Top Half"),
                    ("4", "↓ Tile Bottom Half"),
                    ("M", "□ Maximize"),
                    ("N", "_ Minimize"),
                    ("⏎", "◉ Focus"),
                    ("C", "✕ Close"),
                ];
                
                for (key, label) in action_items {
                    panel = panel.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .py(px(spacing.padding_xs))
                            // Key badge
                            .child(
                                div()
                                    .w(px(24.0))
                                    .h(px(20.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(visual.radius_sm))
                                    .bg(rgba((colors.background_tertiary << 8) | 0x80))
                                    .text_xs()
                                    .text_color(rgb(text_secondary))
                                    .child(key)
                            )
                            // Label
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(text_primary))
                                    .child(label)
                            )
                    );
                }
            }
            None => {
                // Empty state
                panel = panel.child(
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(text_muted))
                        .child("No window selected")
                );
            }
        }
        
        panel
    }
    
    /// Execute a window action (tile, maximize, minimize, close)
    fn execute_window_action(&mut self, window_id: u32, action: &str, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Window action: {} on window {}", action, window_id));
        
        let result = match action {
            "tile_left" => window_control::tile_window(window_id, window_control::TilePosition::LeftHalf),
            "tile_right" => window_control::tile_window(window_id, window_control::TilePosition::RightHalf),
            "tile_top" => window_control::tile_window(window_id, window_control::TilePosition::TopHalf),
            "tile_bottom" => window_control::tile_window(window_id, window_control::TilePosition::BottomHalf),
            "maximize" => window_control::maximize_window(window_id),
            "minimize" => window_control::minimize_window(window_id),
            "close" => window_control::close_window(window_id),
            "focus" => window_control::focus_window(window_id),
            _ => {
                logging::log("ERROR", &format!("Unknown window action: {}", action));
                return;
            }
        };
        
        match result {
            Ok(()) => {
                logging::log("EXEC", &format!("Window action {} succeeded", action));
                
                // Show success toast
                self.toast_manager.push(
                    components::toast::Toast::success(
                        format!("Window {}", action.replace("_", " ")),
                        &self.theme
                    ).duration_ms(Some(2000))
                );
                
                // Refresh window list after action
                if let AppView::WindowSwitcherView { windows, selected_index, .. } = &mut self.current_view {
                    match window_control::list_windows() {
                        Ok(new_windows) => {
                            *windows = new_windows;
                            // Adjust selected index if needed
                            if *selected_index >= windows.len() && !windows.is_empty() {
                                *selected_index = windows.len() - 1;
                            }
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to refresh windows: {}", e));
                        }
                    }
                }
            }
            Err(e) => {
                logging::log("ERROR", &format!("Window action {} failed: {}", action, e));
                
                // Show error toast
                self.toast_manager.push(
                    components::toast::Toast::error(
                        format!("Failed to {}: {}", action.replace("_", " "), e),
                        &self.theme
                    ).duration_ms(Some(5000))
                );
            }
        }
        
        cx.notify();
    }
}

fn start_hotkey_listener(config: config::Config) {
    std::thread::spawn(move || {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                logging::log("HOTKEY", &format!("Failed to create hotkey manager: {}", e));
                return;
            }
        };
        
        // Convert config hotkey to global_hotkey::Code
        let code = match config.hotkey.key.as_str() {
            "Semicolon" => Code::Semicolon,
            "KeyK" => Code::KeyK,
            "KeyP" => Code::KeyP,
            "Space" => Code::Space,
            "Enter" => Code::Enter,
            "Digit0" => Code::Digit0,
            "Digit1" => Code::Digit1,
            "Digit2" => Code::Digit2,
            "Digit3" => Code::Digit3,
            "Digit4" => Code::Digit4,
            "Digit5" => Code::Digit5,
            "Digit6" => Code::Digit6,
            "Digit7" => Code::Digit7,
            "Digit8" => Code::Digit8,
            "Digit9" => Code::Digit9,
            "KeyA" => Code::KeyA,
            "KeyB" => Code::KeyB,
            "KeyC" => Code::KeyC,
            "KeyD" => Code::KeyD,
            "KeyE" => Code::KeyE,
            "KeyF" => Code::KeyF,
            "KeyG" => Code::KeyG,
            "KeyH" => Code::KeyH,
            "KeyI" => Code::KeyI,
            "KeyJ" => Code::KeyJ,
            "KeyL" => Code::KeyL,
            "KeyM" => Code::KeyM,
            "KeyN" => Code::KeyN,
            "KeyO" => Code::KeyO,
            "KeyQ" => Code::KeyQ,
            "KeyR" => Code::KeyR,
            "KeyS" => Code::KeyS,
            "KeyT" => Code::KeyT,
            "KeyU" => Code::KeyU,
            "KeyV" => Code::KeyV,
            "KeyW" => Code::KeyW,
            "KeyX" => Code::KeyX,
            "KeyY" => Code::KeyY,
            "KeyZ" => Code::KeyZ,
            // Function keys
            "F1" => Code::F1,
            "F2" => Code::F2,
            "F3" => Code::F3,
            "F4" => Code::F4,
            "F5" => Code::F5,
            "F6" => Code::F6,
            "F7" => Code::F7,
            "F8" => Code::F8,
            "F9" => Code::F9,
            "F10" => Code::F10,
            "F11" => Code::F11,
            "F12" => Code::F12,
            other => {
                logging::log("HOTKEY", &format!(
                    "Unknown key code: '{}'. Valid keys: KeyA-KeyZ, Digit0-Digit9, F1-F12, Space, Enter, Semicolon. Falling back to Semicolon",
                    other
                ));
                Code::Semicolon
            }
        };
        
        // Convert modifiers from config strings to Modifiers flags
        let mut modifiers = Modifiers::empty();
        for modifier in &config.hotkey.modifiers {
            match modifier.as_str() {
                "meta" => modifiers |= Modifiers::META,
                "ctrl" => modifiers |= Modifiers::CONTROL,
                "alt" => modifiers |= Modifiers::ALT,
                "shift" => modifiers |= Modifiers::SHIFT,
                other => {
                    logging::log("HOTKEY", &format!("Unknown modifier: {}", other));
                }
            }
        }
        
        let hotkey = HotKey::new(Some(modifiers), code);
        let hotkey_id = hotkey.id();
        
        let hotkey_display = format!(
            "{}{}",
            config.hotkey.modifiers.join("+"),
            if config.hotkey.modifiers.is_empty() { String::new() } else { "+".to_string() }
        ) + &config.hotkey.key;
        
        if let Err(e) = manager.register(hotkey) {
            logging::log("HOTKEY", &format!("Failed to register {}: {}", hotkey_display, e));
            return;
        }
        
        logging::log("HOTKEY", &format!("Registered global hotkey {} (id: {})", hotkey_display, hotkey_id));
        
        let receiver = GlobalHotKeyEvent::receiver();
        
        loop {
            if let Ok(event) = receiver.recv() {
                // Only respond to key PRESS, not release
                // This prevents double-triggering on a single key press
                if event.id == hotkey_id && event.state == HotKeyState::Pressed {
                    let count = HOTKEY_TRIGGER_COUNT.fetch_add(1, Ordering::SeqCst);
                    // Send via async_channel for immediate event-driven handling (replaces AtomicBool polling)
                    if hotkey_channel().0.send_blocking(()).is_err() {
                        logging::log("HOTKEY", "Hotkey channel closed, cannot send");
                    }
                    logging::log("HOTKEY", &format!("{} pressed (trigger #{})", hotkey_display, count + 1));
                } else if event.id == hotkey_id && event.state == HotKeyState::Released {
                    // Ignore key release events - just log for debugging
                    logging::log("HOTKEY", &format!("{} released (ignored)", hotkey_display));
                }
            }
        }
    });
}

/// Ensure the window has MoveToActiveSpace collection behavior.
/// MUST be called BEFORE any window activation/ordering.
/// This makes the window move to the current space rather than forcing a space switch.
#[cfg(target_os = "macos")]
fn ensure_move_to_active_space() {
    unsafe {
        // Use WindowManager to get the main window (not keyWindow, which may not exist yet)
        let window = match window_manager::get_main_window() {
            Some(w) => w,
            None => {
                logging::log(
                    "PANEL",
                    "WARNING: Main window not registered, cannot set MoveToActiveSpace",
                );
                return;
            }
        };

        // NSWindowCollectionBehaviorMoveToActiveSpace = (1 << 1) = 2
        // This makes the window MOVE to the current active space when shown
        let collection_behavior: u64 = 2;
        let _: () = msg_send![window, setCollectionBehavior:collection_behavior];

        logging::log(
            "PANEL",
            "Set MoveToActiveSpace collection behavior (before activation)",
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn ensure_move_to_active_space() {}

/// Configure the current window as a floating macOS panel that appears above other apps
#[cfg(target_os = "macos")]
fn configure_as_floating_panel() {
    unsafe {
        let app: id = NSApp();

        // Get the key window (the most recently activated window)
        let window: id = msg_send![app, keyWindow];

        if window != nil {
            // NSFloatingWindowLevel = 3
            // This makes the window float above normal windows
            let floating_level: i32 = 3;
            let _: () = msg_send![window, setLevel:floating_level];

            // NSWindowCollectionBehaviorMoveToActiveSpace = (1 << 1)
            // This makes the window MOVE to the current active space when shown
            // (instead of forcing user back to the space where window was last visible)
            let collection_behavior: u64 = 2;
            let _: () = msg_send![window, setCollectionBehavior:collection_behavior];
            
            // CRITICAL: Disable macOS window state restoration
            // This prevents macOS from remembering and restoring the window position
            // when the app is relaunched or the window is shown again
            let _: () = msg_send![window, setRestorable:false];
            
            // Also disable the window's autosave frame name which can cause position caching
            let empty_string: id = msg_send![class!(NSString), string];
            let _: () = msg_send![window, setFrameAutosaveName:empty_string];

            logging::log(
                "PANEL",
                "Configured window as floating panel (level=3, MoveToActiveSpace, restorable=false, no autosave)",
            );
        } else {
            logging::log("PANEL", "Warning: No key window found to configure as panel");
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_as_floating_panel() {}

fn start_hotkey_event_handler(cx: &mut App, window: WindowHandle<ScriptListApp>) {
    let handler = cx.new(|_| HotkeyPoller::new(window));
    handler.update(cx, |p, cx| {
        p.start_listening(cx);
    });
}

fn main() {
    logging::init();
    
    // Initialize clipboard history monitoring (background thread)
    if let Err(e) = clipboard_history::init_clipboard_history() {
        logging::log("APP", &format!("Failed to initialize clipboard history: {}", e));
    } else {
        logging::log("APP", "Clipboard history monitoring initialized");
    }
    
    // Load config early so we can use it for hotkey registration
    let loaded_config = config::load_config();
    logging::log("APP", &format!("Loaded config: hotkey={:?}+{}, bun_path={:?}", 
        loaded_config.hotkey.modifiers, loaded_config.hotkey.key, loaded_config.bun_path));
    
    start_hotkey_listener(loaded_config);
    
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
    Application::new().run(move |cx: &mut App| {
        logging::log("APP", "GPUI Application starting");
        
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
        let bounds = calculate_eye_line_bounds_on_mouse_display(window_size, cx);
        
        let window: WindowHandle<ScriptListApp> = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                is_movable: true,
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |_, cx| {
                logging::log("APP", "Window opened, creating ScriptListApp");
                cx.new(ScriptListApp::new)
            },
        )
        .unwrap();
        
        window
            .update(cx, |view: &mut ScriptListApp, window: &mut Window, cx: &mut Context<ScriptListApp>| {
                let focus_handle = view.focus_handle(cx);
                window.focus(&focus_handle, cx);
                logging::log("APP", "Focus set on ScriptListApp");
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
        
        start_hotkey_event_handler(cx, window);
        
        // Appearance change watcher - event-driven with async_channel
        let window_for_appearance = window;
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            // Event-driven: blocks until appearance change event received
            while let Ok(_event) = appearance_rx.recv().await {
                logging::log("APP", "System appearance changed, updating theme");
                let _ = cx.update(|cx| {
                    let _ = window_for_appearance.update(cx, |view: &mut ScriptListApp, _window: &mut Window, ctx: &mut Context<ScriptListApp>| {
                        view.update_theme(ctx);
                    });
                });
            }
            logging::log("APP", "Appearance watcher channel closed");
        }).detach();
        
        // Config reload watcher - watches ~/.kenv/config.ts for changes
        let window_for_config = window;
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            loop {
                Timer::after(std::time::Duration::from_millis(200)).await;
                
                if config_rx.try_recv().is_ok() {
                    logging::log("APP", "Config file changed, reloading");
                    let _ = cx.update(|cx| {
                        let _ = window_for_config.update(cx, |view: &mut ScriptListApp, _window: &mut Window, ctx: &mut Context<ScriptListApp>| {
                            view.update_config(ctx);
                        });
                    });
                }
            }
        }).detach();
        
        // Script/scriptlets reload watcher - watches ~/.kenv/scripts/ and ~/.kenv/scriptlets/
        let window_for_scripts = window;
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            loop {
                Timer::after(std::time::Duration::from_millis(200)).await;
                
                if script_rx.try_recv().is_ok() {
                    logging::log("APP", "Scripts or scriptlets changed, reloading");
                    let _ = cx.update(|cx| {
                        let _ = window_for_scripts.update(cx, |view: &mut ScriptListApp, _window: &mut Window, ctx: &mut Context<ScriptListApp>| {
                            view.refresh_scripts(ctx);
                        });
                    });
                }
            }
        }).detach();
        
        // NOTE: Prompt message listener is now spawned per-script in execute_interactive()
        // using event-driven async_channel instead of 50ms polling
        
        // Test command file watcher - allows smoke tests to trigger script execution
        let window_for_test = window;
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
                                let _ = cx.update(|cx| {
                                    let _ = window_for_test.update(cx, |view: &mut ScriptListApp, _window: &mut Window, ctx: &mut Context<ScriptListApp>| {
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
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("STDIN", "Async stdin command handler started");
            
            // Event-driven: recv().await yields until a command arrives
            while let Ok(cmd) = stdin_rx.recv().await {
                logging::log("STDIN", &format!("Processing external command: {:?}", cmd));
                
                let _ = cx.update(|cx| {
                    let _ = window_for_stdin.update(cx, |view: &mut ScriptListApp, window: &mut Window, ctx: &mut Context<ScriptListApp>| {
                        match cmd {
                            ExternalCommand::Run { ref path } => {
                                logging::log("STDIN", &format!("Executing script: {}", path));
                                // Show and focus window first
                                WINDOW_VISIBLE.store(true, Ordering::SeqCst);
                                ctx.activate(true);
                                window.activate_window();
                                let focus_handle = view.focus_handle(ctx);
                                window.focus(&focus_handle, ctx);
                                
                                // Send RunScript message to be handled
                                view.handle_prompt_message(PromptMessage::RunScript { path: path.clone() }, ctx);
                            }
                            ExternalCommand::Show => {
                                logging::log("STDIN", "Showing window");
                                WINDOW_VISIBLE.store(true, Ordering::SeqCst);
                                ctx.activate(true);
                                window.activate_window();
                                let focus_handle = view.focus_handle(ctx);
                                window.focus(&focus_handle, ctx);
                            }
                            ExternalCommand::Hide => {
                                logging::log("STDIN", "Hiding window");
                                WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                                ctx.hide();
                            }
                            ExternalCommand::SetFilter { ref text } => {
                                logging::log("STDIN", &format!("Setting filter to: '{}'", text));
                                view.filter_text = text.clone();
                                let _ = view.get_filtered_results_cached(); // Update cache
                                view.selected_index = 0;
                                view.update_window_size();
                            }
                        }
                        ctx.notify();
                    });
                });
            }
            
            logging::log("STDIN", "Async stdin command handler exiting");
        }).detach();
        
        // Tray menu event handler - polls for menu events
        // Clone config for use in tray handler
        let config_for_tray = config::load_config();
        if let Some(tray_mgr) = tray_manager {
            let window_for_tray = window;
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
                                let _ = cx.update(|cx| {
                                    // Show and focus window (same logic as hotkey handler)
                                    WINDOW_VISIBLE.store(true, Ordering::SeqCst);
                                    
                                    // Calculate new bounds on display with mouse
                                    let window_size = size(px(750.), initial_window_height());
                                    let new_bounds = calculate_eye_line_bounds_on_mouse_display(window_size, cx);
                                    
                                    // Move window first
                                    move_first_window_to_bounds(&new_bounds);
                                    
                                    // Activate the app
                                    cx.activate(true);
                                    
                                    // Configure as floating panel on first show
                                    if !PANEL_CONFIGURED.swap(true, Ordering::SeqCst) {
                                        configure_as_floating_panel();
                                    }
                                    
                                    // Focus the window
                                    let _ = window_for_tray.update(cx, |view: &mut ScriptListApp, win: &mut Window, ctx: &mut Context<ScriptListApp>| {
                                        win.activate_window();
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
