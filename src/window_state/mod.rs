//! Window State Persistence
//!
//! This module handles saving and restoring window positions for the main launcher,
//! Notes window, and AI window. Positions are stored in `~/.sk/kit/window-state.json`.
//!
//! # Architecture (Following Expert Review Recommendations)
//!
//! 1. **Canonical coordinate space**: "Global top-left origin (CoreGraphics-style), y increases downward"
//! 2. **Persistence via WindowBounds**: Aligns with GPUI's `WindowBounds` (Windowed/Maximized/Fullscreen)
//! 3. **Restore via WindowOptions.window_bounds**: No "jump after open"
//! 4. **Validation via geometry intersection**: Not display IDs (which can change)
//! 5. **Save on close/hide**: Main window saves on hide (since it's often hidden not closed)

// --- merged from part_000.rs ---
use crate::logging;
use crate::windows::DisplayBounds;
use gpui::{point, px, Bounds, Pixels, WindowBounds};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
// ============================================================================
// Save Suppression (for reset operations)
// ============================================================================

/// Flag to temporarily suppress position saving after a reset.
/// This prevents the bounds change callback from immediately re-saving
/// the position after reset_all_positions() deletes the state file.
static SUPPRESS_SAVE: AtomicBool = AtomicBool::new(false);
/// Suppress position saving temporarily.
/// Call this before reset_all_positions() to prevent immediate re-save.
pub fn suppress_save() {
    SUPPRESS_SAVE.store(true, Ordering::SeqCst);
    logging::log("WINDOW_STATE", "Position saving suppressed");
}
/// Allow position saving again.
/// Call this when the window is shown again after a reset.
pub fn allow_save() {
    SUPPRESS_SAVE.store(false, Ordering::SeqCst);
    logging::log("WINDOW_STATE", "Position saving allowed");
}
/// Check if position saving is currently suppressed.
pub fn is_save_suppressed() -> bool {
    SUPPRESS_SAVE.load(Ordering::SeqCst)
}
// ============================================================================
// Types
// ============================================================================

/// Identifies which window we're tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum WindowRole {
    Main,
    Notes,
    Ai,
    AiMini,
    AgentChat,
}
impl WindowRole {
    /// Get a lowercase string key for persistence/file paths
    pub fn as_str(&self) -> &'static str {
        match self {
            WindowRole::Main => "main",
            WindowRole::Notes => "notes",
            WindowRole::Ai => "ai",
            WindowRole::AiMini => "ai_mini",
            WindowRole::AgentChat => "agent_chat",
        }
    }

    /// Get a human-readable name for logging
    pub fn name(&self) -> &'static str {
        match self {
            WindowRole::Main => "Main",
            WindowRole::Notes => "Notes",
            WindowRole::Ai => "AI",
            WindowRole::AiMini => "AI Mini",
            WindowRole::AgentChat => "Agent Chat",
        }
    }
}
/// Window mode (matches GPUI WindowBounds variants)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PersistedWindowMode {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}
/// Persisted bounds for a single window.
/// Uses canonical "top-left origin" coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PersistedWindowBounds {
    pub mode: PersistedWindowMode,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Default for PersistedWindowBounds {
    fn default() -> Self {
        Self {
            mode: PersistedWindowMode::Windowed,
            x: 0.0,
            y: 0.0,
            width: 750.0,
            height: 475.0,
        }
    }
}
impl PersistedWindowBounds {
    /// Convert to GPUI WindowBounds
    #[allow(clippy::wrong_self_convention)]
    pub fn to_gpui(&self) -> WindowBounds {
        let bounds = Bounds {
            origin: point(px(self.x as f32), px(self.y as f32)),
            size: gpui::size(px(self.width as f32), px(self.height as f32)),
        };
        match self.mode {
            PersistedWindowMode::Windowed => WindowBounds::Windowed(bounds),
            PersistedWindowMode::Maximized => WindowBounds::Maximized(bounds),
            PersistedWindowMode::Fullscreen => WindowBounds::Fullscreen(bounds),
        }
    }

    /// Create from GPUI WindowBounds
    pub fn from_gpui(wb: WindowBounds) -> Self {
        let (mode, b): (PersistedWindowMode, Bounds<Pixels>) = match wb {
            WindowBounds::Windowed(b) => (PersistedWindowMode::Windowed, b),
            WindowBounds::Maximized(b) => (PersistedWindowMode::Maximized, b),
            WindowBounds::Fullscreen(b) => (PersistedWindowMode::Fullscreen, b),
        };
        Self {
            mode,
            x: f64::from(b.origin.x),
            y: f64::from(b.origin.y),
            width: f64::from(b.size.width),
            height: f64::from(b.size.height),
        }
    }

    /// Create from raw coordinates (already in top-left canonical space)
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            mode: PersistedWindowMode::Windowed,
            x,
            y,
            width,
            height,
        }
    }
}
/// The full persisted state file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowStateFile {
    #[serde(default = "default_version")]
    pub version: u32,
    /// Legacy main window position (for backwards compatibility)
    pub main: Option<PersistedWindowBounds>,
    /// Main window positions stored per display (keyed by display dimensions + origin)
    #[serde(default)]
    pub main_per_display: HashMap<String, PersistedWindowBounds>,
    /// Legacy notes window position (for backwards compatibility)
    pub notes: Option<PersistedWindowBounds>,
    /// Notes window positions stored per display
    #[serde(default)]
    pub notes_per_display: HashMap<String, PersistedWindowBounds>,
    /// Legacy AI window position (for backwards compatibility)
    pub ai: Option<PersistedWindowBounds>,
    /// AI window positions stored per display
    #[serde(default)]
    pub ai_per_display: HashMap<String, PersistedWindowBounds>,
    /// Mini AI window position (separate from full AI)
    #[serde(default)]
    pub ai_mini: Option<PersistedWindowBounds>,
    /// Agent Chat chat detached window position
    #[serde(default)]
    pub agent_chat: Option<PersistedWindowBounds>,
}
fn default_version() -> u32 {
    3 // Version 3 adds per-display support for AI and Notes windows
}
// ============================================================================
// File Path
// ============================================================================

/// Get the path to the window state file: ~/.sk/kit/window-state.json
pub fn get_state_file_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".sk").join("kit").join("window-state.json")
}
// ============================================================================
// Load / Save
// ============================================================================

/// Load the entire window state file
pub fn load_state_file() -> Option<WindowStateFile> {
    let path = get_state_file_path();
    if !path.exists() {
        return None;
    }
    match fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str(&contents) {
            Ok(state) => Some(state),
            Err(e) => {
                logging::log(
                    "WINDOW_STATE",
                    &format!("Failed to parse window-state.json: {}", e),
                );
                None
            }
        },
        Err(e) => {
            logging::log(
                "WINDOW_STATE",
                &format!("Failed to read window-state.json: {}", e),
            );
            None
        }
    }
}
/// Save the entire window state file (atomic write)
pub fn save_state_file(state: &WindowStateFile) -> bool {
    let path = get_state_file_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            logging::log(
                "WINDOW_STATE",
                &format!("Failed to create directory: {}", e),
            );
            return false;
        }
    }
    let json = match serde_json::to_string_pretty(state) {
        Ok(j) => j,
        Err(e) => {
            logging::log("WINDOW_STATE", &format!("Failed to serialize: {}", e));
            return false;
        }
    };
    // Atomic write via a UNIQUE temp file + rename. Window saves are not
    // serialized (multiple windows / instances can save concurrently), and a
    // fixed temp path let those interleave into a corrupt file.
    if let Err(e) = crate::atomic_file::write_atomic(&path, json.as_bytes()) {
        logging::log(
            "WINDOW_STATE",
            &format!("Failed to atomically write window state: {}", e),
        );
        return false;
    }
    logging::log("WINDOW_STATE", "Window state saved successfully");
    true
}
/// Load bounds for a specific window role
pub fn load_window_bounds(role: WindowRole) -> Option<PersistedWindowBounds> {
    let state = load_state_file()?;
    match role {
        WindowRole::Main => state.main,
        WindowRole::Notes => state.notes,
        WindowRole::Ai => state.ai,
        WindowRole::AiMini => state.ai_mini,
        WindowRole::AgentChat => state.agent_chat,
    }
}
/// Save bounds for a specific window role.
/// Respects the save suppression flag for the Main window role.
pub fn save_window_bounds(role: WindowRole, bounds: PersistedWindowBounds) {
    // Skip saving Main window position if suppressed (e.g., after reset)
    if role == WindowRole::Main && is_save_suppressed() {
        logging::log(
            "WINDOW_STATE",
            "Skipping save_window_bounds(Main) - position saving is suppressed",
        );
        return;
    }

    let mut state = load_state_file().unwrap_or_default();
    state.version = 3;
    match role {
        WindowRole::Main => state.main = Some(bounds),
        WindowRole::Notes => state.notes = Some(bounds),
        WindowRole::Ai => state.ai = Some(bounds),
        WindowRole::AiMini => state.ai_mini = Some(bounds),
        WindowRole::AgentChat => state.agent_chat = Some(bounds),
    }
    save_state_file(&state);
    logging::log(
        "WINDOW_STATE",
        &format!(
            "Saved {} bounds: ({:.0}, {:.0}) {}x{}",
            role.as_str(),
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height
        ),
    );
}
/// Reset all window positions (delete the state file)
pub fn reset_all_positions() {
    let path = get_state_file_path();
    if path.exists() {
        if let Err(e) = fs::remove_file(&path) {
            logging::log("WINDOW_STATE", &format!("Failed to delete: {}", e));
        } else {
            logging::log("WINDOW_STATE", "All window positions reset to defaults");
        }
    }
}
/// Check if any window positions have been customized
pub fn has_custom_positions() -> bool {
    load_state_file().is_some_and(|s| {
        s.main.is_some() || !s.main_per_display.is_empty() || s.notes.is_some() || s.ai.is_some()
    })
}
// ============================================================================
// Per-Display Position Storage (Main Window)
// ============================================================================

/// Generate a stable key for a display based on its dimensions AND origin.
pub fn display_key(display: &DisplayBounds) -> String {
    format!(
        "{}x{}@{},{}",
        display.width as u32,
        display.height as u32,
        display.origin_x as i32,
        display.origin_y as i32
    )
}
/// Find which display contains the given point (typically mouse cursor).
pub fn find_display_containing_point(
    x: f64,
    y: f64,
    displays: &[DisplayBounds],
) -> Option<&DisplayBounds> {
    for display in displays {
        let in_x = x >= display.origin_x && x < display.origin_x + display.width;
        let in_y = y >= display.origin_y && y < display.origin_y + display.height;
        if in_x && in_y {
            return Some(display);
        }
    }
    None
}
/// Find which display contains the center of the given bounds.
pub fn find_display_for_bounds<'a>(
    bounds: &PersistedWindowBounds,
    displays: &'a [DisplayBounds],
) -> Option<&'a DisplayBounds> {
    find_best_display_for_bounds(bounds, displays)
}
/// Save main window position for a specific display.
/// Respects the save suppression flag (set during reset_all_positions).
pub fn save_main_position_for_display(display: &DisplayBounds, bounds: PersistedWindowBounds) {
    // Skip saving if suppressed (e.g., right after reset_all_positions)
    if is_save_suppressed() {
        logging::log(
            "WINDOW_STATE",
            "Skipping save - position saving is suppressed (reset in progress)",
        );
        return;
    }

    let key = display_key(display);
    let mut state = load_state_file().unwrap_or_default();
    state.version = 3;
    state.main_per_display.insert(key.clone(), bounds);
    state.main = Some(bounds);
    save_state_file(&state);
    logging::log(
        "WINDOW_STATE",
        &format!(
            "Saved main position for display {}: ({:.0}, {:.0}) {}x{}",
            key, bounds.x, bounds.y, bounds.width, bounds.height
        ),
    );
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainPositionSaveOutcome {
    SavedPerDisplay,
    SavedLegacyOnly,
    Suppressed,
}
/// Save main window bounds using per-display persistence when possible.
///
/// Falls back to legacy `main` bounds when no matching display can be found.
#[must_use]
pub fn save_main_position_with_display_detection(
    bounds: PersistedWindowBounds,
    displays: &[DisplayBounds],
) -> MainPositionSaveOutcome {
    if is_save_suppressed() {
        logging::log(
            "WINDOW_STATE",
            "Skipping main save - position saving is suppressed",
        );
        return MainPositionSaveOutcome::Suppressed;
    }

    if let Some(display) = find_display_for_bounds(&bounds, displays) {
        save_main_position_for_display(display, bounds);
        MainPositionSaveOutcome::SavedPerDisplay
    } else {
        // Keep legacy persistence as a fallback when display detection fails.
        save_window_bounds(WindowRole::Main, bounds);
        logging::log(
            "WINDOW_STATE",
            "Could not determine display for main window bounds; saved legacy main bounds only",
        );
        MainPositionSaveOutcome::SavedLegacyOnly
    }
}
#[cfg(test)]
pub fn get_main_position_for_display(display: &DisplayBounds) -> Option<PersistedWindowBounds> {
    let state = load_state_file()?;
    let key = display_key(display);
    state.main_per_display.get(&key).copied()
}
/// Get the best main window position for the mouse display.
pub fn get_main_position_for_mouse_display(
    mouse_x: f64,
    mouse_y: f64,
    displays: &[DisplayBounds],
) -> Option<(PersistedWindowBounds, DisplayBounds)> {
    let display = find_display_containing_point(mouse_x, mouse_y, displays)?;
    let key = display_key(display);
    let state = load_state_file()?;

    if let Some(saved) = state.main_per_display.get(&key) {
        logging::log(
            "WINDOW_STATE",
            &format!("Restoring per-display position for {}", key),
        );
        return Some((*saved, display.clone()));
    }

    if let Some(legacy) = state.main {
        if let Some(legacy_display) = find_best_display_for_bounds(&legacy, displays) {
            if display_key(legacy_display) == key {
                return Some((legacy, display.clone()));
            }
        }
    }

    logging::log(
        "WINDOW_STATE",
        &format!("No saved position for display {}", key),
    );
    None
}
// ============================================================================
// ============================================================================
// Visibility Validation
// ============================================================================

const MIN_VISIBLE_AREA: f64 = 64.0 * 64.0;
const MIN_EDGE_MARGIN: f64 = 50.0;
/// Check if saved bounds are still visible on current displays.
pub fn is_bounds_visible(bounds: &PersistedWindowBounds, displays: &[DisplayBounds]) -> bool {
    if displays.is_empty() {
        return false;
    }
    for display in displays {
        if let Some((_, _, w, h)) = rect_intersection(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            display.origin_x,
            display.origin_y,
            display.width,
            display.height,
        ) {
            if w * h >= MIN_VISIBLE_AREA {
                return true;
            }
        }
    }
    false
}
#[allow(clippy::too_many_arguments)]
fn rect_intersection(
    x1: f64,
    y1: f64,
    w1: f64,
    h1: f64,
    x2: f64,
    y2: f64,
    w2: f64,
    h2: f64,
) -> Option<(f64, f64, f64, f64)> {
    let left = x1.max(x2);
    let top = y1.max(y2);
    let right = (x1 + w1).min(x2 + w2);
    let bottom = (y1 + h1).min(y2 + h2);
    if left < right && top < bottom {
        Some((left, top, right - left, bottom - top))
    } else {
        None
    }
}
pub fn clamp_bounds_to_displays(
    bounds: &PersistedWindowBounds,
    displays: &[DisplayBounds],
) -> Option<PersistedWindowBounds> {
    if displays.is_empty() {
        return None;
    }
    let target = find_best_display_for_bounds(bounds, displays)?;
    let mut clamped = *bounds;
    clamped.width = clamped.width.min(target.width - MIN_EDGE_MARGIN * 2.0);
    clamped.height = clamped.height.min(target.height - MIN_EDGE_MARGIN * 2.0);
    let min_x = target.origin_x + MIN_EDGE_MARGIN;
    let max_x = target.origin_x + target.width - clamped.width - MIN_EDGE_MARGIN;
    clamped.x = clamped.x.max(min_x).min(max_x);
    let min_y = target.origin_y + MIN_EDGE_MARGIN;
    let max_y = target.origin_y + target.height - clamped.height - MIN_EDGE_MARGIN;
    clamped.y = clamped.y.max(min_y).min(max_y);
    Some(clamped)
}
fn find_best_display_for_bounds<'a>(
    bounds: &PersistedWindowBounds,
    displays: &'a [DisplayBounds],
) -> Option<&'a DisplayBounds> {
    let cx = bounds.x + bounds.width / 2.0;
    let cy = bounds.y + bounds.height / 2.0;
    for d in displays {
        if cx >= d.origin_x
            && cx < d.origin_x + d.width
            && cy >= d.origin_y
            && cy < d.origin_y + d.height
        {
            return Some(d);
        }
    }
    let mut best: Option<&DisplayBounds> = None;
    let mut best_area = 0.0;
    for d in displays {
        if let Some((_, _, w, h)) = rect_intersection(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            d.origin_x,
            d.origin_y,
            d.width,
            d.height,
        ) {
            if w * h > best_area {
                best_area = w * h;
                best = Some(d);
            }
        }
    }
    best.or_else(|| displays.first())
}
// ============================================================================
// High-Level API
// ============================================================================

pub fn get_initial_bounds(
    role: WindowRole,
    default_bounds: Bounds<Pixels>,
    displays: &[DisplayBounds],
) -> Bounds<Pixels> {
    if let Some(saved) = load_window_bounds(role) {
        if is_bounds_visible(&saved, displays) {
            logging::log(
                "WINDOW_STATE",
                &format!(
                    "Restoring {} to ({:.0}, {:.0})",
                    role.as_str(),
                    saved.x,
                    saved.y
                ),
            );
            return saved.to_gpui().get_bounds();
        }
        if let Some(clamped) = clamp_bounds_to_displays(&saved, displays) {
            logging::log(
                "WINDOW_STATE",
                &format!(
                    "Clamped {} to ({:.0}, {:.0})",
                    role.as_str(),
                    clamped.x,
                    clamped.y
                ),
            );
            return clamped.to_gpui().get_bounds();
        }
        logging::log(
            "WINDOW_STATE",
            &format!("{} saved position no longer visible", role.as_str()),
        );
    }
    default_bounds
}
pub fn save_window_from_gpui(role: WindowRole, window_bounds: WindowBounds) {
    save_window_bounds(role, PersistedWindowBounds::from_gpui(window_bounds));
}
// Tests are in src/window_state_persistence_tests.rs

// Audit/decision-lock tests live in tests.rs → tests/{window_state,persistence}.rs.
// This declaration is load-bearing: without it the whole audit suite silently
// never compiles (discovered 2026-06-12 — the file had been orphaned).
#[cfg(test)]
mod tests;
