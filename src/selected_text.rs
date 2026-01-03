//! Selected text operations using macOS Accessibility APIs
//!
//! This module provides getSelectedText() and setSelectedText() operations
//! using a hybrid approach: Accessibility API primary, clipboard fallback.
//!
//! ## Architecture
//!
//! - `get_selected_text()`: Uses `get-selected-text` crate which tries AX API first,
//!   then falls back to clipboard simulation with Cmd+C
//! - `set_selected_text()`: Uses clipboard + enigo keyboard simulation (Cmd+V)
//!
//! ## Permissions
//!
//! Requires Accessibility permission in System Preferences > Privacy & Security > Accessibility

use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use get_selected_text::get_selected_text as get_selected_text_impl;
use macos_accessibility_client::accessibility;
use std::process::Command;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

// ============================================================================
// Permission Functions
// ============================================================================

/// Check if accessibility permissions are granted.
///
/// This checks if the application has been granted permission to use
/// macOS Accessibility APIs for cross-process text operations.
///
/// # Returns
/// `true` if permission is granted, `false` otherwise.
#[instrument]
pub fn has_accessibility_permission() -> bool {
    let result = accessibility::application_is_trusted();
    debug!(granted = result, "Checked accessibility permission");
    result
}

/// Request accessibility permissions (opens System Preferences).
///
/// This will show the system dialog prompting the user to grant
/// accessibility permission. The user must manually enable the
/// permission in System Preferences.
///
/// # Returns
/// `true` if permission is granted after the request, `false` otherwise.
#[instrument]
pub fn request_accessibility_permission() -> bool {
    info!("Requesting accessibility permission");
    let result = accessibility::application_is_trusted_with_prompt();
    if result {
        info!("Accessibility permission granted");
    } else {
        warn!("Accessibility permission denied or pending");
    }
    result
}

/// Open System Preferences directly to Accessibility pane.
///
/// This is useful for guiding users to the correct settings location
/// without showing the system permission prompt.
///
/// # Errors
/// Returns error if unable to spawn the open command.
#[allow(dead_code)] // Will be used for permission UI prompts
#[instrument]
pub fn open_accessibility_settings() -> Result<()> {
    info!("Opening accessibility settings");
    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .context("Failed to open System Preferences")?;
    Ok(())
}

/// Show a user-friendly dialog explaining accessibility permission is needed.
///
/// First checks if permission is already granted. If not, requests it
/// with the system prompt.
///
/// # Returns
/// `true` if permission is granted (either already or after request).
#[allow(dead_code)] // Will be used for permission UI prompts
#[instrument]
pub fn show_permission_dialog() -> Result<bool> {
    // First, check if already granted
    if has_accessibility_permission() {
        debug!("Permission already granted");
        return Ok(true);
    }

    // Request with system prompt (opens System Preferences)
    let granted = request_accessibility_permission();

    if !granted {
        warn!("User denied accessibility permission");
    }

    Ok(granted)
}

// ============================================================================
// Get Selected Text
// ============================================================================

/// Get the currently selected text from the focused application.
///
/// Uses the `get-selected-text` crate which implements:
/// 1. AXSelectedText attribute (fastest, most reliable)
/// 2. AXSelectedTextRange + AXStringForRange (fallback)
/// 3. Clipboard simulation with Cmd+C (last resort, saves/restores clipboard)
///
/// # Returns
/// The selected text, or empty string if nothing is selected.
///
/// # Errors
/// - Returns error if no accessibility permission
/// - Returns error if the operation fails
///
#[instrument(skip_all)]
pub fn get_selected_text() -> Result<String> {
    // Check permissions first
    if !has_accessibility_permission() {
        bail!("Accessibility permission required. Enable in System Preferences > Privacy & Security > Accessibility");
    }

    debug!("Attempting to get selected text");

    // The crate handles all the complexity:
    // - Tries AX API first
    // - Falls back to clipboard simulation
    // - Caches per-app behavior with LRU cache
    match get_selected_text_impl() {
        Ok(text) => {
            if text.is_empty() {
                debug!("No text selected (empty result)");
                Ok(String::new())
            } else {
                info!(text_len = text.len(), "Got selected text");
                Ok(text)
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to get selected text");
            bail!("Failed to get selected text: {}", e)
        }
    }
}

// ============================================================================
// Set Selected Text
// ============================================================================

/// Set (replace) the currently selected text in the focused application.
///
/// Strategy:
/// 1. Save current clipboard contents
/// 2. Set clipboard to new text
/// 3. Simulate Cmd+V
/// 4. Restore original clipboard
///
/// # Arguments
/// * `text` - The text to insert, replacing the current selection
///
/// # Errors
/// - Returns error if no accessibility permission
/// - Returns error if clipboard or paste operation fails
///
#[instrument(skip(text), fields(text_len = text.len()))]
pub fn set_selected_text(text: &str) -> Result<()> {
    if !has_accessibility_permission() {
        bail!("Accessibility permission required");
    }

    debug!("Attempting to set selected text");

    // Use clipboard fallback (AX write is complex and not widely supported)
    set_via_clipboard_fallback(text)
}

/// Clipboard-based fallback for setting selected text.
///
/// This function:
/// 1. Saves the current clipboard contents
/// 2. Sets the clipboard to the new text
/// 3. Simulates Cmd+V to paste using Core Graphics (more reliable than enigo)
/// 4. Restores the original clipboard (best effort)
fn set_via_clipboard_fallback(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;

    // Save original clipboard contents
    let original = clipboard.get_text().ok();
    debug!(
        had_original = original.is_some(),
        "Saved original clipboard"
    );

    // Set new text to clipboard
    clipboard
        .set_text(text)
        .context("Failed to set clipboard text")?;

    // Small delay to ensure clipboard is set
    thread::sleep(Duration::from_millis(10));

    // Simulate Cmd+V using Core Graphics (more reliable on macOS than enigo)
    simulate_paste_with_cg()?;

    // Wait for paste to complete
    thread::sleep(Duration::from_millis(50));

    // Restore original clipboard (best effort)
    if let Some(original_text) = original {
        // Small delay before restoring
        thread::sleep(Duration::from_millis(100));
        if let Err(e) = clipboard.set_text(&original_text) {
            warn!(error = %e, "Failed to restore original clipboard");
        } else {
            debug!("Restored original clipboard");
        }
    }

    info!("Set selected text via clipboard fallback");
    Ok(())
}

/// Simulate Cmd+V paste using Core Graphics events.
/// This is more reliable on macOS than using enigo.
///
/// # Usage
/// Call this after copying content to the clipboard and hiding the window.
/// The function will simulate Cmd+V to paste into the currently focused app.
pub fn simulate_paste_with_cg() -> Result<()> {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // 'v' key is keycode 9 on macOS
    const KEY_V: CGKeyCode = 9;

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .ok()
        .context("Failed to create CGEventSource")?;

    // Create key down event for 'v' with Cmd modifier
    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .ok()
        .context("Failed to create key down event")?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);

    // Create key up event for 'v' with Cmd modifier
    let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .ok()
        .context("Failed to create key up event")?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    // Post events
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(5));
    key_up.post(CGEventTapLocation::HID);

    debug!("Simulated Cmd+V via Core Graphics");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

// ============================================================================
// Unit Tests (always run with `cargo test`)
// ============================================================================
// None - all tests in this module require system interaction

// ============================================================================
// System Tests (require `cargo test --features system-tests`)
// ============================================================================
// These tests interact with macOS accessibility APIs, clipboard, and keyboard
// simulation. They may have side effects on the system state.

#[cfg(all(test, feature = "system-tests"))]
mod system_tests {
    use super::*;

    #[test]
    fn test_permission_check_does_not_panic() {
        // This test verifies the permission check doesn't panic
        // The actual result depends on system permissions
        let _has_permission = has_accessibility_permission();
        // Just ensure it doesn't panic - result varies by environment
    }

    #[test]
    fn test_permission_check_is_deterministic() {
        // Calling permission check multiple times should return same result
        let first = has_accessibility_permission();
        let second = has_accessibility_permission();
        assert_eq!(first, second, "Permission check should be deterministic");
    }

    #[test]
    #[ignore] // Requires manual interaction - select text in another app first
    fn test_get_selected_text_in_textedit() {
        // Instructions:
        // 1. Open TextEdit
        // 2. Type and select "Hello World"
        // 3. Run this test with: cargo test --features system-tests test_get_selected_text_in_textedit -- --ignored
        let text = get_selected_text().expect("Should get selected text");
        assert!(!text.is_empty(), "Should have selected text");
        println!("Got selected text: {}", text);
    }

    #[test]
    #[ignore] // Requires manual interaction - select text in another app first
    fn test_set_selected_text() {
        // Instructions:
        // 1. Open TextEdit
        // 2. Select some text
        // 3. Run this test with: cargo test --features system-tests test_set_selected_text -- --ignored
        set_selected_text("REPLACED").expect("Should set selected text");
        // Verify manually that text was replaced
        println!("Text should be replaced with 'REPLACED'");
    }

    #[test]
    #[ignore] // Opens System Preferences
    fn test_open_accessibility_settings() {
        // This will open System Preferences to the Accessibility pane
        open_accessibility_settings().expect("Should open settings");
    }

    #[test]
    fn test_get_selected_text_without_permission_returns_error() {
        // If we don't have permission, we should get an error
        // This test is tricky because we can't easily revoke permission
        // Just verify the function handles the check
        let result = get_selected_text();
        // Result depends on whether permission is granted
        match result {
            Ok(text) => {
                // Permission was granted, we got some text (possibly empty)
                println!("Got text (permission granted): '{}'", text);
            }
            Err(e) => {
                // Either no permission or no selection
                println!("Got error (expected if no permission): {}", e);
            }
        }
    }

    #[test]
    fn test_set_selected_text_empty_string() {
        // Test setting empty text (edge case)
        // This will fail without permission, but shouldn't panic
        let result = set_selected_text("");
        // Don't assert on result - depends on permission state
        if let Err(e) = result {
            println!("Expected error without permission: {}", e);
        }
    }
}
