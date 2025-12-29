//! Text Injection Module for macOS
//!
//! Provides text injection functionality for text expansion/snippet systems.
//! Uses the proven Espanso/Raycast pattern:
//! 1. Delete trigger text with simulated backspace key events
//! 2. Insert replacement text via clipboard paste (Cmd+V)
//!
//! ## Architecture
//!
//! - `delete_chars()`: Simulates N backspace key events using CGEventPost
//! - `paste_text()`: Clipboard-based paste with save/restore pattern
//! - `inject_text()`: Convenience function combining both operations
//!
//! ## Configurable Delays
//!
//! All timing is configurable via `TextInjectorConfig`:
//! - `key_delay_ms`: Delay between backspace events (default: 2ms)
//! - `pre_paste_delay_ms`: Delay before paste operation (default: 50ms)
//! - `post_paste_delay_ms`: Delay before restoring clipboard (default: 100ms)
//!
//! ## Permissions
//!
//! Requires Accessibility permission in System Preferences > Privacy & Security > Accessibility

use anyhow::{Context, Result};
use arboard::Clipboard;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for text injection timing and behavior
#[derive(Debug, Clone)]
pub struct TextInjectorConfig {
    /// Delay in milliseconds between backspace key events (default: 2ms)
    pub key_delay_ms: u64,
    /// Delay in milliseconds before paste operation (default: 50ms)
    pub pre_paste_delay_ms: u64,
    /// Delay in milliseconds before restoring clipboard (default: 100ms)
    pub post_paste_delay_ms: u64,
}

impl Default for TextInjectorConfig {
    fn default() -> Self {
        Self {
            key_delay_ms: 2,
            pre_paste_delay_ms: 50,
            post_paste_delay_ms: 100,
        }
    }
}

// ============================================================================
// Text Injector
// ============================================================================

/// Text injector for deleting trigger text and inserting replacements
///
/// Uses macOS Core Graphics API for key simulation and clipboard for pasting.
/// This is the same reliable pattern used by Espanso and Raycast.
#[derive(Debug, Clone)]
pub struct TextInjector {
    config: TextInjectorConfig,
}

impl Default for TextInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInjector {
    /// Create a new TextInjector with default configuration
    pub fn new() -> Self {
        Self {
            config: TextInjectorConfig::default(),
        }
    }

    /// Create a new TextInjector with custom configuration
    pub fn with_config(config: TextInjectorConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &TextInjectorConfig {
        &self.config
    }

    /// Delete characters by sending backspace key events
    ///
    /// Simulates N backspace keystrokes using CGEventPost to delete
    /// the trigger text that the user typed.
    ///
    /// # Arguments
    /// * `count` - Number of characters to delete (backspace events to send)
    ///
    /// # Errors
    /// Returns error if CGEventPost fails
    ///
    /// # Example
    /// ```ignore
    /// let injector = TextInjector::new();
    /// injector.delete_chars(5)?; // Delete 5 characters
    /// ```
    #[instrument(skip(self), fields(count))]
    pub fn delete_chars(&self, count: usize) -> Result<()> {
        if count == 0 {
            debug!("No characters to delete");
            return Ok(());
        }

        debug!(count, "Deleting characters via backspace simulation");

        for i in 0..count {
            simulate_backspace()?;

            // Add delay between keystrokes for reliability
            if i < count - 1 && self.config.key_delay_ms > 0 {
                thread::sleep(Duration::from_millis(self.config.key_delay_ms));
            }
        }

        info!(count, "Deleted characters successfully");
        Ok(())
    }

    /// Paste text using clipboard and Cmd+V simulation
    ///
    /// This function:
    /// 1. Saves the current clipboard contents
    /// 2. Sets the clipboard to the new text
    /// 3. Simulates Cmd+V to paste
    /// 4. Waits for paste to complete
    /// 5. Restores the original clipboard contents
    ///
    /// # Arguments
    /// * `text` - The text to paste
    ///
    /// # Errors
    /// Returns error if clipboard or paste operation fails
    ///
    /// # Example
    /// ```ignore
    /// let injector = TextInjector::new();
    /// injector.paste_text("Hello, World!")?;
    /// ```
    #[instrument(skip(self, text), fields(text_len = text.len()))]
    pub fn paste_text(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            debug!("Empty text, nothing to paste");
            return Ok(());
        }

        debug!(text_len = text.len(), "Pasting text via clipboard");

        let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;

        // Save original clipboard contents (text only for now)
        let original = clipboard.get_text().ok();
        debug!(had_original = original.is_some(), "Saved original clipboard");

        // Set new text to clipboard
        clipboard
            .set_text(text)
            .context("Failed to set clipboard text")?;

        // Pre-paste delay to ensure clipboard is ready
        if self.config.pre_paste_delay_ms > 0 {
            thread::sleep(Duration::from_millis(self.config.pre_paste_delay_ms));
        }

        // Simulate Cmd+V using Core Graphics
        simulate_paste()?;

        // Post-paste delay before restoring clipboard
        if self.config.post_paste_delay_ms > 0 {
            thread::sleep(Duration::from_millis(self.config.post_paste_delay_ms));
        }

        // Restore original clipboard (best effort)
        if let Some(original_text) = original {
            if let Err(e) = clipboard.set_text(&original_text) {
                warn!(error = %e, "Failed to restore original clipboard");
            } else {
                debug!("Restored original clipboard");
            }
        }

        info!(text_len = text.len(), "Pasted text successfully");
        Ok(())
    }

    /// Inject text by deleting trigger characters and pasting replacement
    ///
    /// This is a convenience function that combines `delete_chars()` and
    /// `paste_text()` for the common text expansion use case.
    ///
    /// # Arguments
    /// * `delete_count` - Number of characters to delete (trigger length)
    /// * `replacement` - The text to insert
    ///
    /// # Errors
    /// Returns error if delete or paste operation fails
    ///
    /// # Example
    /// ```ignore
    /// let injector = TextInjector::new();
    /// // User typed "btw" (3 chars), replace with "by the way"
    /// injector.inject_text(3, "by the way")?;
    /// ```
    #[allow(dead_code)]
    #[instrument(skip(self, replacement), fields(delete_count, replacement_len = replacement.len()))]
    pub fn inject_text(&self, delete_count: usize, replacement: &str) -> Result<()> {
        info!(
            delete_count,
            replacement_len = replacement.len(),
            "Injecting text"
        );

        // Delete trigger characters
        self.delete_chars(delete_count)?;

        // Small delay between delete and paste operations
        if self.config.pre_paste_delay_ms > 0 {
            thread::sleep(Duration::from_millis(self.config.pre_paste_delay_ms));
        }

        // Paste replacement text
        self.paste_text(replacement)?;

        info!("Text injection completed");
        Ok(())
    }
}

// ============================================================================
// Core Graphics Key Simulation
// ============================================================================

/// Simulate a single backspace keypress using Core Graphics
///
/// Sends both key down and key up events for the backspace key.
fn simulate_backspace() -> Result<()> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // Backspace key is keycode 51 on macOS
    const KEY_BACKSPACE: CGKeyCode = 51;

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .ok()
        .context("Failed to create CGEventSource")?;

    // Key down event
    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_BACKSPACE, true)
        .ok()
        .context("Failed to create backspace key down event")?;

    // Key up event
    let key_up = CGEvent::new_keyboard_event(source, KEY_BACKSPACE, false)
        .ok()
        .context("Failed to create backspace key up event")?;

    // Post events to HID system
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(1)); // Brief delay between down/up
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

/// Simulate Cmd+V paste keystroke using Core Graphics
///
/// Sends key down and key up events for 'v' with Command modifier.
fn simulate_paste() -> Result<()> {
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
        .context("Failed to create paste key down event")?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);

    // Create key up event for 'v' with Cmd modifier
    let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .ok()
        .context("Failed to create paste key up event")?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    // Post events
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(5)); // Brief delay between down/up
    key_up.post(CGEventTapLocation::HID);

    debug!("Simulated Cmd+V via Core Graphics");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TextInjectorConfig::default();
        assert_eq!(config.key_delay_ms, 2);
        assert_eq!(config.pre_paste_delay_ms, 50);
        assert_eq!(config.post_paste_delay_ms, 100);
    }

    #[test]
    fn test_custom_config() {
        let config = TextInjectorConfig {
            key_delay_ms: 5,
            pre_paste_delay_ms: 100,
            post_paste_delay_ms: 200,
        };
        let injector = TextInjector::with_config(config.clone());
        assert_eq!(injector.config().key_delay_ms, 5);
        assert_eq!(injector.config().pre_paste_delay_ms, 100);
        assert_eq!(injector.config().post_paste_delay_ms, 200);
    }

    #[test]
    fn test_injector_new() {
        let injector = TextInjector::new();
        assert_eq!(injector.config().key_delay_ms, 2);
    }

    #[test]
    fn test_injector_default() {
        let injector = TextInjector::default();
        assert_eq!(injector.config().key_delay_ms, 2);
    }

    #[test]
    fn test_delete_chars_zero() {
        // Deleting zero chars should succeed without doing anything
        let injector = TextInjector::new();
        // This won't actually simulate keys in tests, but checks the early return
        let result = injector.delete_chars(0);
        assert!(result.is_ok());
    }
}

// ============================================================================
// System Tests (require `cargo test --features system-tests`)
// ============================================================================

#[cfg(all(test, feature = "system-tests"))]
mod system_tests {
    use super::*;

    #[test]
    #[ignore] // Requires accessibility permission and user interaction
    fn test_delete_chars_sends_backspaces() {
        // Instructions:
        // 1. Open TextEdit and type "hello"
        // 2. Run: cargo test --features system-tests test_delete_chars_sends_backspaces -- --ignored
        // 3. The last 2 characters should be deleted, leaving "hel"
        let injector = TextInjector::new();
        injector.delete_chars(2).expect("Should delete chars");
        println!("Deleted 2 characters");
    }

    #[test]
    #[ignore] // Requires accessibility permission and user interaction
    fn test_paste_text() {
        // Instructions:
        // 1. Open TextEdit with cursor positioned
        // 2. Run: cargo test --features system-tests test_paste_text -- --ignored
        // 3. "TEST PASTE" should be inserted
        let injector = TextInjector::new();
        injector.paste_text("TEST PASTE").expect("Should paste");
        println!("Pasted text");
    }

    #[test]
    #[ignore] // Requires accessibility permission and user interaction
    fn test_inject_text() {
        // Instructions:
        // 1. Open TextEdit and type "btw"
        // 2. Run: cargo test --features system-tests test_inject_text -- --ignored
        // 3. "btw" should be replaced with "by the way"
        let injector = TextInjector::new();
        injector
            .inject_text(3, "by the way")
            .expect("Should inject");
        println!("Injected text");
    }

    #[test]
    fn test_empty_paste() {
        // Pasting empty text should succeed
        let injector = TextInjector::new();
        let result = injector.paste_text("");
        assert!(result.is_ok());
    }
}
