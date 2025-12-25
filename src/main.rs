use gpui::{
    div, prelude::*, px, point, rgb, rgba, size, App, Application, Bounds, Context, Render,
    Window, WindowBounds, WindowOptions, SharedString, FocusHandle, Focusable,
    WindowHandle, Timer, Pixels, WindowBackgroundAppearance, AnyElement, BoxShadow, hsla,
};
use global_hotkey::{GlobalHotKeyManager, GlobalHotKeyEvent, HotKeyState, hotkey::{HotKey, Modifiers, Code}};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
mod syntax;

use std::sync::{Arc, Mutex, mpsc};
use protocol::{Message, Choice};
use actions::{ScriptInfo, POPUP_WIDTH, POPUP_MAX_HEIGHT};
use syntax::highlight_code_lines;

/// Channel for sending prompt messages from script thread to UI
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


/// Move the application's first window to new bounds, regardless of keyWindow status.
/// This is the reliable way to move the window because we don't depend on keyWindow
/// being set (which has timing issues with macOS window activation).
fn move_first_window_to(x: f64, y: f64, width: f64, height: f64) {
    unsafe {
        let app: id = NSApp();
        
        // Get all windows and find our main window directly
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        
        logging::log("POSITION", &format!("move_first_window_to: app has {} windows", count));
        
        if count > 0 {
            // Get the first window (our main window)
            let window: id = msg_send![windows, objectAtIndex:0usize];
            
            if window != nil {
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
                
                // Also bring to front and make key
                let _: () = msg_send![window, makeKeyAndOrderFront:nil];
                
                // Verify the move worked
                let after_frame: NSRect = msg_send![window, frame];
                logging::log("POSITION", &format!(
                    "Window moved: actual=({:.0}, {:.0}) size={:.0}x{:.0}",
                    after_frame.origin.x, after_frame.origin.y,
                    after_frame.size.width, after_frame.size.height
                ));
            } else {
                logging::log("POSITION", "ERROR: First window is nil!");
            }
        } else {
            logging::log("POSITION", "ERROR: No windows found!");
        }
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

/// Calculate window bounds positioned at eye-line height on the display containing the mouse cursor.
/// 
/// - Finds the display where the mouse cursor is located
/// - Centers the window horizontally on that display
/// - Positions the window at "eye-line" height (upper 1/4 of the screen)
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
        
        // Eye-line: position window top at ~1/4 from screen top (input bar at eye level)
        let eye_line_y = display.origin_y + display.height * 0.25;
        
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
static HOTKEY_TRIGGERED: AtomicBool = AtomicBool::new(false);
static HOTKEY_TRIGGER_COUNT: AtomicU64 = AtomicU64::new(0);
static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(false); // Track window visibility for toggle (starts hidden)

/// Application state - what view are we currently showing
#[derive(Debug, Clone)]
enum AppView {
    /// Showing the script list
    ScriptList,
    /// Showing the actions dialog (mini searchable popup)
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
}

/// Wrapper to hold a script session that can be shared across async boundaries
type SharedSession = Arc<Mutex<Option<executor::ScriptSession>>>;

/// Messages sent from the prompt poller back to the main app
#[derive(Debug, Clone)]
enum PromptMessage {
    ShowArg { id: String, placeholder: String, choices: Vec<Choice> },
    ShowDiv { id: String, html: String, tailwind: Option<String> },
    ScriptExit,
}

/// A simple model that polls for hotkey triggers
struct HotkeyPoller {
    window: WindowHandle<ScriptListApp>,
}

impl HotkeyPoller {
    fn new(window: WindowHandle<ScriptListApp>) -> Self {
        Self { window }
    }
    
    fn start_polling(&self, cx: &mut Context<Self>) {
        let window = self.window.clone();
        cx.spawn(async move |_this, cx: &mut gpui::AsyncApp| {
            loop {
                Timer::after(std::time::Duration::from_millis(100)).await;
                
                if HOTKEY_TRIGGERED.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                    logging::log("HOTKEY", "=== Hotkey triggered ===");
                    
                    // Check current visibility state for toggle behavior
                    let is_visible = WINDOW_VISIBLE.load(Ordering::SeqCst);
                    
                    if is_visible {
                        // Update visibility state FIRST to prevent race conditions
                        // Even though the hide is async, we mark it as hidden immediately
                        WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                        logging::log("HOTKEY", "Visibility state updated: visible -> hidden");
                        
                        // Window is visible - check if in prompt mode
                        let window_clone = window.clone();
                        
                        // First check if we're in a prompt - if so, cancel and hide
                        let mut in_prompt = false;
                        let _ = cx.update(move |cx: &mut App| {
                            let _ = window_clone.update(cx, |view: &mut ScriptListApp, _win: &mut Window, ctx: &mut Context<ScriptListApp>| {
                                if view.is_in_prompt() {
                                    logging::log("HOTKEY", "In prompt mode - canceling script before hiding");
                                    view.cancel_script_execution(ctx);
                                    in_prompt = true;
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
                        // Update visibility state FIRST to prevent race conditions
                        WINDOW_VISIBLE.store(true, Ordering::SeqCst);
                        logging::log("HOTKEY", "Visibility state updated: hidden -> visible");
                        
                        // Window is hidden - SHOW it
                        logging::log("HOTKEY", "Showing window (toggle: hidden -> visible)");
                        
                        let window_clone = window.clone();
                        let _ = cx.update(move |cx: &mut App| {
                            // Step 1: Calculate new bounds on display with mouse, at eye-line height
                            let window_size = size(px(750.), px(500.0));
                            let new_bounds = calculate_eye_line_bounds_on_mouse_display(window_size, cx);
                            
                            logging::log("HOTKEY", &format!(
                                "Calculated bounds: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
                                f64::from(new_bounds.origin.x),
                                f64::from(new_bounds.origin.y),
                                f64::from(new_bounds.size.width),
                                f64::from(new_bounds.size.height)
                            ));
                            
                            // Step 2: FIRST activate the app (makes window visible)
                            // We MUST activate first because move_key_window_to() uses NSApp().keyWindow
                            // and hidden windows are NOT the key window
                            // Step 2: Move window FIRST (before activation)
                            // We use move_first_window_to_bounds which doesn't depend on keyWindow
                            // This ensures the window is in the right position before it becomes visible
                            move_first_window_to_bounds(&new_bounds);
                            logging::log("HOTKEY", "Window repositioned to mouse display");
                            
                            // Step 3: NOW activate the app (makes window visible at new position)
                            cx.activate(true);
                            logging::log("HOTKEY", "App activated (window now visible)");
                            
                            // Step 4: Activate the specific window and focus it
                            let _ = window_clone.update(cx, |view: &mut ScriptListApp, win: &mut Window, cx: &mut Context<ScriptListApp>| {
                                win.activate_window();
                                let focus_handle = view.focus_handle(cx);
                                win.focus(&focus_handle, cx);
                                logging::log("HOTKEY", "Window activated and focused");
                            });
                            
                            logging::log("HOTKEY", "Window shown - hotkey handling complete");
                        });
                    }
                    
                    logging::log("HOTKEY", "=== Hotkey handling complete ===");
                }
            }
        }).detach();
    }
}

struct ScriptListApp {
    scripts: Vec<scripts::Script>,
    scriptlets: Vec<scripts::Scriptlet>,
    selected_index: usize,
    filter_text: String,
    last_output: Option<SharedString>,
    focus_handle: FocusHandle,
    show_logs: bool,
    theme: theme::Theme,
    config: config::Config,
    // Interactive script state
    current_view: AppView,
    script_session: SharedSession,
    // Prompt-specific state (used when view is ArgPrompt or DivPrompt)
    arg_input_text: String,
    arg_selected_index: usize,
    // Channel for receiving prompt messages from script thread
    prompt_receiver: Option<mpsc::Receiver<PromptMessage>>,
    // Channel for sending responses back to script
    response_sender: Option<mpsc::Sender<Message>>,
    // Scroll state for script list
    scroll_offset: usize,           // Index of first visible item
    viewport_height: usize,         // Number of items that fit in viewport
    // Actions popup overlay
    show_actions_popup: bool,
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
        
        let total_elapsed = load_start.elapsed();
        logging::log("PERF", &format!(
            "Startup loading: {:.2}ms total ({} scripts in {:.2}ms, {} scriptlets in {:.2}ms)",
            total_elapsed.as_secs_f64() * 1000.0,
            scripts.len(),
            scripts_elapsed.as_secs_f64() * 1000.0,
            scriptlets.len(),
            scriptlets_elapsed.as_secs_f64() * 1000.0
        ));
        logging::log("APP", &format!("Loaded {} scripts from ~/.kenv/scripts", scripts.len()));
        logging::log("APP", &format!("Loaded {} scriptlets from ~/.kenv/scriptlets/scriptlets.md", scriptlets.len()));
        logging::log("APP", "Loaded theme with system appearance detection");
        logging::log("APP", &format!("Loaded config: hotkey={:?}+{}, bun_path={:?}", 
            config.hotkey.modifiers, config.hotkey.key, config.bun_path));
        ScriptListApp {
            scripts,
            scriptlets,
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
            scroll_offset: 0,
            viewport_height: 9,  // ~9 items visible at a time (allows margin for footer)
            show_actions_popup: false,
        }
    }
    
    /// Poll for prompt messages from the script thread
    fn poll_prompt_messages(&mut self, cx: &mut Context<Self>) {
        // Collect messages first to avoid borrow conflicts
        let messages: Vec<PromptMessage> = if let Some(ref receiver) = self.prompt_receiver {
            let mut msgs = Vec::new();
            while let Ok(msg) = receiver.try_recv() {
                msgs.push(msg);
            }
            msgs
        } else {
            Vec::new()
        };
        
        // Now process collected messages
        for msg in messages {
            self.handle_prompt_message(msg, cx);
        }
    }
    
    fn update_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = theme::load_theme();
        logging::log("APP", "Theme reloaded based on system appearance");
        cx.notify();
    }
    
    fn update_config(&mut self, cx: &mut Context<Self>) {
        logging::log("APP", "Config file reloaded");
        cx.notify();
    }
    
    fn refresh_scripts(&mut self, cx: &mut Context<Self>) {
        self.scripts = scripts::read_scripts();
        self.scriptlets = scripts::read_scriptlets();
        self.selected_index = 0;
        self.scroll_offset = 0;
        logging::log("APP", &format!("Scripts refreshed: {} scripts, {} scriptlets loaded", self.scripts.len(), self.scriptlets.len()));
        logging::log("SCROLL", "Scripts refreshed - reset: selected_index=0, scroll_offset=0");
        cx.notify();
    }

    /// Get unified filtered results combining scripts and scriptlets
    fn filtered_results(&self) -> Vec<scripts::SearchResult> {
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
            
            // Scroll if item is above visible area
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
            
            logging::log("SCROLL", &format!(
                "Up: selected_index={}, scroll_offset={}, viewport={}",
                self.selected_index, self.scroll_offset, self.viewport_height
            ));
            cx.notify();
        }
    }

    fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        let filtered_len = self.filtered_results().len();
        if self.selected_index < filtered_len.saturating_sub(1) {
            self.selected_index += 1;
            
            // Scroll if item is below visible area
            let last_visible = self.scroll_offset + self.viewport_height;
            if self.selected_index >= last_visible {
                self.scroll_offset = self.selected_index - self.viewport_height + 1;
            }
            
            logging::log("SCROLL", &format!(
                "Down: selected_index={}, scroll_offset={}, viewport={}, last_visible={}",
                self.selected_index, self.scroll_offset, self.viewport_height, last_visible
            ));
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
            }
        }
    }

    fn update_filter(&mut self, new_char: Option<char>, backspace: bool, clear: bool, cx: &mut Context<Self>) {
        if clear {
            self.filter_text.clear();
            self.selected_index = 0;
            self.scroll_offset = 0;
            logging::log("SCROLL", "Filter cleared - reset: selected_index=0, scroll_offset=0");
        } else if backspace && !self.filter_text.is_empty() {
            self.filter_text.pop();
            self.selected_index = 0;
            self.scroll_offset = 0;
            logging::log("SCROLL", "Filter backspace - reset: selected_index=0, scroll_offset=0");
        } else if let Some(ch) = new_char {
            self.filter_text.push(ch);
            self.selected_index = 0;
            self.scroll_offset = 0;
            logging::log("SCROLL", &format!("Filter char '{}' - reset: selected_index=0, scroll_offset=0", ch));
        }
        cx.notify();
    }
    
    fn toggle_logs(&mut self, cx: &mut Context<Self>) {
        self.show_logs = !self.show_logs;
        cx.notify();
    }
    
    fn open_actions(&mut self, cx: &mut Context<Self>) {
        logging::log("KEY", "Toggling actions popup");
        self.show_actions_popup = !self.show_actions_popup;
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
    
    /// Edit a script in $EDITOR
    fn edit_script(&mut self, path: &std::path::Path) {
        logging::log("UI", &format!("Opening script in editor: {}", path.display()));
        
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
        let path_str = path.to_string_lossy().to_string();
        
        std::thread::spawn(move || {
            use std::process::Command;
            let _ = Command::new(&editor)
                .arg(&path_str)
                .spawn();
            logging::log("UI", &format!("Editor spawned: {}", editor));
        });
    }
    
    /// Execute a script interactively (for scripts that use arg/div prompts)
    fn execute_interactive(&mut self, script: &scripts::Script, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Starting interactive execution: {}", script.name));
        
        match executor::execute_script_interactive(&script.path) {
            Ok(session) => {
                logging::log("EXEC", "Interactive session started successfully");
                *self.script_session.lock().unwrap() = Some(session);
                
                // Create channel for script thread to send prompt messages to UI
                let (tx, rx) = mpsc::channel();
                self.prompt_receiver = Some(rx);
                
                // We need separate threads for reading and writing to avoid deadlock
                // The read thread blocks on receive_message(), so we can't check for responses in the same loop
                
                // Take ownership of the session and split it
                let session = self.script_session.lock().unwrap().take().unwrap();
                let split = session.split();
                
                let mut stdin = split.stdin;
                let mut stdout_reader = split.stdout_reader;
                
                // Channel for sending responses from UI to writer thread
                let (response_tx, response_rx) = mpsc::channel::<Message>();
                
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
                std::thread::spawn(move || {
                    loop {
                        match stdout_reader.next_message() {
                            Ok(Some(msg)) => {
                                logging::log("EXEC", &format!("Received message: {:?}", msg));
                                let prompt_msg = match msg {
                                    Message::Arg { id, placeholder, choices } => {
                                        Some(PromptMessage::ShowArg { id, placeholder, choices })
                                    }
                                    Message::Div { id, html, tailwind } => {
                                        Some(PromptMessage::ShowDiv { id, html, tailwind })
                                    }
                                    Message::Exit { .. } => {
                                        Some(PromptMessage::ScriptExit)
                                    }
                                    _ => None,
                                };
                                
                                if let Some(prompt_msg) = prompt_msg {
                                    if tx.send(prompt_msg).is_err() {
                                        logging::log("EXEC", "Prompt channel closed, reader exiting");
                                        break;
                                    }
                                }
                            }
                            Ok(None) => {
                                logging::log("EXEC", "Script stdout closed (EOF)");
                                let _ = tx.send(PromptMessage::ScriptExit);
                                break;
                            }
                            Err(e) => {
                                logging::log("EXEC", &format!("Error reading from script: {}", e));
                                let _ = tx.send(PromptMessage::ScriptExit);
                                break;
                            }
                        }
                    }
                    logging::log("EXEC", "Reader thread exited");
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
    
    /// Handle a prompt message from the script
    fn handle_prompt_message(&mut self, msg: PromptMessage, cx: &mut Context<Self>) {
        match msg {
            PromptMessage::ShowArg { id, placeholder, choices } => {
                logging::log("UI", &format!("Showing arg prompt: {} with {} choices", id, choices.len()));
                self.current_view = AppView::ArgPrompt { id, placeholder, choices };
                self.arg_input_text.clear();
                self.arg_selected_index = 0;
                cx.notify();
            }
            PromptMessage::ShowDiv { id, html, tailwind } => {
                logging::log("UI", &format!("Showing div prompt: {}", id));
                self.current_view = AppView::DivPrompt { id, html, tailwind };
                cx.notify();
            }
            PromptMessage::ScriptExit => {
                logging::log("EXEC", "Script exited, calling reset_to_script_list");
                self.reset_to_script_list(cx);
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
    
    /// Reset all state and return to the script list view
    fn reset_to_script_list(&mut self, cx: &mut Context<Self>) {
        let old_view = match &self.current_view {
            AppView::ScriptList => "ScriptList",
            AppView::ActionsDialog => "ActionsDialog",
            AppView::ArgPrompt { .. } => "ArgPrompt",
            AppView::DivPrompt { .. } => "DivPrompt",
        };
        
        logging::log("UI", &format!("Resetting to script list (was: {})", old_view));
        
        // Reset view
        self.current_view = AppView::ScriptList;
        
        // Clear arg prompt state
        self.arg_input_text.clear();
        self.arg_selected_index = 0;
        
        // Clear output
        self.last_output = None;
        
        // Clear channels (they will be dropped, closing the connections)
        self.prompt_receiver = None;
        self.response_sender = None;
        
        // Clear script session
        if let Ok(mut session_guard) = self.script_session.lock() {
            *session_guard = None;
        }
        
        logging::log("UI", "State reset complete - view is now ScriptList");
        cx.notify();
    }
    
    /// Check if we're currently in a prompt view (script is running)
    fn is_in_prompt(&self) -> bool {
        matches!(self.current_view, AppView::ArgPrompt { .. } | AppView::DivPrompt { .. })
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
        // Ensure we have focus on every render
        let is_focused = self.focus_handle.is_focused(window);
        if !is_focused {
            window.focus(&self.focus_handle, cx);
        }
        
        // Poll for prompt messages from script thread
        self.poll_prompt_messages(cx);
        
        // Dispatch to appropriate view - clone to avoid borrow issues
        let current_view = self.current_view.clone();
        match current_view {
            AppView::ScriptList => self.render_script_list(cx),
            AppView::ActionsDialog => self.render_actions_dialog(cx),
            AppView::ArgPrompt { id, placeholder, choices } => self.render_arg_prompt(id, placeholder, choices, cx),
            AppView::DivPrompt { id, html, tailwind } => self.render_div_prompt(id, html, tailwind, cx),
        }
    }
}

impl ScriptListApp {
    /// Read the first N lines of a script file for preview
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
    
    /// Render the preview panel showing details of the selected script/scriptlet
    fn render_preview_panel(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let filtered = self.filtered_results();
        let selected_result = filtered.get(self.selected_index);
        let theme = &self.theme;
        
        // Preview panel container with left border separator
        let mut panel = div()
            .w_full()
            .h_full()
            .bg(rgb(theme.colors.background.main))
            .border_l_1()
            .border_color(rgba((theme.colors.ui.border << 8) | 0x80))
            .p(px(16.))
            .flex()
            .flex_col()
            .overflow_y_hidden()
            .font_family(".AppleSystemUIFont");
        
        match selected_result {
            Some(result) => {
                match result {
                    scripts::SearchResult::Script(script_match) => {
                        let script = &script_match.script;
                        logging::log("UI", &format!(
                            "Preview panel updated: Script '{}' ({})",
                            script.name, script.extension
                        ));
                        
                        // Script name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(theme.colors.text.primary))
                                .pb(px(8.))
                                .child(format!("{}.{}", script.name, script.extension))
                        );
                        
                        // Type badge
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .pb(px(12.))
                                .child(
                                    div()
                                        .text_xs()
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(4.))
                                        .bg(rgb(0x4a90e2))
                                        .text_color(rgb(0xffffff))
                                        .child("Script")
                                )
                        );
                        
                        // Description (if present)
                        if let Some(desc) = &script.description {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(12.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(theme.colors.text.muted))
                                            .pb(px(2.))
                                            .child("Description")
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(theme.colors.text.secondary))
                                            .child(desc.clone())
                                    )
                            );
                        }
                        
                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(1.))
                                .bg(rgba((theme.colors.ui.border << 8) | 0x60))
                                .my(px(8.))
                        );
                        
                        // Code preview header
                        panel = panel.child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme.colors.text.muted))
                                .pb(px(8.))
                                .child("Code Preview")
                        );
                        
                        // Read and display code preview with syntax highlighting
                        let code_preview = Self::read_script_preview(&script.path, 15);
                        let lang = script.extension.as_str();
                        let lines = highlight_code_lines(&code_preview, lang);
                        
                        // Build code container - render line by line with monospace font
                        let mut code_container = div()
                            .w_full()
                            .min_w(px(280.))
                            .p(px(12.))
                            .rounded(px(6.))
                            .bg(rgba((theme.colors.background.search_box << 8) | 0x80))
                            .overflow_hidden()
                            .flex()
                            .flex_col();
                        
                        // Render each line as a row of spans with monospace font
                        for line in lines {
                            let mut line_div = div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .font_family("Menlo")
                                .text_xs()
                                .min_h(px(16.)); // Line height
                            
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
                        logging::log("UI", &format!(
                            "Preview panel updated: Scriptlet '{}' ({})",
                            scriptlet.name, scriptlet.tool
                        ));
                        
                        // Scriptlet name header
                        panel = panel.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(theme.colors.text.primary))
                                .pb(px(8.))
                                .child(scriptlet.name.clone())
                        );
                        
                        // Type and tool badges
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .pb(px(12.))
                                .child(
                                    div()
                                        .text_xs()
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(4.))
                                        .bg(rgb(0x7ed321))
                                        .text_color(rgb(0xffffff))
                                        .child("Snippet")
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(4.))
                                        .bg(rgba((theme.colors.ui.border << 8) | 0xff))
                                        .text_color(rgb(theme.colors.text.secondary))
                                        .child(scriptlet.tool.clone())
                                )
                        );
                        
                        // Description (if present)
                        if let Some(desc) = &scriptlet.description {
                            panel = panel.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pb(px(12.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(theme.colors.text.muted))
                                            .pb(px(2.))
                                            .child("Description")
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(theme.colors.text.secondary))
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
                                    .pb(px(12.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(theme.colors.text.muted))
                                            .pb(px(2.))
                                            .child("Hotkey")
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(theme.colors.text.secondary))
                                            .child(shortcut.clone())
                                    )
                            );
                        }
                        
                        // Divider
                        panel = panel.child(
                            div()
                                .w_full()
                                .h(px(1.))
                                .bg(rgba((theme.colors.ui.border << 8) | 0x60))
                                .my(px(8.))
                        );
                        
                        // Content preview header
                        panel = panel.child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme.colors.text.muted))
                                .pb(px(8.))
                                .child("Content Preview")
                        );
                        
                        // Display scriptlet code with syntax highlighting (first 15 lines)
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
                            .p(px(12.))
                            .rounded(px(6.))
                            .bg(rgba((theme.colors.background.search_box << 8) | 0x80))
                            .overflow_hidden()
                            .flex()
                            .flex_col();
                        
                        // Render each line as a row of spans with monospace font
                        for line in lines {
                            let mut line_div = div()
                                .flex()
                                .flex_row()
                                .w_full()
                                .font_family("Menlo")
                                .text_xs()
                                .min_h(px(16.)); // Line height
                            
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
                        .text_color(rgb(theme.colors.text.muted))
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
            }
        } else {
            None
        }
    }
    
    /// Render the actions overlay popup as a static div (ActionsDialog is a Render component)
    fn render_actions_overlay(&self, _cx: &mut Context<Self>, script_info: Option<ScriptInfo>) -> impl IntoElement {
        // Render a simple actions popup inline rather than using the ActionsDialog component
        // since ActionsDialog is a Render component (entity) that can't be used directly as IntoElement
        let theme = &self.theme;
        
        let mut actions_container = div()
            .flex()
            .flex_col()
            .w(px(POPUP_WIDTH))
            .max_h(px(POPUP_MAX_HEIGHT))
            .bg(rgba(0x1e1e1ee6))
            .rounded(px(12.))
            .shadow_lg()
            .border_1()
            .border_color(rgba(0x3d3d3d80))
            .overflow_hidden()
            .p(px(8.));
        
        // Header with script context
        if let Some(info) = &script_info {
            actions_container = actions_container.child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .text_xs()
                    .text_color(rgba(0x888888ff))
                    .child(format!("Actions for: {}", info.name))
            );
        }
        
        // Define actions
        let actions = vec![
            ("edit_script", "Edit Script", "⌘E"),
            ("view_logs", "View Logs", "⌘L"),
            ("reveal_in_finder", "Reveal in Finder", "⌘⇧F"),
            ("copy_path", "Copy Path", "⌘⇧C"),
            ("create_script", "Create New Script", "⌘N"),
            ("reload_scripts", "Reload Scripts", "⌘R"),
            ("settings", "Settings", "⌘,"),
            ("quit", "Quit", "⌘Q"),
        ];
        
        for (_id, title, shortcut) in actions {
            actions_container = actions_container.child(
                div()
                    .w_full()
                    .px(px(12.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .hover(|s| s.bg(rgba(0x0e47a1cc)))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme.colors.text.secondary))
                            .child(title)
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(0x66666699))
                            .child(shortcut)
                    )
            );
        }
        
        // Footer hint
        actions_container = actions_container.child(
            div()
                .mt(px(8.))
                .pt(px(8.))
                .border_t_1()
                .border_color(rgba(0x3d3d3d60))
                .px(px(12.))
                .text_xs()
                .text_color(rgba(0x666666ff))
                .child("Press Esc to close")
        );
        
        actions_container
    }
    
    fn render_script_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let filtered = self.filtered_results();
        let filtered_len = filtered.len();
        let total_len = self.scripts.len() + self.scriptlets.len();
        let theme = &self.theme;
        
        // Handle edge cases for scroll bounds
        // Keep selected_index in valid bounds
        if self.selected_index >= filtered_len && filtered_len > 0 {
            self.selected_index = filtered_len.saturating_sub(1);
        }
        
        // Ensure scroll_offset doesn't go past list
        let max_offset = filtered_len.saturating_sub(self.viewport_height);
        if self.scroll_offset > max_offset {
            self.scroll_offset = max_offset;
        }

        // Build script list - tight, clean spacing
        let mut list_container = div().flex().flex_col().w_full();

        if filtered_len == 0 {
            list_container = list_container.child(
                div()
                    .w_full()
                    .py(px(24.))
                    .text_center()
                    .text_color(rgb(theme.colors.text.muted))
                    .font_family(".AppleSystemUIFont")
                    .child(if self.filter_text.is_empty() {
                        "No scripts or snippets found".to_string()
                    } else {
                        format!("No results match '{}'", self.filter_text)
                    }),
            );
        } else {
            // Only render visible items (scroll_offset to scroll_offset + viewport_height)
            // This provides a performance benefit - we only render what's visible
            let visible_items: Vec<_> = filtered.iter()
                .skip(self.scroll_offset)
                .take(self.viewport_height)
                .enumerate()
                .collect();
            
            // PERF: Log viewport rendering stats
            let visible_count = visible_items.len();
            logging::log("PERF", &format!(
                "Rendering {} items (scroll_offset={}, viewport={}, total={})",
                visible_count, self.scroll_offset, self.viewport_height, filtered_len
            ));
            
            for (display_idx, result) in visible_items {
                // The actual index in the filtered list
                let actual_idx = self.scroll_offset + display_idx;
                let is_selected = actual_idx == self.selected_index;
                
                // Get name and badge based on type
                let (name_display, badge_text, badge_color) = match result {
                    scripts::SearchResult::Script(sm) => {
                        (format!("{}.{}", sm.script.name, sm.script.extension), "Script", 0x4a90e2u32)
                    }
                    scripts::SearchResult::Scriptlet(sm) => {
                        (sm.scriptlet.name.clone(), "Snippet", 0x7ed321u32)
                    }
                };
                
                list_container = list_container.child(
                    div()
                        .w_full()
                        .px(px(12.))
                        .child(
                            div()
                                .w_full()
                                .px(px(12.))
                                .py(px(8.))
                                .rounded(px(8.))
                                .bg(if is_selected { rgb(theme.colors.accent.selected) } else { rgb(theme.colors.background.main) })
                                .text_color(if is_selected { rgb(theme.colors.text.primary) } else { rgb(theme.colors.text.secondary) })
                                .font_family(".AppleSystemUIFont")
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .px(px(5.))
                                        .py(px(2.))
                                        .rounded(px(3.))
                                        .bg(rgb(badge_color))
                                        .text_color(rgb(0xffffff))
                                        .child(badge_text)
                                )
                                .child(name_display)
                        ),
                );
            }
        }

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
                .p(px(12.))
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
            SharedString::from("Type a command...")
        } else {
            SharedString::from(self.filter_text.clone())
        };
        let filter_is_empty = self.filter_text.is_empty();

        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            let has_cmd = event.keystroke.modifiers.platform;
            
            logging::log("KEY", &format!("Key pressed: '{}' cmd={}", key_str, has_cmd));
            
            if has_cmd {
                match key_str.as_str() {
                    "l" => { this.toggle_logs(cx); return; }
                    "k" => { this.open_actions(cx); return; }
                    _ => {}
                }
            }
            
            match key_str.as_str() {
                "up" | "arrowup" => this.move_selection_up(cx),
                "down" | "arrowdown" => this.move_selection_down(cx),
                "enter" => this.execute_selected(cx),
                "escape" => {
                    // If actions popup is open, close it first
                    if this.show_actions_popup {
                        this.show_actions_popup = false;
                        cx.notify();
                        return;
                    }
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
        let bg_hex = theme.colors.background.main;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        
        // Create box shadows from theme
        let box_shadows = self.create_box_shadows();
        
        let mut main_div = div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(12.))
            .text_color(rgb(theme.colors.text.primary))
            .font_family(".AppleSystemUIFont")
            .key_context("script_list")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header: Logo + Search Input - compact
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(14.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Logo - smaller, cleaner
                    .child(
                        div()
                            .w(px(24.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(theme.colors.accent.selected))
                            .text_lg()
                            .child("▶")
                    )
                    // Search input - large but not huge
                    .child(
                        div()
                            .flex_1()
                            .text_xl()
                            .text_color(if filter_is_empty { rgb(theme.colors.text.muted) } else { rgb(theme.colors.text.primary) })
                            .child(filter_display)
                    )
                    // Count - subtle
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme.colors.text.dimmed))
                            .child(format!("{}/{}", filtered_len, total_len))
                    ),
            )
            // Subtle divider - semi-transparent
            .child(
                div()
                    .mx(px(16.))
                    .h(px(1.))
                    .bg(rgba((theme.colors.ui.border << 8) | 0x60))
            )
            // Main content area - 60/40 split: List on left, Preview on right
            // Uses min_h(px(0.)) to prevent flex children from overflowing
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))  // Critical: allows flex container to shrink properly
                    .w_full()
                    .overflow_hidden()
                    // Left side: Script list (60% width)
                    .child(
                        div()
                            .w(px(450.))  // 60% of 750px window = 450px
                            .flex_1()     // Take available vertical space
                            .min_h(px(0.))  // Allow shrinking
                            .flex()
                            .flex_col()
                            .py(px(4.))
                            .overflow_y_hidden()
                            .child(list_container)
                    )
                    // Right side: Preview panel (40% width = remaining space)
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.))  // Allow shrinking
                            .child(self.render_preview_panel(cx))
                    ),
            );
        
        if let Some(panel) = log_panel {
            main_div = main_div.child(panel);
        }
        
        // Footer - compact with semi-transparent border
        main_div = main_div.child(
            div()
                .w_full()
                .px(px(16.))
                .py(px(10.))
                .border_t_1()
                .border_color(rgba((theme.colors.ui.border << 8) | 0x60))
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(theme.colors.text.muted))
                        .child(
                            if let Some(output) = &self.last_output {
                                output.clone()
                            } else {
                                SharedString::from("↑↓ navigate • ⏎ run")
                            }
                        )
                )
                .child(
                    div()
                        .px(px(8.))
                        .py(px(4.))
                        .bg(rgba((theme.colors.background.search_box << 8) | 0x80))
                        .rounded(px(4.))
                        .text_xs()
                        .text_color(rgb(theme.colors.text.muted))
                        .child("Actions ⌘K")
                ),
        );
        
        // Wrap in relative container for overlay positioning
        let script_info = self.get_focused_script_info();
        let show_popup = self.show_actions_popup;
        
        let mut container = div()
            .relative()
            .w_full()
            .h_full()
            .child(main_div);
        
        // Add actions popup overlay when visible
        if show_popup {
            container = container.child(
                div()
                    .absolute()
                    .right(px(16.))
                    .bottom(px(56.))  // Above the footer
                    .w(px(POPUP_WIDTH))
                    .max_h(px(POPUP_MAX_HEIGHT))
                    .child(self.render_actions_overlay(cx, script_info))
            );
        }
        
        container.into_any_element()
    }
    
    fn render_arg_prompt(&mut self, id: String, placeholder: String, choices: Vec<Choice>, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        let filtered = self.filtered_arg_choices();
        let filtered_len = filtered.len();
        
        // Key handler for arg prompt
        let prompt_id = id.clone();
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            logging::log("KEY", &format!("ArgPrompt key: '{}'", key_str));
            
            match key_str.as_str() {
                "up" | "arrowup" => {
                    if this.arg_selected_index > 0 {
                        this.arg_selected_index -= 1;
                        cx.notify();
                    }
                }
                "down" | "arrowdown" => {
                    let filtered = this.filtered_arg_choices();
                    if this.arg_selected_index < filtered.len().saturating_sub(1) {
                        this.arg_selected_index += 1;
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
                        cx.notify();
                    }
                }
                _ => {
                    if let Some(ref key_char) = event.keystroke.key_char {
                        if let Some(ch) = key_char.chars().next() {
                            if !ch.is_control() {
                                this.arg_input_text.push(ch);
                                this.arg_selected_index = 0;
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
        
        // Build choice list
        let mut list_container = div().flex().flex_col().w_full();
        
        if filtered_len == 0 {
            list_container = list_container.child(
                div()
                    .w_full()
                    .py(px(24.))
                    .text_center()
                    .text_color(rgb(theme.colors.text.muted))
                    .child("No choices match your filter"),
            );
        } else {
            for (idx, (_, choice)) in filtered.iter().enumerate() {
                let is_selected = idx == self.arg_selected_index;
                
                let mut item = div()
                    .w_full()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(if is_selected { rgb(theme.colors.accent.selected) } else { rgb(theme.colors.background.main) })
                    .text_color(if is_selected { rgb(theme.colors.text.primary) } else { rgb(theme.colors.text.secondary) })
                    .child(choice.name.clone());
                
                if let Some(desc) = &choice.description {
                    item = item.child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme.colors.text.muted))
                            .child(desc.clone())
                    );
                }
                
                list_container = list_container.child(
                    div().w_full().px(px(12.)).child(item)
                );
            }
        }
        
        // Use theme opacity and shadow settings
        let opacity = self.theme.get_opacity();
        let bg_hex = theme.colors.background.main;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(12.))
            .text_color(rgb(theme.colors.text.primary))
            .font_family(".AppleSystemUIFont")
            .key_context("arg_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header with input
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(14.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(24.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(theme.colors.accent.selected))
                            .text_lg()
                            .child("?")
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xl()
                            .text_color(if input_is_empty { rgb(theme.colors.text.muted) } else { rgb(theme.colors.text.primary) })
                            .child(input_display)
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme.colors.text.dimmed))
                            .child(format!("{} choices", choices.len()))
                    ),
            )
            // Divider
            .child(
                div()
                    .mx(px(16.))
                    .h(px(1.))
                    .bg(rgba((theme.colors.ui.border << 8) | 0x60))
            )
            // Choice list
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .py(px(4.))
                    .child(list_container)
            )
            // Footer
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(10.))
                    .border_t_1()
                    .border_color(rgba((theme.colors.ui.border << 8) | 0x60))
                    .text_xs()
                    .text_color(rgb(theme.colors.text.muted))
                    .child("↑↓ navigate • ⏎ select • Esc cancel")
            )
            .into_any_element()
    }
    
    fn render_div_prompt(&mut self, id: String, html: String, _tailwind: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        
        // Strip HTML tags for plain text display
        let display_text = {
            let mut result = String::new();
            let mut in_tag = false;
            for ch in html.chars() {
                match ch {
                    '<' => in_tag = true,
                    '>' => in_tag = false,
                    _ if !in_tag => result.push(ch),
                    _ => {}
                }
            }
            result.trim().to_string()
        };
        
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
        
        // Use theme opacity and shadow settings
        let opacity = self.theme.get_opacity();
        let bg_hex = theme.colors.background.main;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(12.))
            .text_color(rgb(theme.colors.text.primary))
            .font_family(".AppleSystemUIFont")
            .key_context("div_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Content area
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .p(px(24.))
                    .text_lg()
                    .child(display_text)
            )
            // Footer
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(10.))
                    .border_t_1()
                    .border_color(rgba((theme.colors.ui.border << 8) | 0x60))
                    .text_xs()
                    .text_color(rgb(theme.colors.text.muted))
                    .child("Press Enter or Escape to continue")
            )
            .into_any_element()
    }    
    fn render_actions_dialog(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        
        // Use theme opacity and shadow settings
        let opacity = self.theme.get_opacity();
        let bg_hex = theme.colors.background.main;
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
        let box_shadows = self.create_box_shadows();
        
        // Key handler for actions dialog
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            logging::log("KEY", &format!("ActionsDialog key: '{}'", key_str));
            
            match key_str.as_str() {
                "escape" => {
                    logging::log("KEY", "ESC in ActionsDialog - returning to script list");
                    this.current_view = AppView::ScriptList;
                    cx.notify();
                }
                _ => {}
            }
        });
        
        // Simple actions dialog stub
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .rounded(px(12.))
            .p(px(24.))
            .text_color(rgb(theme.colors.text.primary))
            .font_family(".AppleSystemUIFont")
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
                    .text_color(rgb(theme.colors.text.muted))
                    .mt(px(12.))
                    .child("• Create script\n• Edit script\n• Reload\n• Settings\n• Quit")
            )
            .child(
                div()
                    .mt(px(16.))
                    .text_xs()
                    .text_color(rgb(theme.colors.text.dimmed))
                    .child("Press Esc to close")
            )
            .into_any_element()
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
            other => {
                logging::log("HOTKEY", &format!("Unknown key code: {}. Falling back to Semicolon", other));
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
                    HOTKEY_TRIGGERED.store(true, Ordering::SeqCst);
                    logging::log("HOTKEY", &format!("{} pressed (trigger #{})", hotkey_display, count + 1));
                } else if event.id == hotkey_id && event.state == HotKeyState::Released {
                    // Ignore key release events - just log for debugging
                    logging::log("HOTKEY", &format!("{} released (ignored)", hotkey_display));
                }
            }
        }
    });
}

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

            // NSWindowCollectionBehaviorCanJoinAllSpaces = (1 << 0)
            // This makes the window appear on all spaces/desktops
            let collection_behavior: u64 = 1;
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
                "Configured window as floating panel (level=3, restorable=false, no autosave)",
            );
        } else {
            logging::log("PANEL", "Warning: No key window found to configure as panel");
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_as_floating_panel() {}

fn start_hotkey_poller(cx: &mut App, window: WindowHandle<ScriptListApp>) {
    let poller = cx.new(|_| HotkeyPoller::new(window));
    poller.update(cx, |p, cx| {
        p.start_polling(cx);
    });
}

fn main() {
    logging::init();
    
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
    Application::new().run(move |cx: &mut App| {
        logging::log("APP", "GPUI Application starting");
        
        // Calculate window bounds: centered on display with mouse, at eye-line height
        let window_size = size(px(750.), px(500.0));
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
                cx.new(|cx| ScriptListApp::new(cx))
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
        
        cx.activate(true);
        
        // Configure window as floating panel on macOS
        configure_as_floating_panel();
        
        // IMPORTANT: Update visibility state now that window is actually created and visible
        WINDOW_VISIBLE.store(true, Ordering::SeqCst);
        logging::log("HOTKEY", "Window visibility state set to true (window now visible)");
        
        start_hotkey_poller(cx, window.clone());
        
        let window_for_appearance = window.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            loop {
                Timer::after(std::time::Duration::from_millis(200)).await;
                
                if let Ok(_) = appearance_rx.try_recv() {
                    logging::log("APP", "System appearance changed, updating theme");
                    let _ = cx.update(|cx| {
                        let _ = window_for_appearance.update(cx, |view: &mut ScriptListApp, _window: &mut Window, ctx: &mut Context<ScriptListApp>| {
                            view.update_theme(ctx);
                        });
                    });
                }
            }
        }).detach();
        
        // Config reload watcher - watches ~/.kit/config.ts for changes
        let window_for_config = window.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            loop {
                Timer::after(std::time::Duration::from_millis(200)).await;
                
                if let Ok(_) = config_rx.try_recv() {
                    logging::log("APP", "Config file changed, reloading");
                    let _ = cx.update(|cx| {
                        let _ = window_for_config.update(cx, |view: &mut ScriptListApp, _window: &mut Window, ctx: &mut Context<ScriptListApp>| {
                            view.update_config(ctx);
                        });
                    });
                }
            }
        }).detach();
        
        // Prompt message poller - checks for script messages and triggers re-render
        let window_for_prompts = window.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            loop {
                Timer::after(std::time::Duration::from_millis(50)).await;
                
                let _ = cx.update(|cx| {
                    let _ = window_for_prompts.update(cx, |view: &mut ScriptListApp, _window: &mut Window, ctx: &mut Context<ScriptListApp>| {
                        // Trigger render which will poll messages
                        view.poll_prompt_messages(ctx);
                    });
                });
            }
        }).detach();
        
        // Test command file watcher - allows smoke tests to trigger script execution
        let window_for_test = window.clone();
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
        
        logging::log("APP", "Application ready - Cmd+; to show, Esc to hide, Cmd+K for actions");
    });
}
