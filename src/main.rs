use gpui::{
    div, prelude::*, px, point, rgb, rgba, size, App, Application, Bounds, Context, Render,
    Window, WindowBounds, WindowOptions, SharedString, FocusHandle, Focusable,
    WindowHandle, Timer, Pixels, WindowBackgroundAppearance, AnyElement,
};
use global_hotkey::{GlobalHotKeyManager, GlobalHotKeyEvent, hotkey::{HotKey, Modifiers, Code}};
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

use std::sync::{Arc, Mutex, mpsc};
use protocol::{Message, Choice};
use prompts::{ArgPrompt, DivPrompt};
use actions::ActionsDialog;

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

/// Move the key window (focused window) to a new position using native macOS APIs.
/// Position is specified as origin (top-left corner) in screen coordinates.
fn move_key_window_to(x: f64, y: f64, width: f64, height: f64) {
    unsafe {
        let app: id = NSApp();
        let window: id = msg_send![app, keyWindow];
        if window != nil {
            // Get the screen height for coordinate conversion
            // macOS uses bottom-left origin, we need to flip Y
            let screens: id = msg_send![class!(NSScreen), screens];
            let main_screen: id = msg_send![screens, firstObject];
            let screen_frame: NSRect = msg_send![main_screen, frame];
            let screen_height = screen_frame.size.height;
            
            // Convert from top-left origin to bottom-left origin
            let flipped_y = screen_height - y - height;
            
            let new_frame = NSRect::new(
                NSPoint::new(x, flipped_y),
                NSSize::new(width, height),
            );
            
            let _: () = msg_send![window, setFrame:new_frame display:true animate:false];
            logging::log("POSITION", &format!("Moved window to ({:.0}, {:.0})", x, y));
        } else {
            logging::log("POSITION", "No key window to move");
        }
    }
}

/// Move a specific NSWindow to new bounds
#[allow(dead_code)]
fn move_window_to_bounds(bounds: &Bounds<Pixels>) {
    let x: f64 = bounds.origin.x.into();
    let y: f64 = bounds.origin.y.into();
    let width: f64 = bounds.size.width.into();
    let height: f64 = bounds.size.height.into();
    move_key_window_to(x, y, width, height);
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
    cx: &App,
) -> Bounds<Pixels> {
    let displays = cx.displays();
    
    // Try to get mouse position and find which display contains it
    let target_display = if let Some((mouse_x, mouse_y)) = get_global_mouse_position() {
        logging::log("POSITION", &format!("Mouse at ({:.0}, {:.0})", mouse_x, mouse_y));
        
        // Find the display that contains the mouse cursor
        displays.iter().find(|display| {
            let bounds = display.bounds();
            let origin_x: f64 = bounds.origin.x.into();
            let origin_y: f64 = bounds.origin.y.into();
            let width: f64 = bounds.size.width.into();
            let height: f64 = bounds.size.height.into();
            
            mouse_x >= origin_x && mouse_x < origin_x + width &&
            mouse_y >= origin_y && mouse_y < origin_y + height
        }).cloned()
    } else {
        logging::log("POSITION", "Could not get mouse position, using primary display");
        None
    };
    
    // Use the found display, or fall back to primary display
    let display = target_display
        .or_else(|| cx.primary_display())
        .or_else(|| displays.into_iter().next());
    
    if let Some(display) = display {
        let display_bounds = display.bounds();
        
        logging::log("POSITION", &format!(
            "Using display at ({:.0}, {:.0}) size {:.0}x{:.0}",
            f64::from(display_bounds.origin.x),
            f64::from(display_bounds.origin.y),
            f64::from(display_bounds.size.width),
            f64::from(display_bounds.size.height)
        ));
        
        // Eye-line: position window top at ~1/4 from screen top (input bar at eye level)
        let eye_line_y = display_bounds.origin.y + display_bounds.size.height * 0.25;
        
        // Center horizontally on the display
        let center_x = display_bounds.origin.x + (display_bounds.size.width - window_size.width) * 0.5;
        
        Bounds {
            origin: point(center_x, eye_line_y),
            size: window_size,
        }
    } else {
        logging::log("POSITION", "No displays found, using default centered bounds");
        Bounds::centered(None, window_size, cx)
    }
}

// Global state for hotkey signaling between threads
static HOTKEY_TRIGGERED: AtomicBool = AtomicBool::new(false);
static HOTKEY_TRIGGER_COUNT: AtomicU64 = AtomicU64::new(0);

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
                    logging::log("HOTKEY", "Poller detected trigger");
                    
                    let window_clone = window.clone();
                    let _ = cx.update(move |cx: &mut App| {
                        // Calculate new bounds on display with mouse, at eye-line height
                        let window_size = size(px(750.), px(500.0));
                        let new_bounds = calculate_eye_line_bounds_on_mouse_display(window_size, cx);
                        
                        let _ = window_clone.update(cx, |view: &mut ScriptListApp, win: &mut Window, cx: &mut Context<ScriptListApp>| {
                            win.activate_window();
                            let focus_handle = view.focus_handle(cx);
                            win.focus(&focus_handle, cx);
                            logging::log("HOTKEY", "Window activated and focused");
                        });
                        cx.activate(true);
                        
                        // Move the window to the new position using native macOS APIs
                        move_window_to_bounds(&new_bounds);
                    });
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
}

impl ScriptListApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let scripts = scripts::read_scripts();
        let scriptlets = scripts::read_scriptlets();
        let theme = theme::load_theme();
        let config = config::load_config();
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
        logging::log("APP", &format!("Scripts refreshed: {} scripts, {} scriptlets loaded", self.scripts.len(), self.scriptlets.len()));
        cx.notify();
    }

    /// Get unified filtered results combining scripts and scriptlets
    fn filtered_results(&self) -> Vec<scripts::SearchResult> {
        scripts::fuzzy_search_unified(&self.scripts, &self.scriptlets, &self.filter_text)
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
            cx.notify();
        }
    }

    fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        let filtered_len = self.filtered_results().len();
        if self.selected_index < filtered_len.saturating_sub(1) {
            self.selected_index += 1;
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
        } else if backspace && !self.filter_text.is_empty() {
            self.filter_text.pop();
            self.selected_index = 0;
        } else if let Some(ch) = new_char {
            self.filter_text.push(ch);
            self.selected_index = 0;
        }
        cx.notify();
    }
    
    fn toggle_logs(&mut self, cx: &mut Context<Self>) {
        self.show_logs = !self.show_logs;
        cx.notify();
    }
    
    fn open_actions(&mut self, cx: &mut Context<Self>) {
        logging::log("UI", "Actions menu opened (Cmd+K)");
        self.current_view = AppView::ActionsDialog;
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
                logging::log("UI", "Script exited, returning to list");
                self.current_view = AppView::ScriptList;
                *self.script_session.lock().unwrap() = None;
                cx.notify();
            }
         }
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
    fn render_script_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let filtered = self.filtered_results();
        let filtered_len = filtered.len();
        let total_len = self.scripts.len() + self.scriptlets.len();
        let theme = &self.theme;

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
            for (idx, result) in filtered.iter().enumerate() {
                let is_selected = idx == self.selected_index;
                
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
                    if !this.filter_text.is_empty() {
                        this.update_filter(None, false, true, cx);
                    } else {
                        cx.hide();
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
        // Convert theme background to rgba with ~90% opacity (E6 = 230)
        let bg_hex = theme.colors.background.main;
        let bg_with_alpha = (bg_hex << 8) | 0xE6; // Shift RGB left, add alpha
        
        let mut main_div = div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
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
            // List - fills available space
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .py(px(4.))
                    .child(list_container),
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
        
        main_div.into_any_element()
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
                    this.submit_prompt_response(prompt_id.clone(), None, cx);
                    this.current_view = AppView::ScriptList;
                    *this.script_session.lock().unwrap() = None;
                    cx.notify();
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
        
        let bg_hex = theme.colors.background.main;
        let bg_with_alpha = (bg_hex << 8) | 0xE6;
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
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
                "enter" | "escape" => {
                    this.submit_prompt_response(prompt_id.clone(), None, cx);
                }
                _ => {}
            }
        });
        
        let bg_hex = theme.colors.background.main;
        let bg_with_alpha = (bg_hex << 8) | 0xE6;
        
        div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
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
    fn render_actions_dialog(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        
        // Simple actions dialog stub
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(theme.colors.background.main))
            .rounded(px(12.))
            .p(px(24.))
            .text_color(rgb(theme.colors.text.primary))
            .font_family(".AppleSystemUIFont")
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
            .into_any_element()
    }
}

fn start_hotkey_listener() {
    std::thread::spawn(|| {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                logging::log("HOTKEY", &format!("Failed to create hotkey manager: {}", e));
                return;
            }
        };
        
        let hotkey = HotKey::new(Some(Modifiers::META), Code::Semicolon);
        let hotkey_id = hotkey.id();
        
        if let Err(e) = manager.register(hotkey) {
            logging::log("HOTKEY", &format!("Failed to register Cmd+;: {}", e));
            return;
        }
        
        logging::log("HOTKEY", &format!("Registered global hotkey Cmd+; (id: {})", hotkey_id));
        
        let receiver = GlobalHotKeyEvent::receiver();
        
        loop {
            if let Ok(event) = receiver.recv() {
                if event.id == hotkey_id {
                    let count = HOTKEY_TRIGGER_COUNT.fetch_add(1, Ordering::SeqCst);
                    HOTKEY_TRIGGERED.store(true, Ordering::SeqCst);
                    logging::log("HOTKEY", &format!("Cmd+; pressed (trigger #{})", count + 1));
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

            logging::log(
                "PANEL",
                "Configured window as floating panel (NSFloatingWindowLevel)",
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
    
    start_hotkey_listener();
    
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
