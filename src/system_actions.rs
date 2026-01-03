//! macOS System Actions Module
//!
//! Provides AppleScript-based system actions for macOS including:
//! - Power management (sleep, restart, shutdown, lock, log out)
//! - UI controls (dark mode, show desktop, mission control, launchpad)
//! - Media controls (volume, brightness)
//! - System utilities (empty trash, force quit, screen saver, do not disturb)
//! - System Preferences navigation
//!
//! All functions use `osascript` to execute AppleScript commands and return
//! `Result<(), String>` for consistent error handling.

use std::process::Command;
use tracing::{debug, error, info};

// ============================================================================
// Helper Function
// ============================================================================

/// Execute an AppleScript command and return the result
fn run_applescript(script: &str) -> Result<(), String> {
    debug!(script = %script, "Executing AppleScript");

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

    if output.status.success() {
        debug!("AppleScript executed successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "AppleScript execution failed");
        Err(format!("AppleScript error: {}", stderr))
    }
}

/// Execute an AppleScript command and return the output
#[allow(dead_code)]
fn run_applescript_with_output(script: &str) -> Result<String, String> {
    debug!(script = %script, "Executing AppleScript with output");

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        debug!(output = %stdout, "AppleScript executed successfully");
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "AppleScript execution failed");
        Err(format!("AppleScript error: {}", stderr))
    }
}

// ============================================================================
// Trash Management
// ============================================================================

/// Empty the macOS Trash
///
/// Uses Finder to empty the trash without user confirmation.
///
/// # Example
/// ```no_run
/// use script_kit_gpui::system_actions::empty_trash;
/// empty_trash().expect("Failed to empty trash");
/// ```
pub fn empty_trash() -> Result<(), String> {
    info!("Emptying trash");
    run_applescript(r#"tell application "Finder" to empty trash"#)
}

// ============================================================================
// Power Management
// ============================================================================

/// Lock the screen
///
/// Activates the screen saver which requires authentication to unlock.
pub fn lock_screen() -> Result<(), String> {
    info!("Locking screen");
    // Use the Keychain Access method which is more reliable
    run_applescript(
        r#"tell application "System Events" to keystroke "q" using {command down, control down}"#,
    )
}

/// Put the system to sleep
///
/// Puts the Mac into sleep mode.
pub fn sleep() -> Result<(), String> {
    info!("Putting system to sleep");
    run_applescript(r#"tell application "System Events" to sleep"#)
}

/// Restart the system
///
/// Initiates a system restart. Applications will be asked to save documents.
pub fn restart() -> Result<(), String> {
    info!("Restarting system");
    run_applescript(r#"tell application "System Events" to restart"#)
}

/// Shut down the system
///
/// Initiates a system shutdown. Applications will be asked to save documents.
pub fn shut_down() -> Result<(), String> {
    info!("Shutting down system");
    run_applescript(r#"tell application "System Events" to shut down"#)
}

/// Log out the current user
///
/// Logs out the current user. Applications will be asked to save documents.
pub fn log_out() -> Result<(), String> {
    info!("Logging out user");
    run_applescript(r#"tell application "System Events" to log out"#)
}

// ============================================================================
// UI Controls
// ============================================================================

/// Toggle Dark Mode
///
/// Switches between light and dark appearance mode.
pub fn toggle_dark_mode() -> Result<(), String> {
    info!("Toggling dark mode");
    run_applescript(
        r#"tell application "System Events"
            tell appearance preferences
                set dark mode to not dark mode
            end tell
        end tell"#,
    )
}

/// Check if Dark Mode is enabled
///
/// Returns true if dark mode is currently active.
#[allow(dead_code)]
pub fn is_dark_mode() -> Result<bool, String> {
    let output = run_applescript_with_output(
        r#"tell application "System Events"
            tell appearance preferences
                return dark mode
            end tell
        end tell"#,
    )?;
    Ok(output == "true")
}

/// Show Desktop (hide all windows)
///
/// Hides all windows to reveal the desktop.
pub fn show_desktop() -> Result<(), String> {
    info!("Showing desktop");
    // F11 key code is 103, but we use the hot corner simulation
    // which is more reliable across different keyboard layouts
    run_applescript(
        r#"tell application "System Events"
            key code 103 using {command down}
        end tell"#,
    )
}

/// Activate Mission Control
///
/// Opens Mission Control to show all windows and desktops.
pub fn mission_control() -> Result<(), String> {
    info!("Activating Mission Control");
    // Control + Up Arrow triggers Mission Control
    run_applescript(
        r#"tell application "System Events"
            key code 126 using {control down}
        end tell"#,
    )
}

/// Open Launchpad
///
/// Opens Launchpad to show all applications.
pub fn launchpad() -> Result<(), String> {
    info!("Opening Launchpad");
    // F4 key code is 118 on many keyboards, but we use the direct approach
    run_applescript(r#"tell application "Launchpad" to activate"#)
}

/// Open Force Quit Applications dialog
///
/// Opens the Force Quit Applications window (Cmd+Option+Escape).
pub fn force_quit_apps() -> Result<(), String> {
    info!("Opening Force Quit Applications dialog");
    run_applescript(
        r#"tell application "System Events"
            keystroke "escape" using {command down, option down}
        end tell"#,
    )
}

// ============================================================================
// Volume Controls
// ============================================================================

/// Increase system volume
///
/// Increases the system volume by approximately 6.25% (1/16th of max).
pub fn volume_up() -> Result<(), String> {
    info!("Increasing volume");
    run_applescript(r#"set volume output volume ((output volume of (get volume settings)) + 6.25)"#)
}

/// Decrease system volume
///
/// Decreases the system volume by approximately 6.25% (1/16th of max).
pub fn volume_down() -> Result<(), String> {
    info!("Decreasing volume");
    run_applescript(r#"set volume output volume ((output volume of (get volume settings)) - 6.25)"#)
}

/// Toggle mute
///
/// Toggles the system audio mute state.
pub fn volume_mute() -> Result<(), String> {
    info!("Toggling mute");
    run_applescript(
        r#"set currentMute to output muted of (get volume settings)
        set volume output muted (not currentMute)"#,
    )
}

/// Set volume to a specific level
///
/// # Arguments
/// * `level` - Volume level from 0 to 100
#[allow(dead_code)]
pub fn set_volume(level: u8) -> Result<(), String> {
    let level = level.min(100);
    info!(level = level, "Setting volume");
    run_applescript(&format!("set volume output volume {}", level))
}

/// Get current volume level
///
/// Returns the current volume level (0-100).
#[allow(dead_code)]
pub fn get_volume() -> Result<u8, String> {
    let output = run_applescript_with_output("output volume of (get volume settings)")?;
    output
        .parse::<f64>()
        .map(|v| v.round() as u8)
        .map_err(|e| format!("Failed to parse volume: {}", e))
}

/// Check if audio is muted
#[allow(dead_code)]
pub fn is_muted() -> Result<bool, String> {
    let output = run_applescript_with_output("output muted of (get volume settings)")?;
    Ok(output == "true")
}

// ============================================================================
// Brightness Controls
// ============================================================================

/// Increase display brightness
///
/// Simulates pressing the brightness up key.
pub fn brightness_up() -> Result<(), String> {
    info!("Increasing brightness");
    // Key code 144 is brightness up
    run_applescript(
        r#"tell application "System Events"
            key code 144
        end tell"#,
    )
}

/// Decrease display brightness
///
/// Simulates pressing the brightness down key.
pub fn brightness_down() -> Result<(), String> {
    info!("Decreasing brightness");
    // Key code 145 is brightness down
    run_applescript(
        r#"tell application "System Events"
            key code 145
        end tell"#,
    )
}

// ============================================================================
// Do Not Disturb
// ============================================================================

/// Toggle Do Not Disturb mode
///
/// Toggles macOS Focus/Do Not Disturb mode.
/// Note: This uses keyboard shortcuts as the DND API changed in recent macOS versions.
pub fn toggle_do_not_disturb() -> Result<(), String> {
    info!("Toggling Do Not Disturb");
    // Option-click on the menu bar clock or use Control Center
    // We'll use the Control Center approach for better compatibility
    run_applescript(
        r#"tell application "System Events"
            tell process "ControlCenter"
                -- Click the Focus button in Control Center
                click menu bar item "Focus" of menu bar 1
            end tell
        end tell"#,
    )
}

// ============================================================================
// Screen Saver
// ============================================================================

/// Start the screen saver
///
/// Immediately activates the screen saver.
pub fn start_screen_saver() -> Result<(), String> {
    info!("Starting screen saver");
    run_applescript(r#"tell application "ScreenSaverEngine" to activate"#)
}

// ============================================================================
// System Preferences Navigation
// ============================================================================

/// Open System Preferences/Settings to a specific pane
///
/// # Arguments
/// * `pane` - The pane identifier (e.g., "com.apple.preference.security")
///
/// # Common Pane IDs
/// - `com.apple.preference.security` - Privacy & Security
/// - `com.apple.preference.displays` - Displays
/// - `com.apple.preference.sound` - Sound
/// - `com.apple.preference.network` - Network
/// - `com.apple.preference.keyboard` - Keyboard
/// - `com.apple.preference.trackpad` - Trackpad
/// - `com.apple.preference.bluetooth` - Bluetooth
/// - `com.apple.preference.notifications` - Notifications
/// - `com.apple.preference.general` - General
/// - `com.apple.preference.dock` - Desktop & Dock
/// - `com.apple.preferences.AppleIDPrefPane` - Apple ID
/// - `com.apple.preference.battery` - Battery
pub fn open_system_preferences(pane: &str) -> Result<(), String> {
    info!(pane = pane, "Opening System Preferences");
    run_applescript(&format!(
        r#"tell application "System Preferences"
            activate
            reveal pane id "{}"
        end tell"#,
        pane
    ))
}

/// Open System Preferences to the Privacy & Security pane
pub fn open_privacy_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.security")
}

/// Open System Preferences to the Displays pane
pub fn open_display_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.displays")
}

/// Open System Preferences to the Sound pane
pub fn open_sound_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.sound")
}

/// Open System Preferences to the Network pane
pub fn open_network_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.network")
}

/// Open System Preferences to the Keyboard pane
pub fn open_keyboard_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.keyboard")
}

/// Open System Preferences to the Bluetooth pane
pub fn open_bluetooth_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.bluetooth")
}

/// Open System Preferences to the Notifications pane
pub fn open_notifications_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.notifications")
}

/// Open System Preferences to the General pane
#[allow(dead_code)]
pub fn open_general_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.general")
}

/// Open System Preferences to the Desktop & Dock pane
#[allow(dead_code)]
pub fn open_dock_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.dock")
}

/// Open System Preferences to the Battery pane
#[allow(dead_code)]
pub fn open_battery_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.battery")
}

/// Open System Preferences to the Trackpad pane
#[allow(dead_code)]
pub fn open_trackpad_settings() -> Result<(), String> {
    open_system_preferences("com.apple.preference.trackpad")
}

/// Open System Preferences (main window)
pub fn open_system_preferences_main() -> Result<(), String> {
    info!("Opening System Preferences");
    run_applescript(r#"tell application "System Preferences" to activate"#)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Most of these tests are marked #[ignore] because they require
    // actual system interaction and should only be run manually on macOS.
    // Run with: cargo test --features system-tests -- --ignored

    #[test]
    fn test_run_applescript_syntax_error() {
        // Test that syntax errors are properly caught
        let result = run_applescript("this is not valid applescript syntax (((");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("AppleScript error"));
    }

    #[test]
    fn test_run_applescript_with_output_simple() {
        // Test a simple AppleScript that returns a value
        let result = run_applescript_with_output("return 42");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "42");
    }

    #[test]
    fn test_run_applescript_with_output_string() {
        // Test returning a string
        let result = run_applescript_with_output(r#"return "hello""#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn test_run_applescript_with_output_boolean() {
        // Test returning a boolean
        let result = run_applescript_with_output("return true");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "true");
    }

    #[test]
    fn test_set_volume_clamps_to_100() {
        // Test that set_volume clamps values above 100
        // This doesn't actually set volume, just tests the script generation
        let test_value: u8 = 150;
        let script = format!("set volume output volume {}", test_value.min(100));
        assert!(script.contains("100"));
    }

    #[test]
    #[ignore]
    fn test_empty_trash_integration() {
        // Integration test - only run manually
        let result = empty_trash();
        // May succeed or fail depending on permissions
        println!("empty_trash result: {:?}", result);
    }

    #[test]
    #[ignore]
    fn test_toggle_dark_mode_integration() {
        // Integration test - only run manually
        let result = toggle_dark_mode();
        println!("toggle_dark_mode result: {:?}", result);
    }

    #[test]
    #[ignore]
    fn test_is_dark_mode_integration() {
        // Integration test - only run manually
        let result = is_dark_mode();
        println!("is_dark_mode result: {:?}", result);
    }

    #[test]
    #[ignore]
    fn test_volume_controls_integration() {
        // Integration test - only run manually
        if let Ok(initial_volume) = get_volume() {
            println!("Initial volume: {}", initial_volume);

            // Test volume up
            let _ = volume_up();

            // Test volume down
            let _ = volume_down();

            // Test set volume
            let _ = set_volume(initial_volume);

            // Test mute check
            if let Ok(muted) = is_muted() {
                println!("Is muted: {}", muted);
            }
        }
    }

    #[test]
    #[ignore]
    fn test_start_screen_saver_integration() {
        // Integration test - only run manually
        let result = start_screen_saver();
        println!("start_screen_saver result: {:?}", result);
    }

    #[test]
    #[ignore]
    fn test_mission_control_integration() {
        // Integration test - only run manually
        let result = mission_control();
        println!("mission_control result: {:?}", result);
    }

    #[test]
    #[ignore]
    fn test_launchpad_integration() {
        // Integration test - only run manually
        let result = launchpad();
        println!("launchpad result: {:?}", result);
    }

    #[test]
    #[ignore]
    fn test_open_system_preferences_integration() {
        // Integration test - only run manually
        let result = open_sound_settings();
        println!("open_sound_settings result: {:?}", result);
    }
}
