//! macOS Launch at Login functionality using SMAppService
//!
//! This module provides functions to enable/disable launch-at-login
//! behavior using the modern SMAppService API available in macOS 13+.
//!

// Allow dead code as this module provides utility functions that may not all be used yet
#![allow(dead_code)]
//!
//! # Platform Support
//! - macOS 13+: Uses SMAppService
//! - Other platforms: No-op (returns success, status always false)

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};

/// Enables the current application to launch at login.
///
/// Uses SMAppService on macOS 13+ to register the app as a login item.
///
/// # Errors
/// Returns an error if the registration fails (e.g., permission denied).
///
/// # Platform Behavior
/// - macOS 13+: Registers app with SMAppService
/// - Other platforms: Returns Ok(()) (no-op)
#[cfg(target_os = "macos")]
pub fn enable_login_item() -> Result<()> {
    use smappservice_rs::{AppService, ServiceType};

    info!("Enabling launch at login");

    let service = AppService::new(ServiceType::MainApp);

    match service.register() {
        Ok(()) => {
            info!("Successfully enabled launch at login");
            Ok(())
        }
        Err(e) => {
            error!(error = %e, "Failed to enable launch at login");
            Err(anyhow::anyhow!("Failed to register login item: {}", e))
                .context("SMAppService registration failed")
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn enable_login_item() -> Result<()> {
    debug!("enable_login_item: no-op on non-macOS platform");
    Ok(())
}

/// Disables the current application from launching at login.
///
/// Uses SMAppService on macOS 13+ to unregister the app as a login item.
///
/// # Errors
/// Returns an error if the unregistration fails.
///
/// # Platform Behavior
/// - macOS 13+: Unregisters app with SMAppService
/// - Other platforms: Returns Ok(()) (no-op)
#[cfg(target_os = "macos")]
pub fn disable_login_item() -> Result<()> {
    use smappservice_rs::{AppService, ServiceType};

    info!("Disabling launch at login");

    let service = AppService::new(ServiceType::MainApp);

    match service.unregister() {
        Ok(()) => {
            info!("Successfully disabled launch at login");
            Ok(())
        }
        Err(e) => {
            // Note: unregister may fail if not currently registered, which is fine
            warn!(error = %e, "Failed to disable launch at login (may not be registered)");
            Err(anyhow::anyhow!("Failed to unregister login item: {}", e))
                .context("SMAppService unregistration failed")
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn disable_login_item() -> Result<()> {
    debug!("disable_login_item: no-op on non-macOS platform");
    Ok(())
}

/// Checks if the current application is configured to launch at login.
///
/// # Returns
/// - `true` if the app is registered as a login item
/// - `false` if not registered or on non-macOS platforms
///
/// # Platform Behavior
/// - macOS 13+: Queries SMAppService status
/// - Other platforms: Always returns `false`
#[cfg(target_os = "macos")]
pub fn is_login_item_enabled() -> bool {
    use smappservice_rs::{AppService, ServiceStatus, ServiceType};

    let service = AppService::new(ServiceType::MainApp);
    let status = service.status();

    debug!(?status, "Login item status");

    matches!(status, ServiceStatus::Enabled)
}

#[cfg(not(target_os = "macos"))]
pub fn is_login_item_enabled() -> bool {
    debug!("is_login_item_enabled: always false on non-macOS platform");
    false
}

/// Opens System Settings to the Login Items section.
///
/// Use this to help users manually approve the app if needed.
///
/// # Platform Behavior
/// - macOS 13+: Opens System Settings > General > Login Items
/// - Other platforms: No-op
#[cfg(target_os = "macos")]
pub fn open_login_items_settings() {
    use smappservice_rs::AppService;

    info!("Opening Login Items settings");
    AppService::open_system_settings_login_items();
}

#[cfg(not(target_os = "macos"))]
pub fn open_login_items_settings() {
    debug!("open_login_items_settings: no-op on non-macOS platform");
}

/// Toggles the login item status.
///
/// If currently enabled, disables it. If currently disabled, enables it.
///
/// # Returns
/// The new enabled state after toggling.
///
/// # Errors
/// Returns an error if the enable/disable operation fails.
pub fn toggle_login_item() -> Result<bool> {
    let currently_enabled = is_login_item_enabled();

    if currently_enabled {
        disable_login_item()?;
        Ok(false)
    } else {
        enable_login_item()?;
        Ok(true)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that the public API has the correct signatures.
    /// This test always runs and doesn't modify system state.
    #[test]
    fn test_api_signatures() {
        // Verify enable_login_item returns Result<()>
        let _: fn() -> Result<()> = enable_login_item;

        // Verify disable_login_item returns Result<()>
        let _: fn() -> Result<()> = disable_login_item;

        // Verify is_login_item_enabled returns bool
        let _: fn() -> bool = is_login_item_enabled;

        // Verify toggle_login_item returns Result<bool>
        let _: fn() -> Result<bool> = toggle_login_item;

        // Verify open_login_items_settings exists
        let _: fn() = open_login_items_settings;
    }

    /// Test that is_login_item_enabled doesn't panic.
    /// This is a read-only operation so safe to run without system-tests feature.
    #[test]
    fn test_login_item_status_does_not_panic() {
        // Just check it returns without panicking
        let _status = is_login_item_enabled();
    }

    /// Test enabling the login item.
    /// This modifies system state so requires the system-tests feature.
    #[test]
    #[cfg(feature = "system-tests")]
    #[ignore] // Still ignore by default even with feature - run explicitly with --ignored
    fn test_login_item_enable() {
        // Enable login item
        let result = enable_login_item();

        // On macOS, this should succeed (assuming proper entitlements)
        // May fail in CI due to sandboxing
        if result.is_ok() {
            assert!(is_login_item_enabled(), "Expected login item to be enabled");

            // Clean up - disable it
            let _ = disable_login_item();
        } else {
            eprintln!(
                "Note: enable_login_item failed (may be expected in CI): {:?}",
                result
            );
        }
    }

    /// Test disabling the login item.
    /// This modifies system state so requires the system-tests feature.
    #[test]
    #[cfg(feature = "system-tests")]
    #[ignore] // Still ignore by default even with feature - run explicitly with --ignored
    fn test_login_item_disable() {
        // First enable (if possible)
        let _ = enable_login_item();

        // Then disable
        let result = disable_login_item();

        if result.is_ok() {
            assert!(
                !is_login_item_enabled(),
                "Expected login item to be disabled"
            );
        } else {
            eprintln!(
                "Note: disable_login_item failed (may be expected in CI): {:?}",
                result
            );
        }
    }

    /// Test toggling the login item.
    /// This modifies system state so requires the system-tests feature.
    #[test]
    #[cfg(feature = "system-tests")]
    #[ignore] // Still ignore by default even with feature - run explicitly with --ignored
    fn test_login_item_toggle() {
        let initial_state = is_login_item_enabled();

        // Toggle
        let result = toggle_login_item();

        if let Ok(new_state) = result {
            assert_ne!(initial_state, new_state, "Toggle should change the state");

            // Toggle back to restore
            let _ = toggle_login_item();
        } else {
            eprintln!(
                "Note: toggle_login_item failed (may be expected in CI): {:?}",
                result
            );
        }
    }
}
