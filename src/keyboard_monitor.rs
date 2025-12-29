//! Global keyboard monitoring using macOS CGEventTap API
//!
//! This module provides system-wide keyboard event capture, regardless of which
//! application has focus. This is essential for text expansion/snippet features
//! that need to detect trigger sequences typed in any application.
//!
//! # Requirements
//! - macOS only (uses Core Graphics CGEventTap)
//! - Requires Accessibility permissions to be enabled in System Preferences
//!
//! # Example
//! ```no_run
//! use script_kit_gpui::keyboard_monitor::{KeyboardMonitor, KeyEvent};
//!
//! let monitor = KeyboardMonitor::new(|event: KeyEvent| {
//!     println!("Key pressed: {:?}", event.character);
//! });
//!
//! monitor.start().expect("Failed to start keyboard monitor");
//! // ... monitor runs in background thread ...
//! monitor.stop();
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use core_foundation::runloop::{kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    EventField,
};
use macos_accessibility_client::accessibility;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Errors that can occur when using the keyboard monitor
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum KeyboardMonitorError {
    #[error("Accessibility permissions not granted. Please enable in System Preferences > Privacy & Security > Accessibility")]
    AccessibilityNotGranted,

    #[error("Failed to create event tap - this may indicate accessibility permissions issue")]
    EventTapCreationFailed,

    #[error("Failed to create run loop source from event tap")]
    RunLoopSourceCreationFailed,

    #[error("Monitor is already running")]
    AlreadyRunning,

    #[error("Monitor is not running")]
    NotRunning,

    #[error("Failed to start monitor thread")]
    ThreadSpawnFailed,
}

/// Represents a keyboard event captured by the monitor
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct KeyEvent {
    /// The character that was typed (if available)
    /// This is the actual character produced, taking into account modifiers
    pub character: Option<String>,

    /// The virtual key code (hardware key identifier)
    pub key_code: u16,

    /// Whether the shift modifier was held
    pub shift: bool,

    /// Whether the control modifier was held
    pub control: bool,

    /// Whether the option/alt modifier was held
    pub option: bool,

    /// Whether the command modifier was held
    pub command: bool,

    /// Whether this is an auto-repeat event
    pub is_repeat: bool,
}

/// Callback type for receiving keyboard events
/// Must be Send + Sync since it's shared across threads via Arc
pub type KeyEventCallback = Box<dyn Fn(KeyEvent) + Send + Sync + 'static>;

/// Global keyboard monitor using macOS CGEventTap
///
/// This monitor captures keystrokes system-wide, regardless of which application
/// has focus. It runs on a dedicated background thread with its own CFRunLoop.
pub struct KeyboardMonitor {
    /// Whether the monitor is currently running
    running: Arc<AtomicBool>,

    /// Handle to the background thread running the event loop
    thread_handle: Option<JoinHandle<()>>,

    /// The callback to invoke for each key event
    callback: Arc<KeyEventCallback>,

    /// Run loop reference for stopping (stored after start)
    run_loop: Arc<std::sync::Mutex<Option<CFRunLoop>>>,
}

impl KeyboardMonitor {
    /// Create a new keyboard monitor with the given callback
    ///
    /// The callback will be invoked for each key-down event captured.
    /// The monitor does not start automatically - call `start()` to begin monitoring.
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(KeyEvent) + Send + Sync + 'static,
    {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            callback: Arc::new(Box::new(callback)),
            run_loop: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Check if accessibility permissions are granted
    ///
    /// Returns true if the application has been granted accessibility permissions.
    /// If false, the monitor will fail to start.
    pub fn has_accessibility_permission() -> bool {
        accessibility::application_is_trusted()
    }

    /// Check if accessibility permissions are granted, prompting the user if not
    ///
    /// This will show the macOS accessibility permission dialog if permissions
    /// haven't been granted yet. Returns true if permissions are granted.
    #[allow(dead_code)]
    pub fn request_accessibility_permission() -> bool {
        accessibility::application_is_trusted_with_prompt()
    }

    /// Start the keyboard monitor
    ///
    /// This spawns a background thread that captures keyboard events system-wide.
    /// The provided callback will be invoked for each key-down event.
    ///
    /// # Errors
    /// - `AccessibilityNotGranted` - Accessibility permissions not enabled
    /// - `AlreadyRunning` - Monitor is already running
    /// - `EventTapCreationFailed` - Failed to create the event tap
    pub fn start(&mut self) -> Result<(), KeyboardMonitorError> {
        // Check if already running
        if self.running.load(Ordering::SeqCst) {
            return Err(KeyboardMonitorError::AlreadyRunning);
        }

        // Check accessibility permissions
        if !Self::has_accessibility_permission() {
            warn!("Accessibility permissions not granted for keyboard monitor");
            return Err(KeyboardMonitorError::AccessibilityNotGranted);
        }

        info!("Starting global keyboard monitor");

        let running = Arc::clone(&self.running);
        let callback = Arc::clone(&self.callback);
        let run_loop_storage = Arc::clone(&self.run_loop);

        // Set running flag before spawning thread
        self.running.store(true, Ordering::SeqCst);

        let handle = thread::Builder::new()
            .name("keyboard-monitor".to_string())
            .spawn(move || {
                Self::event_loop(running, callback, run_loop_storage);
            })
            .map_err(|e| {
                error!("Failed to spawn keyboard monitor thread: {}", e);
                self.running.store(false, Ordering::SeqCst);
                KeyboardMonitorError::ThreadSpawnFailed
            })?;

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Stop the keyboard monitor
    ///
    /// This stops the background thread and cleans up resources.
    /// Safe to call even if the monitor is not running.
    pub fn stop(&mut self) {
        if !self.running.load(Ordering::SeqCst) {
            debug!("Keyboard monitor already stopped");
            return;
        }

        info!("Stopping global keyboard monitor");

        // Signal the thread to stop
        self.running.store(false, Ordering::SeqCst);

        // Stop the run loop
        if let Ok(guard) = self.run_loop.lock() {
            if let Some(ref run_loop) = *guard {
                run_loop.stop();
            }
        }

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                error!("Keyboard monitor thread panicked: {:?}", e);
            }
        }

        debug!("Keyboard monitor stopped");
    }

    /// Check if the monitor is currently running
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// The main event loop that runs on the background thread
    fn event_loop(
        running: Arc<AtomicBool>,
        callback: Arc<KeyEventCallback>,
        run_loop_storage: Arc<std::sync::Mutex<Option<CFRunLoop>>>,
    ) {
        debug!("Keyboard monitor event loop starting");

        // Get current run loop and store it for stopping
        let current_run_loop = CFRunLoop::get_current();
        if let Ok(mut guard) = run_loop_storage.lock() {
            *guard = Some(current_run_loop.clone());
        }

        // Create event tap for key down events
        debug!("Creating CGEventTap with HID location for KeyDown events");
        let event_tap_result = CGEventTap::new(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly, // We only observe, don't modify
            vec![CGEventType::KeyDown],
            move |_proxy, _event_type, event: &CGEvent| {
                debug!("CGEventTap callback invoked - key event received!");
                
                // Extract key event information
                let key_event = Self::extract_key_event(event);
                
                debug!(
                    character = ?key_event.character,
                    key_code = key_event.key_code,
                    "Extracted key event"
                );

                // Invoke callback
                callback(key_event);

                // Return None to not modify the event (we're just observing)
                None
            },
        );

        let event_tap = match event_tap_result {
            Ok(tap) => tap,
            Err(()) => {
                error!("Failed to create CGEventTap - accessibility permissions may not be granted");
                running.store(false, Ordering::SeqCst);
                return;
            }
        };

        // Create run loop source from the event tap
        let run_loop_source = match event_tap.mach_port.create_runloop_source(0) {
            Ok(source) => source,
            Err(()) => {
                error!("Failed to create run loop source from event tap");
                running.store(false, Ordering::SeqCst);
                return;
            }
        };

        // Add source to run loop and enable the tap
        unsafe {
            current_run_loop.add_source(&run_loop_source, kCFRunLoopCommonModes);
        }
        event_tap.enable();

        info!("Keyboard monitor event tap enabled, entering run loop");

        // Run the loop until stopped
        while running.load(Ordering::SeqCst) {
            // Run for a short interval, then check if we should stop
            // Note: Must use kCFRunLoopDefaultMode (not kCFRunLoopCommonModes) for run_in_mode
            let result = CFRunLoop::run_in_mode(
                unsafe { kCFRunLoopDefaultMode },
                Duration::from_millis(100),
                true,
            );

            // Check if run loop was stopped externally
            if matches!(
                result,
                core_foundation::runloop::CFRunLoopRunResult::Stopped
            ) {
                debug!("Run loop was stopped");
                break;
            }
        }

        // Clean up: remove source from run loop
        // Note: The event tap will be disabled when it goes out of scope

        debug!("Keyboard monitor event loop exiting");
        running.store(false, Ordering::SeqCst);

        // Clear the stored run loop
        if let Ok(mut guard) = run_loop_storage.lock() {
            *guard = None;
        }
    }

    /// Extract key event information from a CGEvent
    fn extract_key_event(event: &CGEvent) -> KeyEvent {
        use core_graphics::event::CGEventFlags;

        // Get key code
        let key_code =
            event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;

        // Get modifier flags
        let flags = event.get_flags();
        let shift = flags.contains(CGEventFlags::CGEventFlagShift);
        let control = flags.contains(CGEventFlags::CGEventFlagControl);
        let option = flags.contains(CGEventFlags::CGEventFlagAlternate);
        let command = flags.contains(CGEventFlags::CGEventFlagCommand);

        // Check if auto-repeat
        let is_repeat =
            event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0;

        // Try to get the character from the event
        // This uses the CGEventKeyboardGetUnicodeString function internally
        let character = Self::get_character_from_event(event);

        KeyEvent {
            character,
            key_code,
            shift,
            control,
            option,
            command,
            is_repeat,
        }
    }

    /// Get the character string from a keyboard event
    ///
    /// This attempts to get the actual character that would be typed,
    /// taking into account the keyboard layout and modifier keys.
    fn get_character_from_event(event: &CGEvent) -> Option<String> {
        // We need to use the CGEventKeyboardGetUnicodeString function
        // which isn't directly exposed by core-graphics, so we'll use FFI
        extern "C" {
            fn CGEventKeyboardGetUnicodeString(
                event: core_graphics::sys::CGEventRef,
                max_len: libc::c_ulong,
                actual_len: *mut libc::c_ulong,
                buffer: *mut u16,
            );
        }

        let mut buffer: [u16; 4] = [0; 4];
        let mut actual_len: libc::c_ulong = 0;

        unsafe {
            use foreign_types::ForeignType;
            // Get raw pointer to the CGEvent for FFI call
            let event_ptr = event.as_ptr();
            CGEventKeyboardGetUnicodeString(
                event_ptr,
                4,
                &mut actual_len,
                buffer.as_mut_ptr(),
            );
        }

        if actual_len > 0 {
            String::from_utf16(&buffer[..actual_len as usize]).ok()
        } else {
            None
        }
    }
}

impl Drop for KeyboardMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

// KeyboardMonitor is Send because it uses Arc for shared state
// and the callback is required to be Send
unsafe impl Send for KeyboardMonitor {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessibility_check_does_not_panic() {
        // This test just verifies the accessibility check doesn't panic
        // The actual result depends on system permissions
        let _ = KeyboardMonitor::has_accessibility_permission();
    }

    #[test]
    fn test_key_event_creation() {
        let event = KeyEvent {
            character: Some("a".to_string()),
            key_code: 0,
            shift: false,
            control: false,
            option: false,
            command: false,
            is_repeat: false,
        };

        assert_eq!(event.character, Some("a".to_string()));
        assert!(!event.shift);
        assert!(!event.is_repeat);
    }

    #[test]
    fn test_monitor_not_running_initially() {
        let monitor = KeyboardMonitor::new(|_| {});
        assert!(!monitor.is_running());
    }

    #[test]
    fn test_stop_when_not_running_is_safe() {
        let mut monitor = KeyboardMonitor::new(|_| {});
        // Should not panic
        monitor.stop();
        assert!(!monitor.is_running());
    }

    // Integration tests that require accessibility permissions are marked as ignored
    // Run with: cargo test --features system-tests -- --ignored
    #[test]
    #[ignore = "Requires accessibility permissions"]
    fn test_start_and_stop() {
        let mut monitor = KeyboardMonitor::new(|event| {
            println!("Key event: {:?}", event);
        });

        if !KeyboardMonitor::has_accessibility_permission() {
            eprintln!("Skipping test - accessibility permissions not granted");
            return;
        }

        assert!(monitor.start().is_ok());
        assert!(monitor.is_running());

        // Let it run briefly
        std::thread::sleep(std::time::Duration::from_millis(100));

        monitor.stop();
        assert!(!monitor.is_running());
    }

    #[test]
    #[ignore = "Requires accessibility permissions"]
    fn test_double_start_fails() {
        let mut monitor = KeyboardMonitor::new(|_| {});

        if !KeyboardMonitor::has_accessibility_permission() {
            eprintln!("Skipping test - accessibility permissions not granted");
            return;
        }

        assert!(monitor.start().is_ok());
        assert!(matches!(
            monitor.start(),
            Err(KeyboardMonitorError::AlreadyRunning)
        ));

        monitor.stop();
    }
}
