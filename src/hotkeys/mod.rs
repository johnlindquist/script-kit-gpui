//! Global hotkey registration and routing for launcher, notes, AI, and script shortcuts.
//! Key APIs include `HotkeyAction`, `update_hotkeys`, script hotkey registration helpers,
//! and dynamic shortcut registration/unregistration utilities.
//! This module depends on `config`, `scripts`, `shortcuts`, and `logging`, and is used by app startup/reload flows.

pub mod gesture;

// --- merged from part_000.rs ---
use crate::{config, logging, scripts, shortcuts};
use gesture::{GestureClassifier, GestureEvent, GestureInput};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    Error as HotkeyError, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use uuid::Uuid;

const SCRIPT_KIT_SELFIE_COMMAND_ID: &str = "builtin/script-kit-selfie";
const SCRIPT_KIT_SELFIE_SHORTCUT: &str = "cmd+alt+1";
// =============================================================================
// Unified Hotkey Routing System
// =============================================================================
// All hotkey events (main, notes, ai, scripts) are dispatched through a single
// routing table. This ensures:
// 1. Consistent dispatch behavior for all hotkey types
// 2. Proper hot-reload support (routing and registration are coupled)
// 3. No lost hotkeys on failed registration (transactional updates)

/// Action to take when a hotkey is pressed
#[derive(Clone, Debug, PartialEq)]
pub enum HotkeyAction {
    /// Main launcher hotkey
    Main,
    /// Notes window hotkey
    Notes,
    /// AI window hotkey
    Ai,
    /// Toggle log capture hotkey
    ToggleLogs,
    /// Voice dictation toggle hotkey
    Dictation,
    /// Inline AI focused-text editor
    InlineAiTextEdit,
    /// Instant rewrite: capture focused text and stream rewrite variations
    RewriteText,
    /// Dev-only marker capture while launched from ./dev.sh.
    DevMarker,
    /// Script shortcut - run the script at this path
    Script(String),
}
/// Registered hotkey entry with all needed data for unregistration/updates
#[derive(Clone)]
struct RegisteredHotkey {
    /// The HotKey object (needed for unregister)
    hotkey: HotKey,
    /// What action to take on press
    action: HotkeyAction,
    /// Display string for logging (e.g., "cmd+shift+k")
    display: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutConflict {
    pub command_id: String,
    pub command_name: String,
    pub shortcut: String,
}
/// Unified routing table for all hotkeys
/// Uses RwLock for fast reads (event dispatch) with occasional writes (updates)
struct HotkeyRoutes {
    /// Maps hotkey ID -> registered hotkey entry
    routes: HashMap<u32, RegisteredHotkey>,
    /// Reverse lookup: script path -> hotkey ID (for script updates)
    script_paths: HashMap<String, u32>,
    /// Current main hotkey ID (for quick lookup)
    main_id: Option<u32>,
    /// Current notes hotkey ID (for quick lookup)
    notes_id: Option<u32>,
    /// Current AI hotkey ID (for quick lookup)
    ai_id: Option<u32>,
    /// Current logs hotkey ID (for quick lookup)
    logs_id: Option<u32>,
    /// Current dictation hotkey ID (for quick lookup)
    dictation_id: Option<u32>,
    /// Current inline AI text-edit hotkey ID (for quick lookup)
    inline_ai_id: Option<u32>,
    /// Current rewrite hotkey ID (for quick lookup)
    rewrite_id: Option<u32>,
    /// Current dev marker hotkey ID (for quick lookup)
    dev_marker_id: Option<u32>,
}
impl HotkeyRoutes {
    fn new() -> Self {
        Self {
            routes: HashMap::new(),
            script_paths: HashMap::new(),
            main_id: None,
            notes_id: None,
            ai_id: None,
            logs_id: None,
            dictation_id: None,
            inline_ai_id: None,
            rewrite_id: None,
            dev_marker_id: None,
        }
    }

    /// Get the action for a hotkey ID
    fn get_action(&self, id: u32) -> Option<HotkeyAction> {
        self.routes.get(&id).map(|r| r.action.clone())
    }

    /// Add a route (internal - doesn't register with OS)
    fn add_route(&mut self, id: u32, entry: RegisteredHotkey) {
        match &entry.action {
            HotkeyAction::Main => self.main_id = Some(id),
            HotkeyAction::Notes => self.notes_id = Some(id),
            HotkeyAction::Ai => self.ai_id = Some(id),
            HotkeyAction::ToggleLogs => self.logs_id = Some(id),
            HotkeyAction::Dictation => self.dictation_id = Some(id),
            HotkeyAction::InlineAiTextEdit => self.inline_ai_id = Some(id),
            HotkeyAction::RewriteText => self.rewrite_id = Some(id),
            HotkeyAction::DevMarker => self.dev_marker_id = Some(id),
            HotkeyAction::Script(path) => {
                self.script_paths.insert(path.clone(), id);
            }
        }
        self.routes.insert(id, entry);
    }

    /// Remove a route by ID (internal - doesn't unregister from OS)
    fn remove_route(&mut self, id: u32) -> Option<RegisteredHotkey> {
        if let Some(entry) = self.routes.remove(&id) {
            match &entry.action {
                HotkeyAction::Main => {
                    if self.main_id == Some(id) {
                        self.main_id = None;
                    }
                }
                HotkeyAction::Notes => {
                    if self.notes_id == Some(id) {
                        self.notes_id = None;
                    }
                }
                HotkeyAction::Ai => {
                    if self.ai_id == Some(id) {
                        self.ai_id = None;
                    }
                }
                HotkeyAction::ToggleLogs => {
                    if self.logs_id == Some(id) {
                        self.logs_id = None;
                    }
                }
                HotkeyAction::Dictation => {
                    if self.dictation_id == Some(id) {
                        self.dictation_id = None;
                    }
                }
                HotkeyAction::InlineAiTextEdit => {
                    if self.inline_ai_id == Some(id) {
                        self.inline_ai_id = None;
                    }
                }
                HotkeyAction::RewriteText => {
                    if self.rewrite_id == Some(id) {
                        self.rewrite_id = None;
                    }
                }
                HotkeyAction::DevMarker => {
                    if self.dev_marker_id == Some(id) {
                        self.dev_marker_id = None;
                    }
                }
                HotkeyAction::Script(path) => {
                    self.script_paths.remove(path);
                }
            }
            Some(entry)
        } else {
            None
        }
    }

    /// Get script hotkey ID by path
    fn get_script_id(&self, path: &str) -> Option<u32> {
        self.script_paths.get(path).copied()
    }

    /// Get the hotkey entry for an action type
    #[allow(dead_code)]
    fn get_builtin_entry(&self, action: &HotkeyAction) -> Option<&RegisteredHotkey> {
        let id = match action {
            HotkeyAction::Main => self.main_id?,
            HotkeyAction::Notes => self.notes_id?,
            HotkeyAction::Ai => self.ai_id?,
            HotkeyAction::ToggleLogs => self.logs_id?,
            HotkeyAction::Dictation => self.dictation_id?,
            HotkeyAction::InlineAiTextEdit => self.inline_ai_id?,
            HotkeyAction::RewriteText => self.rewrite_id?,
            HotkeyAction::DevMarker => self.dev_marker_id?,
            HotkeyAction::Script(path) => *self.script_paths.get(path)?,
        };
        self.routes.get(&id)
    }
}

/// Byte budget for one route-summary log line's message content. The
/// timestamp/level/cid prefix adds ~60-80 bytes; keeping content under this
/// keeps the full emitted line below the 2048-byte log-hygiene cap that
/// root-typing-lag-benchmark enforces (oversized single lines break
/// line-oriented log tooling).
const ROUTE_SUMMARY_LINE_BUDGET: usize = 1900;

/// Split the hotkey route summary into a header line plus
/// `GLOBAL_HOTKEY_ROUTE_DETAIL` continuation lines, each within
/// [`ROUTE_SUMMARY_LINE_BUDGET`] bytes of content.
fn route_summary_lines(header: String, details: &[String]) -> Vec<String> {
    let mut lines = vec![header];
    let mut chunk: Vec<&str> = Vec::new();
    let mut chunk_len = 0usize;
    let flush = |chunk: &mut Vec<&str>, chunk_len: &mut usize, lines: &mut Vec<String>| {
        if !chunk.is_empty() {
            lines.push(format!(
                "GLOBAL_HOTKEY_ROUTE_DETAIL routes=[{}]",
                chunk.join(",")
            ));
            chunk.clear();
            *chunk_len = 0;
        }
    };
    for detail in details {
        // +1 for the joining comma; ~40 bytes for the detail-line scaffold.
        let projected = chunk_len + detail.len() + 1 + 40;
        if !chunk.is_empty() && projected > ROUTE_SUMMARY_LINE_BUDGET {
            flush(&mut chunk, &mut chunk_len, &mut lines);
        }
        chunk_len += detail.len() + 1;
        chunk.push(detail.as_str());
    }
    flush(&mut chunk, &mut chunk_len, &mut lines);
    lines
}

fn hotkey_action_label(action: &HotkeyAction) -> String {
    match action {
        HotkeyAction::Main => "Main launcher".to_string(),
        HotkeyAction::Notes => "Notes".to_string(),
        HotkeyAction::Ai => "Agent Chat".to_string(),
        HotkeyAction::ToggleLogs => "Logs".to_string(),
        HotkeyAction::Dictation => "Dictation".to_string(),
        HotkeyAction::InlineAiTextEdit => "Inline AI Text Edit".to_string(),
        HotkeyAction::RewriteText => "Rewrite".to_string(),
        HotkeyAction::DevMarker => "Dev Marker".to_string(),
        HotkeyAction::Script(command_id) => command_id.clone(),
    }
}

fn shortcut_conflict_in_routes(
    routes: &HotkeyRoutes,
    command_id: &str,
    shortcut: &str,
) -> Option<ShortcutConflict> {
    let (mods, code) = shortcuts::parse_shortcut(shortcut)?;
    let hotkey = HotKey::new(Some(mods), code);
    let entry = routes.routes.get(&hotkey.id())?;

    if matches!(&entry.action, HotkeyAction::Script(existing) if existing == command_id) {
        return None;
    }

    let conflicting_command_id = match &entry.action {
        HotkeyAction::Script(existing) => existing.clone(),
        other => format!("{:?}", other),
    };

    Some(ShortcutConflict {
        command_id: conflicting_command_id,
        command_name: hotkey_action_label(&entry.action),
        shortcut: entry.display.clone(),
    })
}

pub fn shortcut_conflict_for_recording(
    command_id: &str,
    shortcut: &str,
) -> Option<ShortcutConflict> {
    let routes_guard = routes().read();
    shortcut_conflict_in_routes(&routes_guard, command_id, shortcut)
}
/// Global routing table - protected by RwLock for fast reads
static HOTKEY_ROUTES: LazyLock<RwLock<HotkeyRoutes>> =
    LazyLock::new(|| RwLock::new(HotkeyRoutes::new()));
fn routes() -> &'static RwLock<HotkeyRoutes> {
    &HOTKEY_ROUTES
}
/// The main GlobalHotKeyManager - stored globally so update_hotkeys can access it
static MAIN_MANAGER: OnceLock<Mutex<GlobalHotKeyManager>> = OnceLock::new();
/// Parse a HotkeyConfig into (Modifiers, Code)
fn parse_hotkey_config(hk: &config::HotkeyConfig) -> Option<(Modifiers, Code)> {
    let code = match hk.key.as_str() {
        // Letter keys
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
        "KeyK" => Code::KeyK,
        "KeyL" => Code::KeyL,
        "KeyM" => Code::KeyM,
        "KeyN" => Code::KeyN,
        "KeyO" => Code::KeyO,
        "KeyP" => Code::KeyP,
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
        // Number keys
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
        // Navigation keys
        "Space" => Code::Space,
        "Enter" => Code::Enter,
        "Tab" => Code::Tab,
        "Escape" => Code::Escape,
        "Backspace" => Code::Backspace,
        "Delete" => Code::Delete,
        "ArrowUp" => Code::ArrowUp,
        "ArrowDown" => Code::ArrowDown,
        "ArrowLeft" => Code::ArrowLeft,
        "ArrowRight" => Code::ArrowRight,
        "Home" => Code::Home,
        "End" => Code::End,
        "PageUp" => Code::PageUp,
        "PageDown" => Code::PageDown,
        "Insert" => Code::Insert,
        // Punctuation keys
        "Semicolon" => Code::Semicolon,
        "Quote" => Code::Quote,
        "Comma" => Code::Comma,
        "Period" => Code::Period,
        "Slash" => Code::Slash,
        "Backslash" => Code::Backslash,
        "BracketLeft" => Code::BracketLeft,
        "BracketRight" => Code::BracketRight,
        "Minus" => Code::Minus,
        "Equal" => Code::Equal,
        "Backquote" => Code::Backquote,
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
        "F13" => Code::F13,
        "F14" => Code::F14,
        "F15" => Code::F15,
        "F16" => Code::F16,
        "F17" => Code::F17,
        "F18" => Code::F18,
        "F19" => Code::F19,
        "F20" => Code::F20,
        "F21" => Code::F21,
        "F22" => Code::F22,
        "F23" => Code::F23,
        "F24" => Code::F24,
        // Lock/system keys
        "CapsLock" => Code::CapsLock,
        "NumLock" => Code::NumLock,
        "ScrollLock" => Code::ScrollLock,
        "PrintScreen" => Code::PrintScreen,
        "Pause" => Code::Pause,
        _ => return None,
    };

    let mut modifiers = Modifiers::empty();
    for modifier in &hk.modifiers {
        match modifier.as_str() {
            "meta" => modifiers |= Modifiers::META,
            "ctrl" => modifiers |= Modifiers::CONTROL,
            "alt" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            _ => {}
        }
    }

    Some((modifiers, code))
}
/// Convert a HotkeyConfig to a display string (e.g., "meta+shift+N")
fn hotkey_config_to_display(hk: &config::HotkeyConfig) -> String {
    format!(
        "{}{}{}",
        hk.modifiers.join("+"),
        if hk.modifiers.is_empty() { "" } else { "+" },
        hk.key
    )
}
/// Transactional hotkey rebind: register new BEFORE unregistering old
/// This prevents losing a working hotkey if the new registration fails
#[tracing::instrument(skip(manager, display), fields(action = ?action))]
fn rebind_hotkey_transactional(
    manager: &GlobalHotKeyManager,
    action: HotkeyAction,
    mods: Modifiers,
    code: Code,
    display: &str,
) -> bool {
    let new_hotkey = HotKey::new(Some(mods), code);
    let new_id = new_hotkey.id();
    let action_label = hotkey_action_label(&action);

    // Check if already registered with same ID (no change needed)
    let current_id = {
        let routes_guard = routes().read();
        match &action {
            HotkeyAction::Main => routes_guard.main_id,
            HotkeyAction::Notes => routes_guard.notes_id,
            HotkeyAction::Ai => routes_guard.ai_id,
            HotkeyAction::ToggleLogs => routes_guard.logs_id,
            HotkeyAction::Dictation => routes_guard.dictation_id,
            HotkeyAction::InlineAiTextEdit => routes_guard.inline_ai_id,
            HotkeyAction::RewriteText => routes_guard.rewrite_id,
            HotkeyAction::DevMarker => routes_guard.dev_marker_id,
            HotkeyAction::Script(path) => routes_guard.get_script_id(path),
        }
    };

    if current_id == Some(new_id) {
        logging::log(
            "KEY_SETUP",
            &format!(
                "GLOBAL_HOTKEY_REGISTER_SKIP reason=already_registered action={} display={} id={}",
                action_label, display, new_id
            ),
        );
        return true; // No change needed
    }

    logging::log(
        "KEY_SETUP",
        &format!(
            "GLOBAL_HOTKEY_REGISTER_ATTEMPT action={} display={} id={} mods={:?} code={:?}",
            action_label, display, new_id, mods, code
        ),
    );

    // TRANSACTIONAL: Register new FIRST, before unregistering old
    // This ensures we never lose a working hotkey on registration failure
    if let Err(e) = manager.register(new_hotkey) {
        logging::log(
            "KEY_SETUP",
            &format!(
                "GLOBAL_HOTKEY_REGISTER_FAILED action={} display={} id={} error={}",
                action_label, display, new_id, e
            ),
        );
        logging::log(
            "HOTKEY",
            &format!("Failed to register {}: {} - keeping existing", display, e),
        );
        return false;
    }

    // New registration succeeded - now safe to update routing and unregister old
    let old_entry = {
        let mut routes_guard = routes().write();

        // Get old entry before adding new (they might have same action type)
        let old_id = match &action {
            HotkeyAction::Main => routes_guard.main_id,
            HotkeyAction::Notes => routes_guard.notes_id,
            HotkeyAction::Ai => routes_guard.ai_id,
            HotkeyAction::ToggleLogs => routes_guard.logs_id,
            HotkeyAction::Dictation => routes_guard.dictation_id,
            HotkeyAction::InlineAiTextEdit => routes_guard.inline_ai_id,
            HotkeyAction::RewriteText => routes_guard.rewrite_id,
            HotkeyAction::DevMarker => routes_guard.dev_marker_id,
            HotkeyAction::Script(path) => routes_guard.get_script_id(path),
        };
        let old_entry = old_id.and_then(|id| routes_guard.remove_route(id));

        // Add new route
        routes_guard.add_route(
            new_id,
            RegisteredHotkey {
                hotkey: new_hotkey,
                action: action.clone(),
                display: display.to_string(),
            },
        );

        old_entry
    };

    // Unregister old hotkey (best-effort - it's already removed from routing)
    let old_replaced = old_entry.is_some();
    if let Some(old) = old_entry {
        if let Err(e) = manager.unregister(old.hotkey) {
            logging::log(
                "HOTKEY",
                &format!(
                    "Warning: failed to unregister old {} hotkey: {}",
                    old.display, e
                ),
            );
            // Continue anyway - new hotkey is working
        }
    }

    logging::log(
        "KEY_SETUP",
        &format!(
            "GLOBAL_HOTKEY_REGISTERED action={} display={} id={} old_replaced={}",
            action_label, display, new_id, old_replaced
        ),
    );
    true
}
/// Update hotkeys from config - call this when config changes
/// Uses transactional updates: register new before unregistering old
#[tracing::instrument(skip_all)]
pub fn update_hotkeys(cfg: &config::Config) {
    let manager_guard = match MAIN_MANAGER.get() {
        Some(m) => match m.lock() {
            Ok(g) => g,
            Err(e) => {
                logging::log("HOTKEY", &format!("Failed to lock manager: {}", e));
                return;
            }
        },
        None => {
            logging::log("HOTKEY", "Manager not initialized - hotkeys not updated");
            return;
        }
    };

    // Update main hotkey
    let main_config = &cfg.hotkey;
    if let Some((mods, code)) = parse_hotkey_config(main_config) {
        let display = hotkey_config_to_display(main_config);
        let success =
            rebind_hotkey_transactional(&manager_guard, HotkeyAction::Main, mods, code, &display);
        MAIN_HOTKEY_REGISTERED.store(success, Ordering::Relaxed);
    }

    // Update notes hotkey (only if configured - no default)
    if let Some(notes_config) = cfg.get_notes_hotkey() {
        if let Some((mods, code)) = parse_hotkey_config(&notes_config) {
            let display = hotkey_config_to_display(&notes_config);
            rebind_hotkey_transactional(&manager_guard, HotkeyAction::Notes, mods, code, &display);
        }
    }

    // Update AI hotkey
    if let Some(ai_config) = cfg.get_ai_hotkey() {
        if let Some((mods, code)) = parse_hotkey_config(&ai_config) {
            let display = hotkey_config_to_display(&ai_config);
            rebind_hotkey_transactional(&manager_guard, HotkeyAction::Ai, mods, code, &display);
        }
    }

    // Update dictation hotkey
    if let Some(dictation_config) = cfg.get_dictation_hotkey() {
        if let Some((mods, code)) = parse_hotkey_config(&dictation_config) {
            let display = hotkey_config_to_display(&dictation_config);
            rebind_hotkey_transactional(
                &manager_guard,
                HotkeyAction::Dictation,
                mods,
                code,
                &display,
            );
        }
    }

    // Update logs hotkey
    if let Some(logs_config) = cfg.get_logs_hotkey() {
        if let Some((mods, code)) = parse_hotkey_config(&logs_config) {
            let display = hotkey_config_to_display(&logs_config);
            rebind_hotkey_transactional(
                &manager_guard,
                HotkeyAction::ToggleLogs,
                mods,
                code,
                &display,
            );
        }
    }

    // Update inline AI focused-text hotkey
    if let Some(inline_ai_config) = cfg.get_inline_ai_hotkey() {
        if let Some((mods, code)) = parse_hotkey_config(&inline_ai_config) {
            let display = hotkey_config_to_display(&inline_ai_config);
            rebind_hotkey_transactional(
                &manager_guard,
                HotkeyAction::InlineAiTextEdit,
                mods,
                code,
                &display,
            );
        }
    }

    // Update rewrite hotkey
    if let Some(rewrite_config) = cfg.get_rewrite_hotkey() {
        if let Some((mods, code)) = parse_hotkey_config(&rewrite_config) {
            let display = hotkey_config_to_display(&rewrite_config);
            rebind_hotkey_transactional(
                &manager_guard,
                HotkeyAction::RewriteText,
                mods,
                code,
                &display,
            );
        }
    }
}
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
// --- merged from part_001.rs ---
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

        // Register with the OS - provide specific error messages based on error type
        if let Err(e) = self.manager.register(hotkey) {
            return Err(match e {
                HotkeyError::AlreadyRegistered(hk) => {
                    anyhow::anyhow!(
                        "Hotkey '{}' is already registered (conflict with another app or script). Hotkey ID: {}",
                        shortcut,
                        hk.id()
                    )
                }
                HotkeyError::FailedToRegister(msg) => {
                    anyhow::anyhow!(
                        "System rejected hotkey '{}': {}. This may be reserved by macOS or another app.",
                        shortcut,
                        msg
                    )
                }
                HotkeyError::OsError(os_err) => {
                    anyhow::anyhow!("OS error registering hotkey '{}': {}", shortcut, os_err)
                }
                other => {
                    anyhow::anyhow!("Failed to register hotkey '{}': {}", shortcut, other)
                }
            });
        }

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
    update_script_hotkey(path, None, Some(shortcut))?;

    routes()
        .read()
        .get_script_id(path)
        .ok_or_else(|| anyhow::anyhow!("No script hotkey registered for {}", path))
}

fn unregister_script_hotkey_with_manager(
    manager: &GlobalHotKeyManager,
    path: &str,
) -> anyhow::Result<()> {
    let old_entry = {
        let mut routes_guard = routes().write();
        let old_id = routes_guard.get_script_id(path);
        old_id.and_then(|id| routes_guard.remove_route(id))
    };

    if let Some(old) = old_entry {
        if let Err(error) = manager.unregister(old.hotkey) {
            logging::log(
                "HOTKEY",
                &format!(
                    "Warning: failed to unregister old {} hotkey: {}",
                    old.display, error
                ),
            );
        }

        logging::log(
            "HOTKEY",
            &format!("Unregistered script shortcut for {}", path),
        );
    }

    Ok(())
}
/// Unregister a script hotkey by path.
/// Returns Ok(()) even if the path wasn't registered (no-op).
pub fn unregister_script_hotkey(path: &str) -> anyhow::Result<()> {
    let manager = MAIN_MANAGER
        .get()
        .ok_or_else(|| anyhow::anyhow!("Hotkey manager not initialized"))?;

    let guard = manager
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
    unregister_script_hotkey_with_manager(&guard, path)
}
/// Update a script's hotkey.
/// Handles add (old=None, new=Some), remove (old=Some, new=None), and change (both Some).
pub fn update_script_hotkey(
    path: &str,
    old_shortcut: Option<&str>,
    new_shortcut: Option<&str>,
) -> anyhow::Result<()> {
    logging::log(
        "KEY_SETUP",
        &format!(
            "SCRIPT_HOTKEY_UPDATE_REQUEST path={} old_shortcut={:?} new_shortcut={:?}",
            path, old_shortcut, new_shortcut
        ),
    );

    let manager = MAIN_MANAGER
        .get()
        .ok_or_else(|| anyhow::anyhow!("Hotkey manager not initialized"))?;

    let guard = manager
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;

    match (old_shortcut, new_shortcut) {
        (None, None) => Ok(()),
        (Some(_old), None) => unregister_script_hotkey_with_manager(&guard, path),
        (old, Some(new)) => {
            let (mods, code) = shortcuts::parse_shortcut(new)
                .ok_or_else(|| anyhow::anyhow!("Failed to parse shortcut: {}", new))?;
            logging::log(
                "KEY_SETUP",
                &format!(
                    "SCRIPT_HOTKEY_UPDATE_PARSED path={} shortcut={} mods={:?} code={:?}",
                    path, new, mods, code
                ),
            );
            if rebind_hotkey_transactional(
                &guard,
                HotkeyAction::Script(path.to_string()),
                mods,
                code,
                new,
            ) {
                let verb = if old.is_some() {
                    "Updated"
                } else {
                    "Registered"
                };
                logging::log(
                    "KEY_SETUP",
                    &format!("{} script shortcut '{}' for {}", verb, new, path),
                );
                Ok(())
            } else {
                logging::log(
                    "KEY_SETUP",
                    &format!(
                        "SCRIPT_HOTKEY_UPDATE_FAILED path={} shortcut={} reason=rebind_failed",
                        path, new
                    ),
                );
                Err(anyhow::anyhow!("Failed to register hotkey '{}'", new))
            }
        }
    }
}
/// Get the script path for a given hotkey ID.
#[allow(dead_code)]
pub fn get_script_for_hotkey(hotkey_id: u32) -> Option<String> {
    let guard = routes().read();
    guard
        .routes
        .get(&hotkey_id)
        .and_then(|entry| match &entry.action {
            HotkeyAction::Script(path) => Some(path.clone()),
            HotkeyAction::InlineAiTextEdit => None,
            _ => None,
        })
}
/// Get all registered script hotkeys.
#[allow(dead_code)]
pub fn get_registered_hotkeys() -> Vec<(String, u32)> {
    let guard = routes().read();
    guard
        .script_paths
        .iter()
        .map(|(path, id)| (path.clone(), *id))
        .collect()
}
// =============================================================================
// Dynamic shortcut registration (for config-backed runtime updates)
// =============================================================================

/// Register a shortcut dynamically for a command (scriptlet, builtin, app).
/// This adds the shortcut to the unified routing table and registers with the OS.
///
/// # Arguments
/// * `command_id` - Unique identifier (e.g., "scriptlet/my-scriptlet", "builtin/ai-chat")
/// * `shortcut` - Shortcut string (e.g., "cmd+shift+k")
/// * `display_name` - Human-readable name for logging
///
/// # Returns
/// The hotkey ID on success, or an error if registration fails.
pub fn register_dynamic_shortcut(
    command_id: &str,
    shortcut: &str,
    display_name: &str,
) -> anyhow::Result<u32> {
    logging::log(
        "KEY_SETUP",
        &format!(
            "DYNAMIC_SHORTCUT_REGISTER_REQUEST command_id={} display_name={} shortcut={}",
            command_id, display_name, shortcut
        ),
    );

    let manager = MAIN_MANAGER
        .get()
        .ok_or_else(|| anyhow::anyhow!("Hotkey manager not initialized"))?;

    let manager_guard = manager
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;

    let (mods, code) = shortcuts::parse_shortcut(shortcut)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse shortcut: {}", shortcut))?;

    let hotkey = HotKey::new(Some(mods), code);
    let id = hotkey.id();

    // Check if already registered
    {
        let routes_guard = routes().read();
        if routes_guard.get_action(id).is_some() {
            let existing = routes_guard
                .routes
                .get(&id)
                .map(|entry| hotkey_action_label(&entry.action))
                .unwrap_or_else(|| "<unknown>".to_string());
            logging::log(
                "KEY_SETUP",
                &format!(
                    "DYNAMIC_SHORTCUT_REGISTER_CONFLICT command_id={} shortcut={} id={} existing_action={}",
                    command_id, shortcut, id, existing
                ),
            );
            return Err(anyhow::anyhow!(
                "Shortcut '{}' is already registered",
                shortcut
            ));
        }
    }

    // Register with OS
    logging::log(
        "KEY_SETUP",
        &format!(
            "DYNAMIC_SHORTCUT_OS_REGISTER_ATTEMPT command_id={} shortcut={} id={} mods={:?} code={:?}",
            command_id, shortcut, id, mods, code
        ),
    );
    if let Err(e) = manager_guard.register(hotkey) {
        logging::log(
            "KEY_SETUP",
            &format!(
                "DYNAMIC_SHORTCUT_OS_REGISTER_FAILED command_id={} shortcut={} id={} error={}",
                command_id, shortcut, id, e
            ),
        );
        return Err(anyhow::anyhow!(
            "Failed to register hotkey '{}': {}",
            shortcut,
            e
        ));
    }

    // Add to routing table
    {
        let mut routes_guard = routes().write();
        routes_guard.add_route(
            id,
            RegisteredHotkey {
                hotkey,
                action: HotkeyAction::Script(command_id.to_string()),
                display: shortcut.to_string(),
            },
        );
    }

    logging::log(
        "KEY_SETUP",
        &format!(
            "DYNAMIC_SHORTCUT_REGISTERED command_id={} display_name={} shortcut={} id={}",
            command_id, display_name, shortcut, id
        ),
    );

    Ok(id)
}

/// Unregister a dynamic shortcut by command_id.
/// Returns Ok(()) even if the shortcut wasn't registered (no-op).
#[allow(dead_code)]
pub fn unregister_dynamic_shortcut(command_id: &str) -> anyhow::Result<()> {
    // Remove app routing first so a removed config shortcut cannot invoke the command
    // even if the OS/global unregister call fails.
    let Some((id, hotkey)) = ({
        let mut routes_guard = routes().write();
        routes_guard.get_script_id(command_id).and_then(|id| {
            routes_guard
                .remove_route(id)
                .map(|entry| (id, entry.hotkey))
        })
    }) else {
        logging::log(
            "HOTKEY",
            &format!(
                "No dynamic shortcut registered for {}; unregister treated as no-op",
                command_id
            ),
        );
        return Ok(());
    };

    let Some(manager) = MAIN_MANAGER.get() else {
        logging::log(
            "HOTKEY",
            &format!(
                "Removed dynamic shortcut route for {} (id: {}), but hotkey manager was unavailable",
                command_id, id
            ),
        );
        return Ok(());
    };

    let manager_guard = manager
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;

    // Unregister from OS after route removal; failure is recoverable because dispatch no longer maps the command.
    if let Err(e) = manager_guard.unregister(hotkey) {
        logging::log(
            "HOTKEY",
            &format!(
                "Warning: Failed to unregister hotkey for {}: {}",
                command_id, e
            ),
        );
    }

    logging::log(
        "HOTKEY",
        &format!(
            "Unregistered dynamic shortcut for {} (id: {})",
            command_id, id
        ),
    );

    Ok(())
}
// =============================================================================
// GCD dispatch for immediate main-thread execution (bypasses async runtime)
// =============================================================================

use std::sync::Arc;
/// Callback type for hotkey actions - uses Arc<dyn Fn()> for repeated invocation
pub type HotkeyHandler = Arc<dyn Fn() + Send + Sync>;
/// Static storage for handlers to be invoked on main thread
static NOTES_HANDLER: LazyLock<std::sync::Mutex<Option<HotkeyHandler>>> =
    LazyLock::new(|| std::sync::Mutex::new(None));
static AI_HANDLER: LazyLock<std::sync::Mutex<Option<HotkeyHandler>>> =
    LazyLock::new(|| std::sync::Mutex::new(None));
/// Register a handler to be invoked when the Notes hotkey is pressed.
/// This handler will be executed on the main thread via GCD dispatch_async.
/// The handler can be called multiple times (it's not consumed).
#[allow(dead_code)]
pub fn set_notes_hotkey_handler<F: Fn() + Send + Sync + 'static>(handler: F) {
    let storage = &*NOTES_HANDLER;
    *storage.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(handler));
}
/// Register a handler to be invoked when the AI hotkey is pressed.
/// This handler will be executed on the main thread via GCD dispatch_async.
/// The handler can be called multiple times (it's not consumed).
#[allow(dead_code)]
pub fn set_ai_hotkey_handler<F: Fn() + Send + Sync + 'static>(handler: F) {
    let storage = &*AI_HANDLER;
    *storage.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(handler));
}
fn clone_hotkey_handler_with_poison_recovery(
    storage: &std::sync::Mutex<Option<HotkeyHandler>>,
    handler_kind: &'static str,
) -> Option<HotkeyHandler> {
    match storage.lock() {
        Ok(handler) => handler.clone(),
        Err(poisoned) => {
            tracing::warn!(
                handler_kind = %handler_kind,
                attempted = "clone_hotkey_handler",
                state = "poisoned_lock",
                "Recovered from poisoned hotkey handler mutex"
            );
            poisoned.into_inner().clone()
        }
    }
}
#[cfg(target_os = "macos")]
mod gcd {
    use std::ffi::c_void;
    use std::panic::{catch_unwind, AssertUnwindSafe};

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
    ///
    /// SAFETY: The trampoline uses catch_unwind to prevent panics from unwinding
    /// across the FFI boundary, which would be undefined behavior.
    pub fn dispatch_to_main<F: FnOnce() + Send + 'static>(f: F) {
        let boxed: Box<dyn FnOnce() + Send> = Box::new(f);
        let raw = Box::into_raw(Box::new(boxed));

        extern "C" fn trampoline(context: *mut c_void) {
            // SAFETY: `context` is a valid pointer created by Box::into_raw in
            // dispatch_to_main. We take ownership back via Box::from_raw. The
            // catch_unwind prevents panics from unwinding across the FFI boundary.
            unsafe {
                let boxed: Box<Box<dyn FnOnce() + Send>> = Box::from_raw(context as *mut _);
                // CRITICAL: Catch panics to prevent UB from unwinding across FFI boundary
                let result = catch_unwind(AssertUnwindSafe(|| {
                    boxed();
                }));
                if let Err(e) = result {
                    // Log the panic but don't propagate it across FFI
                    let msg = if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic".to_string()
                    };
                    eprintln!("[HOTKEY] PANIC in GCD dispatch: {}", msg);
                }
            }
        }

        // SAFETY: DISPATCH_MAIN_QUEUE is a valid static GCD queue symbol.
        // `raw` is a valid pointer from Box::into_raw; ownership transfers to
        // the trampoline which will reclaim it via Box::from_raw.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HotkeyEvent {
    pub correlation_id: String,
}

/// Physical phase of the main launcher hotkey (key-down / key-up).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainHotkeyPhase {
    KeyDown,
    KeyUp,
}

/// Main hotkey physical event with an injected timestamp for gesture classification.
#[derive(Debug, Clone)]
pub struct MainHotkeyPhysicalEvent {
    pub phase: MainHotkeyPhase,
    pub correlation_id: String,
    pub at: std::time::Instant,
}

static MAIN_GESTURE_CLASSIFIER: LazyLock<Mutex<GestureClassifier>> =
    LazyLock::new(|| Mutex::new(GestureClassifier::with_defaults()));

static MAIN_HOTKEY_PHYSICAL_CHANNEL: LazyLock<(
    async_channel::Sender<MainHotkeyPhysicalEvent>,
    async_channel::Receiver<MainHotkeyPhysicalEvent>,
)> = LazyLock::new(|| async_channel::bounded(32));

/// Channel carrying key-down/key-up pairs for the main launcher hotkey.
pub fn main_hotkey_physical_channel() -> &'static (
    async_channel::Sender<MainHotkeyPhysicalEvent>,
    async_channel::Receiver<MainHotkeyPhysicalEvent>,
) {
    &MAIN_HOTKEY_PHYSICAL_CHANNEL
}

/// Reset gesture classification when the main window is hidden (Esc dismiss, etc.).
pub fn reset_main_gesture_classifier() {
    if let Ok(mut classifier) = MAIN_GESTURE_CLASSIFIER.lock() {
        classifier.sync_window_hidden();
    }
}

/// Align gesture state when the main window is shown outside the hotkey path.
pub fn sync_main_gesture_window_shown() {
    if let Ok(mut classifier) = MAIN_GESTURE_CLASSIFIER.lock() {
        classifier.sync_window_shown();
    }
}

/// Advance time-based gesture transitions (`HoldStart`, deferred `Tap`).
pub fn poll_main_gesture_classifier(now: std::time::Instant) -> Option<GestureEvent> {
    MAIN_GESTURE_CLASSIFIER
        .lock()
        .ok()
        .and_then(|mut classifier| classifier.poll(now))
}

/// Next poll deadline for hold/double-tap windows.
pub fn main_gesture_next_deadline() -> Option<std::time::Instant> {
    MAIN_GESTURE_CLASSIFIER
        .lock()
        .ok()
        .and_then(|classifier| classifier.next_deadline())
}

pub fn process_main_hotkey_physical_event(event: MainHotkeyPhysicalEvent) -> Vec<GestureEvent> {
    let Ok(mut classifier) = MAIN_GESTURE_CLASSIFIER.lock() else {
        return Vec::new();
    };
    let input = match event.phase {
        MainHotkeyPhase::KeyDown => GestureInput::KeyDown(event.at),
        MainHotkeyPhase::KeyUp => GestureInput::KeyUp(event.at),
    };
    classifier.handle_input(input)
}

/// Agentic/devtools injection — mirrors the global-hotkey thread delivery path.
pub fn inject_main_hotkey_phase_for_agentic(phase: MainHotkeyPhase) {
    let correlation_id = format!("hotkey:main:inject:{}", Uuid::new_v4());
    let event = MainHotkeyPhysicalEvent {
        phase,
        correlation_id,
        at: std::time::Instant::now(),
    };
    if main_hotkey_physical_channel().0.try_send(event).is_err() {
        logging::log(
            "HOTKEY",
            "Main hotkey physical channel full/closed (inject)",
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptHotkeyEvent {
    pub command_id: String,
    pub correlation_id: String,
}
// --- merged from part_002.rs ---
/// Dispatch the Notes hotkey handler to the main thread.
///
/// Strategy (mutually exclusive to prevent double-fire):
/// - If a handler is registered: use it directly via GCD dispatch
/// - Otherwise: send to channel for async polling
///
/// This works even before the main window is activated because GCD dispatch
/// directly integrates with the NSApplication run loop that GPUI uses.
#[tracing::instrument(skip_all)]
fn dispatch_notes_hotkey(event: HotkeyEvent) {
    // Check if a direct handler is registered (takes priority over channel)
    let handler_storage = &*NOTES_HANDLER;
    let handler = clone_hotkey_handler_with_poison_recovery(handler_storage, "notes");

    if let Some(handler) = handler {
        // Handler is set - use direct GCD dispatch (skip channel to avoid double-fire)
        let correlation_id = event.correlation_id.clone();
        gcd::dispatch_to_main(move || {
            let _guard = logging::set_correlation_id(correlation_id);
            handler();
        });
    } else {
        // No handler - use channel approach for async polling
        if notes_hotkey_channel().0.try_send(event).is_err() {
            logging::log("HOTKEY", "Notes hotkey channel full/closed");
        }
        // Dispatch an empty closure to wake GPUI's event loop
        // This ensures the channel message gets processed even if GPUI was idle
        gcd::dispatch_to_main(|| {
            // Empty closure - just wakes the run loop
        });
    }
}
/// Dispatch the AI hotkey handler to the main thread.
///
/// Strategy (mutually exclusive to prevent double-fire):
/// - If a handler is registered: use it directly via GCD dispatch
/// - Otherwise: send to channel for async polling
#[tracing::instrument(skip_all)]
fn dispatch_ai_hotkey(event: HotkeyEvent) {
    // Check if a direct handler is registered (takes priority over channel)
    let handler_storage = &*AI_HANDLER;
    let handler = clone_hotkey_handler_with_poison_recovery(handler_storage, "ai");

    if let Some(handler) = handler {
        // Handler is set - use direct GCD dispatch (skip channel to avoid double-fire)
        let correlation_id = event.correlation_id.clone();
        gcd::dispatch_to_main(move || {
            let _guard = logging::set_correlation_id(correlation_id);
            handler();
        });
    } else {
        // No handler - use channel approach for async polling
        if ai_hotkey_channel().0.try_send(event).is_err() {
            logging::log("HOTKEY", "AI hotkey channel full/closed");
        }
        // Dispatch an empty closure to wake GPUI's event loop
        gcd::dispatch_to_main(|| {
            // Empty closure - just wakes the run loop
        });
    }
}
// HOTKEY_CHANNEL: Event-driven async_channel for hotkey events (replaces AtomicBool polling)
#[allow(dead_code)]
static HOTKEY_CHANNEL: LazyLock<(
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
)> = LazyLock::new(|| async_channel::bounded(10));
/// Get the hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn hotkey_channel() -> &'static (
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
) {
    &HOTKEY_CHANNEL
}
// SCRIPT_HOTKEY_CHANNEL: Channel for script shortcut events (sends script path)
#[allow(dead_code)]
static SCRIPT_HOTKEY_CHANNEL: LazyLock<(
    async_channel::Sender<ScriptHotkeyEvent>,
    async_channel::Receiver<ScriptHotkeyEvent>,
)> = LazyLock::new(|| async_channel::bounded(10));
/// Get the script hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn script_hotkey_channel() -> &'static (
    async_channel::Sender<ScriptHotkeyEvent>,
    async_channel::Receiver<ScriptHotkeyEvent>,
) {
    &SCRIPT_HOTKEY_CHANNEL
}
// NOTES_HOTKEY_CHANNEL: Channel for notes hotkey events
#[allow(dead_code)]
static NOTES_HOTKEY_CHANNEL: LazyLock<(
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
)> = LazyLock::new(|| async_channel::bounded(10));
/// Get the notes hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn notes_hotkey_channel() -> &'static (
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
) {
    &NOTES_HOTKEY_CHANNEL
}
// AI_HOTKEY_CHANNEL: Channel for AI hotkey events
#[allow(dead_code)]
static AI_HOTKEY_CHANNEL: LazyLock<(
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
)> = LazyLock::new(|| async_channel::bounded(10));
/// Get the AI hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn ai_hotkey_channel() -> &'static (
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
) {
    &AI_HOTKEY_CHANNEL
}
// INLINE_AI_HOTKEY_CHANNEL: Channel for the system-wide inline focused-text editor.
#[allow(dead_code)]
static INLINE_AI_HOTKEY_CHANNEL: LazyLock<(
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
)> = LazyLock::new(|| async_channel::bounded(10));
/// Get the inline AI text-edit hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn inline_ai_hotkey_channel() -> &'static (
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
) {
    &INLINE_AI_HOTKEY_CHANNEL
}
// REWRITE_HOTKEY_CHANNEL: Channel for the system-wide instant rewrite flow.
#[allow(dead_code)]
static REWRITE_HOTKEY_CHANNEL: LazyLock<(
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
)> = LazyLock::new(|| async_channel::bounded(10));
/// Get the rewrite hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn rewrite_hotkey_channel() -> &'static (
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
) {
    &REWRITE_HOTKEY_CHANNEL
}
// DICTATION_HOTKEY_CHANNEL: Channel for dictation toggle events
#[allow(dead_code)]
static DICTATION_HOTKEY_CHANNEL: LazyLock<(
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
)> = LazyLock::new(|| async_channel::bounded(10));

/// Push-to-talk hold threshold: when the dictation hotkey press started a
/// recording and the key is held at least this long, releasing it stops and
/// transcribes.  A quicker tap keeps classic toggle behavior (press again to
/// stop).
const DICTATION_PUSH_TO_TALK_MIN_HOLD: std::time::Duration = std::time::Duration::from_millis(500);

/// Instant of the dictation hotkey press that started the current recording.
/// `None` when the last press was a stop-edge (or no press happened yet), so
/// releases after a stop press never re-toggle.
static DICTATION_PTT_PRESSED_AT: parking_lot::Mutex<Option<std::time::Instant>> =
    parking_lot::Mutex::new(None);
/// Get the dictation hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn dictation_hotkey_channel() -> &'static (
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
) {
    &DICTATION_HOTKEY_CHANNEL
}
// LOGS_HOTKEY_CHANNEL: Channel for log capture toggle events
#[allow(dead_code)]
static LOGS_HOTKEY_CHANNEL: LazyLock<(
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
)> = LazyLock::new(|| async_channel::bounded(10));
/// Get the logs hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn logs_hotkey_channel() -> &'static (
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
) {
    &LOGS_HOTKEY_CHANNEL
}
// DEV_MARKER_HOTKEY_CHANNEL: Dev-only marker capture events.
#[allow(dead_code)]
static DEV_MARKER_HOTKEY_CHANNEL: LazyLock<(
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
)> = LazyLock::new(|| async_channel::bounded(10));
pub(crate) fn dev_marker_hotkey_channel() -> &'static (
    async_channel::Sender<HotkeyEvent>,
    async_channel::Receiver<HotkeyEvent>,
) {
    &DEV_MARKER_HOTKEY_CHANNEL
}
/// Tracks whether the main hotkey was successfully registered
/// Used by main.rs to detect if the app has an alternate entry point
static MAIN_HOTKEY_REGISTERED: AtomicBool = AtomicBool::new(false);
/// Check if the main hotkey was successfully registered
pub fn is_main_hotkey_registered() -> bool {
    MAIN_HOTKEY_REGISTERED.load(Ordering::Relaxed)
}
#[allow(dead_code)]
static HOTKEY_TRIGGER_COUNT: AtomicU64 = AtomicU64::new(0);
/// Format a hotkey registration error with helpful context
fn format_hotkey_error(e: &HotkeyError, shortcut_display: &str) -> String {
    match e {
        HotkeyError::AlreadyRegistered(hk) => {
            format!(
                "Hotkey '{}' is already registered by another application or script (ID: {}). \
                 Try a different shortcut or close the conflicting app.",
                shortcut_display,
                hk.id()
            )
        }
        HotkeyError::FailedToRegister(msg) => {
            format!(
                "System rejected hotkey '{}': {}. This shortcut may be reserved by macOS.",
                shortcut_display, msg
            )
        }
        HotkeyError::OsError(os_err) => {
            format!(
                "OS error registering '{}': {}. Check system hotkey settings.",
                shortcut_display, os_err
            )
        }
        other => format!(
            "Failed to register hotkey '{}': {}",
            shortcut_display, other
        ),
    }
}
/// Register a builtin hotkey (main/notes/ai) and add to unified routing table
fn register_builtin_hotkey(
    manager: &GlobalHotKeyManager,
    action: HotkeyAction,
    cfg: &config::HotkeyConfig,
) -> Option<u32> {
    let (mods, code) = parse_hotkey_config(cfg)?;
    let hotkey = HotKey::new(Some(mods), code);
    let id = hotkey.id();
    let display = hotkey_config_to_display(cfg);
    let action_label = hotkey_action_label(&action);

    logging::log(
        "KEY_SETUP",
        &format!(
            "BUILTIN_HOTKEY_REGISTER_ATTEMPT action={} display={} id={} mods={:?} code={:?}",
            action_label, display, id, mods, code
        ),
    );
    match manager.register(hotkey) {
        Ok(()) => {
            let mut routes_guard = routes().write();
            routes_guard.add_route(
                id,
                RegisteredHotkey {
                    hotkey,
                    action: action.clone(),
                    display: display.clone(),
                },
            );
            logging::log(
                "KEY_SETUP",
                &format!(
                    "BUILTIN_HOTKEY_REGISTERED action={} display={} id={}",
                    action_label, display, id
                ),
            );
            Some(id)
        }
        Err(e) => {
            logging::log(
                "KEY_SETUP",
                &format!(
                    "BUILTIN_HOTKEY_REGISTER_FAILED action={} display={} id={} error={}",
                    action_label, display, id, e
                ),
            );
            logging::log("HOTKEY", &format_hotkey_error(&e, &display));
            None
        }
    }
}
/// Register a script hotkey and add to unified routing table
fn register_script_hotkey_internal(
    manager: &GlobalHotKeyManager,
    path: &str,
    shortcut: &str,
    name: &str,
) -> Option<u32> {
    let (mods, code) = shortcuts::parse_shortcut(shortcut)?;
    let hotkey = HotKey::new(Some(mods), code);
    let id = hotkey.id();

    logging::log(
        "KEY_SETUP",
        &format!(
            "SCRIPT_HOTKEY_REGISTER_ATTEMPT name={} path={} shortcut={} id={} mods={:?} code={:?}",
            name, path, shortcut, id, mods, code
        ),
    );
    match manager.register(hotkey) {
        Ok(()) => {
            let mut routes_guard = routes().write();
            routes_guard.add_route(
                id,
                RegisteredHotkey {
                    hotkey,
                    action: HotkeyAction::Script(path.to_string()),
                    display: shortcut.to_string(),
                },
            );
            logging::log(
                "KEY_SETUP",
                &format!(
                    "SCRIPT_HOTKEY_REGISTERED name={} path={} shortcut={} id={}",
                    name, path, shortcut, id
                ),
            );
            Some(id)
        }
        Err(e) => {
            logging::log(
                "KEY_SETUP",
                &format!(
                    "SCRIPT_HOTKEY_REGISTER_FAILED name={} path={} shortcut={} id={} error={}",
                    name, path, shortcut, id, e
                ),
            );
            logging::log(
                "HOTKEY",
                &format!("{} (script: {})", format_hotkey_error(&e, shortcut), name),
            );
            None
        }
    }
}
// --- merged from part_003.rs ---
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

        if MAIN_MANAGER.set(Mutex::new(manager)).is_err() {
            logging::log("HOTKEY", "Manager already initialized (unexpected)");
            return;
        }

        let manager_guard = match MAIN_MANAGER.get() {
            Some(m) => match m.lock() {
                Ok(g) => g,
                Err(e) => {
                    logging::log("HOTKEY", &format!("Failed to lock manager: {}", e));
                    return;
                }
            },
            None => {
                tracing::error!("MAIN_MANAGER not initialized");
                return;
            }
        };

        // Register main hotkey using unified registration
        if register_builtin_hotkey(&manager_guard, HotkeyAction::Main, &config.hotkey).is_some() {
            MAIN_HOTKEY_REGISTERED.store(true, Ordering::Relaxed);
        }

        // Register notes hotkey (defaults to Cmd+Ctrl+N; disable via notesHotkeyEnabled)
        if let Some(notes_hotkey) = config.get_notes_hotkey() {
            register_builtin_hotkey(&manager_guard, HotkeyAction::Notes, &notes_hotkey);
        }
        // Register AI, dictation, and logs hotkeys
        if let Some(ai_hotkey) = config.get_ai_hotkey() {
            register_builtin_hotkey(&manager_guard, HotkeyAction::Ai, &ai_hotkey);
        }
        if let Some(dictation_hotkey) = config.get_dictation_hotkey() {
            register_builtin_hotkey(&manager_guard, HotkeyAction::Dictation, &dictation_hotkey);
        }
        if let Some(logs_hotkey) = config.get_logs_hotkey() {
            register_builtin_hotkey(&manager_guard, HotkeyAction::ToggleLogs, &logs_hotkey);
        }
        if std::env::var("SCRIPT_KIT_DEV_MARKER_HOTKEY")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            let dev_marker_hotkey = config::HotkeyConfig {
                modifiers: vec!["ctrl".to_string()],
                key: "KeyM".to_string(),
            };
            register_builtin_hotkey(&manager_guard, HotkeyAction::DevMarker, &dev_marker_hotkey);
        }
        if let Some(inline_ai_hotkey) = config.get_inline_ai_hotkey() {
            register_builtin_hotkey(
                &manager_guard,
                HotkeyAction::InlineAiTextEdit,
                &inline_ai_hotkey,
            );
        }
        if let Some(rewrite_hotkey) = config.get_rewrite_hotkey() {
            register_builtin_hotkey(&manager_guard, HotkeyAction::RewriteText, &rewrite_hotkey);
        }

        // Track which command IDs have been registered to avoid duplicates
        let mut registered_commands: HashSet<String> = HashSet::new();

        // Priority 1: Load command shortcuts from config.ts.
        let mut config_count = 0;
        if let Some(commands) = &config.commands {
            for (command_id, cmd_config) in commands {
                if let Some(hotkey_config) = &cmd_config.shortcut {
                    let shortcut_str = hotkey_config.to_shortcut_string();
                    if register_script_hotkey_internal(
                        &manager_guard,
                        command_id,
                        &shortcut_str,
                        command_id,
                    )
                    .is_some()
                    {
                        registered_commands.insert(command_id.clone());
                        config_count += 1;
                    }
                }
            }
            if config_count > 0 {
                logging::log(
                    "HOTKEY",
                    &format!(
                        "Registered {} command shortcuts from config.ts",
                        config_count
                    ),
                );
            }
        }

        if !registered_commands.contains(SCRIPT_KIT_SELFIE_COMMAND_ID)
            && register_script_hotkey_internal(
                &manager_guard,
                SCRIPT_KIT_SELFIE_COMMAND_ID,
                SCRIPT_KIT_SELFIE_SHORTCUT,
                "Script Kit Selfie",
            )
            .is_some()
        {
            registered_commands.insert(SCRIPT_KIT_SELFIE_COMMAND_ID.to_string());
        }

        // Priority 2: Register inline script/scriptlet metadata shortcuts only
        // when config.ts does not override the same launcher command.
        let mut script_count = 0;

        let all_scripts = scripts::read_scripts();
        for script in &all_scripts {
            if let Some(ref shortcut) = script.shortcut {
                let command_id = script.launcher_command_id();
                if registered_commands.contains(&command_id)
                    || registered_commands.contains(&script.source_command_id())
                {
                    continue;
                }
                let path = script.path.to_string_lossy().to_string();
                if register_script_hotkey_internal(&manager_guard, &path, shortcut, &script.name)
                    .is_some()
                {
                    script_count += 1;
                }
            }
        }

        let all_scriptlets = scripts::load_scriptlets();
        for scriptlet in &all_scriptlets {
            if let Some(ref shortcut) = scriptlet.shortcut {
                let command_id = scriptlet.launcher_command_id();
                if registered_commands.contains(&command_id)
                    || registered_commands.contains(&scriptlet.source_command_id())
                {
                    continue;
                }
                let path = scriptlet
                    .file_path
                    .clone()
                    .unwrap_or_else(|| scriptlet.name.clone());
                if register_script_hotkey_internal(&manager_guard, &path, shortcut, &scriptlet.name)
                    .is_some()
                {
                    script_count += 1;
                }
            }
        }

        logging::log(
            "HOTKEY",
            &format!("Registered {} script/scriptlet shortcuts", script_count),
        );

        // Log routing table summary. Route details are chunked across
        // continuation lines: a single line enumerating every route breaches
        // the 2048-byte log-hygiene budget once builtin + script hotkeys add
        // up (root-typing-lag-benchmark maxLogLineBytes gate).
        {
            let routes_guard = routes().read();
            let route_details = routes_guard
                .routes
                .iter()
                .map(|(id, entry)| {
                    format!(
                        "{}:{}=>{}",
                        id,
                        entry.display,
                        hotkey_action_label(&entry.action)
                    )
                })
                .collect::<Vec<_>>();
            let header = format!(
                "GLOBAL_HOTKEY_ROUTE_SUMMARY main={:?} notes={:?} ai={:?} logs={:?} dictation={:?} inline_ai={:?} rewrite={:?} scripts={} routes={}",
                routes_guard.main_id,
                routes_guard.notes_id,
                routes_guard.ai_id,
                routes_guard.logs_id,
                routes_guard.dictation_id,
                routes_guard.inline_ai_id,
                routes_guard.rewrite_id,
                routes_guard.script_paths.len(),
                route_details.len(),
            );
            for line in route_summary_lines(header, &route_details) {
                logging::log("KEY_SETUP", &line);
            }
        }

        drop(manager_guard);
        let receiver = GlobalHotKeyEvent::receiver();

        loop {
            if let Ok(event) = receiver.recv() {
                // Look up action in unified routing table (fast read lock)
                let action = {
                    let routes_guard = routes().read();
                    routes_guard.get_action(event.id)
                };

                match (action.as_ref(), event.state) {
                    (_, HotKeyState::Pressed) => {}
                    (Some(HotkeyAction::Main), HotKeyState::Released) => {}
                    // Dictation consumes releases for push-to-talk.
                    (Some(HotkeyAction::Dictation), HotKeyState::Released) => {}
                    _ => continue,
                }

                let action_label = action
                    .as_ref()
                    .map(hotkey_action_label)
                    .unwrap_or_else(|| "<unmapped>".to_string());
                logging::log(
                    "KEY_ROUTE",
                    &format!(
                        "GLOBAL_HOTKEY_EVENT id={} state={:?} action={}",
                        event.id, event.state, action_label
                    ),
                );

                match action {
                    Some(HotkeyAction::Main) => {
                        let correlation_id = format!("hotkey:main:{}", Uuid::new_v4());
                        let _guard = logging::set_correlation_id(correlation_id.clone());

                        let phase = match event.state {
                            HotKeyState::Pressed => MainHotkeyPhase::KeyDown,
                            HotKeyState::Released => MainHotkeyPhase::KeyUp,
                        };
                        let physical = MainHotkeyPhysicalEvent {
                            phase,
                            correlation_id: correlation_id.clone(),
                            at: std::time::Instant::now(),
                        };

                        if main_hotkey_physical_channel().0.try_send(physical).is_err() {
                            logging::log("HOTKEY", "Main hotkey physical channel full/closed");
                        }

                        if phase == MainHotkeyPhase::KeyDown {
                            let count = HOTKEY_TRIGGER_COUNT.fetch_add(1, Ordering::Relaxed);
                            logging::log(
                                "HOTKEY",
                                &format!("Main hotkey key-down (trigger #{})", count + 1),
                            );
                        } else {
                            logging::log("HOTKEY", "Main hotkey key-up");
                        }
                        gcd::dispatch_to_main(|| {});
                    }
                    Some(HotkeyAction::Notes) => {
                        // Set correlation ID for this hotkey event
                        let correlation_id = format!("hotkey:notes:{}", Uuid::new_v4());
                        let _guard = logging::set_correlation_id(correlation_id.clone());

                        logging::log(
                            "HOTKEY",
                            "Notes hotkey pressed - dispatching to main thread",
                        );
                        dispatch_notes_hotkey(HotkeyEvent { correlation_id });
                    }
                    Some(HotkeyAction::Ai) => {
                        // Set correlation ID for this hotkey event
                        let correlation_id = format!("hotkey:ai:{}", Uuid::new_v4());
                        let _guard = logging::set_correlation_id(correlation_id.clone());

                        logging::log("HOTKEY", "AI hotkey pressed - dispatching to main thread");
                        dispatch_ai_hotkey(HotkeyEvent { correlation_id });
                    }
                    Some(HotkeyAction::InlineAiTextEdit) => {
                        let correlation_id = format!("hotkey:inline-ai:{}", Uuid::new_v4());
                        let _guard = logging::set_correlation_id(correlation_id.clone());

                        logging::log(
                            "HOTKEY",
                            "Inline AI hotkey pressed - dispatching focused text capture",
                        );
                        if inline_ai_hotkey_channel()
                            .0
                            .try_send(HotkeyEvent { correlation_id })
                            .is_err()
                        {
                            logging::log("HOTKEY", "Inline AI hotkey channel full/closed");
                        }
                        gcd::dispatch_to_main(|| {});
                    }
                    Some(HotkeyAction::RewriteText) => {
                        let correlation_id = format!("hotkey:rewrite:{}", Uuid::new_v4());
                        let _guard = logging::set_correlation_id(correlation_id.clone());

                        logging::log(
                            "HOTKEY",
                            "Rewrite hotkey pressed - dispatching instant rewrite capture",
                        );
                        if rewrite_hotkey_channel()
                            .0
                            .try_send(HotkeyEvent { correlation_id })
                            .is_err()
                        {
                            logging::log("HOTKEY", "Rewrite hotkey channel full/closed");
                        }
                        gcd::dispatch_to_main(|| {});
                    }
                    Some(HotkeyAction::ToggleLogs) => {
                        // Set correlation ID for this hotkey event
                        let correlation_id = format!("hotkey:logs:{}", Uuid::new_v4());
                        let _guard = logging::set_correlation_id(correlation_id.clone());

                        logging::log("HOTKEY", "Logs hotkey pressed - toggling log capture");
                        // Toggle capture immediately (no need for main thread dispatch)
                        let (is_capturing, path) = logging::toggle_capture();
                        if is_capturing {
                            if let Some(p) = path {
                                logging::log(
                                    "HOTKEY",
                                    &format!("Log capture STARTED: {}", p.display()),
                                );
                            }
                        } else if let Some(p) = path {
                            logging::log(
                                "HOTKEY",
                                &format!("Log capture STOPPED: {}", p.display()),
                            );
                        }
                        // Send to channel for UI notification (HUD)
                        if logs_hotkey_channel()
                            .0
                            .try_send(HotkeyEvent { correlation_id })
                            .is_err()
                        {
                            logging::log("HOTKEY", "Logs hotkey channel full/closed");
                        }
                    }
                    Some(HotkeyAction::Dictation) => {
                        let correlation_id = format!("hotkey:dictation:{}", Uuid::new_v4());
                        let _guard = logging::set_correlation_id(correlation_id.clone());

                        match event.state {
                            HotKeyState::Pressed => {
                                // Arm push-to-talk only when this press starts
                                // a recording; a stop-edge press must not turn
                                // its own release into another toggle.
                                let starts_recording = !crate::dictation::is_dictation_busy();
                                *DICTATION_PTT_PRESSED_AT.lock() =
                                    starts_recording.then(std::time::Instant::now);

                                logging::log(
                                    "HOTKEY",
                                    "Dictation hotkey pressed - dispatching to main thread",
                                );
                                if dictation_hotkey_channel()
                                    .0
                                    .try_send(HotkeyEvent { correlation_id })
                                    .is_err()
                                {
                                    logging::log("HOTKEY", "Dictation hotkey channel full/closed");
                                }
                            }
                            HotKeyState::Released => {
                                let Some(pressed_at) = DICTATION_PTT_PRESSED_AT.lock().take()
                                else {
                                    continue;
                                };
                                if pressed_at.elapsed() < DICTATION_PUSH_TO_TALK_MIN_HOLD {
                                    // Quick tap: classic toggle — keep recording.
                                    continue;
                                }
                                if !crate::config::load_user_preferences()
                                    .dictation
                                    .push_to_talk_enabled()
                                {
                                    continue;
                                }
                                if !crate::dictation::is_dictation_recording() {
                                    // Recording never started (preflight took
                                    // over) or already stopped another way.
                                    continue;
                                }
                                logging::log(
                                    "HOTKEY",
                                    "Dictation hotkey released after hold - stopping (push-to-talk)",
                                );
                                if dictation_hotkey_channel()
                                    .0
                                    .try_send(HotkeyEvent { correlation_id })
                                    .is_err()
                                {
                                    logging::log("HOTKEY", "Dictation hotkey channel full/closed");
                                }
                            }
                        }
                    }
                    Some(HotkeyAction::DevMarker) => {
                        let correlation_id = format!("hotkey:dev-marker:{}", Uuid::new_v4());
                        let _guard = logging::set_correlation_id(correlation_id.clone());

                        logging::log(
                            "HOTKEY",
                            "Dev marker hotkey pressed - dispatching marker capture",
                        );
                        if dev_marker_hotkey_channel()
                            .0
                            .try_send(HotkeyEvent { correlation_id })
                            .is_err()
                        {
                            logging::log("HOTKEY", "Dev marker hotkey channel full/closed");
                        }
                        gcd::dispatch_to_main(|| {});
                    }
                    Some(HotkeyAction::Script(path)) => {
                        // Set correlation ID for this hotkey event
                        let correlation_id = format!("hotkey:script:{}:{}", path, Uuid::new_v4());
                        let _guard = logging::set_correlation_id(correlation_id.clone());

                        // Start benchmark timing for hotkey → chat latency analysis
                        logging::bench_start(&format!("hotkey:{}", path));
                        logging::log("HOTKEY", &format!("Script shortcut triggered: {}", path));
                        // NON-BLOCKING: Use try_send to prevent hotkey thread from blocking
                        if script_hotkey_channel()
                            .0
                            .try_send(ScriptHotkeyEvent {
                                command_id: path.clone(),
                                correlation_id,
                            })
                            .is_err()
                        {
                            logging::log(
                                "HOTKEY",
                                &format!("Script channel full/closed for {}", path),
                            );
                        }
                    }
                    None => {
                        // Set correlation ID even for unknown hotkey events
                        let _guard = logging::set_correlation_id(format!(
                            "hotkey:unknown:{}",
                            Uuid::new_v4()
                        ));

                        // Unknown hotkey ID - can happen during hot-reload transitions
                        logging::log("HOTKEY", &format!("Unknown hotkey event id={}", event.id));
                    }
                }
            }
        }
    });
}
// --- merged from part_004.rs ---
#[cfg(test)]
mod tests {
    use super::*;
    use async_channel::TryRecvError;

    // =============================================================================
    // Unified Routing Table Tests
    // =============================================================================
    mod routing_table_tests {
        use super::*;

        #[test]
        fn route_summary_lines_stay_under_log_hygiene_budget() {
            // Why: a single GLOBAL_HOTKEY_ROUTE_SUMMARY line enumerating every
            // route crossed 2048 bytes (2136 observed 2026-07-18) and failed
            // the typing-lag benchmark's maxLogLineBytes gate. Details must
            // chunk across lines while preserving every route entry.
            let details: Vec<String> = (0..200)
                .map(|i| format!("{i}:cmd+alt+shift+f{}=>script:/Users/example/.scriptkit/scripts/some-long-script-name-{i}.ts", i % 12))
                .collect();
            let lines = route_summary_lines(
                "GLOBAL_HOTKEY_ROUTE_SUMMARY scripts=200 routes=200".to_string(),
                &details,
            );
            assert!(lines.len() > 1, "long route sets must chunk");
            for line in &lines {
                assert!(
                    line.len() <= ROUTE_SUMMARY_LINE_BUDGET,
                    "line exceeds budget ({} bytes): {}",
                    line.len(),
                    &line[..80.min(line.len())]
                );
            }
            let joined = lines.join("\n");
            for detail in &details {
                assert!(
                    joined.contains(detail.as_str()),
                    "route entry dropped: {detail}"
                );
            }
        }

        #[test]
        fn test_hotkey_routes_new() {
            let routes = HotkeyRoutes::new();
            assert!(routes.routes.is_empty());
            assert!(routes.script_paths.is_empty());
            assert!(routes.main_id.is_none());
            assert!(routes.notes_id.is_none());
            assert!(routes.ai_id.is_none());
        }

        #[test]
        fn test_add_main_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META), Code::Semicolon);
            let entry = RegisteredHotkey {
                hotkey,
                action: HotkeyAction::Main,
                display: "cmd+;".to_string(),
            };
            routes.add_route(hotkey.id(), entry);

            assert_eq!(routes.main_id, Some(hotkey.id()));
            assert!(routes.routes.contains_key(&hotkey.id()));
            assert_eq!(routes.get_action(hotkey.id()), Some(HotkeyAction::Main));
        }

        #[test]
        fn test_add_script_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyT);
            let path = "/test/script.ts".to_string();
            let entry = RegisteredHotkey {
                hotkey,
                action: HotkeyAction::Script(path.clone()),
                display: "cmd+shift+t".to_string(),
            };
            routes.add_route(hotkey.id(), entry);

            assert_eq!(routes.script_paths.get(&path), Some(&hotkey.id()));
            assert_eq!(routes.get_script_id(&path), Some(hotkey.id()));
            assert_eq!(
                routes.get_action(hotkey.id()),
                Some(HotkeyAction::Script(path))
            );
        }

        #[test]
        fn test_remove_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META), Code::KeyN);
            let entry = RegisteredHotkey {
                hotkey,
                action: HotkeyAction::Notes,
                display: "cmd+n".to_string(),
            };
            routes.add_route(hotkey.id(), entry);
            assert!(routes.notes_id.is_some());

            let removed = routes.remove_route(hotkey.id());
            assert!(removed.is_some());
            assert!(routes.notes_id.is_none());
            assert!(routes.get_action(hotkey.id()).is_none());
        }

        #[test]
        fn test_remove_script_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META), Code::KeyS);
            let path = "/test/script.ts".to_string();
            let entry = RegisteredHotkey {
                hotkey,
                action: HotkeyAction::Script(path.clone()),
                display: "cmd+s".to_string(),
            };
            routes.add_route(hotkey.id(), entry);
            assert!(routes.script_paths.contains_key(&path));

            routes.remove_route(hotkey.id());
            assert!(!routes.script_paths.contains_key(&path));
        }

        #[test]
        fn shortcut_conflict_reports_existing_live_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyK);
            routes.add_route(
                hotkey.id(),
                RegisteredHotkey {
                    hotkey,
                    action: HotkeyAction::Script("builtin/existing-command".to_string()),
                    display: "cmd+shift+k".to_string(),
                },
            );

            let conflict =
                shortcut_conflict_in_routes(&routes, "builtin/new-command", "cmd+shift+k")
                    .expect("expected conflict with existing live route");

            assert_eq!(conflict.command_id, "builtin/existing-command");
            assert_eq!(conflict.command_name, "builtin/existing-command");
            assert_eq!(conflict.shortcut, "cmd+shift+k");
        }

        #[test]
        fn shortcut_conflict_ignores_current_command_rebind() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyK);
            routes.add_route(
                hotkey.id(),
                RegisteredHotkey {
                    hotkey,
                    action: HotkeyAction::Script("builtin/current-command".to_string()),
                    display: "cmd+shift+k".to_string(),
                },
            );

            assert!(
                shortcut_conflict_in_routes(&routes, "builtin/current-command", "cmd+shift+k",)
                    .is_none(),
                "updating the selected command's current shortcut should not self-conflict"
            );
        }

        #[test]
        fn test_hotkey_action_equality() {
            assert_eq!(HotkeyAction::Main, HotkeyAction::Main);
            assert_eq!(HotkeyAction::Notes, HotkeyAction::Notes);
            assert_eq!(HotkeyAction::Ai, HotkeyAction::Ai);
            assert_eq!(HotkeyAction::ToggleLogs, HotkeyAction::ToggleLogs);
            assert_eq!(
                HotkeyAction::Script("/a.ts".to_string()),
                HotkeyAction::Script("/a.ts".to_string())
            );
            assert_ne!(HotkeyAction::Main, HotkeyAction::Notes);
            assert_ne!(HotkeyAction::Main, HotkeyAction::ToggleLogs);
            assert_ne!(
                HotkeyAction::Script("/a.ts".to_string()),
                HotkeyAction::Script("/b.ts".to_string())
            );
        }
    }

    #[test]
    fn hotkey_channels_are_independent() {
        while hotkey_channel().1.try_recv().is_ok() {}
        while script_hotkey_channel().1.try_recv().is_ok() {}

        hotkey_channel()
            .0
            .send_blocking(HotkeyEvent {
                correlation_id: "cid-main".to_string(),
            })
            .expect("send hotkey");
        assert!(matches!(
            script_hotkey_channel().1.try_recv(),
            Err(TryRecvError::Empty)
        ));
        let hotkey_event = hotkey_channel().1.try_recv().expect("recv hotkey");
        assert_eq!(hotkey_event.correlation_id, "cid-main");

        script_hotkey_channel()
            .0
            .send_blocking(ScriptHotkeyEvent {
                command_id: "script".to_string(),
                correlation_id: "cid-script".to_string(),
            })
            .expect("send script hotkey");
        let script_event = script_hotkey_channel()
            .1
            .try_recv()
            .expect("recv script hotkey");
        assert_eq!(script_event.command_id, "script");
        assert_eq!(script_event.correlation_id, "cid-script");
    }

    #[test]
    fn test_hotkey_handler_mutex_poison_recovery() {
        let storage = std::sync::Arc::new(std::sync::Mutex::new(Some(
            Arc::new(|| {}) as HotkeyHandler
        )));
        let poison_storage = std::sync::Arc::clone(&storage);

        let _ = std::thread::spawn(move || {
            let _guard = poison_storage.lock().unwrap();
            panic!("poison handler lock");
        })
        .join();

        assert!(
            storage.is_poisoned(),
            "mutex should be poisoned for this test"
        );

        let recovered = clone_hotkey_handler_with_poison_recovery(
            storage.as_ref(),
            "test_hotkey_handler_mutex_poison_recovery",
        );
        assert!(
            recovered.is_some(),
            "poison recovery should still return the existing handler"
        );

        // Verify the mutex remains usable after recovery.
        *storage.lock().unwrap_or_else(|e| e.into_inner()) = None;
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
