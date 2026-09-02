//! Frontmost Application Tracker
//!
//! Tracks the "last real application" that was active before Script Kit.
//! This module provides a global, always-updated view of what app the user
//! was working in, which is useful for:
//!
//! - **Menu Bar Actions**: Get menu items from the app the user was in
//! - **Window Tiling**: Tile/move windows of the previous app
//! - **Context Actions**: Any action that should target "the app I was just using"
//!
//! ## Architecture
//!
//! A background observer watches for `NSWorkspaceDidActivateApplicationNotification`.
//! When an app activates:
//! - If it's NOT Script Kit → update the tracked "last real app"
//! - If it IS Script Kit → ignore (keep tracking the previous app)
//!
//! This means when Script Kit opens, we already know which app was active,
//! with no race conditions or timing issues.
//!
//! ## Usage
//!
//! ```ignore
//! use crate::frontmost_app_tracker::{start_tracking, get_last_real_app, get_cached_menu_items};
//!
//! // Start tracking (call once at app startup)
//! start_tracking();
//!
//! // Get the last real app info
//! if let Some(app) = get_last_real_app() {
//!     println!("Last app: {} ({})", app.name, app.bundle_id);
//! }
//!
//! // Get cached menu items (pre-fetched in background)
//! let menu_items = get_cached_menu_items();
//! ```

// --- merged from part_000.rs ---
use crate::logging;
use crate::menu_bar::{get_menu_bar_for_pid, MenuBarItem};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;
/// Information about a tracked application
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedApp {
    /// Process ID
    pub pid: i32,
    /// Bundle identifier (e.g., "com.google.Chrome")
    pub bundle_id: String,
    /// Localized display name (e.g., "Google Chrome")
    pub name: String,
    /// Cached focused window title for the app, if available.
    pub window_title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedMenuStatus {
    Ready,
    Fetching,
    NoTrackedApp,
    NoCache,
    StaleCacheHidden,
}

impl CachedMenuStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Fetching => "fetching",
            Self::NoTrackedApp => "noTrackedApp",
            Self::NoCache => "noCache",
            Self::StaleCacheHidden => "staleCacheHidden",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedMenuSnapshot {
    pub app: Option<TrackedApp>,
    pub items: Vec<MenuBarItem>,
    pub status: CachedMenuStatus,
    pub is_fetching: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MenuCacheIdentity {
    pid: i32,
    bundle_id: String,
}

fn tracked_app_matches(app: Option<&TrackedApp>, identity: &MenuCacheIdentity) -> bool {
    app.map(|app| app.pid == identity.pid && app.bundle_id == identity.bundle_id)
        .unwrap_or(false)
}

fn fetching_request_matches(
    state: &TrackerState,
    identity: &MenuCacheIdentity,
    generation: u64,
) -> bool {
    state.fetching_menu_identity.as_ref() == Some(identity)
        && state.fetching_menu_generation == generation
}

fn advance_fetch_generation(state: &mut TrackerState) -> u64 {
    state.fetching_menu_generation = state
        .fetching_menu_generation
        .checked_add(1)
        .expect("menu fetch generation exhausted");
    state.fetching_menu_generation
}
/// Global state for the frontmost app tracker
#[derive(Default)]
struct TrackerState {
    /// The last "real" application (not Script Kit)
    last_real_app: Option<TrackedApp>,
    /// Cached menu bar items for the last real app
    cached_menu_items: Vec<MenuBarItem>,
    /// Identity that owns the current cached menu items, if any.
    cached_menu_identity: Option<MenuCacheIdentity>,
    /// Exact app identity currently being fetched, if any.
    fetching_menu_identity: Option<MenuCacheIdentity>,
    /// Monotonic request generation for the active fetch.
    fetching_menu_generation: u64,
}
/// Global tracker state, protected by RwLock for concurrent access
static TRACKER_STATE: LazyLock<RwLock<TrackerState>> =
    LazyLock::new(|| RwLock::new(TrackerState::default()));

#[derive(Default)]
struct MenuChangeSubscribers {
    next_id: u64,
    senders: std::collections::BTreeMap<u64, async_channel::Sender<()>>,
}

static MENU_CHANGE_SUBSCRIBERS: LazyLock<parking_lot::Mutex<MenuChangeSubscribers>> =
    LazyLock::new(|| parking_lot::Mutex::new(MenuChangeSubscribers::default()));

/// One Main entity's bounded wake registration. Dropping an old subscription
/// removes only its own monotonically unique registration, never a replacement.
pub struct MenuChangeSubscription {
    id: u64,
    receiver: async_channel::Receiver<()>,
}

impl MenuChangeSubscription {
    pub async fn changed(&self) -> Result<(), async_channel::RecvError> {
        self.receiver.recv().await
    }
}

impl Drop for MenuChangeSubscription {
    fn drop(&mut self) {
        MENU_CHANGE_SUBSCRIBERS.lock().senders.remove(&self.id);
    }
}

/// Subscribe to already-running tracker producers; never starts native tracking.
pub fn subscribe_menu_changes() -> MenuChangeSubscription {
    let (sender, receiver) = async_channel::bounded(1);
    let mut subscribers = MENU_CHANGE_SUBSCRIBERS.lock();
    subscribers.next_id = subscribers
        .next_id
        .checked_add(1)
        .expect("menu subscription generation exhausted");
    let id = subscribers.next_id;
    subscribers.senders.insert(id, sender);
    MenuChangeSubscription { id, receiver }
}

fn notify_menu_changed() {
    MENU_CHANGE_SUBSCRIBERS
        .lock()
        .senders
        .retain(|_, sender| match sender.try_send(()) {
            Ok(()) | Err(async_channel::TrySendError::Full(())) => true,
            Err(async_channel::TrySendError::Closed(())) => false,
        });
}

fn publish_tracked_app(tracked: TrackedApp) {
    let mut state = TRACKER_STATE.write();
    if state.last_real_app.as_ref() == Some(&tracked) {
        return;
    }
    let identity_changed = state.last_real_app.as_ref().is_none_or(|previous| {
        previous.pid != tracked.pid || previous.bundle_id != tracked.bundle_id
    });
    state.last_real_app = Some(tracked);
    if identity_changed {
        state.cached_menu_items.clear();
        state.cached_menu_identity = None;
        state.fetching_menu_identity = None;
    }
    drop(state);
    notify_menu_changed();
}
/// Whether tracking has been started
static TRACKING_STARTED: AtomicBool = AtomicBool::new(false);
/// Our own bundle ID to filter out
const SCRIPT_KIT_BUNDLE_ID: &str = "dev.scriptkit.scriptkit";
#[cfg(target_os = "macos")]
type AXUIElementRef = *const std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFTypeRef = *const std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFStringRef = *const std::ffi::c_void;
#[cfg(target_os = "macos")]
const K_AX_ERROR_SUCCESS: i32 = 0;
#[cfg(target_os = "macos")]
const K_AX_ERROR_NO_VALUE: i32 = -25212;
#[cfg(target_os = "macos")]
const K_CFSTRING_ENCODING_UTF8: u32 = 0x08000100;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const std::ffi::c_void);
    fn CFStringCreateWithCString(
        alloc: *const std::ffi::c_void,
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
    fn CFGetTypeID(cf: CFTypeRef) -> u64;
    fn CFStringGetTypeID() -> u64;
}
#[cfg(target_os = "macos")]
fn require_objc_class(class_name: &str) -> Option<&'static objc::runtime::Class> {
    match objc::runtime::Class::get(class_name) {
        Some(class) => Some(class),
        None => {
            logging::log(
                "APP",
                &format!(
                    "frontmost_tracker_class_missing: class='{}' operation=require_objc_class",
                    class_name
                ),
            );
            None
        }
    }
}
fn make_objc_cstring(input: &str) -> Result<std::ffi::CString, std::ffi::NulError> {
    std::ffi::CString::new(input)
}

fn normalize_window_title(title: Option<String>) -> Option<String> {
    title.and_then(|title| {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(target_os = "macos")]
fn try_create_cf_string(input: &str) -> Option<CFStringRef> {
    let c_str = match make_objc_cstring(input) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(
                target: "script_kit::frontmost_app_tracker",
                %error,
                attribute = input,
                "frontmost app tracker failed to build CFString"
            );
            return None;
        }
    };

    // SAFETY: `c_str` is a valid NUL-terminated UTF-8 string for the duration
    // of the call and CoreFoundation copies the bytes into a new CFString.
    let cf_string = unsafe {
        CFStringCreateWithCString(std::ptr::null(), c_str.as_ptr(), K_CFSTRING_ENCODING_UTF8)
    };
    if cf_string.is_null() {
        tracing::warn!(
            target: "script_kit::frontmost_app_tracker",
            attribute = input,
            "frontmost app tracker received null CFString"
        );
        None
    } else {
        Some(cf_string)
    }
}

#[cfg(target_os = "macos")]
fn cf_release(cf: *const std::ffi::c_void) {
    if !cf.is_null() {
        // SAFETY: The caller only passes owned CoreFoundation references created
        // or returned with +1 retain semantics in this module.
        unsafe { CFRelease(cf) };
    }
}

#[cfg(target_os = "macos")]
fn cf_string_to_string(cf_string: CFStringRef) -> Option<String> {
    if cf_string.is_null() {
        return None;
    }

    // SAFETY: `cf_string` is a valid CFStringRef owned by the caller.
    let length = unsafe { CFStringGetLength(cf_string) };
    if length <= 0 {
        return Some(String::new());
    }

    let buffer_size = (length * 4 + 1) as usize;
    let mut buffer = vec![0_i8; buffer_size];

    // SAFETY: `cf_string` is a valid CFStringRef; `buffer` is a writable,
    // properly sized output buffer for UTF-8 conversion.
    let success = unsafe {
        CFStringGetCString(
            cf_string,
            buffer.as_mut_ptr(),
            buffer_size as i64,
            K_CFSTRING_ENCODING_UTF8,
        )
    };

    if !success {
        return None;
    }

    // SAFETY: `buffer` was written by CFStringGetCString with a trailing NUL.
    unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .ok()
        .map(ToOwned::to_owned)
}

#[cfg(target_os = "macos")]
fn get_ax_string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
    let attr = try_create_cf_string(attribute)?;
    let mut value: CFTypeRef = std::ptr::null();

    // SAFETY: `element` is an AXUIElementRef owned by the caller, `attr` is a
    // valid CFStringRef, and `value` is a stack out-pointer for the API result.
    let result = unsafe { AXUIElementCopyAttributeValue(element, attr, &mut value) };
    cf_release(attr);

    if result == K_AX_ERROR_NO_VALUE || value.is_null() {
        return None;
    }
    if result != K_AX_ERROR_SUCCESS {
        tracing::debug!(
            target: "script_kit::frontmost_app_tracker",
            attribute,
            code = result,
            "frontmost app tracker AX attribute unavailable"
        );
        return None;
    }

    // SAFETY: `value` is returned by AXUIElementCopyAttributeValue and valid for
    // CoreFoundation type inspection until released below.
    let value_type_id = unsafe { CFGetTypeID(value) };
    // SAFETY: CFStringGetTypeID is a pure CoreFoundation query.
    let string_type_id = unsafe { CFStringGetTypeID() };
    let result = if value_type_id == string_type_id {
        cf_string_to_string(value as CFStringRef)
    } else {
        None
    };
    cf_release(value);
    result
}

#[cfg(target_os = "macos")]
fn capture_window_title_for_pid(pid: i32) -> Option<String> {
    if !crate::window_control::has_accessibility_permission() {
        tracing::debug!(
            target: "script_kit::frontmost_app_tracker",
            pid,
            "frontmost app tracker skipped focused window title capture without AX permission"
        );
        return None;
    }

    // SAFETY: AXUIElementCreateApplication returns an owned accessibility object
    // for the supplied PID when the app exists. The pointer is released below.
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return None;
    }

    let title = (|| {
        let attr = try_create_cf_string("AXFocusedWindow")?;
        let mut focused_window: CFTypeRef = std::ptr::null();
        // SAFETY: `app` and `attr` are valid CF objects and `focused_window` is
        // a stack out-pointer for AXUIElementCopyAttributeValue.
        let result = unsafe { AXUIElementCopyAttributeValue(app, attr, &mut focused_window) };
        cf_release(attr);

        if result != K_AX_ERROR_SUCCESS || focused_window.is_null() {
            if result != K_AX_ERROR_NO_VALUE {
                tracing::debug!(
                    target: "script_kit::frontmost_app_tracker",
                    pid,
                    code = result,
                    "frontmost app tracker focused window lookup unavailable"
                );
            }
            return None;
        }

        let title = get_ax_string_attribute(focused_window as AXUIElementRef, "AXTitle");
        cf_release(focused_window);
        title
    })();

    cf_release(app);
    normalize_window_title(title)
}

#[cfg(not(target_os = "macos"))]
fn capture_window_title_for_pid(_pid: i32) -> Option<String> {
    None
}
/// Start the background frontmost app tracker.
///
/// This should be called once at application startup. It sets up an
/// NSWorkspace observer to watch for application activation events.
///
/// Safe to call multiple times - subsequent calls are no-ops.
#[cfg(target_os = "macos")]
pub fn start_tracking() {
    if crate::runtime_policy::is_owned_evaluation() {
        return;
    }
    if TRACKING_STARTED.swap(true, Ordering::SeqCst) {
        // Already started
        return;
    }

    logging::log("APP", "Starting frontmost app tracker");

    // Capture initial state - get current menu bar owner
    capture_current_frontmost_app();

    // Set up NSWorkspace observer for app activation
    std::thread::spawn(|| {
        setup_workspace_observer();
    });
}
#[cfg(not(target_os = "macos"))]
pub fn start_tracking() {
    // No-op on non-macOS platforms
    logging::log(
        "APP",
        "Frontmost app tracking not available on this platform",
    );
}
/// Get the last "real" application that was active before Script Kit.
///
/// Returns `None` if no app has been tracked yet (e.g., Script Kit just launched).
pub fn get_last_real_app() -> Option<TrackedApp> {
    TRACKER_STATE.read().last_real_app.clone()
}

/// Get the bundle identifier for the last real app.
///
/// Returns `None` if no app has been tracked yet.
pub fn get_last_real_app_bundle_id() -> Option<String> {
    TRACKER_STATE
        .read()
        .last_real_app
        .as_ref()
        .map(|app| app.bundle_id.clone())
}
/// Get the cached menu bar items for the last real app.
///
/// These are pre-fetched in the background when the app becomes active,
/// so they're available immediately when Script Kit opens.
///
/// Returns an empty Vec if no menu items are cached.
pub fn get_cached_menu_items() -> Vec<MenuBarItem> {
    let state = TRACKER_STATE.read();
    let Some(tracked) = state.last_real_app.as_ref() else {
        return Vec::new();
    };

    let cache_matches = state
        .cached_menu_identity
        .as_ref()
        .map(|identity| identity.pid == tracked.pid && identity.bundle_id == tracked.bundle_id)
        .unwrap_or(false);

    if cache_matches {
        state.cached_menu_items.clone()
    } else {
        Vec::new()
    }
}

pub struct CachedMenuSummary {
    pub app: Option<TrackedApp>,
    pub item_count: usize,
    pub status: CachedMenuStatus,
}

pub fn get_cached_menu_summary() -> CachedMenuSummary {
    let state = TRACKER_STATE.read();
    let Some(tracked) = state.last_real_app.clone() else {
        return CachedMenuSummary {
            app: None,
            item_count: 0,
            status: CachedMenuStatus::NoTrackedApp,
        };
    };

    let tracked_identity = MenuCacheIdentity {
        pid: tracked.pid,
        bundle_id: tracked.bundle_id.clone(),
    };
    let cache_matches = state.cached_menu_identity.as_ref() == Some(&tracked_identity);
    let is_fetching = state.fetching_menu_identity.as_ref() == Some(&tracked_identity);

    let (item_count, status) = if cache_matches {
        (state.cached_menu_items.len(), CachedMenuStatus::Ready)
    } else if is_fetching {
        (0, CachedMenuStatus::Fetching)
    } else if state.cached_menu_identity.is_some() {
        (0, CachedMenuStatus::StaleCacheHidden)
    } else {
        (0, CachedMenuStatus::NoCache)
    };

    CachedMenuSummary {
        app: Some(tracked),
        item_count,
        status,
    }
}

pub fn get_cached_menu_snapshot() -> CachedMenuSnapshot {
    let state = TRACKER_STATE.read();
    let Some(tracked) = state.last_real_app.clone() else {
        return CachedMenuSnapshot {
            app: None,
            items: Vec::new(),
            status: CachedMenuStatus::NoTrackedApp,
            is_fetching: state.fetching_menu_identity.is_some(),
        };
    };

    let tracked_identity = MenuCacheIdentity {
        pid: tracked.pid,
        bundle_id: tracked.bundle_id.clone(),
    };
    let is_fetching = state.fetching_menu_identity.as_ref() == Some(&tracked_identity);
    let cache_matches = state.cached_menu_identity.as_ref() == Some(&tracked_identity);

    let (items, status) = if cache_matches {
        (state.cached_menu_items.clone(), CachedMenuStatus::Ready)
    } else if is_fetching {
        (Vec::new(), CachedMenuStatus::Fetching)
    } else if state.cached_menu_identity.is_some() {
        (Vec::new(), CachedMenuStatus::StaleCacheHidden)
    } else {
        (Vec::new(), CachedMenuStatus::NoCache)
    };

    CachedMenuSnapshot {
        app: Some(tracked),
        items,
        status,
        is_fetching,
    }
}

#[cfg(target_os = "macos")]
pub fn replace_cached_menu_items(pid: i32, bundle_id: &str, items: Vec<MenuBarItem>) {
    let identity = MenuCacheIdentity {
        pid,
        bundle_id: bundle_id.to_string(),
    };
    let mut state = TRACKER_STATE.write();
    if !tracked_app_matches(state.last_real_app.as_ref(), &identity) {
        logging::log(
            "APP",
            &format!(
                "Skipped cache replace for {} PID {} because the tracked app changed",
                bundle_id, pid
            ),
        );
        return;
    }

    let item_count = items.len();
    let changed = state.cached_menu_items != items
        || state.cached_menu_identity.as_ref() != Some(&identity)
        || state.fetching_menu_identity.as_ref() == Some(&identity);
    state.cached_menu_items = items;
    state.cached_menu_identity = Some(identity.clone());
    if state.fetching_menu_identity.as_ref() == Some(&identity) {
        state.fetching_menu_identity = None;
    }
    drop(state);
    if changed {
        notify_menu_changed();
    }

    logging::log(
        "APP",
        &format!(
            "Replaced cached menu items for {} PID {} with {} top-level items",
            bundle_id, pid, item_count
        ),
    );
}
/// Check if menu items are currently being fetched in the background.
#[allow(dead_code)] // Public API for future use
pub fn is_fetching_menu() -> bool {
    TRACKER_STATE.read().fetching_menu_identity.is_some()
}
/// Get the bundle ID currently being fetched, if any.
/// This is more informative than `is_fetching_menu()` for debugging.
#[allow(dead_code)] // Public API for future use
pub fn get_fetching_bundle_id() -> Option<String> {
    TRACKER_STATE
        .read()
        .fetching_menu_identity
        .as_ref()
        .map(|identity| identity.bundle_id.clone())
}
/// Capture the current frontmost app (used at startup and for manual refresh)
#[cfg(target_os = "macos")]
fn capture_current_frontmost_app() {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let workspace_class = match Class::get("NSWorkspace") {
            Some(c) => c,
            None => return,
        };

        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];

        // Use menuBarOwningApplication since Script Kit is LSUIElement
        let app: *mut Object = msg_send![workspace, menuBarOwningApplication];

        if app.is_null() {
            return;
        }

        let bundle_id = get_nsstring(msg_send![app, bundleIdentifier]);
        let name = get_nsstring(msg_send![app, localizedName]);
        let pid: i32 = msg_send![app, processIdentifier];

        if let Some(bundle_id) = bundle_id {
            // Don't track Script Kit itself
            if bundle_id == SCRIPT_KIT_BUNDLE_ID {
                return;
            }

            let tracked = TrackedApp {
                pid,
                bundle_id: bundle_id.clone(),
                name: name.unwrap_or_else(|| bundle_id.clone()),
                window_title: capture_window_title_for_pid(pid),
            };

            logging::log(
                "APP",
                &format!(
                    "Initial frontmost app: {} ({}) PID {}",
                    tracked.name, tracked.bundle_id, tracked.pid
                ),
            );

            publish_tracked_app(tracked.clone());

            // Fetch menu items in background
            fetch_menu_items_async(tracked.pid, tracked.bundle_id);
        }
    }
}
/// Set up the NSWorkspace notification observer
#[cfg(target_os = "macos")]
fn setup_workspace_observer() {
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{msg_send, sel, sel_impl};
    use std::os::raw::c_void;

    unsafe {
        // Create a custom class to receive notifications
        let Some(superclass) = require_objc_class("NSObject") else {
            return;
        };

        // Try to get or create the observer class
        // Fix: If the class already exists (e.g., from a previous call), fetch it
        // instead of returning early. This ensures the observer is always registered.
        let observer_class = if let Some(existing) = Class::get("ScriptKitAppObserver") {
            logging::log("APP", "Using existing ScriptKitAppObserver class");
            existing
        } else {
            // Class doesn't exist, create it
            let mut decl = match ClassDecl::new("ScriptKitAppObserver", superclass) {
                Some(d) => d,
                None => {
                    // This shouldn't happen if Class::get returned None, but handle it
                    logging::log("ERROR", "Failed to create ScriptKitAppObserver class");
                    return;
                }
            };

            // Add the notification handler method
            //
            // SAFETY: This callback is invoked from Objective-C on a background thread
            // (via NSRunLoop). We must:
            // 1. Use autoreleasepool to drain autoreleased objects created by msg_send!
            //    (e.g., NSStrings from stringWithUTF8String:). Without this, objects
            //    accumulate and leak since there's no enclosing pool on this thread.
            // 2. Use catch_unwind to prevent panics from unwinding across the FFI boundary,
            //    which is undefined behavior.
            extern "C" fn handle_app_activation(
                _this: &Object,
                _sel: Sel,
                notification: *mut Object,
            ) {
                // Catch any panic to prevent UB from unwinding across FFI boundary
                let _ = std::panic::catch_unwind(|| {
                    // Create an autorelease pool to drain autoreleased objects
                    // from msg_send! calls (e.g., NSStrings created in this callback)
                    objc::rc::autoreleasepool(|| unsafe {
                        handle_app_activation_inner(notification)
                    });
                });
            }

            /// Inner implementation of handle_app_activation.
            /// Separated to keep the extern "C" wrapper minimal and focused on FFI safety.
            ///
            /// # Safety
            /// - Must be called within an autoreleasepool
            /// - notification must be a valid Objective-C object pointer or null
            unsafe fn handle_app_activation_inner(notification: *mut Object) {
                if notification.is_null() {
                    return;
                }

                // Get userInfo dictionary
                let user_info: *mut Object = msg_send![notification, userInfo];
                if user_info.is_null() {
                    return;
                }

                // Get the NSRunningApplication from userInfo
                let key = objc_nsstring("NSWorkspaceApplicationKey");
                if key.is_null() {
                    logging::log(
                        "APP",
                        "frontmost_tracker_workspace_key_failed: operation=handle_app_activation_inner",
                    );
                    return;
                }
                let app: *mut Object = msg_send![user_info, objectForKey: key];

                if app.is_null() {
                    return;
                }

                let bundle_id = get_nsstring(msg_send![app, bundleIdentifier]);
                let name = get_nsstring(msg_send![app, localizedName]);
                let pid: i32 = msg_send![app, processIdentifier];

                if let Some(bundle_id) = bundle_id {
                    // Skip Script Kit itself
                    if bundle_id == SCRIPT_KIT_BUNDLE_ID
                        || bundle_id.contains("scriptkit")
                        || bundle_id.contains("script-kit")
                    {
                        logging::log("APP", "App activated: Script Kit (ignoring)");
                        return;
                    }

                    let tracked = TrackedApp {
                        pid,
                        bundle_id: bundle_id.clone(),
                        name: name.unwrap_or_else(|| bundle_id.clone()),
                        window_title: capture_window_title_for_pid(pid),
                    };

                    logging::log(
                        "APP",
                        &format!(
                            "App activated: {} ({}) PID {}",
                            tracked.name, tracked.bundle_id, tracked.pid
                        ),
                    );

                    // Activation also refreshes same-app metadata and menu contents;
                    // the existing bounded fetch fence prevents duplicate active work.
                    publish_tracked_app(tracked.clone());
                    fetch_menu_items_async(tracked.pid, tracked.bundle_id);
                }
            }

            decl.add_method(
                sel!(handleAppActivation:),
                handle_app_activation as extern "C" fn(&Object, Sel, *mut Object),
            );

            decl.register()
        };

        // Create an instance of our observer
        let observer: *mut Object = msg_send![observer_class, alloc];
        let observer: *mut Object = msg_send![observer, init];

        // Get the notification center
        let Some(workspace_class) = require_objc_class("NSWorkspace") else {
            return;
        };
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let notification_center: *mut Object = msg_send![workspace, notificationCenter];

        // Register for NSWorkspaceDidActivateApplicationNotification
        let notification_name = objc_nsstring("NSWorkspaceDidActivateApplicationNotification");
        if notification_name.is_null() {
            logging::log(
                "APP",
                "frontmost_tracker_notification_name_failed: operation=setup_workspace_observer",
            );
            return;
        }

        let _: () = msg_send![
            notification_center,
            addObserver: observer
            selector: sel!(handleAppActivation:)
            name: notification_name
            object: std::ptr::null::<c_void>()
        ];

        logging::log("APP", "NSWorkspace observer registered for app activation");

        // Run the run loop to receive notifications
        // This thread will run forever, receiving notifications
        let Some(run_loop_class) = require_objc_class("NSRunLoop") else {
            return;
        };
        let run_loop: *mut Object = msg_send![run_loop_class, currentRunLoop];
        let _: () = msg_send![run_loop, run];
    }
}
fn begin_menu_fetch(identity: &MenuCacheIdentity) -> Option<u64> {
    let mut state = TRACKER_STATE.write();
    if !tracked_app_matches(state.last_real_app.as_ref(), identity)
        || state.fetching_menu_identity.as_ref() == Some(identity)
    {
        return None;
    }
    let generation = advance_fetch_generation(&mut state);
    state.fetching_menu_identity = Some(identity.clone());
    drop(state);
    notify_menu_changed();
    Some(generation)
}

fn finish_menu_fetch(
    identity: &MenuCacheIdentity,
    generation: u64,
    result: anyhow::Result<Vec<MenuBarItem>>,
    elapsed: std::time::Duration,
) {
    let mut state = TRACKER_STATE.write();
    let active_request_matches = fetching_request_matches(&state, identity, generation);
    match result {
        Ok(items) => {
            if tracked_app_matches(state.last_real_app.as_ref(), identity) && active_request_matches
            {
                let count = items.len();
                state.cached_menu_items = items;
                state.cached_menu_identity = Some(identity.clone());
                logging::log(
                    "APP",
                    &format!(
                        "Published cached menu items for {} PID {} with {} top-level items in {:.2}ms",
                        identity.bundle_id, identity.pid, count, elapsed.as_secs_f64() * 1000.0
                    ),
                );
            } else {
                let reason = if !tracked_app_matches(state.last_real_app.as_ref(), identity) {
                    "the tracked app changed"
                } else {
                    "a newer menu fetch superseded it"
                };
                logging::log(
                    "APP",
                    &format!(
                        "Skipped cached menu publish for {} PID {} because {}",
                        identity.bundle_id, identity.pid, reason
                    ),
                );
            }
        }
        Err(error) => logging::log(
            "WARN",
            &format!(
                "Failed to pre-fetch menu items for {} PID {}: {}",
                identity.bundle_id, identity.pid, error
            ),
        ),
    }
    if active_request_matches {
        state.fetching_menu_identity = None;
        drop(state);
        notify_menu_changed();
    }
}

/// Fetch menu items asynchronously for the given app
#[cfg(target_os = "macos")]
fn fetch_menu_items_async(pid: i32, bundle_id: String) {
    let identity = MenuCacheIdentity {
        pid,
        bundle_id: bundle_id.clone(),
    };
    let Some(generation) = begin_menu_fetch(&identity) else {
        return;
    };

    std::thread::spawn(move || {
        let start = std::time::Instant::now();

        let result = get_menu_bar_for_pid(pid);
        finish_menu_fetch(&identity, generation, result, start.elapsed());
    });
}
/// Helper to convert NSString to Rust String
#[cfg(target_os = "macos")]
unsafe fn get_nsstring(nsstring: *mut objc::runtime::Object) -> Option<String> {
    use objc::{msg_send, sel, sel_impl};

    if nsstring.is_null() {
        return None;
    }

    let utf8: *const i8 = msg_send![nsstring, UTF8String];
    if utf8.is_null() {
        return None;
    }

    std::ffi::CStr::from_ptr(utf8)
        .to_str()
        .ok()
        .map(|s| s.to_string())
}
/// Helper to create an NSString from a Rust string
#[cfg(target_os = "macos")]
unsafe fn objc_nsstring(s: &str) -> *mut objc::runtime::Object {
    use objc::{msg_send, sel, sel_impl};

    let Some(nsstring_class) = require_objc_class("NSString") else {
        return std::ptr::null_mut();
    };
    let cstr = match make_objc_cstring(s) {
        Ok(value) => value,
        Err(error) => {
            logging::log(
                "APP",
                &format!(
                    "frontmost_tracker_invalid_nsstring_input: error={} operation=objc_nsstring",
                    error
                ),
            );
            return std::ptr::null_mut();
        }
    };
    msg_send![nsstring_class, stringWithUTF8String: cstr.as_ptr()]
}

// --- merged from part_001.rs ---
#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    static TRACKER_STATE_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn restore_tracker_state(
        last_real_app: Option<TrackedApp>,
        cached_menu_items: Vec<MenuBarItem>,
        cached_menu_identity: Option<MenuCacheIdentity>,
        fetching_menu_identity: Option<MenuCacheIdentity>,
        fetching_menu_generation: u64,
    ) {
        let mut state = TRACKER_STATE.write();
        state.last_real_app = last_real_app;
        state.cached_menu_items = cached_menu_items;
        state.cached_menu_identity = cached_menu_identity;
        state.fetching_menu_identity = fetching_menu_identity;
        state.fetching_menu_generation = fetching_menu_generation;
    }

    #[test]
    fn test_tracker_state_default() {
        let state = TrackerState::default();
        assert!(state.last_real_app.is_none());
        assert!(state.cached_menu_items.is_empty());
        assert!(state.cached_menu_identity.is_none());
        assert!(state.fetching_menu_identity.is_none());
        assert_eq!(state.fetching_menu_generation, 0);
    }

    #[test]
    fn test_fetching_menu_identity_race_condition_fix() {
        let mut state = TrackerState {
            fetching_menu_identity: Some(MenuCacheIdentity {
                pid: 1,
                bundle_id: "com.app.A".to_string(),
            }),
            ..Default::default()
        };
        assert_eq!(
            state.fetching_menu_identity.as_ref(),
            Some(&MenuCacheIdentity {
                pid: 1,
                bundle_id: "com.app.A".to_string(),
            })
        );

        state.fetching_menu_identity = Some(MenuCacheIdentity {
            pid: 2,
            bundle_id: "com.app.B".to_string(),
        });
        assert_eq!(
            state.fetching_menu_identity.as_ref(),
            Some(&MenuCacheIdentity {
                pid: 2,
                bundle_id: "com.app.B".to_string(),
            })
        );

        if state.fetching_menu_identity.as_ref()
            == Some(&MenuCacheIdentity {
                pid: 1,
                bundle_id: "com.app.A".to_string(),
            })
        {
            state.fetching_menu_identity = None;
        }
        assert_eq!(
            state.fetching_menu_identity.as_ref(),
            Some(&MenuCacheIdentity {
                pid: 2,
                bundle_id: "com.app.B".to_string(),
            }),
            "Thread A should not clear B's fetch state"
        );

        if state.fetching_menu_identity.as_ref()
            == Some(&MenuCacheIdentity {
                pid: 2,
                bundle_id: "com.app.B".to_string(),
            })
        {
            state.fetching_menu_identity = None;
        }
        assert!(
            state.fetching_menu_identity.is_none(),
            "Thread B should clear its own fetch state"
        );
    }

    #[test]
    fn test_fetching_menu_generation_blocks_stale_same_identity_completion() {
        let identity = MenuCacheIdentity {
            pid: 2,
            bundle_id: "com.app.A".to_string(),
        };
        let mut state = TrackerState {
            fetching_menu_identity: Some(identity.clone()),
            fetching_menu_generation: 2,
            ..Default::default()
        };

        assert!(
            !fetching_request_matches(&state, &identity, 1),
            "older same-identity fetches must not match the active request"
        );
        assert!(
            fetching_request_matches(&state, &identity, 2),
            "latest same-identity fetch must remain active"
        );

        if fetching_request_matches(&state, &identity, 1) {
            state.fetching_menu_identity = None;
        }
        assert_eq!(
            state.fetching_menu_identity.as_ref(),
            Some(&identity),
            "stale same-identity completion must not clear the active fetch"
        );
    }

    #[test]
    fn test_tracked_app_clone() {
        let app = TrackedApp {
            pid: 123,
            bundle_id: "com.test.app".to_string(),
            name: "Test App".to_string(),
            window_title: Some("Window".to_string()),
        };
        let cloned = app.clone();
        assert_eq!(cloned.pid, 123);
        assert_eq!(cloned.bundle_id, "com.test.app");
        assert_eq!(cloned.name, "Test App");
        assert_eq!(cloned.window_title.as_deref(), Some("Window"));
    }

    #[test]
    fn test_get_last_real_app_bundle_id_returns_none_when_not_set() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous_last_real_app = TRACKER_STATE.read().last_real_app.clone();
        let previous_cached_items = TRACKER_STATE.read().cached_menu_items.clone();
        let previous_cached_identity = TRACKER_STATE.read().cached_menu_identity.clone();
        let previous_fetching = TRACKER_STATE.read().fetching_menu_identity.clone();
        let previous_fetching_generation = TRACKER_STATE.read().fetching_menu_generation;

        let mut state = TRACKER_STATE.write();
        state.last_real_app = None;
        drop(state);

        assert_eq!(get_last_real_app_bundle_id(), None);

        restore_tracker_state(
            previous_last_real_app,
            previous_cached_items,
            previous_cached_identity,
            previous_fetching,
            previous_fetching_generation,
        );
    }

    #[test]
    fn test_get_last_real_app_bundle_id_returns_bundle_id_when_set() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous_last_real_app = TRACKER_STATE.read().last_real_app.clone();
        let previous_cached_items = TRACKER_STATE.read().cached_menu_items.clone();
        let previous_cached_identity = TRACKER_STATE.read().cached_menu_identity.clone();
        let previous_fetching = TRACKER_STATE.read().fetching_menu_identity.clone();
        let previous_fetching_generation = TRACKER_STATE.read().fetching_menu_generation;

        let mut state = TRACKER_STATE.write();
        state.last_real_app = Some(TrackedApp {
            pid: 42,
            bundle_id: "com.example.bundle".to_string(),
            name: "Example".to_string(),
            window_title: Some("Editor".to_string()),
        });
        drop(state);

        assert_eq!(
            get_last_real_app_bundle_id().as_deref(),
            Some("com.example.bundle")
        );

        restore_tracker_state(
            previous_last_real_app,
            previous_cached_items,
            previous_cached_identity,
            previous_fetching,
            previous_fetching_generation,
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_replace_cached_menu_items_updates_matching_tracked_app() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous_last_real_app = TRACKER_STATE.read().last_real_app.clone();
        let previous_cached_items = TRACKER_STATE.read().cached_menu_items.clone();
        let previous_cached_identity = TRACKER_STATE.read().cached_menu_identity.clone();
        let previous_fetching = TRACKER_STATE.read().fetching_menu_identity.clone();
        let previous_fetching_generation = TRACKER_STATE.read().fetching_menu_generation;

        {
            let mut state = TRACKER_STATE.write();
            state.last_real_app = Some(TrackedApp {
                pid: 42,
                bundle_id: "com.example.bundle".to_string(),
                name: "Example".to_string(),
                window_title: None,
            });
            state.cached_menu_items = Vec::new();
            state.cached_menu_identity = None;
            state.fetching_menu_identity = Some(MenuCacheIdentity {
                pid: 42,
                bundle_id: "com.example.bundle".to_string(),
            });
            state.fetching_menu_generation = 1;
        }

        let replacement = vec![MenuBarItem {
            title: "File".to_string(),
            enabled: true,
            shortcut: None,
            children: vec![],
            ax_element_path: vec![0],
        }];

        let older_subscription = subscribe_menu_changes();
        let newer_subscription = subscribe_menu_changes();
        replace_cached_menu_items(42, "com.example.bundle", replacement.clone());

        let state = TRACKER_STATE.read();
        assert_eq!(state.cached_menu_items, replacement);
        assert_eq!(
            state.cached_menu_identity.as_ref(),
            Some(&MenuCacheIdentity {
                pid: 42,
                bundle_id: "com.example.bundle".to_string(),
            })
        );
        assert!(state.fetching_menu_identity.is_none());
        drop(state);
        let mut latest_items = replacement;
        latest_items[0].title = "Edit".to_string();
        replace_cached_menu_items(42, "com.example.bundle", latest_items.clone());
        assert_eq!(older_subscription.receiver.try_recv(), Ok(()));
        assert_eq!(newer_subscription.receiver.try_recv(), Ok(()));
        assert_eq!(
            older_subscription.receiver.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        );
        assert_eq!(
            newer_subscription.receiver.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        );
        assert_eq!(get_cached_menu_snapshot().items, latest_items);

        replace_cached_menu_items(42, "com.example.bundle", latest_items.clone());
        assert_eq!(
            newer_subscription.receiver.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        );
        let retired_receiver = older_subscription.receiver.clone();
        drop(older_subscription);
        assert_eq!(
            retired_receiver.try_recv(),
            Err(async_channel::TryRecvError::Closed)
        );
        latest_items[0].enabled = false;
        replace_cached_menu_items(42, "com.example.bundle", latest_items.clone());
        assert_eq!(newer_subscription.receiver.try_recv(), Ok(()));
        assert_eq!(
            newer_subscription.receiver.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        );
        assert_eq!(get_cached_menu_snapshot().items, latest_items);

        restore_tracker_state(
            previous_last_real_app,
            previous_cached_items,
            previous_cached_identity,
            previous_fetching,
            previous_fetching_generation,
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_replace_cached_menu_items_ignores_stale_app() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous_last_real_app = TRACKER_STATE.read().last_real_app.clone();
        let previous_cached_items = TRACKER_STATE.read().cached_menu_items.clone();
        let previous_cached_identity = TRACKER_STATE.read().cached_menu_identity.clone();
        let previous_fetching = TRACKER_STATE.read().fetching_menu_identity.clone();
        let previous_fetching_generation = TRACKER_STATE.read().fetching_menu_generation;

        let existing = vec![MenuBarItem {
            title: "Edit".to_string(),
            enabled: true,
            shortcut: None,
            children: vec![],
            ax_element_path: vec![1],
        }];

        {
            let mut state = TRACKER_STATE.write();
            state.last_real_app = Some(TrackedApp {
                pid: 7,
                bundle_id: "com.example.current".to_string(),
                name: "Current".to_string(),
                window_title: None,
            });
            state.cached_menu_items = existing.clone();
            state.cached_menu_identity = Some(MenuCacheIdentity {
                pid: 7,
                bundle_id: "com.example.current".to_string(),
            });
            state.fetching_menu_identity = Some(MenuCacheIdentity {
                pid: 7,
                bundle_id: "com.example.current".to_string(),
            });
            state.fetching_menu_generation = 1;
        }

        let subscription = subscribe_menu_changes();
        replace_cached_menu_items(
            8,
            "com.example.other",
            vec![MenuBarItem {
                title: "File".to_string(),
                enabled: true,
                shortcut: None,
                children: vec![],
                ax_element_path: vec![0],
            }],
        );

        let state = TRACKER_STATE.read();
        assert_eq!(state.cached_menu_items, existing);
        assert_eq!(
            state.cached_menu_identity.as_ref(),
            Some(&MenuCacheIdentity {
                pid: 7,
                bundle_id: "com.example.current".to_string(),
            })
        );
        assert_eq!(
            state.fetching_menu_identity.as_ref(),
            Some(&MenuCacheIdentity {
                pid: 7,
                bundle_id: "com.example.current".to_string(),
            })
        );
        drop(state);
        assert_eq!(
            subscription.receiver.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        );

        restore_tracker_state(
            previous_last_real_app,
            previous_cached_items,
            previous_cached_identity,
            previous_fetching,
            previous_fetching_generation,
        );
    }

    #[test]
    fn test_tracked_app_metadata_and_relaunch_publish_latest_snapshot() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous = std::mem::take(&mut *TRACKER_STATE.write());
        let subscription = subscribe_menu_changes();
        let mut tracked = TrackedApp {
            pid: 100,
            bundle_id: "com.test.app".to_string(),
            name: "Test App".to_string(),
            window_title: Some("Main".to_string()),
        };
        publish_tracked_app(tracked.clone());
        assert_eq!(subscription.receiver.try_recv(), Ok(()));
        publish_tracked_app(tracked.clone());
        assert_eq!(
            subscription.receiver.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        );

        let identity = MenuCacheIdentity {
            pid: tracked.pid,
            bundle_id: tracked.bundle_id.clone(),
        };
        let items = vec![MenuBarItem {
            title: "File".to_string(),
            enabled: true,
            shortcut: None,
            children: vec![],
            ax_element_path: vec![0],
        }];
        let generation = begin_menu_fetch(&identity).unwrap();
        finish_menu_fetch(
            &identity,
            generation,
            Ok(items.clone()),
            std::time::Duration::ZERO,
        );
        assert_eq!(subscription.receiver.try_recv(), Ok(()));
        tracked.window_title = Some("Editor".to_string());
        publish_tracked_app(tracked.clone());
        assert_eq!(subscription.receiver.try_recv(), Ok(()));
        tracked.name = "Renamed App".to_string();
        publish_tracked_app(tracked.clone());
        assert_eq!(subscription.receiver.try_recv(), Ok(()));
        let snapshot = get_cached_menu_snapshot();
        assert_eq!(snapshot.app, Some(tracked.clone()));
        assert_eq!(
            snapshot.items, items,
            "metadata changes retain the matching menu cache"
        );

        tracked.pid = 200;
        publish_tracked_app(tracked.clone());
        assert_eq!(subscription.receiver.try_recv(), Ok(()));
        let snapshot = get_cached_menu_snapshot();
        assert_eq!(snapshot.app, Some(tracked));
        assert!(
            snapshot.items.is_empty(),
            "relaunch retires the old PID's menu cache"
        );
        assert!(!snapshot.is_fetching);
        assert_eq!(
            subscription.receiver.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        );
        restore_tracker_state(
            previous.last_real_app,
            previous.cached_menu_items,
            previous.cached_menu_identity,
            previous.fetching_menu_identity,
            previous.fetching_menu_generation,
        );
    }

    #[test]
    fn test_same_app_menu_fetch_failure_releases_exact_worker_for_retry() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous = std::mem::take(&mut *TRACKER_STATE.write());
        let tracked = TrackedApp {
            pid: 42,
            bundle_id: "com.example.bundle".to_string(),
            name: "Example".to_string(),
            window_title: None,
        };
        publish_tracked_app(tracked.clone());
        let identity = MenuCacheIdentity {
            pid: tracked.pid,
            bundle_id: tracked.bundle_id,
        };
        let items = vec![MenuBarItem {
            title: "File".to_string(),
            enabled: true,
            shortcut: None,
            children: vec![],
            ax_element_path: vec![0],
        }];
        let first = begin_menu_fetch(&identity).unwrap();
        finish_menu_fetch(
            &identity,
            first,
            Ok(items.clone()),
            std::time::Duration::ZERO,
        );
        let subscription = subscribe_menu_changes();
        let failed = begin_menu_fetch(&identity).unwrap();
        assert_eq!(subscription.receiver.try_recv(), Ok(()));
        assert_eq!(begin_menu_fetch(&identity), None);
        let other_identity = MenuCacheIdentity {
            pid: identity.pid + 1,
            bundle_id: identity.bundle_id.clone(),
        };
        assert_eq!(begin_menu_fetch(&other_identity), None);
        assert_eq!(
            subscription.receiver.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        );
        assert!(
            is_fetching_menu(),
            "rejected duplicate does not retire the live worker"
        );
        finish_menu_fetch(
            &identity,
            failed,
            Err(anyhow::anyhow!("menu read failed")),
            std::time::Duration::ZERO,
        );
        assert_eq!(subscription.receiver.try_recv(), Ok(()));
        assert!(!is_fetching_menu());
        assert_eq!(get_cached_menu_snapshot().items, items);

        let retry = begin_menu_fetch(&identity).unwrap();
        assert_eq!(subscription.receiver.try_recv(), Ok(()));
        finish_menu_fetch(&identity, failed, Ok(Vec::new()), std::time::Duration::ZERO);
        finish_menu_fetch(
            &identity,
            failed,
            Err(anyhow::anyhow!("late error")),
            std::time::Duration::ZERO,
        );
        assert_eq!(
            subscription.receiver.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        );
        assert!(
            is_fetching_menu(),
            "stale completion cannot release the retry worker"
        );
        assert_eq!(get_cached_menu_snapshot().items, items);
        finish_menu_fetch(&identity, retry, Ok(Vec::new()), std::time::Duration::ZERO);
        assert_eq!(subscription.receiver.try_recv(), Ok(()));
        assert!(!is_fetching_menu());
        assert!(get_cached_menu_snapshot().items.is_empty());
        restore_tracker_state(
            previous.last_real_app,
            previous.cached_menu_items,
            previous.cached_menu_identity,
            previous.fetching_menu_identity,
            previous.fetching_menu_generation,
        );
    }

    #[test]
    fn test_get_cached_menu_items_returns_empty_when_identity_is_stale() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous_last_real_app = TRACKER_STATE.read().last_real_app.clone();
        let previous_cached_items = TRACKER_STATE.read().cached_menu_items.clone();
        let previous_cached_identity = TRACKER_STATE.read().cached_menu_identity.clone();
        let previous_fetching = TRACKER_STATE.read().fetching_menu_identity.clone();
        let previous_fetching_generation = TRACKER_STATE.read().fetching_menu_generation;

        let cached_items = vec![MenuBarItem {
            title: "File".to_string(),
            enabled: true,
            shortcut: None,
            children: vec![],
            ax_element_path: vec![0],
        }];

        {
            let mut state = TRACKER_STATE.write();
            state.last_real_app = Some(TrackedApp {
                pid: 2,
                bundle_id: "com.example.bundle".to_string(),
                name: "Example".to_string(),
                window_title: None,
            });
            state.cached_menu_items = cached_items.clone();
            state.cached_menu_identity = Some(MenuCacheIdentity {
                pid: 1,
                bundle_id: "com.example.bundle".to_string(),
            });
            state.fetching_menu_identity = None;
        }

        assert!(
            get_cached_menu_items().is_empty(),
            "same-bundle cache must be hidden after a PID change"
        );

        {
            let mut state = TRACKER_STATE.write();
            state.cached_menu_identity = Some(MenuCacheIdentity {
                pid: 2,
                bundle_id: "com.example.bundle".to_string(),
            });
        }

        assert_eq!(get_cached_menu_items(), cached_items);

        restore_tracker_state(
            previous_last_real_app,
            previous_cached_items,
            previous_cached_identity,
            previous_fetching,
            previous_fetching_generation,
        );
    }

    #[test]
    fn test_get_cached_menu_snapshot_returns_items_when_identity_matches() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous_last_real_app = TRACKER_STATE.read().last_real_app.clone();
        let previous_cached_items = TRACKER_STATE.read().cached_menu_items.clone();
        let previous_cached_identity = TRACKER_STATE.read().cached_menu_identity.clone();
        let previous_fetching = TRACKER_STATE.read().fetching_menu_identity.clone();
        let previous_fetching_generation = TRACKER_STATE.read().fetching_menu_generation;

        let cached_items = vec![MenuBarItem {
            title: "File".to_string(),
            enabled: true,
            shortcut: Some(crate::menu_bar::KeyboardShortcut::new(
                "F".to_string(),
                crate::menu_bar::ModifierFlags::COMMAND,
            )),
            children: vec![],
            ax_element_path: vec![0],
        }];

        {
            let mut state = TRACKER_STATE.write();
            state.last_real_app = Some(TrackedApp {
                pid: 42,
                bundle_id: "com.example.bundle".to_string(),
                name: "Example".to_string(),
                window_title: Some("Window".to_string()),
            });
            state.cached_menu_items = cached_items.clone();
            state.cached_menu_identity = Some(MenuCacheIdentity {
                pid: 42,
                bundle_id: "com.example.bundle".to_string(),
            });
            state.fetching_menu_identity = None;
        }

        let snapshot = get_cached_menu_snapshot();
        assert_eq!(snapshot.status, CachedMenuStatus::Ready);
        assert!(!snapshot.is_fetching);
        assert_eq!(snapshot.items, cached_items);
        assert_eq!(snapshot.app.as_ref().map(|app| app.pid), Some(42));

        restore_tracker_state(
            previous_last_real_app,
            previous_cached_items,
            previous_cached_identity,
            previous_fetching,
            previous_fetching_generation,
        );
    }

    #[test]
    fn test_get_cached_menu_snapshot_hides_same_bundle_different_pid_cache() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous_last_real_app = TRACKER_STATE.read().last_real_app.clone();
        let previous_cached_items = TRACKER_STATE.read().cached_menu_items.clone();
        let previous_cached_identity = TRACKER_STATE.read().cached_menu_identity.clone();
        let previous_fetching = TRACKER_STATE.read().fetching_menu_identity.clone();
        let previous_fetching_generation = TRACKER_STATE.read().fetching_menu_generation;

        {
            let mut state = TRACKER_STATE.write();
            state.last_real_app = Some(TrackedApp {
                pid: 2,
                bundle_id: "com.example.bundle".to_string(),
                name: "Example".to_string(),
                window_title: None,
            });
            state.cached_menu_items = vec![MenuBarItem {
                title: "File".to_string(),
                enabled: true,
                shortcut: None,
                children: vec![],
                ax_element_path: vec![0],
            }];
            state.cached_menu_identity = Some(MenuCacheIdentity {
                pid: 1,
                bundle_id: "com.example.bundle".to_string(),
            });
            state.fetching_menu_identity = None;
        }

        let snapshot = get_cached_menu_snapshot();
        assert_eq!(snapshot.status, CachedMenuStatus::StaleCacheHidden);
        assert!(snapshot.items.is_empty());
        assert!(!snapshot.is_fetching);
        assert_eq!(snapshot.app.as_ref().map(|app| app.pid), Some(2));

        restore_tracker_state(
            previous_last_real_app,
            previous_cached_items,
            previous_cached_identity,
            previous_fetching,
            previous_fetching_generation,
        );
    }

    #[test]
    fn test_get_cached_menu_snapshot_reports_fetching_without_triggering_fetch() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous_last_real_app = TRACKER_STATE.read().last_real_app.clone();
        let previous_cached_items = TRACKER_STATE.read().cached_menu_items.clone();
        let previous_cached_identity = TRACKER_STATE.read().cached_menu_identity.clone();
        let previous_fetching = TRACKER_STATE.read().fetching_menu_identity.clone();
        let previous_fetching_generation = TRACKER_STATE.read().fetching_menu_generation;

        {
            let mut state = TRACKER_STATE.write();
            state.last_real_app = Some(TrackedApp {
                pid: 9,
                bundle_id: "com.example.fetching".to_string(),
                name: "Fetching".to_string(),
                window_title: None,
            });
            state.cached_menu_items = Vec::new();
            state.cached_menu_identity = None;
            state.fetching_menu_identity = Some(MenuCacheIdentity {
                pid: 9,
                bundle_id: "com.example.fetching".to_string(),
            });
            state.fetching_menu_generation = 99;
        }

        let snapshot = get_cached_menu_snapshot();
        assert_eq!(snapshot.status, CachedMenuStatus::Fetching);
        assert!(snapshot.is_fetching);
        assert!(snapshot.items.is_empty());
        assert_eq!(TRACKER_STATE.read().fetching_menu_generation, 99);

        restore_tracker_state(
            previous_last_real_app,
            previous_cached_items,
            previous_cached_identity,
            previous_fetching,
            previous_fetching_generation,
        );
    }

    #[test]
    fn test_get_cached_menu_snapshot_reports_no_tracked_app() {
        let _lock = TRACKER_STATE_TEST_LOCK.lock();
        let previous_last_real_app = TRACKER_STATE.read().last_real_app.clone();
        let previous_cached_items = TRACKER_STATE.read().cached_menu_items.clone();
        let previous_cached_identity = TRACKER_STATE.read().cached_menu_identity.clone();
        let previous_fetching = TRACKER_STATE.read().fetching_menu_identity.clone();
        let previous_fetching_generation = TRACKER_STATE.read().fetching_menu_generation;

        {
            let mut state = TRACKER_STATE.write();
            state.last_real_app = None;
            state.cached_menu_items = Vec::new();
            state.cached_menu_identity = None;
            state.fetching_menu_identity = None;
        }

        let snapshot = get_cached_menu_snapshot();
        assert_eq!(snapshot.status, CachedMenuStatus::NoTrackedApp);
        assert!(snapshot.app.is_none());
        assert!(snapshot.items.is_empty());

        restore_tracker_state(
            previous_last_real_app,
            previous_cached_items,
            previous_cached_identity,
            previous_fetching,
            previous_fetching_generation,
        );
    }

    #[test]
    fn test_make_objc_cstring_rejects_interior_nul() {
        let result = make_objc_cstring("bad\0value");
        assert!(result.is_err(), "interior NUL should be rejected");
    }

    #[test]
    fn test_make_objc_cstring_accepts_valid_string() {
        let result = make_objc_cstring("NSWorkspaceDidActivateApplicationNotification");
        assert!(result.is_ok(), "valid strings should convert to CString");
    }

    #[test]
    fn test_normalize_window_title_trims_and_drops_empty_values() {
        assert_eq!(
            normalize_window_title(Some("  README.md  ".to_string())).as_deref(),
            Some("README.md")
        );
        assert_eq!(normalize_window_title(Some("   ".to_string())), None);
        assert_eq!(normalize_window_title(None), None);
    }
}
