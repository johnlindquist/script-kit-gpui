//! Menu Action Executor module using macOS Accessibility APIs
//!
//! This module provides functionality to execute menu bar actions on applications.
//! It navigates the AX hierarchy (App -> MenuBar -> MenuBarItem -> Menu -> MenuItem)
//! and performs the AXPress action on the target menu item.
//!
//! ## Architecture
//!
//! The execution flow:
//! 1. Verify the target app is frontmost (required for menu access)
//! 2. Navigate to the AXMenuBar of the application
//! 3. Find each menu item in the path by title
//! 4. Open intermediate menus (AXPress on MenuBarItems/MenuItems with submenus)
//! 5. Execute the final action (AXPress on the target MenuItem)
//!
//! ## Permissions
//!
//! Requires Accessibility permission in System Preferences > Privacy & Security > Accessibility
//!
//! ## Usage
//!
//! ```ignore
//! use script_kit_gpui::menu_executor::execute_menu_action;
//!
//! // Execute "File" -> "New Window" in Safari
//! execute_menu_action("com.apple.Safari", &["File", "New Window"])?;
//! ```

#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use anyhow::{bail, Context, Result};
use std::ffi::c_void;
use thiserror::Error;
use tracing::{debug, instrument, warn};

// Import shared types from window_control and menu_bar
use crate::window_control::has_accessibility_permission;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during menu action execution
#[derive(Error, Debug)]
pub enum MenuExecutorError {
    /// The target menu item is disabled and cannot be clicked
    #[error("Menu item at path {path:?} is disabled")]
    MenuItemDisabled { path: Vec<String> },

    /// The menu item was not found in the expected location
    #[error("Menu item {path:?} not found in {searched_in}")]
    MenuItemNotFound {
        path: Vec<String>,
        searched_in: String,
    },

    /// The application is not frontmost (menu bar not accessible)
    #[error("Application {bundle_id} is not frontmost - cannot access menu bar")]
    AppNotFrontmost { bundle_id: String },

    /// The menu structure has changed since the cached data
    #[error("Menu structure changed - expected path {expected_path:?}: {reason}")]
    MenuStructureChanged {
        expected_path: Vec<String>,
        reason: String,
    },

    /// Accessibility permission not granted
    #[error("Accessibility permission required for menu execution")]
    AccessibilityPermissionDenied,

    /// Failed to perform the AXPress action
    #[error("Failed to perform AXPress on menu item: {0}")]
    ActionFailed(String),
}

// ============================================================================
// CoreFoundation FFI bindings
// ============================================================================

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

// ============================================================================
// ApplicationServices (Accessibility) FFI bindings
// ============================================================================

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> i32;
}

// AXError codes
const kAXErrorSuccess: i32 = 0;
const kAXErrorAPIDisabled: i32 = -25211;
const kAXErrorNoValue: i32 = -25212;
const kAXErrorActionUnsupported: i32 = -25215;
const kAXErrorCannotComplete: i32 = -25204;

type AXUIElementRef = *const c_void;
type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFArrayRef = *const c_void;

// ============================================================================
// CoreFoundation String FFI bindings
// ============================================================================

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(
        alloc: *const c_void,
        c_str: *const i8,
        encoding: u32,
    ) -> CFStringRef;
    fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut i8,
        buffer_size: i64,
        encoding: u32,
    ) -> bool;
    fn CFStringGetLength(string: CFStringRef) -> i64;
    fn CFArrayGetCount(array: CFArrayRef) -> i64;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, index: i64) -> CFTypeRef;
    fn CFGetTypeID(cf: CFTypeRef) -> u64;
    fn CFStringGetTypeID() -> u64;
    fn CFBooleanGetValue(boolean: CFTypeRef) -> bool;
    fn CFBooleanGetTypeID() -> u64;
}

const kCFStringEncodingUTF8: u32 = 0x08000100;

// ============================================================================
// AX Attribute Constants
// ============================================================================

const AX_MENU_BAR: &str = "AXMenuBar";
const AX_CHILDREN: &str = "AXChildren";
const AX_TITLE: &str = "AXTitle";
const AX_ENABLED: &str = "AXEnabled";
const AX_ROLE: &str = "AXRole";
const AX_PRESS: &str = "AXPress";

// AX Role values for reference
const _AX_ROLE_MENU_BAR_ITEM: &str = "AXMenuBarItem";
const _AX_ROLE_MENU_ITEM: &str = "AXMenuItem";
const _AX_ROLE_MENU: &str = "AXMenu";

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a CFString from a Rust string
fn create_cf_string(s: &str) -> CFStringRef {
    unsafe {
        let c_str = std::ffi::CString::new(s).unwrap();
        CFStringCreateWithCString(std::ptr::null(), c_str.as_ptr(), kCFStringEncodingUTF8)
    }
}

/// Convert a CFString to a Rust String
fn cf_string_to_string(cf_string: CFStringRef) -> Option<String> {
    if cf_string.is_null() {
        return None;
    }

    unsafe {
        let length = CFStringGetLength(cf_string);
        if length <= 0 {
            return Some(String::new());
        }

        // Allocate buffer with extra space for UTF-8 expansion
        let buffer_size = (length * 4 + 1) as usize;
        let mut buffer: Vec<i8> = vec![0; buffer_size];

        if CFStringGetCString(
            cf_string,
            buffer.as_mut_ptr(),
            buffer_size as i64,
            kCFStringEncodingUTF8,
        ) {
            let c_str = std::ffi::CStr::from_ptr(buffer.as_ptr());
            c_str.to_str().ok().map(|s| s.to_string())
        } else {
            None
        }
    }
}

/// Release a CoreFoundation object
fn cf_release(cf: CFTypeRef) {
    if !cf.is_null() {
        unsafe {
            CFRelease(cf);
        }
    }
}

/// Get an attribute value from an AXUIElement
fn get_ax_attribute(element: AXUIElementRef, attribute: &str) -> Result<CFTypeRef> {
    let attr_str = create_cf_string(attribute);
    let mut value: CFTypeRef = std::ptr::null();

    let result =
        unsafe { AXUIElementCopyAttributeValue(element, attr_str, &mut value as *mut CFTypeRef) };

    cf_release(attr_str);

    match result {
        kAXErrorSuccess => Ok(value),
        kAXErrorAPIDisabled => bail!("Accessibility API is disabled"),
        kAXErrorNoValue => bail!("No value for attribute: {}", attribute),
        _ => bail!("Failed to get attribute {}: error {}", attribute, result),
    }
}

/// Get a string attribute from an AXUIElement
fn get_ax_string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
    match get_ax_attribute(element, attribute) {
        Ok(value) => {
            let type_id = unsafe { CFGetTypeID(value) };
            let string_type_id = unsafe { CFStringGetTypeID() };

            let result = if type_id == string_type_id {
                cf_string_to_string(value as CFStringRef)
            } else {
                None
            };

            cf_release(value);
            result
        }
        Err(_) => None,
    }
}

/// Get a boolean attribute from an AXUIElement
fn get_ax_bool_attribute(element: AXUIElementRef, attribute: &str) -> Option<bool> {
    match get_ax_attribute(element, attribute) {
        Ok(value) => {
            let type_id = unsafe { CFGetTypeID(value) };
            let bool_type_id = unsafe { CFBooleanGetTypeID() };

            let result = if type_id == bool_type_id {
                Some(unsafe { CFBooleanGetValue(value) })
            } else {
                None
            };

            cf_release(value);
            result
        }
        Err(_) => None,
    }
}

/// Perform an action on an AXUIElement
fn perform_ax_action(element: AXUIElementRef, action: &str) -> Result<()> {
    let action_str = create_cf_string(action);

    let result = unsafe { AXUIElementPerformAction(element, action_str) };

    cf_release(action_str);

    match result {
        kAXErrorSuccess => Ok(()),
        kAXErrorAPIDisabled => bail!("Accessibility API is disabled"),
        kAXErrorActionUnsupported => bail!("Action {} is not supported", action),
        kAXErrorCannotComplete => bail!(
            "Cannot complete action {} - element may be disabled",
            action
        ),
        _ => bail!("Failed to perform action {}: error {}", action, result),
    }
}

/// Get the children array from an AXUIElement
fn get_ax_children(element: AXUIElementRef) -> Result<(CFArrayRef, i64)> {
    let children_value = get_ax_attribute(element, AX_CHILDREN)?;
    let count = unsafe { CFArrayGetCount(children_value as CFArrayRef) };
    Ok((children_value as CFArrayRef, count))
}

/// Get the frontmost application's PID and bundle ID
fn get_frontmost_app_info() -> Result<(i32, String)> {
    unsafe {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};

        let workspace_class = Class::get("NSWorkspace").context("Failed to get NSWorkspace")?;
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let frontmost_app: *mut Object = msg_send![workspace, frontmostApplication];

        if frontmost_app.is_null() {
            bail!("No frontmost application found");
        }

        let pid: i32 = msg_send![frontmost_app, processIdentifier];

        if pid <= 0 {
            bail!("Invalid process identifier for frontmost application");
        }

        // Get bundle identifier
        let bundle_id: *mut Object = msg_send![frontmost_app, bundleIdentifier];
        let bundle_id_str = if !bundle_id.is_null() {
            let utf8: *const i8 = msg_send![bundle_id, UTF8String];
            if !utf8.is_null() {
                std::ffi::CStr::from_ptr(utf8)
                    .to_str()
                    .unwrap_or("unknown")
                    .to_string()
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        };

        Ok((pid, bundle_id_str))
    }
}

/// Find a menu item by title in a list of AX children
fn find_menu_item_by_title(
    children: CFArrayRef,
    count: i64,
    title: &str,
) -> Option<AXUIElementRef> {
    for i in 0..count {
        let child = unsafe { CFArrayGetValueAtIndex(children, i) };
        if child.is_null() {
            continue;
        }

        let child_title = get_ax_string_attribute(child as AXUIElementRef, AX_TITLE);
        if let Some(ref t) = child_title {
            if t == title {
                return Some(child as AXUIElementRef);
            }
        }
    }
    None
}

/// Navigate through AX hierarchy to find and open a submenu
fn open_menu_at_element(element: AXUIElementRef) -> Result<AXUIElementRef> {
    // First, press the element to open it (works for MenuBarItem and MenuItem with submenu)
    perform_ax_action(element, AX_PRESS)?;

    // Small delay to let the menu open
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Get the children - one should be the AXMenu
    let (children, count) = get_ax_children(element)?;

    for i in 0..count {
        let child = unsafe { CFArrayGetValueAtIndex(children, i) };
        if child.is_null() {
            continue;
        }

        let role = get_ax_string_attribute(child as AXUIElementRef, AX_ROLE);
        if let Some(ref r) = role {
            if r == "AXMenu" {
                cf_release(children as CFTypeRef);
                return Ok(child as AXUIElementRef);
            }
        }
    }

    cf_release(children as CFTypeRef);
    bail!("No AXMenu child found after opening element")
}

// ============================================================================
// Validation Functions
// ============================================================================

/// Validate that a menu path is not empty
pub fn validate_menu_path(path: &[String]) -> Result<()> {
    if path.is_empty() {
        bail!("Menu path cannot be empty");
    }
    Ok(())
}

// ============================================================================
// Public API
// ============================================================================

/// Execute a menu action on an application by navigating the menu path.
///
/// This function navigates the accessibility hierarchy to find and click
/// the specified menu item. The application must be frontmost for this to work.
///
/// # Arguments
/// * `bundle_id` - The bundle identifier of the target application (e.g., "com.apple.Safari")
/// * `menu_path` - The path to the menu item (e.g., ["File", "New Window"])
///
/// # Returns
/// `Ok(())` if the action was executed successfully.
///
/// # Errors
/// Returns `MenuExecutorError` variants for specific failure modes:
/// - `AppNotFrontmost` - The target app is not the frontmost application
/// - `MenuItemNotFound` - The menu item path doesn't exist
/// - `MenuItemDisabled` - The menu item exists but is disabled
/// - `MenuStructureChanged` - The menu structure is different than expected
/// - `AccessibilityPermissionDenied` - Accessibility permission not granted
/// - `ActionFailed` - The AXPress action failed
///
/// # Example
/// ```ignore
/// use script_kit_gpui::menu_executor::execute_menu_action;
///
/// // Execute "File" -> "New Window" in Finder
/// execute_menu_action("com.apple.finder", &["File", "New Finder Window"])?;
/// ```
#[instrument(skip(menu_path), fields(bundle_id = %bundle_id, path = ?menu_path))]
pub fn execute_menu_action(bundle_id: &str, menu_path: &[String]) -> Result<()> {
    // Validate inputs
    validate_menu_path(menu_path)?;

    // Check accessibility permission
    if !has_accessibility_permission() {
        return Err(MenuExecutorError::AccessibilityPermissionDenied.into());
    }

    // Verify the target app is frontmost
    let (pid, frontmost_bundle_id) = get_frontmost_app_info()?;
    if frontmost_bundle_id != bundle_id {
        return Err(MenuExecutorError::AppNotFrontmost {
            bundle_id: bundle_id.to_string(),
        }
        .into());
    }

    debug!(
        pid,
        frontmost_bundle_id, "Executing menu action on frontmost app"
    );

    // Create AXUIElement for the application
    let ax_app = unsafe { AXUIElementCreateApplication(pid) };
    if ax_app.is_null() {
        return Err(MenuExecutorError::MenuStructureChanged {
            expected_path: menu_path.to_vec(),
            reason: format!("Failed to create AXUIElement for app (pid: {})", pid),
        }
        .into());
    }

    // Get the menu bar
    let menu_bar = match get_ax_attribute(ax_app, AX_MENU_BAR) {
        Ok(mb) => mb,
        Err(e) => {
            cf_release(ax_app);
            return Err(MenuExecutorError::MenuStructureChanged {
                expected_path: menu_path.to_vec(),
                reason: format!("Failed to get menu bar: {}", e),
            }
            .into());
        }
    };

    if menu_bar.is_null() {
        cf_release(ax_app);
        return Err(MenuExecutorError::MenuStructureChanged {
            expected_path: menu_path.to_vec(),
            reason: "Application has no menu bar".to_string(),
        }
        .into());
    }

    // Navigate the menu path
    let result = navigate_and_execute_menu_path(menu_bar as AXUIElementRef, menu_path);

    // Cleanup
    cf_release(menu_bar);
    cf_release(ax_app);

    result
}

/// Internal function to navigate the menu path and execute the action
fn navigate_and_execute_menu_path(menu_bar: AXUIElementRef, menu_path: &[String]) -> Result<()> {
    let mut current_menu_container = menu_bar;
    let mut path_so_far: Vec<String> = Vec::new();

    for (i, menu_title) in menu_path.iter().enumerate() {
        let is_last = i == menu_path.len() - 1;
        path_so_far.push(menu_title.clone());

        // Get children of current container
        let (children, count) = get_ax_children(current_menu_container).map_err(|e| {
            MenuExecutorError::MenuStructureChanged {
                expected_path: path_so_far.clone(),
                reason: format!("Failed to get children: {}", e),
            }
        })?;

        // Find the menu item by title
        let menu_item = find_menu_item_by_title(children, count, menu_title);

        if menu_item.is_none() {
            cf_release(children as CFTypeRef);
            return Err(MenuExecutorError::MenuItemNotFound {
                path: path_so_far,
                searched_in: format!("menu level {}", i),
            }
            .into());
        }

        let menu_item = menu_item.unwrap();

        // Check if enabled (only matters for the final item)
        if is_last {
            let enabled = get_ax_bool_attribute(menu_item, AX_ENABLED).unwrap_or(true);
            if !enabled {
                cf_release(children as CFTypeRef);
                return Err(MenuExecutorError::MenuItemDisabled { path: path_so_far }.into());
            }

            // Execute the action
            debug!(menu_title, "Pressing final menu item");
            perform_ax_action(menu_item, AX_PRESS).map_err(|e| {
                MenuExecutorError::ActionFailed(format!(
                    "Failed to press menu item '{}': {}",
                    menu_title, e
                ))
            })?;

            cf_release(children as CFTypeRef);
            return Ok(());
        }

        // Not the last item - need to open the submenu
        debug!(menu_title, "Opening intermediate menu");

        // We need to release children before opening menu (menu opening may change hierarchy)
        cf_release(children as CFTypeRef);

        // Open the menu to get to its children
        let submenu = open_menu_at_element(menu_item).map_err(|e| {
            MenuExecutorError::MenuStructureChanged {
                expected_path: path_so_far.clone(),
                reason: format!("Failed to open submenu at '{}': {}", menu_title, e),
            }
        })?;

        current_menu_container = submenu;
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "menu_executor_tests.rs"]
mod tests;
