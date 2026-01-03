//! Window Manager Module for Script Kit GPUI
//!
//! # Problem
//! When GPUI creates windows and macOS creates tray icons, the app's windows array
//! contains multiple windows in unpredictable order. Using `objectAtIndex:0` to find
//! "our" window fails because:
//! - Tray icon popups appear as windows
//! - Menu bar items create windows
//! - System overlays create windows
//!
//! Debug logs showed:
//! - Window[0]: 34x24 - Tray icon popup
//! - Window[1]: 0x37 - Menu bar
//! - Window[2]: 0x24 - System window
//! - Window[3]: 750x501 - Our main window (the one we want!)
//!
//! # Solution
//! This module provides a thread-safe registry to track our windows by role.
//! After GPUI creates a window, we register it with its role (MainWindow, etc.)
//! and later retrieve it reliably, avoiding the index-based lookup problem.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    Window Manager Architecture                       │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                      │
//! │  ┌──────────────┐     ┌───────────────────────────────────────┐    │
//! │  │  main.rs     │     │     WindowManager (Global Singleton)  │    │
//! │  │              │     │                                        │    │
//! │  │ cx.open_window()   │  ┌─────────────────────────────────┐  │    │
//! │  │      │        │────▶│  │ OnceLock<Mutex<WindowManager>> │  │    │
//! │  │      ▼        │     │  │                                 │  │    │
//! │  │ register_     │     │  │ windows: HashMap<WindowRole,id> │  │    │
//! │  │   main_window │     │  │                                 │  │    │
//! │  │              │     │  │ • MainWindow -> id               │  │    │
//! │  └──────────────┘     │  │ • (future roles...)              │  │    │
//! │                       │  └─────────────────────────────────┘  │    │
//! │  ┌──────────────┐     │                                        │    │
//! │  │ window_      │     │  Public API:                          │    │
//! │  │ resize.rs    │────▶│  • register_window(role, id)          │    │
//! │  │              │     │  • get_window(role) -> Option<id>     │    │
//! │  │ get_main_    │◀────│  • get_main_window() -> Option<id>    │    │
//! │  │   window()   │     │  • find_main_window_by_size()         │    │
//! │  └──────────────┘     └───────────────────────────────────────┘    │
//! │                                                                      │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//!
//! # Thread Safety
//!
//! The module uses `OnceLock<Mutex<WindowManager>>` for thread-safe global access:
//! - `OnceLock` ensures one-time initialization (like lazy_static but in std)
//! - `Mutex` protects concurrent access to the HashMap
//! - All public functions handle locking internally
//!
//! # Platform Support
//!
//! This module is macOS-specific. On other platforms, all functions are no-ops
//! that return None or do nothing.

#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::NSRect;
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

#[cfg(target_os = "macos")]
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "macos")]
use crate::logging;

/// Identifies the role/purpose of a window in the application.
///
/// This enum allows us to track multiple windows by their purpose,
/// making it easy to retrieve the correct window when needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowRole {
    /// The main Script Kit launcher window
    MainWindow,
    // Future roles can be added here:
    // SettingsWindow,
    // PreviewWindow,
    // etc.
}

/// A thread-safe wrapper for NSWindow ID pointers.
///
/// # Safety
///
/// This wrapper implements `Send` and `Sync` because:
/// 1. NSWindow IDs are stable identifiers that don't change
/// 2. macOS allows accessing window metadata from any thread
/// 3. Actual window mutations must still occur on the main thread
///    (enforced by the caller, not this module)
///
/// The pointer is stored as a raw address to avoid lifetime issues.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct WindowId(usize);

#[cfg(target_os = "macos")]
impl WindowId {
    /// Create a new WindowId from a native window pointer
    fn from_id(window: id) -> Self {
        Self(window as usize)
    }

    /// Convert back to a native window pointer
    fn to_id(self) -> id {
        self.0 as id
    }
}

// Safety: The window ID is just a numeric identifier. Accessing window
// properties is safe from any thread on macOS. Mutations are done on
// the main thread by the caller.
#[cfg(target_os = "macos")]
unsafe impl Send for WindowId {}
#[cfg(target_os = "macos")]
unsafe impl Sync for WindowId {}

/// Thread-safe window registry.
///
/// Maintains a mapping from window roles to their native macOS window IDs.
/// Access this through the module-level functions, not directly.
#[cfg(target_os = "macos")]
struct WindowManager {
    /// Map of window roles to their native window IDs (wrapped for thread safety)
    windows: HashMap<WindowRole, WindowId>,
}

#[cfg(target_os = "macos")]
impl WindowManager {
    /// Create a new empty WindowManager
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    /// Register a window with a specific role
    fn register(&mut self, role: WindowRole, window_id: id) {
        logging::log(
            "WINDOW_MGR",
            &format!("Registering window: {:?} -> {:?}", role, window_id),
        );
        self.windows.insert(role, WindowId::from_id(window_id));
    }

    /// Get a window by role
    fn get(&self, role: WindowRole) -> Option<id> {
        self.windows.get(&role).map(|wid| wid.to_id())
    }

    /// Remove a window registration
    #[allow(dead_code)]
    fn unregister(&mut self, role: WindowRole) -> Option<id> {
        logging::log("WINDOW_MGR", &format!("Unregistering window: {:?}", role));
        self.windows.remove(&role).map(|wid| wid.to_id())
    }

    /// Check if a role is registered
    #[allow(dead_code)]
    fn is_registered(&self, role: WindowRole) -> bool {
        self.windows.contains_key(&role)
    }
}

/// Global singleton for the window manager
#[cfg(target_os = "macos")]
static WINDOW_MANAGER: OnceLock<Mutex<WindowManager>> = OnceLock::new();

/// Get or initialize the global WindowManager
#[cfg(target_os = "macos")]
fn get_manager() -> &'static Mutex<WindowManager> {
    WINDOW_MANAGER.get_or_init(|| Mutex::new(WindowManager::new()))
}

// ============================================================================
// Public API - macOS Implementation
// ============================================================================

/// Register a window with a specific role.
///
/// Call this after GPUI creates a window to track it by role.
/// Subsequent calls with the same role will overwrite the previous registration.
///
/// # Arguments
/// * `role` - The purpose/role of this window
/// * `window_id` - The native macOS window ID (NSWindow pointer)
///
#[cfg(target_os = "macos")]
pub fn register_window(role: WindowRole, window_id: id) {
    if let Ok(mut manager) = get_manager().lock() {
        manager.register(role, window_id);
    } else {
        logging::log("WINDOW_MGR", "ERROR: Failed to acquire lock for register");
    }
}

/// Get a window by its role.
///
/// # Arguments
/// * `role` - The role to look up
///
/// # Returns
/// The native window ID if registered, None otherwise
#[cfg(target_os = "macos")]
pub fn get_window(role: WindowRole) -> Option<id> {
    if let Ok(manager) = get_manager().lock() {
        manager.get(role)
    } else {
        logging::log("WINDOW_MGR", "ERROR: Failed to acquire lock for get");
        None
    }
}

/// Convenience function to get the main window.
///
/// # Returns
/// The main window's native ID if registered, None otherwise
#[cfg(target_os = "macos")]
pub fn get_main_window() -> Option<id> {
    get_window(WindowRole::MainWindow)
}

/// Find and register the main window by its expected size.
///
/// This function searches through NSApp's windows array and identifies
/// our main window by its characteristic size (750x~500 pixels).
/// This is necessary because tray icons and other system elements
/// create windows that appear before our main window in the array.
///
/// # Expected Window Size
/// - Width: ~750 pixels
/// - Height: ~400-600 pixels (varies based on content)
///
/// # Returns
/// `true` if the main window was found and registered, `false` otherwise
#[cfg(target_os = "macos")]
pub fn find_and_register_main_window() -> bool {
    // Expected main window dimensions (with tolerance)
    const EXPECTED_WIDTH: f64 = 750.0;
    const WIDTH_TOLERANCE: f64 = 50.0;
    const MIN_HEIGHT: f64 = 100.0;
    const MAX_HEIGHT: f64 = 800.0;

    unsafe {
        let app: id = NSApp();
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];

        logging::log(
            "WINDOW_MGR",
            &format!(
                "Searching for main window among {} windows (expecting ~{:.0}x400-600)",
                count, EXPECTED_WIDTH
            ),
        );

        for i in 0..count {
            let window: id = msg_send![windows, objectAtIndex:i];
            if window == nil {
                continue;
            }

            let frame: NSRect = msg_send![window, frame];
            let width = frame.size.width;
            let height = frame.size.height;

            logging::log(
                "WINDOW_MGR",
                &format!("  Window[{}]: {:.0}x{:.0}", i, width, height),
            );

            // Check if this looks like our main window
            let width_matches = (width - EXPECTED_WIDTH).abs() < WIDTH_TOLERANCE;
            let height_matches = (MIN_HEIGHT..=MAX_HEIGHT).contains(&height);

            if width_matches && height_matches {
                logging::log(
                    "WINDOW_MGR",
                    &format!(
                        "Found main window at index {}: {:.0}x{:.0}",
                        i, width, height
                    ),
                );
                register_window(WindowRole::MainWindow, window);
                return true;
            }
        }

        logging::log(
            "WINDOW_MGR",
            "WARNING: Could not find main window by size. No window matched expected dimensions.",
        );
        false
    }
}

/// Unregister a window by role.
///
/// # Arguments
/// * `role` - The role to unregister
///
/// # Returns
/// The previously registered window ID, if any
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn unregister_window(role: WindowRole) -> Option<id> {
    if let Ok(mut manager) = get_manager().lock() {
        manager.unregister(role)
    } else {
        logging::log("WINDOW_MGR", "ERROR: Failed to acquire lock for unregister");
        None
    }
}

/// Check if a window role is currently registered.
///
/// # Arguments
/// * `role` - The role to check
///
/// # Returns
/// `true` if a window is registered for this role
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn is_window_registered(role: WindowRole) -> bool {
    if let Ok(manager) = get_manager().lock() {
        manager.is_registered(role)
    } else {
        false
    }
}

// ============================================================================
// Public API - Non-macOS Stubs
// ============================================================================

/// Non-macOS stub: register_window is a no-op
#[cfg(not(target_os = "macos"))]
pub fn register_window(_role: WindowRole, _window_id: *mut std::ffi::c_void) {
    // No-op on non-macOS platforms
}

/// Non-macOS stub: get_window always returns None
#[cfg(not(target_os = "macos"))]
pub fn get_window(_role: WindowRole) -> Option<*mut std::ffi::c_void> {
    None
}

/// Non-macOS stub: get_main_window always returns None
#[cfg(not(target_os = "macos"))]
pub fn get_main_window() -> Option<*mut std::ffi::c_void> {
    None
}

/// Non-macOS stub: find_and_register_main_window always returns false
#[cfg(not(target_os = "macos"))]
pub fn find_and_register_main_window() -> bool {
    false
}

/// Non-macOS stub: unregister_window always returns None
#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn unregister_window(_role: WindowRole) -> Option<*mut std::ffi::c_void> {
    None
}

/// Non-macOS stub: is_window_registered always returns false
#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn is_window_registered(_role: WindowRole) -> bool {
    false
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that WindowRole can be used as HashMap key
    #[test]
    fn test_window_role_hash_eq() {
        let role1 = WindowRole::MainWindow;
        let role2 = WindowRole::MainWindow;
        assert_eq!(role1, role2);

        // Verify it's Copy
        let role3 = role1;
        assert_eq!(role1, role3);
    }

    /// Test WindowRole Debug formatting
    #[test]
    fn test_window_role_debug() {
        let role = WindowRole::MainWindow;
        let debug_str = format!("{:?}", role);
        assert!(debug_str.contains("MainWindow"));
    }

    // macOS-specific tests
    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::super::*;

        /// Test WindowId wrapper
        #[test]
        fn test_window_id_wrapper() {
            let ptr_value: usize = 0x12345678;
            let mock_id = ptr_value as id;

            let window_id = WindowId::from_id(mock_id);
            let recovered = window_id.to_id();

            assert_eq!(recovered as usize, ptr_value);
        }

        /// Test basic registration and retrieval
        /// Note: Uses a mock pointer since we can't create real NSWindow in tests
        #[test]
        fn test_register_and_get_window() {
            // Create a mock window ID (don't actually use this pointer!)
            let mock_id: id = 0x12345678 as id;

            // Register the window
            register_window(WindowRole::MainWindow, mock_id);

            // Retrieve it
            let retrieved = get_window(WindowRole::MainWindow);
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap(), mock_id);
        }

        /// Test get_main_window convenience function
        #[test]
        fn test_get_main_window_convenience() {
            let mock_id: id = 0x87654321 as id;
            register_window(WindowRole::MainWindow, mock_id);

            let retrieved = get_main_window();
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap(), mock_id);
        }

        /// Test is_window_registered
        #[test]
        fn test_is_window_registered() {
            let mock_id: id = 0xABCDEF00 as id;

            // Register it
            register_window(WindowRole::MainWindow, mock_id);

            // Should be registered now
            assert!(is_window_registered(WindowRole::MainWindow));
        }

        /// Test that registration overwrites previous value
        #[test]
        fn test_registration_overwrites() {
            let first_id: id = 0x11111111 as id;
            let second_id: id = 0x22222222 as id;

            register_window(WindowRole::MainWindow, first_id);
            assert_eq!(get_window(WindowRole::MainWindow), Some(first_id));

            register_window(WindowRole::MainWindow, second_id);
            assert_eq!(get_window(WindowRole::MainWindow), Some(second_id));
        }

        /// Test WindowManager internal struct
        #[test]
        fn test_window_manager_struct() {
            let mut manager = WindowManager::new();

            let mock_id: id = 0x33333333 as id;

            // Initially empty
            assert!(!manager.is_registered(WindowRole::MainWindow));
            assert!(manager.get(WindowRole::MainWindow).is_none());

            // Register
            manager.register(WindowRole::MainWindow, mock_id);
            assert!(manager.is_registered(WindowRole::MainWindow));
            assert_eq!(manager.get(WindowRole::MainWindow), Some(mock_id));

            // Unregister
            let removed = manager.unregister(WindowRole::MainWindow);
            assert_eq!(removed, Some(mock_id));
            assert!(!manager.is_registered(WindowRole::MainWindow));
        }
    }

    // Non-macOS tests
    #[cfg(not(target_os = "macos"))]
    mod non_macos_tests {
        use super::super::*;

        #[test]
        fn test_stubs_return_none() {
            // All stub functions should return None or false
            assert!(get_window(WindowRole::MainWindow).is_none());
            assert!(get_main_window().is_none());
            assert!(!find_and_register_main_window());
            assert!(!is_window_registered(WindowRole::MainWindow));
            assert!(unregister_window(WindowRole::MainWindow).is_none());
        }
    }
}
