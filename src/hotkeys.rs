use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::{config, logging, scripts, shortcuts};

// =============================================================================
// Dynamic Script Hotkey Manager
// =============================================================================

/// Manages dynamic registration/unregistration of script hotkeys.
/// Uses a thread-safe global singleton pattern for access from multiple contexts.
pub struct ScriptHotkeyManager {
    /// The underlying global hotkey manager
    manager: GlobalHotKeyManager,
    /// Maps hotkey ID -> script path
    hotkey_map: HashMap<u32, String>,
    /// Maps script path -> hotkey ID (reverse lookup for unregistration)
    path_to_id: HashMap<String, u32>,
    /// Maps script path -> HotKey object (needed for proper unregistration)
    path_to_hotkey: HashMap<String, HotKey>,
}

impl ScriptHotkeyManager {
    /// Create a new ScriptHotkeyManager.
    /// NOTE: Must be created on the main thread.
    fn new(manager: GlobalHotKeyManager) -> Self {
        Self {
            manager,
            hotkey_map: HashMap::new(),
            path_to_id: HashMap::new(),
            path_to_hotkey: HashMap::new(),
        }
    }

    /// Register a hotkey for a script.
    /// Returns the hotkey ID on success.
    pub fn register(&mut self, path: &str, shortcut: &str) -> anyhow::Result<u32> {
        // Parse the shortcut
        let (mods, code) = shortcuts::parse_shortcut(shortcut)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse shortcut: {}", shortcut))?;

        let hotkey = HotKey::new(Some(mods), code);
        let hotkey_id = hotkey.id();

        // Register with the OS
        self.manager
            .register(hotkey)
            .map_err(|e| anyhow::anyhow!("Failed to register hotkey: {}", e))?;

        // Track the mapping
        self.hotkey_map.insert(hotkey_id, path.to_string());
        self.path_to_id.insert(path.to_string(), hotkey_id);
        self.path_to_hotkey.insert(path.to_string(), hotkey);

        logging::log(
            "HOTKEY",
            &format!(
                "Registered script hotkey '{}' for {} (id: {})",
                shortcut, path, hotkey_id
            ),
        );

        Ok(hotkey_id)
    }

    /// Unregister a hotkey for a script by path.
    /// Returns Ok(()) even if the path wasn't registered (no-op).
    pub fn unregister(&mut self, path: &str) -> anyhow::Result<()> {
        if let Some(hotkey_id) = self.path_to_id.remove(path) {
            // Remove from hotkey_map
            self.hotkey_map.remove(&hotkey_id);

            // Unregister from OS using stored HotKey object
            if let Some(hotkey) = self.path_to_hotkey.remove(path) {
                if let Err(e) = self.manager.unregister(hotkey) {
                    logging::log(
                        "HOTKEY",
                        &format!(
                            "Warning: Failed to unregister hotkey for {} (id: {}): {}",
                            path, hotkey_id, e
                        ),
                    );
                    // Continue anyway - the internal tracking is already updated
                }
            }

            logging::log(
                "HOTKEY",
                &format!(
                    "Unregistered script hotkey for {} (id: {})",
                    path, hotkey_id
                ),
            );
        }
        // If path wasn't registered, this is a no-op (success)
        Ok(())
    }

    /// Update a script's hotkey.
    /// Handles add (old=None, new=Some), remove (old=Some, new=None), and change (both Some).
    pub fn update(
        &mut self,
        path: &str,
        old_shortcut: Option<&str>,
        new_shortcut: Option<&str>,
    ) -> anyhow::Result<()> {
        match (old_shortcut, new_shortcut) {
            (None, None) => {
                // No change needed
                Ok(())
            }
            (None, Some(new)) => {
                // Add new hotkey
                self.register(path, new)?;
                Ok(())
            }
            (Some(_old), None) => {
                // Remove old hotkey
                self.unregister(path)
            }
            (Some(_old), Some(new)) => {
                // Change: unregister old, register new
                self.unregister(path)?;
                self.register(path, new)?;
                Ok(())
            }
        }
    }

    /// Get the script path for a given hotkey ID.
    pub fn get_script_path(&self, hotkey_id: u32) -> Option<&String> {
        self.hotkey_map.get(&hotkey_id)
    }

    /// Get all registered hotkeys as (path, hotkey_id) pairs.
    pub fn get_registered_hotkeys(&self) -> Vec<(String, u32)> {
        self.path_to_id
            .iter()
            .map(|(path, id)| (path.clone(), *id))
            .collect()
    }

    /// Check if a script has a registered hotkey.
    #[allow(dead_code)]
    pub fn is_registered(&self, path: &str) -> bool {
        self.path_to_id.contains_key(path)
    }
}

/// Global singleton for the ScriptHotkeyManager.
/// Initialized when start_hotkey_listener is called.
static SCRIPT_HOTKEY_MANAGER: OnceLock<Mutex<ScriptHotkeyManager>> = OnceLock::new();

/// Initialize the global ScriptHotkeyManager.
/// Must be called from the main thread.
/// Returns an error if already initialized.
#[allow(dead_code)]
pub fn init_script_hotkey_manager(manager: GlobalHotKeyManager) -> anyhow::Result<()> {
    SCRIPT_HOTKEY_MANAGER
        .set(Mutex::new(ScriptHotkeyManager::new(manager)))
        .map_err(|_| anyhow::anyhow!("ScriptHotkeyManager already initialized"))
}

/// Register a script hotkey dynamically.
/// Returns the hotkey ID on success.
pub fn register_script_hotkey(path: &str, shortcut: &str) -> anyhow::Result<u32> {
    let manager = SCRIPT_HOTKEY_MANAGER
        .get()
        .ok_or_else(|| anyhow::anyhow!("ScriptHotkeyManager not initialized"))?;

    let mut guard = manager
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
    guard.register(path, shortcut)
}

/// Unregister a script hotkey by path.
/// Returns Ok(()) even if the path wasn't registered (no-op).
pub fn unregister_script_hotkey(path: &str) -> anyhow::Result<()> {
    let manager = SCRIPT_HOTKEY_MANAGER
        .get()
        .ok_or_else(|| anyhow::anyhow!("ScriptHotkeyManager not initialized"))?;

    let mut guard = manager
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
    guard.unregister(path)
}

/// Update a script's hotkey.
/// Handles add (old=None, new=Some), remove (old=Some, new=None), and change (both Some).
pub fn update_script_hotkey(
    path: &str,
    old_shortcut: Option<&str>,
    new_shortcut: Option<&str>,
) -> anyhow::Result<()> {
    let manager = SCRIPT_HOTKEY_MANAGER
        .get()
        .ok_or_else(|| anyhow::anyhow!("ScriptHotkeyManager not initialized"))?;

    let mut guard = manager
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
    guard.update(path, old_shortcut, new_shortcut)
}

/// Get the script path for a given hotkey ID.
#[allow(dead_code)]
pub fn get_script_for_hotkey(hotkey_id: u32) -> Option<String> {
    let manager = SCRIPT_HOTKEY_MANAGER.get()?;
    let guard = manager.lock().ok()?;
    guard.get_script_path(hotkey_id).cloned()
}

/// Get all registered script hotkeys.
#[allow(dead_code)]
pub fn get_registered_hotkeys() -> Vec<(String, u32)> {
    SCRIPT_HOTKEY_MANAGER
        .get()
        .and_then(|m| m.lock().ok())
        .map(|guard| guard.get_registered_hotkeys())
        .unwrap_or_default()
}

// =============================================================================
// GCD dispatch for immediate main-thread execution (bypasses async runtime)
// =============================================================================

use std::sync::Arc;

/// Callback type for hotkey actions - uses Arc<dyn Fn()> for repeated invocation
pub type HotkeyHandler = Arc<dyn Fn() + Send + Sync>;

/// Static storage for handlers to be invoked on main thread
static NOTES_HANDLER: OnceLock<std::sync::Mutex<Option<HotkeyHandler>>> = OnceLock::new();
static AI_HANDLER: OnceLock<std::sync::Mutex<Option<HotkeyHandler>>> = OnceLock::new();

/// Register a handler to be invoked when the Notes hotkey is pressed.
/// This handler will be executed on the main thread via GCD dispatch_async.
/// The handler can be called multiple times (it's not consumed).
#[allow(dead_code)]
pub fn set_notes_hotkey_handler<F: Fn() + Send + Sync + 'static>(handler: F) {
    let storage = NOTES_HANDLER.get_or_init(|| std::sync::Mutex::new(None));
    *storage.lock().unwrap() = Some(Arc::new(handler));
}

/// Register a handler to be invoked when the AI hotkey is pressed.
/// This handler will be executed on the main thread via GCD dispatch_async.
/// The handler can be called multiple times (it's not consumed).
#[allow(dead_code)]
pub fn set_ai_hotkey_handler<F: Fn() + Send + Sync + 'static>(handler: F) {
    let storage = AI_HANDLER.get_or_init(|| std::sync::Mutex::new(None));
    *storage.lock().unwrap() = Some(Arc::new(handler));
}

#[cfg(target_os = "macos")]
mod gcd {
    use std::ffi::c_void;

    // Link to libSystem for GCD functions
    // Note: dispatch_get_main_queue is actually a macro that returns &_dispatch_main_q
    // We use the raw symbol directly instead
    #[link(name = "System", kind = "framework")]
    extern "C" {
        fn dispatch_async_f(
            queue: *const c_void,
            context: *mut c_void,
            work: extern "C" fn(*mut c_void),
        );
        // The main dispatch queue is a global static symbol, not a function
        #[link_name = "_dispatch_main_q"]
        static DISPATCH_MAIN_QUEUE: c_void;
    }

    /// Dispatch a closure to the main thread via GCD.
    /// This is the key to making hotkeys work before the GPUI event loop is "warmed up".
    pub fn dispatch_to_main<F: FnOnce() + Send + 'static>(f: F) {
        let boxed: Box<dyn FnOnce() + Send> = Box::new(f);
        let raw = Box::into_raw(Box::new(boxed));

        extern "C" fn trampoline(context: *mut c_void) {
            unsafe {
                let boxed: Box<Box<dyn FnOnce() + Send>> = Box::from_raw(context as *mut _);
                boxed();
            }
        }

        unsafe {
            let main_queue = &DISPATCH_MAIN_QUEUE as *const c_void;
            dispatch_async_f(main_queue, raw as *mut c_void, trampoline);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod gcd {
    /// Fallback for non-macOS: just call the closure directly (in the current thread)
    pub fn dispatch_to_main<F: FnOnce() + Send + 'static>(f: F) {
        f();
    }
}

/// Dispatch the Notes hotkey handler to the main thread.
///
/// Strategy:
/// 1. Send to channel (wakes any async waiters)
/// 2. Dispatch a no-op to main thread via GCD (ensures GPUI event loop processes)
///
/// This works even before the main window is activated because GCD dispatch
/// directly integrates with the NSApplication run loop that GPUI uses.
fn dispatch_notes_hotkey() {
    // Send to channel - this wakes any async task waiting on recv()
    if notes_hotkey_channel().0.try_send(()).is_err() {
        logging::log("HOTKEY", "Notes hotkey channel full/closed");
    }

    // Also try the handler approach for immediate execution
    let handler = NOTES_HANDLER
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap()
        .clone();

    if let Some(handler) = handler {
        gcd::dispatch_to_main(move || {
            handler();
        });
    } else {
        // Dispatch an empty closure to wake GPUI's event loop
        // This ensures the channel message gets processed even if GPUI was idle
        gcd::dispatch_to_main(|| {
            // Empty closure - just wakes the run loop
        });
    }
}

/// Dispatch the AI hotkey handler to the main thread.
/// Same strategy as Notes hotkey.
fn dispatch_ai_hotkey() {
    // Send to channel - this wakes any async task waiting on recv()
    if ai_hotkey_channel().0.try_send(()).is_err() {
        logging::log("HOTKEY", "AI hotkey channel full/closed");
    }

    // Also try the handler approach for immediate execution
    let handler = AI_HANDLER
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap()
        .clone();

    if let Some(handler) = handler {
        gcd::dispatch_to_main(move || {
            handler();
        });
    } else {
        // Dispatch an empty closure to wake GPUI's event loop
        gcd::dispatch_to_main(|| {
            // Empty closure - just wakes the run loop
        });
    }
}

// HOTKEY_CHANNEL: Event-driven async_channel for hotkey events (replaces AtomicBool polling)
#[allow(dead_code)]
static HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();

/// Get the hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn hotkey_channel() -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>)
{
    HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

// SCRIPT_HOTKEY_CHANNEL: Channel for script shortcut events (sends script path)
#[allow(dead_code)]
static SCRIPT_HOTKEY_CHANNEL: OnceLock<(
    async_channel::Sender<String>,
    async_channel::Receiver<String>,
)> = OnceLock::new();

/// Get the script hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn script_hotkey_channel() -> &'static (
    async_channel::Sender<String>,
    async_channel::Receiver<String>,
) {
    SCRIPT_HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

// NOTES_HOTKEY_CHANNEL: Channel for notes hotkey events
#[allow(dead_code)]
static NOTES_HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();

/// Get the notes hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn notes_hotkey_channel(
) -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>) {
    NOTES_HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

// AI_HOTKEY_CHANNEL: Channel for AI hotkey events
#[allow(dead_code)]
static AI_HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();

/// Get the AI hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn ai_hotkey_channel(
) -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>) {
    AI_HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

#[allow(dead_code)]
static HOTKEY_TRIGGER_COUNT: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
pub(crate) fn start_hotkey_listener(config: config::Config) {
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
                logging::log(
                    "HOTKEY",
                    &format!(
                        "Unknown key code: '{}'. Valid keys: KeyA-KeyZ, Digit0-Digit9, F1-F12, Space, Enter, Semicolon. Falling back to Semicolon",
                        other
                    ),
                );
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
        let main_hotkey_id = hotkey.id();

        let hotkey_display = format!(
            "{}{}",
            config.hotkey.modifiers.join("+"),
            if config.hotkey.modifiers.is_empty() {
                String::new()
            } else {
                "+".to_string()
            }
        ) + &config.hotkey.key;

        if let Err(e) = manager.register(hotkey) {
            logging::log(
                "HOTKEY",
                &format!("Failed to register {}: {}", hotkey_display, e),
            );
            return;
        }

        logging::log(
            "HOTKEY",
            &format!(
                "Registered global hotkey {} (id: {})",
                hotkey_display, main_hotkey_id
            ),
        );

        // Register notes hotkey (Cmd+Shift+N by default)
        let notes_config = config.get_notes_hotkey();
        let notes_code = match notes_config.key.as_str() {
            "KeyN" => Code::KeyN,
            "KeyM" => Code::KeyM,
            "KeyO" => Code::KeyO,
            "KeyP" => Code::KeyP,
            _ => Code::KeyN, // Default to N
        };

        let mut notes_modifiers = Modifiers::empty();
        for modifier in &notes_config.modifiers {
            match modifier.as_str() {
                "meta" => notes_modifiers |= Modifiers::META,
                "ctrl" => notes_modifiers |= Modifiers::CONTROL,
                "alt" => notes_modifiers |= Modifiers::ALT,
                "shift" => notes_modifiers |= Modifiers::SHIFT,
                _ => {}
            }
        }

        let notes_hotkey = HotKey::new(Some(notes_modifiers), notes_code);
        let notes_hotkey_id = notes_hotkey.id();

        let notes_display = format!(
            "{}{}",
            notes_config.modifiers.join("+"),
            if notes_config.modifiers.is_empty() {
                String::new()
            } else {
                "+".to_string()
            }
        ) + &notes_config.key;

        if let Err(e) = manager.register(notes_hotkey) {
            logging::log(
                "HOTKEY",
                &format!("Failed to register notes hotkey {}: {}", notes_display, e),
            );
        } else {
            logging::log(
                "HOTKEY",
                &format!(
                    "Registered notes hotkey {} (id: {})",
                    notes_display, notes_hotkey_id
                ),
            );
        }

        // Register AI hotkey (Cmd+Shift+Space by default)
        let ai_config = config.get_ai_hotkey();
        let ai_code = match ai_config.key.as_str() {
            "Space" => Code::Space,
            "KeyA" => Code::KeyA,
            "KeyI" => Code::KeyI,
            _ => Code::Space, // Default to Space
        };

        let mut ai_modifiers = Modifiers::empty();
        for modifier in &ai_config.modifiers {
            match modifier.as_str() {
                "meta" => ai_modifiers |= Modifiers::META,
                "ctrl" => ai_modifiers |= Modifiers::CONTROL,
                "alt" => ai_modifiers |= Modifiers::ALT,
                "shift" => ai_modifiers |= Modifiers::SHIFT,
                _ => {}
            }
        }

        let ai_hotkey = HotKey::new(Some(ai_modifiers), ai_code);
        let ai_hotkey_id = ai_hotkey.id();

        let ai_display = format!(
            "{}{}",
            ai_config.modifiers.join("+"),
            if ai_config.modifiers.is_empty() {
                String::new()
            } else {
                "+".to_string()
            }
        ) + &ai_config.key;

        if let Err(e) = manager.register(ai_hotkey) {
            logging::log(
                "HOTKEY",
                &format!("Failed to register AI hotkey {}: {}", ai_display, e),
            );
        } else {
            logging::log(
                "HOTKEY",
                &format!("Registered AI hotkey {} (id: {})", ai_display, ai_hotkey_id),
            );
        }

        // Register script shortcuts
        // Map from hotkey ID to script path
        let mut script_hotkey_map: std::collections::HashMap<u32, String> =
            std::collections::HashMap::new();

        // Load scripts with shortcuts
        let all_scripts = scripts::read_scripts();
        for script in &all_scripts {
            if let Some(ref shortcut) = script.shortcut {
                if let Some((mods, key_code)) = shortcuts::parse_shortcut(shortcut) {
                    let script_hotkey = HotKey::new(Some(mods), key_code);
                    let script_hotkey_id = script_hotkey.id();

                    match manager.register(script_hotkey) {
                        Ok(()) => {
                            script_hotkey_map.insert(
                                script_hotkey_id,
                                script.path.to_string_lossy().to_string(),
                            );
                            logging::log(
                                "HOTKEY",
                                &format!(
                                    "Registered script shortcut '{}' for {} (id: {})",
                                    shortcut, script.name, script_hotkey_id
                                ),
                            );
                        }
                        Err(e) => {
                            logging::log(
                                "HOTKEY",
                                &format!(
                                    "Failed to register shortcut '{}' for {}: {}",
                                    shortcut, script.name, e
                                ),
                            );
                        }
                    }
                } else {
                    logging::log(
                        "HOTKEY",
                        &format!(
                            "Failed to parse shortcut '{}' for script {}",
                            shortcut, script.name
                        ),
                    );
                }
            }
        }

        // Load scriptlets with shortcuts
        let all_scriptlets = scripts::load_scriptlets();
        for scriptlet in &all_scriptlets {
            if let Some(ref shortcut) = scriptlet.shortcut {
                if let Some((mods, key_code)) = shortcuts::parse_shortcut(shortcut) {
                    let scriptlet_hotkey = HotKey::new(Some(mods), key_code);
                    let scriptlet_hotkey_id = scriptlet_hotkey.id();

                    // Use file_path as the identifier (already includes #command)
                    let scriptlet_path = scriptlet
                        .file_path
                        .clone()
                        .unwrap_or_else(|| scriptlet.name.clone());

                    match manager.register(scriptlet_hotkey) {
                        Ok(()) => {
                            script_hotkey_map.insert(scriptlet_hotkey_id, scriptlet_path.clone());
                            logging::log(
                                "HOTKEY",
                                &format!(
                                    "Registered scriptlet shortcut '{}' for {} (id: {})",
                                    shortcut, scriptlet.name, scriptlet_hotkey_id
                                ),
                            );
                        }
                        Err(e) => {
                            logging::log(
                                "HOTKEY",
                                &format!(
                                    "Failed to register shortcut '{}' for {}: {}",
                                    shortcut, scriptlet.name, e
                                ),
                            );
                        }
                    }
                }
            }
        }

        logging::log(
            "HOTKEY",
            &format!(
                "Registered {} script/scriptlet shortcuts",
                script_hotkey_map.len()
            ),
        );

        let receiver = GlobalHotKeyEvent::receiver();

        // Log all registered hotkey IDs for debugging
        logging::log(
            "HOTKEY",
            &format!(
                "Hotkey ID map: main={}, notes={}, ai={}",
                main_hotkey_id, notes_hotkey_id, ai_hotkey_id
            ),
        );

        loop {
            if let Ok(event) = receiver.recv() {
                // Only respond to key PRESS, not release
                if event.state != HotKeyState::Pressed {
                    continue;
                }

                // Log EVERY hotkey event with its ID for debugging
                logging::log(
                    "HOTKEY",
                    &format!(
                        "Received event id={} (main={}, notes={}, ai={})",
                        event.id, main_hotkey_id, notes_hotkey_id, ai_hotkey_id
                    ),
                );

                // Check if it's the main app hotkey
                if event.id == main_hotkey_id {
                    let count = HOTKEY_TRIGGER_COUNT.fetch_add(1, Ordering::SeqCst);
                    // Send via async_channel for immediate event-driven handling
                    if hotkey_channel().0.send_blocking(()).is_err() {
                        logging::log("HOTKEY", "Hotkey channel closed, cannot send");
                    }
                    logging::log(
                        "HOTKEY",
                        &format!("{} pressed (trigger #{})", hotkey_display, count + 1),
                    );
                }
                // Check if it's the notes hotkey - dispatch directly to main thread via GCD
                else if event.id == notes_hotkey_id {
                    logging::log(
                        "HOTKEY",
                        &format!(
                            "{} pressed (notes) - dispatching to main thread",
                            notes_display
                        ),
                    );
                    dispatch_notes_hotkey();
                }
                // Check if it's the AI hotkey - dispatch directly to main thread via GCD
                else if event.id == ai_hotkey_id {
                    logging::log(
                        "HOTKEY",
                        &format!("{} pressed (AI) - dispatching to main thread", ai_display),
                    );
                    dispatch_ai_hotkey();
                }
                // Check if it's a script shortcut
                else if let Some(script_path) = script_hotkey_map.get(&event.id) {
                    logging::log(
                        "HOTKEY",
                        &format!("Script shortcut triggered: {}", script_path),
                    );
                    // Send the script path to be executed
                    if script_hotkey_channel()
                        .0
                        .send_blocking(script_path.clone())
                        .is_err()
                    {
                        logging::log("HOTKEY", "Script hotkey channel closed, cannot send");
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_channel::TryRecvError;

    #[test]
    fn hotkey_channels_are_independent() {
        while hotkey_channel().1.try_recv().is_ok() {}
        while script_hotkey_channel().1.try_recv().is_ok() {}

        hotkey_channel().0.send_blocking(()).expect("send hotkey");
        assert!(matches!(
            script_hotkey_channel().1.try_recv(),
            Err(TryRecvError::Empty)
        ));
        assert!(hotkey_channel().1.try_recv().is_ok());

        script_hotkey_channel()
            .0
            .send_blocking("script".to_string())
            .expect("send script hotkey");
        assert_eq!(
            script_hotkey_channel()
                .1
                .try_recv()
                .expect("recv script hotkey"),
            "script"
        );
    }

    // =============================================================================
    // ScriptHotkeyManager Unit Tests
    // =============================================================================
    // Note: These tests cannot actually register system hotkeys in the test environment
    // because GlobalHotKeyManager requires a running event loop and proper OS permissions.
    // Instead, we test the logic of the manager's internal tracking.

    mod script_hotkey_manager_tests {
        use super::*;

        /// Helper to create a manager for testing.
        /// Note: Registration will fail without an event loop, but we can test tracking logic.
        fn create_test_manager() -> Option<ScriptHotkeyManager> {
            // GlobalHotKeyManager::new() may fail in test environment
            GlobalHotKeyManager::new()
                .ok()
                .map(ScriptHotkeyManager::new)
        }

        #[test]
        fn test_manager_creation() {
            // Just verify we can create the struct (manager creation may fail in CI)
            if let Some(manager) = create_test_manager() {
                assert!(manager.hotkey_map.is_empty());
                assert!(manager.path_to_id.is_empty());
            }
        }

        #[test]
        fn test_get_registered_hotkeys_empty() {
            if let Some(manager) = create_test_manager() {
                assert!(manager.get_registered_hotkeys().is_empty());
            }
        }

        #[test]
        fn test_is_registered_false_for_unknown_path() {
            if let Some(manager) = create_test_manager() {
                assert!(!manager.is_registered("/some/unknown/path.ts"));
            }
        }

        #[test]
        fn test_unregister_nonexistent_is_noop() {
            if let Some(mut manager) = create_test_manager() {
                // Should not error when unregistering a path that was never registered
                let result = manager.unregister("/nonexistent/path.ts");
                assert!(result.is_ok());
            }
        }

        #[test]
        fn test_update_none_to_none_is_noop() {
            if let Some(mut manager) = create_test_manager() {
                // No old, no new -> no-op, should succeed
                let result = manager.update("/some/path.ts", None, None);
                assert!(result.is_ok());
            }
        }

        // Note: The following tests would require a working GlobalHotKeyManager
        // which may not be available in all test environments.
        // In a real CI environment, these would be integration tests.

        #[test]
        fn test_register_tracks_mapping() {
            if let Some(mut manager) = create_test_manager() {
                // Try to register - this may fail in test environment, that's OK
                let result = manager.register("/test/script.ts", "cmd+shift+t");
                if result.is_ok() {
                    // If registration succeeded, verify tracking
                    assert!(manager.is_registered("/test/script.ts"));
                    let hotkeys = manager.get_registered_hotkeys();
                    assert_eq!(hotkeys.len(), 1);
                    assert_eq!(hotkeys[0].0, "/test/script.ts");
                }
                // If it failed (no event loop), that's expected in test env
            }
        }

        #[test]
        fn test_unregister_removes_tracking() {
            if let Some(mut manager) = create_test_manager() {
                // Try to register first
                if manager.register("/test/script.ts", "cmd+shift+u").is_ok() {
                    assert!(manager.is_registered("/test/script.ts"));

                    // Now unregister
                    let result = manager.unregister("/test/script.ts");
                    assert!(result.is_ok());
                    assert!(!manager.is_registered("/test/script.ts"));
                }
            }
        }

        #[test]
        fn test_update_add_hotkey() {
            if let Some(mut manager) = create_test_manager() {
                // None -> Some = add
                let result = manager.update("/test/add.ts", None, Some("cmd+shift+a"));
                if result.is_ok() {
                    assert!(manager.is_registered("/test/add.ts"));
                }
            }
        }

        #[test]
        fn test_update_remove_hotkey() {
            if let Some(mut manager) = create_test_manager() {
                // First register
                if manager.register("/test/remove.ts", "cmd+shift+r").is_ok() {
                    // Some -> None = remove
                    let result = manager.update("/test/remove.ts", Some("cmd+shift+r"), None);
                    assert!(result.is_ok());
                    assert!(!manager.is_registered("/test/remove.ts"));
                }
            }
        }

        #[test]
        fn test_update_change_hotkey() {
            if let Some(mut manager) = create_test_manager() {
                // First register with old shortcut
                if manager.register("/test/change.ts", "cmd+shift+c").is_ok() {
                    // Some -> Some (different) = change
                    let result =
                        manager.update("/test/change.ts", Some("cmd+shift+c"), Some("cmd+alt+c"));
                    if result.is_ok() {
                        // Should still be registered (with new shortcut)
                        assert!(manager.is_registered("/test/change.ts"));
                    }
                }
            }
        }

        #[test]
        fn test_get_script_path() {
            if let Some(mut manager) = create_test_manager() {
                if let Ok(hotkey_id) = manager.register("/test/lookup.ts", "cmd+shift+l") {
                    let path = manager.get_script_path(hotkey_id);
                    assert_eq!(path, Some(&"/test/lookup.ts".to_string()));

                    // Unknown ID returns None
                    assert!(manager.get_script_path(99999).is_none());
                }
            }
        }
    }
}
